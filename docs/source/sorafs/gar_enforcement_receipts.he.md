---
lang: he
direction: rtl
source: docs/source/sorafs/gar_enforcement_receipts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f316f66e7a514e03454af63cc55cac52af7712166e9ab45523ded01957c58f1d
source_last_modified: "2026-01-22T15:56:36.412436+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# GAR Enforcement Receipts

Roadmap item **SNNet-15G1 — GAR enforcement receipts & audits** requires every
gateway action (purge, freeze, rate-limit change, legal hold, …) to emit a
deterministic artefact so auditors can trace policy decisions without replaying
logs. The `iroha_data_model::sorafs::gar::GarEnforcementReceiptV1` type captures
that artefact and is now the canonical payload exchanged between the CDN, DNS
automation, and governance tooling.

## Schema overview

| Field | Description |
|-------|-------------|
| `receipt_id` | 16-byte identifier (ULID/guid) that correlates the receipt with telemetry and runbooks. |
| `gar_name` | Registered SoraDNS label (e.g., `docs.sora`). |
| `canonical_host` | Exact host affected by the action (e.g., `docs.gateway.sora.net`). |
| `action` | `GarEnforcementActionV1` enum describing the operation (`purge_static_zone`, `ttl_override`, `rate_limit_override`, `geo_fence`, `legal_hold`, `moderation`, `audit_notice`, or a caller-provided `custom` slug). |
| `triggered_at_unix` | Unix timestamp (seconds) when the enforcement began. |
| `expires_at_unix` | Optional expiry timestamp for temporary actions. |
| `policy_version` | Optional release/manifest label (e.g., `2026-q2`). |
| `policy_digest` | Optional BLAKE3 digest of the exact policy blob. |
| `operator` | Account ID of the guardian/operator that executed the change. |
| `reason` | Human-readable reason surfaced in dashboards/runbooks. |
| `notes` | Optional free-form note for follow-up investigations. |
| `evidence_uris` | References to supporting artefacts (logs, CAR manifests, dashboards). |
| `labels` | Machine-readable tags (guardian ticket, incident slug, drill name, etc.). |

All optional fields default to `None`/empty vectors so the receipts can be
emitted even when a GAR lacks a published digest.

## Example JSON / Norito payload

```json
{
  "receipt_id": "30313233343536373839616263646566",
  "gar_name": "docs.sora",
  "canonical_host": "docs.gateway.sora.net",
  "action": { "kind": "rate_limit_override" },
  "triggered_at_unix": 1747483200,
  "expires_at_unix": 1747569600,
  "policy_version": "2026-q2",
  "policy_digest": "abababababababababababababababababababababababababababababababab",
  "operator": "<i105-account-id>",
  "reason": "Guardian freeze window",
  "notes": "Escalated during SNNet-15 drill",
  "evidence_uris": [
    "sora://gar/receipts/docs/0123",
    "https://ops.sora.net/incidents/SN15-0001"
  ],
  "labels": [
    "guardian-freeze",
    "sn15-drill"
  ]
}
```

## Usage

- CDN/DNS automation emits a `GarEnforcementReceiptV1` every time an operator
  applies a GAR policy action. The receipt is persisted on-chain or archived in
  the governance evidence bundle.
- Observability exports the receipts through the existing SNS dashboards so
  auditors can filter by `labels`, `gar_name`, or `operator`.
- Governance CLI/SDK helpers consume the same Norito type, ensuring that
  evidence bundles, transparency reports, and automated GAR tests stay in sync.

### CLI helper (`sorafs gar receipt`)

Generate JSON + Norito artefacts straight from the CLI:

```bash
cargo run --bin iroha -- sorafs gar receipt \
  --gar-name docs.sora \
  --canonical-host docs.gateway.sora.net \
  --action rate-limit-override \
  --operator <i105-account-id> \
  --reason "Guardian freeze window" \
  --policy-version 2026-q2 \
  --policy-digest abab...abab \
  --label guardian-freeze \
  --label sn15-drill \
  --evidence-uri sora://gar/receipts/docs/0123 \
  --evidence-uri https://ops.sora.net/incidents/SN15-0001 \
  --json-out artifacts/gar/docs_receipt.json \
  --norito-out artifacts/gar/docs_receipt.to
```

Flags accept RFC 3339 (`--triggered-at 2026-05-10T10:15:00Z`) or `@unix`
timestamps, custom GAR action slugs
(`--action custom --custom-action-slug purge-l7`), and optional policy digests.
When `--json-out` / `--norito-out` are omitted, the command prints the JSON
payload to stdout while still allowing callers to pipe the Norito bytes into
downstream tooling.

See `crates/iroha_data_model/src/sorafs/gar.rs` for the canonical Norito
definition, and the SNNet-15G1 checklist in `roadmap.md` for the surrounding
automation requirements.

### Audit export helper (`cargo xtask soranet-gar-export`)

Bundle receipts and ACK files into a single JSON/Markdown report for
governance packets:

```bash
cargo xtask soranet-gar-export \
  --pop soranet-pop-m0 \
  --json-out artifacts/soranet/gateway/soranet-pop-m0/gar_receipts_summary.json \
  --markdown-out artifacts/soranet/gateway/soranet-pop-m0/gar_receipts_summary.md
```

Flags:
- `--pop <label>` picks defaults under `artifacts/soranet/gateway/<pop>/`
  (`gar_receipts/`, `gar_acks/`, and the summary paths) when explicit paths are
  not provided.
- `--receipts-dir` / `--acks-dir` override the receipt/ack roots (JSON or Norito
  receipts are both accepted).
- `--json-out <path|->` writes the merged summary (stdout when `-`).
- `--markdown-out <path>` emits a lightweight table for human review.
- `--now <unix>` computes receipt age and ack lag in the summary (optional).
