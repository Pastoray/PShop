# Failure modes

What happens when each dependency fails, what the client sees, what is protected, and how you would notice.

| # | Failure | Behaviour | Client sees | Data safety | Signal |
|---|---|---|---|---|---|
| 1 | **Redis down** | Reservation times out/errors; order-service refuses to accept without a stock decision | `503 redis_unavailable` | Nothing accepted → nothing to lose | `/readyz` fails (pod leaves Service), `ServiceDown`, rejected{reason} |
| 2 | **RabbitMQ down / not connected** | Publisher `NotReady`; reservation **released** | `503 queue_unavailable` | Stock returned, idempotency key cleared so the client can retry | `OrdersBeingShed`, readiness fails |
| 3 | **RabbitMQ queue full** (backlog ≥ `QUEUE_MAX_LEN`) | Broker NACKs the confirm (`reject-publish`); reservation **released** | `503 queue_full`, `Retry-After: 1` | Bounded memory; no unbounded backlog | `PaymentBacklogGrowing`, queue-depth panel |
| 4 | **Publish outcome unknown** (timeout / link drop after send) | Message *may* be queued. Reservation **kept** | `503`, retry with same `Idempotency-Key` | Retry hits the idempotency key and returns the original order id; at worst one unit is held, never oversold | `pshop_orders_unknown_outcome_total` |
| 5 | **Pod flood** (> `MAX_INFLIGHT`) | `try_acquire` fails instantly | `503 overload` | n/a | `OrdersBeingShed` |
| 6 | **Postgres slow/down** | Settle fails → retry queue (5 s TTL, ≤ `MAX_ATTEMPTS`) → DLQ | None: orders were already `202`d | Messages stay in RabbitMQ (durable); orders are not lost | `PaymentsNotDraining`, DB-write latency panel |
| 7 | **Payment gateway transient failure** | Same retry path via `payments.retry` | None | Order settles on a later attempt | `retry` outcome counter |
| 8 | **Retries exhausted** | 1) message to **SQS DLQ**, 2) stock **released** (idempotent marker), 3) **SNS** fan-out to email + SMS queues | None (already `202`d; needs human/refund flow) | Nothing silently dropped; stock not double-credited on redelivery | `DeadLetteredPayments`, `pshop_dlq_messages_total` |
| 9 | **Poison message** (undecodable JSON) | Parked in DLQ, acked | None | Retrying could never succeed; humans can inspect | `outcome="malformed"` |
| 10 | **billing-service crash mid-settle** | Unacked message redelivered | None | `ON CONFLICT DO NOTHING`, deterministic S3 key → effectively-once | redelivery in logs |
| 11 | **Kafka down** | Outbox rows accumulate in Postgres; relay backs off | None | Events are committed with the order, so none are lost; published when Kafka returns | `OutboxStuck`, `pshop_outbox_pending` |
| 12 | **Duplicate Kafka delivery / reordering** | search-sync applies only if `version` is newer | None | ES converges to the highest version | n/a |
| 13 | **Elasticsearch down** | search-sync retries the batch with backoff and **stops consuming**; offsets are not committed | Search stale | Lag accrues in Kafka (durable, cheap), not process memory | `SearchIndexLagging` |
| 14 | **Lost event / failed release** (e.g. stock released on dead-letter emits no event) | Reconcile loop re-projects Redis → ES every 30 s | Search briefly stale | Redis stays authoritative | reconcile counter |
| 15 | **Redis release itself fails** | Counted loudly; stock under-reported until fixed | None | Under-selling, not overselling | `StockReleaseFailing` (critical) |
| 16 | **Secrets Manager unavailable at boot** | billing-service retries with backoff (up to ~5 min); in Kubernetes the startup probe (3 min) restarts the pod first | None | Never starts with default credentials (`SECRETS_BACKEND=env` is an explicit opt-in) | startup probe / restarts |
| 17 | **Redis restart** | AOF (`everysec`) restores stock; up to ~1 s of mutations may be lost | None | Loss window means *possible overselling by ≤ 1 s of traffic*. Accepted trade-off: see below | n/a |
| 18 | **Node loss** | `topologySpreadConstraints` + PDB keep ≥ 2 order pods; HPA replaces capacity | Transient 503s from the dead pod | n/a | `ServiceDown` |

## Design choices worth defending

**Why under-selling over overselling (rows 4, 15).** A held unit is a refundable inconvenience; a sold unit that doesn't exist is a broken promise. When the outcome is ambiguous we choose the recoverable error.

**Why the reservation is released on a definitive NACK but not on a timeout.** A NACK tells us the broker did *not* take the message. A timeout tells us nothing. Releasing on a timeout could hand the same unit to someone else while the original message is still in the queue.

**Why not retry POST at the load balancer.** `proxy_next_upstream` is limited to connect errors. Replaying a request a pod may already have processed would double-reserve stock (unless the client sent an `Idempotency-Key`, which we cannot rely on).

**Why RabbitMQ *and* Kafka.** They answer different questions. RabbitMQ carries *commands* (do this once, with retries, DLQ, per-message acks). Kafka carries *facts* (this happened; any number of consumers may read, replay and rebuild). Using one for both forces a bad trade.

**Row 17: durability vs latency.** `appendfsync always` would close the loss window at a large latency cost on the hot path, which defeats the purpose. A production design would use Redis replicas with `WAIT`, or accept the window and reconcile against Postgres. The simulation takes the fast option and says so.
