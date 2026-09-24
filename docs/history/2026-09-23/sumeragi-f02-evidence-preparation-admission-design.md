# F02 committed-evidence preparation admission

2026-09-23, `optimizations`. This is an implementation design and source audit,
not an installed production cutover or a release-gate pass. No Core or
configuration code changed for this record.

The [current prune planner](../../../crates/iroha_core/src/sumeragi/evidence.rs)
calls `v2_committed_evidence_snapshot` before State acquisition. That snapshot
preallocates 124 entries and deeply clones each retained `EvidenceRecord`,
including its context, roster, proofs of possession and signed artifacts. The
limits are 124 records, 4 MiB per proof and 16 MiB of aggregate encoded proof
bytes. Encoded bytes do not bound the clone's nested allocation layouts.
`v2_committed_evidence_prune_keys_from_state` needs only copied 32-byte keys.

A bounded first slice can scan borrowed rows under one immutable `StateView`,
check *every* row against the existing count and proof-byte caps, and append only
terminal keys whose subject height is below
`current_height.saturating_sub(horizon)` for a present, nonzero parent horizon.
Missing or zero horizon, pending records and in-horizon terminal replay fences
remain unpruned. Exactly 124 rows and exactly 16 MiB pass; row 125, an
individual proof above 4 MiB or an aggregate above 16 MiB must leave no partial
plan, as the current snapshot does. `incoming_records` cannot trigger eviction.
Sort keys strictly ascending before use: the [application check](../../../crates/iroha_core/src/sumeragi/penalties.rs)
requires canonical order and rechecks each key against the post-execution
horizon. Parent and post-execution checks both remain necessary when a candidate
updates the signed horizon.

The smallest testable helper would take an explicit `AllocationBudget`, create a
`mv::allocation::ChargedBuffer<Hash>` of fixed capacity 124 **before** opening
the State view, then scan and sort in place. Its requested backing allocation
is `Layout::array::<Hash>(124)`, 3,968 bytes. Tests should compare its result
with the existing planner across mixed pending/applied/cancelled records,
nontrivial hash insertion order, horizon boundaries, record and byte limits,
and zero, exact and one-byte-short allocation pools. That helper has **not**
been added or installed; its tests would prove only prune-plan equivalence and
local allocation refusal, not complete F02 admission.

Production cutover must be atomic with policy and error routing:

- Add a distinct node-local `consensus_evidence_preparation_bytes` field through
  [user](../../../crates/iroha_config/src/parameters/user.rs),
  [actual](../../../crates/iroha_config/src/parameters/actual.rs) and
  [defaults](../../../crates/iroha_config/src/parameters/defaults.rs)
  `NexusStorage`, then construct one immutable pool in `State`. This is the
  narrowest existing configuration owner already available to ordinary and
  Native State validation. The `SumeragiStorage` policy is runner-owned; the
  `SumeragiV2RuntimeLimits` policy participates in shared consensus geometry.
  The existing retained-carrier shell pool explicitly does not fund nested
  evidence. Set the default and minimum from checked simultaneous live layouts,
  not the encoded-proof ceiling.
- Move the charged key owner through
  [prepared pristine effects](../../../crates/iroha_core/src/block.rs), Native
  controls and the merge-beacon capability without replacing it. Borrow its
  slice during pristine application; keep the same original candidate and
  executed overlay through retries. Once application has installed the result
  in the retained overlay, the key backing can be released. Do not rederive
  after execution or from a candidate-updated horizon.
- Represent capacity, layout overflow and allocator refusal with a typed local
  `EvidencePreparationError`. Propagate it from the prune planner and evidence
  admission through `prepare_pristine_consensus_effects`,
  `state_block_for_execution` and `prepare_native_execution_controls`. Extend
  `BlockValidationError`, its no-rejection event mapping, and the
  `V2ApplyService` local-refusal classifier. Preserve a pool-capacity release
  wait for retry; permanent limit/allocator errors require local recovery.
  `PenaltyApplier::derive_from_stable_parent` currently returns `eyre::Result`;
  both [block validation](../../../crates/iroha_core/src/block.rs) and
  [candidate assembly](../../../crates/iroha_core/src/sumeragi/v2_runner.rs)
  must downcast this typed refusal rather than convert it to invalid NPoS
  effects or a rejected proposal marker.

The [full evidence snapshot](../../../crates/iroha_core/src/sumeragi/evidence.rs)
still serves admission, local pending selection and penalty derivation. A
charged prune-key buffer does not fund it. Replace whole-record clones with
bounded borrowed indexes and a charged due-penalty projection where possible,
or plan every actual nested clone layout and materialize it fallibly before
State acquisition. Penalty planning presently needs the key, admission and
context heights, the conflict signer and its roster identity after releasing
the view; it does not need the whole signed proof. Its validator index, scratch
overlay, action vectors and selected local pending proofs also need separate
resource charges. These remaining owners keep F02 open.
