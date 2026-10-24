//! Redis key layout + the Lua scripts that mutate stock.
//!
//! Every key for one SKU carries the same `{sku}` hash tag, so all keys touched
//! by a script land in one slot. The scripts therefore stay valid unchanged if
//! Redis is later moved to Redis Cluster.
//!
//! Redis is the *source of truth* for stock. Postgres holds settled orders and
//! Elasticsearch holds a derived, eventually-consistent projection.

pub fn stock(sku: &str) -> String {
    format!("stock:{{{sku}}}")
}
pub fn stock_version(sku: &str) -> String {
    format!("stockver:{{{sku}}}")
}
pub fn price(sku: &str) -> String {
    format!("price:{{{sku}}}")
}
pub fn idempotency(sku: &str, key: &str) -> String {
    format!("idem:{{{sku}}}:{key}")
}

/// Marker that makes the dead-letter compensation idempotent.
pub fn released(sku: &str, order_id: &str) -> String {
    format!("released:{{{sku}}}:{order_id}")
}

pub const STOCK_SCAN_PATTERN: &str = "stock:{*}";

/// `stock:{SKU-1}` -> `SKU-1`
pub fn sku_from_stock_key(key: &str) -> Option<&str> {
    key.strip_prefix("stock:{")?.strip_suffix('}')
}

/// Atomic reservation.
///
/// KEYS: stock, stock_version, idempotency, price
/// ARGV: qty, idempotency_ttl_seconds, order_id
/// Returns:
///   {0, stock}                       insufficient stock
///   {1, remaining, version, price}   reserved
///   {2, existing_order_id}           duplicate idempotency key (replay)
///   {3}                              unknown SKU
pub const RESERVE_LUA: &str = r#"
local existing = redis.call('GET', KEYS[3])
if existing then return {2, existing} end
local stock = tonumber(redis.call('GET', KEYS[1]))
if stock == nil then return {3} end
local qty = tonumber(ARGV[1])
if stock < qty then return {0, stock} end
local remaining = redis.call('DECRBY', KEYS[1], qty)
local version = redis.call('INCR', KEYS[2])
redis.call('SET', KEYS[3], ARGV[3], 'EX', ARGV[2])
local price = tonumber(redis.call('GET', KEYS[4]) or '0')
return {1, remaining, version, price}
"#;

/// Compensation: give the reservation back (publish failed, or the payment was
/// dead-lettered). Bumps the version so the Elasticsearch projection converges.
///
/// KEYS: stock, stock_version, idempotency
/// ARGV: qty
/// Returns {remaining, version}
pub const RELEASE_LUA: &str = r#"
local remaining = redis.call('INCRBY', KEYS[1], ARGV[1])
local version = redis.call('INCR', KEYS[2])
redis.call('DEL', KEYS[3])
return {remaining, version}
"#;

/// Same as `RELEASE_LUA` but safe to run any number of times per order: a
/// marker key makes every call after the first a no-op. The billing service
/// needs this because dead-letter handling is at-least-once.
///
/// KEYS: stock, stock_version, idempotency, released_marker
/// ARGV: qty, marker_ttl_seconds
/// Returns {remaining, version} or {-1, -1} when already released.
pub const RELEASE_ONCE_LUA: &str = r#"
if not redis.call('SET', KEYS[4], '1', 'NX', 'EX', ARGV[2]) then return {-1, -1} end
local remaining = redis.call('INCRBY', KEYS[1], ARGV[1])
local version = redis.call('INCR', KEYS[2])
redis.call('DEL', KEYS[3])
return {remaining, version}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_layout_uses_hash_tags() {
        assert_eq!(stock("SKU-1"), "stock:{SKU-1}");
        assert_eq!(idempotency("SKU-1", "abc"), "idem:{SKU-1}:abc");
        assert_eq!(sku_from_stock_key("stock:{SKU-1}"), Some("SKU-1"));
        assert_eq!(sku_from_stock_key("price:{SKU-1}"), None);
    }
}
