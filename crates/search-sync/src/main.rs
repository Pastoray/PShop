//! Search / Inventory Sync.
//!
//! Kafka `pshop.orders.completed` -> Elasticsearch stock balances, plus a small
//! `/v1/search` read API and a self-healing reconcile loop against Redis.
//!
//! Correctness model:
//!   * Redis is the source of truth for stock; ES is a projection.
//!   * Every stock mutation carries a per-SKU monotonic version.
//!   * ES applies an update only if its version is newer (scripted update), so
//!     Kafka redelivery, replays and out-of-order arrival are all harmless.
//!   * Offsets are committed only after ES acknowledged the batch.
//!   * Events can still be lost upstream (e.g. stock released on dead-letter
//!     emits no event), so a reconcile loop periodically re-projects Redis.

mod es;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, LazyLock,
    },
    time::Duration,
};

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use es::{coalesce, Es, StockUpdate};
use prometheus::{
    register_gauge, register_histogram, register_int_counter, register_int_counter_vec, Gauge,
    Histogram, IntCounter, IntCounterVec,
};
use pshop_common::{
    config::{env_or, env_parse},
    events::{OrderCompleted, KAFKA_TOPIC_ORDER_COMPLETED},
    keys, metrics, telemetry, util,
};
use rdkafka::{
    config::ClientConfig,
    consumer::{CommitMode, Consumer, StreamConsumer},
    message::Message,
    Offset, TopicPartitionList,
};
use serde::Deserialize;

static EVENTS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "pshop_search_sync_events_total",
        "Kafka events by result",
        &["result"]
    )
    .unwrap()
});
static BULK_SECONDS: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_search_sync_bulk_seconds",
        "Elasticsearch bulk latency",
        metrics::SLOW_BUCKETS.to_vec()
    )
    .unwrap()
});
static BATCH_SIZE: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_search_sync_batch_size",
        "Events per consumer batch",
        vec![1.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0]
    )
    .unwrap()
});
static LAG_SECONDS: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!(
        "pshop_search_sync_lag_seconds",
        "Age of the newest event when it reached Elasticsearch"
    )
    .unwrap()
});
static ES_FAILURES: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_search_sync_es_failures_total",
        "Failed Elasticsearch bulk attempts"
    )
    .unwrap()
});
static RECONCILED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_search_sync_reconciled_total",
        "SKUs re-projected from Redis"
    )
    .unwrap()
});

#[derive(Clone)]
struct AppState {
    es: Arc<Es>,
    ready: Arc<AtomicBool>,
}

#[derive(Deserialize)]
struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    category: Option<String>,
}
fn default_limit() -> usize {
    20
}

async fn search(
    State(st): State<AppState>,
    Query(p): Query<SearchQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Only known categories are accepted; anything else is a client error, not an empty page.
    if let Some(c) = p.category.as_deref() {
        if !pshop_common::catalogue::CATEGORIES.contains(&c) {
            return Err(StatusCode::BAD_REQUEST);
        }
    }
    st.es
        .search(&p.q, p.category.as_deref(), p.limit, p.offset)
        .await
        .map(Json)
        .map_err(|e| {
            tracing::warn!(error = %e, "search failed");
            StatusCode::BAD_GATEWAY
        })
}

async fn product(
    State(st): State<AppState>,
    Path(sku): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // SKUs are `[A-Za-z0-9_-]{1,64}`: reject everything else before it reaches a URL path.
    if sku.is_empty()
        || sku.len() > 64
        || !sku
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    match st.es.get_product(&sku).await {
        Ok(Some(p)) => Ok(Json(p)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::warn!(error = %e, "get product failed");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

async fn readyz(State(st): State<AppState>) -> StatusCode {
    if st.ready.load(Ordering::Relaxed) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

/// Retry forever with capped backoff: if Elasticsearch is down we deliberately
/// stop consuming. Lag grows in Kafka (cheap, durable) instead of memory here.
async fn bulk_until_ok(es: &Es, updates: &[StockUpdate]) {
    let mut delay = Duration::from_millis(250);
    loop {
        let t = std::time::Instant::now();
        match es.bulk_stock(updates).await {
            Ok(out) => {
                BULK_SECONDS.observe(t.elapsed().as_secs_f64());
                if out.rejected_permanent > 0 {
                    EVENTS
                        .with_label_values(&["rejected"])
                        .inc_by(out.rejected_permanent as u64);
                }
                return;
            }
            Err(e) => {
                ES_FAILURES.inc();
                tracing::warn!(error = %e, "es bulk failed; will retry (consumption paused)");
                tokio::time::sleep(delay).await;
                delay = (delay * 2).min(Duration::from_secs(5));
            }
        }
    }
}

async fn consume(
    es: Arc<Es>,
    brokers: String,
    group: String,
    max_batch: usize,
    max_wait: Duration,
) -> Result<()> {
    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &brokers)
        .set("group.id", &group)
        .set("enable.auto.commit", "false") // we commit after ES confirms
        .set("auto.offset.reset", "earliest")
        .set("session.timeout.ms", "10000")
        .set("allow.auto.create.topics", "true")
        .create()?;
    consumer.subscribe(&[KAFKA_TOPIC_ORDER_COMPLETED])?;
    tracing::info!(%brokers, %group, topic = KAFKA_TOPIC_ORDER_COMPLETED, "kafka consumer subscribed");

    loop {
        // Block for the first message, then keep filling until size or time limit.
        let first = consumer.recv().await?;
        let deadline = tokio::time::Instant::now() + max_wait;
        let mut batch = Batch::default();
        batch.push(&first);
        drop(first);
        while batch.count < max_batch {
            match tokio::time::timeout_at(deadline, consumer.recv()).await {
                Ok(Ok(m)) => batch.push(&m),
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            }
        }

        BATCH_SIZE.observe(batch.count as f64);
        let count = batch.count;
        let folded = coalesce(batch.updates);
        bulk_until_ok(&es, &folded).await;
        EVENTS.with_label_values(&["applied"]).inc_by(count as u64);
        if let Some(n) = batch.newest {
            LAG_SECONDS.set((chrono::Utc::now() - n).num_milliseconds().max(0) as f64 / 1000.0);
        }

        // Commit only after Elasticsearch acknowledged: at-least-once, never lossy.
        let mut tpl = TopicPartitionList::new();
        for (p, off) in batch.offsets {
            tpl.add_partition_offset(KAFKA_TOPIC_ORDER_COMPLETED, p, Offset::Offset(off + 1))?;
        }
        consumer.commit(&tpl, CommitMode::Async)?;
    }
}

#[derive(Default)]
struct Batch {
    updates: Vec<StockUpdate>,
    newest: Option<chrono::DateTime<chrono::Utc>>,
    offsets: HashMap<i32, i64>,
    count: usize,
}

impl Batch {
    fn push(&mut self, m: &rdkafka::message::BorrowedMessage<'_>) {
        self.count += 1;
        let slot = self.offsets.entry(m.partition()).or_insert(-1);
        *slot = (*slot).max(m.offset());
        match m.payload().map(serde_json::from_slice::<OrderCompleted>) {
            Some(Ok(e)) => {
                self.newest = Some(
                    self.newest
                        .map_or(e.completed_at, |n| n.max(e.completed_at)),
                );
                self.updates.push(StockUpdate {
                    sku: e.sku,
                    stock: e.stock_remaining,
                    version: e.stock_version,
                });
            }
            // A poison pill must not wedge the partition: count it, skip it, move on.
            _ => EVENTS.with_label_values(&["poison"]).inc(),
        }
    }
}

/// Re-project Redis (truth) into ES. Cheap insurance against lost events.
async fn reconcile_loop(es: Arc<Es>, redis_url: String, every: Duration) {
    let client = match redis::Client::open(redis_url) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "reconcile disabled: bad redis url");
            return;
        }
    };
    let mut tick = tokio::time::interval(every);
    tick.tick().await; // skip the immediate first tick
    loop {
        tick.tick().await;
        if let Err(e) = reconcile_once(&es, &client).await {
            tracing::warn!(error = %e, "reconcile pass failed");
        }
    }
}

async fn reconcile_once(es: &Es, client: &redis::Client) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let mut cursor = 0u64;
    let mut skus: Vec<String> = Vec::new();
    loop {
        let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(keys::STOCK_SCAN_PATTERN)
            .arg("COUNT")
            .arg(500)
            .query_async(&mut conn)
            .await?;
        skus.extend(
            batch
                .iter()
                .filter_map(|k| keys::sku_from_stock_key(k))
                .map(str::to_owned),
        );
        cursor = next;
        if cursor == 0 {
            break;
        }
    }
    skus.sort();
    skus.dedup();

    for chunk in skus.chunks(200) {
        // MULTI/EXEC => one consistent snapshot of (stock, version) pairs.
        let mut pipe = redis::pipe();
        pipe.atomic();
        for sku in chunk {
            pipe.get(keys::stock(sku));
            pipe.get(keys::stock_version(sku));
        }
        let vals: Vec<Option<i64>> = pipe.query_async(&mut conn).await?;
        let updates: Vec<StockUpdate> = chunk
            .iter()
            .zip(vals.chunks(2))
            .filter_map(|(sku, v)| {
                match (v.first().copied().flatten(), v.get(1).copied().flatten()) {
                    (Some(stock), Some(version)) => Some(StockUpdate {
                        sku: sku.clone(),
                        stock,
                        version,
                    }),
                    _ => None,
                }
            })
            .collect();
        bulk_until_ok(es, &updates).await;
        RECONCILED.inc_by(updates.len() as u64);
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init_tracing("search-sync");
    LazyLock::force(&EVENTS);
    LazyLock::force(&BULK_SECONDS);
    LazyLock::force(&BATCH_SIZE);
    LazyLock::force(&LAG_SECONDS);
    LazyLock::force(&ES_FAILURES);
    LazyLock::force(&RECONCILED);
    for r in ["applied", "poison", "rejected"] {
        EVENTS.with_label_values(&[r]).inc_by(0);
    }

    let es = Arc::new(Es::new(
        env_or("ES_URL", "http://127.0.0.1:9200"),
        env_or("ES_INDEX", "pshop-products"),
    )?);
    let ready = Arc::new(AtomicBool::new(false));

    util::retry("elasticsearch ping", 90, Duration::from_millis(500), || {
        es.ping()
    })
    .await?;
    es.ensure_index().await?;
    es.seed_catalogue(
        env_parse("SEED_SKUS", 100),
        env_parse("SEED_STOCK_PER_SKU", 1_000_000),
    )
    .await?;
    ready.store(true, Ordering::Relaxed);

    let es_c = Arc::clone(&es);
    tokio::spawn(async move {
        loop {
            let r = consume(
                Arc::clone(&es_c),
                env_or("KAFKA_BROKERS", "127.0.0.1:9092"),
                env_or("KAFKA_GROUP", "search-sync"),
                env_parse("BATCH_MAX", 500),
                Duration::from_millis(env_parse("BATCH_WAIT_MS", 200)),
            )
            .await;
            tracing::error!(error = ?r.err(), "consumer loop exited; restarting in 2s");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    });

    tokio::spawn(reconcile_loop(
        Arc::clone(&es),
        env_or("REDIS_URL", "redis://127.0.0.1:6379"),
        Duration::from_secs(env_parse("RECONCILE_INTERVAL_SECS", 30)),
    ));

    let app = Router::new()
        .route("/v1/search", get(search))
        .route("/v1/products/{sku}", get(product))
        .route("/readyz", get(readyz))
        .with_state(AppState { es, ready })
        .merge(metrics::base_router());
    metrics::serve(&env_or("BIND_ADDR", "0.0.0.0:8080"), app).await
}
