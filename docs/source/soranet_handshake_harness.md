# SoraNet Handshake Harness Plan

This note tracks the `soranet-handshake-harness` crate used to validate the
handshake RFC (SNNet-1e deliverable). The harness now ships TLV parsing,
transcript hashing, deterministic Noise XX simulation (ML-KEM material +
dual-signature frames), salt/telemetry helpers (including SoraNetTelemetryV1 builders),
and CLI entry points wired into `cargo xtask soranet-fixtures`.

## Objectives

- Reproduce QUIC + Noise XX handshake with ML-KEM-768/1024 shares and transcript
  hashing.
- Verify capability TLV parsing (required vs optional, GREASE preservation).
- Exercise SaltAnnouncementV1 recovery flow and `SaltMismatch` handling.
- Emit DowngradeAlarmReportV1/SoraNetTelemetryV1 samples for telemetry checks.
- Generate signed fixture bundle under `fixtures/soranet_handshake/`.
- Publish deterministic NK2/NK3 KATs for downstream SDKs (Rust/Go/C++) alongside fuzz/perf harnesses.

## Components

1. **Harness binary** (`soranet-handshake-harness`): orchestrates capability TLV
   parsing, transcript hashing, deterministic Noise XX frame synthesis (with
   ML-KEM shares, Ed25519/Dilithium signatures, and 1024-byte padding), telemetry
   scaffolding, and fixture regeneration plus JSON export helpers (`--json-out`,
   `--telemetry-out`). The crate lives under
   `tools/soranet-handshake-harness`; the current CLI (`inspect`, `summary`,
   `salt`, `fixtures`, `simulate`) covers inspection and regression workflows.
2. **Transcript module:** provides the transcript hashing / dual-KDF inputs
   shared with the RFC spec so SDK parity tests can reuse the implementation.
3. **Telemetry module:** emits DowngradeAlarmReportV1 payloads and SoraNetTelemetryV1
   JSON blobs with deterministic Dilithium3 + Ed25519 signatures so relays/tests can
   exercise the telemetry pipeline end-to-end.
4. **CLI integration:** `cargo xtask soranet-fixtures` already shells out to the
   harness to regenerate or verify capability + salt fixtures. Once telemetry
   signing lands, the xtask will produce the canonical fixture bundle checked by
   CI.

## Test Matrix

- Success (ML-KEM-768, Dilithium3) — confirm transcript hash, padding, salt
  epoch acceptance.
- Downgrade (missing `snnet.pqkem`) — harness aborts and emits alarm.
- Descriptor digest mismatch — verify transcript commit enforcement.
- Salt recovery — client missing two epochs fetches announcements and resumes.
- Emergency rotation — validates incident logging and telemetry fields.
- NK2/NK3 KATs — cross-language handshake vectors consumed by Rust/Go/C++ SDK test suites.
- Noise XX state machine fuzz target — exercises relay-side parsing against adversarial payloads (`cargo fuzz run handshake_state_machine`).
- Performance gate — ensures NK2/NK3 simulations stay under the 900 ms P99 ceiling and within 15% mean latency variance in release builds (`tools/soranet-handshake-harness/tests/perf_gate.rs`). Debug builds retain the same P99 ceiling but allow a 35% mean envelope to account for instrumentation overhead; CI release runs continue to enforce the 15% gate.

## Deliverables

- `fixtures/soranet_handshake/capabilities/*.norito.json` — generated today via
  the harness CLI (unsigned reference fixtures).
- `fixtures/soranet_handshake/salt/*.norito.json` — sample
  `SaltAnnouncementV1` payloads emitted by the CLI.
- `fixtures/soranet_handshake/telemetry/*.norito.json` — DowngradeAlarmReportV1 and
  SoraNetTelemetryV1 payloads emitted by the CLI with deterministic Dilithium3 + Ed25519
  signatures (derived from the fixture signing key).
- `fixtures/soranet_handshake/interop/{rust,go,cpp}/snnet-interop-nk{2,3}-v1.json` — deterministic NK2/NK3 handshake vectors shared with Rust/Go/C++ SDKs (session keys, transcript hashes, confirmation tags).
- CI job invoking the harness via `cargo xtask soranet-fixtures --verify` to
  compare hashes against expected values (to be wired in once signing is ready).

## Timeline

- ✅ Harness skeleton + CLI wiring (delivered 2026-03-20)
- Fixture signing + CI integration: **2026-04-10**
- Status update + roadmap close-out: **2026-04-15**

## Current CLI snapshot

`cargo xtask soranet-fixtures` shells out to the harness today to keep fixtures
deterministic:

- `cargo xtask soranet-fixtures` — regenerates capability fixtures, downgrade
  telemetry, and salt announcements under
  `tests/interop/soranet/{capabilities,telemetry,salt}`.
- `cargo xtask soranet-fixtures --verify` — regenerates into a temp directory
  and fails if any capability, telemetry, or salt fixture differs.

The harness binary can also be executed directly:

- `inspect` — decode capability vectors, print transcript hash, and highlight
  missing required capabilities.
- `summary` — render structured TLV breakdown (required flags, GREASE, etc.).
- `salt` — emit `SaltAnnouncementV1` payloads for recovery drills.
- `telemetry` — render `SoraNetTelemetryV1` payloads for telemetry regression tests. Pass `--signature`/`--witness-signature` to supply precomputed envelopes or `--relay-static-sk-hex` to derive deterministic Dilithium3 + Ed25519 signatures.
- `fixtures` — regenerate or verify the reference fixture bundle.
  Current bundle includes `snnet-cap-006-constant-rate` so SDKs can assert the
  expected downgrade warning/telemetry slug when relays omit the constant-rate
  TLV.
- `simulate` — compute transcript hash + capability warnings given capability
  vectors, descriptor commit, nonces, negotiated KEM/signature IDs, and 32-byte
  X25519 static keys. Emits padded Noise XX frames, deterministic ML-KEM shares,
  hybrid signatures, and a telemetry JSON blob. Use `--json-out <path|->` to
  emit a structured report (stdout when `-`), `--frames-out <dir>` to persist
  the binary frames (`client_hello.bin`, etc.), `--telemetry-out <path>` to write
  the first telemetry payload with deterministic Dilithium3 + Ed25519 signatures,
  `--show-steps` to print the generated handshake timeline, and `--only-capability <type>` (repeatable) to filter warnings/output to specific capability IDs.
- `cargo fuzz run handshake_state_machine` — exercises the relay-side parser and Noise XX state machine against adversarial payloads with deterministic RNG seeding.
