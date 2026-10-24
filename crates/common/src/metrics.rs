//! Prometheus plumbing shared by all services.
//!
//! Each service registers its own counters/histograms on the default registry
//! (see `LazyLock` statics in each crate) and mounts `base_router()`.

use axum::{
    http::{header, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use prometheus::TextEncoder;

/// Buckets tuned around the 5 ms acceptance SLO of the order path.
pub const FAST_BUCKETS: &[f64] = &[
    0.0005, 0.001, 0.002, 0.003, 0.005, 0.0075, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// For database / object-store / downstream calls.
pub const SLOW_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Exposes `process_resident_memory_bytes`, `process_cpu_seconds_total`,
/// open fds, threads... (the "memory buffers" panel in Grafana).
pub fn init_process_metrics() {
    #[cfg(target_os = "linux")]
    {
        let _ = prometheus::default_registry().register(Box::new(
            prometheus::process_collector::ProcessCollector::for_self(),
        ));
    }
}

pub fn render() -> String {
    let mut buf = String::new();
    let families = prometheus::gather();
    if let Err(e) = TextEncoder::new().encode_utf8(&families, &mut buf) {
        tracing::error!(error = %e, "failed to encode metrics");
    }
    buf
}

async fn metrics_handler() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        render(),
    )
}

/// `/metrics` + `/healthz` (liveness: the process is up and the runtime is
/// scheduling tasks). Readiness is service-specific.
pub fn base_router() -> Router {
    Router::new()
        .route("/metrics", get(metrics_handler))
        .route("/healthz", get(|| async { "ok" }))
}

pub async fn serve(addr: &str, app: Router) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "http listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(crate::util::shutdown_signal())
        .await?;
    Ok(())
}
