# Authenticated external lookup for canonical map nodes

The existing `iroha_crypto::MerkleMap` now exposes a fixed-size root envelope
and immutable node descriptors. A caller can retain a trusted version without
retaining its resident `Arc` graph and authenticate each externally loaded node
against that root. Missing nodes, loader failures, wrong content and invalid
paths remain distinct local errors; none becomes transaction non-membership.

The lookup checks the independently supplied root before I/O, authenticates
node content, and validates compressed prefixes, strictly increasing split
bits and the original parent side. Its iterative kernel retains one parent
and one fixed-size node, with at most 257 node reads and no heap-owned search
path or history collection. The loader remains responsible for its own I/O
and allocation admission. Cold export streams child nodes before parents and
stops at the original visitor error. The original root/leaf/branch hash kernels
are shared; fixed vectors and insertion-order/replacement controls remain
unchanged. No new hashing algorithm, wire format, disk layout or compatibility
path is introduced.

Eight new tests compare external lookup with real persistent map versions,
retain old roots after resident-map destruction, distinguish prefix/leaf absence
from missing/corrupt/I/O failures, reject root metadata and malformed parent
paths, cover every valid split bit on the default thread stack, and retain
exact visitor refusal. The six prior Merkle-map controls remain selected.

Qualification: the Crypto test build passes without warnings, and all 14
Merkle-map tests pass on unchanged captured inputs. Downstream Core library
compilation also passes; it retains one warning for two unchanged, unused State
cache helpers. The full Crypto library run completes with 1,418 passes, zero
failures and three existing ignored fixture-generation controls. Every one of
the 1,421 roster entries is accounted for; the focused and full runs use the
same retained executable and 7,552 unchanged captured Rust inputs. Formatting,
codec-retirement, archive and whitespace checks also pass. Source manifests,
commands, logs and independent receipts are under
`dist/sumeragi-main-work/generation160-crypto/`.

This is a required lookup primitive, not the complete durable State membership
owner. External path updates, funded node/read retirement, exact original State
root custody, authenticated restore and incremental complete-State checkpoint
construction remain required before production history eviction. Existing
production Apply still materializes full-history checkpoints. Retained
Validate-to-Apply integration and unchanged four/seven-validator fault/restart/
final-transaction qualification remain open; L1–L6 remain active.

The preceding [indexed publication repair](native-indexed-publication-recovery.md)
has its independent generation159 receipt. It verifies 1,004 Core, 21 Torii,
165 frozen formal controls and a fresh 12,305-input canonical gate. A subsequent
external merge changed nine blanket-copied SoraFS Python files; the explicit
formal-scope join preserves their exact delta and 1,083 unchanged inspected
formal/test sources. It does not qualify SoraFS changes or claim the complete
old frozen tree still matches. The generation160 Crypto changes are a separate
candidate and do not retroactively alter that prior qualification.
