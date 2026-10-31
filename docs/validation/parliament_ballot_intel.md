# Supplementary Intel ballot proof parity

On 2026-10-22 the standalone ballot proof library reproduced every pinned
candidate and comparator proof on an Intel Core i7-11800H under WSL2, with
and without its `accel` feature. The host and toolchain are recorded in
[Intel AVX2 execution](intel_avx2.md). This is supplementary correctness evidence;
the required Xeon Silver 4310, frequency/SMT controls, physical-cost calibration,
phone tests and independent review remain outstanding. No timing verdict was
derived from this shared development host.

Both commands used
`scripts/cargo_fast.sh --target-slot intel-platform --stable-local-metadata --jobs 4 --`:

```sh
test --locked --profile local-release -p iroha_parliament_ballot_proof --features diagnostics --lib --test vectors
test --locked --profile local-release -p iroha_parliament_ballot_proof --features accel,diagnostics --lib --test vectors
```

Each passed 64 unit tests and 8 proof-vector tests: registration and votes with
3, 500 and 1,000 members on each row. The tests compare complete proof bytes,
every field digest, the accepted counter, public values and witness commitments
with the independently generated project fixtures, then verify the proofs.
The ordinary candidate-only vector suite also passed all 4 cases. `accel`
enables runtime-selected hardware AES; these runs establish output parity,
without claiming an instruction receipt for that backend.

The 244-file content inventory of `iroha_parliament_ballot_proof`,
`iroha_zkp_vole`, `vendor/aes` and build manifests remained unchanged across the
paired runs. Its SHA-256 is
`e015b92cd8e546676242c7b47e46f2efd8dea15b428f0aae19fcdfd73078500c`,
using the sorted path-and-digest convention in the linked execution record.
Inventory and logs remain untracked under `dist/intel-platform/`:
`parliament-source.json`, `parliament-scalar-vectors.log`,
`parliament-scalar-diagnostics-vectors.log` and `parliament-accel-vectors.log`.
