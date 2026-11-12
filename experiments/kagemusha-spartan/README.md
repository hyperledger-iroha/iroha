# KAGEMUSHA Spartan research experiment

This standalone research executable preserves the Nova/Spartan research for
KAGEMUSHA in the canonical Iroha checkout. It is not a wallet implementation or a
shipping proof backend. It depends on the checkout's `crates/iroha_pasta`; it does
not vendor another Iroha checkout and is not a member of the production workspace.

The experiment builds linked zero-knowledge Spartan proofs over the Pasta cycle
for both offline operations, Receive and Send, and for Bootstrap, the first link
of every wallet's chain, using uniform circuit arrays, LogUp copy/lookups and
deferred matrix claims. All three kinds are rebuilt at a shared layout that
covers each phase's maximum and fixed-row alignment. The predecessor slot takes
a proof of any of the three kinds: its two public kind bits select the key and
matrices. A successful fixed-point run must establish every order under one
circuit per kind; a linked proof alone does not establish that property.
Bootstrap verifies no proof; its accumulator is the empty claim. The
fixed-point runner checks the final accumulated claims natively.
Native claim-polynomial evaluation is batched over register-reusing groups of
four points.

Separate authentication relations now cover the exact current-wire 108-byte
Enrollment certificate and the approved future-generation 532-byte credential.
The root key is fixed, the expected scheme is a trusted public input, and the
credential issuer's comb table is constrained to the key authenticated by its
certificate. See [CERTIFICATE.md](CERTIFICATE.md) and
[CREDENTIAL.md](CREDENTIAL.md) for exact byte layouts, vectors and boundaries.
These modules now compose into the exact future-generation Bootstrap relation
and its recursive harness; see [BOOTSTRAP.md](BOOTSTRAP.md).
The new [monetary components](MONETARY_COMPONENTS.md) cover state, Request,
statement, fee, Send arithmetic and quota/time rules. They are not yet composed
into the recursive Send and Receive transitions.
The production credential codec remains 468 bytes.
The credential relation also enforces its signed evidence fact rules,
self-contained renewal ordering, and regulatory response/lease consistency;
original issuer admission and predecessor-state joins remain separate.

## Limits that remain

- Send and Receive still use census-sized synthetic own relations. Bootstrap
  authenticates its credential, exact initial 30-field state, statement and receipt;
  the remaining transitions and native integration are unfinished.
- The pair binding runs inside both proven circuits, in a link phase between the
  first and second phases (A2-UNIFORM.md, "The pair binding inside the
  circuits"). Its mask `mu` is derived from the pair's shared values here; a
  prover samples it per pair.
- Every chain starts at a Bootstrap proof (A2-UNIFORM.md, "The Bootstrap
  kind"; owner: "Bootstrap kind"). Send and Receive have no base-case flag, and
  their predecessor slot takes a Receive, Send or Bootstrap proof by its two
  public kind bits. Bootstrap authenticates separate root, issuer and payment
  signatures. Its signed credential binds the payment table commitment to the
  omitted wallet row; its public head is the exact initial state commitment.
  Send and Receive consume the verified predecessor's head and carry its immutable
  credential context, but their successor hashes and monetary logic remain
  synthetic. The receiver's payer proof cannot supply the wallet predecessor.
- Spartan proof acceptance alone does not establish deferred-claim validity.
  Both native accumulator checks must hold. Intermediate template-building
  hops deliberately contain false accumulators and are not valid payments.
- `SpikeRng` is deterministic research randomness, not custody randomness.
- Full soundness review, exact monetary wire encoding, wallet lifecycle/native
  integration and device validation remain open. The fixed point with both
  operations has not been measured on a phone.
- The issuer and native wallet must recompute the payment table commitment from
  the hardware key before signing/intake, as the approved protocol requires.
  Bootstrap binds the signed commitment to its wallet row. The production codec
  still needs the future 532-byte credential and 30-field Core cutover.

Nothing here changes the installed KAGEMUSHA protocol, wallet generation, Taira
state or the production prover. These gaps must be closed before proposing a
production integration.

## Reproduce

Run from this directory. Keep one target directory across repeated runs; the
following reuses the current research lane in this checkout:

```sh
export CARGO_TARGET_DIR="$PWD/target"
# Include the recursive wrong-parent regression under its lean transcript.
ZKS_SCHEDULE=lean RAYON_NUM_THREADS=4 \
  cargo test --locked --offline --release -- --include-ignored --test-threads=2
cargo build --locked --offline --release
ZKS_SCHEDULE=lean ZKS_HOPS=8 ZKS_DETERMINISM=1 RAYON_NUM_THREADS=4 \
  "$CARGO_TARGET_DIR/release/zk-spartan" fixed-point
```

A fresh checkout may need an initial dependency fetch before `--offline` works.
`ZKS_*` variables select research benchmarks and comparison implementations in
this executable only; no production application depends on them.

At the preserved pre-authentication fixed point all six circuits (P and S for Receive, Send and
Bootstrap) have `2^19` constraints and 64 rows; each linked pair's proofs
occupy 5,024 and 4,928 bytes before the full payment envelope. The matrices
must agree from hop 5 on. The hop chain starts from toy pairs, so its
accumulators stay false; every Bootstrap pair's accumulators (the empty claim)
hold. At the last hop two wallets with different keys start at Bootstrap pairs:
every order (Send and Receive after a Bootstrap, a Receive and a Send) must run
on the hop's circuits, verify, and have accumulators that hold natively;
changed kind bits, a changed empty claim or a changed table-commitment limb
must break a row, and a proof checked under the other wallet's table
commitment must be refused. Every operation must consume proof 0's head;
changing its signed predecessor or kind must break a row. The regression
suite also re-signs and re-proves an internally valid own statement with a
wrong predecessor, including the payer's head, for both Send and a Receive
with two input proofs: the recursive proof must refuse it. The benchmark asserts
exact native `q` coefficient equality between batched and pointwise
evaluation.

That 9,952-byte synthetic pair exceeds the current 5,120-byte lineage cap and
the historical future-generation 8,458-byte proof allowance. The owner's later
future-generation target permits a 12,000-byte full QR carrier; the complete
serialized envelope must be measured before claiming a fit. Standalone
authentication proofs do not replace that end-to-end size test.
The pair's two `Proof::to_bytes` outputs exclude the separate 32-byte Link R, and ordinary
`Instance.io` needs explicit reconstruction or encoding. `carrier.rs` now strictly
round-trips the actual Norito pair envelope and supports separately authenticated
public-input reconstruction; it does not yet serialize the whole Payment. With the
historical 3,542-byte carrier overhead, that historical pair already accounts for
at least `9,952 + 32 + 3,542 = 13,526` bytes. A hypothetical 32-row pair would
account for `7,904 + 32 + 3,542 = 11,478` bytes before additional inputs.
Neither calculation is a complete envelope measurement or a future fit claim.

The current composed Bootstrap run measured an actual 11,731-byte Norito
pair envelope: 3,840 + 3,424 proof bytes, 4,032 ordinary-input bytes, 32 bytes
for Link R and 403 framing/profile bytes. Reconstructing all 126 ordinary inputs
from the explicitly supplied Bootstrap context reduced that envelope to 7,699
bytes. The omitted, separately authenticated context and credential must still
be counted in the complete Payment. The eight-hop run passes with the final
Bootstrap-seeded chain rebuilt at the same shared layouts as the hop templates:
P has `2^19` constraints and 32 rows of 65,536; S has `2^18` constraints and
16 rows of 32,768. All six Send/Receive predecessor orderings preserve circuit
identity, verify under two wallets' certified tables and pass both final native
accumulator checks. These are successful recursive harness and pair-encoding
results; Send/Receive monetary semantics and the complete Payment remain open.

## Research records

[A2-UNIFORM.md](A2-UNIFORM.md) records the current compiler/fixed-point work and
native batching. The other Markdown files retain earlier research measurements;
their stage-specific claims are not current production-readiness statements.
[The design record](research/DESIGN-100X.md) and the related reviews are preserved
under `research/`, including the unresolved soundness review.
[The construction record](research/CONSTRUCTION.md) pins the research generator
derivation, commitment layout and the remaining production obligations.

The current checkpoint passed all 126 research tests with the
lean schedule and ignored tests included: zero failures, ignored or filtered
tests. This covers authentication, composed Bootstrap, monetary components,
strict carrier decoding and the existing recursion regressions. The test run
took 427.37 seconds; the experiment's source hashes remained unchanged across
compilation and testing. Evidence is retained under
`output/kagemusha-100x-20261108/spike/runs/codex-20261112/` in
`checkpoint-test-validation.json` and `checkpoint-full-tests.log`.
The separately executed eight-hop runner, with deterministic rebuild checks,
also passes; its complete output is `bootstrap-resume-fixed-point-8.log` in
that directory, with source/binary hashes and results in
`bootstrap-resume-fixed-point-validation.json`. Each of the six templates matches
its own preceding-hop matrices from hop 4 through hop 8, and all 32 deterministic
rebuild comparisons pass. The run rejects swapped proof partners, changed
predecessor kind or table limbs and a proof checked under the other wallet's
table commitment.
The published 68-test authentication milestone remains historical evidence,
not the current count.

The source was moved from
`output/kagemusha-100x-20261108/spike/zk-spartan` without leaving a duplicate source
tree. Historical run logs, benchmark binaries and phone packages remain ignored
under `output/kagemusha-100x-20261108`; they are not part of this crate or its
build. The unique design and review records were moved into `research/` as well.
No credentials, generated proof artifacts,
phone binaries or benchmark logs are tracked here.
