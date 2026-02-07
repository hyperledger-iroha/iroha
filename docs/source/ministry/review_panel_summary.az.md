---
lang: az
direction: ltr
source: docs/source/ministry/review_panel_summary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7325e72d18ec406eb134622ab51211fbb6582ebcc26bd719499e209db70f761b
source_last_modified: "2025-12-29T18:16:35.983094+00:00"
translation_last_reviewed: 2026-02-07
title: Review Panel Summary Workflow (MINFO-4a)
summary: Generate the neutral referendum summary with balanced citations, AI manifest references, and volunteer brief coverage.
---

# Review Panel Summary (MINFO-4a)

Roadmap item **MINFO-4a — Neutral summary generator** requires a reproducible workflow that turns an accepted agenda proposal, the volunteer brief corpus, and the attested AI moderation manifest into a neutral referendum summary. The deliverable must:

- Record the output as a Norito structure (`ReviewPanelSummaryV1`) so governance can archive it alongside manifests and ballots.
- Lint the source material, failing fast when the review panel does not have balanced support/oppose coverage or when facts are missing citations.
- Reference the AI manifest and the proposal evidence bundle in every highlight, ensuring the policy jury sees both automated and human context before voting.

## CLI usage

The workflow ships as part of `cargo xtask`:

```bash
cargo xtask ministry-panel synthesize \
  --proposal docs/examples/ministry/agenda_proposal_example.json \
  --volunteer docs/examples/ministry/volunteer_brief_template.json \
  --ai-manifest docs/examples/ai_moderation_calibration_manifest_202602.json \
  --panel-round RP-2026-05 \
--output artifacts/review_panel/AC-2026-001-RP-2026-05.json
```

Required inputs:

1. `--proposal` – JSON payload adhering to `AgendaProposalV1`. The helper validates the schema before generating the summary.
2. `--volunteer` – JSON array of volunteer briefs that follow `docs/source/ministry/volunteer_brief_template.md`. Off-topic entries are ignored automatically.
3. `--ai-manifest` – Governance-signed `ModerationReproManifestV1` describing the AI committee that screened the content.
4. `--panel-round` – Identifier for the current review round (`RP-YYYY-##`).
5. `--output` – Destination file or `-` to stream to stdout. Use `--language` to override the proposal language and `--generated-at` to supply a deterministic Unix timestamp (milliseconds) when backfilling history.

Once the standalone summary is generated, run the
[`cargo xtask ministry-panel packet`](referendum_packet.md) helper to assemble
the complete referendum dossier (`ReferendumPacketV1`). Supplying
`--summary-out` to the packet command will persist the same summary file while
embedding it inside the packet object for downstream consumers.

### Automation via `ministry-transparency ingest`

Teams that already run `cargo xtask ministry-transparency ingest` for quarterly evidence bundles can now stitch the review panel summary into the same pipeline:

```bash
cargo xtask ministry-transparency ingest \
  --quarter 2026-Q4 \
  --ledger artifacts/ministry/ledger.json \
  --appeals artifacts/ministry/appeals.json \
  --denylist artifacts/ministry/denylist.json \
  --treasury artifacts/ministry/treasury.json \
  --volunteer artifacts/ministry/volunteer_briefs.json \
  --panel-proposal artifacts/ministry/proposal_AC-2026-041.json \
  --panel-ai-manifest artifacts/ministry/ai_manifest.json \
  --panel-round RP-2026-05 \
  --panel-summary-out artifacts/ministry/review_panel_summary.json \
  --output artifacts/ministry/ingest.json
```

All four `--panel-*` flags must be supplied together (and require `--volunteer`). The command emits the review panel summary to `--panel-summary-out`, embeds the parsed payload inside the ingest snapshot, and records a checksum so downstream tooling can attest to the evidence.

## Linting and failure modes

`cargo xtask ministry-panel synthesize` enforces the following invariants before writing the summary:

- **Balanced stances:** at least one support brief and one oppose brief must be present. Missing coverage terminates the run with a descriptive error.
- **Citation coverage:** highlights are only produced from fact rows that include citations. Missing citations never block the build, but each affected brief is listed under `warnings[]` in the output.
- **Per-highlight references:** every highlight includes references to (a) the volunteer fact row(s), (b) the AI manifest ID, and (c) the first evidence attachment from the proposal so the packet always links back to the signed artefacts.

If any check fails, the command exits with a non-zero status and points at the problematic record. Successful runs write a JSON file that matches the `ReviewPanelSummaryV1` schema and can be embedded in governance manifests.

## Output structure

`ReviewPanelSummaryV1` lives in `crates/iroha_data_model/src/ministry/mod.rs` and is available to every consumer via the `iroha_data_model` crate. Key sections include:

- `overview` – Title, neutral summary sentence, and decision context for the policy jury packet.
- `stance_distribution` – Count of briefs and fact rows per stance. Downstream dashboards read this to confirm coverage before publishing.
- `highlights` – Up to two fact summaries per stance with fully qualified citations.
- `ai_manifest` – Extracted metadata from the reproducibility manifest (manifest UUID, runner version, thresholds).
- `volunteer_references` – Per-brief statistics (language, stance, rows, cited rows) for audit.
- `warnings` – Free-form lint messages describing skipped items (e.g., fact rows with missing citations).

## Example

`docs/examples/ministry/review_panel_summary_example.json` contains a full sample produced with the helper. It demonstrates balanced support/oppose coverage, citation wiring, manifest references, and warning strings for fact rows that could not be promoted to highlights. Use it when extending dashboards, governance manifests, or SDK tooling that need to consume the neutral summary.

> **Tip:** include the generated summary alongside the signed AI manifest and volunteer brief digest in the referendum evidence bundle so policy juries can verify every artifact referenced by the review panel.
