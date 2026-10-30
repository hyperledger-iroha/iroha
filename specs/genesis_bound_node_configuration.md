# Genesis-bound node configuration

Status: proposal for owner decision (2026-10-29). Step 1, the field-level
mismatch diagnostic, is implemented; G1 and G2 below are not.

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
| `[nexus.da]` | `q_in_slot_total`, `q_in_slot_per_ds_min`, `sample_size_base`, `sample_size_max`, `threshold_base`, `per_attester_shards`, `ingest_quota_window_blocks`, `ingest_quota_max_count_per_account`, `ingest_quota_max_bytes_per_account` |
| `[nexus.da.audit]` | `sample_size`, `window_count`, `interval_ms` |
| `[nexus.da.recovery]` | `request_timeout_ms` |
| `[nexus.da.rotation]` | `max_hits_per_window`, `window_slots`, `seed_tag`, `latency_decay` |

The context also holds protocol-fixed projection versions and reserved autoscale
metadata, which are not editable node configuration, and values genesis execution fixes (root fee policy,
committed AMX budgets, public-lane validators, lane lineage, autoscale
transition height). Execution recomputes those; no node file sets them.

### Execution policy outside the context (G2 scope)

Field lists are the `config_catalogs` of `specs/zk_runtime_validity_census.json`;
counts are fields in that catalog.

| Section | Fields | Census entry |
| --- | --- | --- |
| `[gov]` | 88 (`governance.*`) | `cfg.governance` |
| `[oracle]` | 29 (`oracle.*`) | `cfg.oracle` |
| `[content]` | 11 (`content.*`) | `cfg.content` |
| `[settlement.router]` | 7 (`settlement.router.*`) | `cfg.settlement_router` |
| `[nexus.endorsement]` | `committee_keys`, `quorum` | `cfg.nexus.policy` |
| `[nexus.governance]` | `default_module`, `modules` | `cfg.nexus.policy` |
| `[nexus.hf_shared_leases]` | `drain_grace` | `cfg.nexus.policy` |
| `[nexus.uploaded_models]` | `max_plaintext_bytes_per_model`, `max_chunk_count_per_model` | `cfg.nexus.bounds` |
| `[nexus.atomic_private_settlement]` | 15 (policy and bounds) | `cfg.nexus.policy`, `cfg.nexus.bounds`, `cfg.nexus.settlement_activation` |
| `[nexus.compliance]` | `enabled`, `audit_only`, loaded policy-set digest | `cfg.nexus.policy` |
| `[nexus.registry]` | loaded lane-manifest policy-set digest | `cfg.nexus.policy` |

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
2. Adopt G2 as the completion of F.4 for `[gov]`, `[oracle]`, `[content]`,
   `[settlement.router]` and the Nexus service rows.
3. Tie Taira's G2 adoption to its next public reset.
4. Approve the Minamoto recomputation test as the G2 activation gate.
