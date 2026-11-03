# Native PR validation

PR jobs use GitHub-hosted Ubuntu 24.04 runners and this composite action. No
prebuilt CI image or registry credentials are required. The action installs
native compiler/library prerequisites, the toolchain in `rust-toolchain.toml`,
Python 3.12 with the repository's script requirements, Temurin 21 and Node
22.23.4. Each calling job owns its Cargo cache.

The optional `toolchain` input is reserved for jobs that require a different
toolchain. PR coverage explicitly selects `nightly-2026-10-23` because LLVM
branch and doctest coverage require nightly; all other Rust jobs use the
repository toolchain. Workspace tests, network proofs, native IVM release tests,
strict lint, documentation, fixture parity and formal-model tiers remain checks.

The Python job runs the existing Norito codec, Torii client and script tests.
The old Poetry test directories contain fixture modules and the former network
setup script is absent. Real-peer coverage remains in the Rust workspace and
the explicit SORA Nexus network jobs.

Run the configuration regressions with
`python -m pytest -q pytests/scripts/native_pr_workflow_test.py`. They validate
real target names, network binary handoff, coverage settings, nonempty Python
suites and the Apalache command. These checks do not replace executing the
Rust and formal suites in CI.
