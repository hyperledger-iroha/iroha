# Genesis-bound node configuration

Status: proposal for owner decision (2026-10-29). Step 1, the field-level
mismatch diagnostic, is implemented. The `[settlement.router]` section and the
validity keys of `[content]` are removed from the node schema (owner decision
of 2026-11-11) and their values are protocol constants until a governed change
set activates the typed parameters `content_policy_v1` and
`settlement_router_policy_v1` (implemented, see "Frozen-baseline constraint on
G2"). The `[nexus.uploaded_models]` section and the two DA ingest quota
ceilings of `[nexus.da]` are removed the same way, until a change set
activates `nexus_quota_policy_v1`. G1 and the other typed parameters of G2 are
not implemented. G2 as first written is superseded by the frozen-baseline
constraint below.

## Problem

A signed genesis commits two digests of policy that a node still reads from
its own configuration file:

- `sumeragi_context.nexus_amx_context_hash`: the Nexus/AMX context preimage
  (`iroha_data_model::nexus::amx_context`, written by
  `iroha_config::parameters::actual::sumeragi_nexus_amx_context_preimage`);
- `sumeragi_context.execution_policy_hash`:
  `iroha_config::parameters::actual::execution_policy_digest_v1`, which also
  hashes the Nexus consensus-policy digest.

Native genesis execution (`crates/iroha_core/src/block/native_genesis_policy.rs`)
recomputes both from the node's staged State and refuses the genesis when either
differs. Every node file must therefore restate every non-default value the
genesis was signed with. A Kagami Sora network genesis plus a minimal
`kagami node init` file fails until `[nexus.fusion] exit_teu = 50000000` is
copied in (code default 6000). The value is already fixed by the signed genesis;
the node file only has to repeat it.

## What is already genesis-owned

These families are typed committed parameters. Node configuration cannot change
them; the digests read their committed value or protocol default.

| Family | Committed source | Census entry |
| --- | --- | --- |
| Pipeline execution limits, AMX budgets, `dynamic_prepass` | `pipeline_validity_policy_v1` | `cfg.pipeline.bounds` (done) |
| Pipeline gas identity | validated committed gas projection | `cfg.pipeline.gas_identity` |
| ZK, SCCP and confidential validity limits | `zk_validity_policy_v1` | `cfg.zk.validity_policy` (done) |
| Fraud admission | `fraud_admission_policy_v1` | `cfg.fraud.committed_policy` (done) |
| Root fees | `root_fees_v1` | `cfg.root_fees.committed_policy` |
| Public-lane staking terms | `public_lane_staking_v1` | — |
| Signature admission | `crypto_admission_policy_v1` | `cfg.crypto.algorithms` |

`[pipeline]`, `[zk]` and `[fraud_monitoring]` keep only node-local resources, so
they can no longer cause a genesis policy mismatch.

## What is fixed by the protocol

These values have no node key and no committed parameter. The digests hash the
protocol constant, which equals the former `iroha_config` default.

| Family | Fixed value | Census entry |
| --- | --- | --- |
| Settlement router inputs | former `[settlement.router]` defaults, until a change set activates `settlement_router_policy_v1` | `cfg.settlement_router` (done) |
| Content-bundle limits and defaults | former `[content]` defaults, until a change set activates `content_policy_v1` | `cfg.content` (done) |
| Uploaded-model ceilings and per-account DA ingest quotas | former `[nexus.uploaded_models]` and `[nexus.da]` quota defaults, until a change set activates `nexus_quota_policy_v1` | `cfg.nexus.bounds` (done) |
| Content publisher allow-list | empty: publishing is the executor's admission decision | `cfg.content` (done) |
| Content default read authorization mode | `Public`: a bundle manifest states any other mode | `cfg.content` (done) |

The node schema has no `[settlement]` section, and `[content]` keeps gateway
resources only.

## What a node file still restates

### Nexus/AMX context (G1 scope)

The exact config keys behind every node-configuration field of the context, as
named by `nexus_amx_context_fields_v1`:

| Section | Keys |
| --- | --- |
| `[nexus]` | `lane_count` |
| `[[nexus.lane_catalog]]` (keyed by `index`) | `alias`, `dataspace`, `shard_id`, `visibility`, `lane_type`, `governance`, `settlement`, `storage`, `proof_scheme`, `manifest_policy`, `confidential_compute`, `scheduler`, `settlement_buffer` |
| `[[nexus.dataspace_catalog]]` (keyed by `id`) | `alias`, `fault_tolerance`, `fee_sponsor_program_id` |
| `[nexus.routing_policy]` | `default_lane`, `default_dataspace`, `rules[].lane`, `rules[].dataspace`, `rules[].matcher.account`, `rules[].matcher.instruction` |
| `[nexus.staking]` | `public_validator_mode`, `restricted_validator_mode`, `max_validators` |
| `[nexus.axt]` | `slot_length_ms`, `max_clock_skew_ms`, `proof_cache_ttl_slots`, `replay_retention_slots` |
| `[nexus.fusion]` | `floor_teu`, `exit_teu`, `observation_slots`, `max_window_slots` |
| `[nexus.autoscale]` | `enabled`, `min_lane_id`, `max_lane_id_exclusive`, `target_block_ms`, `scale_out_latency_ratio`, `scale_in_latency_ratio`, `scale_out_utilization_ratio`, `scale_in_utilization_ratio`, `scale_out_window_blocks`, `scale_in_window_blocks`, `cooldown_blocks`, `per_lane_target_tps` |
| `[nexus.commit]` | `window_slots` |
| `[nexus.da]` | `q_in_slot_total`, `q_in_slot_per_ds_min`, `sample_size_base`, `sample_size_max`, `threshold_base`, `per_attester_shards`, `ingest_quota_window_blocks` |
| `[nexus.da.audit]` | `sample_size`, `window_count`, `interval_ms` |
| `[nexus.da.recovery]` | `request_timeout_ms` |
| `[nexus.da.rotation]` | `max_hits_per_window`, `window_slots`, `seed_tag`, `latency_decay` |

The context also holds protocol-fixed projection versions and reserved autoscale
metadata, which are not editable node configuration, and values genesis execution fixes (root fee policy,
committed AMX budgets, public-lane validators, lane lineage, autoscale
transition height). Execution recomputes those; no node file sets them. Its two
DA ingest quota ceilings (`nexus.da.ingest_quota_max_count_per_account`,
`nexus.da.ingest_quota_max_bytes_per_account`) keep their signed first-release
values (`NexusConsensusQuotaSlotsV1::FROZEN`); the quotas a chain applies are
its committed `nexus_quota_policy_v1` or that policy's protocol default.

### Execution policy outside the context (G2 scope)

Field lists are the `config_catalogs` of `specs/zk_runtime_validity_census.json`;
counts are fields in that catalog.

| Section | Fields | Census entry |
| --- | --- | --- |
| `[gov]` | 88 (`governance.*`) | `cfg.governance` |
| `[oracle]` | 29 (`oracle.*`) | `cfg.oracle` |
| `[nexus.endorsement]` | `committee_keys`, `quorum` | `cfg.nexus.policy` |
| `[nexus.governance]` | `default_module`, `modules` | `cfg.nexus.policy` |
| `[nexus.hf_shared_leases]` | `drain_grace` | `cfg.nexus.policy` |
| `[nexus.compliance]` | `enabled`, `audit_only`, loaded policy-set digest | `cfg.nexus.policy` |
| `[nexus.registry]` | loaded lane-manifest policy-set digest | `cfg.nexus.policy` |

The Nexus consensus-policy preimage still carries the atomic private-settlement
and uploaded-model and DA ingest quota slots, at their frozen first-release
values; no node file restates them. Private-settlement validity is the
committed privacy policy, and the quotas are `nexus_quota_policy_v1`.

## Step 1: field-level diagnostics (implemented)

`GenesisPolicyMismatch` retains the Nexus/AMX context preimage the node derived.
`iroha3d --check-config` reads `nexus-amx-context.v1.bin` beside `genesis.file`,
uses it only if `Hash::new(bytes)` equals the signed commitment, and compares it
with the derived preimage through `diff_nexus_amx_context_v1`:

```text
this node's Nexus/AMX context differs from the signed one (node/nexus-amx-context.v1.bin) in 1 field:
  nexus.fusion.exit_teu: signed genesis 50000000, this node 6000
```

Lanes, data spaces and lane lineage are matched by identity and routing rules
by position, so an entry present on one side is reported once. Without the file
the report says where to put it; a file of another genesis is refused by hash.
The execution policy has no published preimage, so an execution-only mismatch
names the sections in the G2 table.

## G1: derive the Nexus/AMX context from the signed genesis

No protocol change. Every chain already commits the preimage hash, so G1 needs
no new genesis.

1. `[genesis] nexus_context_file` names the preimage; it defaults to
   `nexus-amx-context.v1.bin` beside `genesis.file`. After its first start the
   node keeps the verified copy under `data_dir`, so a restart from Kura needs
   no operator file.
2. At configuration load, before Kura geometry, State, the DA proof-policy
   bundle or the handshake capabilities derive anything from `config.nexus`,
   `iroha3d` authenticates the bytes against the signed
   `nexus_amx_context_hash` and decodes every node-configuration field (the
   G1 table) with the strict grammar reader. Those values replace the
   corresponding `actual::Nexus` fields. Genesis-fixed fields are not applied;
   execution recomputes them and the existing hash comparison still guards them.
3. The `iroha3d` node loader rejects the G1 keys, as the ZK validity keys are
   rejected (`cfg.zk.node_schema`): a node file that sets one fails to load with
   an error naming the key and the genesis context that supplies it. The keys
   stay in the `iroha_config` schema only as genesis authoring input for Kagami
   (`kagami genesis sign --config`, Sora network generation). Compiled profiles move
   `[static.nexus]` into `[genesis_recipe]`, so `[static]` holds only settings no
   genesis digest covers (network frame limits, key algorithms, confidential
   switches), each tracked by its census entry.
4. Lane and data space descriptions and routing matcher descriptions are not in
   the context. They become empty on nodes that derive the catalog; they carry
   no consensus meaning.

Activation without migration:

| Network | Preimage source | Effect |
| --- | --- | --- |
| Kagami Sora networks | `kagami sora-network` already writes the file beside the signed genesis | Generated peer files drop their `[nexus]` catalog and `[nexus.fusion]` sections |
| Taira | already a public-reset input (`iroha taira`, `kagami genesis validate-prepared --nexus-context-output`) | Validator configs drop the G1 keys at their next rollout |
| Minamoto | `iroha3d --check-config --write-nexus-context <path>` on any validator whose current configuration passes, or Kagami restaging against the reference configuration; either output is accepted only by hash | Node files drop the three-lane catalog and routing |

Required tests: a minimal node file plus a Kagami Sora network genesis and its
context file starts without `[nexus.fusion]`; a substituted or truncated context
file is refused before Kura opens; a node file setting a G1 key fails to load;
restart from Kura uses the retained copy; the handshake `nexus_policy_digest`
and DA proof-policy hash equal those of a node configured with the explicit
values.

## G2: commit the remaining execution-policy families

Protocol change; completes F.4 for the G2 table, following the
`pipeline_validity_policy_v1` and `zk_validity_policy_v1` pattern.

1. Typed custom parameters, each with its type, protocol default and canonical
   validation in `iroha_data_model`: `governance_policy_v1`,
   `oracle_policy_v1`, `content_policy_v1`, `settlement_router_policy_v1` and
   `nexus_service_policy_v1` (the Nexus rows of the G2 table except the two
   loaded policy sets). Protocol defaults are today's `iroha_config` defaults;
   no default changes.
2. Genesis authors them with ordinary `SetParameter` instructions. Kagami
   renders them from the same authoring sections it reads today. A change takes
   effect from the block after the one that commits it.
3. The execution-policy digest hashes the effective value (committed parameter
   or protocol default) of each family, as it already does for pipeline, ZK and
   fraud. The G2 keys leave the node schema. Once no family is node-sourced,
   `execution_policy_hash` is removed from the genesis context (the existing
   `TODO(F.4)` on `execution_policy_digest_v1`).
4. Compliance and lane-manifest policy sets stay content-addressed files the
   node loads; their digests stay bound until G.3 commits the sets in State.

Activation without migration holds for a chain whose signed execution policy
equals the digest over protocol defaults for every moved family:

- Minamoto: its template sets no G2 key, so this is expected to hold. Before
  G2 lands, a regression test must recompute Minamoto's signed
  `execution_policy_hash` from protocol defaults plus its G1 context.
- Taira: its template sets `[gov]`, `[nexus.governance]`, `[nexus.registry]`
  and others, so its current genesis does not satisfy the rule. Taira adopts G2
  at a public reset whose genesis authors the G2 parameters.
- Kagami localnets are regenerated.

A chain that fails the rule needs a new genesis. Keeping a node-configuration
source for chains without the parameters would be a second source per fact and
a compatibility path, which the first release prohibits.

## Frozen-baseline constraint on G2 (2026-11-11)

A root that runs the live-upgrade foundation executes its frozen baseline
unchanged (`specs/live_runtime_upgrades.md` §1.1, §4). Recognizing a new typed
custom parameter is new behaviour: a `CanSetParameters` holder can submit
`SetParameter` for any unreserved custom identifier, the baseline stores it
without interpretation, and a binary that validated or applied it would disagree
with one that does not while both execute the same selection. Two rules follow.

1. A G2 parameter is published by a release addition and activated by a governed
   change set. Until the root's selection activates it, every binary stores the
   identifier opaquely, and validation, restore, projection, the State cells, the
   digests and the executor-migration ownership list all ignore it. The checkpoint
   policy of the lifecycle addition is the pattern
   (`crates/iroha_core/src/runtime/checkpoint.rs`, `admit_parameter`). Activation
   defines what happens to a value stored before it.
2. Adoption is never a reset. The paragraphs above that tie Taira's adoption to a
   public reset and prescribe a new genesis for a chain that fails the rule are
   superseded: such a chain adopts by change set, with the addition's activation
   installing the values its signed genesis bound.

Removing a node key and fixing its value at the former default is not new
behaviour for a chain whose configuration never set the key: both binaries
compute the same results, and the digest and State-cell terms keep their bytes.
`[settlement.router]`, the `[content]` validity keys, `[nexus.uploaded_models]`
and the two DA ingest quota ceilings of `[nexus.da]` were removed this way.
Neither `configs/soranexus/nexus/config.toml` (Minamoto) nor
`configs/soranexus/taira/config.toml` sets one; a chain whose signed genesis
bound another value fails its genesis policy check at startup, and a validator
file that still names a removed key fails to load.

### Implemented: content limits and router inputs

`content_policy_v1` (`iroha_data_model::content_policy::ContentPolicyV1`) and
`settlement_router_policy_v1`
(`iroha_data_model::settlement_router_policy::SettlementRouterPolicyV1`)
follow rule 1.

- **Publication.** The release catalog publishes one Parameter definition per
  parameter: its identifier, the Norito schema of the typed record and the
  canonical bytes of its protocol default
  (`crates/iroha_core/src/runtime/catalog/parameters.rs`). Nothing else is
  published for it, so the definition activates from every selection a root
  may run, Taira's included. `draft_addition` drafts the change set under the
  module name `iroha.parameter.content_and_router_policy.v1`.
- **Recognition** (`crates/iroha_core/src/runtime/committed_policy.rs`). A root
  recognizes a parameter while its definition is active and no `Disabled`
  restriction names it. Nothing stores recognition: every reader derives it
  from the selection and parameters version it reads (block construction, views,
  `State::content_snapshot` and `settlement_snapshot`, the State execution-policy
  digest, and each side of the `state.content` and `state.settlement` cells), so
  restore, snapshots and the undo side of a cell need no further rule. A root
  that does not recognize a parameter decodes nothing and applies the protocol
  constants.
- **Values stored before activation.** The change set that makes a parameter
  recognized (activation, or lifting its `Disabled` restriction) writes the
  definition's initial value over whatever the root stored while the identifier
  was opaque, so such a value never becomes policy. The initial value is the
  protocol default, which equals the constants, so activation changes no
  committed byte. For these two parameters the values a signed genesis bound
  (rule 2) are those constants on every tracked network, so the initial value
  is also what rule 2 requires.
- **Recognized.** `SetParameter` admits only a valid policy, executor
  reconciliation can neither declare, replace nor retire the parameter, and a
  value committed in block N governs block N + 1 on, like every committed
  policy.
- **Retirement.** Block construction freezes policy before block start applies
  the height's change sets, so a change set that stops recognizing a parameter
  (a `Disabled` restriction or deactivation) lapses unless the committed value
  equals the initial value. That keeps its own block, like every later one, at
  the constants the successor selection requires (`live_runtime_upgrades.md`
  §6, "Scheduling"); governance sets the value back first. §7.1 defines the
  other restrictions for interfaces and implementations only; they do not
  change a parameter.
- **Local refusal.** Writing the initial value goes through the funded
  parameter owner. A finite-pool refusal stays a local storage refusal the node
  retries; any other refusal is a release defect and stops the node at that
  height. Neither turns an activation into a lapse.

### Implemented: uploaded-model and DA ingest quotas

`nexus_quota_policy_v1` (`iroha_data_model::nexus_quota_policy::NexusQuotaPolicyV1`)
carries the plaintext-byte and chunk ceilings of one uploaded-model bundle and
the count and byte ceilings of one account's DA ingests in a quota window. It
follows rule 1 exactly as the two parameters above: the release publishes its
Parameter definition beside theirs, the change set drafted under the same
module activates whichever of the three a root does not recognize yet, and
recognition, the initial value, admission and retirement follow the same rules.
Validation refuses a zero DA ceiling; a zero uploaded-model ceiling refuses
every bundle.

- **Freezing.** Block construction freezes the quotas of the predecessor with
  the other construction inputs (`StaticExecutionPolicy::nexus_quotas`).
  Uploaded-model registration and DA ingest staging read that value; static
  block validation reads the value of the State the block builds on, the
  predecessor's selection and parameters for a same-height replacement
  (`StateQueryView::nexus_quotas_for_validation`).
- **Signed commitments.** The Nexus consensus-policy preimage, its State
  encoding and the Nexus/AMX context keep the four quota slots at their
  first-release values (`NexusConsensusQuotaSlotsV1::FROZEN` in
  `iroha_config`), as the atomic private-settlement slots already are. The
  execution-policy digest, the `state.nexus` cell and the signed context
  therefore never move with a governed quota change, and the protocol default
  equals the frozen slots, so a chain that never set a quota key keeps every
  signed byte.
- **Node configuration.** The node schema has no `[nexus.uploaded_models]`
  section, and `[nexus.da]` keeps only `ingest_quota_window_blocks`, which the
  consensus-policy digest still binds.

### Implemented: content publishers

The deleted `[content]` publisher allow-list became an executor permission
(owner decision of 2026-11-11). Recognizing a new permission name is new
behaviour too, so `CanPublishContentBundle`
(`iroha_executor_data_model::permission::content`) follows rule 1 the same way
(`crates/iroha_core/src/runtime/content_publish_permission.rs`).

- **Publication.** The release catalog publishes one Permission definition for
  it, outside `INITIAL_EXECUTOR_PERMISSION_NAMES`, so the frozen baseline's
  inventory is unchanged. Nothing else is published for it: `iroha.grant` and
  `iroha.revoke` keep whichever implementation the root selected, the
  restricted-read successor included, and each reaches the same Initial rule
  for this name. `draft_addition` drafts the change set under the module name
  `iroha.permission.content_publish.v1`.
- **Recognition.** Every reader derives it from the selection it executes. A
  root that has not selected the definition executes exactly as the frozen
  baseline does: the Initial executor admits `PublishContentBundle` and
  `RetireContentBundle` in genesis only. A root that selected it also admits,
  after genesis, publishing by an account holding the exact unit permission
  directly while no restriction of any kind names the definition, and
  retirement by the bundle's creator.
- **Grants.** On a root that selected the definition, the permission is a
  direct account grant, the deleted allow-list one entry per account. No role
  carries it: Core refuses to put it in a role for every executor, so only its
  issuer confers it. Only a `CanSetParameters` holder grants or revokes it; a
  holder cannot delegate it and may renounce its own grant. Any restriction
  (`live_runtime_upgrades.md` §7.1, Permissions) refuses new grants and keeps
  revocation, so the grant set only shrinks while it lasts.
- **Activation.** A change set that activates the definition lapses while any
  account or role holds a permission of this name, so a token granted under
  the baseline's rules never becomes publishing authority.
- **Retirement.** A change set that retires the definition lapses while any
  account or role holds it, or while any content bundle published after
  genesis remains, expired ones included. Retiring returns
  `RetireContentBundle` to the baseline's genesis-only rule and nothing prunes
  expired bundles, so such a bundle would lose its only removal path; a grant
  left behind would be an unknown token that blocks a later activation.
  Governance keeps the definition restricted until both clear.
- **Cleanup.** While the definition is selected, revoking a held direct grant
  and retiring a bundle published after genesis run as completion work under
  closed admission and completion-only. A change set that would leave
  `iroha.revoke` unbound or disabled lapses while a direct grant remains, and
  one that would do so to `RetireContentBundle` lapses while such a bundle
  remains. Nothing else waits for them: they pin no completion closure, so a
  rebinding of either instruction, a kernel or profile change and an executor
  replacement proceed.

## Alternatives not proposed

- Publish execution-policy and Nexus-policy preimages and invert them into
  configuration. Several projections are not invertible (canonical sets, digests
  of loaded policy files), it needs a decoder per field, and it entrenches a
  digest F.4 removes.
- Fall back to node configuration when a parameter is absent: two sources for
  one fact, which C1 of `specs/configuration_simplification.md` and the
  first-release policy reject.
- Change protocol defaults to localnet values: Minamoto's genesis uses the
  current defaults (`exit_teu` 6000).
- Typed parameters for the G1 fields: correct for new chains, but Minamoto's
  genesis has none, so it would need a new genesis. G1 reaches the same node
  file without one.

## Decisions requested

1. Adopt G1, including removal of the G1 keys from the `iroha3d` node schema.
2. Adopt G2, under the frozen-baseline constraint, as the completion of F.4
   for `[gov]`, `[oracle]` and the Nexus service rows.
3. Tie Taira's G2 adoption to its next public reset.
4. Approve the Minamoto recomputation test as the G2 activation gate.
