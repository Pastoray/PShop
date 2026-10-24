//! Wire schemas. `OrderCommand` travels on RabbitMQ (work to do, exactly one
//! consumer). `OrderCompleted` travels on Kafka (fact that happened, many
//! independent consumers, replayable).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const KAFKA_TOPIC_ORDER_COMPLETED: &str = "pshop.orders.completed";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCommand {
    pub order_id: Uuid,
    pub user_id: String,
    pub sku: String,
    pub qty: u32,
    pub unit_price_cents: u64,
    /// Stock left in Redis right after this reservation.
    pub stock_remaining: i64,
    /// Monotonic per-SKU version assigned by the reservation script.
    pub stock_version: i64,
    pub idempotency_key: String,
    pub accepted_at: DateTime<Utc>,
}

impl OrderCommand {
    pub fn total_cents(&self) -> u64 {
        self.unit_price_cents * u64::from(self.qty)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderCompleted {
    pub order_id: Uuid,
    pub user_id: String,
    pub sku: String,
    pub qty: u32,
    pub total_cents: u64,
    pub stock_remaining: i64,
    pub stock_version: i64,
    pub invoice_key: String,
    pub accepted_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
}
