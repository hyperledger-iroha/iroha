---
lang: am
direction: ltr
source: docs/source/soranet/reports/circuit_stability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8855cc70cf88dcee3fe157b2521de282a026b245b699970ee3d16387a2ef9039
source_last_modified: "2025-12-29T18:16:36.194825+00:00"
translation_last_reviewed: 2026-02-07
---

# Circuit Stability Soak Report

This soak exercise validates the new SoraNet circuit lifecycle manager shipped
for SNNet-5. The harness is implemented in
`crates/sorafs_orchestrator/src/soranet.rs` under the
`circuit_manager_soak_maintains_latency_stability` test and simulates three
guard rotations with deterministic latency samples.

| Rotation | Guard Relay | Samples (ms)     | Average (ms) | Min (ms) | Max (ms) |
|----------|-------------|------------------|--------------|----------|----------|
| 0        | 0x04…04     | 50, 52, 49       | 50.33        | 49       | 52       |
| 1        | 0x04…04     | 51, 53, 50       | 51.33        | 50       | 53       |
| 2        | 0x04…04     | 52, 54, 51       | 52.33        | 51       | 54       |

The rotation history stays within a 2 ms window over three renewals, meeting the
roadmap requirement that latency remains stable across at least three guard
rotations. The test also asserts that the manager records deterministic rotation
telemetry and enforces the configured TTL.
