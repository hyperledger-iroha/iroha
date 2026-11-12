# Path 2 experiment construction

This document describes the executable in `experiments/kagemusha-spartan`, not
an installed wallet relation. Its purpose is to make the current construction
reproducible and to identify what a production implementation must still prove.
The production wallet continues to use its existing proof backend and credential
format. No parameter, generation or ledger change follows from this document.

## Public commitment generators

`ipa::GENERATOR_DOMAIN` is the UTF-8 string
`kagemusha-zks-spike/gens`. For either Pasta curve, define

```text
point_C(i) = C::hash_to_curve(GENERATOR_DOMAIN, LE64(i))
Gens_C(N).g[i] = point_C(i), 0 <= i < N
Gens_C(N).h = point_C(N)
Gens_C(N).u = point_C(N + 1)
```

`C::hash_to_curve` is the canonical `iroha_pasta` implementation in
`crates/iroha_pasta/src/curve/hash_to_curve.rs`: BLAKE2b-512 XMD, two field
elements, simplified SWU on the isogenous curve, addition there, and the
3-isogeny to the selected Pasta curve. Its domain separation tag appends
`-`, the curve ID, and `_XMD:BLAKE2b_SSWU_RO_` to the prefix. Pallas and Vesta
therefore have different bases even for the same index.

The capacity `N` is part of the generator-set identity. A prefix retains its
parent's `h` and `u`; constructing a new set at the shorter capacity does not.
The generator tests pin encoded public points and this distinction. The
canonical Pasta crate separately checks its hash-to-curve implementation
against its reference implementation.

The payment-key table commitment uses the first 3,584 Vesta `g` bases and no
blinding. Table row `r = 256*t + e` contains seven field elements: the row
index, three x-coordinate limbs and three y-coordinate limbs. The limb widths
are 88, 88 and 80 bits. Its column-major commitment is

```text
C_table = sum(cell[c][r] * g[512*c + r]), 0 <= c < 7, 0 <= r < 512.
```

Its public encoding is two canonical little-endian base-field coordinates
`x || y`, 64 bytes. The circuit represents them as four 128-bit limbs,
`x_low, x_high, y_low, y_high`. These are public parameters and public key
tables, not wallet private-key material.

These rules pin the experiment's derivation. A shipping relation must bind its
generator rule, capacities, circuit matrices and verification keys through its
authenticated artifact identity. This document does not designate the research
domain as a production parameter set.

## Proof pair and chain acceptance

Each operation has a P circuit over Fp, committed on Vesta, and an S circuit
over Fq, committed on Pallas. Both use uniform arrays, copy and lookup arguments,
and deferred matrix claims. Send, Receive and Bootstrap use common layouts;
their matrices and fixed-row commitments remain operation-specific.

Two Boolean public kind bits select Receive `(0,0)`, Send `(1,0)` or Bootstrap
`(0,1)`. `(1,1)` is not an operation. It supplies the zero corner of the
combined matrix polynomial for Bootstrap's empty accumulator. Bootstrap has no
predecessor proof. Send and Receive must verify their predecessor; Receive also
verifies the incoming payer proof.

The pair's link phase follows the first commitment phase and precedes the second.
It binds the shared values inside both proven circuits. Public link words are
at P io positions 124–127. The experiment derives its mask from shared values
for reproducibility; a real prover needs fresh custody-grade randomness. The
same requirement applies to `SpikeRng`, which is deliberately deterministic.

Acceptance requires both linked proofs **and** both final native accumulator
checks. A successful Spartan verifier call by itself does not settle the
deferred generator or matrix claims. The template-building hops intentionally
start from toy proofs and can contain false accumulators. They are compiler
construction inputs, never acceptable monetary evidence. The final chain tests
start from Bootstrap and require every accumulator to hold.

The recursive research circuits expose the signed statement sponge's successor
digest at P io position 4. Their statement absorbs the operation kind and the
authenticated head of predecessor proof 0. For Receive, proof 1 is the incoming
payer and cannot supply the wallet's predecessor. Bootstrap constrains its
predecessor to zero. The successor digest is constrained through the sponge
output, not accepted as a caller-selected public label. This provides a
testable recursive linkage boundary; the signed statement is still synthetic
and is not the canonical monetary `CoreHead`.

## Certified table row

The payment key's witness table occupies the first fixed row, before the
relation's ordinary constant rows. Its commitment is supplied by the P proof's
public inputs at 56–59; its row bytes are omitted from the proof. P circuits
tie a verified proof's limbs to the shared values, and S circuits constrain the
verified P proof's row commitment to those limbs. A wallet's own predecessor
must carry the same table commitment.

The approved credential specification requires the issuer to derive this
commitment from the attested payment key before signing, and requires the
wallet to recompute it at credential intake. Its implementation is still
pending in the production codec, issuer and native wallet. A synthetic signed
statement in this experiment is not that production credential.

Separate lookup tables for distinct signature keys alone do not authenticate
an issuer. The standalone [certificate relation](../CERTIFICATE.md) verifies
the exact 108-byte certificate under a fixed root, constrains its Enrollment
role and trusted public scheme, and exposes its canonical key and object
digest. The [credential relation](../CREDENTIAL.md) verifies the approved
532-byte body, joins that exact certificate digest, derives the issuer's
table from the authenticated key, and binds every row to the distinct lookup
table consumed by the signature verifier. Rotation and different trusted
scheme inputs share the same root/provider verification key; the scheme is
not a VK constant that would introduce an identity cycle.

The current certificate transcript has no table-commitment field, and none
was added. The credential's 64-byte payment-table commitment is signed and
canonically encoded, but issuer/native intake recomputation and binding the
authenticated commitment to the monetary wallet row remain separate obligations.
These standalone relations are not yet the
recursive Bootstrap relation. Other policy signer roles likewise still need
their own authority bindings. The production credential codec and existing
native fixture remain at 468 bytes; research vectors document the explicit
532-byte derivation without changing those production inputs.

The composed credential relation includes the self-contained native evidence,
renewal and regulatory predicate on the signed byte tape. Original platform
admission, predecessor renewal continuity and runtime time-anchor decisions
remain external or subsequent relation obligations. The complete 68-test
research suite passed on 2026-11-12, including the full standalone
authentication proofs and the existing recursive regressions; this does not
turn the synthetic recursive own relations into monetary transitions.

## Remaining release obligations

The following are implementation obligations, not a claim of completed review:

- Replace census-sized synthetic relations with the actual state transition,
  credential/certificate, activation, policy, amount, fee, lineage and replay
  rules. A link between synthetic statement heads does not establish monetary
  balance or conservation.
- Integrate the validated standalone root/issuer authentication and derived
  issuer-table binding into Bootstrap, bind the payment-table commitment,
  and authenticate the remaining policy signer roles. The approved future
  30-field core includes `prior_credential_root`; production's 29-field core
  cannot supply its field indices unchanged.
- Define the full construction and canonical wire encoding, including strict
  decoding and final accumulator decisions at every accepting boundary.
- Meet the actual transport budgets. Before the new continuity work, the
  fixed-point proof pair was 5,024 + 4,928 = 9,952 bytes. That pair does not fit
  the current 5,120-byte lineage-proof cap or the historical future 8,458-byte
  proof allowance. The owner's later future target permits a 12,000-byte full
  QR carrier; the complete envelope still needs serialization and measurement.
  A projected smaller witness is not a passing wire-size test.
  The pair's two `Proof::to_bytes` outputs omit the separate 32-byte Link R, and ordinary
  `Instance.io` requires explicit reconstruction or encoding. The historical
  3,542-byte carrier overhead gives at least `9,952 + 32 + 3,542 = 13,526`
  bytes for the current pair. A hypothetical 32-row pair gives
  `7,904 + 32 + 3,542 = 11,478` bytes before additional inputs; neither is a
  complete envelope measurement or a demonstrated future fit.
- Use cryptographic randomness and the existing hardware-backed custody,
  durable exact-byte retry and recovery boundaries in the canonical native
  wallet and SDKs. Do not add a second monetary implementation in the apps.
- Complete independent soundness review and end-to-end device measurements.
  A timing model such as Receive 2.21 s or Send 1.55 s is not a measured
  production mobile payment.
- Deliver a deliberately selected future wallet generation. The running PoC
  must retain its data and working wallet format; this work authorizes no
  Taira reset or replacement of its installed relation.

The production v88 prover improvements and this research backend have separate
release status. Publishing v88 does not make Path 2 available to users.
