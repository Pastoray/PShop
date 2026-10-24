//! Transactional-outbox relay: Postgres -> Kafka.
//!
//! Guarantees at-least-once publication of every committed order event, in
//! per-SKU order (Kafka key = sku, idempotent producer). Consumers must be
//! idempotent; search-sync is, via per-SKU version gating.
//!
//! `FOR UPDATE SKIP LOCKED` lets every billing replica run a relay without
//! stepping on each other or double-publishing a row at the same moment.

use std::time::Duration;

use anyhow::Result;
use rdkafka::{
    config::ClientConfig,
    producer::{FutureProducer, FutureRecord},
};
use sqlx::PgPool;

use crate::m;

pub fn producer(bootstrap: &str) -> Result<FutureProducer> {
    Ok(ClientConfig::new()
        .set("bootstrap.servers", bootstrap)
        .set("enable.idempotence", "true") // no duplicates/reordering from retries inside the producer
        .set("acks", "all")
        .set("linger.ms", "5")
        .set("message.timeout.ms", "15000")
        .create()?)
}

pub async fn relay(pool: PgPool, producer: FutureProducer, batch: i64, poll: Duration) {
    loop {
        match relay_once(&pool, &producer, batch).await {
            Ok(0) => tokio::time::sleep(poll).await,
            Ok(_) => {}
            Err(e) => {
                m::OUTBOX_FAILURES.inc();
                tracing::warn!(error = %e, "outbox relay error; backing off");
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn relay_once(pool: &PgPool, producer: &FutureProducer, batch: i64) -> Result<usize> {
    let mut tx = pool.begin().await?;
    let rows: Vec<(i64, String, String, serde_json::Value)> = sqlx::query_as(
        "SELECT id, topic, key, payload FROM outbox \
         WHERE published_at IS NULL ORDER BY id LIMIT $1 FOR UPDATE SKIP LOCKED",
    )
    .bind(batch)
    .fetch_all(&mut *tx)
    .await?;
    if rows.is_empty() {
        return Ok(0);
    }

    // Enqueue everything first so the producer can batch, then await acks.
    let mut in_flight = Vec::with_capacity(rows.len());
    let mut failed = 0usize;
    for (id, topic, key, payload) in &rows {
        let body = serde_json::to_vec(payload)?;
        match producer.send_result(FutureRecord::to(topic).key(key).payload(&body)) {
            Ok(delivery) => in_flight.push((*id, delivery)),
            Err((e, _)) => {
                failed += 1;
                tracing::warn!(error = %e, "kafka enqueue failed");
            }
        }
    }

    let mut done = Vec::with_capacity(in_flight.len());
    for (id, delivery) in in_flight {
        match delivery.await {
            Ok(Ok(_)) => done.push(id),
            Ok(Err((e, _))) => {
                failed += 1;
                tracing::warn!(error = %e, "kafka delivery failed");
            }
            Err(_canceled) => failed += 1,
        }
    }

    if !done.is_empty() {
        sqlx::query("UPDATE outbox SET published_at = now() WHERE id = ANY($1)")
            .bind(&done)
            .execute(&mut *tx)
            .await?;
        m::OUTBOX_PUBLISHED.inc_by(done.len() as u64);
    }
    tx.commit().await?;

    if failed > 0 {
        m::OUTBOX_FAILURES.inc_by(failed as u64);
        if done.is_empty() {
            anyhow::bail!("kafka unavailable: {failed} publishes failed");
        }
    }
    Ok(done.len())
}

/// Backlog gauge + housekeeping (published rows are only kept for a short audit window).
pub async fn housekeeping(pool: PgPool) {
    let mut tick = tokio::time::interval(Duration::from_secs(2));
    let mut n = 0u64;
    loop {
        tick.tick().await;
        match sqlx::query_scalar::<_, i64>("SELECT count(*) FROM outbox WHERE published_at IS NULL")
            .fetch_one(&pool)
            .await
        {
            Ok(c) => m::OUTBOX_PENDING.set(c),
            Err(e) => tracing::warn!(error = %e, "outbox gauge query failed"),
        }
        n += 1;
        if n.is_multiple_of(30) {
            let _ = sqlx::query(
                "DELETE FROM outbox WHERE published_at < now() - interval '10 minutes'",
            )
            .execute(&pool)
            .await;
        }
    }
}
