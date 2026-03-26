---
lang: fr
direction: ltr
source: docs/source/soracles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 22477c865a1623257683bbe05647f317e2dbd1433635d66f3a258c9cd992745d
source_last_modified: "2026-01-20T05:38:13.108570+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Soracles — Validator-Backed Oracle Layer

This document captures the canonical schemas and deterministic scheduling rules
for the validator-operated oracle layer. It completes roadmap items OR-1
through OR-3, OR-6, and OR-12 by pinning Norito/JSON layouts, committee/leader
derivation, connector hashing/cadence/PII redaction, and the replay surface for
gossip.

- **Code reference:** `crates/iroha_data_model/src/oracle/mod.rs`
- **Fixtures:** `fixtures/oracle/*.json`

## Data Model

### Feed configuration (`FeedConfig`)
- `feed_id: FeedId` — normalized name (UTS‑46/NFC enforced by `Name`).
- `feed_config_version: u32` — monotonic per feed.
- `providers: Vec<OracleId>` — validator-bound oracle keys eligible for draws.
- `connector_id/version: string/u32` — off-chain connector pinned to the feed.
- `cadence_slots: NonZeroU64` — feed active when `slot % cadence == 0`.
- `aggregation: AggregationRule` — `MedianMad(u16)` or `Percentile(u16)`; Norito
  JSON encodes these as `{ "aggregation_rule": "<Variant>", "value": <u16> }`.
- `outlier_policy: OutlierPolicy` — `Mad(u16)` or `Absolute { max_delta }`; JSON
  layout uses `{ "outlier_policy": "<Variant>", "value": ... }`.
- `min_signers/committee_size: u16` — safety bounds (must exceed byzantine `f`).
- `risk_class: RiskClass` — `Low | Medium | High` (drives governance/quorum).
  JSON uses `{ "risk_class": "<Variant>", "value": null }` for unit variants.
- Caps: `max_observers`, `max_value_len`, `max_error_rate_bps`,
  `dispute_window_slots`, `replay_window_slots`.

### Observations and reports
- `ObservationBody` — `{feed_id, feed_config_version, slot, provider_id,
  connector_id/version, request_hash: Hash, outcome, timestamp_ms?}`.
  - `ObservationOutcome` — `Value(ObservationValue)` or
    `Error(ObservationErrorCode)`.
  - `ObservationValue` — fixed-point `{mantissa, scale}`.
  - `ObservationErrorCode` — `ResourceUnavailable | AuthFailed | Timeout |
    Missing | Other(u16)`.
    - Fault classes: honest (`ResourceUnavailable | Timeout | Other`),
      misconfigured (`AuthFailed`), missing (`Missing`, no payload/parse error).
  - Hash and signature helpers: `ObservationBody::hash()` and `Observation::hash()`.
- `ReportBody` — `{feed_id, feed_config_version, slot, request_hash, entries[],
  submitter}` where `entries` are sorted by `oracle_id`.
  - `ReportEntry` — `{oracle_id, observation_hash, value, outlier}`.
  - Hash/signature helpers: `ReportBody::hash()` and `Report::hash()`.
- `FeedEventOutcome` — `Success { value, entries } | Error { code } | Missing`,
  emitted as `FeedEvent` `{feed_id, feed_config_version, slot, outcome}`.

### Connector requests (OR-6)
- Canonical schema: `{feed_id, feed_config_version, slot, connector_id/version,
  method, endpoint, query: BTreeMap, headers: BTreeMap<String,
  RedactedHeaderValue>, body_hash}`.
- Headers may be `Plain` or `Hashed`; prefer `Hashed` for API keys so secrets
  remain off-chain. `validate_redaction` rejects sensitive header names
  (`authorization`, `cookie`, `x-api-*`, etc.) unless hashed. `body_hash`
  hashes the request payload instead of storing it.
- `ConnectorRequest::hash()` yields the canonical `request_hash` advertised by
  observations/reports/events (XOR/USD sample:
  `hash:26A12D920ACC7312746C7534926D971D58FF443C1345B9C14DAF5C3C5E3E6A69#D88C`).
- `ConnectorResponse` mirrors the payload hash + optional error code when
  operators need to archive connector responses without leaking content.
- PII filtering: use `KeyedHash { pepper_id, digest }` for social identifiers or
  other PII, deriving `digest = Hash::new(pepper || payload)`; `KeyedHash::verify`
  checks keyed hashes for audit while keeping cleartext off-chain.

### Fixtures
Example fixtures live under `fixtures/oracle/`:
- `feed_config_price_xor_usd.json`
- `connector_request_price_xor_usd.json`
- `observation_price_xor_usd.json`
- `report_price_xor_usd.json`
- `feed_event_price_xor_usd.json`
- `feed_config_social_follow.json`
- `connector_request_social_follow.json`
- `observation_social_follow.json`
- `report_social_follow.json`
- `feed_event_social_follow.json`
- **Twitter binding registry:** The twitter follow feed now persists keyed-hash attestations via
  `RecordTwitterBinding` and supports manual cleanup with `RevokeTwitterBinding`. Attestations
  are stored under `HMAC(pepper, twitter_user_id||epoch)` → `{uaid, status, tweet_id,
  challenge_hash, expires_ms}` with no cleartext PII; lookups use the keyed hash through
  `FindTwitterBindingByHash`, and revocations emit typed events for subscribers.

#### Viral incentive flows (SOC-2)

Once Twitter follow attestations land in the registry, the viral incentive contract exposes a
small set of ISIs over the keyed-hash binding:

- `ClaimTwitterFollowReward { binding_hash }` pays a configured reward from the viral incentive
  pool to the UAID’s bound account (once per binding, capped per UAID/day and per-binding), updates
  `viral_daily_counters` / `viral_binding_claims`, and debits the daily budget guarded by
  `governance.viral_incentives`. If an escrow created earlier exists for the same binding, the
  reward path releases the escrow to the UAID account and records a one-time sender bonus.
- `SendToTwitter { binding_hash, amount }` either delivers the amount immediately to the bound UAID
  account (when a fresh `Following` attestation is present) or escrows the funds under
  `viral_escrows` until a matching binding appears, at which point `ClaimTwitterFollowReward`
  releases the escrow as part of the reward flow.
- `CancelTwitterEscrow { binding_hash }` lets the sender reclaim an outstanding escrow when no
  binding arrives, subject to the global `halt` switch and deny-lists in configuration.

Governance configures the incentive pool, escrow account, reward/bonus amounts, per-UAID/day caps,
per-binding caps, daily budget, and deny-lists via the `ViralIncentives` section of
`iroha_config::parameters::Governance`. Promotion controls now include `promo_starts_at_ms` /
`promo_ends_at_ms` (window gating enforced for both sends and claims) and a campaign-wide
`campaign_cap` that tracks every reward and sender bonus across the full promo. Attempts outside the
window or after exhausting the campaign cap are rejected deterministically before any state changes.
`ViralRewardApplied` events now carry the active promo flag, halt flag, campaign spend snapshot, and
configured cap, driving the new `iroha_social_campaign_*`/`iroha_social_promo_active`/`iroha_social_halted`
metrics and the accompanying dashboard (`dashboards/grafana/social_follow_campaign.json`).
A `halt` flag freezes all viral flows without touching the underlying Twitter binding registry.

For tooling:
- The CLI offers `iroha social claim-twitter-follow-reward|send-to-twitter|cancel-twitter-escrow`
  subcommands that accept a Norito JSON `KeyedHash` payload (binding hash) and, for sends, a
  `Numeric` amount, then build and submit the corresponding instructions.
- The JS SDK mirrors these helpers via
  `buildClaimTwitterFollowRewardInstruction`, `buildSendToTwitterInstruction`, and
  `buildCancelTwitterEscrowInstruction`, which accept the same keyed-hash shape and quantities and
  return Norito-ready instruction objects for inclusion in transactions.

These fixtures use the canonical hash literal form (`hash:...#...`), uppercase
signatures, and i105 provider IDs derived from deterministic ed25519 keys (e.g.,
`soraゴヂアヌメネヒョタルアキュカンコプヱガョラツゴヸナゥヘガヮザネチョヷニャヒュニョメヺェヅヤアキャヅアタタナイス`).

For social/PII-bearing feeds, `ObservationValue::from_hash`, `from_keyed_hash`,
and `from_uaid` derive deterministic fixed-point values from hashed identifiers
so the social follow kit can stay PII-safe while remaining deterministic across
validators. These helpers keep the scale at zero and clear the top bit of the
mantissa to avoid negative encodings when turning hashes into on-ledger values.

Reference kits for SDK/CLI examples live in `crates/iroha_data_model/src/oracle/mod.rs::kits`.
They load the same fixtures to provide ready-made `OracleKit` bundles for the
XOR/USD price feed and the twitter follow binding feed, keeping code samples and
tests aligned with the canonical JSON payloads.
- Additional reference kits:
  - `twitter_follow_binding` feed: `feed_config_twitter_follow.json`,
    `connector_request_twitter_follow.json`,
    `observation_twitter_follow.json`, `report_twitter_follow.json`,
    `feed_event_twitter_follow.json` capture the keyed-hash UAID mapping for the
    Twitter follow oracle (pepper `pepper-social-v1`, request hash
    `hash:25BD3C09F859A100396B5FD39066A3AEC5FB2EE6458D8FEDD0E403A35B5B3745#8EF1`).
  - The existing `price_xor_usd` fixtures were refreshed to align with the
    current connector hash (`hash:59CFEA4268FB255E2FDB550B37CB83DF836D62744F835F121E5731AB62679BDB#844C`)
    and observation/report/event digests to keep parity across SDKs.

### Aggregation helper

`aggregate_observations` constructs a `ReportBody` plus `FeedEventOutcome`
while enforcing caps and outlier policy:

```rust
let output = aggregate_observations(
    &feed_config,
    slot,
    request_hash,
    submitter_oracle_id,
    &observations,
)?;
assert!(matches!(output.outcome, FeedEventOutcome::Success(_)));
```

Validations include:
- Feed/config/connector pins and cadence (`validate_observation_meta`).
- Slot + request-hash consistency, provider membership, and duplicate detection.
- Caps: `max_observers`, `max_value_len`, duplicate oracle entries (via
  `validate_report_caps`).
- Outlier marking via `OutlierPolicy::Mad` or `OutlierPolicy::Absolute`; median
  or percentile aggregation via `AggregationRule`.

Errors surface as `OracleAggregationError` to wire into on-chain admission
paths. When all observations are errors (with the same code), the outcome is
`FeedEventOutcome::Error`; when no observations arrive, the outcome is
`FeedEventOutcome::Missing`.

Host consumers: `iroha_core::oracle::OracleAggregator` and
`ObservationAdmission` wrap the same helpers for on-node validation, replay
guards, and report/outcome generation.

## Committee and Leader Selection (OR-2)

Committee draws are deterministic and scoped to `(feed_id, feed_config_version,
epoch, validator_set_root, providers[])`. The helper
`derive_committee(feed_id, version, epoch, root, providers, committee_size)`
returns a `CommitteeDraw { seed, members[] }` where members are the
lowest-score providers after hashing `(seed || provider_id)` with Blake2b-256.
Duplicate providers are dropped before scoring.

Leaders are deterministic per slot:

```rust
let draw = derive_committee(...);
let leader = draw.leader_for_slot(slot);
```

The leader index is derived from `Hash(seed || slot)` so every validator can
compute the same submitter for a given slot without coordination.

## Gossip Keys and Replay Windows (OR-3)

Gossip keys scope replay protection to `(feed_id, feed_config_version, slot)`.
Replay keys extend this with the canonical `request_hash` so idempotent
replays (same upstream request) can be detected and cached cleanly.

The `ReplayProtection` helper applies a sliding window in slots:

```rust
let mut guard = ReplayProtection::new(replay_window_slots);
match guard.record(replay_key, current_slot) {
    ReplayStatus::Fresh => { /* process */ }
    ReplayStatus::Duplicate => { /* drop, already seen in window */ }
    ReplayStatus::Expired => { /* drop, outside replay window */ }
}
```

This enforces the `(feed_id, version, slot, request_hash)` idempotency window
outlined in the roadmap and keeps gossip buffers bounded.

Helpers on `Observation` and `Report` expose typed keys directly:

```rust
let observation: Observation = /* ... */;
let key: GossipKey = observation.gossip_key();
let replay: ReplayKey = observation.replay_key();
```

These keys are also used to build the ABI fingerprint.

## ABI Hash and Caps (OR-1)

The canonical oracle ABI manifest is pinned through
`OracleAbiManifest::v1()` with a stable hash exposed via `oracle_abi_hash()`
observations or reports. For the current schema the ABI hash is
`25d675ad1e61609f0b5951743ecac11ae06ba3df9e4aaecdf10db61c2dd9507b`.

`validate_report_caps(&FeedConfig, &ReportBody)` checks:
- Entry count ≤ `max_observers`
- No duplicate `oracle_id` entries

Additional caps (value length, error-rate thresholds) are carried in
`FeedConfig` for host-side enforcement.

## Connector Cadence and Backoff (OR-6)

- Cadence guard: `FeedConfig::ensure_active_slot(slot)` enforces
  `slot % cadence == 0`; `validate_observation_meta` additionally rejects feed
  id/version or connector pin mismatches.
- Committee-only fetching: `plan_committee_fetches` returns an empty plan for
  non-members and a deterministic schedule for committee members using
  `FetchDiscipline { max_attempts, base_backoff_slots, jitter_max_slots }`.
  Attempts are sorted/deduped and jitter derives from
  `(provider_id, request_hash, attempt_idx)`.
- Secrets stay hashed: connector requests/responses and the request hash only
  expose hashes, keeping API keys and payloads off-chain while still allowing
  replay detection and auditability.

## Evidence Bundles and SoraFS (OR-7/OR-13)

- On-chain events keep raw payloads off-ledger: `FeedEventRecord` stores only
  `evidence_hashes`, leaving connector responses/observations in SoraFS or
  operator storage.
- The CLI bundler `iroha soracles bundle --events <feed_events.json> --output <dir> [--observations <dir>] [--reports <dir>] [--responses <dir>] [--disputes <dir>] [--telemetry <path>]`
  copies provided evidence into `artifacts/<HASH>.*`, deduplicates by
  Blake2b-256 hash, and emits `bundle.json` listing feed events, bundled paths,
  and any missing evidence hashes per slot (aligned with
  `fixtures/oracle/feed_event_price_xor_usd.json` and other feed/event exports).
- Evidence retention + GC: `bundle.json` records `generated_at_unix` for each
  bundle, and the `iroha soracles evidence-gc` helper prunes bundles older than
  the configured window (default 180 days, or 365 days for bundles containing
  disputes via `--dispute-retention-days`), optionally removing unreferenced
  artefacts. See `docs/source/soracles_evidence_retention.md` for retention
  defaults and GC usage.
- Evidence hashes remain Iroha `Hash` literals (Blake2b-256 with LSB set). To
  keep SoraFS parity, upload the hashed artefacts alongside `bundle.json` so
  `evidence_hashes` can be verified against the pinned payloads.
- Telemetry exports coverage counters for dashboards:
  `iroha_oracle_feed_events_total`, `iroha_oracle_feed_events_with_evidence_total`,
  and `iroha_oracle_evidence_hashes_total` track how many feed slots carried
  evidence and how many hashes were attached.

## Change Governance (OR-8)

- Change classes (`Low|Medium|High`) drive quorum using `oracle.governance`
  thresholds (`cop_min_votes`, `policy_jury_min_votes`) with class-aware
  helpers baked into the data model.
- Pipeline is deterministic and stage-gated:
  `Intake → RulesCommittee → CopReview → TechnicalAudit → PolicyJury → Enactment`
  with SLAs `intake_sla_blocks`, `rules_sla_blocks`, `cop_sla_blocks`,
  `technical_sla_blocks`, `policy_jury_sla_blocks`, and `enact_sla_blocks`.
- Proposals land via `ProposeOracleChange` (carries `FeedConfig`, change class,
  and evidence hashes), progress with `VoteOracleChangeStage` (approve/reject +
  optional evidence), and can be explicitly reverted with `RollbackOracleChange`.
- Events: `ChangeProposed` advertises the new change + payload hash; every vote
  or auto-rollback emits `ChangeStageUpdated` with approvals/rejections and the
  current status so dashboards can track quorum and deadline failures.
- Enactment applies the proposed `FeedConfig` once the policy jury stage reaches
  quorum; missed SLAs or explicit rollbacks mark the change as `Failed` while
  preserving evidence hashes for audit.
