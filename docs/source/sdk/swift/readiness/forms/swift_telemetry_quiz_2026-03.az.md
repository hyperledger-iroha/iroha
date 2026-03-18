---
lang: az
direction: ltr
source: docs/source/sdk/swift/readiness/forms/swift_telemetry_quiz_2026-03.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a776c0d7a49ef2bd1a0c595424d225bda96ab2e26fd36e3a9b0094eac18a24c
source_last_modified: "2025-12-29T18:16:36.079358+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Swift Telemetry Readiness Quiz — March 2026

Use this questionnaire to capture knowledge-check results after the IOS7/IOS8
readiness session. Import the questions into Google Forms (single response,
required questions, >90 % to pass) and export CSV results into
`docs/source/sdk/swift/readiness/forms/responses/2026-03.csv`.

## Questions

1. **Which configuration value supplies the telemetry hashing salt and how often
   does it rotate?**
   - a) `IROHA_TORII_ENDPOINT`, monthly
   - b) `iroha_config.telemetry.redaction_salt`, quarterly ✅
   - c) `SWIFT_TELEMETRY_SALT`, yearly
   - d) `IOS7_SALT_OVERRIDE`, weekly
2. **What is the approved way to create a temporary telemetry override?**
   - a) Manually editing the override ledger in Git
   - b) Posting raw JSON to the dashboard webhook
   - c) Running `scripts/swift_status_export.py telemetry-override create …` ✅
   - d) Triggering a buildkite job
3. **Which signals must be captured when running the exporter outage drill?**
   - a) Only `swift.sdk.error`
   - b) `swift.telemetry.export.status`, `swift.torii.http.request`, and the
        buffered queue logs ✅
   - c) None — exporter outages are silent
   - d) `swift.pipeline.submit` only
4. **Where are lab screenshots and log bundles stored?**
   - a) `/tmp`
   - b) `docs/source/sdk/swift/readiness/screenshots/<date>/` and the matching
        `archive/<date>/` directory ✅
   - c) Personal cloud storage
   - d) Buildkite artefact bucket
5. **Who owns the telemetry redaction policy for Swift?**
   - a) Release Engineering
   - b) Swift Observability TL ✅
   - c) Android Foundations TL
   - d) External auditors

## Scoring & Follow-Up

- Passing threshold: **≥90 %** (≥5/5 or 4/5 with documented remediation).
- Store raw responses in `forms/responses/2026-03.csv`.
- For scores below threshold, schedule a 15-minute follow-up and log notes in
  the archive README.
