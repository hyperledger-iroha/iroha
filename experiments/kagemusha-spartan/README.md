# KAGEMUSHA Spartan research experiment

This standalone, unpublished executable preserves the Nova/Spartan research for
KAGEMUSHA in the canonical Iroha checkout. It is not a wallet implementation or a
shipping proof backend. It depends on the checkout's `crates/iroha_pasta`; it does
not vendor another Iroha checkout and is not a member of the production workspace.

The experiment builds linked zero-knowledge Spartan proofs over the Pasta cycle
for both offline operations, Receive and Send, using uniform circuit arrays,
LogUp copy/lookups and deferred matrix claims. Send's circuits are built at
Receive's layout, and the predecessor slot takes a proof of either kind (its
kind bit, read from the proof's io, selects the key and the matrices), so the
recursion's fixed point holds over all four orders with one circuit per
operation. The fixed-point runner checks the final accumulated claims natively.
Native claim-polynomial evaluation is batched over register-reusing groups of
four points.

## Limits that remain

- Send's and Receive's own relations have the census's gadgets and sizes over
  synthetic values, not the monetary semantics.
- The pair binding runs inside both proven circuits, in a link phase between the
  first and second phases (A2-UNIFORM.md, "The pair binding inside the
  circuits"). Its mask `mu` is derived from the pair's shared values here; a
  prover samples it per pair.
- The public base-case flag can discard predecessor claims. Authenticated
  first-state admission and its binding to the real wallet protocol are not
  implemented (an owner decision: the spec's Bootstrap step is the natural first
  link). The fixed-point runner deliberately uses this flag only to bootstrap its
  research template after the matrices converge, starting Send's chain one hop
  before Receive's.
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
ZKS_SCHEDULE=lean ZKS_HOPS=8 ZKS_GENESIS_HOP=7 \
  "$CARGO_TARGET_DIR/release/zk-spartan" fixed-point
```

A fresh checkout may need an initial dependency fetch before `--offline` works.
`ZKS_*` variables select research benchmarks and comparison implementations in
this executable only; no production application depends on them.

At the current fixed point all four circuits (P-recv, S-recv, P-send, S-send)
have `2^19` constraints and 64 rows; each linked pair's proofs occupy 4,992 and
4,896 bytes before the full payment envelope. Hops 7 and 8 must report both
operations' native accumulators as true (Send's already at hop 6); the repeated
matrix evaluations must agree at the final hops; at the last hop the circuits
rebuilt over a Send predecessor must equal the hop's, and Send after Send and
Receive after Send must verify. The benchmark asserts exact native `q`
coefficient equality between batched and pointwise evaluation.

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
