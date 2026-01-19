# Status

Last update: 2026-01-19

- Lint: not run
- Build: not run
- Tests: `cargo test -p ivm` (timed out after 20m while running `kotodama_foreach_reads_durable_state_map_entries`)
- Proposal assembly now schedules missing-block fetches when the highest QC parent is unavailable locally.
- Proposal assembly defers early when the highest QC block is missing to avoid building on the wrong parent.
