# Allocation-free State scans

Checkpoint128 removes read-time heap allocation from the original MV/Concread
engine used by production State. Work remains in
`/Users/takemiyamakoto/dev/iroha`, branch `optimizations`.

## Original borrowed traversal

`StorageReadOnly` now returns concrete associated full/range iterator types for
views, blocks and transactions. The boxed trait-object adapters are removed.
Full iteration exposes its original exact remaining length and both directions;
range traversal still accepts borrowed unsized keys with exact inclusive and
exclusive bounds. References remain bounded by their original view or writer. Account-role queries
return the same borrowed range through an opaque iterator with an explicit
State lifetime; lookup account arguments are not retained. The State regression
checks this across views, blocks and transactions in both directions.

Both B+tree paths now use inline arrays of original node/index positions. A
valid balanced tree has at least two children per branch and a nonempty leaf
under every non-root path; its representable `usize` entry count therefore
bounds branch depth by `usize::BITS`. One extra frame holds the leaf. This is the
existing teardown bound, not an estimated user-configurable capacity. Reads
allocate no path storage, clone no keys or values and create no mutable node
references. On 64-bit hosts the two paths use about 2 KiB inline; this is a
bounded stack cost and is checked with ordinary default-stack consumers.

Allocator-observed regressions cover empty/multilevel trees, forward/reverse
and alternating traversal, removed entries, original old readers, nested
checkpoint apply/abort, MV transaction views and rollback, predecessor merging,
and borrowed `str` bounds. The prepaid test fills the original memory pool
completely before scanning, forbids ordinary payload Clone, and checks that
credits and payload-copy counters remain unchanged.

## Consumer migration

The full consumer build also exposed stale developer-tool bindings. `xtask`
must explicitly select Torii's test fixtures under its existing `dev-tools`
feature. Lane configuration aliases no longer identify Kura objects: canonical
storage is chain-owned, while lane artifacts use exact instance identities.
The maintenance command reports
declared metadata and observed physical entries separately, without deriving
active/retired authority from aliases or offering config-only compaction. Its
CLI and filesystem regressions cover read-only behavior and reject compaction
and file-output options. The command emits JSON only to stdout, so a report path
cannot truncate Kura data; an actual configured run preserves every fixture file
and byte. No alias-based path compatibility helper is restored.

## Qualification

Current candidate receipts belong to `dist/sumeragi-main-work/generation128-final/`
and `generation128-vendor/`; `validation127.json` remains the prior candidate's
receipt. The acceptance selection retains all 217 prior MV controls and adds
three allocator regressions, all 171 prior Core runtime controls plus a State
role-range lifetime regression, complete Core test compilation, the six production
MV consumer test builds, the gated `xtask` consumer check and test compilation,
and its seven exact CLI/filesystem runtime regressions; the vendor feature
matrix, native default/skinny sanitizers, strict MV Clippy, public ownership
compile controls and the canonical source/ledger gates. Receipts must join exact
before/after/current source keysets and original compiled artifacts.

## Remaining production work

Live scalar Validate still drops its prepared execution and Apply executes again.
The retained service's descriptor reservation does not admit World execution;
`prepare_journals` runs after that work. World capture/publication still names
Untracked map modes. A real charged field migration needs mode propagation,
concrete payload custody, original pool/configuration and local capacity-refusal
propagation through actual execution callers. The insertion-only fixed-size AXT
handle counter is a candidate, but its current caller also stages maps/vectors
and updates policies; changing its field alias alone cannot close admission.

The next bounded production slice is the actual `BlockHashes` journal. Canonical
runtime acquisition opens it before World and start effects; carrier metadata
checks the exact next height and appends one fixed-size hash. Admit one original
visible buffer before copying its predecessor, represent pending entries by its
suffix, and install that same buffer at commit instead of making a second chain
copy. Initial Strict loading and publication identities also need original
allocation custody. A distinct configured resident pool must not borrow the
meaning of `max_wsv_memory_bytes`, which controls hot-tier storage budgeting.
Capacity refusal must cross State and ValidBlock as a local retry with the
original release wake, never become a semantic validation rejection or durable
marker. Exact-limit, release/retry, abort, replacement, capture and publication
tests must exercise this production owner. This slice alone cannot enable the
fully admitted retained Validate-to-Apply service.

Native runtime/identity allocations, complete model payload policies, closed
removal/mutation, bounded execution work, aggregate Validate-to-Apply integration
and unchanged four/seven-validator qualification remain open. No L1–L6 completion
or release claim follows from this scan change.
