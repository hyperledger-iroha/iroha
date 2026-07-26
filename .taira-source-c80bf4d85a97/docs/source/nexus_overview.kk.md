---
lang: kk
direction: ltr
source: docs/source/nexus_overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bda1352ff13cc866cd02a08f9db6be962798b547e905f2fccf236cd803eb0eda
source_last_modified: "2025-12-29T18:16:35.991754+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus Overview & Operator Context

**Roadmap link:** NX-14 — Nexus documentation & operator runbooks  
**Status:** Drafted 2026-03-24 (pairs with `docs/source/nexus_operations.md`)  
**Audience:** Program managers, operations engineers, and partner teams that
need a single-page summary of the Sora Nexus (Iroha 3) architecture before
diving into the detailed specifications (`docs/source/nexus.md`,
`docs/source/nexus_lanes.md`, `docs/source/nexus_transition_notes.md`).

## 1. Release Lines & Shared Tooling

- **Iroha 2** remains the self-hosted track for consortium deployments.
- **Iroha 3 / Sora Nexus** introduces multi-lane execution, data spaces, and
  shared governance. The same repository, toolchain, and CI pipelines build both
  release lines, so fixes to the Iroha Virtual Machine (IVM), Kotodama compiler,
  or SDKs automatically apply to Nexus.
- **Artifacts:** `iroha3-<version>-<os>.tar.zst` bundles and OCI images contain
  binaries, sample configs, and the Nexus profile metadata. Operators reference
  `docs/source/sora_nexus_operator_onboarding.md` for the end-to-end artefact
  validation workflow.
- **Shared SDK surface:** Rust, Python, JS/TS, Swift, and Android SDKs consume
  the same Norito schemas and address fixtures (`fixtures/account/address_vectors.json`)
  so wallets and automation can switch between Iroha 2 and Nexus networks without
  format forks.

## 2. Architectural Building Blocks

| Component | Description | Key references |
|-----------|-------------|----------------|
| **Data Space (DS)** | Governance-scoped execution domain that defines validator membership, privacy class, fee policy, and data-availability profile. Each DS owns one or more *lanes*. | `docs/source/nexus.md`, `docs/source/nexus_transition_notes.md` |
| **Lane** | Deterministic shard of execution and state. Lane manifests declare validator sets, settlement hooks, telemetry metadata, and routing permissions. The global consensus ring orders lane commitments. | `docs/source/nexus_lanes.md` |
| **Space Directory** | Registry contract (and CLI helpers) that stores DS manifests, validator rotations, and capability grants. Keeps historical manifests signed so auditors can reconstruct state. | `docs/source/nexus.md#space-directory` |
| **Lane Catalog** | Configuration section (`[nexus]` in `config.toml`) that maps lane IDs to aliases, routing policies, and retention knobs. Operators can introspect the effective catalog via `irohad --sora --config … --trace-config`. | `docs/source/sora_nexus_operator_onboarding.md` |
| **Settlement Router** | Routes XOR movements between lanes (e.g., private CBDC lanes ↔ public liquidity lanes). Policy defaults live in `docs/source/cbdc_lane_playbook.md`. | `docs/source/cbdc_lane_playbook.md` |
| **Telemetry & SLOs** | Dashboards and alert rules under `dashboards/grafana/nexus_*.json` capture lane height, DA backlog, settlement latency, and governance queue depth. The remediation plan is tracked in `docs/source/nexus_telemetry_remediation_plan.md`. | `dashboards/grafana/nexus_lanes.json`, `dashboards/alerts/nexus_audit_rules.yml` |

### Lane & Data-Space Classes

- `default_public` lanes anchor fully public workloads under Sora Parliament.
- `public_custom` lanes allow program-specific economics while remaining
  transparent.
- `private_permissioned` lanes serve CBDCs or consortium apps; they export only
  commitments and proofs.
- `hybrid_confidential` lanes pair zero-knowledge proofs with selective
  disclosure hooks.

Each lane declares:

1. **Lane manifest:** governance-approved metadata tracked through the Space
   Directory.
2. **Data-availability policy:** erasure coding parameters, recovery hooks, and
   audit requirements.
3. **Telemetry profile:** dashboards + on-call runbooks that must be updated
   whenever governance toggles a lane.

## 3. Rollout Timeline Snapshot

| Phase | Focus | Exit criteria |
|-------|-------|---------------|
| **N0 – Closed beta** | Council-managed registrar, `.sora` namespace only, manual operator onboarding. | DS manifests signed, lane catalog static, governance rehearsals logged. |
| **N1 – Public launch** | Adds `.nexus` suffixes, auctions, and self-service registrar. Settlements wire up to XOR treasury. | Resolver/gateway sync tests green, billing reconciliation dashboards live, dispute tabletop complete. |
| **N2 – Expansion** | Enables `.dao`, reseller APIs, analytics, dispute portal, steward scorecards. | Compliance artefacts versioned, policy-jury toolkit live, treasury transparency reports published. |
| **NX-12/13/14 gate** | Compliance engine, telemetry dashboards, and documentation must land together before opening the partner pilot. | `docs/source/nexus_overview.md` + `docs/source/nexus_operations.md` published, dashboards wired with alerts, policy engine hooked into governance. |

## 4. Operator Responsibilities

| Responsibility | Description | Evidence |
|----------------|-------------|----------|
| Config hygiene | Keep `config/config.toml` in sync with the published lane & dataspace catalog; record deltas in change tickets. | `irohad --sora --config … --trace-config` output archived with release artefacts. |
| Manifest tracking | Watch Space Directory updates and refresh local caches/allowlists. | Signed manifest bundle stored with onboarding ticket. |
| Telemetry coverage | Ensure dashboards listed in Section 2 are reachable, alerts wired into PagerDuty, and quarterly reviews logged. | On-call review minutes + Alertmanager export. |
| Incident reporting | Follow the severity matrix defined in `docs/source/nexus_operations.md` and file post-incident reports within five business days. | Post-incident template archived per incident ID. |
| Governance readiness | Participate in Nexus council votes when lane policy changes affect your deployment; rehearse rollback instructions quarterly. | Council attendance + rehearsal checklist stored under `docs/source/project_tracker/nexus_config_deltas/`. |

## 5. Related Documentation Map

- **Deep dive specification:** `docs/source/nexus.md`
- **Lane geometry & storage layout:** `docs/source/nexus_lanes.md`
- **Transition plan & placeholder routing:** `docs/source/nexus_transition_notes.md`
- **Operator onboarding walkthrough:** `docs/source/sora_nexus_operator_onboarding.md`
- **CBDC lane policy & settlement plan:** `docs/source/cbdc_lane_playbook.md`
- **Telemetry remediation & dashboard map:** `docs/source/nexus_telemetry_remediation_plan.md`
- **Runbook / incident process:** `docs/source/nexus_operations.md`

Keep this overview in sync with roadmap item NX-14 whenever substantial changes
land in the linked documents or when new lane classes/governance flows are
introduced.
