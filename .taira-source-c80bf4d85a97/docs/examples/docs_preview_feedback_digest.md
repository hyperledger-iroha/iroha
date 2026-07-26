# Docs Portal Preview Feedback Digest (Template)

Use this template when summarising a preview wave for governance, release
reviews, or `status.md`. Copy the Markdown into the tracking ticket, replace
placeholders with real data, and attach the JSON summary exported via
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. The
`preview:digest` helper (`npm run --prefix docs/portal preview:digest -- --wave <label>`)
generates the metrics section shown below so you only need to fill in the
highlights/actions/artefacts rows.

```markdown
## Wave preview-<tag> feedback digest (YYYY-MM-DD)
- Invite window: <start → end>
- Reviewers invited: <count> (open: <count>)
- Feedback submissions: <count>
- Issues opened: <count>
- Latest event timestamp: <ISO8601 from summary.json>

| Category | Details | Owner / Follow-up |
| --- | --- | --- |
| Highlights | <e.g., “ISO builder walkthrough landed well”> | <owner + due date> |
| Blocking findings | <list issue IDs or tracker links> | <owner> |
| Minor polish items | <group cosmetic or copy edits> | <owner> |
| Telemetry anomalies | <link to dashboard snapshot / probe log> | <owner> |

## Actions
1. <Action item + link + ETA>
2. <Optional second action>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Wave summary: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Dashboard snapshot: `<link or path>`

```

Keep each digest with the invite-tracking ticket so reviewers and governance can
replay the evidence trail without digging through CI logs.
