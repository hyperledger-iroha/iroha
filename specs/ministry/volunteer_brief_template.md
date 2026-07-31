---
title: Volunteer Brief Template
summary: Structured template for roadmap item MINFO-3a covering balanced briefs, fact tables, conflict disclosures, and moderation tags.
---

# Volunteer Brief Template (MINFO-3a)

Roadmap reference: **MINFO-3a — Balanced brief templates & conflict disclosure.**

Volunteer brief submissions summarise positions that citizen panels want governance to review when blacklist changes or other Ministry enforcement motions are proposed. MINFO-3a requires that every brief follows a deterministic structure so the transparency pipeline can (1) render comparable fact tables, (2) confirm that conflicts-of-interest are disclosed, and (3) drop or flag off-topic submissions automatically. This page defines the canonical fields, CSV-style fact table layout, and moderation tags expected by the tooling shipped in `cargo xtask ministry-transparency`.

> **Norito schema:** the `iroha_data_model::ministry::VolunteerBriefV1` struct (version `1`) is the authoritative schema for all submissions. Repository-local validators call `VolunteerBriefV1::validate` before accepting a brief or referencing it in panel summaries.

## Submission payload structure

| Section | Fields | Requirements |
|---------|--------|--------------|
| **Envelope** | `version` (u16) | Must be `1`. The version guard allows the Ministry to evolve the schema without ambiguity. |
| **Identity & stance** | `brief_id` (string, unique per calendar year), `proposal_id` (links to the blacklist or policy motion), `language` (BCP-47), `stance` (`support`/`oppose`/`context`), `submitted_at` (RFC 3339) | All fields required. `stance` feeds dashboards and must match the allowed vocabulary. |
| **Author info** | `author.name`, `author.organization` (optional), `author.contact`, `author.no_conflicts_certified` (bool) | `author.contact` is redacted from public dashboards but stored in the raw artefact. Set `no_conflicts_certified: true` only if the author attests that no disclosures apply. |
| **Summary** | `summary.title`, `summary.abstract`, `summary.requested_action` | Textual overview surfaced beside the fact table. Limit `summary.abstract` to ≤2 000 characters. |
| **Fact table** | `fact_table` array (see next section) | Required even for short briefs. The CLI and transparency ingest job reject submissions without a fact table. |
| **Disclosures** | `disclosures` array OR `author.no_conflicts_certified: true` | Each disclosure row must include `type` (`financial`, `employment`, `governance`, `family`, `other`), `entity`, `relationship`, and `details`. |
| **Moderation metadata** | `moderation.off_topic` (bool), `moderation.tags` (array of enum strings), `moderation.notes` | Used by reviewers to suppress astroturfing or unrelated submissions. Off-topic entries do not contribute to dashboards. |

## Fact table specification

Each `fact_table` row captures a machine-readable claim. Store the rows as JSON objects with the following fields:

| Field | Description |
|-------|-------------|
| `claim_id` | Stable identifier (e.g., `VB-2026-04-F1`). |
| `claim` | Single-sentence statement of fact or impact. |
| `status` | One of `corroborated`, `disputed`, `context-only`. |
| `impact` | Array containing one or more of `governance`, `technical`, `compliance`, `community`. |
| `citations` | Non-empty array of strings. URLs, Torii case IDs, or CID references are accepted. |
| `evidence_digest` | Optional BLAKE3 checksum of supporting documents. |

Automation notes:
- The ingest job counts `fact_rows` and `fact_rows_with_citation` to build publication scorecards. Rows without citations still appear in the human-readable table but are tracked as missing evidence.
- Keep claims concise and reference the same identifiers used in governance proposals so cross-linking is deterministic.

## Conflict disclosure requirements

1. Provide at least one disclosure entry when a financial, employment, governance, or familial tie exists.
2. Use `author.no_conflicts_certified: true` to assert “no known conflicts.” Submissions must include either a disclosure entry or a `true` certification; otherwise, they’re flagged during ingest.
3. Include `disclosures[i].evidence` whenever public documentation exists (e.g., corporate filings, DAO votes). Evidence is optional for “none” certifications but strongly recommended.

## Moderation tags & off-topic handling

Moderation reviewers can label submissions before they enter the transparency pipeline:

- `moderation.off_topic: true` removes the entry from aggregate counts while incrementing an `off_topic_rejections` counter. The row is still available in raw archives for audit.
- `moderation.tags` accepts enum values: `duplicate`, `needs-translation`, `needs-follow-up`, `spam`, `astroturf`, `policy-escalation`. Tags help downstream reviewers triage without re-reading the full brief.
- `moderation.notes` stores a short justification for the moderation decision (≤512 characters).

## Submission checklist

1. Fill out the JSON payload using this template or the helper CLI described below.
2. Populate at least one fact table row; include citations for each row.
3. Provide disclosures or explicitly set `author.no_conflicts_certified: true`.
4. Attach moderation metadata (default `off_topic: false`) so reviewers can triage quickly.
5. Validate the payload with `cargo xtask ministry-transparency ingest --volunteer <file>` or any Norito validator before uploading.

## Validation CLI (MINFO-3)

The repository now ships a dedicated validator for volunteer briefs:

```bash
cargo xtask ministry-transparency volunteer-validate \
  --input fixtures/documentation/ministry/volunteer_brief_template.json \
  --json-output artifacts/ministry/volunteer_lint_report.json
```

Key behaviour:

- Accepts individual JSON objects *or* arrays of briefs; pass `--input` multiple times to lint several files in one run.
- Emits a per-brief summary showing the number of errors and warnings; warnings highlight empty citation lists or overlong notes, while errors block publication.
- Ensures required fields (`brief_id`, `proposal_id`, `stance`, fact table contents, disclosures or `no_conflicts_certified`) match this template and that enum values stay within the documented vocabularies.
- When `--json-output <path>` is set the validator writes a machine-readable manifest summarising every brief (proposal id, stance, status, errors/warnings). Store this report with the governed intake and release evidence so reviewers can reproduce the validation result.

Run the command in repository-local CI or governed intake workflows to keep volunteer submissions compliant with **MINFO-3** before they reach the transparency ingest job.

## Example payload

See `fixtures/documentation/ministry/volunteer_brief_template.json` for a fully populated example, including fact table rows, disclosures, and moderation tags. Downstream dashboards consume the raw JSON and automatically calculate:

- `total_briefs` (off-topic submissions excluded)
- `fact_rows` / `fact_rows_with_citation`
- `disclosures_missing`
- `off_topic_rejections`

If new fields are required, update this document and the ingest summariser (`xtask/src/ministry.rs`) in the same change so the governance evidence remains reproducible.

## Publication and evidence handoff (MINFO-3)

This repository owns the schema, validation, ingest tooling, sanitized metrics,
and governed release evidence. It does not contain a documentation site or
per-proposal web publisher. After validation:

1. Archive the input and optional JSON validation report with the governed
   intake evidence.
2. Supply accepted briefs with `--volunteer <file>` when running
   `cargo xtask ministry-transparency ingest`; off-topic entries remain
   auditable but do not contribute to aggregate counts.
3. Build, sanitize, and package the quarterly transparency release using the
   workflow in [`transparency_plan.md`](./transparency_plan.md). That workflow
   publishes the governed artifacts through SoraFS and the governance DAG.

Public user and operator guidance belongs in the sibling
[`hyperledger-iroha/iroha-docs`](https://github.com/hyperledger-iroha/iroha-docs)
repository and is published at <https://docs.iroha.tech/>. Keep repository-local
evidence and public guidance aligned without duplicating a site publisher
here.
