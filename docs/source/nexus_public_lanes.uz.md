---
lang: uz
direction: ltr
source: docs/source/nexus_public_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 65540923620ca8a96dd0c0b07b3000b6c77e22deef4bd390725abbb3a6ed1193
source_last_modified: "2026-01-31T19:25:45.077354+00:00"
translation_last_reviewed: 2026-02-07
title: Nexus Public Lane Staking
description: NX-9 specification for permissionless validator admission, stake accounting, and reward records.
---

# Nexus Public Lane Staking (NX-9)

Status: 🈺 In Progress → **runtime + operator docs aligned** (Apr 2026)  
Owners: Economics WG / Governance WG / Core Runtime  
Roadmap ref: NX-9 – Public lane staking & reward module

This note captures the canonical data model, instruction surface, governance
controls, and operational hooks for the Nexus public-lane staking program. The
goal is to let permissionless validators join the public lanes, bond stake,
service blocks, and receive rewards while governance maintains deterministic
slashing/runbook levers.

The code scaffolding now lives in:

- Data model types: `crates/iroha_data_model/src/nexus/staking.rs`
- ISI definitions: `crates/iroha_data_model/src/isi/staking.rs`
- Core executor stub (returns a deterministic guard error until NX-9 logic lands):
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Torii/SDKs can begin plumbing the Norito payloads ahead of the full runtime
implementation; stake instructions now lock the configured staking asset by
withdrawing from the `stake_account`/`staker` into a bonded escrow account
(`nexus.staking.stake_escrow_account_id`). Slashes debit the escrow and credit
the configured sink (`nexus.staking.slash_sink_account_id`), and unbonds return
funds to the originating account once the timer expires.

## 1. Ledger State & Types

### 1.1 Validator Records

`PublicLaneValidatorRecord` tracks the canonical state for each validator:

| Field | Description |
|-------|-------------|
| `lane_id: LaneId` | Lane the validator services. |
| `validator: AccountId` | Account that signs consensus messages. |
| `stake_account: AccountId` | Account that supplies the self-bond (may differ from the validator identity). |
| `total_stake: Numeric` | Self stake + approved delegations. |
| `self_stake: Numeric` | Stake provided by the validator. |
| `metadata: Metadata` | Commission %, telemetry ids, jurisdiction flags, contact info. |
| `status: PublicLaneValidatorStatus` | Lifecycle (pending/active/jailed/exiting/etc.). The `PendingActivation` payload encodes the target epoch. |
| `activation_epoch: Option<u64>` | Epoch when the validator became active (set on activation). |
| `activation_height: Option<u64>` | Block height recorded at activation. |
| `last_reward_epoch: Option<u64>` | Epoch that last produced a payout. |

`PublicLaneValidatorStatus` enumerates lifecycle phases:

- `PendingActivation(epoch)` — waiting for the governance-specified activation epoch; the tuple payload stores the earliest activation epoch (usually `current_epoch + 1`, derived from `epoch_length_blocks`; genesis bootstrap registrations target `current_epoch` so validators can activate in the genesis block).
- `Active` — participates in consensus and can collect rewards.
- `Jailed { reason }` — temporarily suspended (downtime, telemetry breach, etc.).
- `Exiting { releases_at_ms }` — unbonding; rewards stop accruing.
- `Exited` — removed from the set.
- `Slashed { slash_id }` — governance slashing event recorded for audits.

Activation metadata is monotonic: `activation_epoch`/`activation_height` are set the first time a
pending validator becomes active and any attempt to reactivate at an earlier epoch/height is rejected.
Pending validators are promoted automatically at the start of the first block whose epoch meets the
scheduled boundary, and the activation metrics counter (`nexus_public_lane_validator_activation_total`)
records the promotion alongside the status change.

Permissioned deployments keep the genesis peer roster active even before any
public-lane validator stake exists: as long as the peers have live consensus
keys, the runtime falls back to the genesis peers for the validator set. This
avoids a bootstrap deadlock while staking admission is disabled or still being
rolled out.

### 1.2 Stake Shares & Unbonding

Delegators (and validators topping up their own bond) are modelled via
`PublicLaneStakeShare`:

- `bonded: Numeric` — live bonded amount.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — pending withdrawals keyed by a
  client-supplied `request_id`.
- `metadata` stores UX/back-office hints (e.g., custody desk reference numbers).

`PublicLaneUnbonding` holds the deterministic withdrawal schedule
(`amount`, `release_at_ms`). Torii now exposes the live shares and pending
withdrawals via `GET /v1/nexus/public_lanes/{lane}/stake` so wallets can show
timers without bespoke RPCs.

Lifecycle hooks (runtime enforced):

- `PendingActivation(epoch)` entries automatically flip to `Active` once the
  current epoch reaches `epoch`. Activation records `activation_epoch` and
  `activation_height`, and regressions are rejected both for auto-activation
  and explicit `ActivatePublicLaneValidator` calls.
- `Exiting(releases_at_ms)` entries transition to `Exited` when the block
  timestamp passes `releases_at_ms`, clearing stake-share rows so validator
  capacity can be reclaimed without manual cleanup.
- Reward recording rejects validator shares unless the validator is `Active`,
  keeping pending/exiting/jailed validators from accruing payouts.

### 1.3 Reward Records

Reward distributions use `PublicLaneRewardRecord` and `PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU",
  "total_reward": "250.0000",
  "shares": [
    { "account": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ", "role": "Validator", "amount": "150" },
    { "account": "34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r", "role": "Nominator", "amount": "100" }
  ],
  "metadata": {
    "telemetry_epoch_root": "0x4afe…",
    "distribution_tx": "0xaabbccdd"
  }
}
```

Records give auditors and dashboards deterministic evidence for each payout. The
reward struct flows into the `RecordPublicLaneRewards` ISI.

Runtime guards:

- Nexus builds must be enabled; offline/stub builds reject reward recording.
- Reward epochs advance monotonically per lane; stale or duplicate epochs are rejected.
- Reward assets must match the configured fee sink (`nexus.fees.fee_sink_account_id` /
  `nexus.fees.fee_asset_id`) and the sink balance must fully cover `total_reward`.
- Each share must be positive and respect the reward asset’s numeric spec; share totals must
  equal `total_reward`.

## 2. Instruction Catalog

All instructions live under `iroha_data_model::isi::staking`. They derive Norito
encoders/decoders so SDKs can submit the payloads without bespoke codecs.

### 2.1 `RegisterPublicLaneValidator`

Registers a validator and bonds an initial stake:

```norito
{
  "lane_id": 1,
  "validator": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
  "stake_account": "soraゴヂアニィルサフユイサヹピビレッデヹボテハキョメベチュヒャネィギチュヲベァヱェベモネェネツデトツオチハセ",
  "initial_stake": "150000",
  "metadata": {
    "commission_bps": 750,
    "jurisdiction": "JP",
    "telemetry_id": "val-01"
  }
}
```

Validation rules:

- `initial_stake` ≥ `min_self_stake` (governance parameter).
- Metadata MUST include contact/telemetry hooks before activation.
- Governance approves/denies the entry; until then the status is `PendingActivation` and the runtime promotes the validator to `Active` at the next epoch boundary once the target activation epoch (`current_epoch + 1` at registration, or `current_epoch` for genesis bootstrap) is reached.

### 2.2 `BondPublicLaneStake`

Bonds additional stake (validator self-bond or delegator contribution).

Key fields: `staker`, `amount`, optional metadata for statements. Runtime must
enforce lane-specific limits (`max_delegators`, `min_bond`, `commission caps`).

### 2.3 `SchedulePublicLaneUnbond`

Starts the unbonding timer. Submitters provide a deterministic `request_id`
(recommendation: `blake2b(invoice)`), `amount`, and `release_at_ms`. Runtime must
verify the amount ≤ bonded stake and clamp `release_at_ms` to the configured
unbonding period.

### 2.4 `FinalizePublicLaneUnbond`

After the timer expires, this ISI unlocks the pending stake and returns it to
`staker`. The executor validates the request id, ensures the unlock timestamp is
in the past, emits a `PublicLaneStakeShare` update, and records telemetry.

### 2.5 `SlashPublicLaneValidator`

Governance uses this instruction to debit stake and jail/eject validators.

- `slash_id` ties the event to telemetry + incident docs.
- `reason_code` is a stable enum string (e.g., `double_sign`, `downtime`,
  `safety_violation`).
- `metadata` stores hashes of evidence bundles, runbook pointers, or regulator IDs.

Slashes ripple to delegators based on governance policy (proportional or
validator-first loss). Runtime logic will emit `PublicLaneRewardRecord`
annotations once NX-9 lands.

### 2.6 `RecordPublicLaneRewards`

Records the payout for an epoch. Fields:

- `reward_asset`: asset distributed (default `xor#nexus`).
- `total_reward`: minted/transferred total.
- `shares`: vector of `PublicLaneRewardShare` entries.

### 2.7 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record is marked `penalty_cancelled` and `penalty_cancelled_at_height`, preventing slashing when `slashing_delay_blocks` elapses.
- `metadata`: references to payout transactions, root hashes, or dashboards.

This ISI is idempotent per `(lane_id, epoch)` and underpins nightly accounting.

## 3. Operations, lifecycle, and tooling

- **Lifecycle + modes:** stake-elected lanes are enabled via
  `nexus.staking.public_validator_mode = stake_elected` while restricted lanes
  stay admin-managed (`nexus.staking.restricted_validator_mode = admin_managed`).
  Permissioned deployments keep genesis peers active until stake exists; for
  stake-elected lanes we still require a registered peer with a live consensus
  key present in the commit topology before `RegisterPublicLaneValidator`
  succeeds. Genesis fingerprints and `use_stake_snapshot_roster` decide whether
  the runtime derives the roster from stake snapshots or falls back to genesis
  peers.
- **Activation/exit operations:** registrations land in `PendingActivation` for
  `current_epoch + 1` (genesis bootstrap registrations use `current_epoch`) and
  auto-promote at the first block whose epoch meets that boundary (epochs are
  derived from `epoch_length_blocks`). Operators can also call
  `ActivatePublicLaneValidator` after the boundary to force promotion. Exits
  move validators to `Exiting(release_at_ms)` and free capacity only once the
  block timestamp reaches `release_at_ms`; re-registration after a slash still
  requires exiting so the record is marked `Exited` and capacity is reclaimed.
  Capacity checks use `nexus.staking.max_validators` and run after the exit
  finalizer, so future-dated exits block new registrations until the timer
  elapses.
- **Config knobs:** `nexus.staking.min_validator_stake`,
  `nexus.staking.stake_asset_id`, `nexus.staking.stake_escrow_account_id`,
  `nexus.staking.slash_sink_account_id`, `nexus.staking.unbonding_delay`,
  `nexus.staking.withdraw_grace`, `nexus.staking.max_validators`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, and the
  validator-mode switches above.
  Thread them through
  `iroha_config::parameters::actual::Nexus` and surface them in `status.md`
  once GA values are ratified.
- **Torii/CLI quickstart:**
  - `iroha app nexus lane-report --summary` shows lane catalog entries, manifest
    readiness, and validator modes (stake-elected vs admin-managed) so operators
    can confirm whether staking admission is enabled for a lane.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary]`
    surfaces lifecycle/activation markers (pending target epoch, `activation_epoch` /
    `activation_height`, exit release, slash id) alongside bonded/self stake.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator soraカタカナ...] [--summary]`
    mirrors the `/stake` endpoint with pending-unbond hints per `(validator, staker)` pair.
  - Torii snapshots for dashboards and SDKs:
    - `GET /v1/nexus/public_lanes/{lane}/validators` – metadata, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation
      epoch/height, release timers, bonded stake, last reward epoch.
      Optional `canonical Katakana i105 literal rendering` controls the literal rendering
      (canonical Katakana i105 output only).
    - `GET /v1/nexus/public_lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) plus pending unbond timers. Optional
      `?validator=soraカタカナ...` filters the response for dashboards that focus
      on a single validator; `canonical Katakana i105 rendering` applies to all literals.
    - `GET /v1/nexus/public_lanes/{lane}/rewards/pending` – pending rewards per
      asset for the requested account. Requires `account=soraカタカナ...` and accepts
      optional `asset_id` and `upto_epoch` filters; `canonical Katakana i105 rendering` applies to
      the account literal in the response.
  - Lifecycle ISIs use the standard transaction path (Torii
    `/v1/transactions` or the CLI instruction pipeline). Example Norito JSON
    payloads:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "soraカタカナ..." } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "soraカタカナ...",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Telemetry + runbooks:** metrics expose validator counts, bonded and pending
  stake, reward totals, and slash counters under the
  `nexus_public_lane_*` family. Wire dashboards to the same data set used by
  NX-9 acceptance tests so validator deltas and reward/slash evidence remain
  auditable. Slashing instructions remain governance-only; reward recording must
  prove payout totals (hash of payout batch).

## 4. Roadmap alignment

- ✅ Runtime and WSV storages implement the NX-9 validator lifecycle; regressions
  cover activation timing, peer prerequisites, delayed exits, and
  re-registration after slashes.
- ✅ Torii exposes `/v1/nexus/public_lanes/{lane}/{validators,stake,rewards/pending}` with
  Norito JSON so SDKs and dashboards can monitor lane state without custom RPCs.
- ✅ Config and telemetry knobs are documented; mixed deployments keep
  stake-elected and admin-managed lanes isolated so validator rosters stay
  deterministic.
