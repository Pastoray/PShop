use std::{fmt::Display, future::Future, time::Duration};

/// Retry with capped exponential backoff. Used for *startup* dependencies
/// (broker not up yet, LocalStack still applying Terraform, ...). Hot paths
/// must NOT use this: they fail fast and let backpressure do its job.
pub async fn retry<T, E, Fut>(
    what: &str,
    attempts: u32,
    base: Duration,
    mut f: impl FnMut() -> Fut,
) -> Result<T, E>
where
    Fut: Future<Output = Result<T, E>>,
    E: Display,
{
    let mut delay = base;
    let mut n = 0;
    loop {
        n += 1;
        match f().await {
            Ok(v) => return Ok(v),
            Err(e) if n >= attempts => return Err(e),
            Err(e) => {
                tracing::warn!(error = %e, attempt = n, "{what} failed; retrying in {delay:?}");
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(5));
            }
        }
    }
}

/// Resolves on SIGINT or SIGTERM (Kubernetes sends SIGTERM on pod stop).
pub async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let term = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = term => {} }
    tracing::info!("shutdown signal received");
}
