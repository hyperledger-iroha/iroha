# iroha_logger

Utilities for setting up logging in Hyperledger Iroha.

## Configuration

The [`Config`](src/lib.rs) structure controls logger behaviour:

- `level` – logging verbosity level.
- `filter` – additional filtering directives.
- `format` – output formatting style.
- `terminal_colors` – whether to emit ANSI colors to the terminal.

## Features

- `log-obfuscation` – redact telemetry fields that match sensitive keywords before emitting them
  (enabled by default).

Telemetry redaction policy (strict vs allow-list vs disabled) is configured via
`iroha_config.telemetry_redaction`.
