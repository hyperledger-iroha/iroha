---
lang: az
direction: ltr
source: docs/automation/da/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8757f0bf8699b532ece29437af953353526b3201b4b129ebec7d6bf5d224f038
source_last_modified: "2025-12-29T18:16:35.061402+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Data-Availability Threat-Model Automation (DA-1)

Roadmap item DA-1 and `status.md` call for a deterministic automation loop that
produces the Norito PDP/PoTR threat-model summaries surfaced in
`docs/source/da/threat_model.md` and the Docusaurus mirror. This directory
captures the artefacts referenced by:

- `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`
- `.github/workflows/da-threat-model-nightly.yml`
- `make docs-da-threat-model` (which runs `scripts/docs/render_da_threat_model_tables.py`)
- `cargo xtask da-commitment-reconcile --receipt <path> --block <path> [--json-out <path|->]`
- `cargo xtask da-privilege-audit --config <torii.toml> [--extra-path <path> ...] [--json-out <path|->]`

## Flow

1. **Generate the report**
   ```bash
   cargo xtask da-threat-model-report \
     --config configs/da/threat_model.toml \
     --out artifacts/da/threat_model_report.json
   ```
   The JSON summary records the simulated replication failure rate, chunker
   thresholds, and any policy violations detected by the PDP/PoTR harness in
   `integration_tests/src/da/pdp_potr.rs`.
2. **Render the Markdown tables**
   ```bash
   make docs-da-threat-model
   ```
   This runs `scripts/docs/render_da_threat_model_tables.py` to rewrite
   `docs/source/da/threat_model.md` and `docs/portal/docs/da/threat-model.md`.
3. **Archive the artefact** by copying the JSON report (and optional CLI log) to
   `docs/automation/da/reports/<timestamp>-threat_model_report.json`. When
   governance decisions rely on a specific run, include the git commit hash and
   simulator seed in a sibling `<timestamp>-metadata.md`.

## Evidence Expectations

- JSON files should remain <100 KiB so they can live in git. Larger execution
  traces belong in external storage—reference their signed hash in the metadata
  note if needed.
- Each archived file must list the seed, config path, and simulator version so
  reruns can be reproduced exactly when auditing DA release gates.
- Link back to the archived file from `status.md` or the roadmap entry whenever
  the DA-1 acceptance criteria advance, ensuring reviewers can verify the
  baseline without rerunning the harness.

## Commitment Reconciliation (Sequencer Omission)

Use `cargo xtask da-commitment-reconcile` to compare DA ingest receipts against
DA commitment records, catching sequencer omission or tampering:

```bash
cargo xtask da-commitment-reconcile \
  --receipt artifacts/da/receipts/ \
  --block storage/blocks/ \
  --json-out artifacts/da/commitment_reconciliation.json
```

- Accepts receipts in Norito or JSON form and commitments from
  `SignedBlockWire`, `.norito`, or JSON bundles.
- Fails when any ticket is missing from the block log or when hashes diverge;
  `--allow-unexpected` ignores block-only tickets when you intentionally scope
  the receipt set.
- Attach the emitted JSON to governance packets/Alertmanager for omission
  alerts; defaults to `artifacts/da/commitment_reconciliation.json`.

## Privilege Audit (Quarterly Access Review)

Use `cargo xtask da-privilege-audit` to scan the DA manifest/replay directories
(plus optional extra paths) for missing, non-directory, or world-writable
entries:

```bash
cargo xtask da-privilege-audit \
  --config configs/torii.dev.toml \
  --extra-path /var/lib/iroha/da-manifests \
  --json-out artifacts/da/privilege_audit.json
```

- Reads the DA ingest paths from the provided Torii config and inspects Unix
  permissions where available.
- Flags missing/not-a-directory/world-writable paths and returns a non-zero exit
  code when issues are present.
- Sign and attach the JSON bundle (`artifacts/da/privilege_audit.json` by
  default) to quarterly access-review packets and dashboards.
