# Current-wire Enrollment certificate relation

`src/certificate.rs` is a standalone research relation for the existing
KAGEMUSHA Enrollment signer certificate. Its P-256 scheme-root public key is
fixed by the verification key. The nonzero scheme identifier is a public
input supplied from the verifier's trusted state/context. The delegated
issuer key remains a witness, so issuer rotation and separately authorized
scheme inputs use the same root verification key.

The verifier must supply the expected scheme; it must not accept an
untrusted proof's choice of scheme as authority. Carrying the scheme in
public inputs avoids an identity cycle through the relation/verification-key
catalogue that participates in the scheme identity. The credential relation
joins its scheme bytes to these authenticated certificate cells.

The public output is the complete 65-byte uncompressed SEC1 issuer key at
instance slots 128–192 and the certificate object digest at slot 193. The
expected scheme occupies slots 194–225 and is constrained to the signed body. The
108-byte body and 64-byte raw signature are private witness inputs. The
statement proves that these public outputs belong to a valid Enrollment
certificate under the fixed context.

The relation follows the existing specification without adding wire fields:

- `LE16(version=1) || scheme_id[32] || role=1 || key[65] || LE64(serial)`.
- `P_bytes(kgwcert1, body)` uses the byte length followed by 31-byte
  little-endian chunks, including the short final chunk. Domain words are
  the little-endian integers of their eight ASCII bytes.
- The fresh RP57 sponge starts at `[2^64, 0, 0]`, absorbs in lanes 1 and 2,
  applies the native padding, and returns lane 1. The synthetic operation
  pool's different lane convention is not used.
- The message's exact canonical 32-byte little-endian encoding feeds
  SHA-256. Byte range checks, recomposition, and an integer comparison with
  the scalar-field modulus prevent the alternate encoding `m + modulus`.
- ECDSA uses the fixed root comb table. The exact raw signature bytes feed
  both the comb's scalar limbs and the object hash; `r` is in `[1,n−1]`
  and `s` is in `[1,floor(n/2)]`.
- The issuer key has SEC1 prefix `04`, coordinates strictly below the P-256
  base-field modulus, and satisfies the curve equation. These are relation
  constraints, including when the scheme root signed an invalid encoding.
- The exported certificate digest is
  `P(kgwocrt1, [m, r_lo128, r_hi128, s_lo128, s_hi128])`.

Byte expressions are materialized into constrained field cells before
copy-argument exports because the uniform compiler relocates range-checked
bytes. Public outputs also have explicit equality rows to private cells.
Neither connection depends on the witness generator alone.

Sources are `specs/kagemusha_wallet_wire_v1.md` §§1 and 3.1, the native
certificate transcript/signing/digest methods in
`crates/iroha_data_model/src/kagemusha/kagemusha_wallet_v1/identity.rs`, and
`fixtures/kagemusha/wallet_v1_vectors.json`. The fixture test pins the payer
issuer certificate's independently published message, raw signature, and
object digest; generated test certificates also exercise issuer rotation.

The focused tests use the experiment's existing warm target directory:

```sh
ZKS_SCHEDULE=lean cargo test --locked --offline --release certificate::tests:: -- --nocapture
```

The tests cover the published fixture, correct-root signatures with an
incorrect role, scheme, version, SEC1 tag, off-curve point or noncanonical
coordinate, byte/message/signature/output tampering, scalar zero/range
rejection, and the high-S twin of a valid signature. Two distinct issuers
and two trusted scheme inputs prove with the same circuit and verification key;
a proof under another fixed root or expected public scheme must fail there.
The builder reports invalid ECDSA inputs before
witness generation; separate witness mutations and semantic-invalid inputs
exercise the constraints themselves.

This module alone authenticates the certificate. The separate
[credential relation](CREDENTIAL.md) now connects issuer-table derivation to
this authenticated key and consumes this exact certificate digest. Neither
module completes recursive Bootstrap or a production monetary proof.
No canonical specification,
production prover, installed carrier, mobile runtime, or live network is
changed by this module. No phone performance or end-to-end QR fit is claimed.

The owner's later future-generation target permits a 12,000-byte full QR
carrier. The current wire still caps lineage proofs at 5,120 bytes. The
existing synthetic recursive pair totals 9,952 bytes, exceeding that cap
and the historical future-generation 8,458-byte proof allowance. A complete
serialized envelope must be measured before asserting that a future design
fits the later full-carrier target.
