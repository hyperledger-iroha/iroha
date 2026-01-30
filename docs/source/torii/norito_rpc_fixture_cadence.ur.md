---
lang: ur
direction: rtl
source: docs/source/torii/norito_rpc_fixture_cadence.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51687c2241c2ee33d23d2a89ddf454665c4ba6fe099b0adb181f979f9ab69d47
source_last_modified: "2026-01-03T18:07:58.854203+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Norito-RPC Fixture Cadence (NRPC-3C)

Roadmap item **NRPC-3C** requires every SDK to exercise the shared
Norito-RPC fixture bundle twice per week so the Torii rollout can prove schema
parity before AND4 lands. This document captures the standing rotation,
execution steps, and evidence requirements for each run. The goal is to keep
`fixtures/norito_rpc/*.json` and the generated schema hashes aligned across the
Rust CLI, Swift, JS, and Android SDKs without bespoke scripts.

## Schedule & Ownership

| Day (UTC) | Primary | Secondary | Notes |
|-----------|---------|-----------|-------|
| Tuesday   | Rust CLI maintainer (LLM) | JS SDK maintainer | Regenerate fixtures after the weekly Torii merge window. |
| Friday    | Swift SDK maintainer | Android networking delegate | Final pre-weekend verification; captures feed QA status before AND4 rotations. |

Each run must be logged in the adoption tracker
(`docs/source/torii/norito_rpc_tracker.md`) with the artefact directory produced
by the script below. Swap duties via the #nrpc-fixtures Slack thread if you
cannot make your slot.

## Execution Steps

1. **Prep environment**
   - Ensure `cargo` and `python3` are available.
   - Pull the latest fixtures: `git pull origin master`.
   - Confirm `fixtures/norito_rpc/transaction_fixtures.manifest.json` and
     `fixtures/norito_rpc/schema_hashes.json` exist.
2. **Run the wrapper**
   ```bash
   ./scripts/run_norito_rpc_fixtures.sh \
     --sdk swift \
     --rotation "$(date -u +'%Y-%V')" \
     --note "weekly cadence" \
     --allow-online \
     --auto-report
   ```
   - `--sdk` identifies the participating SDK (`rust-cli`, `swift`, `js`,
     `android`).
   - `--rotation` is the calendar week or bespoke milestone label.
   - `--note` captures extra context (e.g., Torii PR numbers).
   - Omit `--allow-online` only when CI is running with a populated Cargo cache.
   - `--auto-report` keeps `artifacts/norito_rpc/rotation_status.{json,md}` fresh with a 7‑day staleness gate.
3. **Inspect exit status**
   - Success writes
     `artifacts/norito_rpc/<timestamp>-<sdk>-norito-rpc.*`.
   - Failures still produce logs; notify the next engineer and Torii Platform.
4. **Review the xtask report**
   - Check `<timestamp>-<sdk>-norito-rpc-xtask.json` for:
     - `schema_hashes` stability.
     - Missing fixtures (should be zero).
     - Unexpected additional payloads.
5. **Update tracker**
   - Append a row to `docs/source/torii/norito_rpc_tracker.md` including:
     - Date + SDK label.
     - Result (`passed` / `failed`).
     - Artefact directory (relative path).
     - Action items (e.g., “JS schema drift; PR #12345”).
6. **File incidents when required**
   - Any failure stemming from a Torii schema change must open a Torii platform
     issue before proceeding.

## Artefact Requirements

Every cadence run must archive the following under
`artifacts/norito_rpc/<stamp>-<sdk>-*/`:

- Console log (`*.log`).
- High-level summary (`*.json`).
- `cargo xtask norito-rpc-verify --json-out` payload (`*-xtask.json`).
- Git commit metadata recorded by the script (included in summaries).

These files feed `NRPC-4` adoption evidence and the AND4 readiness gate.

## Reporting & Evidence Automation

Use `scripts/norito_rpc_fixture_report.py` to aggregate the JSON summaries and
highlight stale cadence slots before the NRPC tracker review. The helper scans
`artifacts/norito_rpc/` for `*-norito-rpc.json` files, computes per-SDK freshness,
and emits both JSON (for governance attachments) and Markdown (for meeting
notes). Example:

```bash
python3 scripts/norito_rpc_fixture_report.py \
  --root artifacts/norito_rpc \
  --output artifacts/norito_rpc/rotation_status.json \
  --markdown artifacts/norito_rpc/rotation_status.md \
  --max-age-days 7
```

Attach `rotation_status.{json,md}` to the tracker and `status.md` updates so the
NRPC-4F1 cadence automation has deterministic metadata.

To keep these artefacts fresh automatically, pass `--auto-report` to
`scripts/run_norito_rpc_fixtures.sh` (default paths +
`--report-max-age-days 7`), or supply
`--report-json artifacts/norito_rpc/rotation_status.json --report-markdown artifacts/norito_rpc/rotation_status.md --report-max-age-days 7`
explicitly when customising paths. The wrapper calls the report helper after
each run so the aggregated files stay current without additional manual steps.

## FAQ

- **Why twice per week?** Torii schema changes cluster around the Tuesday merge
  window, with Friday runs catching regressions before weekend builds.
- **Can I regenerate fixtures manually?** Only via the script; it enforces
  metadata consistency and writes the JSON summaries required by the roadmap.
- **Do Android/Swift runs differ?** No. Even mobile SDKs consume the same
  Norito fixtures; the SDK label is purely informational.
- **Where do I reference results?**
  - `docs/source/torii/norito_rpc_tracker.md` (human-readable log)
  - `artifacts/norito_rpc/<stamp>-<sdk>-norito-rpc.json` (machine-readable summary)
  - `roadmap.md` (status bullets)

Keep this document in sync when cadence changes or new SDKs join the rotation.
