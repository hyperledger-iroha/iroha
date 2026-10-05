# Original frozen contract subject relation

`state/contract_subject_validation.rs` is the sole subject-ledger predicate used
by committed `CheckedContractSubjects`, exclusive startup restore and frozen
`complete/frozen_contract_subjects`. The shared original wrapper preserves source
Current/Predecessor before inverse Current/Predecessor. Subject derivation uses
the existing V1 address/tag/counter/hash and strict Ed25519 candidate check with
no new retry limit. Keep every original lifecycle version/revision, pending-owner,
Parliament owner/delegation/origin and emergency-hold invariant; exact subject and
current/pending owner account existence; exact optional active-code/instance
membership; and both inverse directions. Inactive/Parliament bindings and absent
historical deployer/proposer accounts keep their original accepted semantics.
No artifact, permission, alias, activation, execution effect or AXT predicate is
added. Capture never reconstructs or repairs an index.

The existing Images surface is closed to native CommittedStorageView,
FrozenStorageImages and exclusive startup History. Each exact-size physical
advance is prepaid before next. Empty exhaustion is free. Predecessor masking
and lookup scan every original candidate after a match, retaining None/no-op
preimages and a found borrow until traversal finishes. Complete key equality is
paid before Eq; no native get/contains/Ord, key parse, filtered predecessor
iterator, copied row or allocating scratch occurs in bounded validation.

Account controller geometry uses the existing domain/account helper, whose body,
charging order and tests are unchanged. Address comparisons admit both original
spellings. Stored subject geometry and derived expected bytes are admitted before
the original single-Ed25519 check. Every hash-to-point attempt admits its exact
hash bytes and strict curve check first. Lifecycle validation admits the fixed
version/revision controls, current/pending owner controllers and discriminants,
delegation/origin/hold options, Parliament origin hashes and complete hold
hashes/heights/reason before the unchanged predicate. Both optional active hashes
are admitted before copying/comparing. Historical origin accounts remain
uninspected. Single Ed25519 controller comparison costs34 per operand; two-member
multisig costs84. Direct lifecycle geometry costs49; a pending two-member owner
adds85; a hold adds112 plus actual reason bytes. Optional active hashes cost1 or33.
These are local source-work references, never gas, controller, row or validity
limits. The committed16384 row allowance and startup unbounded Work are unchanged.
TODO: finite startup work and reconstruction allocation custody remain open.

Committed capture computes all four native currentness Results before propagation
or combination and preserves original typed refusal/source order. Currentness
precedes success, semantic rejection or work refusal. Existing grouped encoding
retains the Result through native fences, reader release and the State generation
fence before exposing either outcome.

Frozen capture accepts only the actual Frozen StateBlock bindings/reverse/accounts/
instances fields. Every original must be unreleased, belong to its exact State
storage target and share the same actual Ordinary/Replace mode. All four target
probes are computed before combination. Both images validate before original
current binding rows enter the unchanged canonical paired encoder under that
State's original ivm_execution_budget. No fresh State/World view, target refresh,
caller rows or replacement pool is acquired. Incomplete/foreign/released/mixed
sources return None; local work/pool/row/payload refusal leaves the original block
retryable. Later target publication cannot change its borrows. Existing snapshot
charges refund after final backing retirement; paired schema-name/selection/
serializer scratch funding remains open. Native committed reader/control
retention still has its existing separate resource-admission gate.

This scoped output advances217 catalog entries to192 native callbacks, seven
completed checked outputs and18 explicit missing adapters. Coverage does not
supply a complete State root, joint predecessor coherence, private disclosure,
artifact/entrypoint permission, execution effects, AXT issuer/asset/replay/proof
authority, consensus finality, deployment or release qualification.
TODO: complete all18 adapters, canonical cells/frontiers, authenticated history,
joint original owner/mode/predecessor/currentness custody and the sole
StatePublication plus durable Kura publication/recovery before claiming closure.
