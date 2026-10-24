//! Order Service — the hot path.
//!
//! Budget: < 5 ms p99 to HTTP 202 at 5k+ req/s across the fleet.
//!
//! Per request, exactly two network hops, both sub-millisecond on a local net:
//!
//! 1. one Lua script on Redis: atomic stock check + decrement + idempotency
//! 2. one publisher-confirmed publish to RabbitMQ: durable hand-off
//!
//! Nothing else is on the path: no Postgres, no S3, no Kafka, no Elasticsearch.
//!
//! Failure policy (the interesting part):
//!
//! * Redis down: 503. We never accept an order without a stock decision.
//! * Broker NACK or not up: definitive "not queued", so release the reservation and answer 503.
//! * Timeout or link drop: AMBIGUOUS (the message may be queued). Do NOT release;
//!   under-selling is recoverable, overselling is not.
//! * Too many in flight: 503 immediately (load shedding beats queueing latency).

use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, LazyLock,
    },
    time::{Duration, Instant},
};

use anyhow::{bail, Result};
use axum::{
    extract::{rejection::JsonRejection, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use prometheus::{
    register_histogram, register_int_counter, register_int_counter_vec, register_int_gauge,
    Histogram, IntCounter, IntCounterVec, IntGauge,
};
use pshop_common::{
    config::{env_or, env_parse},
    events::OrderCommand,
    keys, metrics,
    mq::{self, ConfirmedPublisher, PublishError},
    telemetry, util,
};
use redis::{aio::ConnectionManager, Script, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Semaphore;
use uuid::Uuid;

// ---------------------------------------------------------------- metrics ---

static ACCEPTED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_orders_accepted_total",
        "Orders reserved and durably queued"
    )
    .unwrap()
});
static REPLAYED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_orders_replayed_total",
        "Idempotent replays answered from Redis"
    )
    .unwrap()
});
static REJECTED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "pshop_orders_rejected_total",
        "Rejected orders by reason",
        &["reason"]
    )
    .unwrap()
});
static UNKNOWN_OUTCOME: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_orders_unknown_outcome_total",
        "Publishes whose outcome is ambiguous; reservation intentionally kept"
    )
    .unwrap()
});
static RELEASE_FAILURES: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_stock_release_failures_total",
        "Compensations that themselves failed"
    )
    .unwrap()
});
static INFLIGHT: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!("pshop_orders_inflight", "Requests currently being handled").unwrap()
});
static ACCEPT_LATENCY: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_order_accept_seconds",
        "Handler latency for POST /v1/orders",
        metrics::FAST_BUCKETS.to_vec()
    )
    .unwrap()
});
static REDIS_LATENCY: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_redis_reserve_seconds",
        "Latency of the stock reservation script",
        metrics::FAST_BUCKETS.to_vec()
    )
    .unwrap()
});
static RABBIT_LATENCY: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_rabbit_publish_seconds",
        "Latency until the broker confirmed the publish",
        metrics::FAST_BUCKETS.to_vec()
    )
    .unwrap()
});

fn force_metrics() {
    LazyLock::force(&ACCEPTED);
    LazyLock::force(&REPLAYED);
    LazyLock::force(&REJECTED);
    LazyLock::force(&UNKNOWN_OUTCOME);
    LazyLock::force(&RELEASE_FAILURES);
    LazyLock::force(&INFLIGHT);
    LazyLock::force(&ACCEPT_LATENCY);
    LazyLock::force(&REDIS_LATENCY);
    LazyLock::force(&RABBIT_LATENCY);
    for r in [
        "sold_out",
        "unknown_sku",
        "invalid",
        "overload",
        "redis_unavailable",
        "queue_unavailable",
        "queue_full",
    ] {
        REJECTED.with_label_values(&[r]).inc_by(0);
    }
}

// ------------------------------------------------------------------ state ---

/// A few multiplexed connections, round-robined. One connection pipelines well,
/// but several remove head-of-line blocking behind a slow reply.
struct RedisPool {
    conns: Vec<ConnectionManager>,
    next: AtomicUsize,
}

impl RedisPool {
    fn get(&self) -> ConnectionManager {
        self.conns[self.next.fetch_add(1, Ordering::Relaxed) % self.conns.len()].clone()
    }
}

struct AppState {
    redis: RedisPool,
    reserve: Script,
    release: Script,
    publisher: Arc<ConfirmedPublisher>,
    permits: Semaphore,
    idem_ttl_secs: u64,
    max_qty: u32,
    io_timeout: Duration,
}

type Shared = Arc<AppState>;

// ------------------------------------------------------------ API schemas ---

#[derive(Debug, Deserialize)]
struct CreateOrder {
    user_id: String,
    sku: String,
    #[serde(default = "one")]
    qty: u32,
}

fn one() -> u32 {
    1
}

#[derive(Serialize)]
struct Accepted {
    order_id: String,
    status: &'static str,
}

fn valid_token(s: &str) -> bool {
    // Strict charset: SKUs become part of Redis keys with a `{hash-tag}`.
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn reject(status: StatusCode, reason: &'static str, message: &str) -> Response {
    REJECTED.with_label_values(&[reason]).inc();
    let mut resp = (status, Json(json!({ "error": reason, "message": message }))).into_response();
    if status == StatusCode::SERVICE_UNAVAILABLE {
        resp.headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from_static("1"));
    }
    resp
}

enum Reservation {
    Reserved {
        remaining: i64,
        version: i64,
        price: u64,
    },
    SoldOut,
    Duplicate(String),
    UnknownSku,
}

fn parse_reservation(v: Vec<Value>) -> Result<Reservation> {
    let int = |i: usize| -> Result<i64> {
        match v.get(i) {
            Some(Value::Int(n)) => Ok(*n),
            other => bail!("unexpected script reply at {i}: {other:?}"),
        }
    };
    Ok(match int(0)? {
        0 => Reservation::SoldOut,
        1 => Reservation::Reserved {
            remaining: int(1)?,
            version: int(2)?,
            price: int(3)?.max(0) as u64,
        },
        2 => match v.get(1) {
            Some(Value::BulkString(b)) => {
                Reservation::Duplicate(String::from_utf8_lossy(b).into_owned())
            }
            other => bail!("unexpected duplicate payload: {other:?}"),
        },
        3 => Reservation::UnknownSku,
        n => bail!("unknown script status {n}"),
    })
}

// --------------------------------------------------------------- handlers ---

async fn create_order(
    State(st): State<Shared>,
    headers: HeaderMap,
    body: Result<Json<CreateOrder>, JsonRejection>,
) -> Response {
    let started = Instant::now();
    INFLIGHT.inc();
    let resp = handle(&st, &headers, body).await;
    INFLIGHT.dec();
    ACCEPT_LATENCY.observe(started.elapsed().as_secs_f64());
    resp
}

async fn handle(
    st: &AppState,
    headers: &HeaderMap,
    body: Result<Json<CreateOrder>, JsonRejection>,
) -> Response {
    // Load shedding first: cheaper to refuse than to queue inside the process.
    let Ok(_permit) = st.permits.try_acquire() else {
        return reject(
            StatusCode::SERVICE_UNAVAILABLE,
            "overload",
            "server is shedding load",
        );
    };

    let Json(req) = match body {
        Ok(b) => b,
        Err(e) => return reject(StatusCode::BAD_REQUEST, "invalid", &e.body_text()),
    };
    if !valid_token(&req.sku) || !valid_token(&req.user_id) || req.qty == 0 || req.qty > st.max_qty
    {
        return reject(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid",
            "invalid sku, user_id or qty",
        );
    }

    let order_id = Uuid::now_v7(); // time-ordered: friendly to the Postgres PK index
    let idem = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|v| valid_token(v))
        .map(str::to_owned)
        .unwrap_or_else(|| order_id.to_string());

    // 1) atomic reservation ------------------------------------------------
    let t = Instant::now();
    let mut conn = st.redis.get();
    let reply = tokio::time::timeout(
        st.io_timeout,
        st.reserve
            .key(keys::stock(&req.sku))
            .key(keys::stock_version(&req.sku))
            .key(keys::idempotency(&req.sku, &idem))
            .key(keys::price(&req.sku))
            .arg(req.qty)
            .arg(st.idem_ttl_secs)
            .arg(order_id.to_string())
            .invoke_async::<Vec<Value>>(&mut conn),
    )
    .await;
    REDIS_LATENCY.observe(t.elapsed().as_secs_f64());

    let reservation = match reply {
        Ok(Ok(v)) => match parse_reservation(v) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, "bad reservation reply");
                return reject(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "redis_unavailable",
                    "stock store error",
                );
            }
        },
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "redis error");
            return reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "redis_unavailable",
                "stock store unavailable",
            );
        }
        Err(_) => {
            return reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "redis_unavailable",
                "stock store timeout",
            )
        }
    };

    let (remaining, version, price) = match reservation {
        Reservation::Reserved {
            remaining,
            version,
            price,
        } => (remaining, version, price),
        Reservation::SoldOut => {
            return reject(StatusCode::CONFLICT, "sold_out", "insufficient stock")
        }
        Reservation::UnknownSku => {
            return reject(StatusCode::NOT_FOUND, "unknown_sku", "unknown sku")
        }
        Reservation::Duplicate(existing) => {
            REPLAYED.inc();
            return (
                StatusCode::ACCEPTED,
                Json(Accepted {
                    order_id: existing,
                    status: "duplicate_of_accepted_order",
                }),
            )
                .into_response();
        }
    };

    // 2) durable hand-off to the billing pipeline ---------------------------
    let cmd = OrderCommand {
        order_id,
        user_id: req.user_id,
        sku: req.sku.clone(),
        qty: req.qty,
        unit_price_cents: price,
        stock_remaining: remaining,
        stock_version: version,
        idempotency_key: idem.clone(),
        accepted_at: chrono::Utc::now(),
    };
    let payload = match serde_json::to_vec(&cmd) {
        Ok(p) => p,
        Err(e) => {
            tracing::error!(error = %e, "serialize");
            release(st, &req.sku, req.qty, &idem).await;
            return reject(
                StatusCode::INTERNAL_SERVER_ERROR,
                "invalid",
                "internal error",
            );
        }
    };

    let t = Instant::now();
    let published = tokio::time::timeout(
        st.io_timeout,
        st.publisher.publish(
            mq::EXCHANGE,
            mq::RK_PAYMENT,
            &payload,
            mq::persistent_json(),
        ),
    )
    .await;
    RABBIT_LATENCY.observe(t.elapsed().as_secs_f64());

    match published {
        Ok(Ok(())) => {
            ACCEPTED.inc();
            let mut resp = (
                StatusCode::ACCEPTED,
                Json(Accepted {
                    order_id: order_id.to_string(),
                    status: "accepted",
                }),
            )
                .into_response();
            if let Ok(loc) = HeaderValue::from_str(&format!("/v1/order-status/{order_id}")) {
                resp.headers_mut().insert(header::LOCATION, loc);
            }
            resp
        }
        // Definitive: the broker did not take the message.
        Ok(Err(PublishError::NotReady)) => {
            release(st, &req.sku, req.qty, &idem).await;
            reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "queue_unavailable",
                "message broker unavailable",
            )
        }
        Ok(Err(PublishError::Nacked)) => {
            release(st, &req.sku, req.qty, &idem).await;
            reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "queue_full",
                "billing backlog is full",
            )
        }
        // Ambiguous: the message may be in the queue. Keep the reservation.
        Ok(Err(PublishError::Amqp(e))) => {
            UNKNOWN_OUTCOME.inc();
            tracing::warn!(error = %e, %order_id, "publish outcome unknown; keeping reservation");
            reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "queue_unavailable",
                "outcome unknown, retry with same Idempotency-Key",
            )
        }
        Err(_elapsed) => {
            UNKNOWN_OUTCOME.inc();
            tracing::warn!(%order_id, "publish timed out; keeping reservation");
            reject(
                StatusCode::SERVICE_UNAVAILABLE,
                "queue_unavailable",
                "outcome unknown, retry with same Idempotency-Key",
            )
        }
    }
}

/// Compensating action. Best effort: if it fails we count it loudly; the
/// search-sync reconcile loop and the alert on this counter catch the drift.
async fn release(st: &AppState, sku: &str, qty: u32, idem: &str) {
    let mut conn = st.redis.get();
    let res = tokio::time::timeout(
        st.io_timeout,
        st.release
            .key(keys::stock(sku))
            .key(keys::stock_version(sku))
            .key(keys::idempotency(sku, idem))
            .arg(qty)
            .invoke_async::<Vec<i64>>(&mut conn),
    )
    .await;
    if !matches!(res, Ok(Ok(_))) {
        RELEASE_FAILURES.inc();
        tracing::error!(%sku, qty, "stock release failed; stock under-reported until reconciled");
    }
}

async fn readyz(State(st): State<Shared>) -> StatusCode {
    if !st.publisher.is_ready() {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    let mut conn = st.redis.get();
    match tokio::time::timeout(
        Duration::from_millis(200),
        redis::cmd("PING").query_async::<String>(&mut conn),
    )
    .await
    {
        Ok(Ok(_)) => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

// ------------------------------------------------------------------- boot ---

/// Dev/sim convenience: make a catalogue of `SEED_SKUS` products exist.
/// `SET ... NX` makes it safe for N replicas to race on startup.
async fn seed(conn: &mut ConnectionManager, skus: u32, stock: i64) -> Result<()> {
    if skus == 0 {
        return Ok(());
    }
    let mut pipe = redis::pipe();
    for i in 1..=skus {
        let sku = pshop_common::catalogue::sku(i);
        let price = pshop_common::catalogue::price_cents(i);
        pipe.cmd("SET")
            .arg(keys::stock(&sku))
            .arg(stock)
            .arg("NX")
            .ignore();
        pipe.cmd("SET")
            .arg(keys::stock_version(&sku))
            .arg(0)
            .arg("NX")
            .ignore();
        pipe.cmd("SET")
            .arg(keys::price(&sku))
            .arg(price)
            .arg("NX")
            .ignore();
    }
    pipe.query_async::<()>(conn).await?;
    tracing::info!(skus, stock, "catalogue seeded (NX)");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init_tracing("order-service");
    force_metrics();

    let redis_url = env_or("REDIS_URL", "redis://127.0.0.1:6379");
    let amqp_url = env_or("AMQP_URL", "amqp://guest:guest@127.0.0.1:5672/%2f");
    let bind = env_or("BIND_ADDR", "0.0.0.0:8080");

    let client = redis::Client::open(redis_url)?;
    let pool_size: usize = env_parse("REDIS_CONNECTIONS", 4);
    let mut conns = Vec::with_capacity(pool_size);
    for _ in 0..pool_size.max(1) {
        let c = util::retry("redis connect", 60, Duration::from_millis(500), || {
            ConnectionManager::new(client.clone())
        })
        .await?;
        conns.push(c);
    }
    seed(
        &mut conns[0].clone(),
        env_parse("SEED_SKUS", 100),
        env_parse("SEED_STOCK_PER_SKU", 1_000_000),
    )
    .await?;

    let publisher = ConfirmedPublisher::spawn(
        amqp_url,
        env_parse("AMQP_CHANNELS", 8),
        env_parse("QUEUE_MAX_LEN", 500_000),
    );

    let state = Arc::new(AppState {
        redis: RedisPool {
            conns,
            next: AtomicUsize::new(0),
        },
        reserve: Script::new(keys::RESERVE_LUA),
        release: Script::new(keys::RELEASE_LUA),
        publisher,
        permits: Semaphore::new(env_parse("MAX_INFLIGHT", 4096)),
        idem_ttl_secs: env_parse("IDEMPOTENCY_TTL_SECS", 86_400),
        max_qty: env_parse("MAX_QTY", 10),
        io_timeout: Duration::from_millis(env_parse("IO_TIMEOUT_MS", 250)),
    });

    let app = Router::new()
        .route("/v1/orders", post(create_order))
        .route("/readyz", get(readyz))
        .with_state(state)
        .merge(metrics::base_router());

    metrics::serve(&bind, app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_validation() {
        assert!(valid_token("SKU-0001"));
        assert!(!valid_token(""));
        assert!(!valid_token("sku}:evil{"));
        assert!(!valid_token(&"a".repeat(65)));
    }

    #[test]
    fn parses_script_replies() {
        let r = parse_reservation(vec![
            Value::Int(1),
            Value::Int(99),
            Value::Int(7),
            Value::Int(1299),
        ])
        .unwrap();
        assert!(matches!(
            r,
            Reservation::Reserved {
                remaining: 99,
                version: 7,
                price: 1299
            }
        ));
        assert!(matches!(
            parse_reservation(vec![Value::Int(0), Value::Int(0)]).unwrap(),
            Reservation::SoldOut
        ));
        assert!(matches!(
            parse_reservation(vec![Value::Int(3)]).unwrap(),
            Reservation::UnknownSku
        ));
        let d = parse_reservation(vec![Value::Int(2), Value::BulkString(b"abc".to_vec())]).unwrap();
        assert!(matches!(d, Reservation::Duplicate(ref s) if s == "abc"));
        assert!(parse_reservation(vec![Value::Int(9)]).is_err());
    }
}
