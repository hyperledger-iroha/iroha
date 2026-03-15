## Norito-RPC Transport Brief (NRPC-1)

**Audience**: SDK leads, Torii Platform, Docs/DevRel  
**Published**: 2026-03-18 (references `docs/source/torii/norito_rpc.md`)

### Why it matters
- Norito-RPC provides deterministic, checksum-protected messaging for all Torii RPC calls.
- It keeps `/v2/pipeline` paths but moves clients toward a binary transport with schema hashes.
- RFC sign-off unblocks NRPC-2 (server rollout plan) and NRPC-3 (Android client architecture).

### What changed
- Transport RFC defines request/response negotiation, Norito header expectations, compression rules, and error semantics.
- Client helpers exist for Rust, Python, Android; JS/Swift parity scheduled via tracker.
- Telemetry and rate limiting already tag Norito sessions (`scheme="norito_rpc"`).

### Immediate actions
| Owner | Action | Deadline | Link |
| ----- | ------ | -------- | ---- |
| Torii Platform TL | Review RFC, confirm NRPC-2 scope doc outline | 2026-03-24 | `docs/source/torii/norito_rpc_sync_notes.md` |
| SDK Program Lead | Ensure SDK representatives update tracker with parity checkpoints | 2026-03-25 | `docs/source/torii/norito_rpc_tracker.md` |
| Docs/DevRel | Publish developer portal overview and Try-It updates | 2026-03-25 | `docs/portal/docs/devportal/torii-rpc-overview.md` |

### Key references
- Transport RFC: `docs/source/torii/norito_rpc.md`
- Tracker: `docs/source/torii/norito_rpc_tracker.md`
- Portal doc (public facing): `docs/portal/docs/devportal/torii-rpc-overview.md`

### Mock Harness Contract (2026-06-18 workshop outcome)
- **Purpose:** Provide a reusable Torii mock harness so Android AND4, Swift IOS3, and JS JS4 can share the same deterministic fixtures, retry semantics, and telemetry hooks during NRPC rollouts.
- **Implementation:** Source of truth lives in the Android test harness (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/mock/ToriiMockServer.java`) and is extracted into the workspace-wide CLI under `tools/torii_mock_harness/`.
- **Fixture governance:** Harness consumes the Norito fixture catalogue produced by `scripts/android_fixture_regen.sh` and recorded in `docs/source/android_fixture_changelog.md`. Clients must bump the fixture version string (`mock_harness.fixture_version`) whenever schema hashes change.
- **Telemetry requirements:** Every harness-backed CI job must emit:
  - `torii_mock_harness_retry_total{sdk}` — retries triggered by injected failures.
  - `torii_mock_harness_duration_ms{sdk,scenario}` — end-to-end duration per scenario (submit, query, governance, etc.).
  - `torii_mock_harness_fixture_version{sdk}` — gauge/label recording the Norito fixture bundle hash.
- **CI integration:** Each SDK adds a dedicated “Mock Harness Smoke” lane that executes the CLI (`tools/torii_mock_harness/bin/torii-mock-harness replay <scenario>`), reports telemetry via OpenTelemetry, and publishes green badges referenced from `status.md`.
- **Documentation:** Workshop notes (`docs/source/torii/archive/meetings/2026-06-18/notes.md`) and the CLI README describe installation, configuration, and the metrics above. Updates to the contract must be mirrored in this brief and announced in `#sdk-council`.

### Open questions (to close in 2026-03-21 sync)
1. Do we require dual-stack staging before enabling Norito responses in production?
2. Should the SDK parity checklist include explicit sample payloads per endpoint?
3. How do we communicate fallback guidance for operators that cannot serve Norito yet?
