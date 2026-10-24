//! Metrics Gateway: Prometheus -> WebSocket bridge for the admin dashboard.
//!
//! One poller queries Prometheus on a fixed cadence and broadcasts the serialized
//! snapshot to every connected socket. N dashboards therefore cost the same
//! Prometheus load as one, and a slow browser can only hurt itself (it lags and
//! skips frames; it never back-pressures the poller).

use std::{
    collections::BTreeMap,
    sync::{Arc, LazyLock, RwLock},
    time::Duration,
};

use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use prometheus::{register_int_counter, register_int_gauge, IntCounter, IntGauge};
use pshop_common::{
    config::{env_or, env_parse},
    metrics, telemetry,
};
use serde::Serialize;
use serde_json::Value;
use tokio::sync::broadcast;

static WS_CLIENTS: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!("pshop_gateway_ws_clients", "Connected dashboard sockets").unwrap()
});
static QUERY_FAILURES: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_gateway_prometheus_query_failures_total",
        "Failed PromQL queries"
    )
    .unwrap()
});

// ---------------------------------------------------------------- snapshot ---

#[derive(Serialize, Default, Clone)]
struct Snapshot {
    ts_ms: i64,
    prometheus_ok: bool,
    throughput: Throughput,
    latency: Latency,
    queues: Queues,
    redis: RedisStats,
    rejected_by_reason: BTreeMap<String, f64>,
    memory_by_service: BTreeMap<String, f64>,
    health: Vec<Health>,
}

#[derive(Serialize, Default, Clone)]
struct Throughput {
    accepted_rps: Option<f64>,
    rejected_rps: Option<f64>,
    payments_ok_rps: Option<f64>,
    events_published_rps: Option<f64>,
}

#[derive(Serialize, Default, Clone)]
struct Latency {
    accept_p50_ms: Option<f64>,
    accept_p99_ms: Option<f64>,
    db_write_p95_ms: Option<f64>,
    end_to_end_p95_s: Option<f64>,
}

#[derive(Serialize, Default, Clone)]
struct Queues {
    rabbit_ready: Option<f64>,
    rabbit_unacked: Option<f64>,
    outbox_pending: Option<f64>,
    es_sync_lag_s: Option<f64>,
    dlq_5m: Option<f64>,
}

#[derive(Serialize, Default, Clone)]
struct RedisStats {
    ops_per_sec: Option<f64>,
    memory_bytes: Option<f64>,
}

#[derive(Serialize, Clone)]
struct Health {
    name: String,
    up: f64,
    total: f64,
    /// From kube-state-metrics when running in Kubernetes.
    replicas_available: Option<f64>,
    replicas_desired: Option<f64>,
}

// -------------------------------------------------------------- prometheus ---

struct Prom {
    http: reqwest::Client,
    base: String,
}

fn quantile(q: f64, hist: &str, window: &str) -> String {
    format!("histogram_quantile({q}, sum by (le) (rate({hist}_bucket[{window}])))")
}

impl Prom {
    async fn raw(&self, promql: &str) -> Result<Vec<(BTreeMap<String, String>, f64)>> {
        let url = reqwest::Url::parse_with_params(
            &format!("{}/api/v1/query", self.base),
            &[("query", promql)],
        )?;
        let v: Value = self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        parse_vector(&v)
    }

    async fn scalar(&self, promql: &str) -> Option<f64> {
        match self.raw(promql).await {
            Ok(r) => r.first().map(|(_, v)| *v).filter(|v| v.is_finite()),
            Err(e) => {
                QUERY_FAILURES.inc();
                tracing::debug!(error = %e, promql, "query failed");
                None
            }
        }
    }

    async fn by_label(&self, promql: &str, label: &str) -> BTreeMap<String, f64> {
        match self.raw(promql).await {
            Ok(r) => r
                .into_iter()
                .filter(|(_, v)| v.is_finite())
                .filter_map(|(m, v)| m.get(label).map(|l| (l.clone(), v)))
                .collect(),
            Err(e) => {
                QUERY_FAILURES.inc();
                tracing::debug!(error = %e, promql, "query failed");
                BTreeMap::new()
            }
        }
    }

    async fn snapshot(&self) -> Snapshot {
        let accept = "pshop_order_accept_seconds";
        // Bound to locals: `join!` borrows them for the whole duration of the futures.
        let q_p50 = quantile(0.5, accept, "15s");
        let q_p99 = quantile(0.99, accept, "15s");
        let q_db95 = quantile(0.95, "pshop_payment_db_write_seconds", "30s");
        let q_e2e95 = quantile(0.95, "pshop_order_end_to_end_seconds", "30s");
        let (
            accepted,
            rejected,
            pay_ok,
            published,
            p50,
            p99,
            db95,
            e2e95,
            rb_ready,
            rb_unacked,
            outbox,
            lag,
            dlq,
            redis_ops,
            redis_mem,
        ) = tokio::join!(
            self.scalar("sum(rate(pshop_orders_accepted_total[15s]))"),
            self.scalar("sum(rate(pshop_orders_rejected_total[15s]))"),
            self.scalar("sum(rate(pshop_payments_processed_total{outcome=\"ok\"}[15s]))"),
            self.scalar("sum(rate(pshop_outbox_published_total[15s]))"),
            self.scalar(&q_p50),
            self.scalar(&q_p99),
            self.scalar(&q_db95),
            self.scalar(&q_e2e95),
            self.scalar("sum(rabbitmq_queue_messages_ready{queue=\"payments.process\"})"),
            self.scalar("sum(rabbitmq_queue_messages_unacked{queue=\"payments.process\"})"),
            self.scalar("max(pshop_outbox_pending)"),
            self.scalar("max(pshop_search_sync_lag_seconds)"),
            self.scalar("sum(increase(pshop_dlq_messages_total[5m]))"),
            self.scalar("sum(rate(redis_commands_processed_total[15s]))"),
            self.scalar("sum(redis_memory_used_bytes)"),
        );
        let (reasons, mem, up, total, avail, desired) = tokio::join!(
            self.by_label("sum by (reason) (rate(pshop_orders_rejected_total[15s]))", "reason"),
            self.by_label("sum by (job) (process_resident_memory_bytes{job=~\"order-service|billing-service|search-sync|metrics-gateway\"})", "job"),
            self.by_label("sum by (job) (up)", "job"),
            self.by_label("count by (job) (up)", "job"),
            self.by_label("sum by (deployment) (kube_deployment_status_replicas_available{namespace=\"pshop\"})", "deployment"),
            self.by_label("sum by (deployment) (kube_deployment_spec_replicas{namespace=\"pshop\"})", "deployment"),
        );

        let health = total
            .iter()
            .map(|(job, t)| Health {
                name: job.clone(),
                up: up.get(job).copied().unwrap_or(0.0),
                total: *t,
                replicas_available: avail.get(job).copied(),
                replicas_desired: desired.get(job).copied(),
            })
            .collect();

        Snapshot {
            ts_ms: chrono_now_ms(),
            prometheus_ok: accepted.is_some() || !up.is_empty(),
            throughput: Throughput {
                accepted_rps: accepted,
                rejected_rps: rejected,
                payments_ok_rps: pay_ok,
                events_published_rps: published,
            },
            latency: Latency {
                accept_p50_ms: p50.map(|s| s * 1000.0),
                accept_p99_ms: p99.map(|s| s * 1000.0),
                db_write_p95_ms: db95.map(|s| s * 1000.0),
                end_to_end_p95_s: e2e95,
            },
            queues: Queues {
                rabbit_ready: rb_ready,
                rabbit_unacked: rb_unacked,
                outbox_pending: outbox,
                es_sync_lag_s: lag,
                dlq_5m: dlq,
            },
            redis: RedisStats {
                ops_per_sec: redis_ops,
                memory_bytes: redis_mem,
            },
            rejected_by_reason: reasons,
            memory_by_service: mem,
            health,
        }
    }
}

fn chrono_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn parse_vector(v: &Value) -> Result<Vec<(BTreeMap<String, String>, f64)>> {
    if v["status"] != "success" {
        anyhow::bail!("prometheus error: {}", v["error"]);
    }
    let mut out = Vec::new();
    for r in v["data"]["result"].as_array().into_iter().flatten() {
        let labels: BTreeMap<String, String> = r["metric"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_owned())))
                    .collect()
            })
            .unwrap_or_default();
        let val = r["value"][1]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(f64::NAN);
        out.push((labels, val));
    }
    Ok(out)
}

// --------------------------------------------------------------------- web ---

#[derive(Clone)]
struct AppState {
    latest: Arc<RwLock<Arc<str>>>,
    tx: broadcast::Sender<Arc<str>>,
    allowed_origins: Arc<Vec<String>>,
}

/// Blocks cross-site WebSocket hijacking. An empty allow-list disables the check
/// (fine when only the same-origin ingress can reach the gateway).
fn origin_ok(headers: &HeaderMap, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }
    match headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
        Some(o) => allowed.iter().any(|a| a == o),
        None => true, // non-browser clients (curl, tests) send no Origin
    }
}

async fn ws_handler(
    State(st): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response {
    if !origin_ok(&headers, &st.allowed_origins) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| client_loop(socket, st))
}

async fn client_loop(mut socket: WebSocket, st: AppState) {
    WS_CLIENTS.inc();
    let mut rx = st.tx.subscribe();
    let first = st.latest.read().unwrap().clone();
    if !first.is_empty() && socket.send(Message::text(first.to_string())).await.is_err() {
        WS_CLIENTS.dec();
        return;
    }
    loop {
        tokio::select! {
            msg = rx.recv() => match msg {
                Ok(s) => if socket.send(Message::text(s.to_string())).await.is_err() { break },
                Err(broadcast::error::RecvError::Lagged(_)) => continue, // slow client: skip frames
                Err(_) => break,
            },
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {} // ignore pings/text from the client
            },
        }
    }
    WS_CLIENTS.dec();
}

async fn snapshot_handler(State(st): State<AppState>) -> impl IntoResponse {
    let body = st.latest.read().unwrap().to_string();
    (
        [(header::CONTENT_TYPE, "application/json")],
        if body.is_empty() {
            "{}".to_owned()
        } else {
            body
        },
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init_tracing("metrics-gateway");
    LazyLock::force(&WS_CLIENTS);
    LazyLock::force(&QUERY_FAILURES);

    let prom = Arc::new(Prom {
        http: reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()?,
        base: env_or("PROM_URL", "http://prometheus:9090")
            .trim_end_matches('/')
            .to_owned(),
    });
    let interval = Duration::from_millis(env_parse("POLL_MS", 1000));
    let (tx, _) = broadcast::channel::<Arc<str>>(16);
    let latest: Arc<RwLock<Arc<str>>> = Arc::new(RwLock::new(Arc::from("")));

    {
        let (tx, latest, prom) = (tx.clone(), Arc::clone(&latest), Arc::clone(&prom));
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(interval);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                let snap = prom.snapshot().await;
                if let Ok(json) = serde_json::to_string(&snap) {
                    let json: Arc<str> = Arc::from(json);
                    *latest.write().unwrap() = Arc::clone(&json);
                    let _ = tx.send(json); // Err just means "no listeners"
                }
            }
        });
    }

    let allowed: Vec<String> = env_or("ALLOWED_ORIGINS", "")
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/api/snapshot", get(snapshot_handler))
        .with_state(AppState {
            latest,
            tx,
            allowed_origins: Arc::new(allowed),
        })
        .merge(metrics::base_router());
    metrics::serve(&env_or("BIND_ADDR", "0.0.0.0:8080"), app).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_prometheus_vector() {
        let v = json!({"status":"success","data":{"resultType":"vector","result":[
            {"metric":{"reason":"sold_out"},"value":[1.0,"12.5"]},
            {"metric":{"reason":"overload"},"value":[1.0,"NaN"]}]}});
        let r = parse_vector(&v).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0["reason"], "sold_out");
        assert_eq!(r[0].1, 12.5);
        assert!(r[1].1.is_nan());
    }

    #[test]
    fn prometheus_error_is_an_error() {
        assert!(parse_vector(&json!({"status":"error","error":"bad"})).is_err());
    }

    #[test]
    fn origin_allowlist() {
        let mut h = HeaderMap::new();
        h.insert(header::ORIGIN, "https://evil.example".parse().unwrap());
        assert!(!origin_ok(&h, &["http://localhost:3000".into()]));
        assert!(origin_ok(&h, &[]));
        h.insert(header::ORIGIN, "http://localhost:3000".parse().unwrap());
        assert!(origin_ok(&h, &["http://localhost:3000".into()]));
        assert!(origin_ok(
            &HeaderMap::new(),
            &["http://localhost:3000".into()]
        ));
    }
}
