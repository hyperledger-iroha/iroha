# Scoped original asset-definition relation

This relation is coupled to `iroha_core::state::authority_registry` modules
`grouped_ownership/asset_definitions.rs`, `grouped_ownership/confidential_policies.rs`,
`complete/grouped_capture.rs` and `complete/frozen_asset_definitions.rs`. One sealed
`RawStorageImages` validator serves retained committed capture and the frozen
`world.asset_definitions` catalog adapter. It borrows all seven actual native
owners: definitions, domains, optional definition-domain contexts, domain and
owner definition groups, pending-transition entries and per-height counts.

## Preserved relation and precedence

Each phase checks Current then exact Predecessor, in this order: definition/domain
references and context inverses; owner groups; domain groups; pending policy shape,
transition inverses and exact per-height counts. Sources precede their inverse
indexes. The original Source reasons and MissingMember, EmptyGroup and
ForeignMember classifications remain unchanged. Restricted definitions require an
owning domain; every explicit owning domain must exist. Context rows agree with the
stored optional domain. Owner/domain groups contain precisely their source members
and have no empty, missing or foreign groups. The unchanged model policy predicate
checks pending transition shape. Each pending row has its (height, stored ID) entry
and nonzero height count; every transition has its source and height, and each
nonzero count equals all pending definitions at that height. Overflow preserves the
original fixed Source reason.

No account existence, embedded definition ID equality, UUID reparse, Domain value
or owner equality, asset balance/quantity/alias permission, `zk_assets`, parameter
registry or feature-authority predicate is added. Malformed stored typed account
keys remain compared as stored, without parsing or an Ord-based lookup.

## Prepaid inspection work

A physical current/undo/member advance costs one before access. Each predecessor
current row visits every original undo candidate, including candidates after a mask
match, funding both keys before Eq; each final undo visit also funds its Option tag.
Lookups and membership scans fund every original candidate and continue through the
complete tail after a match. Empty-group, optional-domain and policy tags are funded
before inspection. Every comparison funds the complete two operands even if an
initial field differs. The asset ID callback admits its existing sixteen UUID bytes;
domain keys admit both complete label byte lengths. Account comparisons reuse the
unchanged borrowed controller helper, including all actual nested member advances,
algorithm/weight fields and public-key bytes. Heights cost eight and counts four.
Pending policy inspection funds mode and pending tag (2), then new mode, height,
previous mode, transition ID and window tag (43), and an actual window (8). No absent
window, unused policy feature payload, Domain body or unrelated quantity is scanned.
Checked conversion/subtraction is local WorkLimit, distinct from ledger validity.

The named committed allowance uses one Single Ed25519 owner (34), one existing
maximum 63+63-byte domain (126), one definition and all one-row inverses, and one
valid pending-window policy (53), with no undo: both images cost
`2 * (831 context/reference + 207 owner + 578 domain + 318 policy) = 3868`.
The original no-pending fixture costs 434 without context and 1218 with its actual
25-byte domain. One absent definition undo increases those fixtures by 175 and 245.
The original standalone policy fixture costs 1710; a source no-op plus absent
preimage adds 808, an absent transition preimage adds 300 and an absent count
preimage adds 57. Independent fixture equations use the original maps, not discovery
by trying production allowances. Wider controllers and quadratic current/undo or
inverse cuts can need more local admitted work; this reference introduces no row,
controller, gas, codec, configuration or policy validity limit.

## Original owners, pool and publication fences

Committed capture acquires definitions, domains, contexts, domain groups, owner
groups, transitions and counts in the original order. All seven native currentness
Results are materialized before propagation. Native identity changes win over
success, semantic failure and WorkLimit. Readers remain held through the existing
paired canonical encoder; every Result is probed again, readers are dropped, and
the State generation fence is checked before the encoding result can escape.
The unchanged original State allocation pool funds admitted paired output backing.

Frozen capture requires a wholly Frozen World in an actual StateBlock, seven
still-live frozen image pairs belonging to the corresponding original State
storage targets, and seven matching Ordinary or Replace acquisition modes. It
retains all original images and the same State pool through validation and paired
encoding of current rows. Partial, foreign, mixed or released sources return None;
no target refresh, reconstructed owner or alternate pool is acquired. Local work,
row, payload or allocation refusal leaves the same block available for retry.
Later target publication does not replace the retained frozen images.

Tests retain all original selectors and semantic assertions and add independent
work boundaries, full masked/tombstone/member tails, typed refusal, original
predicate absence, phased precedence, all-owner publication checks, encoding/State
fences, actual seven-owner histories and replacement modes, canonical bytes/root
parity and original-pool retry/final-owner refund. Staged tests are not execution
qualification; exact candidate compilation and runtime checks remain required.

TODO: fund remaining native reader/control acquisition, paired schema/name and
serializer scratch, restore/rebuild allocations and other State/native backing.
TODO: consume every remaining checked adapter, canonical cell/frontier and original
current/predecessor relation together in the sole StatePublication and durable Kura
history/recovery owner. This scoped table relation establishes no complete-State
finality, finalized execution anchor, disclosure proof, Musubi/AXT/private settlement,
VM/State/hardware/network or release readiness.
