---
lang: az
direction: ltr
source: docs/source/sorafs_gateway_compliance_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3ea04f6ac217749839351798378d0eacea584d3236fb760edba3bdc685da995
source_last_modified: "2025-12-29T18:16:36.139893+00:00"
translation_last_reviewed: 2026-02-07
title: Gateway Compliance, Moderation & Transparency
summary: Final specification for SFM-4 covering denylist ingestion, moderation toggles, transparency reports, proof tokens, observability, and rollout.
---

# Gateway Compliance, Moderation & Transparency

## Goals & Scope
- Equip SoraFS gateways with a deterministic compliance layer that ingests external denylists, enforces operator-controlled moderation toggles, and logs every decision for auditability.
- Provide transparent reporting (weekly/monthly) and cryptographic proof tokens so clients can verify moderation outcomes without exposing sensitive details.
- Integrate with governance, moderation appeal workflows (SFM-4b), AI pre-screen screening (SFM-4a), and transparency dashboards (SFM-4c) while maintaining privacy and legal compliance.

This specification completes **SFM-4 — Gateway compliance & transparency module** at the architectural level; sub-workstreams (4a/4b/4c) complement this plan with domain-specific detail.

## Architecture Overview
| Component | Responsibility | Notes |
|-----------|----------------|-------|
| Compliance controller (`gateway_complianced`) | Fetches denylists, normalises entries, applies policy matrix, distributes updates to gateway nodes. | Runs centrally per environment; publishes signed updates via Torii. |
| Gateway enforcement module (`gateway::compliance`) | Evaluates requests/responses against active rules, issues deny/allow decisions, emits proof tokens + telemetry. | Embedded in each gateway binary (Rust). |
| Moderation toggle service (`moderation_toggle_api`) | Provides operator UI/API for enabling/disabling policy categories per gateway with audit trail. | Backed by Postgres audit log + multi-sig approvals. |
| Transparency reporter (`transparency_reporter`) | Aggregates moderation events, emits weekly/monthly reports, publishes to Governance DAG + dashboards. | Writes `TransparencyReportV1` Norito payloads. |
| Proof token issuer (`proof_token_service`) | Generates opaque tokens per moderation action for clients to verify decisions off-chain. | Uses blinded tokens + Merkle commitments. |
| Storage & audit | RocksDB on gateways for active enforcement; Postgres + object storage for historical data. | Hot retention 180 days, cold archive 7 years. |

### Data Flow
1. Compliance controller fetches external denylists (BadBits, regional, legal takedowns), normalises to `DenylistEntryV1`.
2. Controller applies policy matrix, signs update (`ComplianceUpdateV1`) with Dilithium3 key, publishes via Torii + IPFS.
3. Gateways subscribe via WebSocket/REST, verify signature, update local RocksDB cache, and confirm receipt.
4. During request handling, gateway compliance interceptor checks request (CID, URL, headers, account) against active rules + operator toggles. If block required, returns appropriate response and issues proof token.
5. All decisions logged locally and forwarded to transparency reporter; weekly reports and Merkle commitments published to Governance DAG.
6. Operators review dashboards, answer appeals (SFM-4b), adjust toggles as needed with audit entries.

## Denylist Ingestion
- **Sources** (extensible):
  - BadBits canonical feed (`https://badbits.sora.net/v2/list.json`)
  - Regional compliance feeds (EU DSA, DMCA, etc.)
  - Internal governance actions (manual takedowns).
  - AI pre-screen quarantine (from SFM-4a service).
- **Schema** (`sorafs_manifest::compliance` module):
  ```norito
  struct DenylistEntryV1 {
      list_id: String,
      entry_id: String,
      kind: DenylistKindV1,   // url | hash | cid | account | manifest | provider
      value: String,
      jurisdiction: Option<String>,
      issued_at: Timestamp,
      expires_at: Option<Timestamp>,
      reason_code: Option<String>,
      confidence: Option<u8>, // 0-100
      source_signature: Signature,
  }
  ```
- **Update packaging**:
  ```norito
  struct ComplianceUpdateV1 {
      update_id: Uuid,
      generated_at: Timestamp,
      entries: Vec<DenylistEntryV1>,
      removed_entries: Vec<String>, // entry_id
      policy_snapshot: CompliancePolicyV1,
      signature: Signature,
  }
  ```
- Updates published every 10 minutes (incremental) and daily full snapshot pinned to IPFS. Gateways use `If-None-Match` semantics with `update_id`.
- Controller maintains dedupe LRU to avoid replay; update history stored in Postgres (`compliance_updates` table).
- Gateways acknowledge updates via Torii (`POST /compliance/update/ack`). Missing ack -> alert.
- **Operator tooling:** `cargo xtask sorafs-gateway denylist pack --input <denylist.json> [--out <dir>] [--label <name>]` canonicalises the JSON denylist, computes the Merkle root, and writes:
  - `<label>_<timestamp>.json` — signed-friendly bundle JSON (with canonical entries + proofs).
  - `<label>_<timestamp>.to` — Norito-encoded bundle for Torii ingestion.
  - `<label>_<timestamp>_root.txt` — textual Merkle root + timestamp for audit attachments.
  The packer enforces stable ordering and produces inclusion proofs so compliance can attach the artefacts to GAR attestations and transparency reports without re-running bespoke scripts.
- **Change tracking:** `cargo xtask sorafs-gateway denylist diff --old <bundle.json> --new <bundle.json> [--report-json <path>]` compares two packed bundles, prints the added/removed entries, and optionally emits a JSON evidence record. Ministry reviewers attach the diff output to the MINFO-6 governance packet so every denylist promotion ships with a deterministic before/after trail.
- **Bundle verification:** `cargo xtask sorafs-gateway denylist verify --bundle <bundle.json> [--norito <bundle.to>] [--root <root.txt>] [--report-json <path>]` recomputes every canonical entry, Merkle proof, Norito payload, and `_root` sidecar before an operator pushes an update. The command fails fast on digest, proof, or metadata drift and can emit a JSON receipt for inclusion in GAR submissions so reviewers know the published artefacts were locally validated.

## Policy Matrix & Moderation Toggles
- **Policy matrix** (`compliance_policy.toml`) maps `list_id` to actions:
  ```
  [lists.badbits]
  actions = ["block_content", "log_incident", "gar_notify"]
  severity = "critical"
  
  [lists.eu_dsa]
  actions = ["geo_block", "log_incident", "legal_notify"]
  jurisdictions = ["EU"]
  
  [lists.operator_manual]
  actions = ["block_content"]
  ```
- **Operator toggles**:
  - API `POST /v2/moderation/toggle` accepts `ToggleRequestV1` (gateway_id, policy_id, enabled, expires_at, reason, operator_signature).
  - Requires quorum: default 2-of-3 for production; toggles stored in `moderation_toggles` table with full audit record.
  - Gateways poll for toggles; apply union of global policy + per-gateway overrides.
- **Categories**: content types (illegal, copyright, sensitive), network-level toggles (denylist-only, manual), feature toggles (Proof tokens on/off for testing).
- **Appeal integration**: Toggles reference `appeal_case_id` from SFM-4b when toggles applied due to panel decision.

## Enforcement Workflow
- **Request interception**:
  1. For incoming request/gateway streaming event, compliance module computes evidence vector: `(manifest_cid, provider_id, account_id, url, ip, jurisdiction)`.
  2. Check active entries in RocksDB using prefix indexes per kind. Evaluate policy matrix + toggles.
  3. If action includes `geo_block`, examine request geo (MaxMind DB) and enforce accordingly.
  4. On block: return HTTP 451 (Unavailable For Legal Reasons) or configured status. Include `Sora-Moderation-Token`.
- **Response instrumentation**:
  - Always set `Sora-Compliance-Version` header with update ID and policy version.
  - For allowed content flagged as sensitive (via AI pre-screen warnings), optionally add `Sora-Moderation-Warning`.
- **Proof tokens** (`ProofTokenV1`):
  ```norito
  struct ProofTokenV1 {
      token_id: Uuid,
      moderation_type: ModerationTypeV1,
      entry_ids: Vec<String>,
      issued_at: Timestamp,
      expires_at: Option<Timestamp>,
      blinded_digest: Digest32,  // HMAC over evidence + secret
      gateway_signature: Ed25519Signature,
  }
  ```
  - Gateway logs token and includes in response header `Sora-Moderation-Token: base64url(...)`.
  - Transparency reporter aggregates tokens; clients can present token for appeal/review without revealing exact resource.
  - Implementation lives in `iroha_crypto::sorafs::proof_token` and emits the
    `SFGT` binary frame (`magic || version || flags || moderation || issued_at || expires_at?
    || token_id || entry_count || entries || blinded_digest || signature`). The helper clamps lists
    to 32 entries (≤255 B each), derives `blinded_digest` via keyed BLAKE3 with
    domain `sorafs.proof_token.digest.v1`, signs `SIGNING_DOMAIN || body` using
    Ed25519, and surfaces `encode_base64()` for header wiring.【crates/iroha_crypto/src/sorafs/proof_token.rs:1】
  - Digest keys rotate each month; only auditors/Governance receive the keyed
    digest secret while the Ed25519 public key is published so clients can
    verify signatures without learning the evidence payload.
  - Gateways also emit `Sora-Cache-Version` (mirrored as `sora-cache-version` in client fetchers);
    policy proofs are bound to `blake3(b"sorafs.policy.binding.v1" || cache_version)` so stale
    cache entries cannot replay tokens. The orchestrator honey probe helper
    (`sorafs_car::policy::run_honey_probe`) enforces 451/`denylisted` status, cache-version
    presence, and proof-token verification for each provider before adoption drills.

## Transparency Reporting
- Weekly report (`TransparencyReportV1`):
  ```norito
  struct TransparencyReportV1 {
      report_id: Uuid,
      period_start: Timestamp,
      period_end: Timestamp,
      gateway_id: String,
      summary: TransparencySummaryV1,
      stats: Vec<TransparencyMetricV1>,
      tokens_published: Vec<ProofTokenIndexV1>,
      signature: Signature,
  }
  ```
  - Stats include counts per list, per jurisdiction, appeal outcomes, average response time.
  - Reports published to Governance DAG and pinned to IPFS; hashed for tamper evidence.
- Monthly aggregated report combining all gateways + legal commentary.
- Dashboards fetch `TransparencyReportV1` and display metrics via Grafana.

## Observability & Alerts
- Metrics:
  - `sorafs_compliance_update_lag_seconds`
  - `sorafs_compliance_entries_total{list_id,action}`
  - `sorafs_compliance_hits_total{gateway_id,list_id,action}`
  - `sorafs_moderation_toggles_active{policy_id}`
  - `sorafs_proof_tokens_issued_total{gateway_id,type}`
  - `sorafs_compliance_update_errors_total`
- Logs: JSON logs per enforcement event (`compliance_event`) capturing request ID, gateway, entry_id, action, policy version, token_id.
- Alerts:
  - Update lag > 30 minutes (warning) / > 2 hours (critical).
  - Unknown list entry encountered.
  - Toggle approval backlog > 12 hours.
  - Proof token issuance failure rate > 1% (signature issues).

## Security & Governance
- Updates signed with Dilithium3; keys stored in HSM. Gateways reject unsigned/invalid updates.
- TLS/mTLS for update transport. Gateways verify certificate + OCSP.
- Toggle approvals require governance multi-sig; audit log hashed and committed daily to Governance DAG.
- Proof tokens hashed with secret per gateway rotated monthly; tokens include expiry.
- Privacy: moderation tokens avoid PII; only hashed identifiers stored.
- Disaster recovery: controller keeps last 30 updates in local storage; gateways fallback to last-known good with degrade mode.

## Testing Strategy
- Unit tests for policy evaluation, toggle application, proof token signing.
- Integration tests with simulated denylists, verifying update propagation and enforcement.
- Chaos tests: drop updates, corrupt entries, assert gateway alerts and safe mode.
- Performance: ensure compliance checks add < 1 ms median latency.
- Security tests: signature forgery attempts, toggle misuse, injection protections.
- Reporting tests: verify transparency report matches logged events & Merkle root.

## Rollout Plan
1. Implement compliance controller, Norito schemas, gateway module (feature-flagged).
2. Deploy staging environment with synthetic denylists; run enforcement tests, token verification, transparency report generation.
3. Integrate operator UI/API for toggles; complete audit workflow.
4. Connect AI pre-screen service (SFM-4a) to feed quarantine lists; integrate appeals pipeline (SFM-4b) for override decisions.
5. Production rollout:
   - Stage 0: passive mode (log-only) to ensure accuracy.
   - Stage 1: enable enforcement for BadBits + manual lists; monitor metrics.
   - Stage 2: enable regional lists, proof tokens, transparency reports; publish first weekly report.
   - Stage 3: enforce automatic blocking for critical lists with alert gating.
6. Update operator documentation (`docs/source/sorafs_gateway_operator_playbook.md`) and transparency dashboards.
7. Record completion in status/roadmap once metrics stable and compliance audits pass.

## Implementation Checklist
- [x] Define compliance schemas, policy matrix, and update mechanism.
- [x] Specify gateway enforcement workflow, proof tokens, and toggles.
- [x] Document transparency reporting and publication surfaces.
- [x] Capture observability metrics, alerts, and security requirements.
- [x] Outline testing strategy and rollout sequence.
- [x] Note integration points with AI pre-screen, appeals, and dashboards.

With this specification, teams can implement the compliance, moderation, and transparency layer needed to satisfy legal requirements and maintain community trust across the SoraFS gateway network.
