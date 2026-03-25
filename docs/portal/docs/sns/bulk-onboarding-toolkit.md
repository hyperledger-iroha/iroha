<!--
  SPDX-License-Identifier: Apache-2.0
-->
---
id: bulk-onboarding-toolkit
title: SNS Bulk Onboarding Toolkit
sidebar_label: Bulk onboarding toolkit
description: CSV to RegisterNameRequestV1 automation for SN-3b registrar runs.
---

:::note Canonical Source
Mirrors `docs/source/sns/bulk_onboarding_toolkit.md` so external operators see
the same SN-3b guidance without cloning the repository.
:::

# SNS Bulk Onboarding Toolkit (SN-3b)

**Roadmap reference:** SN-3b "Bulk onboarding tooling"  
**Artifacts:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Large registrars often pre-stage hundreds of ledger-backed SNS names with the
same governance approvals and settlement rails. Manually crafting JSON payloads
or re-running the CLI does not scale, so SN-3b ships a deterministic CSV to
Norito builder that prepares `RegisterNameRequestV1` structures for Torii or
the CLI. The helper validates every row up front, emits both an aggregated
manifest and optional newline-delimited JSON, and can submit the payloads
automatically while recording structured receipts for audits.

## 1. CSV schema

The parser requires the following header row (order is flexible):

| Column | Required | Description |
|--------|----------|-------------|
| `label` | Yes | Requested label (mixed case accepted; tool normalises per Norm v1 and UTS-46). |
| `suffix_id` | Yes | Numeric namespace identifier (`0x1001` account-alias, `0x1002` domain, `0x1003` dataspace; decimal or `0x` hex accepted). |
| `owner` | Yes | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Yes | Integer `1..=255`. |
| `payment_asset_id` | Yes | Settlement asset (for example `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Yes | Unsigned integers representing asset-native units. |
| `settlement_tx` | Yes | JSON value or literal string describing the payment transaction or hash. |
| `payment_payer` | Yes | AccountId that authorised the payment. |
| `payment_signature` | Yes | JSON or literal string containing the steward or treasury signature proof. |
| `controllers` | Optional | Semicolon- or comma-separated list of controller account addresses. Defaults to `[owner]` when omitted. |
| `metadata` | Optional | Inline JSON or `@path/to/file.json` providing resolver hints, TXT records, etc. Defaults to `{}`. |
| `governance` | Optional | Inline JSON or `@path` pointing at a `GovernanceHookV1`. `--require-governance` enforces this column. |

Any column may reference an external file by prefixing the cell value with `@`.
Paths are resolved relative to the CSV file.

## 2. Running the helper

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Key options:

- `--require-governance` rejects rows without a governance hook (useful for
  premium auctions or reserved assignments).
- `--default-controllers {owner,none}` decides whether empty controller cells
  fall back to the owner account.
- `--controllers-column`, `--metadata-column`, and `--governance-column` rename
  optional columns when working with upstream exports.

On success the script writes an aggregated manifest:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":4098,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"4098":118,"4099":2}
  }
}
```

If `--ndjson` is provided, each `RegisterNameRequestV1` is also written as a
single-line JSON document so automations can stream requests directly into
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Automated submissions

### 3.1 Torii REST mode

Specify `--submit-torii-url` plus either `--submit-token` or
`--submit-token-file` to push every manifest entry directly into Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --submission-log artifacts/sns_bulk_submit.log
```

- The helper issues one `POST /v1/sns/names` per request and aborts on
  the first HTTP error. Responses are appended to the log path as NDJSON
  records.
- `--poll-status` re-queries `/v1/sns/names/{namespace}/{literal}` after each
  submission (up to `--poll-attempts`, default 5) to confirm that the record is
  visible. The helper derives `{namespace}` directly from the fixed `suffix_id`,
  so `--suffix-map` is no longer required for the canonical polling path.
- Tunables: `--submit-timeout`, `--poll-attempts`, and `--poll-interval`.

### 3.2 iroha CLI mode

To route each manifest entry through the CLI, supply the binary path:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controllers must be `Account` entries (`controller_type.kind = "Account"`)
  because the CLI currently exposes only account-based controllers.
- Metadata and governance blobs are written to temporary files per request and
  forwarded to `iroha sns register --metadata-json ... --governance-json ...`.
- CLI stdout and stderr plus exit codes are logged; non-zero exit codes abort
  the run.

Both submission modes can run together (Torii and CLI) to cross-check registrar
deployments or rehearse fallbacks.

### 3.3 Submission receipts

When `--submission-log <path>` is provided, the script appends NDJSON entries
capturing:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Successful Torii responses include structured fields extracted from
`NameRecordV1` or `RegisterNameResponseV1` (for example `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) so dashboards and governance
reports can parse the log without inspecting free-form text. Attach this log to
registrar tickets alongside the manifest for reproducible evidence.

## 4. Docs portal release automation

CI and portal jobs call `docs/portal/scripts/sns_bulk_release.sh`, which wraps
the helper and stores artefacts under `artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

The script:

1. Builds `registrations.manifest.json`, `registrations.ndjson`, and copies the
   original CSV into the release directory.
2. Submits the manifest using Torii and/or the CLI (when configured), writing
   `submissions.log` with the structured receipts above.
3. Emits `summary.json` describing the release (paths, Torii URL, CLI path,
   timestamp) so portal automation can upload the bundle to artefact storage.
4. Produces `metrics.prom` (override via `--metrics`) containing
   Prometheus-format counters for total requests, suffix distribution,
   asset totals, and submission outcomes. The summary JSON links to this file.

Workflows simply archive the release directory as a single artefact, which now
contains everything governance needs for auditing.

## 5. Telemetry & dashboards

The metrics file generated by `sns_bulk_release.sh` exposes the following
series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="4098"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Feed `metrics.prom` into your Prometheus sidecar (for example via Promtail or a
batch importer) to keep registrars, stewards, and governance peers aligned on
bulk progress. Grafana board
`dashboards/grafana/sns_bulk_release.json` visualises the same data with panels
for per-suffix counts, payment volume, and submission success/failure ratios.
The board filters by `release` so auditors can drill into a single CSV run.

## 6. Validation and failure modes

- **Label canonicalisation:** inputs are normalised with Python IDNA plus
  lowercase and Norm v1 character filters. Invalid labels fail fast before any
  network calls.
- **Numeric guardrails:** suffix ids, term years, and pricing hints must fall
  within `u16` and `u8` bounds. Payment fields accept decimal or hex integers
  up to `i64::MAX`.
- **Metadata or governance parsing:** inline JSON is parsed directly; file
  references are resolved relative to the CSV location. Non-object metadata
  produces a validation error.
- **Controllers:** blank cells honour `--default-controllers`. Provide explicit
  controller lists (for example `i105...;i105...`) when delegating to non-owner
  actors.

Failures are reported with contextual row numbers (for example
`error: row 12 term_years must be between 1 and 255`). The script exits with
code `1` on validation errors and `2` when the CSV path is missing.

## 7. Testing and provenance

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` covers CSV parsing,
  NDJSON emission, governance enforcement, and the CLI or Torii submission
  paths.
- The helper is pure Python (no additional dependencies) and runs anywhere
  `python3` is available. Commit history is tracked alongside the CLI in the
  main repository for reproducibility.

For production runs, attach the generated manifest and NDJSON bundle to the
registrar ticket so stewards can replay the exact payloads that were submitted
to Torii.
