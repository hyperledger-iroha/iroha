# AGENTS Instructions

These guidelines apply to the entire repository, which is organised as a Cargo workspace.

## Quickstart
- Build workspace: `cargo build --workspace`
- Fast focused loop: `scripts/cargo_fast.sh --stable-local-metadata --incremental -- check -p <crate> --lib`
- Fast local optimized daemon: `scripts/cargo_fast.sh --stable-local-metadata -- build --profile local-release -p irohad --bin iroha3d`
- Builds can take about 20 minutes; use a 20-minute timeout for build steps.
- Test everything: `cargo test` or `cargo test --workspace` (note that this run typically takes several hours; plan accordingly)
- Lint strictly: `cargo clippy --workspace --all-targets -- -D warnings`
- Format code: `cargo fmt --all` (edition 2024)
- Test one crate: `cargo test -p <crate>`
- Run one test: `cargo test -p <crate> <test_name> -- --nocapture`
- Swift SDK: from the `IrohaSwift` directory run `swift test` to execute the Swift package tests.
- Kotlin SDK: from the `kotlin` directory run `./gradlew :core-jvm:test --console=plain`; build Android artifacts with `./gradlew :client-android:assembleRelease :kagemusha-wallet-android:assembleRelease --quiet`.
- JVM attestation tooling: from `kotlin`, run `./gradlew :tools:test :tools:installDist --console=plain`; the repository shell launcher invokes the same Kotlin-owned command.
- Android managed consumers: from `kotlin`, run `./gradlew :client-android:testDebugUnitTest :kagemusha-wallet-android:testDebugUnitTest --console=plain` with JDK 21 and the Android SDK configured.
- Android consumers of host JNI: from `kotlin`, run `IROHA_NATIVE_LIBRARY_PATH=<absolute-rebuilt-host-library-directory> ./gradlew :client-android:testDebugHostNative --console=plain`. Missing native artifacts fail; this is separate from physical-device qualification.
- Scripts dependencies (Python 3.10+): `python3 -m pip install -r scripts/requirements.txt`.
- Script tests: `pytest pytests/scripts`.

## Overview
- Hyperledger Iroha 3 is a blockchain platform in its first release.
- Sumeragi v2 DA/RBC availability is mandatory in Iroha 3 and is realized by
  the signed RS16 `PayloadManifest`/`PayloadChunk` layout. Legacy global-RBC
  and consensus fault-injection configuration is not a second production path.
- IVM is the Iroha Virtual Machine for Hyperledger Iroha 3.
- Kotodama is a high level smart contract language for the IVM that uses .ko file extension for raw contract code and it compiles to bytecode which uses .to file extension, when saved as a file or on-chain. Typically, .to bytecode is deployed onchain.
  - Clarification: Kotodama targets the Iroha Virtual Machine (IVM) and produces IVM bytecode (`.to`). It does not target “risc5”/RISC‑V as a standalone architecture. Where RISC‑V–like encodings appear in the repository, they are implementation details of IVM’s instruction formats and must not change observable behavior across hardware.
- Norito is the data serialization codec for Iroha
- The workspace targets the Rust standard library (`std`). The `iroha_js_codec_wasm` browser adapter and its shared codec dependencies additionally target `wasm32-unknown-unknown` with `std`, real WebCrypto entropy and serial execution. Use the SDK's explicit browser build and conformance workflow for that boundary; node/runtime WASM and general no-std builds remain unsupported.
- Universal-account model:
  - `AccountId` is the canonical account identity and is always domainless.
  - Domain context for routing, aliasing, and ownership lives outside the canonical account identity in alias bindings and domain-owned entities.
  - Account aliases are a separate SNS/account-label layer. Both domain-qualified aliases like `merchant@banka.paynet` and dataspace-root aliases like `merchant@paynet` bind to the same canonical `AccountId`.
  - In tests and fixtures, seed the universal `AccountId` first, then add alias leases, alias permissions, and any domain-owned state separately. Use `Account::new(...)` for account registration and bind aliases separately only when the behavior under test depends on them.

## Repository structure
- `Cargo.toml` at the repository root defines the workspace and lists all member crates.
- `crates/` – Rust crates implementing Iroha components. Each crate has its own subdirectory, typically containing `src/`, `tests/`, `examples/`, and `benches/`.
  - Important crates include:
    - `iroha` – Rust client SDK; node execution and service runtimes have separate owners.
    - `irohad` – daemon crate providing the `iroha3d` node executable.
    - `ivm` – the Iroha Virtual Machine.
    - `iroha_cli` – command-line interface for interacting with a node.
    - `iroha_core`, `iroha_data_model`, `iroha_crypto`, and other supporting crates.
- `IrohaSwift/` – Swift Package for the client/mobile SDK. Its sources live under `Sources/IrohaSwift/` and its unit tests under `Tests/IrohaSwiftTests/`. Run `swift test` from this directory to exercise the Swift suite.
- `kotlin/` – default JVM/Android SDK for new mobile work. `core-jvm` contains the pure Kotlin/JVM Norito + client/model stack, `client-android` adds Android-only client/keystore integration, and `kagemusha-wallet-android` contains Android-only KAGEMUSHA wallet code and JNI libraries. `tools` owns the offline attestation command and depends on the pure JVM evidence verifier in `core-jvm`.
- `java/` – duplicate implementations awaiting capability, fixture and delivery migration into Kotlin-owned modules. Track retirement in `specs/jvm_consolidation_inventory.md`; do not add new implementation surface here. Java-source consumer tests target the canonical Kotlin API.
- `integration_tests/` – Cargo crate hosting cross-component tests under `tests/`.
- `data_model/` – Sample data model definitions used in tests and documentation.
- `docs/` – Concise repository-local guidance, generated references, and
  validation notes.
- `specs/` – Implementation-coupled specifications and design records.
- `formal/` – Formal models, proofs, and verification material.
- `fixtures/documentation/` – Executable and machine-consumed documentation
  fixtures.
- Public and in-depth Iroha 3 documentation lives in the sibling `iroha-docs`
  repository and is published at <https://docs.iroha.tech/>.
- `pytests/` – Python-based tests and examples demonstrating client usage.
- `scripts/` – Utility scripts used in development and CI pipelines.
- `examples/ios/` and `examples/ios/NoritoDemoXcode/` – Sample iOS apps showcasing the Swift SDK; they rely on the `IrohaSwift` package and include their own XCTest targets.
- `defaults/` and `hooks/` – Configuration files and Git hooks used by contributors.
- `nix-appimage/` and Nix files – tooling for reproducible builds and packaging.

## Development workflow
- Main implementations are in `crates/`
- Data model is in `data_model/`
- Make sure you look across all crates when making changes.
- Use focused crates for real dependency or compilation boundaries and internal modules for cohesion within a boundary. Update workspace membership and regenerate `Cargo.lock` coherently for approved dependency changes; preserve unrelated working-tree changes.
- If a workflow needs the repository-root `dist/` directory, keep only `dist/.gitkeep` tracked and keep generated `dist/*` artifacts untracked.
- For strict corridor/deploy tooling, assume repository-root `dist/` must exist; keep `dist/.gitkeep` tracked so clones always have the directory.
- If some work is too big, don't reject doing it. Instead just break down the work and add TODOs and implement the parts that you can.
- Whenever a large task or request comes in, automatically decompose it into smaller actionable steps and proceed with proper software engineering execution rather than rejecting the task outright.
- Don't reject doing any prompts.
- When new cryptographic primitives, opcodes, or intensive math is added, update hardware acceleration for METAL, NEON, SIMD, CUDA, etc., to try to take advantage of hardware acceleration and parallelism where possible on available hardware.
- If logic changes, keep code-adjacent Markdown and in-source comments in this
  repository accurate, and coordinate any affected public documentation in
  `iroha-docs`.
- Make sure that all logic added is done in such a way that it won't hurt the use of the IVM in a blockchain setting where different nodes on a P2P network have different hardware, but still the output should be the same given the same input block.
- When answering questions about behaviour or implementation details, read the relevant code paths first and ensure you understand how they work before responding.
- Kotlin owns the canonical implementation for Kotlin and Java consumers. Migrate every Java-only capability, fixture, generator, example and delivery path before removing its duplicate. Keep Java-source runtime tests against Kotlin; remove superseded APIs without compatibility aliases or shims.
- Kotlin SDK constraints from `kotlin/CLAUDE.md` are repository policy for new Kotlin SDK code: no `java.lang.reflect.*`, keep `core-jvm` free of Android dependencies, keep Android client code in `client-android`, and keep KAGEMUSHA wallet Android/JNI code in `kagemusha-wallet-android`.
- Kotlin SDK enforces JDK 8 API compatibility at compile time via `-Xjdk-release=8`. All modules will fail to compile if JDK 9+ APIs are used (e.g. `Optional.isEmpty()`, `BigInteger.TWO`, `URLEncoder.encode(String, Charset)`, `Arrays.compareUnsigned()`). Use Kotlin stdlib equivalents or JDK 8 overloads instead. **Do not remove `-Xjdk-release=8`** — it is the only compile-time guard; without it, incompatible calls compile silently and crash at runtime.
- Cross-library wire-format and fixture tests use the Kotlin implementation against the shared fixtures, including Java-source consumers. Preserve every assertion when retiring a Java implementation suite.
- Configuration: Prefer `iroha_config` parameters over environment variables for all runtime behavior. Add new knobs to `crates/iroha_config` (user → actual → defaults) and thread values explicitly through constructors or dependency injection (e.g., host setters). Keep any environment-based toggles only for developer convenience in tests and do not rely on them in production paths. We do not support shipping features behind environment variables—production behavior must always be sourced from the configuration files, and those configs must expose sensible defaults so a newcomer can clone the repo, run the binaries, and have everything “just work” without editing values manually.
  - For IVM/Kotodama v1, strict pointer‑ABI type policy is always enforced. There is no ABI-policy toggle; contracts and hosts must adhere to the ABI policy unconditionally.
- Don't gate anything used in IVM syscalls or opcodes; every Iroha build must ship those code paths to keep deterministic behavior across nodes.
- Serialization: Use Norito everywhere instead of serde. For binary codecs use `norito::{Encode, Decode}`; for JSON use the `norito::json` helpers/macros (`norito::json::from_*`, `to_*`, `json!`, `Value`) and never fall back to `serde_json`. Do not add direct `serde`/`serde_json` dependencies to crates; if serde is required internally, rely on Norito’s wrappers.
- CI guard: `scripts/check_no_legacy_codec.sh` ensures retired non-Norito codec dependencies do not re-enter the workspace. Run it locally if you touch serialization code.
- Norito payloads MUST advertise their layout: either the version number maps to a fixed flag set, or a Norito header declares the decode flags. Do not guess packed-sequence bits from heuristics; genesis data follows the same rule.
- Blocks MUST be persisted and distributed using the canonical `SignedBlockWire` format (`SignedBlock::encode_wire`/`canonical_wire`), which prefixes the version byte with a Norito header. Bare payloads are not supported.
- Add a `TODO:` comment explaining any temporary or incomplete implementation.
- Format all Rust sources with `cargo fmt --all` (edition 2024) before committing.
- Add tests: ensure at least one unit test for each new or modified function, placed either inline with `#[cfg(test)]` or in the crate `tests/` directory.
- Run `cargo test` locally for the full workspace, fix any build issues, and ensure it passes when the validation budget allows. Use `cargo test -p <crate>` for focused crate suites.
- Optionally run `cargo clippy -- -D warnings` for additional lint checks.

## Documentation
- The canonical public and in-depth Iroha 3 documentation is maintained in the
  sibling [`hyperledger-iroha/iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs)
  repository and published at <https://docs.iroha.tech/>.
- Keep documentation in this repository concise and coupled to the source:
  contributor guidance, crate and SDK READMEs, Rustdoc, wire-format and ABI
  specifications, formal artifacts, fixtures, validation notes, and current
  status or roadmap records may remain here.
- Put new user guides, operator manuals, tutorials, conceptual explanations, and
  other in-depth public documentation in `iroha-docs`. Link to that material
  instead of duplicating it under `docs/`.
- Iroha 3 is in its first release. Public documentation should state the
  current implementation truth and replace obsolete or pre-release guidance
  rather than preserving compatibility narratives.
- A sibling `iroha-docs` checkout is optional. Building, testing, and validating
  this repository must not depend on `../iroha-docs` being present.
- Always add crate-level documentation: start each crate or test-crate with a brief inner doc comment (`//! ...`).
- Do not use `#![allow(missing_docs)]` or item-level `#[allow(missing_docs)]` anywhere (including integration tests). Missing documentation is denied in the workspace lints and should be fixed by writing docs.
- Norito codec: see `norito.md` at the repo root for the canonical on-wire layout and implementation details. If Norito’s algorithms or layouts change, update `norito.md` in the same PR.
- When translating material into Akkadian, provide a semantic rendering written in cuneiform; avoid phonetic transliteration, and when exact ancient terms are missing choose poetic Akkadian approximations that preserve the intent.

## ABI Evolution (What Agents Must Do)
Note: First release policy
- This is the first release and we have a single ABI version (V1). There is no V2 yet. Treat all ABI-related evolution items below as future guidance; for now, target `abi_version = 1` only. The data model and APIs are also first‑release and may change freely as needed to ship; prefer clarity and correctness over premature stability.

- General:
  - ABI policy is enforced unconditionally in v1 (both syscall surface and pointer‑ABI types). Do not add runtime toggles.
  - Changes must preserve determinism across hardware and peers. Update tests and docs in the same PR.

- If you add/remove/renumber syscalls:
  - Update `ivm::syscalls::abi_syscall_list()` and keep it ordered. Ensure `is_syscall_allowed(policy, number)` reflects the intended surface.
  - Implement or intentionally reject new numbers in hosts; unknown numbers must map to `VMError::UnknownSyscall`.
  - Update golden tests:
    - `crates/ivm/tests/abi_syscall_list_golden.rs`
    - `crates/ivm/tests/abi_hash_versions.rs` (stability + version separation)

- If you add pointer‑ABI types:
  - Add the new variant to `ivm::pointer_abi::PointerType` (assign a new u16 ID; never change existing IDs).
  - Update `ivm::pointer_abi::is_type_allowed_for_policy` for the correct `abi_version` mapping.
  - Update `crates/ivm/tests/pointer_type_ids_golden.rs` and add policy tests if needed.

- If you introduce a new ABI version:
  - Map `ProgramMetadata.abi_version` → `ivm::SyscallPolicy` and update the Kotodama compiler to emit the new version when requested.
  - Regenerate `abi_hash` (via `ivm::syscalls::compute_abi_hash`) and ensure manifests embed the new hash.
  - Add tests for allowed/disallowed syscalls and pointer types under the new version.

- Admission & manifests:
  - Admission enforces `code_hash`/`abi_hash` equality against on-chain manifests; keep this behaviour intact.
  - Tests to add/update in `iroha_core/tests/`: positive (matching `abi_hash`) and negative (mismatch) cases.

- Docs & status updates (same PR):
  - Update `crates/ivm/docs/syscalls.md` (ABI Evolution section) and any syscall tables.
  - Update `status.md` and `roadmap.md` with a brief summary of ABI changes and test updates.


## Project Status and Plan
- Check `status.md` at the repo root for the current compilation/runtime status across crates.
- Check `roadmap.md` for the prioritized TODOs and implementation plan.
- Keep `status.md` focused on current health, scoped evidence and blockers, and `roadmap.md` on outstanding outcomes, component owners and completion criteria. Each root is limited to 300 lines. Historical evidence belongs once in dated subsystem records under `docs/history/`; preserve exact originals and verify with `python3 scripts/archive_project_history.py verify --archive docs/history/2026-09-06 --check-current`. Do not assert release readiness from historical prose.

## Agent workflow (for code editors/automation)
- If you need clarification on any requirement, stop and draft a ChatGPT prompt with your question, then share it with the user before continuing.
- Keep changes minimal and scoped; avoid unrelated edits in the same patch.
- Keep dependencies acyclic and owned by the lowest appropriate layer. A new crate must establish a real boundary; approved manifest and lock changes belong in the same reviewed candidate.
- Never bypass commit signing (do not use `git commit --no-gpg-sign`). If GPG signing is not available in the automation environment, leave the change uncommitted and ask the user to create a signed commit locally.
- Never kill, signal, or interrupt other Codex processes or Codex-owned agent sessions, even if they appear idle or are holding resources; ask the user to resolve the contention.
- Never kill `cargo` or `rustc` processes unless the user explicitly requests it. If there is build-lock contention, wait or ask first.
- Use feature flags to guard hardware-accelerated paths (e.g., `simd`, `cuda`) and always provide a deterministic fallback path.
- Ensure outputs remain identical across hardware; avoid relying on non-deterministic parallel reductions.
- Update documentation and examples when public APIs or behavior change.
- Validate serialization changes in `iroha_data_model` with roundtrip tests to preserve Norito layout guarantees.
- Integration tests spin real multi-peer networks; use at least 4 peers when constructing test networks (single-peer configs are not representative and can deadlock in Sumeragi).
- Do not attempt to disable DA/RBC in tests or add a legacy RBC bypass.
  Revision-4 genesis and every height context must carry one valid signed RS16
  DA layout.
- Revision-4 QCs require exactly `2f + 1` equal validator votes from an exact
  `3f + 1` committee; observers never pad Prepare, Commit, or Timeout quorum.
- Exercise message loss with the feature-isolated authenticated consensus
  message controller (`with_consensus_message_control`) and prove its hold/drop
  acknowledgement before healing. Retired `[sumeragi.debug.rbc]` keys are
  configuration errors, not fault-injection controls.
- When the user asks about the live SORA Taira testnet or deployed Torii MCP
  workflows, consult `skills/sora-taira-testnet/SKILL.md` in this repo and
  prefer the curated `iroha.*` tool surface. Treat
  `https://taira.sora.org/v1/mcp` as the current primary public Taira MCP
  endpoint unless the user or operator gives you a different public Torii
  root for the deployment under test.
- Treat any Taira/runtime signing inputs such as `authority`,
  `private_key`, bearer tokens, or forwarded auth headers as runtime-only
  secrets and never persist them in repo files or committed docs.
- For a disposable four-validator Taira deployment, use
  `python3 scripts/taira_devnet.py up --inrou-canary-dir <owner-only-workspace>`;
  use its `check` and `down` subcommands for inspection and teardown. Guest
  workload qualification is mandatory; request `--full-doctor` only when the
  broader public product-route surface is also under test.
- For public Taira diagnostics use the same-revision compiled
  `iroha taira doctor`. The public-reset coordinator owns signed canary
  execution. Its low-level `iroha taira write-canary` child accepts exactly one
  ordered operation and exactly one prepare, submit, or read-only recovery
  action over an inherited numeric descriptor; never replace that durable
  protocol with a one-shot invocation. Keep the populated client config and
  owner-only onboarding-token file runtime-only.
- If a live public Taira write fails with `route_unavailable`, treat it as an
  ingress or authoritative-peer routing failure first, not a user payload bug.
- If a live Taira signed canary fails with `Failed to find asset`, check the
  CLI-owned faucet/bootstrap result before changing deploy topology: the signer
  may simply be unfunded for the fee asset.
- When the user asks about the live SORA Minamoto mainnet or deployed Torii MCP
  workflows, consult `skills/sora-minamoto-mainnet/SKILL.md` in this repo and
  prefer the curated `iroha.*` tool surface. Treat
  `https://minamoto.sora.org/v1/mcp` as the current primary public Minamoto MCP
  endpoint unless the user or operator gives you a different public Torii
  root for the deployment under test.
- For Minamoto mainnet work, stay read-only until the user explicitly asks to
  mutate live state. Do not use Taira testnet faucet/bootstrap/canary
  assumptions on Minamoto; prefer pre-signed transaction envelopes for
  irreversible or value-moving operations, and keep any Minamoto signing inputs
  runtime-only.

## Navigation tips
- Search code: `rg '<term>'` and list files: `fd <name>`.
- Explore crates: `fd --type f Cargo.toml crates | xargs -I{} dirname {}`.
- Find examples/benches quickly: `fd . crates -E target -t d -d 3 -g "*{examples,benches}"`.
- Python tip: some environments don’t provide `python`; try `python3` instead when running scripts.

## Proc-Macro Tests
- Unit tests: use for pure parsing, codegen helpers, and utilities (fast, no compiler involved).
- UI tests (trybuild): use to validate compile-time behavior and diagnostics of derive/proc-macros (success and expected failure cases with `.stderr`).
- Prefer both when adding/changing macros: unit tests for internals + UI tests for user-facing behavior and error messages.
- Avoid panics; emit clear diagnostics (e.g., via `syn::Error` or `proc_macro_error`). Keep messages stable and update `.stderr` only for intentional changes.

## Pull Request message
Include a short summary of the changes and a `Testing` section describing the commands you ran.
