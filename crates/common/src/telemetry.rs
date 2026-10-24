use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// JSON logs when `LOG_FORMAT=json` (Kubernetes), compact text otherwise.
pub fn init_tracing(service: &'static str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let registry = tracing_subscriber::registry().with(filter);
    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        registry
            .with(fmt::layer().json().with_current_span(false))
            .init();
    } else {
        registry.with(fmt::layer().compact()).init();
    }
    crate::metrics::init_process_metrics();
    tracing::info!(service, "service starting");
}
