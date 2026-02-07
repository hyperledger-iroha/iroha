---
lang: my
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
---

//! Confidential assets audit & operations playbook referenced by `roadmap.md:M4`.

# Confidential Assets Audit & Operations Runbook

This guide consolidates the evidence surfaces auditors and operators rely on
when validating confidential-asset flows. It complements the rotation playbook
(`docs/source/confidential_assets_rotation.md`) and the calibration ledger
(`docs/source/confidential_assets_calibration.md`).

## 1. Selective Disclosure & Event Feeds

- Every confidential instruction emits a structured `ConfidentialEvent` payload
  (`Shielded`, `Transferred`, `Unshielded`) captured in
  `crates/iroha_data_model/src/events/data/events.rs:198` and serialized by the
  executors (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  The regression suite exercises the concrete payloads so auditors can rely on
  deterministic JSON layouts (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii exposes these events via the standard SSE/WebSocket pipeline; auditors
  subscribe using `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  optionally scoping to a single asset definition. CLI example:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Policy metadata and pending transitions are available through
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), mirrored by the Swift SDK
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) and documented in
  both the confidential-assets design and SDK guides
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Telemetry, Dashboards, and Calibration Evidence

- Runtime metrics surface tree depth, commitment/frontier history, root eviction
  counters, and verifier-cache hit ratios
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana dashboards in
  `dashboards/grafana/confidential_assets.json` ship the associated panels and
  alerts, with the workflow documented in `docs/source/confidential_assets.md:401`.
- Calibration runs (NS/op, gas/op, ns/gas) with signed logs live in
  `docs/source/confidential_assets_calibration.md`. The latest Apple Silicon
  NEON run is archived at
  `docs/source/confidential_assets_calibration_neon_20260428.log`, and the same
  ledger records the temporary waivers for SIMD-neutral and AVX2 profiles until
  the x86 hosts come online.

## 3. Incident Response & Operator Tasks

- Rotation/upgrade procedures reside in
  `docs/source/confidential_assets_rotation.md`, covering how to stage new
  parameter bundles, schedule policy upgrades, and notify wallets/auditors. The
  tracker (`docs/source/project_tracker/confidential_assets_phase_c.md`) lists
  runbook owners and rehearsal expectations.
- For production rehearsals or emergency windows, operators attach evidence to
  `status.md` entries (e.g., the multi-lane rehearsal log) and include:
  `curl` proof of policy transitions, Grafana snapshots, and the relevant event
  digests so auditors can reconstruct mint→transfer→reveal timelines.

## 4. External Review Cadence

- Security review scope: confidential circuits, parameter registries, policy
  transitions, and telemetry. This document plus the calibration ledger forms
  the evidence packet sent to vendors; review scheduling is tracked via
  M4 in `docs/source/project_tracker/confidential_assets_phase_c.md`.
- Operators must keep `status.md` updated with any vendor findings or follow-up
  action items. Until the external review completes, this runbook serves as the
  operational baseline auditors can test against.
