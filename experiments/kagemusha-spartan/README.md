# KAGEMUSHA Spartan research experiment

This standalone, unpublished executable preserves the Nova/Spartan research for
KAGEMUSHA in the canonical Iroha checkout. It is not a wallet implementation or a
shipping proof backend. It depends on the checkout's `crates/iroha_pasta`; it does
not vendor another Iroha checkout and is not a member of the production workspace.

The experiment builds linked zero-knowledge Spartan proofs over the Pasta cycle
for both offline operations, Receive and Send, and for Bootstrap, the first link
of every wallet's chain, using uniform circuit arrays, LogUp copy/lookups and
deferred matrix claims. Send's and Bootstrap's circuits are built at Receive's
layout, and the predecessor slot takes a proof of any of the three kinds (its
two kind bits, read from the proof's io, select the key and the matrices), so
the recursion's fixed point holds over every order with one circuit per kind.
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
These modules have not been integrated into the recursive Bootstrap circuit.
The production credential codec remains 468 bytes.
The credential relation also enforces its signed evidence fact rules,
self-contained renewal ordering, and regulatory response/lease consistency;
original issuer admission and predecessor-state joins remain separate.

## Limits that remain

- Send's, Receive's and Bootstrap's own relations have the census's gadgets and
  sizes over synthetic values, not the monetary semantics.
- The pair binding runs inside both proven circuits, in a link phase between the
  first and second phases (A2-UNIFORM.md, "The pair binding inside the
  circuits"). Its mask `mu` is derived from the pair's shared values here; a
  prover samples it per pair.
- Every chain starts at a Bootstrap proof (A2-UNIFORM.md, "The Bootstrap
  kind"; owner: "Bootstrap kind"). Send and Receive have no base-case flag, and
  their predecessor slot takes a Receive, Send or Bootstrap proof by its two
  public kind bits. Bootstrap's own relation is a census-sized stand-in: the
  zero state, and its receipt, credential and certificate signatures, all on the
  comb. Each wallet's comb tables are its own "wallet row" in every P
  circuit (A2-UNIFORM.md, "Issuer-certified key tables"): a fixed row whose
  commitment the issuer computes from the payment key, Bootstrap's credential
  signs, and every verifier takes from the proof's io, so one circuit serves
  every wallet. (Deriving the tables in-circuit, `nonnative::key_tables`, does
  not fit Bootstrap at Receive's layout; `ZKS_BOOT_TABLES=on` tries it.) Not
  modelled in recursive Bootstrap: the issuer's and root's own keys (its three
  signatures still use the wallet's tables as a stand-in), the ledger's activation record, and the
  exact wallet state carried from one step to the next. The research head
  boundary is now constrained: each Send/Receive statement consumes its
  verified predecessor's public head, signs that head together with its
  operation kind, and exposes its signed statement-sponge digest as the next
  head. Bootstrap pins its predecessor to zero. This closes the recursive
  head link but does not implement the monetary `CoreHead` encoding or
  balances, counters and map transitions.
- Spartan proof acceptance alone does not establish deferred-claim validity.
  Both native accumulator checks must hold. Intermediate template-building
  hops deliberately contain false accumulators and are not valid payments.
- `SpikeRng` is deterministic research randomness, not custody randomness.
- Full soundness review, exact monetary wire encoding, wallet lifecycle/native
  integration and device validation remain open. The fixed point with both
  operations has not been measured on a phone.
- Authenticating the credential's signed payment-table commitment does not
  prove its derivation from the payment key or bind it into Bootstrap's wallet
  row. The approved future state also has a 30-field core including
  `prior_credential_root`; the production 29-field core is not its layout.

Nothing here changes the installed KAGEMUSHA protocol, wallet generation, Taira
state or the production prover. These gaps must be closed before proposing a
production integration.

## Reproduce

Run from this directory. Keep one target directory across repeated runs; the
following reuses the current research lane in this checkout:

```sh
export CARGO_TARGET_DIR="$PWD/target"
# Include the recursive wrong-parent regression under its lean transcript.
ZKS_SCHEDULE=lean cargo test --locked --offline --release -- --include-ignored
cargo build --locked --offline --release
ZKS_SCHEDULE=lean ZKS_HOPS=8 \
  "$CARGO_TARGET_DIR/release/zk-spartan" fixed-point
```

A fresh checkout may need an initial dependency fetch before `--offline` works.
`ZKS_*` variables select research benchmarks and comparison implementations in
this executable only; no production application depends on them.

At the current fixed point all six circuits (P and S for Receive, Send and
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
`Instance.io` still needs explicit reconstruction or encoding. With the
historical 3,542-byte carrier overhead, the current pair already accounts for
at least `9,952 + 32 + 3,542 = 13,526` bytes. A hypothetical 32-row pair would
account for `7,904 + 32 + 3,542 = 11,478` bytes before additional inputs.
Neither calculation is a complete envelope measurement or a future fit claim.

## Research records

[A2-UNIFORM.md](A2-UNIFORM.md) records the current compiler/fixed-point work and
native batching. The other Markdown files retain earlier research measurements;
their stage-specific claims are not current production-readiness statements.
[The design record](research/DESIGN-100X.md) and the related reviews are preserved
under `research/`, including the unresolved soundness review.
[The construction record](research/CONSTRUCTION.md) pins the research generator
derivation, commitment layout and the remaining production obligations.

Validation on 2026-11-12: all 68 research tests passed with the lean schedule
and ignored tests included, covering the new authentication proofs and the
existing recursion regressions. The earlier eight-hop fixed point remains
the recursive baseline; its `full.rs` implementation was unchanged by these
authentication modules. Source hashes and the complete test log are retained
under `output/kagemusha-100x-20261108/spike/runs/codex-20261112/` as
`authentication-validation.json` and `authentication-full-tests.log`.

The source was moved from
`output/kagemusha-100x-20261108/spike/zk-spartan` without leaving a duplicate source
tree. Historical run logs, benchmark binaries and phone packages remain ignored
under `output/kagemusha-100x-20261108`; they are not part of this crate or its
build. The unique design and review records were moved into `research/` as well.
No credentials, generated proof artifacts,
phone binaries or benchmark logs are tracked here.
