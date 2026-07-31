# Hyperledger Iroha

[![GitHub License](https://img.shields.io/github/license/hyperledger-iroha/iroha)](./LICENSE)
[![OpenSSF Best Practices](https://www.bestpractices.dev/projects/960/badge)](https://www.bestpractices.dev/projects/960)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/hyperledger/iroha/badge)](https://scorecard.dev/viewer/?uri=github.com/hyperledger/iroha)

Hyperledger Iroha 3 is a deterministic blockchain platform for public,
permissioned, and consortium deployments. It provides account and asset
management, on-chain permissions, and smart contracts through the Iroha
Virtual Machine (IVM).

> Workspace status and recent changes are tracked in [`status.md`](./status.md).

## First-release Scope

Iroha 3 is the first public release described by this repository. Workspace
defaults, quickstarts, SDK guidance, and operator documentation target Iroha 3,
where data availability and reliable broadcast are consensus requirements.
Public and in-depth guidance is maintained in the sibling `iroha-docs`
repository; this repository retains only concise contributor notes and
code-adjacent specifications.

## Repository Layout

- [`crates/`](./crates): core Rust crates (`iroha`, `irohad`, `iroha_cli`, `iroha_core`, `ivm`, `norito`, etc.).
- [`integration_tests/`](./integration_tests): cross-component network/integration tests.
- [`IrohaSwift/`](./IrohaSwift): Swift SDK package.
- [`java/iroha_android/`](./java/iroha_android): Android SDK package.
- [`docs/`](./docs): concise repository-local and code-adjacent documentation;
  public Iroha 3 documentation is maintained in
  [`iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs).

## Quickstart

### Prerequisites

- Rust 1.93.1, pinned by [`rust-toolchain.toml`](./rust-toolchain.toml)
- Optional: Docker + Docker Compose for local multi-peer runs

### Build and Test (Workspace)

```bash
cargo test
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
```

Notes:

- Plain `cargo test` from the repository root runs the default workspace
  members; use `cargo test --workspace` when an explicit full-workspace run is
  needed. Use `cargo test -p <crate>` for a focused crate suite.
- The repo-local Cargo config caps default build parallelism at `jobs = 1` so
  large Rust test builds do not overcommit WSL or memory-constrained VMs. On a
  high-memory machine, override this with `cargo test -j <N>` or
  `CARGO_BUILD_JOBS=<N> cargo test`. The dev/test profiles keep the large
  `iroha_data_model` crate at eight codegen units: 8-, 16-, and 64-unit builds
  had approximately the same peak, while one unit was substantially worse
  because coalescing its generated decoder IR creates a larger LLVM module.
  Sharing the derive-heavy canonical/prefix decoder control flow reduced the
  exact focused-build high-water mark to 11.466 GiB, but this is still a large
  single-process frontend/monomorphization load. One Cargo job does not change
  the internal partition count.
- Plain `cargo test` skips the oversized private Sumeragi main-loop unit-test
  harness so local WSL runs do not need a ~10 GiB `iroha_core --test` compile.
  Run `cargo test -p iroha_core --lib --features sumeragi-main-loop-tests` on a
  high-memory host when changing that private consensus harness.
- Full workspace build can take about 20 minutes.
- Full workspace tests can take multiple hours.
- On WSL, make sure the Windows-side `.wslconfig` gives the VM enough memory,
  swap, and host disk headroom for a large Rust workspace. Plain `cargo test`
  is now conservative by default; if the VM is still constrained, use
  `scripts/run_full_tests.sh --wsl-safe --target-dir /tmp/iroha-wsl-tests` to
  also serialize libtest execution.
- The workspace targets `std` (WASM/no-std builds are not supported).
- Heavier local UI/media helpers are explicit features in default builds:
  `cargo run -p mochi-ui --features gui` for the egui desktop shell and
  `cargo run -p iroha_cli --features offline-visual-codecs -- ...` for Petal
  visual-codec commands. The SoraFS browser/SDK local QUIC proxy is available
  with `cargo build -p sorafs_orchestrator --features local-quic-proxy`.

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
- `GET /v1/pipeline/preflight`
- `GET /metrics`
- `GET /v1/parameters`
- `GET /v1/events/sse`

For liveness checks, prefer the queue-aware fields in `/status`: use
`queue_size` as the gate and compare `time_since_last_block_ms` or
`time_since_last_non_empty_block_ms` against the
`/v1/pipeline/preflight.sumeragi.stall_threshold_ms` value. An old block
timestamp alone is not a stall when the queue is empty.

See the full endpoint reference in the
[public Iroha documentation](https://docs.iroha.tech/reference/torii-endpoints.html).

Contract deployment is a locally signed consensus transaction flow. Clients
upload and finalize bytecode, register the locally signed manifest, then submit
`CommitContractDeployment` with the exact expected authority nonce and previous
alias target. Torii never accepts deployment private keys and does not expose a
server-side deploy or deploy-bundle route. The maintained public HTTP paths are:

- `GET /v1/contracts/code/{code_hash}` and
  `GET /v1/contracts/code-bytes/{code_hash}` for registered artifacts
- `POST /v1/contracts/aliases/resolve` for the current signed alias binding
- `POST /v1/contracts/view/batch` for batched read-only contract queries in one
  round-trip

For the public-safe Torii posture, contract call/view/status routes stay public,
while higher-risk app-facing surfaces are opt-in:

- `torii.webhooks_enabled = false` by default
- `torii.zk_attachments_enabled = false` by default
- trader/app rollups such as `/v1/contracts/rollups/swaps/fills` and
  `/v1/contracts/rollups/trader/account` remain app-facing surfaces rather than
  part of the public-safe baseline
- enable them explicitly when the node is meant to expose those app features

## Codex Integration

This repo includes Codex-facing SORA live-network surfaces:

- [`plugins/iroha/`](./plugins/iroha): an installable Codex app/plugin with the
  built-in Taira MCP preset.
- [`skills/sora-taira-testnet/`](./skills/sora-taira-testnet): a standalone
  Codex skill for live Taira testnet workflows.
- [`skills/sora-minamoto-mainnet/`](./skills/sora-minamoto-mainnet): a
  standalone Codex skill for live Minamoto mainnet workflows.

Install a standalone skill from a GitHub checkout of this repo with:

```bash
python3 "${CODEX_HOME:-$HOME/.codex}"/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo <owner>/<repo> \
  --path skills/sora-taira-testnet

python3 "${CODEX_HOME:-$HOME/.codex}"/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo <owner>/<repo> \
  --path skills/sora-minamoto-mainnet
```

Restart Codex after installation so the selected skill appears in the Skills
tab.

If you are operating the public Taira deployment itself, render per-validator
configs from `configs/soranexus/taira/validator_roster.example.toml` plus
`configs/soranexus/taira/validator_secrets.example.toml` with
`python3 scripts/render_taira_validator_bundle.py --roster ... --secrets ... --output-dir ...`
instead of cloning the checked-in peer-1 `config.toml` by hand. The secrets
template now also carries the shared onboarding/faucet authority and streaming
identity material, so the checked-in Taira config remains a template rather
than a secret-bearing runtime profile.

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

## Documentation

Public and in-depth Iroha 3 documentation is published at
[docs.iroha.tech](https://docs.iroha.tech/) from the
[`hyperledger-iroha/iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs)
repository.

This repository keeps concise contributor guidance and documentation coupled to
the implementation, including the [`docs/` index](./docs/README.md), the
[Norito wire-format specification](./norito.md), crate and SDK READMEs, formal
artifacts, and the current [`status.md`](./status.md) and
[`roadmap.md`](./roadmap.md). Building or testing Iroha does not require a
sibling `iroha-docs` checkout.

The protected Stage-6 scratch source gives proof bodies through non-Completion
node/candidate capacity descent, fair causal admission, no-debt acquisition,
the Completion admissible bridge, and
`FairProtectedStage6RankProgress`. The protected Stage-2 scratch source adds
reachable one-/two-step Busy-phase descent, the production Busy fence,
deferred-drain debt, and target-specific class-prefix/cursor descent in
`ProtectedStage2RankProgressFromFenceObligation`. Both are explicit
`specified_unproved` proof-ledger leaves: a source proof body, SANY acceptance,
or parse-only obligation generation is not strict TLAPS evidence, and neither
leaf has a fresh pinned strict-TLAPS receipt.

The asynchronous proof graph is deliberately acyclic.
`SumeragiV2AsyncLivenessProofs` is the proof-bearing base and contains no
declaration-only theorem. The Stage-2, Stage-3, and Stage-6 leaves extend only
that base. `SumeragiV2AsyncRankClosureProofs` then gives source proof bodies for
the aggregate protected-service rank and starvation theorems; both remain
`specified_unproved` until fresh strict receipts exist.
The progress-witness chain now continues through
`SumeragiV2DecisionWitnessPreservationProofs`,
`SumeragiV2HistoricalLockedBodyWitnessPreservationProofs`, and
`SumeragiV2ProgressWitnessFinalClosureProofs`; together they give a source
proof of the inductive progress-witness theorem. The release ledger entry
nevertheless remains `specified_unproved` until a fresh strict TLAPS receipt
is bound to this source. `SumeragiV2AsyncTemporalClosureProofs` imports that
final closure plus five proof-bearing temporal reduction leaves. It retains
exactly three declaration-only kernels:
`HeightProductivityResetBoundaryObligation`,
`AdequateLeaderServiceKernelObligation`, and
`ExactDecisionStageServiceObligation`. The release-facing height-productivity,
rotating-leader, timeout/view, locked-body reproposal, and per-validator
application-completion obligations now have source proof bodies reducing them
to those kernels; timeout/view and locked-body are not independent debts.
Chain/epoch refinement routes through that temporal layer. No ledger entry is
promoted without a fresh strict receipt. The current 60-obligation ledger records
28 `tlaps_proved`, 25 `specified_unproved`, 6 `trusted_contract`, and 1
`out_of_scope`; `machine_checked_completion` remains false.

## Translations

Localized public documentation is published at
[docs.iroha.tech](https://docs.iroha.tech/) and maintained in
[`iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs).

## Contributing and Help

- Contribution guide: [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- Community/support information: [public Iroha documentation](https://docs.iroha.tech/)
- Security policy: [`SECURITY.md`](./SECURITY.md)

## License

Iroha is licensed under Apache-2.0. See [`LICENSE`](./LICENSE).

Documentation is licensed under CC-BY-4.0: http://creativecommons.org/licenses/by/4.0/
