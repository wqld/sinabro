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

#[instrument(skip(ctx, sinabro), fields(trace_id))]
async fn reconcile(sinabro: Arc<Sinabro>, ctx: Arc<Context>) -> Result<Action> {
    let trace_id = telemetry::get_trace_id();
    Span::current().record("trace_id", &field::display(&trace_id));
    let _timer = ctx.metrics.count_and_measure();
    ctx.diagnostics.write().await.last_event = Utc::now();

    // Sinabro is namespace scoped
    let ns = sinabro.namespace().unwrap();
    let sinabros: Api<Sinabro> = Api::namespaced(ctx.client.clone(), &ns);

    info!("Reconciling Sinabro '{}' in {}", sinabro.name_any(), ns);

    finalizer(&sinabros, SINABRO_FINALIZER, sinabro, |event| async {
        match event {
            Finalizer::Apply(sinabro) => sinabro.reconcile(ctx.clone()).await,
            Finalizer::Cleanup(sinabro) => sinabro.cleanup(ctx.clone()).await,
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
        let sinabros: Api<Sinabro> = Api::namespaced(client, &ns);

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
        let _o = sinabros
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
    fn recorder(&self, client: Client, sinabro: &Sinabro) -> Recorder {
        Recorder::new(client, self.reporter.clone(), sinabro.object_ref(&()))
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

fn error_policy(sinabro: Arc<Sinabro>, error: &Error, ctx: Arc<Context>) -> Action {
    warn!("reconcile failed: {error:?}");
    ctx.metrics.reconcile_failure(&sinabro, error);
    Action::requeue(Duration::from_secs(5 * 60))
}

/// Initialize the controller and shared state (given the CRD is installed)
pub async fn run(state: State) {
    let client = Client::try_default()
        .await
        .expect("failed to create kube client");
    let sinabros = Api::<Sinabro>::all(client.clone());

    if let Err(e) = sinabros.list(&ListParams::default().limit(1)).await {
        error!("CRD is not queryable; {e:?}. Is the CRD installed?");
        info!("Installation: cargo run --bin crdgen | kubectl apply -f -");
        std::process::exit(1);
    }

    Controller::new(sinabros, Config::default().any_semantic())
        .shutdown_on_signal()
        .run(reconcile, error_policy, state.to_context(client))
        .filter_map(|x| async move { std::result::Result::ok(x) })
        .for_each(|_| futures::future::ready(()))
        .await;
}
