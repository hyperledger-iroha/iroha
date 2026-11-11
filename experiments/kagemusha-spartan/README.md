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
  comb. The wallet's comb tables are derived in-circuit by
  `nonnative::key_tables` (tested against the native tables), but with them
  Bootstrap needs 74 witness rows where Receive's layout leaves 64, so the
  fixed point runs without them (`ZKS_BOOT_TABLES=on` tries them); how the
  tables are certified is an owner decision (A2-UNIFORM.md, "Bootstrap's key
  tables"). Not modelled: the wallet's table rows in Send and Receive (they
  read one key's tables from constant columns), the issuer's and root's own
  keys, the ledger's activation record, and the wallet state carried from one
  step to the next (`σ.predecessor = Ω.head`).
- Spartan proof acceptance alone does not establish deferred-claim validity.
  Both native accumulator checks must hold. Intermediate template-building
  hops deliberately contain false accumulators and are not valid payments.
- `SpikeRng` is deterministic research randomness, not custody randomness.
- Full soundness review, exact monetary wire encoding, wallet lifecycle/native
  integration and device validation remain open. The fixed point with both
  operations has not been measured on a phone.

Nothing here changes the installed KAGEMUSHA protocol, wallet generation, Taira
state or the production prover. These gaps must be closed before proposing a
production integration.

## Reproduce

Run from this directory. Keep one target directory across repeated runs; the
following reuses the original research lane in this checkout:

```sh
export CARGO_TARGET_DIR="$PWD/../../output/kagemusha-100x-20261108/spike/target-zks"
cargo test --locked --offline --release
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
hold. At the last hop two wallets' chains start at Bootstrap pairs: every order
(Send and Receive after a Bootstrap, a Receive and a Send) must run on the
hop's circuits, verify, and have accumulators that hold natively, and changed
kind bits or a changed empty claim must break a row. The benchmark asserts
exact native `q` coefficient equality between batched and pointwise
evaluation.

## Research records

[A2-UNIFORM.md](A2-UNIFORM.md) records the current compiler/fixed-point work and
native batching. The other Markdown files retain earlier research measurements;
their stage-specific claims are not current production-readiness statements.
[The design record](research/DESIGN-100X.md) and the related reviews are preserved
under `research/`, including the unresolved soundness review.

The source was moved from
`output/kagemusha-100x-20261108/spike/zk-spartan` without leaving a duplicate source
tree. Historical run logs, benchmark binaries and phone packages remain ignored
under `output/kagemusha-100x-20261108`; they are not part of this crate or its
build. The unique design and review records were moved into `research/` as well.
No credentials, generated proof artifacts,
phone binaries or benchmark logs are tracked here.
