# zk-X509 all-P-256 opening bound, 2026-09-24

This is a byte-geometry screen of the current `optimizations` source, not a
replacement proof, a measured maximum fixture, or release qualification. The
complete strict-DER/RFC 5280 certificate, signed-CRL, CA-membership, projection,
SHA-256, five-signature P-256, byte-memory, and holder-ownership relation remains
required. The fixed production X5S1 ceiling is 9,437,184 bytes.

The MAIN aggregate currently opens 5,623 base-field trace columns at 136
positions and two rows per position, using eight bytes per value. Its 12,235,648
direct-opening bytes are part of the exact 19,156,074-byte whole-X5S1 maximum.
The P-256 source assertions fix 596 base plus 204 auxiliary columns at log 5,
430 plus 375 at log 16, and 1,395 plus 1,000 at log 19. Thus **all P-256 groups
own 4,000 columns and 8,704,000 sampled bytes**, leaving 1,623 current direct
columns. See `crates/iroha_core/src/privacy_engines/zk_x509/stark.rs` at the
`P256_MAIN_LOG5_*`, `P256_MAIN_LOG16_*`, and `P256_MAIN_LOG19_*` assertions and
`crates/iroha_core/src/privacy_engines/zk_x509/profile.rs` at the shared MAIN
opening and X5S1 maximum constants.

Even the impossible experiment of deleting *every* P-256 direct trace opening
without replacing its proof leaves `19,156,074 - 8,704,000 = 10,452,074` bytes:
**1,014,890 bytes above the full cap**. With the other current bytes fixed,
another `ceil(1,014,890 / 2,176) = 467` non-P-256 direct-opening columns must
also be replaced. Any actual P-256 replacement argument adds bytes and raises
that required saving. This strengthens the earlier log-19-only screen, which
showed that removing all 2,395 log-19 P-256 columns still left 13,944,554 bytes.
These are necessary inequalities for the present current/next sampled opening
format, not lower bounds for every possible redesigned argument.

A candidate that serializes the five signature computations across shared
columns must specify the exact witness-independent segment schedule, role and
instance tags, start/end and padding constraints, and equality proofs connecting
all signature inputs and outputs to the DER/RFC, SHA, CRL, projection, byte-memory
and wallet-ownership terminals. It also has to replace enough non-P-256 opening
cost to clear the additional 1,014,890-byte deficit, account for every new bus,
lookup, masking, DEEP and FRI byte, and retain the joint transcript chronology
that fixes all six MAIN base roots and the CA base root before shared X5B1
challenges. A recursive candidate must instead prove every child acceptance and
cross-family terminal link in a verifier-fixed outer relation and bound the
complete outer X5S1 wire. Neither construction nor its composed 128-bit
soundness/privacy argument and measured full-size fixture exists in this source.

`python3 scripts/check_zk_x509_proof_geometry.py` now reads the three Rust
P-256 width assertions and reports the all-group bound while retaining
`production_qualified: false`. Five source-drift tests passed with
`python3 -m pytest -q scripts/tests/check_zk_x509_proof_geometry_test.py`.
This screen does not change a proof layout, decoder, cap, or activation gate.
