# Status

## 2026-01-19
- Proposal assembly now schedules missing-block fetches when the highest QC parent is unavailable locally.
- Proposal assembly defers early when the highest QC block is missing to avoid building on the wrong parent.
