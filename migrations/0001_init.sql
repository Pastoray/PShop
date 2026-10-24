-- PShop: ACID order persistence + transactional outbox.
-- Owned and applied by billing-service at startup (sqlx advisory-locks the
-- migration run, so N replicas starting at once is safe).

CREATE TABLE orders (
    order_id         uuid        PRIMARY KEY,           -- UUIDv7: time-ordered, index-friendly
    user_id          text        NOT NULL,
    sku              text        NOT NULL,
    qty              integer     NOT NULL CHECK (qty > 0),
    unit_price_cents bigint      NOT NULL CHECK (unit_price_cents >= 0),
    total_cents      bigint      NOT NULL CHECK (total_cents >= 0),
    status           text        NOT NULL DEFAULT 'PAID',
    invoice_key      text        NOT NULL,
    idempotency_key  text        NOT NULL,
    accepted_at      timestamptz NOT NULL,
    paid_at          timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX orders_user_paid_idx ON orders (user_id, paid_at DESC);
CREATE INDEX orders_sku_paid_idx  ON orders (sku, paid_at DESC);

-- Transactional outbox: the OrderCompleted event is committed in the SAME
-- transaction as the order, then relayed to Kafka. This removes the classic
-- dual-write hole (row committed but event lost, or event sent for a rolled
-- back order).
CREATE TABLE outbox (
    id           bigserial   PRIMARY KEY,
    topic        text        NOT NULL,
    key          text        NOT NULL,                  -- Kafka key (sku) => per-SKU ordering
    payload      jsonb       NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    published_at timestamptz
);

-- Partial index: stays tiny because published rows drop out of it.
CREATE INDEX outbox_unpublished_idx ON outbox (id) WHERE published_at IS NULL;
