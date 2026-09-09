# FASTPQ ordinary source statements

Updated 2026-09-07. This is the source-coupled contract for the pending
`optimizations` integration candidate. The candidate lives under ignored
`target/fastpq-optimizations-integration/port-candidate` in
`/Users/takemiyamakoto/dev/iroha`; it has not been applied or compiled on that
branch. Production compact admission remains disabled. The
[readiness ledger](fastpq_production_readiness.md) separates retained reference
validation from the required current-branch gate.

## Transaction wires and execution sources

`PublicIO.tx_set_hash` retains the branch's authoritative ordered canonical
transaction-wire commitment. Both block execution paths compute it through
`axt_ordered_transaction_set_digest_v1` from the ordered external and time
entrypoints. It is cached before source-inventory finalization. An execution-call
identity list must never replace that commitment. Missing or zero cached hashes
fail before transcript digest finalization, and failure is latched.

Execution sources have a separate complete canonical projection:

1. External entrypoints in block order, using actual execution-call identities
   and validated routes. Sealed reveals retain their inner signed call identity.
2. Time invocations in invocation order, including failed invocations, using
   their distinct runtime call hashes and an explicitly absent lane.
3. Remaining applied transcript sources in ascending hash order, including
   internally derived calls and typed native protocol purposes.

This projection includes external/time entries without transfers. Native work
without a recorded transfer is not an additional proof source. Projection order
is independent of physical fragment scheduling.

The nominal model type `FastpqSourceExecutionEntryV1` contains the call/purpose
hash, execution kind, route and dataspace. `FastpqSourceExecutionKindV1`
distinguishes `ExecutionCall` from `ProtocolPurpose`; an execution call alone
asserts no external signature. `FastpqSourceRouteV1` distinguishes `Unrouted`
from a lane ID and its full incarnation `Hash`. No lane or hash sentinel is used.

`fastpq_source_execution_entries_digest_v1` commits the complete ordered entry
projection, including entries without statements. Its preimage is the domain
`iroha:fastpq:source-execution-entries:v1\0`, a little-endian `u32` entry count,
then each entry's little-endian `u32` canonical-frame length and complete nominal
Norito frame. The entry cap is checked before traversal. Streaming hashing uses
bounded per-entry scratch and does not allocate a concatenated archive. The
2,048-byte structural frame bound is not a production resource policy.
Completeness and identity uniqueness require the execution owner.

## Manifest and bounded opening

A statement leaf commits source network/height, statement and entry positions,
original transcript index and complete per-entry count, all four source-entry
fields, and the canonical path-free statement digest. Multiple deltas in one
original transcript remain one occurrence. All leaves for one entry have matching
source fields and consecutive transcript indices covering its exact count.

The manifest contains source network/height, executed-entry count,
`source_entries_digest`, statement count and application-Merkle root. The builder
receives the complete entry slice, enforces separate entry/statement caps and
checks every leaf against the entry at its advertised index. Empty statement
sets use a fixed empty root while retaining the complete source-entry digest.
A zero-entry manifest must carry the exact empty-entry digest.

The manifest's canonical frame is the ordinary-write value at the reserved fixed
`FASTPQ_ORDINARY_SOURCE_STATEMENTS_WITNESS_KEY_V1` key (`0xD7`). Membership uses a
count-aware application-Merkle path, including ragged trees. One source opening
also has exactly 256 ordinary-write SMT siblings. Verification checks the exact
expected leaf and ordinary root; it does not authenticate the root's finality.

The unqualified first-release manifest layout includes the entry digest. Earlier
prototype layouts must reject during canonical decoding even when they reused
the same nominal schema name. No compatibility fallback or compact production
profile is enabled by this layout.

Core's shared opening/archive preparation takes the complete expected entry
slice immediately after the source context. It checks the entry cap before
scanning witness writes, rejects malformed or duplicate members of the entire
reserved `0xD7` family, decodes the manifest under cumulative limits and rebuilds
it from exact entries and leaves before allocating the ordinary tree. Other
ordinary keys retain the existing last-write-wins rule. An opening checks its
requested position before copying ordinary writes. The returned root is a
computed claim requiring independent authority.

## Complete leaf archive

`FastpqOrdinarySourceStatementArchiveV1` contains an explicit version, manifest,
complete ordered leaf vector and one fixed `[Hash; 256]` manifest path. Empty
archives preserve their manifest and path. Statement preimages and private
transfer paths are excluded.

Model archive verification and decoding receive the complete expected entry
slice separately, alongside source context and expected ordinary root. They
recompute the entry digest and complete manifest. The expected entries must
come from an independently authoritative owner; they cannot be reconstructed
from leaves because non-transfer entries have no leaf.

Decoding rejects excessive wire bytes and expected-entry count before body work,
and narrows the sole variable sequence to the statement cap before allocation.
Field, cumulative element/allocation and nesting limits retain stricter enclosing
budgets. Fixed arrays do not consume the variable-sequence cap, so an empty leaf
archive can use a zero leaf cap. Complete-archive consistency checking traverses
entries and leaves; it is separate from succinct proof verification.

The Core archive constructor borrows the witness and expected entries, validates
the complete owned leaf vector, then moves that allocation into the result. It
computes one shared manifest path. Successful construction establishes
consistency with supplied facts, without granting source authority or spending
permission.

## Execution ownership and local derivation

`StateBlock` freezes network, height and active lane incarnations before staging
and lifecycle effects. Transactions capture each transfer's runtime identity,
kind, explicit route, dataspace and contributing fragment locally. `apply()`
publishes captures and capture errors together; dropping a transaction discards
both. Applied conflicts invalidate the complete source map. Changing an active
call hash after recording does not re-key an earlier occurrence.

Inventory finalization reconciles exact capture keys with the still-owned
transcript accumulator. It rejects empty/misidentified bundles, duplicate
identities, wrong routes/kinds and source-height conflicts. It completes missing
valid singleton digests only after validating supplied digests, then seals the
original finalized public occurrences. The owned inventory retains the canonical
wire hash unchanged and publishes the complete per-source dataspace map.
Neither success nor failure can be replaced by retrying with a smaller archive.

`derive_fastpq_ordinary_source_manifest_v1` takes source context, complete entries,
slot, permission root, explicit nonzero transaction-wire hash, transcript map and
construction limits. Every map key must match one entry. The owner wrapper also
requires its exact transcript-key set and public-content seal. Public quantities,
identities, optional digests and occurrence grouping must remain unchanged.

Six limits cover entries, cumulative transcripts, cumulative deltas, canonical
private-input bytes, largest public-statement frame and cumulative statement
bytes. Shared public preparation checks all six before private-tree construction.
It measures real canonical quantity frames and identities; only fixed-width
context placeholders are used for byte measurement. Missing singleton digests
reject rather than being enlarged after accounting. Final per-statement encoding
still enforces individual and cumulative byte caps. Repeated public preparation
costs CPU time that requires measurement on the final candidate.

Full-domain quantity preparation supports the ledger's nonnegative 512-bit
mantissa and scales 0 through 28 using 19 little-endian `u32` limbs. Canonical
value frames bind scale and all limbs, including zero padding. Narrow and
full-domain factories reject each other's value encodings. Checked arithmetic,
exact occurrence coverage and repeated-key balance continuity precede private
SMT materialization. Tree limits bound updates, unique keys, occupied nodes and
path allocation. Statements are built and dropped one at a time.

Statement roots describe touched balances for one whole recorded operation. They
exclude the ordinary-write tree that contains the manifest. Legal mint/burn work
between transfer operations does not establish transfer-only continuity across
those operations. Whole-ledger balances, supply and authorization require their
own authenticated execution relations.

Final witness capture, extraction and commit recheck the owned inventory,
canonical wire cache, dataspace cache and exact finalized public transcript
contents. Later applied transfers or changed contents invalidate publication.
The scoped witness recorder clears/deactivates its owned overlay on errors;
repeated capture checks retained contents without reading another block's
recorder. Authenticated replay does not invent local source ownership.

The test-only qualification helper `prepare_owned_fastpq_d7_capture` retains the
inventory allocation, exact header milliseconds, saturating nanosecond slot,
permission root, manifest and limits.
Subsequent checks reject context drift, including timestamps that saturate to the
same slot. The helper does not insert the D7 write or confer finality. Permission
scans and repeated context checks require their own resource accounting; account
membership and direct permissions are not proved by the role-table root.

## Activation requirements

TODO: implement execution-owned resource reservations and per-entry rollback,
source-aware proposal packing, and bounded mandatory trigger/native outcomes
before inserting D7 into the final ordinary commitment. A final-capture limit
alone can repeatedly invalidate a selected block. Truncation or omission of an
applied source is invalid. Candidate measurements are not shipping defaults.

TODO: bind prepared D7 contents to checked capture, authoritative source finality,
exact AXT spending/replay protection and durable archive publication/retention.
The separate Kura content store uses current ownership locks and composite
capacity reservations but does not provide these source-owner lifecycle hooks.

Current-branch compilation, all affected tests, fresh complete artifacts,
independent cryptographic qualification, release-hardware CPU/Metal/CUDA parity
and a same-source four-validator recovery/adversarial corridor remain required.
The earlier 544-test/two-artifact reference gate is not evidence that this
integration candidate passed those gates or is production ready.
