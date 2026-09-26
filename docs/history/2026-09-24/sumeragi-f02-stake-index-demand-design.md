# F02 stake-index demand and ownership design, 2026-09-24

This record covers a source audit, a demand pass, and the later funded flat
share-key backing cut on the existing `optimizations` checkout. The paragraphs
below retain the original design audit; the final addendum states the current
implementation. Complete production resource qualification remains open.

At the initial demand-pass audit, `PublicLaneStakeIndex::from_world` in
`crates/iroha_core/src/smartcontracts/isi/staking.rs` built a
`BTreeMap<(LaneId, AccountId), IndexedValidatorStake>`, grew one share-key
`Vec` per validator, cloned account identifiers into both structures, and
retained `Quantity` aggregates backed by variable-size `BigInt` values. It is
called from both parent penalty preparation and final effects application in
`crates/iroha_core/src/sumeragi/penalties.rs`. Charging only the parent call
would leave the application path and its original owner uncovered.

`StorageReadOnly::iter` yields borrowed keys in canonical order without an
iterator heap allocation. The implemented `PublicLaneStakeIndexDemand` pass
reads that exact world view before the existing builder starts allocating. It
counts share rows and contiguous `(lane, validator)` groups with checked
arithmetic, checks canonical key order, row/key agreement, per-validator share
capacity and per-share pending-unbond capacity, and validates the representable
fixed layouts of a future flat key backing and group-range backing. It compares
borrowed adjacent identities and does not clone an `AccountId` or build a
temporary map. The existing fill then reads the same world overlay and checks
with release-build runtime errors that its built group/row counts match the
demand. The index reports two source-row visits per retained row; the focused
three-row `parent_snapshot_two_pass_stake_index_retains_exact_exposure` case
therefore reports six visits. Its B-tree, growing vectors
and nested clones remain unfunded.

A complete retained-backing cut should replace the standard `BTreeMap` and
growing per-validator vectors; their allocator node/growth layouts cannot be
prepaid exactly through the current API. One `ChargedBuffer` can hold every
canonical share key in source order, and a second can hold group ranges and
aggregate values. A group identifies its validator by the first key in its
range, so binary search yields the same `&[PublicLaneStakeShareKey]` contract
without another identity clone. A move-only charge sidecar for cloned key
allocations must be retained after the share-key owner in struct field order:
the keys and their backing deallocate before those nested charges refund. On
partial construction, local variable declaration/drop order must provide the
same guarantee. The original reservation is split into actual allocation
layouts before each clone and any unused remainder is checked then released.

Each cloned share key contains two `AccountId`s. A single-key controller clones
one compact public-key `Box<[u8]>` holding an algorithm tag plus payload. A
multisig controller also clones its member-vector backing and one compact key
per member. The demand pass can calculate checked candidate layouts using
`AccountId::controller`, `MultisigPolicy::members`,
`PublicKey::try_to_bytes`, and `Layout::array`. The exact geometry of these
derived `Clone` implementations must be confirmed with allocator-observed
single-key and multisig tests before any prepayment is claimed. Aggregate
`Quantity`/`BigInt` allocations and arithmetic scratch remain separate open
charges even after the fixed index backings and key clones are funded.

At that audit point, parent preparation had the State-owned evidence
preparation budget. The final-application call ran through
`StateTransaction`, which then had no handle to that original pool. The
design required borrowing the pool from its enclosing `StateBlock`/`State`,
rather than creating a new pool. In addition,
`PreparedPristineConsensusEffects::apply` then mapped all application
errors to `NposEffectsInvalid`. A capacity refusal from the funded index must
instead preserve `EvidencePreparationError::Admission` as
`BlockValidationError::EvidencePreparation`, with the original release wait
and unchanged accepted work. It must not become a consensus-invalid block or
transaction. The enclosing transaction must be discarded without publishing
partial effects on refusal.

The coordinated Core test binary passed `stake_index_demand_` 4/4 and
`parent_snapshot_two_pass_stake_index_retains_exact_exposure` 1/1. These
results were obtained before a subsequent lifetime-only lint fix in the
F02-owned borrowed-evidence helper; that final source awaits the next combined
Core rebuild. Anonymous lifetime elision failed to compile on this toolchain
(E0658); the final source retains a named lifetime and also bounds the
iterator by it. The focused tests cover empty demand, multiple lanes and group boundaries,
exact share capacity and one over capacity, noncanonical order, mismatched
row/key identity, pending-unbond capacity, fixed-layout overflow, and
materialization-count drift. The
existing parent index test exercises the preflight through `from_world`.
Orphan validator rows remain validated by the existing fill after the pass;
the future charged cut must move this check to a borrowed merge preflight
before allocation. The later charged-index cut also needs single-key and multisig allocation
geometry, exact-capacity/refusal, original-owner lifetime, rollback and
same-State retry tests. Config user/default/actual minima must then include
the new maximum admitted demand without reducing the existing eight concurrent
prune and pending plans. No first-release resource gate is closed by this
bounded pass.

## Funded flat share-key backing addendum

`PublicLaneStakeIndex` now stores all share keys in canonical source order in
one fixed `ChargedBuffer<PublicLaneStakeShareKey>`. Its per-validator B-tree
entries retain ranges into that same backing, so `share_keys` still borrows an
exact validator slice. The full backing is reserved before the index clones
keys or inserts B-tree nodes. A release-build check then verifies that all
ranges partition the exact flat length and that each range's first and last
keys match its indexed validator; an adversarial test rejects skipped ranges
and wrong-group keys. The original State owns a distinct finite
`consensus_stake_index_bytes` pool; it cannot consume the eight evidence
prune/pending plan allowance. Parent preparation borrows this pool from State,
and final application borrows it through `StateBlock.state_ref` into the exact
`StateTransaction`. Replay prevalidation retains the same pool owner. A config
change refuses to replace it while any charge remains live.

The configured default is 64 MiB; the minimum is one flat share-key element.
The latter is only a nonzero usable floor, not a claim that the largest
accepted stake table fits. Total rows are not currently bounded by the
per-validator cap alone. On a flat-backing capacity refusal, parent candidate
construction retains `EvidencePreparationError` and its original release
observation. Pristine final application preserves that typed error as
`BlockValidationError::EvidencePreparation`, allowing the existing follower
path to return local busy/recovery. The speculative State transaction drops
before publishing partial consensus effects. No capacity refusal becomes a
consensus-invalid transaction result.

The new tests cover one-byte-below-capacity refusal with unchanged source,
same-State retry after owner release, exact retained/refunded backing,
configured minimum and live-replacement refusal, replay owner identity, and
pristine error classification. The first combined Core compile found only a
test-module name collision and two test-fixture owner names; these were fixed
after that compile exited. The corrected locked combined Core build then
passed `stake_index_demand_` 5/5. The same Core libtest binary
`target/debug/deps/iroha_core-ac2131f69e3280a3` passed parent flat-backing
refusal/retry 1/1, two-pass exact exposure 1/1, configured pool original-owner
test 1/1, replay original-owner test 1/1, pristine typed local-refusal test
1/1, and the adjacent Native preparation-error family 5/5. The locked config
stake-index finite-pool/minimum test passed 1/1. The six Core selector logs
are `target/f02-<selector>-same-source.log` for `parent_refusal`,
`parent_exposure`, `config_owner`, `replay_owner`, `pristine_local`, and
`native_local`.
Earlier demand 4/4 and parent 1/1 results were from the pre-funding build.
The B-tree node allocations, its cloned AccountId keys, each share-key's
nested AccountId clones, Quantity/BigInt limbs, arithmetic
scratch, and maximum-shape rollback/restart evidence remain open. This cut
does not close F02 or a release gate.

## Funded flat validator-group backing addendum

The next bounded cut removes `PublicLaneStakeIndex`'s per-validator B-tree. A
second `ChargedBuffer` retains one validator group in the same canonical order
as the stake-share table; each group owns its range and aggregate quantities.
Borrowed binary search replaces B-tree point lookup. The demand pass obtains
both exact `Layout::array` shapes, and the original State stake-index pool
atomically reserves their sum before either backing allocates or clones a key.
The prepaid reservation splits into two exact original charges. A partial
constructor refusal drops both owners and refunds the pool; an error after
materialization also drops both backings without changing the source world.
The default remains 64 MiB. The user/actual minimum now covers one share plus
one group; a Core test compares that config floor to the actual two type sizes.

The new focused cases cover canonical group order and duplicate/reordered
refusal, exact combined capacity and one-byte local `Capacity` refusal,
same-State retry after release, two-validator exposure parity, an admitted
first backing followed by a refused second constructor with full refund, and
an orphan source row causing post-allocation refund without source mutation.
The borrowed exposure helper still skips a key removed by an earlier slash in
the same owned, ordered scratch bundle; it rejects a missing row when current
validator totals remain unchanged. This preserves the existing legitimate
multi-slash path and does not authorize an external source to omit inventory.
Physical global-allocator failure has no deterministic injection in the Core
test harness; the source handles its typed `Allocator` result and the tests
exercise the equivalent partial-owner refund path through exact-layout
constructor refusal. Scoped `rustfmt --check` and `git diff --check` passed.
The corrected locked Core libtest build passed `stake_index_demand_` 6/6.
Against the same binary, nine adjacent selectors passed 1/1 each (9/9):
`stake_index_partial_second_backing_refusal_refunds_original_pool`,
`parent_stake_index_backing_refusal_preserves_source_and_retries_after_release`,
`parent_snapshot_two_pass_stake_index_retains_exact_exposure`,
`parent_snapshot_rejects_orphan_stake_share_aggregate`,
`indexed_exposure_borrows_ordered_rows_and_skips_consumed_share`,
`configured_stake_index_pool_keeps_original_charge_through_reconfiguration`,
`replay_prevalidation_retains_original_evidence_preparation_pool`,
`penalty_derivation_capacity_is_local_validation_not_invalid_effects`, and
`evidence_preparation_refusal_is_local_and_keeps_its_original_release`.
Their exact logs are `target/f02-two-buffer-<selector>.log`. The first Core
compile found E0282 on the group buffer's type before its first append; an
explicit `ChargedBuffer<IndexedValidatorStake>` annotation fixed it, and the
corrected source was rebuilt. The config minimum selector is tracked
separately from this Core binary.

This funding covers only two fixed Vec backings. Every cloned `AccountId`'s
nested bytes, `Quantity`/`Numeric` nested storage, arithmetic scratch, source
world lookup storage, maximum-shape admission and restart/fault evidence remain
open. The separate evidence-preparation pool still retains its eight-plan
allowance. F02 and release qualification remain open.

## Next nested-owner cut (read-only design)

The first remaining retained allocation is the `AccountId` material copied by
the index: two identifiers per share-key clone and one validator identifier
per group. A single controller clones one compact-key `ConstVec<u8>` backing
of `1 + payload.len()` bytes. A multisig controller additionally clones one
`Vec<MultisigMember>` backing and each member's compact-key backing. A
source-derived pass can inspect the borrowed controller, member inventory and
public-key payload lengths, check the exact layouts, and add all `2R + G`
copies to the same original State-pool reservation before any copy. The copy
path must be fallible for both controller forms, retain the prepaid nested
owner until the flat keys and groups physically drop, and return a typed
local refusal on capacity or allocation failure. Merely reserving bytes while
calling derived `AccountId::clone` would not establish a fallible allocation
boundary. The existing crypto `PublicKey::try_clone_for_admission` and a
fallible exact member-vector allocation are candidate building blocks; their
full allocation behavior and policy reconstruction need tests before use.

`Quantity` remains a separate lower-layer obligation. It contains `Numeric`
and an adaptive `num_bigint::BigInt`; canonical two's-complement byte length
does not prove the exact retained limb layout or arithmetic scratch used by
derived clone and checked addition. A fallible prepaid bigint/quantity clone
and arithmetic path, with physical owner retention, is required before
claiming that demand funded. The current config minimum covers only one fixed
share-plus-group pair, and the 64 MiB default is not a maximum-shape proof:
multisig admits up to `u16::MAX` members and the protocol key-payload ceiling
is 8,258 bytes. Candidate qualification must either establish a governed
shape bound fitting the pool or measure/configure a sufficient production
pool without turning irreducible `ExceedsLimit` into an endless retry. No
nested-owner implementation or test is claimed by this design addendum.

## Nested account clone funding implementation and focused validation

The stake-index demand scan now measures every retained `AccountId` copy: two
per share row and one per validator group. Each single-key controller contributes
one exact tag-plus-payload compact-key layout. Each multisig controller adds one
exact member-vector layout and one compact-key layout per member. The model's
allocation-free layout visitor and fallible clone share that representation;
the multisig clone requests its exact member-vector layout and each compact key
uses the existing fallible exact `PublicKey` copy. No `AccountId::clone` remains
in index materialization.

The original State stake-index pool atomically prepays both flat buffers, the
nested charge-owner buffer, and every nested key/member layout before any copy.
Each exact split charge remains in the index until the physical share/group keys
drop, including on partial constructor failure. The config minimum now covers
one share and group with three Ed25519 key copies plus their charge owners. The
64 MiB default is unchanged, and larger valid keys/multisig shapes are checked
against the finite pool as actual source demand. Focused tests exercise the
single/multisig layouts and copies, exact pool refusal, partial key-copy failure,
zero-row construction, same-State retry and refund. These focused checks do
not establish full release qualification.

`Quantity`/BigInt limbs and arithmetic scratch in aggregate construction and
exposure remain unfunded, as do source-world lookup storage and full maximum-
shape policy/measurement. This does not close F02 or release qualification.

The DataModel single/multisig fallible-clone selector passed 1/1 in
`target/f02-nested-account-data-model.log`. A Norito cumulative decode-scope
refusal now has its own `EvidencePreparationError::DecodeScope` carrying the
attempted and limit bytes; it is not mislabeled as an exact allocator layout.
The partial multisig-clone test checks that the original prepaid owner refunds
after this refusal. The Apply test checks that the typed refusal remains local
recovery and cannot become a consensus-invalid verdict. The corrected Core
libtest build passed `stake_index_demand_` 7/7 and eleven adjacent selectors
11/11 against the same binary, including partial clone/refund, typed local
Apply classification and prior parent retry/exposure cases (18/18 total).
The first Core test compile found only a return-type mismatch in the test's
Norito limit wrapper; switching to `with_decode_limits_scope` retained the
same thread-local bound and typed outer error. The corrected build log is
`target/f02-nested-account-core-rerun.log`.
The configured minimum selector passed 1/1 in
`target/f02-nested-account-config.log`. The borrowed demand scan visits each
multisig member for each retained account copy; exact byte accounting here
does not admit that execution work or prove a maximum-shape runtime bound.

## Quantity limb and addition boundary (read-only next-cut design)

`PublicLaneStakeIndex::from_world` still clones each source `bonded` and pending
amount, clones the running aggregate, calls `quantity_add`, and later clones
aggregate values for validator-total comparison and `total_exposure`. That
helper calls `Quantity::checked_add`, then `Numeric::try_decimal_add_observed`:
canonicality probes, scale alignment, bigint addition, up to 28 trailing-zero
normalization divisions and final materialization can each allocate. The
parent snapshot also retains `Quantity` in `ValidatorLocator`, and apply-local
slashable exposure and `max_slash_amount` create more values. Funding only the
index's three group fields would move, not close, these owners.

The pinned `num-bigint` 0.4.6 stores each magnitude in `Vec<BigDigit>`: native
digits are `u64` on 64-bit targets and `u32` otherwise. A nonzero canonical
`Quantity` mantissa is at most 511 magnitude bits, so its retained digit layout
is `Layout::array::<Digit>(ceil(bit_len / (8 * size_of::<Digit>())))`: at most
8 `u64` or 16 `u32` digits. Zero has no digit allocation. The two's-complement
encoded length is not this physical layout. `Vec::clone` and the public
`BigInt::new(Vec<u32>)` are not fallible exact-layout handoffs; on 64-bit the
latter repacks into another native-digit Vec. `NumericWorkStep` reports logical
work widths, not every physical allocation. Prepaying encoded bytes and then
calling the current `Clone`/`checked_add` path cannot qualify resource admission.

The smallest separable prerequisite is a reviewed native-digit adoption API
in the pinned bigint backend (or an equivalent owned representation) that
accepts an exact, fallibly allocated native-digit Vec without repacking.
`iroha_primitives` can then expose borrowed `Quantity` clone demand and a
fallible prepaid clone: reserve the exact digit layout from the original State
pool, allocate that layout, fill from the allocation-free digit iterator, and
retain its charge until the cloned value drops. An empty value must make no
heap allocation or charge. A mere `try_reserve_exact` is insufficient because
Vec capacity may exceed the requested layout; the handoff needs one checked
allocation with a documented physical owner. This is a bounded API/test cut,
not permission to use the existing unfunded arithmetic in production.

For complete index construction, replace the three group `Quantity` aggregates
with one inline, canonical fixed-limb accumulator each and borrow source
values. A platform-independent base-2^32 algorithm needs at most 19 limbs for
one conceptual aligned sum: each 511-bit positive input can gain at most 94
bits from `10^28`, then one carry bit, for at most 606 bits. Its checked add,
normalization and signed-512-bit result check must match `Quantity::checked_add`
after *every* row/pending request, including overflow cases. Fixed stack
scratch has no heap owner; the changed `IndexedValidatorStake` layout remains
charged by the original State pool before construction. The eventual
`Quantity` returned to the parent locator/action path still requires the
fallible native-digit materializer and a charge retained through that value's
physical drop. Keep `ValidatorLocator`, apply-local recomputation and slash
math open until their output owners and temporary overlap are similarly
admitted. Make the bounded addition the canonical `Quantity::checked_add`
implementation and verify parity with signed `Numeric` arithmetic; do not
introduce a second production arithmetic path.

Focused validation should compare mixed-scale, carry, zero, trailing-zero,
maximum-positive, overflow-after-normalization and pending-unbond sums against
the existing `Quantity` semantics on 32- and 64-bit targets. Add exact
one-byte-below capacity and injected allocator-refusal tests for clone/output
construction, partial-build refund and same-State retry, group/locator owner
drop-order checks, and a no-source-mutation failure test. Count the borrowed
row, pending and member visits plus the at-most-28 normalization steps as
separate execution work; the 64 MiB default and maximum-shape bounds remain
unqualified until that work and all overlapping retained owners are measured.

## Fallible native-digit clone prerequisite (focused validation)

The pinned 0.4.6 bigint source now has one native-digit Vec adoption entrypoint
that normalizes in place without repacking. A focused backend test checks that
the original pointer and capacity remain the resulting `BigUint` owner.
`iroha_primitives::BigInt` derives the exact native magnitude layout from
borrowed bits, allocates that layout once with a fallible allocator, fills it
from the allocation-free native-digit iterator, and hands the Vec directly to
the pinned backend. Its test covers zero with no allocation, positive and
negative values, exact layouts and injected allocator refusal. `Quantity`
delegates the layout and clone while preserving canonical scale; its test
covers fractional and maximum positive 512-bit values. The original State pool
is deliberately not claimed here: callers still must reserve and retain each
physical charge before using this API. Existing arithmetic, exposure, locator,
source lookup, work and maximum-shape gates remain open.
The locked, offline focused selectors passed 1/1 each: vendored
`num-bigint@0.4.6 native_digits_adopt_the_original_allocation`,
`iroha_primitives admitted_clone_uses_exact_native_digit_layout_and_refuses_allocator`,
and `iroha_primitives quantity_admission_clone_preserves_canonical_value_with_exact_digit_layout`
(3/3 total). The pinned local backend also removes its upstream `no_std`
declaration and conditional non-`std` branches, so it has one standard-library
implementation under the repository's IVM-only policy. The same three focused
selectors passed 3/3 after that adjustment; `python3 scripts/check_ivm_only.py`,
`cargo fmt --all -- --check`, and `git diff --check` passed. The Cargo lockfile
selects this local patch. Full workspace, 32-bit target, State-pool ownership
and release qualification remain open.

## Borrowed validator-total comparison (focused validation)

The stake-index validation loop now compares each materialized group's bonded
and self-bonded `Quantity` values directly with the current validator record.
When no group exists, it compares both record totals with canonical zero. This
preserves the exact mismatch rejection while removing up to two transient
native-digit clones per validator record with a present group. It does not
change retained State-pool demand or fund aggregate construction, arithmetic
scratch/results, `total_exposure`, locator retention, or source lookups.

The combined-source Core libtest run passed `stake_index_demand_` 8/8,
`parent_snapshot_two_pass_stake_index_retains_exact_exposure` 1/1, and
`parent_snapshot_rejects_validator_totals_that_disagree_with_index` 1/1
(10/10 for this cut). The same Core build passed the adjacent signed-genesis
fixture 1/1. Scoped Rust formatting and `git diff --check` passed before the
build. Full arithmetic/resource and release gates remain open.
