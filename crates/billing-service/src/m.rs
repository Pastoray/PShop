//! Metric definitions for the billing pipeline.

use std::sync::LazyLock;

use prometheus::{
    register_histogram, register_int_counter, register_int_counter_vec, register_int_gauge,
    Histogram, IntCounter, IntCounterVec, IntGauge,
};
use pshop_common::metrics::SLOW_BUCKETS;

pub static PROCESSED: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "pshop_payments_processed_total",
        "Payment commands by outcome (ok|duplicate|retry|dead_lettered|malformed)",
        &["outcome"]
    )
    .unwrap()
});
pub static INFLIGHT: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!(
        "pshop_billing_inflight",
        "Payments being processed right now"
    )
    .unwrap()
});
pub static GATEWAY: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_payment_gateway_seconds",
        "Simulated payment gateway latency",
        SLOW_BUCKETS.to_vec()
    )
    .unwrap()
});
pub static INVOICE_UPLOAD: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_invoice_upload_seconds",
        "S3 PutObject latency",
        SLOW_BUCKETS.to_vec()
    )
    .unwrap()
});
pub static DB_WRITE: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_payment_db_write_seconds",
        "Postgres order+outbox transaction latency (ingestion latency)",
        SLOW_BUCKETS.to_vec()
    )
    .unwrap()
});
pub static END_TO_END: LazyLock<Histogram> = LazyLock::new(|| {
    register_histogram!(
        "pshop_order_end_to_end_seconds",
        "HTTP 202 -> order persisted (queue wait + processing)",
        vec![0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0, 600.0]
    )
    .unwrap()
});
pub static OUTBOX_PENDING: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!(
        "pshop_outbox_pending",
        "Outbox rows not yet confirmed by Kafka"
    )
    .unwrap()
});
pub static OUTBOX_PUBLISHED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!("pshop_outbox_published_total", "Events relayed to Kafka").unwrap()
});
pub static OUTBOX_FAILURES: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_outbox_publish_failures_total",
        "Kafka publish failures in the relay"
    )
    .unwrap()
});
pub static DLQ_SENT: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!("pshop_dlq_messages_total", "Messages parked in the SQS DLQ").unwrap()
});
pub static STOCK_RELEASED: LazyLock<IntCounter> = LazyLock::new(|| {
    register_int_counter!(
        "pshop_stock_released_total",
        "Reservations returned after dead-lettering"
    )
    .unwrap()
});

pub static PAYMENTS_MODE: LazyLock<IntGauge> = LazyLock::new(|| {
    register_int_gauge!(
        "pshop_payments_mode",
        "Simulated payment provider mode (0 normal, 1 slow, 2 down)"
    )
    .unwrap()
});

pub fn force_all() {
    LazyLock::force(&PAYMENTS_MODE);
    LazyLock::force(&INFLIGHT);
    LazyLock::force(&GATEWAY);
    LazyLock::force(&INVOICE_UPLOAD);
    LazyLock::force(&DB_WRITE);
    LazyLock::force(&END_TO_END);
    LazyLock::force(&OUTBOX_PENDING);
    LazyLock::force(&OUTBOX_PUBLISHED);
    LazyLock::force(&OUTBOX_FAILURES);
    LazyLock::force(&DLQ_SENT);
    LazyLock::force(&STOCK_RELEASED);
    for o in ["ok", "duplicate", "retry", "dead_lettered", "malformed"] {
        PROCESSED.with_label_values(&[o]).inc_by(0);
    }
}
