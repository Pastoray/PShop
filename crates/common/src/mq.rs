//! RabbitMQ topology + a reconnecting, publisher-confirmed publisher.
//!
//! Topology (declared idempotently by every service that touches it):
//!
//! ```text
//!  order-service --publish(confirm)--> [pshop.orders] --payment.process--> (payments.process)
//!                                                                               |
//!                                                                         billing-service
//!                                                                               |  transient failure
//!  (payments.retry, TTL 5s) <--payment.retry-- [pshop.retry] <---------------------+
//!        |  expires -> dead-letters back to [pshop.orders]/payment.process
//!        +--------------------------------------------------------------> (payments.process)
//! ```
//!
//! After `MAX_ATTEMPTS` the billing service parks the message in the SQS DLQ
//! and raises an SNS alert (retry bookkeeping lives in the `x-attempt` header).
//!
//! `payments.process` is length-capped with `x-overflow: reject-publish`: when
//! billing falls too far behind, the broker *nacks* the publisher-confirm and the
//! order service sheds load with 503 + stock release instead of growing an
//! unbounded backlog. That is the end-of-pipe back-pressure signal.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    time::Duration,
};

use anyhow::Result;
use lapin::{
    options::{
        BasicPublishOptions, ConfirmSelectOptions, ExchangeDeclareOptions, QueueBindOptions,
        QueueDeclareOptions,
    },
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Confirmation, Connection, ConnectionProperties, ExchangeKind,
};

use crate::util::retry;

pub const EXCHANGE: &str = "pshop.orders";
pub const RK_PAYMENT: &str = "payment.process";
pub const QUEUE_PAYMENTS: &str = "payments.process";

pub const EXCHANGE_RETRY: &str = "pshop.retry";
pub const RK_RETRY: &str = "payment.retry";
pub const QUEUE_RETRY: &str = "payments.retry";
pub const RETRY_DELAY_MS: u32 = 5_000;

pub const HEADER_ATTEMPT: &str = "x-attempt";

pub async fn connect(url: &str) -> Result<Connection> {
    let conn = retry("rabbitmq connect", 60, Duration::from_millis(500), || {
        Connection::connect(url, ConnectionProperties::default())
    })
    .await?;
    Ok(conn)
}

pub async fn declare_topology(ch: &Channel, queue_max_len: u32) -> Result<()> {
    let durable_ex = ExchangeDeclareOptions {
        durable: true,
        ..Default::default()
    };
    let durable_q = QueueDeclareOptions {
        durable: true,
        ..Default::default()
    };

    ch.exchange_declare(
        EXCHANGE.into(),
        ExchangeKind::Direct,
        durable_ex,
        FieldTable::default(),
    )
    .await?;
    ch.exchange_declare(
        EXCHANGE_RETRY.into(),
        ExchangeKind::Direct,
        durable_ex,
        FieldTable::default(),
    )
    .await?;

    let mut main_args = FieldTable::default();
    main_args.insert("x-max-length".into(), AMQPValue::LongUInt(queue_max_len));
    main_args.insert(
        "x-overflow".into(),
        AMQPValue::LongString("reject-publish".into()),
    );
    ch.queue_declare(QUEUE_PAYMENTS.into(), durable_q, main_args)
        .await?;
    ch.queue_bind(
        QUEUE_PAYMENTS.into(),
        EXCHANGE.into(),
        RK_PAYMENT.into(),
        QueueBindOptions::default(),
        FieldTable::default(),
    )
    .await?;

    let mut retry_args = FieldTable::default();
    retry_args.insert("x-message-ttl".into(), AMQPValue::LongUInt(RETRY_DELAY_MS));
    retry_args.insert(
        "x-dead-letter-exchange".into(),
        AMQPValue::LongString(EXCHANGE.into()),
    );
    retry_args.insert(
        "x-dead-letter-routing-key".into(),
        AMQPValue::LongString(RK_PAYMENT.into()),
    );
    ch.queue_declare(QUEUE_RETRY.into(), durable_q, retry_args)
        .await?;
    ch.queue_bind(
        QUEUE_RETRY.into(),
        EXCHANGE_RETRY.into(),
        RK_RETRY.into(),
        QueueBindOptions::default(),
        FieldTable::default(),
    )
    .await?;
    Ok(())
}

pub fn persistent_json() -> BasicProperties {
    BasicProperties::default()
        .with_delivery_mode(2) // survive broker restart
        .with_content_type("application/json".into())
}

#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("broker connection not ready")]
    NotReady,
    #[error("broker nacked the publish (queue full or internal error)")]
    Nacked,
    #[error("amqp error: {0}")]
    Amqp(#[from] lapin::Error),
}

/// Pool of confirm-mode channels behind a supervisor that reconnects forever.
///
/// * `publish` resolves only after the broker *confirmed* the message, so a
///   caller that returns HTTP 202 afterwards has a durable hand-off.
/// * While the broker is unreachable `publish` fails immediately (`NotReady`)
///   rather than queueing in memory, which would hide the outage and balloon RSS.
pub struct ConfirmedPublisher {
    pool: RwLock<Option<Arc<Vec<Channel>>>>,
    rr: AtomicUsize,
    ready: AtomicBool,
}

impl ConfirmedPublisher {
    pub fn spawn(url: String, channels: usize, queue_max_len: u32) -> Arc<Self> {
        let this = Arc::new(Self {
            pool: RwLock::new(None),
            rr: AtomicUsize::new(0),
            ready: AtomicBool::new(false),
        });
        tokio::spawn(Arc::clone(&this).supervise(url, channels.max(1), queue_max_len));
        this
    }

    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }

    async fn supervise(self: Arc<Self>, url: String, channels: usize, queue_max_len: u32) {
        loop {
            match self.build(&url, channels, queue_max_len).await {
                Ok(conn) => {
                    tracing::info!(channels, "rabbitmq publisher ready");
                    while conn.status().connected() {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                    tracing::error!("rabbitmq connection lost; publisher entering reconnect loop");
                    self.ready.store(false, Ordering::Relaxed);
                    *self.pool.write().unwrap() = None;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "rabbitmq connect failed; retrying");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }

    async fn build(&self, url: &str, channels: usize, queue_max_len: u32) -> Result<Connection> {
        let conn = Connection::connect(url, ConnectionProperties::default()).await?;
        let mut pool = Vec::with_capacity(channels);
        for i in 0..channels {
            let ch = conn.create_channel().await?;
            if i == 0 {
                declare_topology(&ch, queue_max_len).await?;
            }
            ch.confirm_select(ConfirmSelectOptions::default()).await?;
            pool.push(ch);
        }
        *self.pool.write().unwrap() = Some(Arc::new(pool));
        self.ready.store(true, Ordering::Relaxed);
        Ok(conn)
    }

    pub async fn publish(
        &self,
        exchange: &str,
        routing_key: &str,
        body: &[u8],
        props: BasicProperties,
    ) -> Result<(), PublishError> {
        let pool = self
            .pool
            .read()
            .unwrap()
            .clone()
            .ok_or(PublishError::NotReady)?;
        let idx = self.rr.fetch_add(1, Ordering::Relaxed) % pool.len();
        let confirm = pool[idx]
            .basic_publish(
                exchange.into(),
                routing_key.into(),
                BasicPublishOptions::default(),
                body,
                props,
            )
            .await?
            .await?;
        match confirm {
            Confirmation::Nack(_) => Err(PublishError::Nacked),
            _ => Ok(()),
        }
    }
}
