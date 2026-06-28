---
title: Reserve+Rent & Lifecycle Policy
summary: SFM-6 implementation status for reserve underwriting payloads, quote/ledger helpers, dashboard digest wiring, and remaining reserve-service rollout.
---

# Reserve+Rent & Lifecycle Policy

## Status
SFM-6 is partially implemented in this workspace. The shared data model ships
`ReservePolicyV1`, `ReserveQuote`, `ReserveLedgerProjection`,
`ReserveLifecycleStage`, and `ReserveLifecycleProjection` in
`crates/iroha_data_model/src/sorafs/reserve.rs`; the CLI exposes deterministic
`iroha app sorafs reserve quote`, `iroha app sorafs reserve ledger`,
`iroha app sorafs reserve lifecycle`, and signed local movement/readback
commands; `cargo xtask sorafs-reserve-matrix` emits quote matrices; and
`scripts/telemetry/reserve_ledger_digest.py` feeds the reserve economics
dashboards and Alertmanager rules. `scripts/check_sorafs_reserve_rent_rollout_evidence.py`
now provides the fail-closed rollout evidence gate for staged SFM-6 promotion
packets, and `scripts/run_sorafs_reserve_rent_rollout_evidence.py` provides the
matching reviewed evidence collection planner/runner. `sorafs_node` now also
ships a local reserve lifecycle and movement runtime that stores provider
summaries, derives ledger and lifecycle projections from the shared quote math,
keeps sequenced lifecycle events for replay, and records idempotent local
top-up/withdrawal movement intents with provider balances and custody status
evidence. Accepted lifecycle updates now also record local credit-line state
for the provider, including automatic draw, remaining capacity, shortfall,
accrued interest, and governance/manual-approval flags. Torii exposes that
runtime through canonical-request-authenticated reserve lifecycle routes for
signed provider updates, provider/snapshot/event readback, live lifecycle event
SSE/WebSocket streams, and signed reserve top-up, withdrawal, movement history,
provider-balance, movement custody-status, and private credit-line readback
routes. Rejected custody evidence now recomputes the local movement ledger so
rejected top-ups are no longer spendable, rejected withdrawals restore the
previous balance, and retroactive rejections fail closed when they would make
later withdrawals underflow. The movement ledger now also keeps a separate
chain-confirmed reserve balance projection: intent-recorded and submitted
movements affect the local balance, confirmed custody movements affect the
confirmed balance, and confirmed withdrawals fail closed unless prior
confirmed reserve funds cover them. It also exposes signed local reserve appeal
submission/decision routes and signed lifecycle-policy window update/readback
routes, with matching operator CLI commands for appeal and policy handoff.
Accepted effective lifecycle-policy windows now drive later local lifecycle
projections, and provider/event readback exposes the grace/default windows plus
the applied policy id that produced the projection. Already-effective policy
updates now also reproject retained provider summaries whose lifecycle
observation falls under the new policy, append policy-driven lifecycle replay
events, refresh local credit-line state, and re-run reserve gateway compliance
sync for affected providers. Accepted appeal decisions with a requested stage
now emit a local lifecycle override event, update the provider summary and
credit-line state, and expose the applied appeal id in readback and
transparency entries.
The local runtime can now also deterministically advance retained provider
summaries to an operator-supplied observation timestamp by whole elapsed days,
append time-advance lifecycle replay events, refresh local credit-line state,
and re-run reserve gateway compliance sync through the signed
`/v1/sorafs/reserve/lifecycle/advance` route. Torii can now run the same
advancement path from a shutdown-aware config-backed scheduler under
`torii.sorafs.storage.reserve_lifecycle`, with explicit cadence and initial
delay settings.
Accepted reserve lifecycle, movement, appeal, and lifecycle-policy records now also derive
privacy-safe local transparency/governance source entries for publication
cycles without exposing provider accounts, appeal reasons, or policy authority
accounts in clear text. Local reserve lifecycle stages can now publish
reserve-adjusted reputation snapshots through the existing reputation pipeline,
including accepted-appeal stage overrides and prior reputation score smoothing.
The local orderbook admission path now also consumes reserve lifecycle
provider summaries for ask orders and rejects defaulted providers whose
projection disables adverts, while accepted appeal overrides that move the
provider out of default restore ask admission.
Signed Torii reserve lifecycle and appeal-decision routes now also sync the
current reserve lifecycle summary into the gateway compliance denylist when a
provider enters or leaves the advert-disabled default state, using
reserve-scoped source metadata so static/operator denylist entries are not
overwritten or removed. Signed movement custody updates now also project
rejected reserve custody evidence into the same provider denylist under a
separate reserve-scoped source, and later accepted reserve appeals clear that
reserve-owned movement entry when no higher-priority lifecycle default remains.
The local runtime now also exports Torii reserve
runtime metrics for lifecycle-stage counts, credit-line usage, defaults,
appeal backlog, movement custody state, chain-reconciled movement state, and
reserve service request/rate-limit counters.

The production reserve/rent control plane is still incomplete. Signed local
movement routes now authenticate and record transfer intents, but they do not
submit reserve transfers or verify chain finality. Operators can now attach
submitted, confirmed, or rejected transaction evidence to the local movement
ledger, rejected custody evidence now reconciles the local balance view, and
confirmed custody evidence now updates a separate chain-confirmed balance view.
Lifecycle updates now materialize local credit-line draw/accrual state, but
live chain submission, automatic finality polling, and live account mutation
for credit lines are still target service work. There is
now a local authenticated appeal/policy handoff surface, local accepted-appeal
lifecycle overrides, local governance source-entry handoff, and reserve
lifecycle default-state plus rejected movement-custody application to the
gateway compliance denylist, plus local already-effective lifecycle-policy
reprojection into provider denylist state. Local signed lifecycle advancement
is available for operator-driven time progression, and the opt-in
config-backed scheduler can drive the same advancement path on a production
cadence. Broader governance source-entry effects beyond current provider
denylist projection and live scheduler canary evidence remain target downstream
work.

## Goals & Scope
- Track the implemented financial policy for provider reserves and recurring rent.
- Keep quote/ledger/lifecycle automation, dashboard inputs, and governance evidence aligned with the shared `ReservePolicyV1` schema.
- Identify the remaining service work required for lifecycle stages, reserve movements, appeals, credit lines, and governance policy changes.

## Policy Model
- Key variables:
  - `monthly_rent = base_rate_tier * capacity_gib * duration_factor`
  - `reserve_requirement = underwriting_ratio * monthly_rent`
  - `effective_rent = monthly_rent - min(reserve_balance / underwriting_ratio, monthly_rent)`
  - Reserve top-up threshold = 0.8 × reserve requirement.
- Tier base rates (governance adjustable): hot 12 XOR/GiB-month, warm 6, archive 2.
- Duration factors: monthly 1.0, quarterly 0.9, annual 0.75.
- Underwriting ratios default: Tier A 2.0, Tier B 3.0, Tier C 4.5.
- Credit line caps are encoded in the tier policy: Tier A 2x monthly rent, Tier B 1x, Tier C manual approval.
- APR parameters are encoded per tier (3%, 6%, none). The local lifecycle projection applies capped automatic credit draws for eligible tiers, prorates APR after the configured grace window, and marks manual-approval tiers as requiring operator action instead of inventing an automatic cap.
- Implemented local lifecycle stages:
  - Stage `Active`: provider reserve is current.
  - Stage `Warning`: restrict new manifests.
  - `Grace`: auto-draw credit line.
  - `Delinquent`: penalty APR plus governance notification.
  - `Default`: disable adverts and flag the account for target runtime slashing from reserve and then credit line.
- `ReserveQuote::lifecycle_projection(days_past_due, grace_period_days, default_after_days)` rejects invalid grace/default windows, computes credit draw and remaining credit availability, reports credit shortfall and accrued interest, and enters `Default` when rent cannot be covered or the default threshold is exceeded.
- Manual appeals and lifecycle-policy updates are implemented as local
  authenticated service records for provider/operator handoff. Applying those
  records to live governance execution and downstream policy remains target
  integration work.

## APIs & Services
- Implemented payloads:
  - `ReservePolicyV1` stores storage-class rates, duration factors, tier underwriting ratios, credit caps, APR values, and the top-up threshold.
  - `ReserveQuote` stores the deterministic quote result for a storage class, tier, duration, capacity, and reserve balance.
  - `ReserveLedgerProjection` derives `rent_due`, reserve shortfall, top-up shortfall, and underwriting/top-up booleans from a quote.
  - `ReserveLifecycleProjection` derives lifecycle stage, credit draw, credit shortfall, accrued interest, total remaining due, and service restriction flags from a quote and explicit lifecycle windows.
- Implemented CLI commands:
  - `iroha app sorafs reserve quote --storage-class <hot|warm|cold> --tier <tier-a|tier-b|tier-c> --duration <monthly|quarterly|annual> --gib <capacity>` computes deterministic rent/reserve breakdowns (monthly rent, reserve requirement, top-up threshold, credit line cap) using the embedded policy or JSON/Norito overrides. Quotes are emitted as JSON and can be persisted via `--quote-out`. The CLI reuses the shared `ReservePolicyV1` schema so economics dashboards and SDKs can reference the same Norito payloads without reimplementing the formulas. The JSON payload includes a `ledger_projection` object with:
    - `rent_due` — XOR due for the billing period after applying reserve offsets.
    - `reserve_shortfall` — reserve delta required to satisfy underwriting.
    - `top_up_shortfall` — amount needed to clear the top-up alert threshold.
    - `meets_underwriting` / `needs_top_up_alert` — booleans used by dashboards and admission ISIs to trigger policy transitions.
  - `iroha app sorafs reserve ledger --quote <path> --provider-account <id> --treasury-account <id> --reserve-account <id> --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc` converts a saved quote into the concrete XOR transfers required for rent settlement and reserve top-ups. The helper reads the `ledger_projection` block, echoes the micro-XOR totals, and emits an `instructions` array containing Norito-encoded `Transfer` ISIs that can be piped into automation or stored alongside governance evidence.
  - `iroha app sorafs reserve lifecycle --quote <path> --days-past-due <days> --grace-days <days> --default-after-days <days>` converts a saved quote into a deterministic lifecycle snapshot. The JSON output includes the stage label, rent/reserve/top-up amounts, automatic credit draw, remaining credit availability, credit shortfall, accrued interest, remaining due after credit, and booleans for manifest restriction, advert disablement, governance notification, and manual credit approval.
  - `iroha app sorafs reserve top-up --provider-id-hex <hex> --provider-account <id> --reserve-account <id> --asset-definition <aid> --amount <xor> --idempotency-key <key>` builds the signed reserve movement JSON, converts decimal XOR to micro-XOR, canonicalizes the provider/reserve account ids, and submits the top-up intent to the Torii route above.
  - `iroha app sorafs reserve withdraw --provider-id-hex <hex> --provider-account <id> --reserve-account <id> --asset-definition <aid> --amount <xor> --idempotency-key <key>` submits the matching signed local withdrawal intent.
  - `iroha app sorafs reserve movements --since <seq> --limit <n>` lists the signed caller's visible local movement history.
  - `iroha app sorafs reserve status --provider-id-hex <hex>` fetches the signed caller's visible local provider balance status.
  - `iroha app sorafs reserve custody --movement-id-hex <hex> --status <submitted|confirmed|rejected> --tx-hash-hex <hex>` signs a local custody-status update for a recorded movement and binds the movement to chain transaction evidence.
  - `iroha app sorafs reserve credit-lines --limit <n>` lists the signed caller's visible local credit-line states derived from accepted lifecycle updates.
  - `iroha app sorafs reserve credit-status --provider-id-hex <hex>` fetches one signed visible provider credit-line state.
  - `iroha app sorafs reserve appeal-submit --provider-id-hex <hex> --provider-account <id> --reason <text> --idempotency-key <key> [--requested-stage <stage>] [--evidence-digest-hex <hex>]` submits a signed local provider appeal record.
  - `iroha app sorafs reserve appeals --limit <n>` lists the signed caller's visible local appeal records.
  - `iroha app sorafs reserve appeal-decide --appeal-id-hex <hex> --status <accepted|rejected> --decision-account <id> --rationale <text>` attaches a signed terminal decision to a local reserve appeal.
  - `iroha app sorafs reserve policy-update --authority-account <id> --grace-days <days> --default-after-days <days> --effective-at-unix <seconds> --reason <text> --idempotency-key <key>` submits a signed local lifecycle-policy window update and rejects invalid grace/default windows.
  - `iroha app sorafs reserve policy --limit <n>` fetches signed local lifecycle-policy readback, including the latest accepted policy record.
- Implemented Torii routes:
  - `POST /v1/sorafs/reserve/lifecycle` accepts a Norito JSON `ReserveQuote`
    lifecycle update, verifies the canonical request signer against the
    canonical `provider_account`, stores the provider summary locally, and
    returns the accepted sequenced event plus the derived provider projection.
  - `GET /v1/sorafs/reserve/lifecycle` returns the bounded local provider and
    event snapshot.
  - `GET /v1/sorafs/reserve/lifecycle/providers/{provider_id_hex}` returns one
    provider's latest local summary.
  - `GET /v1/sorafs/reserve/credit-lines` returns private, authenticated local
    credit-line states visible only to the signed provider account.
  - `GET /v1/sorafs/reserve/credit-lines/providers/{provider_id_hex}` returns
    one private, authenticated provider credit-line state with rent due,
    automatic draw, remaining capacity, shortfall, interest, total due, and
    governance/manual approval flags.
  - `GET /v1/sorafs/reserve/lifecycle/events` returns bounded replay events with
    ETag support, while `/events/stream` and `/events/ws` stream the replay
    backlog plus live reserve lifecycle updates.
  - `POST /v1/sorafs/reserve/top-up` and
    `POST /v1/sorafs/reserve/withdraw` accept signed provider movement
    requests, verify the canonical request signer against the canonical
    `provider_account`, record an idempotent local movement, update the
    provider's local reserve balance, and return the recorded movement plus a
    transfer intent that clients must still submit through the production
    transaction path.
  - `GET /v1/sorafs/reserve/movements` returns private, authenticated,
    ETagged movement history visible only to the signed provider or reserve
    account in each record.
  - `POST /v1/sorafs/reserve/movements/{movement_id_hex}/custody` lets the
    signed provider or reserve account attach submitted, confirmed, or rejected
    transaction evidence to a recorded local movement. The route preserves
    terminal custody states and rejects attempts to replace an existing
    transaction hash.
  - `GET /v1/sorafs/reserve/balances/{provider_id_hex}` returns the private
    local balance for a provider when the canonical request signer is that
    provider account or the reserve account.
  - `POST /v1/sorafs/reserve/appeals` accepts signed provider appeal records,
    verifies the signer against `provider_account`, derives a deterministic
    appeal id from the idempotency material, and returns the local appeal
    record. Replays of the same signed material are idempotent.
  - `GET /v1/sorafs/reserve/appeals` returns private, authenticated appeal
    records visible to the signed provider account and, after a decision, the
    signed decision account.
  - `POST /v1/sorafs/reserve/appeals/{appeal_id_hex}/decision` lets the signed
    decision account attach an accepted or rejected terminal decision to an
    open local appeal.
  - `POST /v1/sorafs/reserve/lifecycle/policy` accepts signed lifecycle-policy
    window updates, verifies the signer against `authority_account`, rejects
    invalid grace/default windows, and stores the sequenced local policy record.
    Later lifecycle updates whose observation time is at or after a stored
    policy's effective time use the latest effective local policy windows for
    projection and readback.
  - `GET /v1/sorafs/reserve/lifecycle/policy` returns signed local lifecycle
    policy readback with the latest accepted policy record.
  - `POST /v1/sorafs/reserve/lifecycle/advance` accepts a signed authority
    request, advances retained provider summaries to `observed_at_unix` by
    whole elapsed days, applies effective lifecycle policy and accepted appeal
    overrides at that timestamp, refreshes local credit-line state, emits
    sequenced lifecycle events, and re-runs reserve gateway compliance sync for
    affected providers.
  - `torii.sorafs.storage.reserve_lifecycle.enabled` starts a local
    shutdown-aware scheduler that invokes the same lifecycle advancement path
    without a self-HTTP call. `interval_seconds` controls the cadence, and
    `initial_delay_seconds` delays the first tick.
- Target service/API work:
  - Extend signed top-up/withdrawal intents plus local custody-status handoff
    into live chain custody submission, automatic finality polling, and local
    credit-line state applied to live account state. The local runtime already
    reconciles provider balances when a movement is explicitly marked rejected.
  - Record live canary evidence proving the configured lifecycle scheduler
    advances defaulting providers and preserves provider-bake gate freshness.
  - Apply broader governance source entries to downstream compliance policy
    consumers beyond the current provider denylist projection. Local reserve
    lifecycle stages already feed reserve-adjusted reputation snapshots,
    defaulted provider advert disablement is enforced for local orderbook ask
    admission, and Torii reserve routes sync default/appeal lifecycle,
    signed lifecycle advancement, already-effective lifecycle-policy
    reprojection, and rejected custody movement changes into the gateway
    compliance denylist.

## Integration Points
- **Billing**: implemented quote/ledger/lifecycle helpers and signed local movement routes produce deterministic rent, reserve transfer, movement, local and chain-confirmed balance, lifecycle/credit snapshots, and local credit-line state for settlement automation.
- **Telemetry**: ledger digest output feeds the reserve economics dashboard,
  capacity dashboard, and reserve Alertmanager rules. The Torii metrics
  registry also exports local reserve lifecycle-stage, credit-line, default,
  appeal-backlog, custody, chain-reconciliation, service-request, and
  service-rate-limit metrics for the signed reserve runtime.
- **Governance evidence**: quote, ledger, Markdown digest, Prometheus textfile, and matrix artifacts can be attached to economics reports.
- **Appeals and lifecycle policy**: signed local appeal and policy records now
  support provider/operator handoff, effective local lifecycle-policy
  projection, already-effective policy reprojection for retained provider
  summaries, signed whole-day lifecycle advancement, accepted local
  appeal-decision lifecycle overrides, and privacy-safe local governance source
  entries. Accepted lifecycle, appeal, policy, lifecycle-advance routes, and
  scheduled lifecycle ticks now also update reserve-derived gateway compliance
  provider denylist entries, and accepted appeal decisions can clear
  reserve-owned movement custody entries; broader downstream compliance effects
  for governance source entries remain target integrations.
- **Reputation, orderbook, compliance, and automatic lifecycle policy**:
  reputation snapshots can now consume local reserve lifecycle stages and
  accepted appeal overrides, and local orderbook ask admission now enforces
  reserve lifecycle advert disablement for defaulted providers. Torii also
  syncs advert-disabled reserve lifecycle state and rejected custody movements
  into the gateway compliance provider denylist while preserving non-reserve
  denylist entries, and already-effective lifecycle-policy updates reproject
  retained provider summaries into the same provider denylist projection.
  Signed lifecycle advancement feeds the same provider summaries, replay
  events, credit-line refresh, and compliance/orderbook behavior, and the
  config-backed scheduler invokes that same local path; live custody/credit
  mutation, live scheduler canary evidence, and broader downstream compliance
  governance source-entry effects remain target integrations.

## Observability
- Implemented ledger-digest metrics come from the textfile:
  - `sorafs_reserve_ledger_rent_due_xor`
  - `sorafs_reserve_ledger_reserve_shortfall_xor`
  - `sorafs_reserve_ledger_top_up_shortfall_xor`
  - `sorafs_reserve_ledger_requires_top_up`
  - `sorafs_reserve_ledger_meets_underwriting`
  - `sorafs_reserve_ledger_instruction_total`
  - `sorafs_reserve_ledger_transfer_xor`
- Implemented runtime metrics come from `iroha_telemetry` and are refreshed by
  the local signed reserve runtime after accepted lifecycle, movement,
  custody, appeal, decision, lifecycle-policy, lifecycle-advance records, and
  scheduled lifecycle ticks:
  - `torii_sorafs_reserve_lifecycle_stage_providers`
  - `torii_sorafs_reserve_credit_draw_micro_xor`
  - `torii_sorafs_reserve_credit_shortfall_micro_xor`
  - `torii_sorafs_reserve_accrued_interest_micro_xor`
  - `torii_sorafs_reserve_defaulted_providers`
  - `torii_sorafs_reserve_appeal_backlog`
  - `torii_sorafs_reserve_custody_movements`
  - `torii_sorafs_reserve_chain_reconciled_movements`
  - `torii_sorafs_reserve_service_requests_total`
  - `torii_sorafs_reserve_service_rate_limit_total`
- Implemented dashboards:
  - `dashboards/grafana/sorafs_reserve_economics.json` includes ledger
    economics plus runtime lifecycle-stage, credit exposure, default/appeal,
    custody/reconciliation, service request, and rate-limit panels.
  - runtime reserve defaults, appeal backlog, reconciliation, and rate-limit
    panels are mirrored in `dashboards/grafana/sorafs_capacity_health.json`.
- Implemented alerts in `dashboards/alerts/sorafs_capacity_rules.yml` cover
  ledger top-up requirements, underwriting breaches, missing transfer feeds,
  rent/top-up transfer drift, runtime defaults, uncovered credit shortfall,
  appeal backlog, rejected chain reconciliation, and reserve service
  rate-limit events.
- Remaining observability work is integration coverage that proves
  chain-finality reconciliation and live credit-line mutation update those
  metrics from production services, plus staged provider-bake evidence that the
  dashboard and alert canaries pass against live runtime scrapes.

## Security & Governance
- Current quote/ledger/lifecycle helpers are local/offline tooling. They render deterministic JSON/Norito-backed artifacts and transfer instructions, but they do not submit reserve custody transfers on their own.
- Signed top-up/withdrawal routes authenticate the provider request and record
  the local movement ledger plus transfer intent. Signed custody-status routes
  let the provider or reserve account attach local submitted/confirmed/rejected
  transaction evidence without allowing terminal status reversal or transaction
  hash replacement. Rejected custody evidence rewinds the local balance ledger
  before it is published through readback, and confirmed custody evidence
  advances a separate chain-confirmed balance projection that rejects confirmed
  withdrawals without confirmed reserve funds. The matching CLI commands call
  those signed routes, while production chain custody submission and automatic
  finality polling remain target work.
- Signed credit-line readback routes expose local credit draw/accrual state
  derived from accepted lifecycle updates to the provider account only. They do
  not yet mutate live account balances or credit-line assets.
- Signed appeal routes record provider appeals and terminal accepted/rejected
  decisions locally. Signed lifecycle-policy routes record grace/default window
  updates locally, and effective policies are applied to later local lifecycle
  projections. Accepted appeal decisions with a requested stage now apply a
  local lifecycle override to provider summaries, credit-line state, and
  lifecycle event replay without changing the deterministic financial amounts.
  The signed lifecycle-advance route applies whole-day elapsed time from the
  retained provider observation to an operator-supplied timestamp, uses the
  effective local policy and accepted appeal override at that timestamp, and
  emits deterministic replay events without relying on wall-clock drift inside
  the calculation. The config-backed scheduler invokes the same local
  advancement method at the configured cadence; zero or delayed cadence values
  are normalized through configuration parsing rather than environment toggles.
  Local reserve lifecycle stages can also be published into reserve-adjusted
  reputation snapshots, using prior reputation metrics and score when a matching
  provider already exists and neutral proof metrics for reserve-only providers.
  These records are authenticated and replay-safe; defaulted providers whose
  current lifecycle projection disables adverts are rejected from local
  orderbook ask admission and are synced into the gateway compliance denylist.
  Rejected reserve custody movements are also synced into a reserve-scoped
  provider denylist entry, and later accepted appeal decisions clear that
  movement-derived entry when no lifecycle default still applies. Static and
  operator denylist entries are never overwritten or removed by these reserve
  projections. Already-effective lifecycle-policy updates reproject retained
  provider summaries and refresh the same provider denylist projection. Broader
  governance compliance policy effects remain target work.
  The local node now converts lifecycle,
  movement, appeal, and lifecycle-policy records into transparency source entries with
  deterministic payload digests and private-field digests for governance
  publication cycles; lifecycle entries include the effective grace/default
  windows, applied policy id, and applied appeal id used for the projection.

## Testing & Rollout
- Implemented test coverage:
  - `crates/iroha_data_model/src/sorafs/reserve.rs` covers deterministic rent/reserve calculation and ledger projection behavior.
  - `crates/iroha_data_model/src/sorafs/reserve.rs` covers lifecycle projection warnings, grace credit draws, post-grace interest accrual, uncovered-rent defaulting, and invalid lifecycle windows.
  - `crates/iroha_cli/src/commands/sorafs.rs` and
    `crates/iroha_cli/tests/cli_smoke.rs` cover reserve quote JSON output,
    reserve ledger transfer instruction emission, reserve lifecycle credit-draw
    projection output, signed movement request JSON construction, zero-amount
    rejection, custody-status request JSON construction, invalid custody hash
    rejection, appeal submission/decision request JSON construction,
    lifecycle-policy request JSON construction and invalid-window rejection,
    and compiled command-tree help for the movement/status/custody/credit-line/
    appeal/policy commands.
  - `crates/iroha/src/client.rs` covers signed reserve top-up/withdrawal client
    requests, signed custody-status client requests, private signed
    movement/status/credit-line/appeal/policy readback, appeal-decision client
    requests, provider-id, movement-id, and appeal-id normalization, and
    empty-payload/bad-id rejection.
  - `crates/sorafs_node/src/reserve.rs` and `crates/sorafs_node/src/lib.rs`
    cover local reserve lifecycle summaries/events plus idempotent
    top-up/withdrawal movement recording, duplicate replay, withdrawal
    underflow rejection, custody-status transition/transaction-hash guards,
    rejected-movement balance reconciliation, confirmed-balance projection and
    confirmed-withdrawal underflow rejection, lifecycle-derived credit-line
    state, effective lifecycle-policy window application, local appeals,
    terminal appeal decisions, accepted appeal-decision lifecycle overrides,
    reserve-adjusted reputation snapshot publication, lifecycle-policy updates,
    whole-day lifecycle time advancement, provider balances, snapshots, and
    subscriptions. `crates/sorafs_node/src/transparency.rs` covers local
    reserve lifecycle, movement, appeal, and lifecycle-policy transparency
    source-entry adapters, including private-field digesting and invalid source
    rejection.
  - `crates/iroha_telemetry/src/metrics.rs` covers reserve runtime metric
    export for lifecycle-stage counts, credit-line usage, defaulted providers,
    appeal backlog, custody and chain-reconciled movement counts, service
    requests, and service rate-limit counters.
  - `crates/iroha_torii/src/sorafs/api.rs` covers signed reserve lifecycle
    route authentication, signer/account mismatch rejection, invalid lifecycle
    window mapping, provider readback, bounded snapshots, event replay, and
    ETag emission, plus signed reserve top-up/withdrawal authentication,
    signer/account mismatch rejection, underflow mapping, private movement
    history, custody-status submission/readback, confirmed balance readback,
    unknown custody movement rejection, private credit-line readback,
    private balance readback,
    appeal submission/private readback/decision/missing-appeal handling,
    accepted appeal-decision lifecycle override readback, reserve lifecycle and
    rejected movement-custody gateway compliance denylist sync,
    already-effective lifecycle-policy provider reprojection and compliance
    sync, signed lifecycle advancement and compliance sync, scheduler tick
    advancement and compliance sync,
    lifecycle-policy submission/readback/invalid-window handling, and
    transfer-intent JSON.
  - `crates/iroha_config/src/parameters/user.rs` and
    `crates/sorafs_node/src/config.rs` cover parsed/clamped reserve lifecycle
    scheduler configuration and disabled-scheduler omission from the runtime
    wrapper.
  - `xtask/src/sorafs.rs` covers the reserve matrix report, including ledger projection output.
  - Alert rule tests under `dashboards/alerts/tests/` cover the reserve ledger
    alert paths and runtime reserve default, credit shortfall, appeal backlog,
    chain-rejection, and service-rate-limit alert paths.
  - `scripts/tests/check_sorafs_reserve_rent_rollout_evidence_test.py` covers
    complete staged evidence, response-file arguments, missing signed routes,
    stale ledger digests, payload leakage, missing ledger/runtime metrics,
    unsigned/wrong-account probes, missing policy/matrix ledger bindings,
    mismatched ledger tuples,
    ledger-bound subset gates without anchors, failed provider bakes, explicit
    unknown schemas, ignored unknown directory artifacts in subset mode, invalid
    recognized optional artifacts in subset mode, and unknown required evidence
    kinds.
  - `scripts/tests/run_sorafs_reserve_rent_rollout_evidence_test.py` covers the
    collection planner's complete dry-run command plan, response-file parsing,
    split-token response files, missing required evidence, missing file checks,
    subset gates, and unknown required evidence kinds.
- Remaining rollout work:
  1. Wire signed top-up/withdrawal intents and local custody-status evidence
     into live chain custody submission, automatic finality polling, and live
     account mutation for local credit-line state.
  2. Apply broader governance source entries to downstream compliance policy
     beyond the current lifecycle/policy/rejected-custody provider denylist
     projection.
  3. Add integration tests for chain-reconciled reserve movement, runtime
     credit-line mutation/accrual, downstream governance source-entry
     application to broader compliance policy, lifecycle-policy enforcement,
     accepted appeal-decision lifecycle overrides in service flows,
     reserve-adjusted reputation publication, local orderbook reserve-lifecycle
     ask admission, and runtime metrics fed by those live service paths.
  4. Run a staged provider bake before production rollout and attach payload-free
     signed-route, ledger digest, movement, credit-line, appeal, metrics, provider
     bake, and governance evidence bound to the same policy/matrix/ledger tuple
     and passing the SFM-6 rollout gate.

## Automation & Dashboards

### Rollout Evidence Gate

Use the rollout gate after the reserve lifecycle service, signed route canaries,
reserve movement probes, credit-line accrual checks, appeal policy probes,
metrics, provider bake, and governance packet have produced reviewed,
payload-free JSON evidence:

```bash
python3 scripts/check_sorafs_reserve_rent_rollout_evidence.py \
  @scripts/examples/sorafs_reserve_rent_rollout_evidence.args.example
```

For staged collections with reviewed evidence paths, prefer the planner so the
verifier command, summary path, thresholds, and current required payload-free
field contract are reproducible:

```bash
python3 scripts/run_sorafs_reserve_rent_rollout_evidence.py \
  @scripts/examples/sorafs_reserve_rent_rollout_collection.args.example \
  --dry-run
```

The checker recognizes `sorafs.reserve.*` SFM-6 rollout schemas for policy
configuration, quote matrix, ledger digest, lifecycle service, signed routes,
reserve movements, credit-line accrual, appeal policy, metrics/alerts, provider
bake, and governance approval. It reports `ready` only when every required kind
is present, every recognized artifact is valid, raw ledgers/quotes/transfers,
signed transactions, response bodies, and secrets are absent, ledger/provider
bake timestamps are fresh, lifecycle lag and signed-route latency remain under
the configured thresholds, provider-bake artifacts prove the config-backed
reserve lifecycle scheduler canary ran recently enough before bake completion,
advanced defaulting providers, synced gateway compliance, and preserved
orderbook rejection, reserve-movement artifacts prove live chain submission
coverage, submitted transaction-hash readback, automatic finality polling,
confirmed-status polling, timeout rejection, submitted, confirmed, and
rejected custody evidence plus confirmed-balance readback and
confirmed-withdrawal underflow rejection, credit-line artifacts prove live
account-state mutation/readback, accrual posting, manual-approval tier
non-mutation, and account-state reconciliation, governance approval artifacts
prove source-entry publication, downstream compliance application, consumer
coverage, handoff verification, and non-reserve compliance-entry preservation,
the quote matrix binds to a valid policy
`policy_digest_hex`, the ledger binds to that policy/matrix tuple, and
lifecycle, route, movement, credit-line, appeal, metrics, provider-bake, and
governance artifacts all carry the same payload-free
`policy_digest_hex`/`matrix_digest_hex`/`ledger_digest_hex` tuple. The
governance packet must also be bound to `iroha_config`. Tuple binding failures
are recorded on the offending artifact before required-kind validity is
computed, so the JSON summary matches the fail-closed rollout decision. The
collection planner's dry-run JSON also includes an `evidence_contract` map so
operators can inspect the exact required fields for each requested evidence
kind before collecting or submitting live artifacts.

### Quote Matrix Generator

Run `cargo xtask sorafs-reserve-matrix` to emit a deterministic JSON matrix of
rent/reserve quotes covering the requested storage classes, tiers, durations,
and capacity bands. The task loads `ReservePolicyV1` (either from the embedded
defaults or the supplied `--policy-json`/`--policy-norito` override), applies
the underwriting ratios documented above, and records both the raw micro-XOR
amounts and the policy metadata so dashboards can assert provenance.

```bash
cargo xtask sorafs-reserve-matrix \
  --capacity 10 --capacity 100 --capacity 1000 \
  --storage-class hot --storage-class warm \
  --tier tier-a --tier tier-b \
  --duration monthly --duration annual \
  --reserve-balance 250.5 \
  --out artifacts/sorafs_reserve/matrix.json
```

Use `--label <text>` to tag the generated artefact (useful when comparing
dashboards or governance submissions) and `--reserve-balance <XOR>` to model
effective rent when an operator already maintains a reserve. The JSON output
includes `policy_sha256`, `policy_version`, and `reserve_balance_micro_xor`
fields alongside per-combination quotes so automation and analytics tooling can
trace every data point back to the exact policy used. Each quote entry also
contains a `ledger_projection` block (matching the CLI output) so dashboards,
reserve auditors, and ledger ISIs can render rent/reserve deltas without
recomputing underwriting math.

### Reserve Ledger Digest & Dashboard Wiring

Field teams asked for a deterministic way to embed `iroha app sorafs reserve ledger`
output inside dashboards and governance packets. The workflow below turns the
CLI JSON into a reusable digest and keeps the telemetry panels in sync with the
ledger projection that triggered the payment.

1. **Generate the ledger projection JSON.**
   ```bash
   iroha app sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account <i105-account-id> \
     --treasury-account <i105-account-id> \
     --reserve-account <i105-account-id> \
     --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
     > artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
2. **Normalise the values with the new helper.**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   `scripts/telemetry/reserve_ledger_digest.py` converts the micro‑XOR values
   into XOR, records whether underwriting thresholds were satisfied, and hashes
   the execution timestamp. The helper now also captures the **transfer feed**
   (`transfers` block) so rent and reserve top-ups appear alongside the projected
   ledger deltas, and `instruction_count` proves the CLI emitted both transfers.
   The script accepts multiple `--ledger` paths (plus per-ledger `--label`
   overrides) and can emit NDJSON batches via `--ndjson-out`, letting economics
   automation ingest an entire rent cycle without bespoke glue. The Markdown
   and JSON digests slot directly into governance packets while the JSON
   artefact can be ingested by automation or replayed in dashboards. The
   `--out-prom` flag writes a Prometheus textfile snapshot (`sorafs_reserve_ledger_*`
   metrics, including `sorafs_reserve_ledger_transfer_xor` +
   `sorafs_reserve_ledger_instruction_total`) so any node exporter with the
   textfile collector enabled can surface the latest ledger requirements to
   Grafana and Alertmanager without bespoke exporters; batched runs append every
   ledger to the same textfile so Alertmanager rewires as soon as treasury
   stages a new reserve transfer.
3. **Attach the digest to telemetry.** Publish the `--out-prom` textfile through
   the node exporter textfile collector and keep the JSON digest under
   `artifacts/sorafs_reserve/ledger/<provider>/` so the observability jobs that
   refresh `dashboards/grafana/sorafs_capacity_health.json` and the
   reserve-focused board in `dashboards/grafana/sorafs_reserve_economics.json`
   can locate the latest projection before each rent cycle.
4. **Update the runbook evidence block.** Drop the Markdown digest next to the
   weekly economics report (`docs/source/sorafs/reports/`) and link it from the
   rent burn-down so reviewers see the exact ledger inputs that produced the
   transfers.

### Metrics, Dashboards, and Alerts

Reserve telemetry now hinges on the DA counters emitted by Torii
(`crates/iroha_telemetry/src/metrics.rs`). The table below calls out the panels
and alert packs that consume those metrics so operators know which evidence to
collect after running the ledger helper.

| Metric | Grafana panel / dashboard | Alert / Runbook hook | Notes |
|--------|--------------------------|----------------------|-------|
| `torii_da_rent_base_micro_total` | “DA Rent Distribution (XOR/hour)” in `dashboards/grafana/sorafs_capacity_health.json` | Include in the weekly rent digest; panel traces how much rent was invoiced as XOR. |
| `torii_da_protocol_reserve_micro_total` | Same dashboard/panel (`refId=B`) | Feed into `dashboards/alerts/sorafs_capacity_rules.yml` via the `SoraFSCapacityPressure` context; rising reserve flows drive early warnings when underwriting falls behind. |
| `torii_da_provider_reward_micro_total` | “DA Rent Distribution” (`refId=C`) | Record spurts inside the economics status note so treasury can correlate payouts with ledger digests. |
| `torii_da_pdp_bonus_micro_total` / `torii_da_potr_bonus_micro_total` | “DA Bonus Accrual (XOR/hour)” panel in `dashboards/grafana/sorafs_capacity_health.json` | Reference in the PDP/PoTR compliance runbook; attach Alertmanager output when bonuses exceed policy. |
| `torii_da_rent_gib_months_total` | Capacity Usage widgets (same dashboard) | Pair with the ledger digest to show how many GiB·months were invoiced alongside the XOR amounts. |
| `sorafs_reserve_ledger_*` (rent/top-up/underwriting gauges) | “Reserve Snapshot (XOR)” + “Top-up Required” in `dashboards/grafana/sorafs_reserve_economics.json` (mirrored cards remain on the capacity board for historical context) | `SoraFSReserveLedgerTopUpRequired` and `SoraFSReserveLedgerUnderwritingBreach` inside `dashboards/alerts/sorafs_capacity_rules.yml` fire when the CLI projects a top-up or an underwriting failure. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown”, the coverage cards in `dashboards/grafana/sorafs_reserve_economics.json`, and the mirrored transfer coverage stats on the capacity board (`dashboards/grafana/sorafs_capacity_health.json`) | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing`, `SoraFSReserveLedgerTopUpTransferMissing`, `SoraFSReserveTransferRentMismatch`, and `SoraFSReserveTransferTopUpMismatch` in `dashboards/alerts/sorafs_capacity_rules.yml` cover missing/zeroed or mismatched transfer feeds whenever rent/top-up is required. |

Whenever the counters or dashboards change, re-run
`python3 scripts/telemetry/reserve_ledger_digest.py --ledger <...> --print` (or
point `--ndjson-out` / `--out-prom` at the automation directories) and attach
the refreshed digest to the rent burn-in evidence bundle. This keeps the
dashboards, alert packs, and governance packets aligned with the latest ledger
projection without re-deriving the math by hand. The transfer feed plus coverage
cards make it obvious when rent/reserve instructions drift from the ledger
projection, and the new alerts fire as soon as a digest omits the required rent
or reserve top-up transfers.

## Rollout Status
- Done: deterministic policy formulas, JSON/Norito payloads, quote/ledger/lifecycle CLI helpers, signed reserve top-up/withdrawal/status/movement/custody/credit-line/appeal/policy CLI commands, local lifecycle/credit projection, local accepted-appeal lifecycle overrides, already-effective lifecycle-policy provider reprojection, signed local lifecycle advancement, config-backed reserve lifecycle scheduler automation, reserve-adjusted reputation snapshot publication, local orderbook reserve-lifecycle ask admission, reserve lifecycle, lifecycle-policy, and rejected movement-custody gateway compliance denylist sync, local `sorafs_node` provider-summary, lifecycle-event, credit-line, appeal, lifecycle-policy, and movement-ledger runtime with separate local and chain-confirmed balance projections, signed Torii lifecycle, local movement/readback, custody-status, credit-line, appeal, lifecycle-policy, and lifecycle-advance routes, matrix generation, ledger digest conversion, dashboards, alert rules, fail-closed rollout evidence gate with scheduled lifecycle canary provider-bake checks, live chain submission/finality-poll reserve-movement evidence checks, custody/finality reserve-movement checks, live credit-line account-state evidence checks, downstream governance compliance evidence checks, and dry-run evidence-contract export, collection planner, operator argfile templates, and focused tests for those local paths.
- Remaining: live chain custody submission and automatic finality polling for signed movement intents, live account mutation for local credit-line state, broader downstream compliance application evidence for governance source entries, and staged provider bake evidence, including live scheduled lifecycle canaries, that passes the rollout gate.
