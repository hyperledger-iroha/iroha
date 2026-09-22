---
title: Nexus Public Lane Staking
description: NX-9 specification for permissionless validator admission, stake accounting, and reward records.
---

# Nexus Public Lane Staking (NX-9)

Status: In progress; candidate admission and reward custody are implemented, production committee rotation remains unqualified (September 2026).
Owners: Economics WG / Governance WG / Core Runtime
Roadmap ref: NX-9 – Public lane staking & reward module

This note captures the canonical data model, instruction surface, governance
controls, and operational hooks for the Nexus public-lane staking program. The
goal is to let permissionless validators join the public lanes, bond stake,
service blocks, and receive rewards while governance maintains deterministic
slashing/runbook levers.

The implementation lives in:

- Data model types: `crates/iroha_data_model/src/nexus/staking.rs`
- ISI definitions: `crates/iroha_data_model/src/isi/staking.rs`
- Core instruction execution and custody:
  `crates/iroha_core/src/smartcontracts/isi/staking.rs`

Stake instructions lock the configured staking asset by
withdrawing from the `stake_account`/`staker` into a bonded escrow account
(`nexus.staking.stake_escrow_account_id`). Slashes debit the escrow and credit
the configured sink (`nexus.staking.slash_sink_account_id`), and unbonds return
funds to the originating account once both their timestamp and evidence-liability
height have elapsed. A matured pending unbond remains claimable until finalised.

## 1. Ledger State & Types

### 1.1 Validator Records

`PublicLaneValidatorRecord` tracks the canonical state for each validator:

| Field | Description |
|-------|-------------|
| `lane_id: LaneId` | Lane the validator services. |
| `validator: AccountId` | Authority account used for staking, governance, and reward accounting. |
| `peer_id: PeerId` | Consensus and transport peer identity bound to the validator record. |
| `stake_account: AccountId` | Canonical self-bond account; it must equal `validator`. |
| `total_stake: Quantity` | Self stake + approved delegations. |
| `self_stake: Quantity` | Stake provided by the validator. |
| `metadata: Metadata` | Commission %, telemetry ids, jurisdiction flags, contact info. |
| `status: PublicLaneValidatorStatus` | Lifecycle (pending/active/exiting/exited/slashed). The `PendingActivation` payload encodes the exact activation height. |
| `activation_height: u64` | Inclusive first height at which the validator may be elected; it is fixed when activation is scheduled. |
| `deactivation_height: Option<u64>` | Exclusive first height no longer covered by the validator binding; `None` denotes an open tenure. |
| `last_reward_epoch: Option<u64>` | Epoch that last produced a payout. |

All stake, bond, unbond, slash, and reward amounts use `Quantity`, the canonical nominal non-negative decimal type. Signed `Numeric` values are reserved for genuine rates, ratios, and deltas and enter staking calculations only through explicit arithmetic boundaries.

`PublicLaneValidatorStatus` enumerates lifecycle phases:

- `PendingActivation(height)` — scheduled for election eligibility at the exact
  inclusive `activation_height` carried by the tuple payload.
- `Active` — participates in consensus during its exact-height tenure and can
  collect rewards.
- `Exiting { releases_at_ms }` — unbonding; rewards stop accruing.
- `Exited` — the release timestamp has passed; the retained tenure and custody
  gates still control consensus removal and pruning.
- `Slashed { slash_id }` — governance slashing event recorded for audits.

Consensus eligibility is the half-open exact-height tenure
`[activation_height, deactivation_height)`. A missing `deactivation_height`
leaves the upper bound open. These boundaries, rather than a lifecycle label,
are authoritative for election and historical evidence checks, so a status
transition cannot rewrite an already frozen roster. Pending validators are
promoted automatically when the current block reaches `activation_height`, and
the activation metrics counter
(`nexus_public_lane_validator_activation_total`) records the promotion alongside
the status change.

For stake-elected public lanes the validator authority account and live peer
identity are intentionally decoupled. `validator` remains the staking/governance
identity, while `peer_id` is the authoritative routing and consensus identity.
Torii and stake-derived roster selection read the stored `peer_id` directly and
must not infer a peer from `validator` account signatories.

### 1.2 Stake Shares & Unbonding

Delegators (and validators topping up their own bond) are modelled via
`PublicLaneStakeShare`:

- `bonded: Quantity` — live bonded amount.
- `pending_unbonds: BTreeMap<Hash, PublicLaneUnbonding>` — pending withdrawals keyed by a
  client-supplied `request_id`.
- `metadata` stores UX/back-office hints (e.g., custody desk reference numbers).

Consensus work is explicitly bounded: one validator may retain at most
`nexus.staking.max_stake_shares_per_validator` stake-share rows, and one share
may retain at most `nexus.staking.max_pending_unbonds_per_share` pending
requests. Bond and unbond scheduling reject an operation before mutation when
it would exceed the corresponding bound. The defaults are 256 shares and 8
pending requests per share, bounding one validator to 2,048 pending requests.

`PublicLaneUnbonding` holds the deterministic withdrawal schedule (`amount`,
`release_at_ms`, `slashable_through_height`, and `liability_release_height`). Torii
now exposes the live shares and pending withdrawals via
`GET /v1/nexus/public-lanes/{lane}/stake` so wallets can show timers without
bespoke RPCs. There is no withdrawal-expiry window: after the timestamp and
liability-height gates pass, the request remains claimable until it is
finalised.

`slashable_through_height` is inclusive. At schedule time the canonical
liability high-water is
`slashable_through_height + evidence_horizon_blocks + slashing_delay_blocks`.
Consensus effects run before ordinary transactions at the equality height, so
finalization may release custody there only after every pending evidence lien
for the exact validator tenure has become terminal. Snapshot restore rejects a
stored liability height below this signed formula.

Lifecycle hooks (runtime enforced):

- `PendingActivation(height)` entries automatically flip to `Active` once the
  current block reaches the exact `activation_height`. Explicit
  `ActivatePublicLaneValidator` calls before that height are rejected.
- `Exiting(releases_at_ms)` entries transition to `Exited` when the block
  timestamp passes `releases_at_ms`. Exit or slash schedules an exclusive
  `deactivation_height` at the next unfrozen election height; the timestamp
  transition does not shorten the exact-height consensus tenure. The `Exited`
  record continues to reserve validator capacity and its peer until that height
  is reached and while bonded or pending-unbond custody remains, or while
  pending evidence retains a slashing lien. Only canonical pruning after all
  gates clear frees those reservations.
- Reward recording rejects validator shares unless the validator is `Active`,
  keeping pending, exiting, exited, and slashed validators from accruing payouts.

### 1.3 Reward Records

Reward distributions use `PublicLaneRewardRecord` and `PublicLaneRewardShare`:

```norito
{
  "lane_id": 1,
  "epoch": 4242,
  "asset": "4cuvDVPuLBKJyN6dPbRQhmLh68sU",
  "total_reward": "250.0000",
  "shares": [
    { "account": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE", "role": "Validator", "amount": "150" },
    { "account": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE", "role": "Nominator", "amount": "100" }
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

- The distribution is signed by the configured fee-sink account and targets a stake-elected lane.
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
  "validator": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
  "peer_id": "ed0120F4s1C9m2m4G8Dqv4HY2Q8g7iATgJx6Y5wM1U3Q9H3bQJ7Lh",
  "stake_account": "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
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
- `peer_id` MUST resolve to a registered world-state peer. Lane `0` requires
  an unbounded `Validator` consensus key and current global topology membership
  until the prepared epoch-key transition is implemented. Participant-lane
  admission does not confer global committee membership. A non-zero participant lane prefers an unbounded
  `Committee` key and accepts an unbounded `Validator` key for transparent-path
  compatibility; either key must be live at the scheduled `activation_height`,
  and any peer with a live `Validator` key at that height must already belong to
  the nonempty global topology. This closes admission through another lane while
  global epoch-key transitions remain unavailable.
- A public lane cannot bind the same `peer_id` to multiple retained validator
  records. Exited records continue to reserve both their validator-capacity
  slot and peer identity while any slashable custody remains.
- The stake-owning validator authorizes registration. Binding a distinct peer
  through this instruction additionally requires peer-management authority;
  ordinary operators use signed `RegisterPublicLaneCandidate` consent.
- Until activation the status is
  `PendingActivation(height)`. Genesis bootstrap registration targets the
  genesis block height. Every non-genesis registration targets the next
  unfrozen election height. A registration executed in an epoch-ending block
  targets the following election, not the immediate successor height, because
  that successor roster was frozen from the block's pre-state.

`RegisterPublicLaneCandidate` is the consented atomic admission form. Its exact
fields are `registration: RegisterPublicLaneValidator`, `activation_height: u64`,
`proof_of_possession: Vec<u8>`, and
`peer_signature: SignatureOf<PublicLaneCandidateAuthorization>`. The peer signs
the protocol domain `iroha.staking.public_lane_candidate.v1`, the genesis-derived
network identity, the complete registration, and the planned activation height.
Core recomputes that boundary from the committed NPoS schedule and the ordinary
consensus-key activation lead; a stale or different boundary fails. Consent
therefore cannot be reused for a later validator tenure.

Fresh identities use BLS-normal PoP and the lane's required consensus-key role.
Admission prepares that key without mutation, checks registration and transfers
initial stake into escrow, then publishes the peer/key and candidate together.
An instruction rejected for insufficient funds cannot leave an unbonded peer,
even before its transaction overlay is discarded. Pre-registered peers retain
their existing lifecycle and must provide the exact PoP of a live, unbounded key.

A fresh lane-0 peer outside the current global committee is rejected **before
peer, key, or custody writes**, with `prepared epoch key transition` in the
error. Global committee expansion still requires the exact mint-key and beacon
transition protocol. Independent nonzero stake-elected lanes can admit fresh
committee peers through this instruction; this grants no global voting rights.

With a nonempty global topology, registration, rebind, exit, bond, and unbond
scheduling must also preserve the global election candidate pool. Core shares
the election's candidate extractor and conservatively preserves the future
eligibility unions for affected global peers and topology lanes. Existing-peer
consent therefore permits only a redundant global binding whose election
contribution is already covered. A global exit or self-stake minimum crossing
requires a prepared epoch-key transition, including when its record belongs to
another lane. Above-minimum self-withdrawals, delegator withdrawals, and custody
withdrawals after an ended tenure remain available. Authenticated evidence and
slashing remain unconditional; automatic roster recovery after a penalty still
requires the outstanding epoch-transition protocol.

### 2.2 `RebindPublicLaneValidatorPeer`

Repairs the authoritative `validator -> peer_id` binding before a stake-elected
validator first becomes eligible to vote.

Validation rules:

- Authority MUST be the `validator` account itself.
- An identity-changing rebind at non-genesis execution height `h` is allowed
  only while the validator is `PendingActivation` and before the pre-state
  freeze: `h + 1 < activation_height`. `Active`, `Exiting`, `Exited`, and
  `Slashed` records reject it so evidence always resolves against one immutable
  voting tenure.
- The replacement `peer_id` MUST satisfy the same runtime checks as
  `RegisterPublicLaneValidator` (registered peer, an unbounded lane-appropriate
  consensus key live at the scheduled activation height and no duplicate retained
  lane binding). A distinct replacement peer supplies a signature bound to the
  network, validator, lane, activation height, previous peer and replacement peer;
  existing peer administrators may authorize that binding explicitly.
- `peer_signature` is an explicit optional field in the canonical instruction
  layout. When present it signs `PublicLanePeerBindingAuthorization`: protocol
  domain `iroha.staking.public_lane_peer_binding.v1`, genesis-derived network,
  lane, validator, replacement peer, stored activation height, and stored
  previous peer. This prevents consent replay across pending tenures or bindings.
  An absent signature is permitted only for the validator account's own
  signatory, an exact peer-management authority, or genesis bootstrap.
- Lane-0 replacements, and replacements in any lane with a live `Validator` key
  at activation, also require current membership when the global topology is
  nonempty until prepared epoch-key transitions are available.
- Rebinding to the already-bound `peer_id` succeeds idempotently.

### 2.3 `BondPublicLaneStake`

Bonds additional stake (validator self-bond or delegator contribution).

Key fields: `staker`, `amount`, optional metadata for statements. Runtime must
enforce lane-specific limits (`max_delegators`, `min_bond`, `commission caps`).
New stake is accepted only for `PendingActivation` and `Active` validators;
terminal or exiting records cannot reacquire custody.

### 2.4 `SchedulePublicLaneUnbond`

Starts the unbonding timer. Submitters provide a deterministic `request_id`
(recommendation: `blake2b(invoice)`), `amount`, and `release_at_ms`. Runtime verifies the amount ≤ bonded stake and enforces the configured
unbonding period.

### 2.5 `FinalizePublicLaneUnbond`

After the timer and evidence-liability height expire, this ISI unlocks the
pending stake and returns it to `staker`. The executor validates the request id,
ensures both release gates are in the past, emits a `PublicLaneStakeShare`
update, and records telemetry. A valid matured request has no claim deadline.

### 2.6 `SlashPublicLaneValidator`

Governance uses this instruction to debit stake and eject validators.

- `offence_height` is the exact non-zero consensus height whose custody remains
  liable; it cannot be in the future or outside the validator's retained tenure.
  Pending unbonds whose `slashable_through_height` is earlier are excluded.
- `slash_id` ties the event to telemetry + incident docs.
- `reason_code` is a stable enum string (e.g., `double_sign`, `downtime`,
  `safety_violation`).
- `metadata` stores hashes of evidence bundles, runbook pointers, or regulator IDs.

Slashes apply through the retained stake-share and pending-unbond custody
paths; reward records do not authorize slashing.

### 2.7 `RecordPublicLaneRewards`

Records the payout for an epoch. Fields:

- `reward_asset`: exact configured fee asset held by the configured fee sink.
- `total_reward`: fee-funded amount reserved for the supplied distribution.
- `shares`: vector of `PublicLaneRewardShare` entries.

### 2.8 `CancelConsensusEvidencePenalty`

Cancels consensus slashing before the delayed penalty applies.

- `evidence`: the Norito-encoded `Evidence` payload that was recorded in `consensus_evidence`.
- The record transitions to `penalty_status = cancelled` with the canonical cancellation height, preventing slashing when `slashing_delay_blocks` elapses.
- `metadata`: references to payout transactions, root hashes, or dashboards.

The instruction is idempotent for the exact evidence key: replaying a
cancellation after that record is already cancelled succeeds without changing
its terminal height. For evidence admitted at height `A` with delay `D`, an
ordinary transaction can cancel it only in committed blocks `A + 1` through
`A + D - 1`, exactly `D - 1` opportunities. At `A + D`, due consensus effects
run before ordinary transactions, so cancellation in that block is too late.

Pinned custody records retain each validator's exact scoped escrow asset and
held quantity (bonded plus pending unbond). The aggregate stake reserve and unpaid
reward reserve are additive balance floors for ordinary and native debits.
Deposits reserve real available funds; an escrow account cannot repeatedly bond
its own already-reserved balance. Matured unbond and verified slash owners release
only their authenticated quantity, restoring reserve preimages if transfer fails.
Withdrawals retain their source across configuration and alias changes, while
new deposits must match existing custody. Snapshot current/predecessor cuts,
account migration and entity deletion preserve these obligations.

## 3. Operations, lifecycle, and tooling

- **Lifecycle + modes:** stake-elected lanes are enabled via
  `nexus.staking.public_validator_mode = stake_elected` while restricted lanes
  stay admin-managed (`nexus.staking.restricted_validator_mode = admin_managed`).
  For stake-elected lanes, `RegisterPublicLaneValidator` now binds an explicit
  `peer_id`. Lane `0` requires a registered peer with a live, unbounded
  `Validator` key. Fresh global candidates outside the frozen topology are
  refused until prepared epoch-key transitions can safely activate them. Non-zero participant lanes
  prefer a live, unbounded `Committee` key and accept a `Validator` key for
  transparent-path compatibility without adding that peer to global quorum.
  Stake-elected operators can repair a stale
  binding with `RebindPublicLaneValidatorPeer` only before the pre-state freeze
  for its `activation_height`; an activated tenure must exit and release
  custody before a different peer identity can register. While a validator
  tenure remains open, or the current height is below its scheduled exclusive
  `deactivation_height`, the bound consensus key cannot be rotated or disabled.
  Admin-managed lanes now declare explicit manifest
  validator bindings of the form `{ "validator": "<i105-account-id>",
  "peer_id": "<peer-id>" }`; both lane modes route against stored `peer_id`
  bindings and neither derives authoritative peers from account signatories.
- **Activation/exit operations:** genesis bootstrap registrations use the
  genesis block height. Every non-genesis registration is assigned the next
  unfrozen election height and auto-promotes when that exact height is reached.
  If registration executes in an epoch-ending block, the successor roster is
  already frozen from pre-state, so activation targets the following election.
  Operators can also call `ActivatePublicLaneValidator` at or after the exact
  boundary. Exits move validators to `Exiting(release_at_ms)` and schedule the
  next unfrozen election height as the exclusive end of the half-open tenure
  `[activation_height, deactivation_height)`. Reaching the release timestamp
  records `Exited` but does not rewrite that tenure or immediately release the
  validator-capacity slot or peer reservation. Lane or dataspace reset, peer
  removal, consensus-key rotation or disablement, and canonical record pruning
  must wait until the current height reaches `deactivation_height`. Exiting and
  peer unregistration preserve all stake custody. The retained record is
  canonically pruned only after every bonded and pending-unbond position is
  finalised and no pending evidence lien remains; that pruning alone frees
  capacity and the peer for reuse. Capacity checks use
  `nexus.staking.max_validators` and count every retained record.
- **Lane retirement:** lifecycle and scale-in transitions fail closed while a
  lane retains a validator whose exclusive `deactivation_height` has not been
  reached or any bonded or pending-unbond stake. A lane may be retired or reset
  only after the tenure boundary has passed and its staking custody has been
  explicitly drained; retirement never serves as an implicit withdrawal or
  deletion path.
- **Config knobs:** `nexus.staking.min_validator_stake`,
  `nexus.staking.stake_asset_id`, `nexus.staking.stake_escrow_account_id`,
  `nexus.staking.slash_sink_account_id`, `nexus.staking.unbonding_delay`,
  `nexus.staking.max_validators`,
  `nexus.staking.max_stake_shares_per_validator`,
  `nexus.staking.max_pending_unbonds_per_share`,
  `nexus.staking.max_slash_bps`, `nexus.staking.reward_dust_threshold`, and the
  validator-mode switches above.
  `SumeragiNposParameters.reconfig.epoch_length_blocks` defines the election
  boundary grid. It, `evidence_horizon_blocks`, and
  `slashing_delay_blocks` are immutable after the initial NPoS parameter
  installation; horizon plus delay cannot exceed three epoch lengths.
  Thread them through
  `iroha_config::parameters::actual::Nexus` and surface them in `status.md`
  once GA values are ratified.
- **Torii/CLI quickstart:**
  - `iroha --operator-private-key-file /absolute/runtime/operator.key app nexus lane-report --summary`
    summarizes each lane's governance module, manifest readiness, validator
    roster, quorum, and protected namespaces. Add `--only-missing
    --fail-on-sealed` in rollout gates to reject a required lane-governance
    manifest that remains sealed.
  - The staking CLI requires `--peer-id` on validator registration and exposes
    `staking rebind --lane-id <id> --validator <i105-account-id> --peer-id <peer-id>`
    to repair stake-elected validator peer bindings in place.
  - `iroha_cli app nexus public-lane validators --lane <id> [--summary]`
    surfaces lifecycle/tenure markers (pending target height,
    `activation_height`, exclusive `deactivation_height`, exit release, slash
    id) alongside bonded/self stake and the bound `peer_id`. `peer_id` is
    non-null for both stake-elected and admin-managed lanes.
    `iroha_cli app nexus public-lane stake --lane <id> [--validator <i105-account-id>] [--summary]`
    mirrors the `/stake` endpoint with pending-unbond hints per `(validator, staker)` pair.
  - Torii snapshots for dashboards and SDKs:
    - `GET /v1/nexus/public-lanes/{lane}/validators` – metadata, authoritative
      `peer_id`, status
      (`PendingActivation`/`Active`/`Exiting`/`Exited`/`Slashed`), activation
      height, exclusive deactivation height, release timers, bonded stake, and
      last reward epoch.
      Optional `canonical I105 literal rendering` controls the literal rendering
      (canonical I105 output only).
    - `GET /v1/nexus/public-lanes/{lane}/stake` – stake shares (`validator`,
      `staker`, bonded amount) plus pending unbond timers. Optional
      `?validator=<i105-account-id>` filters the response for dashboards that focus
      on a single validator; `canonical I105 rendering` applies to all literals.
    - `GET /v1/nexus/public-lanes/{lane}/rewards/pending` – pending rewards per
      asset for the requested account. Requires `account=<i105-account-id>` and accepts
      optional `asset_id` and `upto_epoch` filters; `canonical I105 rendering` applies to
      the account literal in the response.
  - Lifecycle ISIs use the standard transaction path (Torii
    `/v1/pipeline/transactions` or the CLI instruction pipeline). Example Norito JSON
    payloads:

    ```jsonc
    [
      { "ActivatePublicLaneValidator": { "lane_id": 1, "validator": "<i105-account-id>" } },
      {
        "ExitPublicLaneValidator": {
          "lane_id": 1,
          "validator": "<i105-account-id>",
          "release_at_ms": 1730000000000
        }
      }
    ]
    ```
- **Torii authoritative routing:** routed public-lane proxy requests use only
  the authoritative peer set resolved from explicit manifest validator bindings
  (admin-managed lanes) or stored validator `peer_id` bindings (stake-elected
  lanes). Torii does not spray unrelated online peers for routed public-lane
  traffic. If authoritative bindings are missing, stale, or all authoritative
  peers are offline, Torii returns deterministic `503 route_unavailable`
  instead of probing non-authoritative peers and timing out.
- **Telemetry + runbooks:** metrics expose validator counts, bonded and pending
  stake, reward totals, and slash counters under the
  `nexus_public_lane_*` family. Wire dashboards to the same data set used by
  NX-9 acceptance tests so validator deltas and reward/slash evidence remain
  auditable. Slashing instructions remain governance-only; reward recording must
  prove payout totals (hash of payout batch).

## 4. Roadmap alignment

- ✅ Runtime and WSV storages implement the NX-9 validator lifecycle; regressions
  cover activation timing, explicit peer bindings, peer prerequisites, delayed
  exits, custody-preserving peer removal, retirement refusal with live custody,
  and re-registration after every retained position is finalised.
- ✅ Torii exposes `/v1/nexus/public-lanes/{lane}/{validators,stake,rewards/pending}` with
  Norito JSON so SDKs and dashboards can monitor lane state without custom RPCs.
- ✅ Torii public-lane proxying now fails closed on missing authoritative peer
  bindings instead of spraying generic online peers.
- ✅ Pending stake-elected registrations can repair authoritative peer drift
  with `RebindPublicLaneValidatorPeer`, while admin-managed lanes publish explicit
  `{ validator, peer_id }` bindings and expose non-null `peer_id` values
  through Torii snapshots.
- ✅ Config and telemetry knobs are documented; mixed deployments keep
  stake-elected and admin-managed lanes isolated so validator rosters stay
  deterministic.

## Candidate admission and conserved rewards

`RegisterPublicLaneCandidate` carries the ordinary registration, a BLS proof of
possession, an exact activation-height target, and a peer signature over the
network-bound registration and target. Core verifies account ownership and peer
consent, installs a fresh peer/key when required, and bonds funds in the same
transaction overlay. The key activation lead time and next unfrozen election
boundary determine eligibility. Failed transactions leave no peer, key or stake
behind. Existing registered peers can consent through the same instruction.
Candidate consent cannot be reused at a different tenure. Fresh global peers
outside the frozen topology are refused before peer, key or balance mutation:
permissionless global eligibility would otherwise allow an election to outpace
its mandatory mint-finality keys. Signed fresh candidate admission is available
for independent nonzero stake-elected lanes; serving those lanes grants no global
vote. See the explicit protocol dependency below.

Activation, exit, bonding and withdrawals authorize the account that owns the
respective state. Raw peer administration retains its separate permission gate.
See the [CLI commands](../crates/iroha_cli/README.md#public-lane-staking-commands).

Reward distributions remain explicit fee-funded treasury decisions; the runtime
does not invent issuance, commission or participation formulas. Recording a
validated distribution increases `public_lane_reward_reserves` for the exact
source asset. Numeric transfer and burn admission protects that reserve,
including aggregate batch debits. If the fee sink also custodies stake, recording
excludes bonded and pending-unbond funds from the distributable balance.

A recipient's claim releases only its paid entitlement. Epoch zero is claimable,
replay cannot pay it twice, and amounts below the payment threshold remain owed
until enough accumulates. A failed payment restores its reserve and claim cursor.
Recorded source assets remain authoritative after fee-policy or lane-mode
changes. Snapshot restoration reconciles current and predecessor reserves with
reward records and claim cursors; lane retirement requires unpaid rewards to be
settled. The pending-rewards query includes unpaid epoch zero.

TODO: close the protocol and deployment outcomes in
[validator staking completion](staking_validator_completion.md) before claiming
permissionless production validator rotation.
