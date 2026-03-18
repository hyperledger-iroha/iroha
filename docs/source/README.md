# Iroha VM + Kotodama Docs Index

This index links the main design and reference documents for IVM, Kotodama, and the IVM‑first pipeline. 日本語訳は [`README.ja.md`](./README.ja.md) を参照してください。

- IVM architecture and language mapping: `../../ivm.md`
- IVM syscall ABI: `ivm_syscalls.md`
- Generated syscall constants: `ivm_syscalls_generated.md` (run `make docs-syscalls` to refresh)
- IVM bytecode header: `ivm_header.md`
- Kotodama grammar and semantics: `kotodama_grammar.md`
- Kotodama examples and syscall mappings: `kotodama_examples.md`
- Transaction pipeline (IVM‑first): `../../new_pipeline.md`
- Torii Contracts API (manifests): `torii_contracts_api.md`
- Universal account/UAID operations guide: `universal_accounts_guide.md`
- JSON query envelope (CLI / tooling): `query_json.md`
- Norito streaming module reference: `norito_streaming.md`
- Runtime ABI samples: `samples/runtime_abi_active.md`, `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- ZK App API (attachments, prover, vote tally): `zk_app_api.md`
- Torii ZK attachments/prover runbook: `zk/prover_runbook.md`
- Torii ZK App API operator guide (attachments/prover; crate doc): `../../crates/iroha_torii/docs/zk_app_api.md`
- Torii MCP API guide (agent/tool bridge; crate doc): `../../crates/iroha_torii/docs/mcp_api.md`
- VK/proof lifecycle (registry, verification, telemetry): `zk/lifecycle.md`
- Torii Operator Aids (endpoints for visibility): `references/operator_aids.md`
- Nexus default-lane quickstart: `quickstart/default_lane.md`
- MOCHI supervisor quickstart & architecture: `mochi/index.md`
- JavaScript SDK guides (quickstart, configuration, publishing): `sdk/js/index.md`
- Swift SDK parity/CI dashboards: `references/ios_metrics.md`
- Governance: `../../gov.md`
- Domain endorsements (committees, policies, validation): `domain_endorsements.md`
- JDG attestations (offline validation tooling): `jdg_attestations.md`
- Clarification coordination prompts: `coordination_llm_prompts.md`
- Roadmap: `../../roadmap.md`
- Docker builder image usage: `docker_build.md`

Usage tips
- Build and run examples in `examples/` using external tools (`koto_compile`, `ivm_run`):
  - `make examples-run` (and `make examples-inspect` if `ivm_tool` is available)
- Optional integration tests (ignored by default) for examples and header checks live in `integration_tests/tests/`.

Pipeline configuration
- All runtime behavior is configured via `iroha_config` files. Environment variables are not used for operators.
- Sensible defaults are provided; most deployments won’t need changes.
- Relevant keys under `[pipeline]`:
  - `dynamic_prepass`: enable IVM read‑only prepass to derive access sets (default: true).
  - `access_set_cache_enabled`: cache derived access sets per `(code_hash, entrypoint)`; disable to debug hints (default: true).
  - `parallel_overlay`: build overlays in parallel; commit remains deterministic (default: true).
  - `gpu_key_bucket`: optional key bucketing for the scheduler prepass using a stable radix on `(key, tx_idx, rw_flag)`; deterministic CPU fallback is always active (default: false).
  - `cache_size`: capacity of the global IVM pre‑decode cache (decoded streams kept). Default: 128. Increasing can reduce decode time for repeated executions.

Docs sync checks
- Syscall constants (docs/source/ivm_syscalls_generated.md)
  - Regenerate: `make docs-syscalls`
  - Check only: `bash scripts/check_syscalls_doc.sh`
- Syscall ABI table (crates/ivm/docs/syscalls.md)
  - Check only: `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Update generated section (and code docs table): `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Pointer‑ABI tables (crates/ivm/docs/pointer_abi.md and ivm.md)
  - Check only: `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Update sections: `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- IVM header policy and ABI hashes (docs/source/ivm_header.md)
  - Check only: `cargo run -p ivm --bin gen_header_doc -- --check` and `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Update sections: `cargo run -p ivm --bin gen_header_doc -- --write` and `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI
- GitHub Actions workflow `.github/workflows/check-docs.yml` runs these checks on every push/PR and will fail if generated docs drift from the implementation.
- [Governance Playbook](governance_playbook.md)
