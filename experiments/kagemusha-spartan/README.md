# KAGEMUSHA Spartan research experiment

This standalone, unpublished executable preserves the Nova/Spartan research for
KAGEMUSHA in the canonical Iroha checkout. It is not a wallet implementation or a
shipping proof backend. It depends on the checkout's `crates/iroha_pasta`; it does
not vendor another Iroha checkout and is not a member of the production workspace.

The Receive experiment builds linked zero-knowledge Spartan proofs over the Pasta
cycle, using uniform circuit arrays, LogUp copy/lookups and deferred matrix
claims. The fixed-point runner checks the final accumulated claims natively.
The latest completed optimization batches native claim-polynomial evaluation
over register-reusing groups of four points and parallelizes independent groups.

## Limits that remain

- The payer's Send proof is a toy stand-in; a complete uniform Send circuit is
  still needed.
- The cross-field binding chain uses stand-in `rho` and `mu` values.
- The public base-case flag can discard predecessor claims. Authenticated
  first-state admission and its binding to the real wallet protocol are not
  implemented. The fixed-point runner deliberately uses this flag only to
  bootstrap its research template after the matrices converge.
- Spartan proof acceptance alone does not establish deferred-claim validity.
  Both native accumulator checks must hold. Intermediate template-building
  hops deliberately contain false accumulators and are not valid payments.
- `SpikeRng` is deterministic research randomness, not custody randomness.
- Full soundness review, exact monetary wire encoding, wallet lifecycle/native
  integration and device validation remain open. The fixed-point pair including
  the batched native work has not been measured on a phone.

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
ZKS_SCHEDULE=lean ZKS_HOPS=6 ZKS_GENESIS_HOP=5 \
  "$CARGO_TARGET_DIR/release/zk-spartan" fixed-point
```

A fresh checkout may need an initial dependency fetch before `--offline` works.
`ZKS_*` variables select research benchmarks and comparison implementations in
this executable only; no production application depends on them.

At the current fixed point the shapes are P-recv `2^19` and S-recv `2^18`
constraints. The linked proofs occupy 5,024 and 4,864 bytes before the full payment
envelope. Hops 5 and 6 must report both native accumulators as true; the repeated
matrix evaluations must agree at the final hops. The benchmark asserts exact
native `q` coefficient equality between batched and pointwise evaluation.

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
