//! Helper methods only available for tests

use std::sync::Arc;

use assert_json_diff::assert_json_include;
use http::{Request, Response};
use hyper::{body::to_bytes, Body};
use kube::{Client, Resource, ResourceExt};
use prometheus::Registry;

use crate::{
    controller::{Context, Sinabro, SinabroSpec, SinabroStatus, SINABRO_FINALIZER},
    metrics::Metrics,
    Result,
};

impl Sinabro {
    /// A Sinabro that will cause the reconciler to fail
    pub fn illegal() -> Self {
        let mut s = Sinabro::new("illegal", SinabroSpec::default());
        s.meta_mut().namespace = Some("default".into());
        s
    }

    /// A normal test sinabro
    pub fn test() -> Self {
        let mut s = Sinabro::new("test", SinabroSpec::default());
        s.meta_mut().namespace = Some("default".into());
        s
    }

    /// Modify sinabro to be set to test
    pub fn needs_test(mut self) -> Self {
        self.spec.test = true;
        self
    }

    /// Modify sinabro to set a deletion timestamp
    pub fn needs_deletes(mut self) -> Self {
        use chrono::prelude::{DateTime, TimeZone, Utc};
        use k8s_openapi::apimachinery::pkg::apis::meta::v1::Time;

        let now: DateTime<Utc> = Utc.with_ymd_and_hms(2017, 4, 2, 12, 50, 32).unwrap();
        self.meta_mut().deletion_timestamp = Some(Time(now));
        self
    }

    /// Modify a sinabro to have the expected finalizer
    pub fn finalized(mut self) -> Self {
        self.finalizers_mut().push(SINABRO_FINALIZER.to_string());
        self
    }

    /// Modify a sinabro to have an expected status
    pub fn with_status(mut self, status: SinabroStatus) -> Self {
        self.status = Some(status);
        self
    }
}

// We wrap tower_test::mock::Handle
type ApiServerHandle = tower_test::mock::Handle<Request<Body>, Response<Body>>;
pub struct ApiServerVerifier(ApiServerHandle);

/// Scenarios we test for in ApiServerVerifier
pub enum Scenario {
    /// objects without finalizers will get a finalizer applied (and not call the apply loop)
    FinalizerCreation(Sinabro),
    /// objects that do not fail and do not cause publishes will only patch
    StatusPatch(Sinabro),
    /// finalized objects with test set causes both an event and then a test patch
    EventPublishThenStatusPatch(String, Sinabro),
    /// finalized objects "with errors" (i.e., the "illegal" object) will short circuit the apply loop
    RadioSilence,
    /// objects with a deletion timestamp will run the cleanup loop sending event and removing the finalizer
    Cleanup(String, Sinabro),
}

pub async fn timeout_after_1s(handle: tokio::task::JoinHandle<()>) {
    tokio::time::timeout(std::time::Duration::from_secs(1), handle)
        .await
        .expect("timeout on mock apiserver")
        .expect("scenario succeeded")
}

impl ApiServerVerifier {
    /// Tests only get to run specific scenarios that has matching handlers
    ///
    /// This setup makes it easy to handle multiple requests by chaining handlers together.
    ///
    /// If the controller is making more calls than we are handling in the scenario,
    /// you then typically see a `KubeError(Service(Closed(())))` from the reconciler.
    ///
    /// You should await the `JoinHandle` (with a timeout) from this function to ensure that
    /// the scenario runs to completion (i.e., all expected calls were responded to),
    /// using the timeout to catch missing api calls to Kubernetes.
    pub fn run(self, scenario: Scenario) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            // moving self => one scenario per test
            match scenario {
                Scenario::FinalizerCreation(sin) => self.handle_finalizer_creation(sin).await,
                Scenario::StatusPatch(sin) => self.handle_status_patch(sin).await,
                Scenario::EventPublishThenStatusPatch(reason, sin) => {
                    self.handle_event_create(reason)
                        .await
                        .unwrap()
                        .handle_status_patch(sin)
                        .await
                }
                Scenario::RadioSilence => Ok(self),
                Scenario::Cleanup(reason, sin) => {
                    self.handle_event_create(reason)
                        .await
                        .unwrap()
                        .handle_finalizer_removal(sin)
                        .await
                }
            }
            .expect("scenario completed without errors");
        })
    }

    // chainable scenario handlers

    async fn handle_finalizer_creation(mut self, sin: Sinabro) -> Result<Self> {
        let (req, send) = self.0.next_request().await.expect("service not called");
        // We expect a json patch to hte specified sinabro adding our finalizer
        assert_eq!(req.method(), http::Method::PATCH);
        assert_eq!(
            req.uri().to_string(),
            format!(
                "/apis/sinabro.io/v1alpha1/namespaces/default/sinabros/{}?",
                sin.name_any()
            )
        );

        let expected_patch = serde_json::json!([
            { "op": "test", "path": "/metadata/finalizers", "value": null },
            { "op": "add", "path": "/metadata/finalizers", "value": vec![SINABRO_FINALIZER] }
        ]);
        let req_body = to_bytes(req.into_body()).await.unwrap();
        let runtime_patch: serde_json::Value =
            serde_json::from_slice(&req_body).expect("valid sinabro from runtime");

        assert_json_include!(actual: runtime_patch, expected: expected_patch);

        // respond as the apiserver would have
        let resp = serde_json::to_vec(&sin.finalized()).unwrap();
        send.send_response(Response::builder().body(Body::from(resp)).unwrap());
        Ok(self)
    }

    async fn handle_finalizer_removal(mut self, sin: Sinabro) -> Result<Self> {
        let (req, send) = self.0.next_request().await.expect("service not called");
        // We expect a json path to the specified sinabro removing our finalizer (at index 0)
        assert_eq!(req.method(), http::Method::PATCH);
        assert_eq!(
            req.uri().to_string(),
            format!(
                "/apis/sinabro.io/v1alpha1/namespaces/default/sinabros/{}?",
                sin.name_any()
            )
        );

        let expected_patch = serde_json::json!([
            { "op": "test", "path": "/metadata/finalizers/0", "value": SINABRO_FINALIZER },
            { "op": "remove", "path": "/metadata/finalizers/0", "path": "/metadata/finalizers/0" }
        ]);
        let req_body = to_bytes(req.into_body()).await.unwrap();
        let runtime_patch: serde_json::Value =
            serde_json::from_slice(&req_body).expect("valid sinabro form runtime");

        assert_json_include!(actual: runtime_patch, expected: expected_patch);

        // respond as the apiserver would have
        let resp = serde_json::to_vec(&sin).unwrap();
        send.send_response(Response::builder().body(Body::from(resp)).unwrap());
        Ok(self)
    }

    async fn handle_event_create(mut self, reason: String) -> Result<Self> {
        let (req, send) = self.0.next_request().await.expect("service not called");
        assert_eq!(req.method(), http::Method::POST);
        assert_eq!(
            req.uri().to_string(),
            format!("/apis/events.k8s.io/v1/namespaces/default/events?")
        );

        // verify the event reason matches the expected
        let req_body = to_bytes(req.into_body()).await.unwrap();
        let post_data: serde_json::Value =
            serde_json::from_slice(&req_body).expect("valid event from runtime");
        dbg!("post_data for event: {}", post_data.clone());
        assert_eq!(
            post_data.get("reason").unwrap().as_str().map(String::from),
            Some(reason)
        );

        // then pass through the body
        send.send_response(Response::builder().body(Body::from(req_body)).unwrap());
        Ok(self)
    }

    async fn handle_status_patch(mut self, sin: Sinabro) -> Result<Self> {
        let (req, send) = self.0.next_request().await.expect("service not called");
        assert_eq!(req.method(), http::Method::PATCH);
        assert_eq!(
            req.uri().to_string(),
            format!(
                "/apis/sinabro.io/v1alpha1/namespaces/default/sinabros/{}/status?&force=true&fieldManager=cntrlr",
                sin.name_any()
            )
        );

        let req_body = to_bytes(req.into_body()).await.unwrap();
        let json: serde_json::Value =
            serde_json::from_slice(&req_body).expect("patch_status object is json");
        let status_json = json.get("status").expect("status object").clone();
        let status: SinabroStatus = serde_json::from_value(status_json).expect("valid status");
        assert_eq!(
            status.tested, sin.spec.test,
            "status.tested iff sin.spec.test"
        );

        let resp = serde_json::to_vec(&sin.with_status(status)).unwrap();
        // pass throgh sinabro "patch accepted"
        send.send_response(Response::builder().body(Body::from(resp)).unwrap());
        Ok(self)
    }
}

impl Context {
    // create a test context with a mocked kube client, locally registered metrics and default diagnostics
    pub fn test() -> (Arc<Self>, ApiServerVerifier, Registry) {
        let (mock_service, handle) = tower_test::mock::pair::<Request<Body>, Response<Body>>();
        let mock_client = Client::new(mock_service, "default");
        let registry = Registry::default();
        let ctx = Self {
            client: mock_client,
            metrics: Metrics::default().register(&registry).unwrap(),
            diagnostics: Arc::default(),
        };
        (Arc::new(ctx), ApiServerVerifier(handle), registry)
    }
}
