# Original source to ordered plane storage

`ordered_storage_handoff_v1.rs` joins the existing materialized source to the
existing two-file storage owner. This is local storage linkage. The current
source-algebra lineage has 38 limbs; native40 governed parameters/source replay,
proof-role witness consumption, Q-mask production and composite admission remain
unqualified and their production seals remain uninhabited.

The consuming entry validates the original radix snapshot/record/seal and replay
Evidence, then obtains the actual original source-opening record. A private
borrow carries those three records to the **existing** context encoder. No new
context domain, challenge, digest language, profile or wire field is introduced.
The existing pair reservation precedes both file creation calls and reserves
5,026,665,600 live file bytes plus 10,053,331,200 write/seal I/O bytes. Only a local
pre-I/O `Capacity` refusal retains the whole source for retry; it does not move
any source cursor or retain a partial pair.

The source owns its writer from comparator ordinal zero. Low-digit and delta
producers retain that same source/writer while appending their existing original
commitments; delta contributes no stored planes. Both comparator and signed
prepared owners consume their actual 32 value chunks and original tail directly
into that writer. Each plane uses global slots `33*ordinal + [0,32]`. Their source
cursor advances only after all 33 writes succeed. The previous internal emission
methods cannot complete a source with an attached writer after dropping chunks.
A scalar/ordinal/write/tail failure consumes the prepared source and both files.

Each existing commitment constructor already computes
`C = sum_v(value_v * G[v]) + rho * H` from the exact prepared vector and admits
that point/rho into the original session before the opening can be stored.
Successful writes consume those same value bytes and the original admitted tail;
there is no callback, caller-selected chunk, duplicate inventory or new mask.
After all 9,288 planes and 306,504 slots, the consuming seal requires the completed
original signed inventory, matching original context and complete writer cursor.
Both actual leaves then authenticate before the pair and source move together.
The completed signed phase is consumed once into an immutable stored-plane
replay owner. That owner retains the same original session, inventory and rho
vectors, and exposes no commitment, entropy or mutation API. Each tail checks
its exact original ticket/rho without rehashing the immutable 344-ticket source
prefix. This avoids 3,195,072 repeated source-ticket visits per full replay. The
fixed 16,384-coordinate mapping digest was already cached by binary topology;
source/caller identities are never cached or replaced by a detached flag.
The leaf is an unlinked temporary spool, not crash-reopenable persistent storage;
write/seal success does not claim filesystem crash durability.

Canonical authenticated replay keeps this same pair and original source. A
local pre-I/O read refusal restores the exact pair, source, snapshot identity and
cursor. Every other error or unwind destroys them. At every 33rd slot the reader
compares the authenticated nonzero canonical scalar and point with the original
retained rho and ticket selected by the sole canonical coordinate:

| Logical ordinal | Physical ticket | Original retained rho |
| --- | --- | --- |
| `[0,688)` | `12040 + ordinal` | top `[ordinal]` |
| `[688,7224)` | `18576 + ordinal - 688` | continuation `[ordinal - 688]` |
| `[7224,9288)` | `25112 + ordinal - 7224` | signed `[ordinal - 7224]` |

The delta gap `[12728,18576)` remains in the original inventory. A stored point,
valid scalar or matching ordinal alone cannot replace its admitted ticket/rho.
AEAD plus the retained immutable pair preserves the original written value
bytes; this replay does not re-prove their commitment with a second MSM.
The final local owner retains both source and pair and exposes no source,
snapshot, plaintext, proof-provider or materializer conversion. Any future
Q-mask continuation must consume that fully verified stored owner explicitly;
there is no parallel path back to mutable commitment production.

One canonical replay adds exactly 5,026,665,600 authenticated I/O bytes to the
same pair ledger: write + seal + this replay is 15,079,996,800 bytes. It keeps no
extra scalar vector, point inventory or plaintext hash list. The pair ledger
still does not cover every earlier source/qPCS consumer, allocator/control
storage, kernel metadata/cache, or hardware RSS. The immutable 512 MiB / 16 GiB /
64 GiB ceilings and all qualification gates are unchanged. The actual statement
3/5/8 role replays must later consume this same source/pair under their original
permits; completing this local pass does not satisfy those proof obligations.

The added tests exercise real tiny two-file storage, exact value/tail bytes,
original source retention, emitted-but-unwritten rejection, malformed/reordered
or replayed storage, original ticket/rho mutations, missing source evidence,
capacity refusal/release/retry and zeroizing owner disposal. Their fixture
sources are explicitly unqualified. Existing canonical crypto tests separately
cover ciphertext/tag partial-write failure, truncation, record substitution and
unwind. Neither these controls nor static source review establish a complete
production source run, native40 security evidence or full-size resource bounds.
