# Exact Bootstrap composition research

`src/bootstrap.rs` composes the authenticated Enrollment certificate and approved
future 532-byte credential with the initial monetary state, exact statement and
payment-key receipt. `full::p_boot` now uses this relation. An undersized recursive
layout fails; it never substitutes the former census-only Bootstrap.

This is a research relation for the future wallet generation. Production codecs,
SDKs, deployed wallets and current generation artifacts are unchanged. Send and
Receive in the recursive harness still use their census relations; authenticating
Bootstrap does not make those transitions or the whole payment protocol complete.

The fixed verification key contains the scheme root and provider policy. The
expected scheme and relation identifiers are public inputs, supplied from trusted
verifier context. Issuer rotation and different wallets use the same relation key.
The implementation reuses the private authenticated ports of `certificate::prepare`
and `credential::prepare`; the standalone wrappers retain their previous public
byte outputs. It does not verify a nested standalone authentication proof.

The initial Core has the approved 30 fields, including the depth-16 all-padding
prior-credential root at index 8. It binds scheme, asset, wallet and credential
digest to the authenticated credential; requires Active, zero balance, burned
value, sequence, ordinals and chains; fixes all four indexed roots to the depth-32
empty tree and quota usage to the depth-6 all-padding root; carries only the
credential's permitted controls, response bound and lease; and requires a nonzero
nonce. All remaining initial policy fields are zero. The state commitment is
`P(kgwcore1, core[30] || P(kgwrest1, rest[6]))` using the native RP57 convention.

The 26-field Bootstrap statement binds the same identifiers, credential and state,
wire operation tag 1, zero predecessor/sequence/lineage fields, the signed
credential's enrollment identifier and a nonzero opaque enrollment marker. Its
unused effect fields are zero. The relation identifier is carried, avoiding a
verification-key identity cycle.

The receipt uses the exact 338-byte signing body and raw 64-byte P-256 signature.
It binds scheme, wallet, provider, sequence, operation ID, predecessor, successor
and statement digest; requires a nonzero canonical signed proof digest and nonzero
opaque capsule identifier; and requires payment digest zero. The operation ID is
`P(kgwopid1, [wallet_lo, wallet_hi, 1, enrollment_lo, enrollment_hi])`. The canonical
Poseidon message bytes feed SHA-256 and low-S ECDSA. The receipt object digest uses
`kgworcp1`. As in the native lean Bootstrap relation, the receipt's proof digest
names a separate step proof which Bootstrap does not consume; it is not equated
circularly to the Bootstrap proof being generated. Native custody authenticates
the capsule opening before signing, and the enrollment marker remains in provider
scope.

The receipt's payment comb table is the omitted wallet commitment row. Its four
public coordinate limbs are linked to the exact signed credential table bytes.
Issuer/native intake and wallet E6 must recompute that commitment from the payment
key, as the approved protocol requires. A focused negative uses a correctly
root/issuer/payment-signed, otherwise consistent Bootstrap whose credential names
another wallet's table: arithmetic rows hold, but complete proof verification
rejects the incorrect omitted-row opening. This is distinct from merely changing
an unauthenticated public input.

The compact P public input profile is: head at 4; scheme, relation, asset, wallet,
credential digest and numeric payment-key halves at 16–28; lifecycle, policy epoch,
enabled controls, burned value and both lineage roots at 32–37; signed table
commitment at 56–59; kind selectors at 60–61. Unused ordinary input slots are zero.
`bootstrap_context` makes Send and Receive inherit the immutable context from the
verified wallet predecessor (slot 0), never the payer slot. Their dynamic monetary
lineage semantics remain unfinished. `bootstrap_layout` takes the componentwise
requirements of all three kinds, including fixed-region alignment.

The deterministic fixture uses public test keys only. Wallet identity uses the
native `H("wallet-id", scheme || asset || SEC1 key || enrollment_id)` framing;
the SHA framing is checked against the published empty-scheme vector and the empty
indexed root against the native golden value. Evidence, enrollment identities,
marker and custody proof inputs are test fixtures, not an admission or activation
record. Real callers must supply authorized credential bytes and hardware-produced
receipt bytes to `prepare`.

Validation recorded in `output/kagemusha-100x-20261108/spike/runs/codex-20261112/`:
`bootstrap-composed-tests.log` reports 14 passing Bootstrap, context, layout and
linked-empty-claim tests, including full proofs, rotated wallet keys, correctly
re-signed malformed receipts, high-S, wrong trusted context, state/statement/public
output tampering, and the signed foreign-table negative. The natural Bootstrap
layout sweep records the same 715,951 cells at 128/64/32 rows for 14/15/16 column
bits. These are arithmetic/layout measurements, not phone performance, a complete
serialized Payment measurement, or a 12,000-byte QR fit claim.
The current 126-test whole-suite run also passes these Bootstrap checks, with
the experiment sources unchanged; see `checkpoint-test-validation.json` and
`checkpoint-full-tests.log` in that same evidence directory.

The final eight-hop run passes after rebuilding the Bootstrap-seeded chain at
the common layouts rather than each Receive helper's natural layout. All six
Send/Receive predecessor orderings match their corresponding hop templates in
shape, sizes and matrices, verify for two wallets with distinct certified tables,
and pass both native accumulator checks. P uses `2^19` constraints and 32 rows of
65,536; S uses
`2^18` constraints and 16 rows of 32,768. The determinism checks, partner-swap
negative and predecessor-kind/table tampering gates also pass. The complete
output is `bootstrap-resume-fixed-point-8.log` in the evidence directory.
`bootstrap-resume-fixed-point-validation.json` records exit zero, unchanged
source and binary hashes, the matrix plateau from hop 4 through hop 8, all 32
deterministic rebuild comparisons and all six native chain verdicts.

Every pair round-trips an actual 11,731-byte Norito envelope, with 3,840 primary
and 3,424 secondary proof bytes. Bootstrap's projection is 7,699 bytes after
reconstructing every ordinary input from its expected context. Neither includes
the whole Payment or the separately transported authenticated context. The
recursive chain now starts from the authenticated Bootstrap relation, but its
Send/Receive monetary transitions remain synthetic.
