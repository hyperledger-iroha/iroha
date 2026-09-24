# BFV eight-party fail-closed decryption boundary, 2026-09-24

The [earlier source audit](bfv-eight-party-verification-prerequisite.md) found
eight RNS modulus limbs but no eight-participant BFV operation. This change
defines the intended distributed operation as **eight-of-eight additive-share
decryption of one registered bounded-noise `R_q` ciphertext**. It does not claim
a threshold or dropout policy. It also does not encode or verify every RNS limb
of the full bootstrap/source relation.

`BfvEightPartyDecryptionStatementV1` freezes the registered parameters, exact
aggregate public key and ciphertext, nonzero session ID, and a fixed eight-member
roster. Each member has a distinct, canonically ordered signing public key and
one fixed 64-coefficient public `b_i` share; the validator checks
`b = sum_i b_i (mod q)`. The signed
contribution binds its member index, session, and domain-separated canonical
statement digest. The authentication validator checks all eight signatures in
order and refuses missing, duplicate, wrong-key, replayed, or malformed input.
Norito is the only wire encoding for the complete statement. The roster and
signed-contribution batch are fixed eight-element arrays, while an explicit
canonical decoder caps the statement frame at 16 KiB, each nested sequence at
64 elements, all sequence elements at 2,048 and cumulative allocation at
64 KiB before checking the exact registered degree. The eight-contribution
decoder has its own 8 KiB frame and 32 KiB allocation limits.
The generic `Hash::new_from_chunks` statement digest is an orchestration
identity signed by participants; it is not a BFV native-STARK commitment or
transcript hash and does not alter or satisfy the six-lane BFV proof-hash
contract.
The test uses compact Ed25519 signing keys. Governing the sole signer algorithm
and matching its key/signature sizes to these decoder ceilings is still an open
interface cut; larger schemes currently fail the bounded decoder rather than
falling back to another wire format.

This public arithmetic and authentication are **not BFV share verification**.
The verifier `verify_bfv_eight_party_decryption_v1` always returns
`EightPartyShareRelationProofUnavailable` after authentication. The signed
payload holds only an opaque contribution commitment. Publishing a raw
`c1*s_i` polynomial could disclose `s_i` when `c1` is invertible, so the test's
raw share arithmetic is kept private to its fixture and is never a production
wire field. The fixture's zero-error key-share construction is likewise only a
diagnostic arithmetic example, not a secure key-generation protocol.

The required construction must govern common `a` generation and independent
secret/error sampling, privately protect each partial decryption, and prove in
zero knowledge that each `s_i` is in the registered secret domain,
`b_i + a*s_i = e_i (mod q)` with bounded `e_i`, and the private contribution is
derived correctly from `c1*s_i`. It must bind the full roster, session,
ciphertext, registered parameters and all full source/RNS limbs; quantify any
share masking against both secret privacy and final BFV noise headroom; verify
each share before deterministic combination; and define durable session replay
and finalized-job authority checks. Production proof size, 512 MiB memory,
16 GiB spool, 64 GiB I/O, and 128-billion work-unit limits still apply.

Tests use eight independently seeded BFV secret keys and eight distinct signing
keys. They check the honest private additive-share identity and exercise missing
and duplicate contributions, cross-session replay, wrong-party signatures,
unsigned late-share tampering, a malicious participant's correctly signed false
commitment, invalid public-key-share coefficients, and canonical Norito
roundtrips. Correctly signed false commitments pass the authentication-only
check and still cannot pass the public verifier. These tests do not complete
eight-party BFV qualification, the full BFV relation, or the audited
parameter/lattice/noise/qROM evidence gate.

Validation: `cargo test -p iroha_crypto --lib eight_party_decryption -- --nocapture`
passed 5/5 on this source. Strict `cargo clippy -p iroha_crypto --lib --features json
-- -D warnings` hit unrelated existing lints in `fastpq_isi`, `iroha_primitives`,
`confidential_spool.rs`, and `merkle_map`. The same scoped Clippy command with
only those five lint classes allowed passed. `require_ram_lfe_bfv_production_qualification_v1`
remains closed.
