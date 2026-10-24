//! Read API for the storefront: "where is my order?" and "give me my invoice".
//!
//! These live in billing-service because it owns the `orders` table and the invoice bucket; the
//! order-service stays free of Postgres/S3 so the hot path cannot be slowed by them.
//!
//! An order id is a UUIDv7 (74 random bits) and acts as an unguessable capability: whoever
//! holds it may view that order. It carries no personal data and the response omits user_id.
//!
//! `404` from /v1/order-status means "not settled (yet)": between the 202 and the Postgres
//! commit the order is legitimately sitting in the payment queue, which can take minutes
//! under load. The storefront shows that as "payment queued", not as an error.

use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct ApiState {
    pub pool: PgPool,
    pub s3: aws_sdk_s3::Client,
    pub bucket: String,
}

pub fn router(state: ApiState) -> Router {
    Router::new()
        .route("/v1/order-status/{id}", get(order_status))
        .route("/v1/invoices/{id}", get(invoice))
        .with_state(state)
}

fn parse_id(raw: &str) -> Option<Uuid> {
    Uuid::parse_str(raw).ok()
}

fn no_store(mut resp: Response) -> Response {
    resp.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    resp
}

type OrderRow = (String, i32, i64, i64, String, DateTime<Utc>, DateTime<Utc>);

async fn order_status(State(st): State<ApiState>, Path(id): Path<String>) -> Response {
    let Some(id) = parse_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let row: Result<Option<OrderRow>, _> = sqlx::query_as(
        "SELECT sku, qty, unit_price_cents, total_cents, status, accepted_at, paid_at \
         FROM orders WHERE order_id = $1",
    )
    .bind(id)
    .fetch_optional(&st.pool)
    .await;

    let resp = match row {
        Ok(Some((sku, qty, unit, total, status, accepted_at, paid_at))) => Json(json!({
            "order_id": id,
            "status": status,
            "sku": sku,
            "qty": qty,
            "unit_price_cents": unit,
            "total_cents": total,
            "accepted_at": accepted_at,
            "paid_at": paid_at,
            "invoice_url": format!("/v1/invoices/{id}"),
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "status": "not_settled_yet" })),
        )
            .into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "order status query failed");
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    };
    no_store(resp)
}

async fn invoice(State(st): State<ApiState>, Path(id): Path<String>) -> Response {
    let Some(id) = parse_id(&id) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let key: Result<Option<String>, _> =
        sqlx::query_scalar("SELECT invoice_key FROM orders WHERE order_id = $1")
            .bind(id)
            .fetch_optional(&st.pool)
            .await;
    let key = match key {
        Ok(Some(k)) => k,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "invoice lookup failed");
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        }
    };

    let obj = match st.s3.get_object().bucket(&st.bucket).key(&key).send().await {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!(error = %aws_sdk_s3::error::DisplayErrorContext(&e), %key, "invoice fetch failed");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let bytes = match obj.body.collect().await {
        Ok(b) => b.into_bytes().to_vec(),
        Err(e) => {
            tracing::warn!(error = %e, %key, "invoice body read failed");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };
    let disposition = format!("inline; filename=\"pshop-invoice-{id}.pdf\"");
    let mut resp = bytes.into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    if let Ok(v) = HeaderValue::from_str(&disposition) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    no_store(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_uuids_are_accepted_as_order_ids() {
        assert!(parse_id("0192f5a4-7b3c-7d3e-8a1b-2c3d4e5f6a7b").is_some());
        assert!(parse_id("not-a-uuid").is_none());
        assert!(parse_id("../../etc/passwd").is_none());
        assert!(parse_id("").is_none());
    }
}
