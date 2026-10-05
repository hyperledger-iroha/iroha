# Original account alias relation

The committed and scoped frozen alias owners use one allocation-free
`account_alias_ownership::validate_original_account_aliases` over sealed original
CommittedStorageView/FrozenStorageImages native current/undo iterators. It grants
no account, name, scope, authorization, State publication or finality authority.

The exact existing pass order is Current accounts/primary labels, Current aliases,
Current reverse buckets, then the same three passes for Predecessor. A primary
label must pass the unchanged `account_label_is_pii` rule and resolve to that
same account. An alias must pass the same PII rule, target a live account, and be
in that account's reverse bucket. Every reverse bucket must be nonempty, and
each member must resolve to its bucket account. The six mismatch kinds remain
PrivateLabel, PrimaryLabel, MissingAccount, MissingAlias, EmptyBucket and
ForeignAlias. WorkLimit is a local refusal, never a validity verdict.

Account identity remains domainless. Full existing alias equality includes its
label bytes, optional alias-domain segment and numeric dataspace. This relation
adds no name normalization/parse, DomainId or dataspace-catalog lookup, controller
admission, account identity hash, lease, permission, scope-directory or rekey rule.
Accounts without primary labels remain valid. Existing PII acceptance/rejection,
including its UTF-8/early-character behavior, remains unchanged.

Every nonempty physical iterator advance is admitted before next. Predecessor
masking scans every original undo candidate even after a match, then every undo
entry including None and equal/no-op preimages. Lookup and reverse membership
scan all original candidates/members after matches. Bounded validation uses no
native get/contains/Ord, clone, parsing, source reconstruction or sorting scratch.

Each Eq prepays both full operands. Alias geometry is actual label bytes + domain
option1 + actual domain-name bytes when present + dataspace8. The shared borrowed
controller helper remains byte-identical, including its admission order and three
tests: Single controller2 + actual payload; multisig12 + actual member count +
sum(member3 + actual payload). Malformed typed compact keys are compared through
prepaid Eq without Ord or parse/error allocation. The primary-label option costs1;
the empty-bucket guard costs1. Each unchanged PII call admits its entire actual
label-byte scan, even when the predicate returns early.

The original two-account/one-merchant fixture costs 722 for both images. Its one
physical absent transient alias preimage adds 37 to the alias pass and37 to the
reverse lookup, for 796. Each extra 37 is (1+17+18) mask work plus one final absent
undo advance. Exact-minus-one and exact boundaries keep those complete tails.
These replace the old row-only fixture boundary 11, preserving every original
semantic/refusal/assertion and mutation control. The old fixture allowance 128 is
replaced by a high test allowance so geometry refusal cannot mask semantic tests.

The committed descriptor 7814 is a local full-geometry reference: one Single
Ed25519 account with a 255-byte primary label, a 255-byte alias-domain segment,
and its reverse member. Alias width 519; per image accounts 1364 + aliases 1433 +
reverse 1110 = 3907. Both images without undo cost 7814. Independent test arithmetic
asserts this exact reference without calling production acceptance to calculate
it. Wider controllers, implicit accounts, member sets and quadratic predecessor
cuts can need a larger local allowance; this changes no name/account/row/gas,
protocol/configuration or ledger-validity limit.

Committed capture retains all three original readers. Their Result currentness
probes materialize before propagation, in accounts/aliases/reverse error order,
for success, WorkLimit and mismatch. Native publication refusal/change wins over
those outcomes. Paired encoding holds its Result through all native probes, reader
release and the existing State generation fence, including encoding refusal.

The frozen owner requires the actual StateBlock World AggregatePublication::Frozen,
three unreleased original images, all three belongs_to probes against the actual
State accounts/aliases/reverse targets and equal actual acquisition modes.
Ordinary and Replace use their real native histories. It borrows only the original
fields.state_ref.ivm_execution_budget, validates both images before allocating,
and encodes original current aliases through the existing canonical paired encoder
with literal world.account_aliases. Incomplete, foreign, mixed or released sources
return None. Local work/pool/row/payload refusal leaves the same block available
for retry; later equal/changed target publication cannot refresh originals. Output
charges remain owned until actual final snapshot drop.

After reviewed composition with the contract-subject adapter, the closed 217-output
catalog has 192 native callbacks, eight checked outputs and 17 explicit missing
outputs. These are encoder coverage counts only, not State closure or authority.

TODO: all remaining adapters, canonical cell/frontier capture, authenticated
history, joint original owner/mode/predecessor/currentness custody, sole
StatePublication and durable Kura publication/recovery remain required. Existing
native committed reader/control allocation admission, paired schema-name/selection/
serializer scratch and alias restore/rebuild funding are open physical gates.
Work admission is not physical allocation funding. Account scope/rekey/history,
SNS lease/permission, Musubi/AXT issuer/asset/replay/proof/consensus/deployment/release
readiness and private-row disclosure remain outside this relation.
