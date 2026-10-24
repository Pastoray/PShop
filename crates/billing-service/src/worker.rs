//! RabbitMQ consumer: throttled, idempotent, with bounded retries then DLQ.
//!
//! Delivery semantics: at-least-once from the broker, made effectively-once by
//!   * `ON CONFLICT (order_id) DO NOTHING` on the order insert,
//!   * a deterministic S3 key (re-PUT overwrites itself),
//!   * an idempotent stock-release script on the dead-letter path.
//!
//! Throttling is two-layered, on purpose:
//!   * `basic_qos(prefetch)` bounds unacked messages held by this process;
//!   * a token-bucket (`BILLING_MAX_RPS`) paces *starts*, so a 5k rps spike at the
//!     edge becomes a steady trickle at Postgres and the queue absorbs the rest.

use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, bail, Result};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_sqs::error::DisplayErrorContext;
use futures_lite::StreamExt;
use governor::{Quota, RateLimiter};
use lapin::{
    message::Delivery,
    options::{BasicAckOptions, BasicConsumeOptions, BasicNackOptions, BasicQosOptions},
    types::{AMQPValue, FieldTable},
};
use pshop_common::{
    events::{OrderCommand, OrderCompleted, KAFKA_TOPIC_ORDER_COMPLETED},
    keys,
    mq::{self, ConfirmedPublisher},
};
use rand::RngExt;
use redis::{aio::ConnectionManager, Script};
use sqlx::PgPool;
use tokio::sync::Semaphore;

use crate::{invoice, m};

pub struct Cfg {
    pub max_attempts: u32,
    pub payment_latency_ms: u64,
    pub payment_failure_rate: f64,
    pub queue_max_len: u32,
}

pub struct Ctx {
    pub pool: PgPool,
    pub s3: aws_sdk_s3::Client,
    pub sqs: aws_sdk_sqs::Client,
    pub sns: aws_sdk_sns::Client,
    pub redis: ConnectionManager,
    pub release_once: Script,
    pub retry_publisher: Arc<ConfirmedPublisher>,
    pub bucket: String,
    pub dlq_url: String,
    pub alert_topic_arn: String,
    pub signing_key: String,
    pub cfg: Cfg,
    pub control: crate::control::Control,
}

fn aws_err<E: std::error::Error>(e: E) -> anyhow::Error {
    anyhow!("{}", DisplayErrorContext(&e))
}

// ------------------------------------------------------------ consumption ---

pub async fn run(
    ctx: Arc<Ctx>,
    amqp_url: String,
    prefetch: u16,
    max_rps: u32,
    max_concurrency: usize,
    ready: Arc<AtomicBool>,
    permits: Arc<Semaphore>,
) {
    let quota = Quota::per_second(NonZeroU32::new(max_rps.max(1)).unwrap());
    let limiter = Arc::new(RateLimiter::direct(quota));
    loop {
        match consume_once(
            &ctx,
            &amqp_url,
            prefetch,
            &limiter,
            max_concurrency,
            &ready,
            &permits,
        )
        .await
        {
            Ok(()) => tracing::warn!("consumer stream ended; reconnecting"),
            Err(e) => tracing::error!(error = %e, "consumer failed; reconnecting"),
        }
        ready.store(false, Ordering::Relaxed);
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn consume_once(
    ctx: &Arc<Ctx>,
    amqp_url: &str,
    prefetch: u16,
    limiter: &Arc<governor::DefaultDirectRateLimiter>,
    _max_concurrency: usize,
    ready: &AtomicBool,
    permits: &Arc<Semaphore>,
) -> Result<()> {
    let conn = mq::connect(amqp_url).await?;
    let ch = conn.create_channel().await?;
    mq::declare_topology(&ch, ctx.cfg.queue_max_len).await?;
    ch.basic_qos(prefetch, BasicQosOptions::default()).await?;
    let mut consumer = ch
        .basic_consume(
            mq::QUEUE_PAYMENTS.into(),
            format!("billing-{}", uuid::Uuid::new_v4()).as_str().into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;
    ready.store(true, Ordering::Relaxed);
    tracing::info!(prefetch, "billing consumer ready");

    while let Some(delivery) = consumer.next().await {
        let delivery = delivery?;
        ctx.control.gate().await; // demo: provider slow/down
        limiter.until_ready().await; // pace the *start* of work
        let permit = Arc::clone(permits).acquire_owned().await?; // bound concurrency
        let ctx = Arc::clone(ctx);
        tokio::spawn(async move {
            m::INFLIGHT.inc();
            if let Err(e) = process(&ctx, delivery).await {
                // Ack/nack itself failed (channel gone). The broker redelivers; idempotency covers it.
                tracing::error!(error = %e, "delivery bookkeeping failed");
            }
            m::INFLIGHT.dec();
            drop(permit);
        });
    }
    Ok(())
}

fn attempt_of(d: &Delivery) -> u32 {
    d.properties
        .headers()
        .as_ref()
        .and_then(|h| h.inner().get(mq::HEADER_ATTEMPT))
        .and_then(|v| match v {
            AMQPValue::LongUInt(n) => Some(*n),
            AMQPValue::LongInt(n) => u32::try_from(*n).ok(),
            AMQPValue::LongLongInt(n) => u32::try_from(*n).ok(),
            AMQPValue::ShortUInt(n) => Some(u32::from(*n)),
            AMQPValue::ShortInt(n) => u32::try_from(*n).ok(),
            _ => None,
        })
        .unwrap_or(0)
}

async fn ack(d: &Delivery) -> Result<()> {
    d.ack(BasicAckOptions::default()).await?;
    Ok(())
}

async fn requeue(d: &Delivery) -> Result<()> {
    // Small pause so an outage doesn't turn into a hot redelivery loop.
    tokio::time::sleep(Duration::from_secs(1)).await;
    d.nack(BasicNackOptions {
        requeue: true,
        ..Default::default()
    })
    .await?;
    Ok(())
}

async fn process(ctx: &Ctx, d: Delivery) -> Result<()> {
    let attempt = attempt_of(&d);

    let cmd: OrderCommand = match serde_json::from_slice(&d.data) {
        Ok(c) => c,
        Err(e) => {
            // Poison message: retrying can never help. Park it for humans.
            tracing::error!(error = %e, "malformed command; parking in DLQ");
            if ctx
                .park_raw(&d.data, &format!("malformed: {e}"))
                .await
                .is_err()
            {
                return requeue(&d).await;
            }
            m::PROCESSED.with_label_values(&["malformed"]).inc();
            return ack(&d).await;
        }
    };

    match ctx.settle(&cmd).await {
        Ok(created) => {
            m::PROCESSED
                .with_label_values(&[if created { "ok" } else { "duplicate" }])
                .inc();
            let waited = (chrono::Utc::now() - cmd.accepted_at)
                .to_std()
                .unwrap_or_default();
            m::END_TO_END.observe(waited.as_secs_f64());
            ack(&d).await
        }
        Err(e) if attempt + 1 < ctx.cfg.max_attempts => {
            tracing::warn!(order_id = %cmd.order_id, attempt = attempt + 1, error = %e, "settle failed; scheduling retry");
            let mut headers = FieldTable::default();
            headers.insert(mq::HEADER_ATTEMPT.into(), AMQPValue::LongUInt(attempt + 1));
            let props = mq::persistent_json().with_headers(headers);
            match ctx
                .retry_publisher
                .publish(mq::EXCHANGE_RETRY, mq::RK_RETRY, &d.data, props)
                .await
            {
                Ok(()) => {
                    m::PROCESSED.with_label_values(&["retry"]).inc();
                    ack(&d).await
                }
                Err(pe) => {
                    tracing::error!(error = %pe, "could not schedule retry; requeueing original");
                    requeue(&d).await
                }
            }
        }
        Err(e) => {
            tracing::error!(order_id = %cmd.order_id, error = %e, "retries exhausted; dead-lettering");
            match ctx
                .dead_letter(&cmd, &d.data, &e.to_string(), attempt + 1)
                .await
            {
                Ok(()) => {
                    m::PROCESSED.with_label_values(&["dead_lettered"]).inc();
                    ack(&d).await
                }
                Err(de) => {
                    tracing::error!(error = %de, "dead-letter path failed; requeueing");
                    requeue(&d).await
                }
            }
        }
    }
}

// ---------------------------------------------------------------- settling ---

impl Ctx {
    /// Charge -> invoice -> ACID write. Returns `true` if this call created the
    /// order, `false` if it already existed (redelivery).
    async fn settle(&self, cmd: &OrderCommand) -> Result<bool> {
        self.charge().await?;

        // Invoice first: it's an idempotent PUT, and an orphan PDF for an order
        // that fails to commit is harmless, whereas an event pointing at a
        // missing invoice would not be.
        let key = invoice::object_key(cmd);
        let pdf = invoice::render(cmd, &invoice::sign(&self.signing_key, cmd));
        let t = std::time::Instant::now();
        self.s3
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type("application/pdf")
            .body(ByteStream::from(pdf))
            .send()
            .await
            .map_err(aws_err)?;
        m::INVOICE_UPLOAD.observe(t.elapsed().as_secs_f64());

        // Order row + outbox event, one transaction.
        let t = std::time::Instant::now();
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO orders (order_id, user_id, sku, qty, unit_price_cents, total_cents, invoice_key, idempotency_key, accepted_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (order_id) DO NOTHING",
        )
        .bind(cmd.order_id)
        .bind(&cmd.user_id)
        .bind(&cmd.sku)
        .bind(i32::try_from(cmd.qty)?)
        .bind(i64::try_from(cmd.unit_price_cents)?)
        .bind(i64::try_from(cmd.total_cents())?)
        .bind(&key)
        .bind(&cmd.idempotency_key)
        .bind(cmd.accepted_at)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;

        if inserted {
            let evt = OrderCompleted {
                order_id: cmd.order_id,
                user_id: cmd.user_id.clone(),
                sku: cmd.sku.clone(),
                qty: cmd.qty,
                total_cents: cmd.total_cents(),
                stock_remaining: cmd.stock_remaining,
                stock_version: cmd.stock_version,
                invoice_key: key,
                accepted_at: cmd.accepted_at,
                completed_at: chrono::Utc::now(),
            };
            sqlx::query("INSERT INTO outbox (topic, key, payload) VALUES ($1, $2, $3)")
                .bind(KAFKA_TOPIC_ORDER_COMPLETED)
                .bind(&cmd.sku)
                .bind(sqlx::types::Json(&evt))
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        m::DB_WRITE.observe(t.elapsed().as_secs_f64());
        Ok(inserted)
    }

    /// Stand-in for a real PSP call: latency plus random transient failures.
    async fn charge(&self) -> Result<()> {
        let (delay_ms, fail) = {
            let mut rng = rand::rng(); // ThreadRng is !Send: keep it out of .await
            let base = self.cfg.payment_latency_ms;
            let jitter = if base > 1 {
                rng.random_range(0..=base / 2)
            } else {
                0
            };
            (
                base + jitter,
                rng.random_bool(self.cfg.payment_failure_rate.clamp(0.0, 1.0)),
            )
        };
        let t = std::time::Instant::now();
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        m::GATEWAY.observe(t.elapsed().as_secs_f64());
        if fail {
            bail!("payment gateway timeout (simulated)");
        }
        Ok(())
    }

    // ----------------------------------------------------------- dead letters ---

    async fn send_dlq(&self, body: serde_json::Value) -> Result<()> {
        self.sqs
            .send_message()
            .queue_url(&self.dlq_url)
            .message_body(body.to_string())
            .send()
            .await
            .map_err(aws_err)?;
        m::DLQ_SENT.inc();
        Ok(())
    }

    async fn park_raw(&self, raw: &[u8], reason: &str) -> Result<()> {
        self.send_dlq(serde_json::json!({
            "reason": reason,
            "raw": String::from_utf8_lossy(raw),
            "failed_at": chrono::Utc::now(),
        }))
        .await
    }

    /// Dead-letter path, in order:
    ///
    /// 1. preserve the message in SQS
    /// 2. give the stock back
    /// 3. page humans
    ///
    /// Steps 1-2 must succeed (the caller requeues otherwise) and are idempotent.
    /// Step 3 is best effort: an alert failure must never block cleanup.
    async fn dead_letter(
        &self,
        cmd: &OrderCommand,
        raw: &[u8],
        reason: &str,
        attempts: u32,
    ) -> Result<()> {
        self.send_dlq(serde_json::json!({
            "order_id": cmd.order_id,
            "sku": cmd.sku,
            "qty": cmd.qty,
            "reason": reason,
            "attempts": attempts,
            "failed_at": chrono::Utc::now(),
            "command": String::from_utf8_lossy(raw),
        }))
        .await?;

        let mut conn = self.redis.clone();
        let res: Vec<i64> = self
            .release_once
            .key(keys::stock(&cmd.sku))
            .key(keys::stock_version(&cmd.sku))
            .key(keys::idempotency(&cmd.sku, &cmd.idempotency_key))
            .key(keys::released(&cmd.sku, &cmd.order_id.to_string()))
            .arg(cmd.qty)
            .arg(7 * 24 * 3600)
            .invoke_async(&mut conn)
            .await?;
        if res.first().is_some_and(|r| *r >= 0) {
            m::STOCK_RELEASED.inc();
        }

        let alert = self
            .sns
            .publish()
            .topic_arn(&self.alert_topic_arn)
            .subject("PShop: payment dead-lettered")
            .message(format!(
                "Order {} (sku {}, qty {}) failed {} attempts and was parked in the DLQ.\nLast error: {}",
                cmd.order_id, cmd.sku, cmd.qty, attempts, reason
            ))
            .send()
            .await;
        if let Err(e) = alert {
            tracing::error!(error = %aws_err(e), "SNS alert failed (non-fatal)");
        }
        Ok(())
    }
}
