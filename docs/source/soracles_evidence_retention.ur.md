---
lang: ur
direction: rtl
source: docs/source/soracles_evidence_retention.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3121ec15db54ca27fab0f0e11a5780839b69eb46c7fa70994d97a6116cc28cf1
source_last_modified: "2026-01-04T10:50:53.653513+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Soracles Evidence Retention & GC

Roadmap item OR-14 requires an auditable retention policy for oracle evidence
artifacts plus tooling to prune stale bundles without mutating on-chain
hashes. This note documents the defaults and the `iroha soracles evidence-gc`
helper added alongside `bundle.json` metadata that records when a bundle was
generated.

## Retention policy

- **Observations, reports, connector responses, telemetry:** retain for **180
  days** by default. This keeps the full ingestion/audit trail across two
  quarters while keeping operator storage bounded. Adjust the GC flag if your
  governance policy requires a shorter/longer window.
- **Dispute evidence:** retain for **365 days** so reopened disputes or slow
  attestations can still be validated. The GC helper keeps bundles containing
  dispute artefacts under the longer **`--dispute-retention-days`** window by
  default.
- **On-chain hashes stay immutable.** `FeedEventRecord` keeps only
  `evidence_hashes`; pruning artifacts never touches ledger state. Attach GC
  reports to governance bundles when artifacts are removed so the audit trail
  stays intact.
- **Bundle timestamp:** `bundle.json` now carries
  `generated_at_unix` (seconds). GC prefers this timestamp and falls back to

## Running garbage collection

Use the new CLI helper to prune bundles older than the retention window and to
optionally drop unreferenced artifact files:

```bash
iroha soracles evidence-gc \
  --root artifacts/soracles \
  --retention-days 180 \
  --dispute-retention-days 365 \
  --prune-unreferenced \
  --report artifacts/soracles/gc_report.json
```

Flags:
- `--root`: directory containing bundle folders (each with `bundle.json`).
- `--retention-days`: remove bundles whose `generated_at_unix` (or `mtime` for
- `--dispute-retention-days`: minimum retention window for bundles that contain
  dispute evidence (default **365**). Use this to keep dispute payloads longer
  than standard observations/reports.
- `--prune-unreferenced`: delete files under `artifact_root` that are not
  referenced by `bundle.json`.
- `--dry-run`: report what would be removed without deleting anything.
- `--report`: optional path for the JSON summary; defaults to
  `<root>/gc_report.json`.

The report captures `removed_bundles`, `pruned_files`, `skipped_bundles`,
`retained_bundles`, `bytes_freed`, the retention window, and whether the run
was a dry-run. Include the report alongside the remaining evidence when
uploading to SoraFS so governance reviewers can trace why particular artifacts
were pruned.

## On-chain cleanup rules

- Evidence hashes on-chain are permanent; GC only removes off-chain copies.
- When pruned, leave the original `evidence_hashes` untouched and attach the
  GC report to the relevant governance packet or SoraFS bundle. This satisfies
  the immutability requirement while keeping storage lean.
- If a bundle must be republished after pruning, keep the same hashed artifact
  names (or include a manifest of pruned hashes) so verifiers understand which
  entries are intentionally absent.
