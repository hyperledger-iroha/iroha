# Authoritative governance JSON vectors

These are hand-authored boundary vectors derived from the current Core/Norito
source contract. They are not Rust-generated output, proof fixtures, evidence of
finality, or completed native/cross-SDK parity. `cases.json` pins the accepted and
rejected cases. Requests use the exact referendum selector `ref-1`.

Norito writes `u64` and `u128` as unquoted decimal JSON integer tokens. The maximum
vectors and `tally-large.json` deliberately exceed JavaScript's safe integer
range; do not load them with ordinary `JSON.parse` before testing a lossless
decoder. Quantities are canonical decimal strings. Tagged enums always contain
`kind` and `content`, including explicit `null` for unit variants. Missing
referenda omit `referendum`; missing lock corpora omit `locks`. Found lock corpora
retain both native `locks` object levels.

The account strings are copied from the first three validator identities in
`fixtures/space_directory/profile/cbdc_lane_profile.json`; the asset definition
comes from `fixtures/nexus/uaid_portfolio/global_default_portfolio.json`. No keys,
genesis state, or signed history are supplied by these vectors. Identifier token
preservation is distinct from verifying identity, permissions, or custody.

The JavaScript reader checks u64/u128 ranges, exact field inventories, frozen
conviction policy, mode/status/result consistency, decision equations, aggregate
overflow, requested tally/corpus selector, evaluated tally coordinates, and
lock-owner/map-key equality. Locks alone do not contain a referendum context;
their quantities cannot establish frozen units, weight, or asset ownership.
Likewise a referendum response carries no proof or requested-id field, so its
contents alone cannot establish a cryptographic selector or finality binding.
Anonymous standalone ballot admission and the private tally protocol remain
unqualified.

Native producer/roundtrip validation remains required for these exact roots:

- `iroha_torii::gov::ReferendumGetResponse`, including the Core
  `GovernanceReferendumRecord` and the model `PlainVotingContextV1`,
  `PlainConvictionPolicyV1`, `PlainVotingResultV1`, and
  `PlainVotingDecisionV1` tagged layouts.
- `iroha_torii::gov::LocksGetResponse`, including Core
  `GovernanceLocksForReferendum`, `GovernanceLockRecord`, and
  `GovernanceLockCustody`. Compare actual producer JSON for the wrapper, then
  roundtrip the decode-capable nested records.
- `iroha_torii::gov::TallyGetResponse`, including zero-height/zero-hash and
  nonzero evaluated coordinates and all u128 boundary values. This response DTO
  is a producer, so verify native serialization before claiming wire parity.

The model/Core source paths are `crates/iroha_data_model/src/governance/conviction.rs`
and `crates/iroha_core/src/state.rs`; response producers are in
`crates/iroha_torii/src/gov.rs`. Rejected vectors must fail for their intended
field or semantic mismatch, while each corresponding unmodified positive vector
must pass. A rejected vector with invalid state semantics can still be valid
Norito syntax; codec decoding alone does not establish state validity.
