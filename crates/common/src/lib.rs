//! Shared building blocks for every PShop service.
//!
//! Design rule: anything two services must agree on (Redis key layout, Lua
//! scripts, queue topology, event schemas, metric buckets) lives here so that
//! the contract is compiled, not documented.

pub mod aws;
pub mod catalogue;
pub mod config;
pub mod events;
pub mod keys;
pub mod metrics;
pub mod mq;
pub mod secrets;
pub mod telemetry;
pub mod util;
