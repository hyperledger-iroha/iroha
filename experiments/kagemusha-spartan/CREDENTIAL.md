# Approved future-generation credential authentication

This research module targets the approved 532-byte credential transcript in
`specs/kagemusha_wallet_wire_v1.md` §3.1. The existing native
`KagemushaWalletCredentialBodyV1::transcript()` and published wallet fixture
still use the previous 468-byte body. They do not contain the 64-byte payment
key-table commitment. They remain untouched; a 532-byte research vector is
derived and signed separately. A native 468-byte credential is not accepted
as an equivalent input to this relation.

The combined public statement retains the certificate outputs and trusted
scheme at slots 128–225. It exposes the payment key bytes at 256–320, signed
table-commitment bytes at 321–384, and credential object digest at 385. The
expected scheme is supplied by the verifier, not selected as authority by
the proof. Canonical point checks and explicit equality rows bind these
outputs to the exact signed body.

The byte offsets, including the new field, are:

| Field | Start | Length |
|---|---:|---:|
| version, little-endian | 0 | 2 |
| scheme | 2 | 32 |
| asset digest | 34 | 32 |
| wallet identifier | 66 | 32 |
| account digest | 98 | 32 |
| payment key, SEC1 | 130 | 65 |
| payment key-table commitment, Eq affine x/y little-endian | 195 | 64 |
| provider contract | 259 | 32 |
| evidence kind | 291 | 1 |
| enrollment evidence | 292 | 56 |
| fresh evidence | 348 | 56 |
| app policy | 404 | 32 |
| regulatory policy | 436 | 12 |
| enrollment identifier | 448 | 32 |
| issued time, little-endian | 480 | 8 |
| renewal sequence, little-endian | 488 | 4 |
| lease expiry, little-endian | 492 | 8 |
| Enrollment certificate digest, canonical Fp little-endian | 500 | 32 |

The signing message is `P_bytes(kgwcred1, body)`: length 532 plus eighteen
31-byte chunks (the last contains five bytes), hence 19 input elements and
11 RP57 permutations with the domain/arity prefix and native padding. Its
canonical 32-byte little-endian encoding is SHA-256 hashed for low-S ECDSA
under the authenticated Enrollment issuer. The object digest is
`P(kgwocrd1, [m, r_lo128, r_hi128, s_lo128, s_hi128])`.

The certificate relation and credential signature are composed into one
witness under the same fixed root and trusted public scheme context. The credential's final
32 bytes must equal the authenticated certificate digest. Its signing key
comes from that certificate's exact SEC1 coordinates. The issuer table is
derived from those coordinates and every row is bound to the distinct
witness table consumed by the credential comb verifier. This is issuer
rotation under a fixed root verification key; the issuer key is not fixed
in that key.

The payment key must be canonical and on P-256. The signed key-table
commitment must be a canonical, finite Eq affine point. Authentication of
that signed commitment does not prove that it is the commitment of the
payment key's derived table. The approved protocol assigns recomputation
to the issuer before signing/delivery and to the wallet at E6 intake. Those
future native implementations and binding this authenticated commitment to
the monetary proof's wallet row remain separate obligations; this module
does not introduce a requirement for in-circuit payment-table derivation.

`src/credential_policy.rs` is always composed into the relation. It enforces
the self-contained native evidence and regulatory rules on these exact
signed bytes: a valid evidence kind, nonzero evidence digests, defined and
platform-compatible fact masks, required enrollment facts, zero Apple patch
levels, identical enrollment/fresh records at renewal zero, and nondecreasing
evidence time thereafter. Only the two defined regulatory controls are
allowed; the response bound is nonzero exactly when a time-dependent control
is permitted, and lease expiry is nonzero exactly when lease control is
permitted. Even a correct issuer signature cannot waive these rules.

Original KeyMint/App Attest evidence and online issuer admission remain
outside this proof. So do predecessor credential continuity and sequence
increment in RefreshCredential, wallet/enrollment/account SHA identity
derivations, runtime lease/time-anchor decisions, payment-table derivation,
activation and monetary state transitions. The authenticated digest lets
later relations consume these exact claims; it does not itself admit a wallet.

The research fixture starts with the published payer credential's 468
transcript bytes, inserts the payment key's newly computed 64-byte table
commitment at offset 195, and retains the existing root-signed issuer
certificate. The new signing message and object digest require a new
signature under the fixture's public test issuer key. The original native
message/signature/digest are retained as the old-layout reference; they are
not assertions about the 532-byte transcript.

The exact retained inputs are in `fixtures/credential-source-vectors.txt`;
the derived body, table commitment, message, low-S signature and object digest
are pinned in `fixtures/credential-532.txt`. The fixture test recomputes all
five derived values and checks the field offsets above.

The expected scheme is a public input supplied by the verifier's trusted
state/context. Certificate bytes are constrained to that input, and the
credential's scheme bytes are constrained equal to the authenticated
certificate. Only the root/provider anchors remain fixed in the verification
key. This avoids a cycle through the relation/verification-key catalogue
that participates in scheme identity. The rotation proof test also varies
the scheme under the same root/provider verification key and rejects an
incorrect expected public scheme.

The issuer's table coordinates are constrained representatives of the
authenticated P-256 points, with 88/88/80-bit limbs. The existing table
derivation works modulo the P-256 field and does not independently normalize
every emitted coordinate below that modulus. This preserves issuer-key
authority. If a future relation proves payment-table commitment derivation,
it must enforce canonical emitted x/y coordinates before claiming equality
with the native unblinded table commitment. Internal slopes need not be canonical.

No production codec, canonical wire rule, installed carrier, mobile runtime,
live network, or recursive monetary proof is changed here. Certificate and
credential authentication alone do not complete Bootstrap or prove that a
full payment fits a QR carrier.

The focused complete-proof regression accepts two issuer keys and two trusted
scheme inputs against one root/provider verification key, and refuses changed
public scheme, issuer, certificate digest, credential digest or table bytes.
Each measured standalone authentication proof is 3,552 bytes at `2^19`
constraints, 64 rows and 16,384 columns. This is neither a recursive monetary
proof pair nor a complete payment envelope, and is not a QR-fit result.
