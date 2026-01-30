---
lang: pt
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2026-01-03T18:07:57.621942+00:00"
translation_last_reviewed: 2026-01-30
---

# Poseidon Metal Shared Constants

Metal kernels, CUDA kernels, the Rust prover, and every SDK fixture must share
the exact same Poseidon2 parameters in order to keep hardware-accelerated
hashing deterministic. This document records the canonical snapshot, how to
regenerate it, and how GPU pipelines are expected to ingest the data.

## Snapshot Manifest

The parameters are published as a `PoseidonSnapshot` RON document. Copies are
kept under version control so GPU toolchains and SDKs do not rely on build-time
code generation.

| Path | Purpose | SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` | Canonical snapshot generated from `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}`; source of truth for GPU builds. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` | Mirrors the canonical snapshot so Swift unit tests and the XCFramework smoke harness load the same constants the Metal kernels expect. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin fixtures share the identical manifest for parity and serialization tests. | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

Every consumer must verify the hash before wiring the constants into a GPU
pipeline. When the manifest changes (new parameter set or profile), the SHA and
the downstream mirrors must be updated in lock-step.

## Regeneration

The manifest is generated from the Rust sources by running the `xtask`
helper. The command writes both the canonical file and the SDK mirrors:

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

Use `--constants <path>`/`--vectors <path>` to override the destinations or
`--no-sdk-mirror` when regenerating only the canonical snapshot. The helper will
mirror the artefacts into the Swift and Android trees when the flag is omitted,
which keeps the hashes aligned for CI.

## Feeding Metal/CUDA Builds

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` and
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` must be regenerated from the
  manifest whenever the table changes.
- Rounded and MDS constants are staged into contiguous `MTLBuffer`/`__constant`
  segments that match the manifest layout: `round_constants[round][state_width]`
  followed by the 3x3 MDS matrix.
- `fastpq_prover::poseidon_manifest()` loads and validates the snapshot at
  runtime (during Metal warm-up) so diagnostic tooling can assert that the
  shader constants match the published hash via
  `fastpq_prover::poseidon_manifest_sha256()`.
- SDK fixture readers (Swift `PoseidonSnapshot`, Android `PoseidonSnapshot`) and
  the Norito offline tooling rely on the same manifest, which prevents GPU-only
  parameter forks.

## Validation

1. After regenerating the manifest, run `cargo test -p xtask` to exercise the
   Poseidon fixture generation unit tests.
2. Record the new SHA-256 in this document and in any dashboards that monitor
   GPU artefacts.
3. `cargo test -p fastpq_prover poseidon_manifest_consistency` parses
   `poseidon2.metal` and `fastpq_cuda.cu` at build time and asserts that their
   serialized constants match the manifest, keeping the CUDA/Metal tables and
   the canonical snapshot in lock-step.

Keeping the manifest alongside the GPU build instructions gives the Metal/CUDA
workflows a deterministic handshake: kernels are free to optimize their memory
layout as long as they ingest the shared constants blob and expose the hash in
telemetry for parity checks.
