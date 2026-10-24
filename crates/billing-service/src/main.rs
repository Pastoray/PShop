//! Billing / Payment Service.
//!
//! RabbitMQ -> (throttled) -> payment -> S3 invoice -> Postgres (+outbox) -> Kafka.
//! Secrets (Postgres credentials, invoice signing key) come from AWS Secrets Manager.

mod api;
mod control;
mod invoice;
mod m;
mod outbox;
mod worker;

use std::{
    sync::{atomic::AtomicBool, atomic::Ordering, Arc},
    time::Duration,
};

use anyhow::Result;
use axum::{extract::State, http::StatusCode, routing::get, Router};
use pshop_common::{
    aws,
    config::{env_or, env_parse},
    metrics,
    mq::ConfirmedPublisher,
    secrets, telemetry, util,
};
use redis::{aio::ConnectionManager, Script};
use sqlx::{postgres::PgConnectOptions, postgres::PgPoolOptions, PgPool};
use tokio::sync::Semaphore;

#[derive(Clone)]
struct Health {
    consumer_ready: Arc<AtomicBool>,
    pool: PgPool,
}

async fn readyz(State(h): State<Health>) -> StatusCode {
    if !h.consumer_ready.load(Ordering::Relaxed) {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    match tokio::time::timeout(
        Duration::from_millis(300),
        sqlx::query("SELECT 1").execute(&h.pool),
    )
    .await
    {
        Ok(Ok(_)) => StatusCode::OK,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    telemetry::init_tracing("billing-service");
    m::force_all();

    // --- secrets (fail fast if they are not obtainable) ----------------------
    let pg = secrets::load_postgres().await?;
    let keys = secrets::load_app_keys().await?;
    tracing::info!(?pg, "postgres secret loaded");

    // --- Postgres ---------------------------------------------------------------
    let opts = PgConnectOptions::new()
        .host(&pg.host)
        .port(pg.port)
        .username(&pg.username)
        .password(&pg.password)
        .database(&pg.dbname);
    let pool = util::retry("postgres connect", 60, Duration::from_millis(500), || {
        PgPoolOptions::new()
            .max_connections(env_parse("PG_POOL_SIZE", 16))
            .acquire_timeout(Duration::from_secs(3))
            .connect_with(opts.clone())
    })
    .await?;
    sqlx::migrate!("../../migrations").run(&pool).await?;
    tracing::info!("migrations applied");

    // --- AWS (LocalStack in dev) ------------------------------------------------
    let sdk = aws::sdk_config().await;
    let s3 = aws_sdk_s3::Client::from_conf(
        aws_sdk_s3::config::Builder::from(&sdk)
            .force_path_style(aws::endpoint_overridden())
            .build(),
    );
    let sqs = aws_sdk_sqs::Client::new(&sdk);
    let sns = aws_sdk_sns::Client::new(&sdk);
    let dlq_name = env_or("DLQ_QUEUE_NAME", "pshop-payments-dlq");
    let dlq_url = util::retry(
        "resolve DLQ url",
        60,
        Duration::from_millis(500),
        || async {
            sqs.get_queue_url()
                .queue_name(&dlq_name)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("{}", aws_sdk_sqs::error::DisplayErrorContext(&e)))
        },
    )
    .await?
    .queue_url
    .ok_or_else(|| anyhow::anyhow!("DLQ has no url"))?;

    // --- Redis (stock compensation only) ---------------------------------------
    let redis_client = redis::Client::open(env_or("REDIS_URL", "redis://127.0.0.1:6379"))?;
    let redis = util::retry("redis connect", 60, Duration::from_millis(500), || {
        ConnectionManager::new(redis_client.clone())
    })
    .await?;

    // --- messaging ---------------------------------------------------------------
    let amqp_url = env_or("AMQP_URL", "amqp://guest:guest@127.0.0.1:5672/%2f");
    let queue_max_len: u32 = env_parse("QUEUE_MAX_LEN", 500_000);
    let retry_publisher = ConfirmedPublisher::spawn(amqp_url.clone(), 2, queue_max_len);
    let kafka = outbox::producer(&env_or("KAFKA_BROKERS", "127.0.0.1:9092"))?;

    let invoice_bucket = env_or("INVOICE_BUCKET", "pshop-invoices");
    let api_state = api::ApiState {
        pool: pool.clone(),
        s3: s3.clone(),
        bucket: invoice_bucket.clone(),
    };

    let control = control::Control::new(Duration::from_millis(env_parse(
        "SLOW_PAYMENT_MS",
        250,
    )));
    let control_enabled = env_or("CONTROL_ENABLED", "false") == "true";
    if control_enabled {
        tokio::spawn(control.clone().follow(redis.clone()));
        tracing::warn!("demo control plane ENABLED (/v1/control/payments)");
    }

    let ctx = Arc::new(worker::Ctx {
        pool: pool.clone(),
        s3,
        sqs,
        sns,
        redis,
        release_once: Script::new(pshop_common::keys::RELEASE_ONCE_LUA),
        retry_publisher,
        bucket: invoice_bucket,
        dlq_url,
        alert_topic_arn: env_or(
            "ALERT_TOPIC_ARN",
            "arn:aws:sns:us-east-1:000000000000:pshop-payment-alerts",
        ),
        signing_key: keys.invoice_signing_key,
        cfg: worker::Cfg {
            max_attempts: env_parse("MAX_ATTEMPTS", 3),
            payment_latency_ms: env_parse("PAYMENT_LATENCY_MS", 20),
            payment_failure_rate: env_parse("PAYMENT_FAILURE_RATE", 0.02),
            queue_max_len,
        },
        control: control.clone(),
    });

    // --- background tasks -------------------------------------------------------
    let ready = Arc::new(AtomicBool::new(false));
    let max_concurrency: usize = env_parse("BILLING_CONCURRENCY", 64);
    let permits = Arc::new(Semaphore::new(max_concurrency));

    tokio::spawn(outbox::relay(
        pool.clone(),
        kafka,
        env_parse("OUTBOX_BATCH", 200),
        Duration::from_millis(env_parse("OUTBOX_POLL_MS", 100)),
    ));
    tokio::spawn(outbox::housekeeping(pool.clone()));
    tokio::spawn(worker::run(
        Arc::clone(&ctx),
        amqp_url,
        env_parse("BILLING_PREFETCH", 64),
        env_parse("BILLING_MAX_RPS", 500),
        max_concurrency,
        Arc::clone(&ready),
        Arc::clone(&permits),
    ));

    // --- http (metrics, probes, storefront read API) ---------------------------------------------------
    let mut app = Router::new()
        .route("/readyz", get(readyz))
        .with_state(Health {
            consumer_ready: ready,
            pool: pool.clone(),
        })
        .merge(api::router(api_state));
    if control_enabled {
        app = app.merge(control::router(control::ApiState {
            control,
            redis: ctx.redis.clone(),
        }));
    }
    let app = app.merge(metrics::base_router());
    metrics::serve(&env_or("BIND_ADDR", "0.0.0.0:8080"), app).await?;

    // --- graceful drain: let in-flight payments finish before exiting --------------
    tracing::info!("draining in-flight payments");
    let _ = tokio::time::timeout(
        Duration::from_secs(25),
        permits.acquire_many(u32::try_from(max_concurrency).unwrap_or(u32::MAX)),
    )
    .await;
    Ok(())
}
