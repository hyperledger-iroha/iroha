# Original account identity relation

`authority_registry/account_identity_ownership.rs` owns the single closed relation
used by committed account capture and `complete/frozen_account_identity.rs`.
Both current and exact predecessor preserve the original six categories:
OpaqueWithoutUaid, UaidBinding, OpaqueBinding, ForeignUaid, ForeignOpaque and
DuplicateOpaque. Implicit accounts with no UAID and no opaque identifiers remain
valid. Every explicit stored UAID has its exact inverse account, each opaque
member has its exact inverse stored UAID, and both indexes have complete reverse
membership. Only after those directions pass does unequal source-member/index
cardinality prove duplicate opaque members. Identifiers remain stored protocol
values; no AccountId hash relation, controller validation, alias, PII, metadata,
permission, signature or execution predicate is added. Neither map is repaired.

The sealed RawStorageImages inputs are only original committed or frozen native
maps. Every actual current/undo/member advance is prepaid before next; empty
iterators are free. Predecessor masking scans all undo candidates, including those
after a match, then every undo row including absent/no-op preimages. Each lookup
scans its whole original logical image. The full reverse opaque slice is scanned
and paid even after a match. No native get/contains/Ord, key parse, clone, sort,
filtering predecessor iterator, allocating error conversion or scratch map occurs
in the production relation.

Each comparison admits both complete keys before Eq. UAID and opaque keys cost
the existing Hash::LENGTH (32) per operand. AccountId controller geometry is shared
with the domain relation through `borrowed_controller_work::prepay_account_id`:
variant 1; Single tag 1 plus actual retained payload; Multisig version 1/threshold 2/
count 8, all member advances, and each tag 1/payload/weight 2. The helper preserves
exact existing domain charging order and the callback's original typed refusal.
Malformed typed compact keys remain exact equality inputs without being granted
validity or producing Ord's fallible key error allocation.

One Single Ed25519 account with one UAID and one opaque costs 264 forward +134
UAID reverse +200 opaque reverse per image, or 1196 over both images without undo.
The explicit-plus-implicit fixture costs 1474; one two-member Ed25519 multisig
costs 1796; one discarded compact controller costs 812. Two opaque members cost 2376,
including the tail after a reverse match. Exact-1/exact tests bind these to actual
physical scans. The named committed descriptor 1196 is a local reference only.
Wider controllers, long vectors and quadratic cuts can defer for a larger admitted
allowance. No controller/member/row/gas/codec/ledger validity limit is changed.

Committed capture retains all three original readers. All three native identity
Result probes are evaluated before applying any error propagation or boolean
combination; original typed failures retain source order. Native publication wins
before success, corruption or work refusal. The account encoder captures its
result, checks all originals, releases readers, then checks the existing State
generation fence before exposing encoding success or its exact refusal.

Frozen capture accepts only actual StateBlock fields, a fully Frozen World,
unreleased original accounts/UAID/opaque images belonging to their exact State
storage targets, and equal actual Ordinary/Replace modes. All target probes are
evaluated before combination. It validates both images and encodes only original
current accounts through the existing paired encoder with that State's original
ivm_execution_budget. Incomplete, foreign, released and mixed sources return None.
Local work/pool/row/payload refusal leaves the same original block available for
retry. Last snapshot owner refunds retained charges after final backing retirement.
Later target publication cannot refresh any original; Replace keeps rewound images.

This scoped adapter advances the closed 217-output catalog to 192 native callbacks,
six completed checked outputs and 19 explicit missing adapters. These are encoder
coverage counts only. It returns no complete State root, joint predecessor
coherence, private disclosure, execution anchor, finality or release qualification.
Existing paired schema-name/selection/serializer scratch funding remains open.
TODO: join every remaining checked output, canonical cell/frontier and authenticated
history with all original owners/modes/predecessors in the sole StatePublication
owner and durable Kura publication/recovery before any complete authority claim.
