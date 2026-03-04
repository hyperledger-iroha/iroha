# Japanese Documentation Translation Backlog

This file tracks Japanese documentation that is still marked `status: needs-translation`,
grouped by category so we can plan upcoming batches. Files matching `CHANGELOG.*`,
`status.*`, and `roadmap.*` are treated as temporary and excluded from the backlog.
Run `python3 scripts/sync_docs_i18n.py --dry-run` to refresh the list before starting a
new batch.

## Overview

- Pending files: 0 (updated 2025-10-26)
- Translation exclusions: continue to skip `CHANGELOG.*`, `status.*`, and `roadmap.*`.
- No backlog is currently open. When new English documents land, run
  `scripts/sync_docs_i18n.py --dry-run` to detect the delta and queue the next batch.

### Recently completed Japanese translations
- `AGENTS.ja.md`, `docs/AGENTS.ja.md`
- `CODE_OF_CONDUCT.ja.md`
- `CONTRIBUTING.ja.md`
- `MAINTAINERS.ja.md`
- `docs/genesis.ja.md`
- `docs/norito_demo_contributor.ja.md`
- `docs/norito_bridge_release.ja.md`
- `docs/profile_build.ja.md`
- `docs/norito_bridge_release.ja.md`
- `docs/norito_demo_contributor.ja.md`
- `docs/genesis.ja.md`
- `docs/source/kaigi_privacy_design.ja.md`
- `docs/source/ivm_architecture_plan.ja.md`
- `docs/source/ivm_syscalls.ja.md`
- `docs/source/ivm_header.ja.md`
- `docs/source/kotodama_grammar.ja.md`
- `docs/source/kotodama_examples.ja.md`
- `docs/source/kotodama_error_codes.ja.md`
- `docs/source/sumeragi.ja.md`
- `docs/source/pipeline.ja.md`
- `docs/source/norito_streaming_transport_design.ja.md`
- `docs/source/norito_streaming.ja.md`
- `docs/source/torii_contracts_api.ja.md`
- `docs/source/global_feature_matrix.ja.md`
- `docs/source/contract_deployment.ja.md`
- `docs/source/runtime_upgrades.ja.md`
- `docs/source/security_hardening_requirements.ja.md`
- `docs/source/data_model.ja.md`
- `docs/source/coordination_llm_prompts.ja.md`
- `docs/source/crypto/dependency_audits.ja.md`
- `docs/source/crypto/gost_performance.ja.md`
- `docs/source/threat_model.ja.md`
- `docs/source/norito_streaming.ja.md`
- `docs/source/governance_api.ja.md`
- `docs/source/samples/find_active_abi_versions.ja.md`
- `docs/source/samples/node_capabilities.ja.md`
- `docs/source/samples/runtime_abi_active.ja.md`
- `docs/source/samples/runtime_abi_hash.ja.md`
- `docs/source/samples/signed_query_find_active_abi_versions.ja.md`
- `docs/source/samples/signed_query_iterable_find_peers.ja.md`
- `docs/source/samples/sumeragi_pacemaker_status.ja.md`
- `docs/source/samples/sumeragi_rbc_status.ja.md`
- `docs/source/references/ci_operations.ja.md`
- `docs/source/references/configuration.ja.md`
- `docs/source/references/nts.ja.md`
- `docs/source/references/operator_aids.ja.md`
- `docs/source/query_json.ja.md`
- `docs/source/query_benchmarks.ja.md`
- `docs/source/torii_query_cursor_modes.ja.md`
- `docs/source/error_mapping.ja.md`
- `docs/source/snapshot_queries.ja.md`
- `docs/source/testing.ja.md`
- `docs/source/state_tiering.ja.md`
- `docs/source/p2p.ja.md`
- `docs/source/sumeragi_aggregators.ja.md`
- `docs/source/sumeragi_evidence_api.ja.md`
- `docs/source/sumeragi_da.ja.md`
- `docs/source/sumeragi_pacemaker.ja.md`
- `docs/source/sumeragi_npos_task_breakdown.ja.md`
- `docs/source/merge_ledger.ja.md`
- `docs/source/norito_json_migration.ja.md`
- `docs/source/norito_json_inventory.ja.md`
- `docs/source/confidential_assets.ja.md`
- `docs/source/confidential_assets_calibration.ja.md`
- `docs/source/norito_crc64_parity_bench.ja.md`
- `docs/source/fastpq_plan.ja.md`
- `docs/source/fastpq_migration_guide.md`
- `docs/source/examples/smt_update.ja.md`
- `docs/source/nexus_refactor_plan.ja.md`
- `docs/source/nexus_transition_notes.ja.md`
- `docs/source/iroha_monitor.ja.md`
- `docs/source/zk1_envelope.ja.md`
- `docs/source/zk_envelopes.ja.md`
- `docs/source/zk_app_api.ja.md`
- `docs/source/references/ios_metrics.ja.md`
- `docs/source/zk/lifecycle.ja.md`
- `docs/source/zk/prover_runbook.ja.md`
- `docs/source/iroha_2_whitepaper.ja.md`
- `docs/source/new_pipeline.ja.md`
- `docs/source/connect_architecture_strawman.ja.md`
- `docs/source/metal_neon_acceleration_plan.ja.md`
- `docs/source/iroha_3_whitepaper.ja.md`
- `docs/source/project_tracker/2026_q1_clarification_prompts.ja.md`
- `docs/source/ivm_isi_kotodama_alignment.ja.md`
- `docs/source/benchmarks.ja.md`
- `docs/source/docker_build.ja.md`
- `docs/source/ivm_syscalls_generated.ja.md`
- `docs/comment_audit.ja.md`
- `docs/dependency_audit.ja.md`
- `docs/account_structure_sdk_alignment.ja.md`
- `docs/connect_config.ja.md`
- `docs/references/configuration.ja.md`
- `docs/connect_examples_readme.ja.md`
- `docs/connect_client_examples.ja.md`
- `docs/connect_kotlin_ws.ja.md`
- `docs/connect_ts_wrapper.ja.md`
- `docs/account_structure.ja.md`

## Suggested batching

> With no outstanding items we are keeping the batch plan as reference material for the
> next wave of documents.

### Batch A – ZK / Torii operational docs
- Goal: keep the ZK attachment and verification runbooks current in Japanese.
- Scope: any remaining `docs/source/zk/*.ja.md` (both `lifecycle` and `prover_runbook` are already complete).
- Deliverable: full ZK operations coverage plus a glossary appended to the runbooks.

### Batch B – IVM / Kotodama / Norito core design
- Goal: strengthen developer discoverability by translating core VM/codec design docs.
- Scope: `docs/source/ivm_*.ja.md`, `docs/source/kotodama_*.ja.md`, `docs/source/norito_*.ja.md`.
- Deliverable: concise summaries and keyword indices for each document.

### Batch C – Sumeragi / network operations
- Goal: provide operational playbooks for consensus, pipeline, and network layers.
- Scope: `docs/source/sumeragi*.ja.md`, `docs/source/governance_api.ja.md`, `docs/source/pipeline.ja.md`, `docs/source/p2p.ja.md`, `docs/source/state_tiering.ja.md`.
- Deliverable: the first Japanese edition of the operator handbook.

### Batch D – Tools / samples / references
- Goal: equip developers with localized reference material and sample coverage.
- Scope: outstanding `docs/source/query_*.ja.md` (and any newly added references).
- Deliverable: ensure key CLI and API references stay aligned with the English source.

## Translation workflow reminders

1. Pick the target files and work in a dedicated branch.
2. After finishing the translation, set `status: complete` in the front matter and add
   yourself as `translator` if needed.
3. Rerun `python3 scripts/sync_docs_i18n.py --dry-run` to verify no stubs remain.
4. Align terminology with existing Japanese documents such as `README.ja.md`.

## Notes

- Watch the Markdown formatting (indentation, tables, fenced blocks) instead of running
  `cargo fmt --all`.
- When the source document changes, update the Japanese edition in the same PR whenever
  possible.
- Follow the terminology conventions shared across the published documentation; schedule
  reviews if wording needs consensus.
- Run `python3 scripts/sync_docs_i18n.py --dry-run` periodically and update this backlog
  if new gaps appear.
