# iroha_logger

Utilities for setting up logging in Hyperledger Iroha.

## Configuration

The [`Config`](src/lib.rs) structure controls logger behaviour:

- `level` – logging verbosity level.
- `filter` – additional filtering directives.
- `format` – output formatting style.
- `terminal_colors` – whether to emit ANSI colors to the terminal.

Telemetry fields matching the repository's sensitive-key taxonomy are always
redacted before emission. This first-release safety rule has no feature gate,
runtime bypass, or allow-list.
