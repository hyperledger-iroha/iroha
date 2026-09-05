# FASTPQ production readiness

Updated: 2026-09-05. **Production qualification is unavailable.** The selected
completion target is succinct verification from bounded authenticated openings.
A successful local test, feature build, arithmetic calculation, or benchmark
manifest is not a release qualification decision.

## Completion goals

| Goal | Required evidence | Current state |
| --- | --- | --- |
| G1: Close admission and evidence gaps | Regression rejection of unanchored remote spend, exact bound arithmetic, full-width contextual commitments, authenticated benchmark evidence | Corrections in progress; validation below |
| G2: Constrain the complete transfer statement | Reviewed AIR constraint ledger and negative tests for every statement relation below | Incomplete; full replay remains necessary |
| G3: Implement succinct verification | Quotient/zerofier relation, correct terminal degree bound, bounded openings, no witness/trace reconstruction in the public verifier | Incomplete |
| G4: Qualify cryptography | Protocol-specific qROM argument, final multi-target digest analysis, independently reproduced constants and vectors, independent review bound to final artifacts | Unavailable |
| G5: Qualify performance and resources | End-to-end proof/verification latency and peak memory, proof size, CPU/Metal/CUDA parity and failure quarantine on release hardware | Native Metal compilation passes; no current runtime capture qualifies the changed candidate |
| G6: Qualify integration and release | Same-source four-validator tests, restart/recovery and adversarial admission, signed immutable source and artifacts, rollout/rollback evidence | Incomplete |

Goals G2 and G3 must complete together before removing replay. The selected
product remains a transfer proof system; replacing it with a replay-only format
would not satisfy this goal.

## Findings and corrections

1. **Arithmetic overflow:** `u128::checked_shl` checks shift counts rather than
   discarded high bits. Security-bound numerators now use checked multiplication
   and reject overflow and zero-target accounting.
2. **Incorrect sampling geometry:** the raw quadratic composition has intended
   exclusive degree `2 * N_trace`, rather than `N_trace`. The current binary FRI
   schedule then overfolds to two terminal evaluations. Rounding its terminal
   exclusive bound up to one admits starting degree below `N_eval / 2`, rather
   than the intended `N_eval / 4`. A deterministic `x^3` regression demonstrates
   this difference for domain size eight. Full replay still compares against the
   expected composition, so this is not a demonstration of a forged transfer
   passing the complete verifier.
3. **Unsupported arithmetic claim:** the corrected diagnostic model accounts for
   both degree expansion and terminal rounding. With current 136 queries its
   sampling term is `54 * 2^64 / 2^68`, which does not meet the 128-bit target.
   The same model selects 400 queries (392 fail); stopping at four terminal
   evaluations would instead select 200. Neither count establishes protocol
   security. The current wire query count is deliberately unchanged; a coherent
   protocol, parameter, fixture and resource-limit migration is still required.
4. **Unanchored remote spending:** a reusable issuer-signed capability does not
   authenticate the exact intent, proof, effective amount, source state roots or
   transaction set. CoreHost rejects an otherwise valid capability before proof
   processing and rejects preexisting handle state at commit. Block admission
   also rejects otherwise valid envelopes carrying handles. Existing malformed
   statement diagnostics remain tested. Finalized lane-relay and fee-vault paths
   retain their separately authenticated boundaries.
5. **Narrow permission commitment:** the host previously zero-padded one
   Goldilocks field element to 32 bytes and used duplicate-last Merkle padding.
   Nonempty permission tables now hash a fixed domain, little-endian u64 entry
   count, and sorted fixed-width role/permission/epoch entries with the existing
   `iroha_crypto::Hash` implementation. Empty tables retain the zero sentinel.
   Existing proofs/witnesses that bind a nonempty permission table must be
   regenerated. This remains contextual input, not an AIR permission proof, and
   its 32-byte width does not establish aggregate 128-bit post-quantum security.
6. **Mismatched default prover/verifier capacity:** the 256-transition ceiling
   and 512 KiB approximate proof ceiling are independent. Even a minimally wide
   16-row transfer proof exceeds the byte ceiling under the current opening
   layout. Public proving now enforces the same default envelope as public
   verification, while raw development diagnostics can use explicit larger
   limits. Raising query counts without redesigning proof size and measured
   resource budgets would aggravate this mismatch. The 20,000-row accelerator
   microbenchmark is not evidence of admitted end-to-end proof capacity.
7. **Rollout evidence:** validation must inspect captured workload shape, actual
   CPU/GPU timings, backend availability, both captured-file hashes, numeric
   finiteness, telemetry and externally trusted manifest signatures. Hardware CI
   must fail on unavailable/broken CUDA rather than silently skip qualification.

## Succinct proof acceptance contract

The current 14 residues cover selector booleanity and relations, active-prefix
shape, a transfer delta, and metadata/dataspace/slot stability. They are not a
complete transfer AIR. `verify_with_limits` must retain its deterministic batch,
transcript, SMT, trace, LDE and root reconstruction until all these gaps close:

- Encode and constrain byte lengths, canonical packed limbs, and u64 balances,
  amounts, range bounds, debit/credit carries and conservation. A field delta
  alone cannot establish checked integer arithmetic.
- Bind canonical accounts, asset identity, key paths, transaction ordering and
  cardinality to authenticated public inputs. Prove the required multiset and
  sequential execution relations, including repeated touched keys.
- Constrain complete hash inputs/outputs for every balance leaf and SMT node;
  scalar projections of 256-bit hashes cannot establish full root binding.
  Prove direction-bit/key agreement, sibling selection, old/new leaf relation,
  root chaining and public boundary roots.
- Authenticate the source state and exact authorization facts independently of
  a prover-carried witness. The private touched-balance root is not automatically
  the consensus-wide state root. The remote-spend gate can reopen only with
  authoritative finalized/QC source anchoring and exact spend authentication.
- Add each boundary and transition zerofier and the corresponding quotient
  relation. Base checking currently treats the final next row as itself while
  LDE composition wraps to row zero; the final-row exclusion must be explicit.
- Bind all composition commitments and challenges to the full statement and
  canonical profile. Fix the FRI stopping rule so its terminal check establishes
  the claimed exclusive degree without rounding away part of the bound.
- Sample and authenticate the required openings and public-input relations with
  a protocol-specific soundness argument. Removing a full-trace check is allowed
  only after a test demonstrates the equivalent authenticated constraint.
- Freeze the resulting first-release parameter/profile identity, query count,
  proof schema, fixtures and decoding/resource limits together. Rebuild every
  consuming SDK, host, proof envelope and release artifact from that source.

Adversarial tests must change each accepted-state relation independently,
including equal-looking field reductions, omitted/duplicated transfers, root
substitution, final-row violations and unauthorized source claims. The final
public verifier must not build a complete trace, FFT/LDE, row hash collection or
Merkle tree. Instrumented scaling tests must demonstrate verification work
bounded by the declared proof openings and public input size.

## Evidence and limits

The worktree contains concurrent unrelated edits. Local checks do not qualify
an immutable release candidate. No live deployment is authorized by a passing
source check, and no production deployment has been performed by this work.

- `cargo iroha-fast -- test --locked -p fastpq_isi --lib`: 36 passed,
  including the 31 independently generated digest vectors checked against Rust
  one-shot and streaming implementations.
- `python3 -m pytest -q scripts/fastpq/tests/test_reference_digest384.py
  scripts/fastpq/tests/test_validate_rollout_manifest.py
  scripts/fastpq/tests/test_launch_geometry_sweep.py`: 72 passed, 14 subtests.
- The final rollout/wrapper/geometry suite passed 98 tests. The expanded tooling
  suite passed 165 and failed 15 at the existing authenticated release bootstrap
  hash pin; that unrelated reviewed-hash mismatch was not bypassed.
- Public documentation: the English page and all 20 translations were corrected
  in the sibling `iroha-docs` checkout. Node 24 production build, built links,
  FASTPQ locale/source-hash checks, RTL output, content/provenance checks,
  TypeScript and 127 focused documentation tests passed. Whole-site locale
  validation still reports 60 existing errors on untouched
  `sora-nexus-services.md` translations. Nothing was published.
- `scripts/check_no_legacy_codec.sh`: passed.
- Actual native Metal compilation: all three shaders compiled with the exact
  build-script Metal 2.4/O3 flags under Apple Metal/AIR-LLD 32023.883 and
  MetalToolchain 17.6.109.0. Linked `fastpq.metallib` is 112,674 bytes with SHA256
  `85642942a5ab48f376a71e7c0abae2cfc13b6eef1d587ae1eb6749f74fc57b72`.
  Commands, source hashes and timings are in the local untracked
  `target/fastpq-native-metal-validation/native_compile.json`. This establishes
  offline shader compilation only; GPU execution/parity/performance are untested.
- `cargo iroha-fast -- test --locked -p fastpq_prover --lib` and the focused
  Core host admission build, plus `cargo iroha-fast -- test --locked -p xtask
  --features dev-tools --bin xtask verify_bench_manifest`: blocked before test execution by concurrent,
  untracked `iroha_data_model` racing enums missing Norito JSON tags. The new
  prover, Core and xtask Rust changes remain unverified. Those concurrent JSON
  annotations have since been fixed; a final prover rerun is queued on the
  shared Cargo build target.
- `cargo fmt --all --check`: reports unrelated worktree formatting differences.
  Changed FASTPQ files are formatted individually; `git diff --check` passes.

The direct sampled oracle-link checks preserve the proof wire and full replay.
The independent Python reference reproduces the pinned parameter SHA3 digest
`84c5055b47cc7289835e0a5f31d4563849244ffddbf51f5d67b1db95222ce3e6`.
Neither result closes the missing succinct semantic argument.

Release prerequisites include independent cryptographic review and access to
release-class Apple/NVIDIA hardware with actual shader/kernel execution. The
current six-lane construction has `p^6` possible canonical outputs, not exactly
`2^384`; its idealized collision term and all 32-byte external commitments need
explicit treatment in the final argument. An implementation cross-check is
regression evidence, not an independent security audit.

## Pending Rust validation after the shared build is repaired

Use the existing warm Cargo target and `cargo iroha-fast`; no clean build or
new target directory is needed. These tests have not passed in this audit:

```sh
cargo iroha-fast -- test --locked -p fastpq_prover --lib
cargo iroha-fast -- test --locked -p fastpq_prover --features dev-tools --test fastpq_integration --test transcript_replay
cargo iroha-fast -- test --locked -p iroha_core --lib fastpq::
cargo iroha-fast -- test --locked -p iroha_core --lib axt_unanchored_admission_tests
cargo iroha-fast -- test --locked -p iroha_core --lib ivm_corehost_axt_tests
cargo iroha-fast -- test --locked -p iroha_core --lib axt_validation
cargo iroha-fast -- test --locked -p xtask --features dev-tools --bin xtask verify_bench_manifest
cargo iroha-fast -- test --locked -p xtask --features dev-tools --bin xtask parse_fastpq_manifest_verification
cargo iroha-fast -- test --locked -p fastpq_prover --features fastpq-gpu --lib cuda_test_requirement
cargo iroha-fast -- test --locked -p fastpq_prover --features dev-tools,fastpq-gpu --bin fastpq_cuda_bench --bin fastpq_metal_bench collect_operations_rejects_gpu_timings_without_a_dispatch
```

Follow with real hardware parity, the four-validator admission/recovery corridor,
full workspace tests and strict Clippy against the settled candidate. A pending
command or missing-device skip is never passing release evidence.
