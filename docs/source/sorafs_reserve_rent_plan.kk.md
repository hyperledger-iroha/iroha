---
lang: kk
direction: ltr
source: docs/source/sorafs_reserve_rent_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04184b558958fdc24b754cd6b01706d54034cec84c19d023398f81db72a4d1ac
source_last_modified: "2026-06-25T18:08:03+00:00"
translation_last_reviewed: 2026-06-25
title: Reserve+Rent & Lifecycle Policy
summary: SFM-6 implementation status for reserve underwriting payloads, quote/ledger helpers, dashboard digest wiring, and remaining reserve-service rollout.
---

# Reserve+Rent & Lifecycle Policy

## Status
SFM-6 is partially implemented in this workspace. The shared data model ships
`ReservePolicyV1`, `ReserveQuote`, `ReserveLedgerProjection`,
`ReserveLifecycleStage`, and `ReserveLifecycleProjection` in
`crates/iroha_data_model/src/sorafs/reserve.rs`; the CLI exposes deterministic
`iroha app sorafs reserve quote`, `iroha app sorafs reserve ledger`, and
`iroha app sorafs reserve lifecycle` helpers; `cargo xtask
sorafs-reserve-matrix` emits quote matrices; and
`scripts/telemetry/reserve_ledger_digest.py` feeds the reserve economics
dashboards and Alertmanager rules. `scripts/check_sorafs_reserve_rent_rollout_evidence.py`
now provides the fail-closed rollout evidence gate for staged SFM-6 promotion
packets, and `scripts/run_sorafs_reserve_rent_rollout_evidence.py` provides the
matching reviewed evidence collection planner/runner.

The production reserve/rent control plane is still outstanding. There is no
reserve daemon, no Torii REST surface for reserve lifecycle management, no
authenticated runtime credit-line drawdown/accrual engine, and no shipped CLI
for provider status, top-ups, withdrawals, appeals, or policy updates.

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
- Manual appeals and policy-update payloads are target service work, not currently shipped data-model types.

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
- Target service/API work:
  - Add a reserve lifecycle service that persists provider summaries, manages authenticated reserve movements, applies the shared credit-line projection to live account state, and emits lifecycle events.
  - Add authenticated Torii endpoints for provider summary, top-up, withdraw, appeal, lifecycle policy, and event history.
  - Add operator CLI commands for status, top-up, withdraw, appeal, and policy/config inspection once those service routes exist.

## Integration Points
- **Billing**: implemented quote/ledger/lifecycle helpers produce deterministic rent, reserve transfer, and lifecycle/credit snapshots for offline settlement automation.
- **Telemetry**: ledger digest output feeds the reserve economics dashboard, capacity dashboard, and reserve Alertmanager rules.
- **Governance evidence**: quote, ledger, Markdown digest, Prometheus textfile, and matrix artifacts can be attached to economics reports.
- **Reputation, orderbook, compliance, and automatic lifecycle policy**: still target integrations because the runtime reserve lifecycle service is not shipped.

## Observability
- Implemented metrics come from the ledger digest textfile:
  - `sorafs_reserve_ledger_rent_due_xor`
  - `sorafs_reserve_ledger_reserve_shortfall_xor`
  - `sorafs_reserve_ledger_top_up_shortfall_xor`
  - `sorafs_reserve_ledger_requires_top_up`
  - `sorafs_reserve_ledger_meets_underwriting`
  - `sorafs_reserve_ledger_instruction_total`
  - `sorafs_reserve_ledger_transfer_xor`
- Implemented dashboards:
  - `dashboards/grafana/sorafs_reserve_economics.json`
  - reserve panels mirrored in `dashboards/grafana/sorafs_capacity_health.json`
- Implemented alerts in `dashboards/alerts/sorafs_capacity_rules.yml` cover ledger top-up requirements, underwriting breaches, missing transfer feeds, and rent/top-up transfer drift.
- Provider balance, live lifecycle-stage, runtime credit-usage, default, appeal-backlog, and service-rate-limit metrics are target work for the reserve lifecycle service.

## Security & Governance
- Current helpers are local/offline tooling. They render deterministic JSON/Norito-backed artifacts and transfer instructions, but they do not submit authenticated reserve movements on their own.
- Production reserve movements must be authenticated through Torii/client signing once the service API is implemented.
- Governance policy updates, manual stage overrides, and appeal decisions remain target payload/service work.

## Testing & Rollout
- Implemented test coverage:
  - `crates/iroha_data_model/src/sorafs/reserve.rs` covers deterministic rent/reserve calculation and ledger projection behavior.
  - `crates/iroha_data_model/src/sorafs/reserve.rs` covers lifecycle projection warnings, grace credit draws, post-grace interest accrual, uncovered-rent defaulting, and invalid lifecycle windows.
  - `crates/iroha_cli/tests/cli_smoke.rs` covers reserve quote JSON output, reserve ledger transfer instruction emission, and reserve lifecycle credit-draw projection output.
  - `xtask/src/sorafs.rs` covers the reserve matrix report, including ledger projection output.
  - Alert rule tests under `dashboards/alerts/tests/` cover the reserve ledger alert paths.
  - `scripts/tests/check_sorafs_reserve_rent_rollout_evidence_test.py` covers
    complete staged evidence, response-file arguments, missing signed routes,
    stale ledger digests, payload leakage, missing metrics, unsigned/wrong-account
    probes, missing policy/matrix ledger bindings, mismatched ledger tuples,
    ledger-bound subset gates without anchors, failed provider bakes, explicit
    unknown schemas, ignored unknown directory artifacts in subset mode, invalid
    recognized optional artifacts in subset mode, and unknown required evidence
    kinds.
  - `scripts/tests/run_sorafs_reserve_rent_rollout_evidence_test.py` covers the
    collection planner's complete dry-run command plan, response-file parsing,
    split-token response files, missing required evidence, missing file checks,
    subset gates, and unknown required evidence kinds.
- Remaining rollout work:
  1. Implement the reserve lifecycle service and signed Torii routes.
  2. Add provider status, top-up, withdrawal, appeal, and policy/config CLI commands.
  3. Wire lifecycle events into governance logs and downstream reputation/compliance/orderbook policy.
  4. Add integration tests for live reserve movement, runtime credit-line mutation/accrual, appeal decisions, and service telemetry.
  5. Run a staged provider bake before production rollout and attach payload-free
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
verifier command and summary path are reproducible:

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
the configured thresholds, the quote matrix binds to a valid policy
`policy_digest_hex`, the ledger binds to that policy/matrix tuple, and
lifecycle, route, movement, credit-line, appeal, metrics, provider-bake, and
governance artifacts all carry the same payload-free
`policy_digest_hex`/`matrix_digest_hex`/`ledger_digest_hex` tuple. The
governance packet must also be bound to `iroha_config`. Tuple binding failures
are recorded on the offending artifact before required-kind validity is
computed, so the JSON summary matches the fail-closed rollout decision.

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
- Done: deterministic policy formulas, JSON/Norito payloads, quote/ledger/lifecycle CLI helpers, local lifecycle/credit projection, matrix generation, ledger digest conversion, dashboards, alert rules, fail-closed rollout evidence gate, collection planner, operator argfile templates, and focused tests for those local paths.
- Remaining: reserve lifecycle service, signed Torii routes, runtime reserve movement/authentication, persisted lifecycle-stage automation, appeal/policy-update payloads, live credit-line mutation/accrual, and staged provider bake evidence that passes the rollout gate.
