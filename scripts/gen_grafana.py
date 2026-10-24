#!/usr/bin/env python3
"""Generates infra/grafana/dashboards/pshop.json (keeps the dashboard reviewable as code)."""
import json, pathlib

DS = {"type": "prometheus", "uid": "prometheus"}
panels, pid = [], 0

def panel(title, targets, x, y, w=8, h=8, kind="timeseries", unit=None, stack=False, thresholds=None, desc=None):
    global pid
    pid += 1
    p = {
        "id": pid, "type": kind, "title": title, "datasource": DS,
        "gridPos": {"x": x, "y": y, "w": w, "h": h},
        "targets": [{"refId": chr(65 + i), "expr": e, "legendFormat": l, "datasource": DS}
                    for i, (e, l) in enumerate(targets)],
        "fieldConfig": {"defaults": {}, "overrides": []},
        "options": {},
    }
    d = p["fieldConfig"]["defaults"]
    if unit: d["unit"] = unit
    if desc: p["description"] = desc
    if kind == "timeseries":
        d["custom"] = {"lineWidth": 2, "fillOpacity": 15 if not stack else 40,
                       "stacking": {"mode": "normal" if stack else "none"}}
        p["options"] = {"legend": {"displayMode": "list", "placement": "bottom"}, "tooltip": {"mode": "multi"}}
    if kind == "stat":
        p["options"] = {"reduceOptions": {"calcs": ["lastNotNull"]}, "colorMode": "value", "graphMode": "area"}
    if thresholds:
        d["thresholds"] = {"mode": "absolute", "steps": [{"color": c, "value": v} for v, c in thresholds]}
    panels.append(p)

def row(title, y):
    global pid
    pid += 1
    panels.append({"id": pid, "type": "row", "title": title, "collapsed": False, "gridPos": {"x": 0, "y": y, "w": 24, "h": 1}})

q = lambda qt, h, w: f"histogram_quantile({qt}, sum by (le) (rate({h}_bucket[{w}])))"

row("Edge & order path", 0)
panel("Accepted vs rejected (req/s)", [
    ("sum(rate(pshop_orders_accepted_total[$__rate_interval]))", "accepted (202)"),
    ("sum by (reason) (rate(pshop_orders_rejected_total[$__rate_interval]))", "rejected: {{reason}}"),
], 0, 1, 12, unit="reqps", desc="Rejections are mostly designed back-pressure (sold_out, overload, queue_full), not failures.")
panel("Order accept latency (SLO p99 < 5 ms)", [
    (q(0.5, "pshop_order_accept_seconds", "$__rate_interval"), "p50"),
    (q(0.99, "pshop_order_accept_seconds", "$__rate_interval"), "p99"),
    (q(0.99, "pshop_redis_reserve_seconds", "$__rate_interval"), "redis p99"),
    (q(0.99, "pshop_rabbit_publish_seconds", "$__rate_interval"), "rabbit confirm p99"),
], 12, 1, 12, unit="s", desc="Handler time is dominated by the broker confirm. Compare the three lines to see who is slow.")
panel("In-flight requests (per pod)", [("pshop_orders_inflight", "{{instance}}")], 0, 9, 8)
panel("Ambiguous publishes / failed releases", [
    ("sum(rate(pshop_orders_unknown_outcome_total[$__rate_interval]))", "unknown outcome"),
    ("sum(rate(pshop_stock_release_failures_total[$__rate_interval]))", "release failures"),
], 8, 9, 8, unit="ops")
panel("Nginx active connections", [("nginx_connections_active", "active"), ("nginx_connections_waiting", "waiting")], 16, 9, 8)

row("Queues & pipeline", 17)
panel("RabbitMQ payments.process depth", [
    ('rabbitmq_queue_messages_ready{queue="payments.process"}', "ready"),
    ('rabbitmq_queue_messages_unacked{queue="payments.process"}', "unacked"),
    ('rabbitmq_queue_messages_ready{queue="payments.retry"}', "retry (5s delay)"),
], 0, 18, 8, desc="This is where the 5k rps spike lives while billing drains at its throttled rate.")
panel("Queue in vs out (msg/s)", [
    ('sum(rate(rabbitmq_queue_messages_published_total{queue="payments.process"}[$__rate_interval]))', "published"),
    ("sum(rate(pshop_payments_processed_total{outcome='ok'}[$__rate_interval]))", "settled"),
    ("sum by (outcome) (rate(pshop_payments_processed_total{outcome!='ok'}[$__rate_interval]))", "{{outcome}}"),
], 8, 18, 8, unit="ops")
panel("Time from 202 to persisted (p50/p95)", [
    (q(0.5, "pshop_order_end_to_end_seconds", "1m"), "p50"),
    (q(0.95, "pshop_order_end_to_end_seconds", "1m"), "p95"),
], 16, 18, 8, unit="s", desc="Queue wait + processing: what the throttle costs the customer.")
panel("Payment gateway / S3 / Postgres latency p95", [
    (q(0.95, "pshop_payment_gateway_seconds", "$__rate_interval"), "gateway"),
    (q(0.95, "pshop_invoice_upload_seconds", "$__rate_interval"), "S3 invoice"),
    (q(0.95, "pshop_payment_db_write_seconds", "$__rate_interval"), "Postgres tx (ingestion)"),
], 0, 26, 8, unit="s")
panel("Outbox backlog & Kafka relay", [
    ("max(pshop_outbox_pending)", "pending rows"),
    ("sum(rate(pshop_outbox_published_total[$__rate_interval]))", "published/s"),
], 8, 26, 8)
panel("Search sync lag & dead letters", [
    ("max(pshop_search_sync_lag_seconds)", "ES lag (s)"),
    ("sum(increase(pshop_dlq_messages_total[5m]))", "DLQ msgs / 5m"),
    ("sum(increase(pshop_stock_released_total[5m]))", "stock released / 5m"),
], 16, 26, 8)

row("Data stores & process memory", 34)
panel("Redis ops/s & memory", [
    ("sum(rate(redis_commands_processed_total[$__rate_interval]))", "ops/s"),
    ("redis_memory_used_bytes", "used bytes"),
], 0, 35, 8)
panel("Postgres connections & tx/s", [
    ("sum(pg_stat_activity_count)", "connections"),
    ("sum(rate(pg_stat_database_xact_commit{datname='pshop'}[$__rate_interval]))", "commits/s"),
], 8, 35, 8)
panel("Process RSS by service (memory buffers)", [
    ("process_resident_memory_bytes{job=~'order-service|billing-service|search-sync|metrics-gateway'}", "{{job}} {{instance}}"),
], 16, 35, 8, unit="bytes")

row("Health", 43)
panel("Targets up", [("sum by (job) (up)", "{{job}}")], 0, 44, 12, h=6, kind="stat", thresholds=[(None, "red"), (1, "green")])
panel("Active alerts", [('count(ALERTS{alertstate="firing"}) or vector(0)', "firing")], 12, 44, 12, h=6, kind="stat",
      thresholds=[(None, "green"), (1, "red")])

dash = {
    "uid": "pshop-main", "title": "PShop Engine", "tags": ["pshop"], "timezone": "browser",
    "schemaVersion": 39, "version": 1, "refresh": "5s", "time": {"from": "now-15m", "to": "now"},
    "panels": panels, "templating": {"list": []},
}
out = pathlib.Path(__file__).resolve().parent.parent / "infra/grafana/dashboards/pshop.json"
out.write_text(json.dumps(dash, indent=2) + "\n")
print(f"wrote {out} ({len(panels)} panels)")
