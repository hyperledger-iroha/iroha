# SoraFS Deal Engine

The SF-8 roadmap track introduces the SoraFS deal engine, providing
deterministic accounting for storage and retrieval agreements between
clients and providers. Agreements are described with the Norito payloads
defined in `crates/sorafs_manifest/src/deal.rs`, covering deal terms, bond
locking, probabilistic micropayments, and settlement records.

The embedded SoraFS worker (`sorafs_node::NodeHandle`) instantiates a
`DealEngine` for every node process. The engine:

- validates every non-zero identifier, price, probability, epoch interval,
  settlement window, capacity, payout, metadata bound, and bond calculation
  before locking collateral; deal epoch bounds are inclusive, including a
  valid one-epoch deal when start and end are equal;
- accrues XOR-denominated charges when replication usage is reported;
- evaluates probabilistic micropayment windows using deterministic
  BLAKE3-based sampling;
- conserves provider deposits as available + locked + slashed collateral and
  client deposits as available + settled debits; and
- produces ledger snapshots and settlement payloads suitable for governance
  publishing.

All monetary state uses exact `u128` nano-XOR values. No ledger conversion
rounds nano-XOR to micro-XOR. Checked arithmetic rejects overflow before any
account, deal, ticket, or checkpoint state changes.

The shared data-model boundary enforces the same rule: `DealTerms` rejects zero
prices, settlement windows, payouts, and probabilities outside 1–10,000 bps;
charge and bond helpers return typed errors instead of saturating. Usage reports
are capped at 4,096 strictly ordered unique tickets, reject wrong-epoch or inert
tickets, and prevent ticket storage or egress coverage from exceeding the
signed report totals.
Deal and settlement ranges use inclusive endpoints, so `start_epoch == end_epoch`
is a valid one-epoch interval; epoch zero, inverted ranges, and settlement
windows longer than the inclusive deal duration are rejected.
Settlement records account for every expected nano-XOR as micropayment credit,
client debit, bond slash, or outstanding carry. Provider bond lock, slash, and
release operations reject backdating, overdraw, corrupt locked/bonded state,
and cumulative overflow without partially mutating the ledger.

Each `DealLedgerSnapshotV1` has a one-based sequence, the exact predecessor
snapshot ID, immutable deal/terms/provider/client bindings, deal and window
epochs, cumulative accounting, per-window deltas, and a domain-separated
canonical snapshot ID. Validation proves the following conservation equations:

- generated ticket credit = applied credit + carried credit;
- provider accrual = generated ticket credit + client debits;
- client liability = applied credit + client debits + bond slashes + outstanding liability;
- initial bond = locked bond + slashed bond + released bond.

Successor validation additionally requires sequence + 1, the exact predecessor
ID, contiguous windows, strictly increasing capture epochs, exact cumulative
deltas, and matching credit/liability/bond flows. `DealSettlementV1` has its own
canonical ID, requires `settled_at` to equal the ledger capture/window-end
epoch, and enforces irreversible finality for `Completed`, `Defaulted`, and
`Cancelled`. `Completed` requires the negotiated terminal epoch; collateral
exhaustion emits `Defaulted` immediately, even if its final slash exactly
satisfies the liability. A final settlement cannot have a successor. Audit notes are
bounded and canonical; they are mandatory for a default, cancellation, or a
window that slashes collateral, and forbidden otherwise.

First-release deal payloads use a single canonical representation. A deal ID is
the domain-separated BLAKE3 digest of the complete Norito-encoded terms (with
the ID field zeroed for derivation), so changing the provider, client, chunker
profile, price, duration, micropayment policy, validity interval, or metadata
invalidates the ID. Client/profile/metadata fields are bounded, metadata is
strictly sorted, and the validity interval must fit the declared duration.
Manifest micropayment receipts are checked against that bound deal ID, the exact
zero-based payment window, the per-window liability cap, their issue time, and
a deterministic receipt hash.

Runtime ticket IDs bind the deal, issue epoch, storage GiB-hours, and egress
bytes. Usage epochs must be strictly increasing inside the current exact
settlement window. Tickets must be non-empty, canonical, unique across restart,
issued at the report epoch, and cannot cover more storage or egress than the
signed report. Winning credit cannot exceed the deterministic charge in its
window. Duplicate, substituted, stale, future, oversized, or over-crediting
reports fail before mutation.

Settlements emit
`DealSettlementV1` governance payloads, wiring directly into the SF-12
publishing pipeline, and update the `sorafs.node.deal_*` OpenTelemetry series
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) for Torii dashboards and SLO
enforcement. The live engine retains only the canonical settlement head; the
governance DAG retains the immutable history linked by predecessor IDs. This
keeps runtime memory bounded without making long-lived deals un-settleable.

When storage is enabled, provider/client/deal accounting, the canonical
settlement head, usage high-water marks, and current-window replay-ticket records are
committed in the node's atomic auxiliary checkpoint. Restore builds and
validates replacement maps off to the side, including sorted/unique indexes,
term and ID derivation, all conservation equations, settlement-head finality,
ticket bindings and coverage, and provider/client aggregate balances. Invalid,
truncated, symlinked, or hard-linked checkpoint state fails closed without
replacing live state. A failed durable write rolls the in-memory mutation back
to the exact prior checkpoint.

Successful settlement retires the closed window's ticket records and releases
their bounded replay-set capacity. This does not weaken replay protection:
ticket IDs bind their issue epoch, reports require that exact epoch, and the
persisted settlement/usage high-water mark rejects every closed-window epoch.

External funding mutations use an exact, one-based `funding_sequence` per
provider or client account. The sequence is part of the signed request and the
durable checkpoint. Replays, gaps, and competing requests for the same sequence
return a conflict before balances change; the exact next sequence remains
enforced after restart. Unsequenced deposit helpers are test-only and are not
compiled into the production `NodeHandle` mutation surface.

An operator can cancel an idle active deal only at its exact next settlement
boundary and strictly before the negotiated terminal epoch. Cancellation is
refused while the current window contains usage, generated/applied credit,
metering totals, outstanding liability, or carried credit. A successful
cancellation releases the remaining collateral, increments the settlement
sequence, links the predecessor snapshot, emits a final canonical `Cancelled`
settlement with the bounded operator rationale, and becomes replay-proof across
restart. Deals at the terminal boundary must use normal settlement.

Usage telemetry now also feeds the `sorafs.node.micropayment_*` metrics set:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, and the ticket counters
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). These totals expose the probabilistic
lottery flow so operators can correlate micropayment wins and credit carry-over
with settlement outcomes.

## Torii Integration

Torii exposes dedicated endpoints for the complete authenticated deal lifecycle:

- `POST /v1/sorafs/deal/fund-provider` adds collateral using the exact next
  durable funding sequence. The complete request must be signed by the current
  Ed25519 key in the targeted provider's valid admitted advert.
- `POST /v1/sorafs/deal/fund-client` adds client credit using the exact next
  durable funding sequence and requires a configured operator signature.
- `POST /v1/sorafs/deal/open` validates and atomically activates the supplied
  canonical `DealProposal`. It requires a configured operator signature, a
  currently admitted provider advert, an existing funded client account, and
  enough provider collateral to lock the exact bond.
- `POST /v1/sorafs/deal/usage` accepts `DealUsageReport` telemetry and returns
  deterministic accounting outcomes (`UsageOutcome`). The exact HTTP method,
  path, query, body hash, timestamp, and nonce must be signed with the current
  Ed25519 key from the deal provider's admitted advert. Freshness and a bounded
  replay cache are checked before the handler binds that key to the provider.
- `POST /v1/sorafs/deal/cancel` performs the conservative boundary-only
  cancellation described above and returns the final base64-encoded
  `DealSettlementV1`. It requires a configured operator signature and publishes
  the cancellation through the same durable governance outbox and SSE event as
  normal settlement. Its trimmed, control-free rationale is limited to 1,024
  UTF-8 bytes and is committed by the settlement ID.
- `POST /v1/sorafs/deal/settle` finalises the current window, streaming the
  resulting `DealSettlementRecord` alongside a base64-encoded `DealSettlementV1`
  ready for governance DAG publication. It requires the same canonical HTTP
  signature envelope from a configured operator key. Settlement epochs must be
  exactly the previous settlement epoch plus the negotiated window; early,
  skipped, repeated, and post-finality transitions are rejected.
- Torii's `/v1/events/sse` feed now broadcasts `SorafsGatewayEvent::DealUsage`
  records summarising each usage submission (epoch, metered GiB-hours, ticket
  counters, deterministic charges), `SorafsGatewayEvent::DealSettlement`
  records that include the canonical settlement ledger snapshot plus the
  BLAKE3 digest/size/base64 of the on-disk governance artefact, and
  `SorafsGatewayEvent::ProofHealth` alerts whenever PDP/PoTR thresholds are
  exceeded (provider, window, strike/cooldown state, penalty amount). Consumers can
  filter by provider to react to new telemetry, settlements, or proof-health alerts without polling.

All deal endpoints participate in the SoraFS quota framework via the
`torii.sorafs.quota.deal_telemetry` window, allowing operators to tune the
allowed submission rate per deployment.

The bounded HTTP nonce cache protects a running Torii process. Durable replay
protection is enforced again at the mutation boundary: funding sequences survive
restart, an open replay collides with the canonical deal ID, usage replays hit
the epoch/ticket high-water marks, and settlement or cancellation replays hit
the persisted window head or final status. Restarting Torii therefore cannot
turn a previously signed lifecycle request into a second state transition.

Focused tests cover canonical identifiers, predecessor forks and sequence
gaps, party/term substitution, stale/future epochs, ticket forgery and replay,
duplicate windows, credit/liability/bond conservation, nano-XOR precision,
overflow, terminal default/completion, bounded long-lived settlement heads,
checkpoint tampering, atomic rollback, restart replay, request-body tampering,
signature freshness, nonce replay, provider-key/algorithm substitution,
funding-sequence replay/fork/gap attacks, unsafe cancellation with unsettled
usage, terminal cancellation, canonical cancellation chaining, and
settled-window ticket-capacity reclamation, closed-window ticket checkpoint
forgery, and cancellation/funding recovery after restart.
