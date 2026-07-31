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
