# Original domain-owner relation

`authority_registry/domain_ownership.rs` owns the single sealed relation used by
retained committed readers and `complete/frozen_domain_ownership.rs`. Both current
and exact predecessor contain the same storage-key/bucket rule: every domain key
occurs in the bucket of its stored `owned_by()`, and every nonempty owner bucket
contains only domain keys stored under that owner. MissingDomain, EmptyBucket and
ForeignDomain retain their exact current/predecessor categories. The rule adds no
account existence, embedded Domain id, authentication or execution authority
predicate. It never repairs either native image.

Each physical current/undo row and set member is prepaid before its actual
iterator advance. Empty iterators consume no work. Predecessor masking scans all
original undo candidates by equality, including every candidate after an equal
key, then visits all undo entries including absent preimages. Full DomainId name
and dataspace UTF-8 bytes for both operands are funded before equality. Full
AccountId controller geometry for both operands is funded before equality:
variant 1; Single compact tag 1 plus actual retained payload; Multisig version 1,
threshold 2, count 8, and every member advance 1, compact tag 1, actual payload and
weight 2. The known member count is admitted before traversal. No fallible key
parse, tree lookup, allocating Ord, clone, sort or scratch allocation occurs.
Malformed typed compact keys preserve exact membership semantics and error
categories; measuring them grants no key validity or authority.

Every domain scans every original owner and member, funding both complete owner
and domain comparisons even when one differs. Every reverse member scans every
original domain with both comparisons funded before combining results. A complete
match boolean is retained until each scan ends. Checked conversion/subtraction
refuses with WorkLimit before unadmitted work, without a validity verdict.

One domain/member with owner geometry A and domain geometry D costs
`12 + 8*(A+D)` over both images without undo. The Single Ed25519 `live.universal`
fixture costs 388 (A=34,D=13); an equal no-op pair costs 584; an inserted pair with
absent preimages costs 294; two otherwise empty maps each retaining an absent undo
row cost 2. Tests bind exact-1/exact and allocation-free results to those formulas.
The named committed descriptor is 1292=`12+8*(34+126)`, for a single Ed25519 owner
and the two existing maximum 63-byte DomainId components. Runtime funding uses
actual borrowed bytes. Wider controllers, multiple rows and quadratic predecessor
scans may defer for a larger admitted local allowance. This changes no controller,
ledger row, codec, gas, execution or validity limit.

Committed capture keeps both original readers through validation, checks both
original publication identities before returning success, corruption or work
refusal, and consumes the same canonical rows with the original State execution
pool. Equal-value publication counts as change. The existing State generation
fence and both identity checks precede any paired-encoding outcome.

Frozen capture accepts only the actual StateBlock with a fully frozen World, two
still-live original images belonging to that State's exact domain and owner-index
storage targets, and equal original modes. It checks both images with the shared
relation and encodes current original Domain rows through the existing paired
encoder using the original State pool. Incomplete, foreign, released and mixed
sources return None. Work/pool/row/payload refusal leaves the same original owner
available for retry; the last paired-snapshot owner refunds its retained capacity.
Later target publication cannot refresh either frozen original. Actual Replace
mode keeps the rewound cut and original modes.

The closed 217-output frozen catalog contains 192 native callbacks and five
completed checked outputs, with 20 explicit adapters still missing. Counts are
scoped encoder coverage only. This group returns no aggregate State success,
State publication, complete root, private-row disclosure or finality authority.
TODO: retain and validate every remaining group, canonical cell, membership
frontier and authenticated history in the sole StatePublication owner before any
complete predecessor coherence or release/finality claim.
