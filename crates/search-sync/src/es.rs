//! Thin Elasticsearch client over plain REST (`reqwest`), covering just what we need:
//! index bootstrap, catalogue seeding, version-gated stock bulk updates, search.

use std::{collections::HashMap, time::Duration};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize)]
pub struct StockUpdate {
    pub sku: String,
    pub stock: i64,
    pub version: i64,
}

/// Several events for one SKU in a batch collapse into the newest one: ES
/// would discard the older ones anyway, so don't make it do the work.
pub fn coalesce(updates: impl IntoIterator<Item = StockUpdate>) -> Vec<StockUpdate> {
    let mut best: HashMap<String, StockUpdate> = HashMap::new();
    for u in updates {
        match best.get(&u.sku) {
            Some(cur) if cur.version >= u.version => {}
            _ => {
                best.insert(u.sku.clone(), u);
            }
        }
    }
    best.into_values().collect()
}

pub struct Es {
    http: reqwest::Client,
    base: String,
    index: String,
}

#[derive(Debug, Default)]
pub struct BulkOutcome {
    pub applied: usize,
    pub rejected_permanent: usize,
}

/// Only applies an update whose version is strictly newer than what is stored.
/// => idempotent (replays are no-ops) and order-insensitive (late events lose).
const GATED_UPDATE: &str = "if (ctx._source.stock_version == null || params.v > ctx._source.stock_version) { \
    ctx._source.stock = params.stock; ctx._source.stock_version = params.v; ctx._source.updated_at = params.ts; \
} else { ctx.op = 'none'; }";

impl Es {
    pub fn new(base: String, index: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(8)
            .build()?;
        Ok(Self {
            http,
            base: base.trim_end_matches('/').to_string(),
            index,
        })
    }

    pub async fn ping(&self) -> Result<()> {
        let r = self
            .http
            .get(format!(
                "{}/_cluster/health?wait_for_status=yellow&timeout=2s",
                self.base
            ))
            .send()
            .await?;
        if !r.status().is_success() {
            bail!("elasticsearch not healthy: {}", r.status());
        }
        Ok(())
    }

    pub async fn ensure_index(&self) -> Result<()> {
        let url = format!("{}/{}", self.base, self.index);
        if self.http.head(&url).send().await?.status().is_success() {
            return Ok(());
        }
        let body = json!({
            "settings": { "number_of_shards": 1, "number_of_replicas": 0, "refresh_interval": "1s" },
            "mappings": { "properties": {
                "sku":           { "type": "keyword" },
                "name":          { "type": "text", "fields": { "raw": { "type": "keyword" } } },
                "category":      { "type": "keyword" },
                "description":   { "type": "text" },
                "price_cents":   { "type": "long" },
                "stock":         { "type": "long" },
                "stock_version": { "type": "long" },
                "updated_at":    { "type": "date" }
            }}
        });
        let r = self.http.put(&url).json(&body).send().await?;
        // 400 resource_already_exists_exception = another replica won the race.
        if !r.status().is_success() && r.status().as_u16() != 400 {
            bail!(
                "create index failed: {} {}",
                r.status(),
                r.text().await.unwrap_or_default()
            );
        }
        Ok(())
    }

    /// `create` (not `index`): never overwrites a live, already-synced document.
    pub async fn seed_catalogue(&self, skus: u32, stock: i64) -> Result<()> {
        if skus == 0 {
            return Ok(());
        }
        let mut body = String::new();
        for i in 1..=skus {
            let p = pshop_common::catalogue::product(i);
            body.push_str(&format!("{}\n", json!({ "create": { "_id": p.sku } })));
            body.push_str(&format!(
                "{}\n",
                json!({ "sku": p.sku, "name": p.name, "category": p.category,
                        "description": p.description, "price_cents": p.price_cents,
                        "stock": stock, "stock_version": 0, "updated_at": chrono::Utc::now() })
            ));
        }
        self.bulk(body).await.context("seeding catalogue")?;
        Ok(())
    }

    pub async fn bulk_stock(&self, updates: &[StockUpdate]) -> Result<BulkOutcome> {
        if updates.is_empty() {
            return Ok(BulkOutcome::default());
        }
        let now = chrono::Utc::now();
        let mut body = String::with_capacity(updates.len() * 256);
        for u in updates {
            body.push_str(&format!(
                "{}\n",
                json!({ "update": { "_id": u.sku, "retry_on_conflict": 5 } })
            ));
            body.push_str(&format!(
                "{}\n",
                json!({
                    "script": { "lang": "painless", "source": GATED_UPDATE,
                                "params": { "stock": u.stock, "v": u.version, "ts": now } },
                    // Document unknown to the index yet (event beat the catalogue): create it.
                    "upsert": { "sku": u.sku, "stock": u.stock, "stock_version": u.version, "updated_at": now }
                })
            ));
        }
        self.bulk(body).await
    }

    async fn bulk(&self, ndjson: String) -> Result<BulkOutcome> {
        let resp = self
            .http
            .post(format!("{}/{}/_bulk", self.base, self.index))
            .header("content-type", "application/x-ndjson")
            .body(ndjson)
            .send()
            .await?;
        let status = resp.status();
        if status.as_u16() == 429 || status.is_server_error() {
            bail!("elasticsearch busy/unavailable: {status}");
        }
        let v: Value = resp.json().await.context("decoding bulk response")?;
        classify_bulk(&v)
    }

    /// Free-text search with an optional category filter and paging.
    /// An empty query lists the catalogue in SKU order (stable paging for the storefront grid).
    pub async fn search(
        &self,
        q: &str,
        category: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Value> {
        let text = if q.trim().is_empty() {
            json!({ "match_all": {} })
        } else {
            json!({ "bool": { "minimum_should_match": 1, "should": [
                { "match":  { "name": { "query": q, "fuzziness": "AUTO" } } },
                { "match":  { "category": { "query": q, "boost": 0.5 } } },
                { "prefix": { "sku":  q.to_uppercase() } }
            ]}})
        };
        let filters: Vec<Value> = category
            .map(|c| json!({ "term": { "category": c } }))
            .into_iter()
            .collect();
        let mut body = json!({
            "size": limit.min(100),
            "from": offset.min(10_000),
            "query": { "bool": { "must": [text], "filter": filters } },
        });
        if q.trim().is_empty() {
            body["sort"] = json!([{ "sku": "asc" }]);
        }
        let r = self
            .http
            .post(format!("{}/{}/_search", self.base, self.index))
            .json(&body)
            .send()
            .await?;
        if !r.status().is_success() {
            bail!("search failed: {}", r.status());
        }
        let v: Value = r.json().await?;
        let hits: Vec<Value> = v["hits"]["hits"]
            .as_array()
            .map(|a| a.iter().map(|h| h["_source"].clone()).collect())
            .unwrap_or_default();
        Ok(json!({ "total": v["hits"]["total"]["value"], "hits": hits }))
    }

    /// One product by SKU (real-time GET: sees a stock update as soon as it is applied,
    /// without waiting for the index refresh). `None` = no such product.
    pub async fn get_product(&self, sku: &str) -> Result<Option<Value>> {
        let r = self
            .http
            .get(format!("{}/{}/_doc/{}", self.base, self.index, sku))
            .send()
            .await?;
        if r.status().as_u16() == 404 {
            return Ok(None);
        }
        if !r.status().is_success() {
            bail!("get product failed: {}", r.status());
        }
        let v: Value = r.json().await?;
        Ok(Some(v["_source"].clone()))
    }
}

/// Retryable item failures (429/5xx) make the whole call fail so the caller
/// retries the batch (safe: the version gate makes re-application a no-op).
/// Other 4xx are permanent for that document and are counted, not retried.
fn classify_bulk(v: &Value) -> Result<BulkOutcome> {
    let mut out = BulkOutcome::default();
    if v["errors"] == json!(false) {
        out.applied = v["items"].as_array().map_or(0, Vec::len);
        return Ok(out);
    }
    for item in v["items"].as_array().into_iter().flatten() {
        let op = item
            .as_object()
            .and_then(|o| o.values().next())
            .cloned()
            .unwrap_or(Value::Null);
        let status = op["status"].as_u64().unwrap_or(500);
        match status {
            200..=299 | 409 => out.applied += 1, // 409: create on existing doc / version conflict = already converged
            429 | 500..=599 => bail!("retryable bulk item failure: status {status}"),
            _ => {
                out.rejected_permanent += 1;
                tracing::error!(%op, "permanent bulk item failure");
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(sku: &str, v: i64, s: i64) -> StockUpdate {
        StockUpdate {
            sku: sku.into(),
            stock: s,
            version: v,
        }
    }

    #[test]
    fn coalesce_keeps_newest_version_per_sku() {
        let mut out = coalesce(vec![u("A", 3, 7), u("A", 5, 5), u("A", 4, 6), u("B", 1, 9)]);
        out.sort_by(|a, b| a.sku.cmp(&b.sku));
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].version, out[0].stock), (5, 5));
        assert_eq!(out[1].sku, "B");
    }

    #[test]
    fn classify_bulk_distinguishes_retryable_from_permanent() {
        let ok = json!({ "errors": false, "items": [ { "update": { "status": 200 } } ] });
        assert_eq!(classify_bulk(&ok).unwrap().applied, 1);

        let conflict = json!({ "errors": true, "items": [ { "update": { "status": 409 } } ] });
        assert_eq!(classify_bulk(&conflict).unwrap().applied, 1);

        let busy = json!({ "errors": true, "items": [ { "update": { "status": 429 } } ] });
        assert!(classify_bulk(&busy).is_err());

        let bad = json!({ "errors": true, "items": [ { "update": { "status": 400 } } ] });
        assert_eq!(classify_bulk(&bad).unwrap().rejected_permanent, 1);
    }
}
