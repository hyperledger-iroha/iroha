# Hyperledger Iroha

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Hyperledger Iroha is a deterministic blockchain platform for permissioned and consortium deployments. It provides account/asset management, on-chain permissions, and smart contracts through the Iroha Virtual Machine (IVM).

> Workspace status and recent changes are tracked in [`status.md`](./status.md).

## Release Tracks

This repository ships two deployment tracks from the same codebase:

- **Iroha 2**: self-hosted permissioned/consortium networks.
- **Iroha 3 (SORA Nexus)**: the Nexus-oriented deployment track using the same core crates.

Both tracks share the same core components, including Norito serialization, Sumeragi consensus, and the Kotodama -> IVM toolchain.

## Repository Layout

- [`crates/`](./crates): core Rust crates (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito`, etc.).
- [`integration_tests/`](./integration_tests): cross-component network/integration tests.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK package.
- [`java/iroha_android/`](./java/iroha_android): Android SDK package.
- [`docs/`](./docs): user/operator/developer documentation.

## Quickstart

### Prerequisites

- [Rust stable](https://www.rust-lang.org/tools/install)
- Optional: Docker + Docker Compose for local multi-peer runs

### Build and Test (Workspace)

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Notes:

- Full workspace build can take about 20 minutes.
- Full workspace tests can take multiple hours.
- The workspace targets `std` (WASM/no-std builds are not supported).

### Targeted Test Commands

```bash
cargo test -p <crate>
cargo test -p <crate> <test_name> -- --nocapture
```

### SDK Test Commands

```bash
cd IrohaSwift
swift test
```

```bash
cd java/iroha_android
JAVA_HOME=$(/usr/libexec/java_home -v 21) \
ANDROID_HOME=~/Library/Android/sdk \
ANDROID_SDK_ROOT=~/Library/Android/sdk \
./gradlew test
```

## Run a Local Network

Start the provided Docker Compose network:

```bash
docker compose -f defaults/docker-compose.yml up
```

Use the CLI against the default client config:

```bash
cargo run --bin iroha -- --config ./defaults/client.toml --help
```

For daemon-specific native deployment steps, see [`crates/irohad/README.md`](./crates/irohad/README.md).

## API and Observability

Torii exposes both Norito and JSON APIs. Common operator endpoints:

- `GET /status`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

See the full endpoint reference in:

- [`docs/source/telemetry.md`](./docs/source/telemetry.md)
- [`docs/portal/docs/reference/README.md`](./docs/portal/docs/reference/README.md)

## Core Crates

- [`crates/iroha`](./crates/iroha): client library.
- [`crates/irohad`](./crates/irohad): peer daemon binaries.
- [`crates/iroha_cli`](./crates/iroha_cli): reference CLI.
- [`crates/iroha_core`](./crates/iroha_core): ledger/core execution engine.
- [`crates/iroha_config`](./crates/iroha_config): typed configuration model.
- [`crates/iroha_data_model`](./crates/iroha_data_model): canonical data model.
- [`crates/iroha_crypto`](./crates/iroha_crypto): cryptographic primitives.
- [`crates/norito`](./crates/norito): deterministic serialization codec.
- [`crates/ivm`](./crates/ivm): Iroha Virtual Machine.
- [`crates/iroha_kagami`](./crates/iroha_kagami): key/genesis/config tooling.

## Documentation Map

- Main docs index: [`docs/README.md`](./docs/README.md)
- Genesis: [`docs/genesis.md`](./docs/genesis.md)
- Consensus (Sumeragi): [`docs/source/sumeragi.md`](./docs/source/sumeragi.md)
- Transaction pipeline: [`docs/source/pipeline.md`](./docs/source/pipeline.md)
- P2P internals: [`docs/source/p2p.md`](./docs/source/p2p.md)
- IVM syscalls: [`docs/source/ivm_syscalls.md`](./docs/source/ivm_syscalls.md)
- Kotodama grammar: [`docs/source/kotodama_grammar.md`](./docs/source/kotodama_grammar.md)
- Norito wire format: [`norito.md`](./norito.md)
- Current work tracking: [`status.md`](./status.md), [`roadmap.md`](./roadmap.md)

## Translations

Japanese overview: [`README.ja.md`](./README.ja.md)

Other overviews:
[`README.he.md`](./README.he.md), [`README.es.md`](./README.es.md), [`README.pt.md`](./README.pt.md), [`README.fr.md`](./README.fr.md), [`README.ru.md`](./README.ru.md), [`README.ar.md`](./README.ar.md), [`README.ur.md`](./README.ur.md)

Translation workflow: [`docs/i18n/README.md`](./docs/i18n/README.md)

## Contributing and Help

- Contribution guide: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Community/support channels: [`CONTRIBUTING.md#contact`](./CONTRIBUTING.md#contact)

## License

Iroha is licensed under Apache-2.0. See [`LICENSE`](./LICENSE).

Documentation is licensed under CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/
