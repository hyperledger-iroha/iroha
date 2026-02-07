---
lang: hy
direction: ltr
source: docs/automation/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c56bacde8ee42c2427d06038a3a6ca65035d4055c42f6e5ded7e54b33c1fe921
source_last_modified: "2025-12-29T18:16:35.060432+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Documentation Automation Baselines

This directory captures the automation surfaces that roadmap items such as
AND5/AND6 (Android Developer Experience + Release Readiness) and DA-1
(Data-Availability threat-model automation) refer to when they call for
auditable documentation evidence. Staging the command references and expected
artefacts in-tree keeps the prerequisites for compliance reviews available even
when CI pipelines or dashboards are offline.

## Directory Layout

| Path | Purpose |
|------|---------|
| `docs/automation/android/` | Android documentation and localization automation baselines (AND5), including i18n stub sync logs, parity summaries, and SDK publishing evidence required before AND6 sign-off. |
| `docs/automation/da/` | Data-Availability threat-model automation outputs referenced by `cargo xtask da-threat-model-report` and the nightly docs refresh. |

Each subdirectory documents the commands that produce the evidence along with
the file layout we expect to check in (usually JSON summaries, run logs, or
manifests). Teams drop new artefacts under the respective folder whenever an
automation run materially changes the published docs, then link to the commit
from the relevant status/roadmap entry.

## Usage

1. **Run the automation** using the commands described in the subdirectory
   README (for example, `ci/check_android_fixtures.sh` or
   `cargo xtask da-threat-model-report`).
2. **Copy the resulting JSON/log artefacts** from `artifacts/…` into the
   matching `docs/automation/<program>/…` folder with an ISO-8601 timestamp in
   the filename so auditors can correlate the evidence with governance minutes.
3. **Reference the commit** in `status.md`/`roadmap.md` when closing a roadmap
   gate so reviewers can confirm the automation baseline used for that decision.
4. **Keep the files lightweight**. The expectation is structured metadata,
   manifests, or summaries—not bulk binary blobs. Larger dumps should stay in
   object storage with the signed reference recorded here.

By centralising these automation notes we unblock the “docs/automation baselines
available for audit” prerequisite that AND6 calls out and give the DA threat
model flow a deterministic home for the nightly reports and manual spot checks.
