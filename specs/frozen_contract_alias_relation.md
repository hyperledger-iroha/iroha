# Scoped frozen contract-alias inverse and lease relation

The implementation is coupled to
`state/authority_registry/grouped_ownership/contract_aliases.rs` and
`state/authority_registry/complete/frozen_contract_aliases.rs` in `iroha_core`.
It replaces the committed checker body with one shared sealed `RawStorageImages`
relation and supplies the exact `world.contract_alias_bindings` frozen adapter.
The same 217-output catalog then retains 21 explicit missing adapters.

## Original relation

Canonical `Storage<ContractAddress, ContractAliasBindingRecord>` and inverse
`Storage<ContractAlias, ContractAddress>` must agree in both current and exact
predecessor images. Each binding has its stored alias/address inverse; each
inverse resolves to that same binding. Missing/wrong entries and duplicate
canonical alias targets yield MissingMember; orphan/foreign aliases yield
ForeignMember. The unchanged shared `alias_lease::violation` also requires expiry
above bound, grace only with expiry, and grace not before expiry.

Expired and undeployed rows remain representable until ordinary cleanup. This
relation grants no deployment, account/contract authority, current resolution,
cleanup, authenticated execution history or finality. Canonical typed strings
are borrowed as stored, including original address spelling; no normalization,
address decode, alias parse, owned text or codec alternative is introduced.

## Local work contract

Every physical current/undo iterator advance is prepaid by one unit, including
masked rows, no-op preimages and absent tombstones. The predecessor merge funds
both complete key texts before ordering. Every inverse candidate funds both
complete alias and address texts before comparing; scans do not exit after a
match or skip the second comparison after inequality. Each binding visit prepays
32 units before the shared lease predicate's at-most-two 8-byte scalar-pair
comparisons. Checked subtraction/conversion overflow is local WorkLimit.

For one 17-byte `router::universal` pair and current 60-byte address, no undo:
`2 * (4 physical + 32 lease + 4*(17+60) text) = 688` units. Equal no-op undo in
both tables costs 1000; one newly inserted pair with absent preimages costs 502;
only one absent tombstone per empty table costs 2. A source no-op plus absent key
costs 932 or 1172 according to its actual original native ordering.

The named committed descriptor allowance is
`2*(4+32+4*((3*MAX_NAME_BYTES+3)+60)) = 6696` per admitted row, using the existing
255-byte Name limits and current Bech32m address geometry only as a local reference.
Runtime comparisons charge actual complete UTF-8 lengths. This adds no text,
table, gas, consensus, validity or decode limit. Larger quadratic/undo cuts can
defer; a work refusal cannot become a stable corruption verdict before admission.

## Retained owners and gates

Committed capture retains both original readers through validation/paired encoding
and preserves its native publication identity fence, which wins over success,
corruption and local work refusal. Frozen capture accepts only an actual StateBlock
whose entire World is frozen, whose two still-live fields belong to the exact
original State targets, and whose acquisition modes match. Both frozen image pairs
and the original State allocation pool remain retained through the shared relation
and existing paired encoding of current canonical rows. Incomplete, foreign,
released or mixed-mode owners return None; local refusal permits same-owner retry.
Later target publication does not refresh or replace the retained source cut.

Tests cover both-image malformed leases/inverses, duplicate targets, every physical
undo form, exact work and relation allocation assertions, maximum/UTF-8 comparisons,
publication races, actual replacement modes, original-pool refusal/refund and paired
root equality. Target-stage test code is not execution evidence; root owns exact
candidate compilation, focused/adjacent tests, formatting and codec checks.

TODO: consume this adapter with every remaining canonical owner/cell and the
transaction frontier in the sole StatePublication capsule, then establish whole
predecessor coherence, authenticated history custody and durable recovery. Scoped
row snapshots must never be substituted for partial authoritative State roots,
finalized anchors or private-row proof/disclosure authority.
