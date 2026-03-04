# soranet-handshake-harness

Work-in-progress harness for generating and validating SoraNet handshake
fixtures (see `docs/source/soranet_handshake_harness.md`).

## Usage

```
cargo run -p soranet-handshake-harness -- fixtures --out tests/interop/soranet/capabilities
cargo run -p soranet-handshake-harness -- fixtures --verify --out tests/interop/soranet/capabilities
```

- `inspect` — decode capability vectors and print transcript hash.
- `summary` — render TLV summaries from hex input.
- `salt` — produce a `SaltAnnouncementV1` JSON payload from CLI arguments.
- `telemetry` — emit a `SoraNetTelemetryV1` JSON payload. Use `--signature` /
  `--witness-signature` to supply precomputed values or `--relay-static-sk-hex`
  to derive deterministic Dilithium3 + Ed25519 signatures from a 32-byte static
  key.
- `salt-verify` — validate an existing salt fixture (`--vector <path>`), ensuring
  timestamp ordering, epoch continuity, salt length, and reporting whether a signature is present.
- `fixtures` — generate or verify the canonical capability, telemetry, and salt fixtures.
- `simulate` — compute transcript hash, capability warnings, and a fully
  simulated Noise XX message flow (including deterministic ML-KEM material,
  Ed25519/Dilithium signatures, 1024-byte padding, and a telemetry record).
  Use `--json-out` to persist the report, `--frames-out <dir>` to dump the raw
  binary frames, and `--telemetry-out` to dump the telemetry payload JSON.
- `cargo fuzz run handshake_state_machine` — exercises the relay-side parser with
  random client payloads to harden the Noise XX state machine against malformed input.

Example:

```
cargo run -p soranet-handshake-harness -- simulate \
  --client-hex 0101000201010102000201010202000200047f100004deadbeef7f110004cafebabe \
  --relay-hex 0101000201010102000201010103002076d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f02010001010202000200047f12000412345678 \
  --client-static-sk-hex 2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a \
  --relay-static-sk-hex d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de \
  --descriptor-commit-hex 76d0f4f511391e6548e6f9c80f30ed61c4cbbb98b5ecec922d8af67233f21f1f \
  --client-nonce-hex 2c1f64028dbe42410d1921cd9a316bed4f8f5b52ffb62b4dcaf149048393ca8a \
  --relay-nonce-hex d5f4f2f9c2b1a39e88bbd3c0a4f9e178d93e7bfacaf0c3e872b712f4a341c9de \
  --kem-id 1 \
  --sig-id 1 \
  --json-out - \
  --frames-out frames \
  --telemetry-out telemetry.json \
  --show-steps
```

Passing `--json-out <path>` writes the structured simulation report either to the
given file or to stdout when the path is `-`.
Use `--show-steps` to print the padded Noise XX handshake frames alongside
the textual summary.
Use `--only-capability <type>` (repeatable, accepts hex like `0x0101` or
decimal) to focus warnings and JSON output on specific capability IDs.
