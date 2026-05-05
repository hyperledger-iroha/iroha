---
lang: uz
direction: ltr
source: docs/source/sns/bulk_onboarding_toolkit.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 060db7fabd91ca0d52d2d9103aa70a8a8d912fe16c42876cf7baef698bbe11b8
source_last_modified: "2026-01-22T16:26:46.589814+00:00"
translation_last_reviewed: 2026-02-07
title: SNS Bulk Onboarding Toolkit (SN-3b)
summary: CSV → RegisterNameRequestV1 automation for large registrants.
---

# SNS Bulk Onboarding Toolkit

**Roadmap reference:** SN-3b “Bulk onboarding tooling”  
**Artifacts:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`

Large registrars often need to pre-stage hundreds of `.sora` / `.nexus`
registrations using the same governance approvals and settlement rails.
Manually crafting JSON payloads or re-running the CLI quickly becomes tedious,
so SN-3b adds a deterministic CSV→Norito builder that prepares
`RegisterNameRequestV1` structures for Torii or the CLI.

The helper validates labels, payment proofs, term lengths, and selector
uniqueness up front, then emits both an aggregated manifest and optional
newline-delimited JSON so automations can stream requests directly into Torii
or `iroha sns register`.

## 1. CSV schema

The parser requires the following header row (order is flexible):

| Column | Required | Description |
|--------|----------|-------------|
| `label` | ✅ | Requested label (mixed case accepted; tool normalises per Norm v1/UTS‑46). |
| `suffix_id` | ✅ | Numeric suffix identifier (decimal or `0x` hex). |
| `owner` | ✅ | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | ✅ | Integer `1..=255`. |
| `payment_asset_id` | ✅ | Settlement asset (e.g., `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | ✅ | Unsigned integers representing asset-native units. |
| `settlement_tx` | ✅ | JSON value or literal string describing the payment transaction/hash. |
| `payment_payer` | ✅ | AccountId that authorised the payment. |
| `payment_signature` | ✅ | JSON or literal string containing the steward/treasury signature proof. |
| `controllers` | ⬜ | Semicolon- or comma-separated list of controller account addresses. Defaults to `[owner]` when omitted. |
| `metadata` | ⬜ | Inline JSON or `@path/to/file.json` providing resolver hints, TXT records, etc. Defaults to `{}`. |
| `governance` | ⬜ | Inline JSON or `@path` pointing at a `GovernanceHookV1`. `--require-governance` enforces this column. |

Any column may reference an external file by prefixing the cell value with
`@`. Paths are resolved relative to the CSV file.

## 2. Running the helper

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Key options:

- `--require-governance` — reject rows without a governance hook (useful for
  premium auctions or reserved assignments).
- `--default-controllers {owner,none}` — choose whether empty controller cells
  fall back to the owner account.
- `--controllers-column`, `--metadata-column`, `--governance-column` — rename
  optional columns when working with upstream exports.

On success the script writes an aggregated manifest:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
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
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

If `--ndjson` is provided, each `RegisterNameRequestV1` is also written as a
single-line JSON document—ideal for streaming into Torii:

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
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- The helper issues one `POST /v1/sns/names` per request and aborts on
  the first HTTP error. Responses (success/failure) are appended to the log path
  as NDJSON records.
- `--poll-status` re-queries `/v1/sns/names/{namespace}/{literal}` after each
  submission (up to `--poll-attempts`, default 5) to confirm that the record is
  visible. Provide `--suffix-map` (JSON of `suffix_id → "suffix"` values) so the
  tool can derive `{label}.{suffix}` literals for polling.
- Tunables: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 iroha CLI mode

To route each manifest entry through the existing CLI, supply the binary path:

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
- CLI stdout/stderr and exit codes are logged; non-zero exit codes abort the run.

Both submission modes can be combined (Torii + CLI) to cross-check registrar
deployments or rehearse fallbacks.

### 3.3 Submission receipts

When `--submission-log <path>` is provided, the script appends NDJSON entries
capturing:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"…"}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Successful Torii responses include structured fields extracted from
`NameRecordV1`/`RegisterNameResponseV1` (e.g., `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) so dashboards and governance reports can consume the log without
parsing the free-form `detail` string.

Attach this log to the registrar ticket alongside the manifest for reproducible
evidence.

## 4. Docs portal release automation

CI/CD jobs driving the docs portal now call
`docs/portal/scripts/sns_bulk_release.sh`, which wraps the helper and stores all
artefacts under `artifacts/sns/releases/<timestamp>/`:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
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

CI workflows simply archive the release directory as a single artefact, which
now contains everything governance needs for auditing.

## 5. Telemetry & dashboards

The metrics file generated by `sns_bulk_release.sh` exposes the following
series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Feed `metrics.prom` into your Prometheus sidecar (for example via Promtail or a
batch importer) to keep registrars, stewards, and governance peers aligned on
bulk progress. Grafana board
`dashboards/grafana/sns_bulk_release.json` visualises the same data with panels
for per-suffix counts, payment volume, and submission success/failure ratios.
The board filters by `release` so auditors can drill into a single CSV run.

## 6. Validation & failure modes

- **Label canonicalisation:** inputs are normalised with Python’s IDNA encoder +
  lowercase + Norm v1 character filters. Invalid labels fail fast before any
  network calls.
- **Numeric guardrails:** suffix ids, term years, and pricing hints must fall
  within `u16`/`u8` bounds. Payment fields accept decimal or hex integers up to
  `i64::MAX`.
- **Metadata/governance parsing:** inline JSON is parsed directly; file
  references are resolved relative to the CSV location. Non-object metadata
  produces a validation error.
- **Controllers:** blank cells honour `--default-controllers`. Provide explicit
  controller lists (e.g., `<i105-account-id>;<i105-account-id>`) when delegating to non-owner actors.

Failures are reported with contextual row numbers, e.g. `error: row 12
term_years must be between 1 and 255`. The script exits with code `1` on
validation errors and `2` when the CSV path is missing.

## 7. Testing & provenance

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` covers CSV parsing,
  NDJSON emission, governance enforcement, and the CLI/Torii submission paths.
- The helper is pure-Python (no additional dependencies) and runs anywhere
  `python3` is available. Commit history is tracked alongside the CLI in the
  main repository for reproducibility.

For production runs, attach the generated manifest and NDJSON bundle to the
registrar ticket so stewards can replay the exact payloads that were submitted
to Torii.
