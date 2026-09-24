# zk-X509 F07 P-256/SHA recursive size contract, 2026-09-24

This is a read-only sizing and design screen on `optimizations` at HEAD
`b45ec0457eb0664e46c2cdfd8a4b619caefd02b9`. It proposes a
specific replacement boundary; it is not a new proof, a measured bound for that
replacement, or release qualification. No Rust, Cargo, proof format, or
activation gate was changed.

## Current lower bound and cap

The canonical X5S1 maximum is 19,156,074 bytes against the fixed 9 MiB
(9,437,184-byte) ceiling, a 9,718,890-byte excess. The 92-byte outer frame and
2,696,222-byte complete CA section leave exactly 6,740,870 bytes for X5M1.
These are source-pinned in `zk_x509/{profile.rs,credential_stark.rs,accumulator_stark.rs}`;
`stark.rs::validate_zk_x509_main_proof_budget_v1` rejects the current complete
MAIN before proof construction. The current wire opens all 5,623 MAIN trace
columns at 136 positions and current/next rows, eight bytes per field:
`5,623 × 136 × 2 × 8 = 12,235,648` bytes. This is an exact unavoidable payload
**for the present direct-opening layout**, already 2,798,464 bytes beyond the
entire X5S1 ceiling. It is not a lower bound on a different argument. The
canonical aggregate decoder also includes two Fp4 DEEP openings per column,
`2 × 32 = 64` bytes, so removing a current direct-opening column from this
layout saves 2,240 bytes before any replacement cost. See
`aggregate_stark.rs::{encoded_non_frontier_bytes_v1,exact_deep_opening_bytes_v1}`.
Even granting this DEEP saving and zero replacement cost, at least
`ceil(9,718,890 / 2,240) = 4,339` present columns must be replaced if all
other bytes stay fixed. The prior 4,467-column screen holds DEEP bytes fixed
as well; these are two different accounting assumptions.

## One falsifiable replacement boundary

Keep the strict-DER, RFC 5280, projection, global byte-memory MAIN relations
and the complete compact-CA proof. Replace **all** direct P-256 and SHA trace
openings with one recursive receipt inside a re-profiled X5M1. Its private
inputs are the full child proofs, not trusted native-validation results. The
receipt must verify every child proof under verifier-fixed profile digests and
export proof-derived terminal claims. The top-level verifier must compare those
claims with the residual MAIN and CA claims, including DER/RFC-to-SHA input and
digest products, P-256 input/output and value/bit/arithmetic-copy products,
byte-memory links, and the root-SPKI/CA membership channel. A new canonical
pre-auxiliary transcript must bind **all** residual MAIN, child, and CA base
roots before deriving the shared challenge families; each auxiliary commitment
must use that same derived state. The present X5B1 schedule derives 272 fields
only after six MAIN and one CA base root
(`zk_x509/credential_pre_aux.rs`), so independently sampled child challenges
would not establish the joined relation. The child proofs can be private to the
receipt, but the receipt's public root/terminal transcript and the outer
statement binding must be verifier-derived and uniquely encoded.

This boundary preserves the admitted maximum shape: two or three complete
leaf-first certificates of at most 4,096 DER bytes each; one complete signed
base CRL of at most 4,096 bytes and 64 active entries with issuer-scoped
nonrevocation against **every** entry; the depth-12 governed CA path; all 29
SHA calls; all five P-256 signatures (certificate chain, CRL, and low-`s`
wallet ownership); up to four salted attribute disclosures; and the existing
strict DER, closed RFC 5280, public projection, and governance checks. See
`zk_x509/{codec.rs,profile.rs,relation.rs,main_assembly.rs,sha_call_bus_stark.rs}`.
No certificate, CRL, optional-chain, or signature case is dropped to obtain
the byte saving.

| Removed current MAIN direct-opening family | Columns | Native rows per column | Current native field cells |
| --- | ---: | ---: | ---: |
| P-256 log 5 | 800 | 32 | 25,600 |
| P-256 log 16 | 805 | 65,536 | 52,756,480 |
| P-256 log 19 | 2,395 | 524,288 | 1,255,669,760 |
| Four SHA log-19 segments, each 89 base + 78 auxiliary | 668 | 524,288 | 350,224,384 |
| **Total** | **4,668** | | **1,658,676,224** |

The P-256 widths are asserted in `zk_x509/stark.rs` at the
`P256_MAIN_LOG{5,16,19}_*` constants; the four 89+78 SHA segments and 29-call
schedule are in `zk_x509/sha_call_bus_stark.rs`. Removing those 4,668 current
columns leaves 955. Their sampled-row saving is
`4,668 × 2,176 = 10,157,568` bytes; the current DEEP saving is
`4,668 × 64 = 298,752` bytes. Holding every other current byte fixed gives
`19,156,074 - 10,456,320 = 8,699,754` bytes before the receipt, hence an
**accounting allowance of 737,430 bytes** for the receipt *and all new roots,
claims, transcript data, padding, and verifier framing* under this fixed-cost
model. The remaining X5M1 portion
would be 6,003,440 bytes against its 6,740,870-byte section cap. If DEEP
openings were retained, the budget would be only 438,678 bytes. These are
design targets for this proposed boundary, not a projected receipt size or a
bound if the new profile changes other wire costs. By comparison, replacing
all 4,000 P-256 columns while crediting their
DEEP savings still leaves 10,196,074 bytes, 758,890 over the cap; at least
`ceil(758,890 / 2,240) = 339` more present columns must disappear even with
zero replacement overhead. The four SHA segments supply 668 candidates.

Reusing the current child traces already entails 1,658,676,224 native field
cells (13,269,409,792 raw bytes if all were resident) and 4,668 columns over
the log-22 common LDE, or 19,579,011,072 field evaluation slots. Streaming
avoids simultaneous retention, but recursion adds verifier work; no 300-second
or 12-GiB claim follows from this screen. An implementation must derive an
exact whole-X5S1 maximum at or below 9,437,184 bytes, a composed 128-bit
soundness and zero-knowledge argument for the new receipt and shared
transcript, full maximum-shape and mutation evidence, and release-machine
resource measurements before changing the fail-closed gate. The current
`python3 scripts/check_zk_x509_proof_geometry.py` screen reports
`production_qualified: false`; it checks the existing layout, not this
candidate.
