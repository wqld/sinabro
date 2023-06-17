use crate::{metrics::Metrics, telemetry, Error, Result};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use kube::{
    api::{Api, ListParams, Patch, PatchParams, ResourceExt},
    client::Client,
    runtime::{
        controller::{Action, Controller},
        events::{Event, EventType, Recorder, Reporter},
        finalizer::{finalizer, Event as Finalizer},
        watcher::Config,
    },
    CustomResource, Resource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::{sync::RwLock, time::Duration};
use tracing::*;

pub static SINABRO_FINALIZER: &str = "sinabro.io";

/// Generate the Kubernetes wrapper struct `Sinabro` from Spec and Status struct
///
/// This provides a hook for generating the CRD yaml (in crdgen.rs)
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, JsonSchema)]
#[cfg_attr(test, derive(Default))]
#[kube(
    kind = "Sinabro",
    group = "sinabro.io",
    version = "v1alpha1",
    namespaced
)]
#[kube(status = "SinabroStatus", shortname = "sin")]
pub struct SinabroSpec {
    pub test: bool,
}

/// The status object of `Sinabro`
#[derive(Deserialize, Serialize, Clone, Default, Debug, JsonSchema)]
pub struct SinabroStatus {
    pub tested: bool,
}

impl Sinabro {
    fn was_tested(&self) -> bool {
        self.status.as_ref().map(|s| s.tested).unwrap_or(false)
    }
}

// Context for reconciler
pub struct Context {
    /// Kubernetes client
    pub client: Client,
    /// Diagnostics read by the web server
    pub diagnostics: Arc<RwLock<Diagnostics>>,
    /// Prometheus metrics
    pub metrics: Metrics,
}

#[instrument(skip(ctx, sin), fields(trace_id))]
async fn reconcile(sin: Arc<Sinabro>, ctx: Arc<Context>) -> Result<Action> {
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();

    // Sinabro is namespace scoped
    let ns = sin.namespace().unwrap();
    let sins: Api<Sinabro> = Api::namespaced(ctx.client.clone(), &ns);

    info!("Reconciling Sinabro '{}' in {}", sin.name_any(), ns);

    finalizer(&sins, SINABRO_FINALIZER, sin, |event| async {
        match event {
            Finalizer::Apply(sin) => sin.reconcile(ctx.clone()).await,
            Finalizer::Cleanup(sin) => sin.cleanup(ctx.clone()).await,
        }
    })
    .await
    .map_err(|e| Error::FinalizerError(Box::new(e)))
}

impl Sinabro {
    // Reconcile (for non-finalizer related changes)
    async fn reconcile(&self, ctx: Arc<Context>) -> Result<Action> {
        let client = ctx.client.clone();
        let recorder = ctx.diagnostics.read().await.recorder(client.clone(), self);
        let ns = self.namespace().unwrap();
        let name = self.name_any();
        let sins: Api<Sinabro> = Api::namespaced(client, &ns);

        let should_test = self.spec.test;
        if !self.was_tested() && should_test {
            // Send an event once per test
            recorder
                .publish(Event {
                    type_: EventType::Normal,
                    reason: "TestRequested".into(),
                    note: Some(format!("Testing `{name}`")),
                    action: "Testing".into(),
                    secondary: None,
                })
                .await
                .map_err(Error::KubeError)?;
        }

        // Error names show up in metrics
        if name == "illegal" {
            return Err(Error::IllegalSinabro);
        }

        // Always overwrite status object with what we saw
        let new_status = Patch::Apply(json!({
            "apiVersion": "sinabro.io/v1alpha1",
            "kind": "Sinabro",
            "status": SinabroStatus {
                tested: should_test,
            }
        }));

        let ps = PatchParams::apply("cntrlr").force();
        let _o = sins
            .patch_status(&name, &ps, &new_status)
            .await
            .map_err(Error::KubeError)?;

        // If no events were received, check back every 5 minutes
        Ok(Action::requeue(Duration::from_secs(5 * 60)))
    }

    // Finalizer cleanup (the object was deleted, ensure nothing is orphaned)
    async fn cleanup(&self, ctx: Arc<Context>) -> Result<Action> {
        let recorder = ctx
            .diagnostics
            .read()
            .await
            .recorder(ctx.client.clone(), self);
        // Sinabro doesn't have any real cleanup, so we just publish an event
        recorder
            .publish(Event {
                type_: EventType::Normal,
                reason: "DeleteRequested".into(),
                note: Some(format!("Delete `{}`", self.name_any())),
                action: "Deleting".into(),
                secondary: None,
            })
            .await
            .map_err(Error::KubeError)?;
        Ok(Action::await_change())
    }
}

/// Diagnostics to be exposed by the web server
#[derive(Clone, Serialize)]
pub struct Diagnostics {
    #[serde(deserialize_with = "from_ts")]
    pub last_event: DateTime<Utc>,
    #[serde(skip)]
    pub reporter: Reporter,
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self {
            last_event: Utc::now(),
            reporter: "sinabro-controller".into(),
        }
    }
}

impl Diagnostics {
    fn recorder(&self, client: Client, sin: &Sinabro) -> Recorder {
        Recorder::new(client, self.reporter.clone(), sin.object_ref(&()))
    }
}

/// State shared between the controller and the web server
#[derive(Clone, Default)]
pub struct State {
    /// Diagnostics populated by the reconciler
    diagnostics: Arc<RwLock<Diagnostics>>,
    /// Metrics registry
    registry: prometheus::Registry,
}

/// State wrapper around the controller outputs for the web server
impl State {
    /// Metrics getter
    pub fn metrics(&self) -> Vec<prometheus::proto::MetricFamily> {
        self.registry.gather()
    }

    /// State getter
    pub async fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.read().await.clone()
    }

    // Create a Controller Context that can update State
    pub fn to_context(&self, client: Client) -> Arc<Context> {
        Arc::new(Context {
            client,
            metrics: Metrics::default().register(&self.registry).unwrap(),
            diagnostics: self.diagnostics.clone(),
        })
    }
}

fn error_policy(sin: Arc<Sinabro>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {error:?}");
    ctx.metrics.reconcile_failure(&sin, error);
    Action::requeue(Duration::from_secs(5 * 60))
}

/// Initialize the controller and shared state (given the CRD is installed)
pub async fn run(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube client");
    let sins = Api::<Sinabro>::all(client.clone());

    if let Err(e) = sins.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    Controller::new(sins, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_context(client))
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}

// Mock tests relying on fixtures.rs and its primitive apiserver mocks
#[cfg(test)]
mod test {
    use crate::fixtures::{timeout_after_1s, Scenario};

    use super::*;

    #[tokio::test]
    async fn sinabro_without_finalizer_gets_a_finalizer() {
        let (ctx, server, _) = Context::test();
        let sin = Sinabro::test();
        let mock_server = server.run(Scenario::FinalizerCreation(sin.clone()));

        reconcile(Arc::new(sin), ctx).await.expect("reconciler");
        timeout_after_1s(mock_server).await;
    }

    #[tokio::test]
    async fn finalized_sin_causes_status_patch() {
        let (ctx, server, _) = Context::test();
        let sin = Sinabro::test().finalized();
        let mock_server = server.run(Scenario::StatusPatch(sin.clone()));

        reconcile(Arc::new(sin), ctx).await.expect("reconciler");
        timeout_after_1s(mock_server).await;
    }

    #[tokio::test]
    async fn finalized_sin_with_test_causes_event_and_test_patch() {
        let (ctx, server, _) = Context::test();
        let sin = Sinabro::test().finalized().needs_test();
        let scenario = Scenario::EventPublishThenStatusPatch("TestRequested".into(), sin.clone());
        let mock_server = server.run(scenario);

        reconcile(Arc::new(sin), ctx).await.expect("reconciler");
        timeout_after_1s(mock_server).await;
    }

    #[tokio::test]
    async fn finalized_sin_with_delete_timestamp_causes_delete() {
        let (ctx, server, _) = Context::test();
        let sin = Sinabro::test().finalized().needs_deletes();
        let mock_server = server.run(Scenario::Cleanup("DeleteRequested".into(), sin.clone()));

        reconcile(Arc::new(sin), ctx).await.expect("reconciler");
        timeout_after_1s(mock_server).await;
    }

    #[tokio::test]
    async fn illegal_sin_reconcile_errors_which_bumps_failure_metrics() {
        let (ctx, server, _registry) = Context::test();
        let sin = Arc::new(Sinabro::illegal().finalized());
        let mock_server = server.run(Scenario::RadioSilence);

        let res = reconcile(sin.clone(), ctx.clone()).await;
        timeout_after_1s(mock_server).await;
        assert!(res.is_err(), "apply reconciler fails on illegal sin");

        let err = res.unwrap_err();
        assert!(err.to_string().contains("Illegal Sinabro"));

        // calling error policy with the reconciler error should cause the correct metric to be set
        error_policy(sin.clone(), &err, ctx.clone());

        let failures = ctx
            .metrics
            .failures
            .with_label_values(&["illegal", "finalizererror(applyfailed(illegalsinabro))"])
            .get();
        assert_eq!(failures, 1);
    }

    // Integration test without mocks
    use kube::api::{Api, ListParams, Patch, PatchParams};
    #[tokio::test]
    #[ignore = "uses k8s current-context"]
    async fn integration_reconcile_should_set_status_and_send_event() {
        let client = kube::Client::try_default().await.unwrap();
        let ctx = super::State::default().to_context(client.clone());

        // create a test sin
        let sin = Sinabro::test().finalized().needs_test();
        let sins: Api<Sinabro> = Api::namespaced(client.clone(), "default");
        let params = PatchParams::apply("ctrltest");
        let patch = Patch::Apply(sin.clone());
        sins.patch("test", &params, &patch).await.unwrap();

        // reconcile it (as if it was just applied to the cluster like this)
        reconcile(Arc::new(sin), ctx).await.unwrap();

        // verify side-effects happened
        let output = sins.get_status("test").await.unwrap();
        assert!(output.status.is_some());

        // verify test event was found
        let events: Api<k8s_openapi::api::core::v1::Event> = Api::all(client.clone());
        let opts =
            ListParams::default().fields("involvedObject.kund=Sinabro,involvedObject.name=test");
        let event = events
            .list(&opts)
            .await
            .unwrap()
            .into_iter()
            .filter(|e| e.reason.as_deref() == Some("TestRequested"))
            .last()
            .unwrap();
        dbg!("got ev: {:?}", &event);
        assert_eq!(event.action.as_deref(), Some("Testing"));
    }
}
