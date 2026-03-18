---
lang: ka
direction: ltr
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 160fb55b80506dae516f71d7e738bcad9ff8009cf9edad548054474cd3cfce51
source_last_modified: "2026-02-05T09:12:02.124019+00:00"
translation_last_reviewed: 2026-02-07
title: Nexus Lane Compliance & Whitelist Policy Engine (NX-12)
---

Status: 🈴 Implemented — this document captures the live policy model and
consensus-critical enforcement referenced by roadmap item
**NX-12 — Lane compliance & whitelist policy engine**.
It explains the data model, governance flows, telemetry, and rollout strategy
implemented inside `crates/iroha_core/src/compliance` and enforced during both
Torii admission and `iroha_core` transaction validation, so every lane and
dataspace can be bound to deterministic jurisdictional policies.

## Goals

- Allow governance to attach allow/deny rules, jurisdiction flags, CBDC transfer
  limits, and audit requirements to each lane manifest.
- Evaluate every transaction against those rules during Torii admission and
  block execution, ensuring deterministic policy enforcement across nodes.
- Produce a cryptographically verifiable audit trail, complete with Norito
  evidence bundles and queriable telemetry for regulators and operators.
- Keep the model flexible: the same policy engine covers private CBDC lanes,
  public settlement DS, and hybrid partner dataspaces without bespoke forks.

## Non-Goals

- Defining AML/KYC procedures or legal escalation workflows. Those live in the
  compliance playbooks that consume the telemetry produced here.
- Introducing per-instruction toggles in IVM; the engine only controls which
  accounts/assets/domains may submit transactions or interact with a lane.
- Obsoleting Space Directory. Manifests continue to be the authoritative source
  for DS metadata; the compliance policy simply references Space Directory
  entries and supplements them.

## Policy Model

### Entities and identifiers

The policy engine operates on:

- `LaneId` / `DataSpaceId` — identifies the scope where rules apply.
- `UniversalAccountId (UAID)` — allows grouping cross-lane identities.
- `JurisdictionFlag` — bitmask enumerating regulatory classifications (e.g.
  `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` — describes who is affected:
  - `AccountId`, `DomainId`, or `UAID`.
  - Prefix-based selectors (`DomainPrefix`, `UaidPrefix`) to match registries.
  - `CapabilityTag` for Space Directory manifests (e.g. only FX-cleared DS).
  - `privacy_commitments_any_of` gating to require lanes to advertise specific
    Nexus privacy commitments before rules match (mirrors the NX-10 manifest
    surface and is enforced in `LanePrivacyRegistry` snapshots).

### LaneCompliancePolicy

Policies are Norito-encoded structs published via governance:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` combines a `ParticipantSelector`, optional jurisdiction override,
  capability tags, and reason codes.
- `DenyRule` mirrors the allow structure but is evaluated first (deny-wins).
- `TransferLimit` captures asset/bucket specific ceilings:
  - `max_notional_xor` and `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (e.g. CBDC retail vs wholesale).
- `AuditControls` configures:
  - Whether Torii must persist every denial in the audit log.
  - Whether successful decisions should be sampled into Norito digests.
  - Required retention window for `LaneComplianceDecisionRecord`.

### Storage and distribution

- Latest policy hashes live in the Space Directory manifest alongside validator
  keys. `LaneCompliancePolicyReference` (policy id + version + hash) becomes a
  manifest field so validators and SDKs can fetch the canonical policy blob.
- `iroha_config` exposes `compliance.policy_cache_dir` to persist the Norito
  payload and its detached signature. Nodes verify signatures before applying
  updates to guard against tampering.
- Policies are also embedded into the Norito admission manifests used by Torii
  so CI/SDKs can replay policy evaluation without talking to validators.

## Governance & Lifecycle

1. **Proposal** — governance submits `ProposeLaneCompliancePolicy` with the
   Norito payload, jurisdiction justification, and activation epoch.
2. **Review** — compliance reviewers sign `LaneCompliancePolicyReviewEvidence`
   (auditable, stored in `governance::ReviewEvidenceStore`).
3. **Activation** — after the delay window, validators ingest the policy by
   calling `ActivateLaneCompliancePolicy`. The Space Directory manifest is
   updated atomically with the new policy reference.
4. **Amend/Revoke** — `AmendLaneCompliancePolicy` carries diff metadata while
   keeping the previous version for forensic replay; `RevokeLaneCompliancePolicy`
   pins the policy id to `denied` so Torii rejects any traffic targeting that
   lane until a replacement is activated.

Torii exposes:

- `GET /v1/lane-compliance/policies/{lane_id}` — fetch latest policy reference.
- `POST /v1/lane-compliance/policies` — governance-only endpoint mirroring the
  ISI proposal helpers.
- `GET /v1/lane-compliance/decisions` — paginated audit log with filters for
  `lane_id`, `decision`, `jurisdiction`, and `reason_code`.

CLI/SDK commands wrap those HTTP surfaces so operators can script reviews and
fetch artefacts (signed policy blob + reviewer attestations).

## Enforcement Pipeline

1. **Admission (Torii)**  
   - `Torii` downloads the active policy when a lane manifest changes or when
     the cached signature expires.  
   - Every transaction entering the `/v1/pipeline` queue is tagged with
     `LaneComplianceContext` (participant ids, UAID, dataspace manifest metadata, policy id, and the
     latest `LanePrivacyRegistry` snapshot described in `crates/iroha_core/src/interlane/mod.rs`).  
   - UAID-carrying authorities are resolved globally by account UAID. If a Space Directory manifest exists for the routed dataspace, it must be active; missing target-dataspace manifests do not block admission before any policy rules are evaluated.  
   - The `compliance::Engine` evaluates `deny` rules, then `allow` rules, and
     finally enforces transfer limits. Failing transactions return a typed error
     (`ERR_LANE_COMPLIANCE_DENIED`) with reason + policy id for audit trails.
   - Admission is a fast prefilter; consensus validation re-checks the same
     rules using the state snapshots to keep enforcement deterministic.
2. **Execution (iroha_core)**  
   - During block construction, `iroha_core::tx::validate_transaction_internal`
     replays the same lane governance/UAID/privacy/compliance checks using the
     `StateTransaction` snapshots (`lane_manifests`, `lane_privacy_registry`,
     `lane_compliance`). This keeps enforcement consensus-critical even if Torii
     caches become stale.  
   - Transactions that mutate lane manifests or compliance policies still flow
     through the same validation path; there is no admission-only bypass.
3. **Async hooks**  
   - RBC gossip and DA fetchers attach the policy id to telemetry so late
     decisions can be traced back to the correct rule version.  
   - `iroha_cli` and SDK helpers expose `LaneComplianceDecision::explain()` so
     automation can render human-readable diagnostics.

The engine is deterministic and pure; it never reaches out to external systems
after the manifest/policy has been downloaded. That keeps CI fixtures and
multi-node reproduction straightforward.

## Audit & Telemetry

- **Metrics**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (should stay < activation delay).
- **Logs**
  - Structured records capture `policy_id`, `version`, `participant`, `UAID`,
    jurisdiction flags, and the Norito hash of the offending transaction.
  - `LaneComplianceDecisionRecord` is Norito-encoded and persisted under
    `world.compliance_logs::<lane_id>::<ts>::<nonce>` when `AuditControls`
    requests durable storage.
- **Evidence bundles**
  - `cargo xtask nexus-lane-audit` gains a `--lane-compliance <path>` mode that
    merges the policy, reviewer signatures, metrics snapshot, and the most
    recent audit log into the JSON + Parquet outputs. The flag expects a JSON
    payload shaped as:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    The CLI validates that each `policy` blob matches the `lane_id` listed in
    the record before embedding it, preventing stale or mismatched evidence from
    entering regulator packets and the roadmap dashboards.
  - `--markdown-out` (defaults to `artifacts/nexus_lane_audit.md`) now renders a
    human-readable summary that calls out lagging lanes, non-zero backlog,
    pending manifests, and missing compliance evidence so annex packets include
    both machine-readable artefacts and a quick review surface.

## Rollout Plan

1. **P0 — Observability only**  
   - Ship the policy types, storage, Torii endpoints, and metrics.  
   - Torii evaluates policies in `audit` mode (no enforcement) to collect data.
2. **P1 — Deny/allow enforcement**  
   - Enable hard failures in Torii + execution when deny rules trigger.  
   - Require policies for all CBDC lanes; public DS may still run in audit mode.
3. **P2 — Limits & jurisdictional overrides**  
   - Activate transfer limit enforcement and jurisdiction flags.  
   - Feed telemetry into `dashboards/grafana/nexus_lanes.json`.
4. **P3 — Full compliance automation**  
   - Integrate audit exports with `SpaceDirectoryEvent` consumers.  
   - Tie policy updates to governance runbooks and release automation.

## Acceptance & Testing

- Integration tests under `integration_tests/tests/nexus/compliance.rs` cover:
  - allow/deny combinations, jurisdiction overrides, and transfer limits;
  - manifest/policy activation races; and
  - Torii vs `iroha_core` decision parity across multi-node runs.
- Unit tests in `crates/iroha_core/src/compliance` validate the pure evaluation
  engine, cache invalidation timers, and metadata parsing.
- Docs/SDK updates (Torii + CLI) must demonstrate fetching policies, submitting
  governance proposals, interpreting error codes, and collecting audit evidence.

Closing NX-12 requires the above artefacts plus status updates in
`status.md`/`roadmap.md` once enforcement is live across staging clusters.
