# F02 stake-index preparation owner boundary — 2026-09-23

The borrowed evidence slice removed nested proof copies from live penalty derivation, but the stake-index and action-planning allocations are still outside original-owner admission. `PenaltyApplier::parent_snapshot` calls `PublicLaneStakeIndex::from_world` before constructing its own `validator_map`. The index inserts `IndexedValidatorStake` values into a standard `BTreeMap` and pushes cloned `(LaneId, AccountId, AccountId)` share keys into nested `Vec`s. The penalty map then clones each active validator's complete share-key slice with `.to_vec()`, so a retained share-key inventory has two independent heap owners during planning. It also allocates validator-count and locator maps. The subsequent action builder creates the scratch State block, action vector, and consensus-effects transactions. The per-validator limits reject an excessive row after some allocations; they do not prepay the complete index or its nested storage.

The existing `State::evidence_preparation_budget` is specifically the original pool for eight fixed committed-evidence prune-key arrays. Its default is `8 × (4 × 31 × size_of::<Hash>()) = 31,744` bytes, while one default staking lane permits 32 validators with up to 256 share rows each. Reusing this pool for an unfunded index would conflate owners and refuse valid maximum-shape stake states. A fresh local `AllocationBudget` in `penalties.rs` would not account for concurrent retained preparations or keep the charge with the actual candidate owner.

Error routing also crosses shared owners. `v2_runner::candidate_attachments` maps only `StateAdmissionError` to `CandidateError::LocalStateAdmission`; other `PenaltyApplier` errors become `V2RunnerError::Candidate(String)`. Block validation in `block.rs` likewise maps only `StateAdmissionError::{History,Membership}` to local refusal; other errors become invalid NPoS effects. The current `StateAdmissionError` has no penalty-preparation variant. Passing a newly created `EvidencePreparationError` or generic `eyre` allocation error through these paths would misclassify a local capacity refusal as a deterministic invalid candidate or block.

The next implementation needs a State-owned, configurable penalty-preparation pool with finite defaults threaded through user and actual config, isolated-state cloning, and restart/lifecycle handling. Before index construction it must compute a checked maximum demand from the exact retained lane and validator policies, atomically reserve its concrete storage and nested share-key layouts, and retain charges with the physical index and any candidate-carried outputs. The index representation or allocator must make each actual `BTreeMap`/nested allocation chargeable; merely holding an estimated reservation beside standard collections is not enough. The validator map should use the index's share-key owner directly rather than cloning it. The scratch State block and action/effects vectors need separate admitted owners or one complete transferable reservation. Shared candidate and block validation types must route the original pool's refusal and release observation as local retry, while preserving deterministic-invalid results for malformed stake/evidence state.

This note changes no shared runtime or consensus behavior. F02 stays open. Required tests include exact configured capacity, one byte short, concurrent reservation contention and release, allocator failure before side effects, full-share/validator shape, malformed final row with no partial publication, StateView generation churn, proposal/validation parity, and restart after retained-candidate release.

## Current-source follow-up, 2026-09-24

There are two production index builds, not one. In addition to
`PenaltyApplier::parent_snapshot` (`crates/iroha_core/src/sumeragi/penalties.rs`),
`apply_npos_consensus_effects_to_transaction_inner` constructs
`PublicLaneStakeIndex::from_world` from the exact applying transaction overlay
when a slash action is present. The second build currently occurs after beacon
advancement, evidence pruning, and evidence admissions have begun. A certified
merge may change stake shares before that overlay exists, so a parent-derived
index cannot simply replace the applying index. The smallest coherent index
admission must fund both builds from one process-lived State pool and move the
applying build ahead of NPoS side effects. A local refusal must leave the
original candidate or applying owner available for retry, with no partial
consensus-effects publication.

The current `derive_consensus_penalty_actions` borrows the index's share-key
slice; the earlier whole-slice `.to_vec()` claim above no longer describes this
call. `validator_share_updates` in `staking.rs` does still clone each selected
share key and share into a scratch `Vec`, and the parent snapshot still creates
validator-count, peer-locator, pending-evidence, scratch-block, and action
storage. Those are separate outstanding owners. For the index itself, an exact
borrowed pre-scan can count canonical share rows and validator groups before
allocation. A chargeable flat representation with sorted validator entries and
one contiguous share-key backing would eliminate standard `BTreeMap` nodes and
per-validator `Vec` growth. Its checked demand must include both backings and
every owned nested copy: `AccountId` controllers contain compact public-key
`ConstVec` boxes, multisig controllers contain a member vector and member
keys, and indexed `Quantity` values can carry adaptive integer storage.
`mv::allocation::ChargedBuffer` currently funds only a fixed backing for
`Copy` elements and explicitly excludes referenced objects; a non-`Copy`
charged owner with a nested-copy policy, or an equivalent physical allocator,
is needed. A byte estimate retained beside the present collections is not
original-owner admission.

The local-error path must be completed before activating this reservation.
`v2_runner::candidate_attachments` downcasts only `StateAdmissionError` and
otherwise turns penalty derivation errors into `V2RunnerError::Candidate`.
`block.rs` maps only its `History` and `Membership` variants during parent
penalty validation. Applying errors are flattened to invalid NPoS effects in
`block/pristine_consensus_effects.rs`, and the certified-merge-beacon branch of
`block.rs` does the same. A typed penalty-preparation refusal must retain its
original `AllocationRefusal` and release observation, route to local candidate
retry, and reach a local `BlockValidationError` excluded from
`map_block_err_to_reason`; malformed staking state must continue through the
deterministic-invalid route. Extending `StateAdmissionError` and
`StateBlockStartError` is one coherent route, but their exhaustive
`MergeLedgerCommitError` conversion and runner deferral must be updated too.

`parent_snapshot` currently validates the entire stake table before it scans
for due penalties. Returning early on an empty pending set would change that
behavior: the no-due corrupt-share, orphan-share, and capacity tests expect
those invariant failures. An allocation-saving early path needs an equivalent
borrowed full-table validator first; simply skipping the index is not a safe
source-preserving optimization.

The next focused tests must cover two concurrent charged index owners and
release-driven retry, exact and one-byte-short nested layouts, allocator
refusal before either build publishes effects, maximum share count with
multisig nested storage, a malformed last row, applying-overlay stake changes
after certified merge execution, and absence of an invalid-block verdict on
local capacity refusal. This follow-up is a design audit only: the current
source remains unfunded and F02 is not qualified.
