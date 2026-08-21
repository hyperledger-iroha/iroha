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
(`specs/torii/norito_rpc_tracker.md`) with the artefact directory produced
by the commands below. Swap duties via the #nrpc-fixtures Slack thread if you
cannot make your slot.

## Execution Steps

1. **Prep environment**
   - Ensure `cargo` and `python3` are available.
   - Pull the latest fixtures: `git pull origin master`.
   - Confirm `fixtures/norito_rpc/transaction_fixtures.manifest.json` and
     `fixtures/norito_rpc/schema_hashes.json` exist.
2. **Run the canonical verifier**
   ```bash
   mkdir -p artifacts/norito_rpc
   cargo run --locked -p xtask --features dev-tools --bin xtask -- \
     norito-rpc-verify \
     --json-out artifacts/norito_rpc/<stamp>-<sdk>-norito-rpc-xtask.json
   ```
   Replace `<stamp>` and `<sdk>` with the UTC run stamp and participating SDK
   label (`rust-cli`, `swift`, `js`, or `android`). Capture the command's console
   output next to the JSON report and record the rotation label and ticket in
   the tracker.
3. **Inspect exit status**
   - Success writes the requested report and prints the verified fixture count.
   - On failure, preserve the captured log and notify the next engineer and
     Torii Platform.
4. **Review the xtask report**
   - Check `<timestamp>-<sdk>-norito-rpc-xtask.json` for:
     - `schema_hashes` stability.
     - Missing fixtures (should be zero).
     - Unexpected additional payloads.
5. **Update tracker**
   - Append a row to `specs/torii/norito_rpc_tracker.md` including:
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
- `cargo run --locked -p xtask --features dev-tools --bin xtask -- norito-rpc-verify --json-out <report-path>` payload (`*-xtask.json`).
- Git commit metadata recorded alongside the report.

These files feed `NRPC-4` adoption evidence and the AND4 readiness gate.

## Reporting & Evidence Automation

Attach each cadence run's canonical `*-xtask.json` verification report,
console log, and Git identity directly to the tracker. There is no summary or
SDK-local regeneration compatibility entry point in V1.

## FAQ

- **Why twice per week?** Torii schema changes cluster around the Tuesday merge
  window, with Friday runs catching regressions before weekend builds.
- **Can I regenerate fixtures manually?** Only with
  `norito-rpc-fixtures --output-root <absent-absolute-external-root>`. Generate two
  independent sealed roots, require identical exact path sets, entry types,
  modes, completion manifests, and every file byte, and apply the reviewed
  identity-relative tracked patch before running
  `norito-rpc-verify`.
- **Do Android/Swift runs differ?** No. Even mobile SDKs consume the same
  Norito fixtures; the SDK label is purely informational.
- **Where do I reference results?**
  - `specs/torii/norito_rpc_tracker.md` (human-readable log)
  - `artifacts/norito_rpc/<stamp>-<sdk>-norito-rpc-xtask.json` (machine-readable report)
  - `roadmap.md` (status bullets)

Keep this document in sync when cadence changes or new SDKs join the rotation.
