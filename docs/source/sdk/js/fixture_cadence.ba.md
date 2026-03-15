---
lang: ba
direction: ltr
source: docs/source/sdk/js/fixture_cadence.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5e18cbfbdaf25c2f2ec6432292dbb9a160039a3a696d7f35c6e8eaa8fd1cad9f
source_last_modified: "2025-12-29T18:16:36.057941+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# JavaScript Fixture Cadence Brief

This brief records how the JavaScript SDK keeps its fixtures aligned with the
canonical Norito/Torii artefacts shared across Android, Python, and Swift. It
fulfils the “cross-SDK fixture cadence alignment” roadmap item by documenting
scope, ownership, cadence, evidence, and rollback expectations.

## Scope

- **Shared fixture directories.** Unit tests and recipes consume the canonical
  JSON/TO fixtures stored under `./fixtures/**`, including Torii responses,
  SoraFS manifests/chunk plans, Norito instruction vectors, Soradns puzzles,
  and zk assets.【javascript/iroha_js/test/toriiClient.test.js:33】【javascript/iroha_js/test/sorafsChunker.fixture.test.js:10】
- **JS-specific projections.** The Torii client test harness replays captured
  projections stored in `javascript/iroha_js/test/fixtures/torii_responses.json`
  so the SDK mirrors the same DTO shapes as the Python/Swift suites.【javascript/iroha_js/test/toriiClient.test.js:33】
- **Integration stack.** `scripts/run_integration.mjs` starts the canonical
  `defaults/docker-compose.single.yml` stack and exercises the latest fixtures
  via `/v1/pipeline`/Connect/ISO helpers, ensuring regression coverage every
  time the fixture bundle rotates.【javascript/iroha_js/scripts/run_integration.mjs:1】

## Cadence

- **Baseline slot:** Wednesday 17:00 UTC, matching the IOS2/AND1 cadence so all
  SDKs refresh within the same 48 h SLA.【docs/source/sdk/swift/ios2_fixture_cadence_brief.md:1】
- **Rotation order:** Android Foundations TL owns odd ISO weeks, JS Lead owns
  even weeks; Python joins whichever owner is active to validate cross-SDK
  diffs before merging.
- **Event-driven regen:** If Torii lands an urgent ABI/discriminator change,
  the change author triggers an out-of-band refresh within 24 h and tags the
  `status.md` entry with `event-driven` plus the Jira ticket/commit.
- **Fallback window:** When governance delays the canonical slot, run a
  Monday/Thursday cadence (mirroring the Swift fallback) and annotate
  `SWIFT_FIXTURE_EXPECTED_CADENCE` with both labels so CI tolerates the wider
  window.【docs/source/sdk/swift/ios2_fixture_cadence_brief.md:33】

## Procedure

1. **Sync canonical artefacts.**
   - Pull the latest signed Norito bundle (or run
     `scripts/swift_fixture_regen.sh` if you are the canonical owner) so
     `fixtures/**` matches the Rust/Android source of truth.【scripts/swift_fixture_regen.sh:1】
   - Update `javascript/iroha_js/test/fixtures/torii_responses.json` when Torii
     projections change; keep the responses trimmed to the fields used by the
     SDK tests.
2. **Diff & review.**
   - Open a PR containing only the fixture changes plus any test adjustments.
   - Tag the Android + Python maintainers for review; diff bot should emit the
     structured comparison so reviewers can confirm nothing regresses.
3. **Validate.**
   - Run `npm test` plus `npm run test:integration` (with
     `IROHA_TORII_INTEGRATION_MUTATE=1`) to prove the new fixtures work across
     the unit and integration suites.【javascript/iroha_js/scripts/run_integration.mjs:1】
   - For Connect/ISO updates, rerun `npm run release:provenance` so the
     provenance bundle captures the refreshed artefacts before publishing.
4. **Report.**
   - Append a bullet to `status.md` summarising the cadence slot, owner, and
     diff outcome; include links to the PR and CI evidence.
   - Update the shared fixture log (governance minutes or roadmap row) so the
     cross-SDK cadence tracker stays accurate.

## Evidence & Rollback

- **Evidence bundle:** Keep the latest provenance folder
  (`artifacts/js-sdk-provenance/v<version>_<stamp>/`) with the workflow
  artifact so auditors can inspect the tarball, metadata, and checksums that
  shipped alongside the fixture change.【javascript/iroha_js/scripts/record-release-provenance.mjs:1】
- **Fallback plan:** If a fixture regression slips through, revert the offending
  fixture file, rerun `npm test`/integration tests, and document the rollback in
  `status.md` plus the governance cadence log. The next scheduled slot still
  runs, even if the rollback happened mid-week, to keep the rotation in sync.
- **Alerts:** Missing a slot (>48 h since the previous successful refresh)
  triggers the same Slack/PagerDuty escalation as the Swift cadence. JS Lead is
  responsible for coordinating the recovery action and updating the status/logs.
