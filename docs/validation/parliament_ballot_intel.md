# Supplementary Intel ballot proof parity

On 2026-10-31 the standalone ballot proof library reproduced every pinned
candidate and comparator proof on an Intel Core i7-11800H under WSL2, with
and without its `accel` feature. The host and toolchain are recorded in
[Intel AVX2 execution](intel_avx2.md). This is supplementary correctness evidence;
the required Xeon Silver 4310, frequency/SMT controls, physical-cost calibration,
phone tests and independent review remain outstanding. No timing verdict was
derived from this shared development host.

Both commands used
`scripts/cargo_fast.sh --target-slot intel-parliament --stable-local-metadata --incremental --jobs 2 --`:

```sh
test --locked --profile local-release -p iroha_parliament_ballot_proof --features diagnostics --lib --test vectors
test --locked --profile local-release -p iroha_parliament_ballot_proof --features accel,diagnostics --lib --test vectors
```

Each passed 64 unit tests and 8 proof-vector tests: registration and votes with
3, 500 and 1,000 members on each row. The tests compare complete proof bytes,
every field digest, the accepted counter, public values and witness commitments
with the independently generated project fixtures, then verify the proofs.
`accel`
enables runtime-selected hardware AES; these runs establish output parity,
without claiming an instruction receipt for that backend.

The 254-file content inventory of `iroha_parliament_ballot_proof`,
`iroha_zkp_vole`, `vendor/aes` and build manifests remained unchanged across the
paired runs. Its SHA-256 is
`fe8787a78ecf926b27d00adb05b86f910ec431aeb5fcd3c9a4d5a4111e2f928d`,
using the sorted path-and-digest convention in the linked execution record.
Inventory, post-run source comparison and log digests remain untracked under
`dist/intel-platform/current-20261031/`: `parliament-evidence.json`,
`parliament-scalar.log` and `parliament-accel.log`. Other native builds ran
concurrently; these durations are not performance or named-host evidence. The
inventory covers the listed standalone owners and root manifests, not complete
transitive dependency custody or a shipping release.
