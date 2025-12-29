# SoraNet Salt Rotation Tabletop Drill — 2026-03-28

This directory stores evidence captured during the SNNet-1b harness tabletop drill
run on 2026-03-28. Operators replayed the salt rotation scenario with the relays
and client SDK harness to validate recovery timings and alert coverage.

Artifacts:
- `salt-announcement.norito.json` — Norito payload generated with
  `cargo run -p soranet-handshake-harness -- salt ...`.
- `salt-verify.log` — Verification output from
  `cargo run -p soranet-handshake-harness -- salt-verify`.
- `simulate-transcript.json` — Transcript captured via
  `cargo run -p soranet-handshake-harness -- simulate --only-salt --dump-transcript`.
- `telemetry-snapshots.json` — Aggregated metrics export documenting publish latency
  and client catch-up timings (P50/P95 within SLA).

All artefacts were redacted for secret material before committing to the repository;
operators retain the full log set in the governance evidence store.
