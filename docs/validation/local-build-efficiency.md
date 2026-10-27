# Local build-efficiency validation

The [local goals L1–L6](../../specs/compile_bloat_optimization_goals.md#active-local-build-goals)
remain open. The implementation includes stable local metadata, matching
incremental dev/test profiles, smaller Peer/Swarm dependency boundaries,
extracted Core test bodies, and a persistent-target profiler. These changes
require no CI adoption or workflow changes.

The PLONK source-identity build script and its private local stamp corridor are
removed. Runtime admission uses explicit proof ABI and exact artifact identities.
The native scanner remains an independent packaging-provenance evaluator used by
`audit_native_source_inputs.py`. The local profiler records Cargo wall time,
complete selected-source snapshots before and after measurement, source-drift
refusal, toolchain identity, and actual compiler evidence. Current-source
qualification must use these owners; the preflight timings below describe the
recorded earlier baseline.

## Accepted baseline

The 24 October 2026 baseline used Rust 1.93.1 on macOS arm64, the existing
repository `target`, stable local metadata, incremental compilation and compiler
instrumentation. Five measured `check -p iroha_plonk --lib` invocations each
reported 157 fresh artifacts and zero rebuilt artifacts. Cargo also received
`--locked --offline --message-format=json --timings`. Warm-up and target-size
accounting were excluded from command timing.

| Component | Median | Worst |
| --- | ---: | ---: |
| Whole command | 24.775 s | 31.154 s |
| Complete source preflight | 23.995 s | 30.363 s |
| Cargo | 0.566 s | 0.575 s |

The accepted Git-selected source fingerprint was
`a9d3869573fd5a20cc32f6f087baf51fd3fd26fc40bfeaa1727584d169645236`:
27,039 files, one deleted path and 956,522,180 bytes. The report also binds the
complete native census before and after measurement, toolchain bytes,
environment, Cargo unit profiles/features and competing load. The dirty source
manifest, rather than Git revision alone, identifies this workload.

Three reversible private-identifier edits were measured for each baseline case.
Each cycle first verified the original source with a zero-rebuild check, then
compiled the edited normal library and restored the exact original bytes.

| Edit and check target | Median command | Worst command | Median summed compiler time | Largest compiler RSS | Allocated cache growth across edited samples |
| --- | ---: | ---: | ---: | ---: | ---: |
| PLONK owner / `iroha_plonk` | 53.900 s | 54.303 s | 3.071 s | 311,328,768 B | 266,240 B |
| Foundational model / `iroha_data_model` | 69.091 s | 69.177 s | 31.704 s | 6,486,884,352 B | 1,179,799,552 B |

Compiler time is the sum of measured units, not parallel wall time. RSS is the
largest single compiler process, not simultaneous system memory. Cache growth
excludes the separate original-source warm-ups. Repeating identical edits can
reuse older incremental versions; these results do not estimate cold builds.

Raw evidence is retained outside the repository in
`/tmp/iroha-local-noop-baseline-v3-20261024.json`, its adjacent transcripts,
`/tmp/iroha-local-baseline-source-v3-20261024.json`, and the
`/tmp/iroha-edit-baseline-{owner,model}-20261024` directories.

## Qualification scope and remaining work

On the recorded source cut, the 19-package consumer library/binary check and
Core/Torii/daemon/test-network library-test compilation passed. Focused runtime
selections passed 108 Core, 27 Torii, nine daemon and 13 test-network tests.
Foundational model and aggregate Peer selections passed 110 and 11 tests.
All 336 moved Core test names remained discoverable with their fixture paths.
Swarm's normal/build graph contained 245 packages, and its zero-rebuild check
skipped the irrelevant PLONK source preflight. Actual dev/test dependency reuse
was verified with the configured profiles.

After the Core/FHE edits, both complete inventory suites passed again: 180
panic-recovery tests and 204 operation-inventory tests with 49 subtests. All
1,312 panic-owner fingerprints matched, with unchanged classifications and
104 catch-unwind, 29 blocking-spawn, 453 task-spawn and ten upgrade-task sites.
The FHE ownership checker also matched 1,039 files and 23 primitives. These
finite owner sets, their inventories and checkers remained unchanged across the
runs; independent build-helper and documentation edits are not a frozen
whole-checkout qualification.

The first candidate measurement was refused during native preflight. Subsequent
inspection found 50 concurrent source changes, including a new Core source,
changed Core/FHE implementations and inventories, and an unrelated active
Cargo build. That run is invalid and is not a speedup result. The diagnostic
fixes and additional package-lookup optimization also postdate the accepted
baseline. Keep that evidence, but establish a fresh matched baseline before
comparing the current candidate.

Remaining acceptance requires a stable source window, five successful
zero-rebuild checks per arm, at least 30% lower median no-op time, and three
matched owner/model edit samples per arm. Recheck affected consumers, current
daemon cases and changed Core tests on that candidate; retain the complete
native input audit. Do not weaken source guards, discard failed attempts or
revert unrelated work to obtain a passing comparison. Full-workspace, cold-build,
network and release resource qualification remain separate.
