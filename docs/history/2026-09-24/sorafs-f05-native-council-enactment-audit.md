# F05 native council enactment audit (2026-09-24)

This is a read-only source audit. The canonical
`ProviderAdmissionCouncilPolicyV1` validates policy shape, lineage, and signed
claims, but Core does not retain or enact it. No native provider admission,
finalized grant, or local-config policy migration is completed here.

The smallest complete native policy cut is a **separate singleton Parliament
subject** and a `World` policy cell. The existing
`SorafsProviderGovernance` action is keyed by `action.provider_id()` and only
establishes, rebinds, or removes provider owners. Reusing it for council
rotation would mix a network-wide policy with a provider-owner subject.
Add one closed policy proposal/instruction to
`crates/iroha_data_model/src/governance/types.rs` and
`crates/iroha_data_model/src/isi/governance.rs`, then its single V1 wire ID,
instruction dispatch, permission mapping, proposal-content/subject handling,
and Parliament risk/attempt policy. Core's proposal path belongs beside
`ProposeSorafsProviderGovernance` in
`crates/iroha_core/src/smartcontracts/isi/world.rs`; the certified enactment
path must use the same `StateTransaction` and the current `World` cell.

Use `Cell<Option<ProviderAdmissionCouncilPolicyV1>>`: `None` means no council
authority, including before the first certified enactment. The initial policy
must be version one, revision one, have no predecessor, and match the exact
genesis-derived `StateTransaction.network_id`. A successor must pass
`validate_successor(current)`, preserving network and policy identity,
incrementing revision exactly once, and naming the current canonical digest.
Check the current cell again at certified enactment, before mutation; a stale
proposal cannot overwrite a newer policy. Pause is retained as a governed
policy state, not a locally selected admission switch. The fixed Parliament
subject must serialize competing rotations, while proposal content remains
bound to the complete policy. A direct, uncertified set instruction must not
exist.

State work spans the `World` inventory, `WorldData`, block, transaction, view,
accessor, default, apply, and teardown paths in
`crates/iroha_core/src/state.rs`. Add a required current-layout snapshot field
and validate any `Some(policy)` while decoding in
`crates/iroha_core/src/state/deserialize_world.rs`; there is no retired
snapshot fallback. Include the new proposal payload's nested signer backing in
`crates/iroha_core/src/state/tiered.rs` measurement, and cover proposal-record
exhaustive matches in `state.rs`, Parliament dispatch in
`crates/iroha_core/src/governance/parliament.rs`, and instruction registration
in `crates/iroha_data_model/src/isi/{mod.rs,registry/wire_ids.rs}` and
`crates/iroha_core/src/{smartcontracts/isi/mod.rs,executor_initial_permission_authority.rs}`.

The candidate's `trusted_signers: Vec<[u8; 32]>` checks its 32-key maximum only
in `validate()`, after generic Norito decode. The native field decoder must
inspect or constrain the advertised count **before** allocating the vector;
Norito provides `inspect_seq_len_slice` and `DecodeLimits`. Prefer a type-local
bounded signer-field decoder so instruction, proposal, and State-snapshot paths
share the same pre-allocation rule. A global 32-element limit on an entire
World snapshot would incorrectly constrain unrelated collections. Also bound
the complete policy frame, nested field bytes, aggregate decode allocation,
and State/journal retention before accepting the proposal.

The daemon currently builds `AdmissionRegistry` from node-local
`trusted_council_keys`, `signature_threshold`, and `envelopes_dir` in
`crates/irohad/src/main.rs::build_shared_sorafs_provider_cache`; Torii gateway,
discovery, and PoTR consumers use that registry. A policy-only native cut must
close that positive production admission path in the same change, removing or
demoting the local trust configuration in `crates/iroha_config` and its
daemon/Torii wiring. Do not pass the new World policy into the local registry
and call that a finalized grant: no admission head or tombstone exists yet.
Leave native grants fail-closed until signed admit/renew/revoke execution,
exact State/Kura projection, and a finalized reader replace local authority.
`capture_projection` in
`crates/iroha_core/src/query/provider_ingest_finalized.rs` currently starts
from `provider_owners` alone and cannot authenticate admission or revocation.

Focused tests for the next cut: 32 keys accepted and 33/hostile advertised
counts rejected before allocation; malformed/weak/duplicate keys; initial,
paused, immediate successor, skipped/stale predecessor, foreign network, and
changed policy ID; competing certified proposals with one exact head; direct
uncertified mutation refusal; State block and snapshot/restart roundtrips,
including missing required field; bounded proposal/journal allocation and
atomic rollback; and a daemon/Torii negative control proving local configured
keys or files cannot authorize a provider. No Cargo command was run for this
audit.
