---
lang: ka
direction: ltr
source: docs/source/torii/pipeline_rollout_sequencing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 203456e8ab2605eb34e5ccaba5a0eae1987438c425daeb96f457b630c4b3299c
source_last_modified: "2025-12-29T18:16:36.235087+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: `/v1/pipeline` Rollout Sequencing (SDKs)
summary: Locked sequencing and evidence requirements for aligning `/v1/pipeline` adoption across Swift, Android, JS, and Torii.
---

# `/v1/pipeline` Rollout Sequencing (SDKs)

This plan locks the cross-SDK order of operations for `/v1/pipeline` adoption so
roadmap coordination actions ("Align `/v1/pipeline` rollout sequencing") have a
single, auditable source. The sequencing was ratified during the Feb 2026 Torii
roadmap sync and is now considered **locked**.

## Final Sequence

| Stage | Scope | Owner(s) | Status | Evidence |
|-------|-------|----------|--------|----------|
| 0 - Torii staging validation | Run the staging validation checklist and record the rollout artefacts | Torii PM / SRE | Completed | `docs/source/torii/pipeline_staging_validation.md` |
| 1 - Swift default path | Move Swift submissions/polling to `/v1/pipeline/transactions`, add retries + downgrade toggles, and document staging steps | Swift Lead / Torii delegate | Completed | `docs/source/sdk/swift/pipeline_adoption_guide.md`, `IrohaSwift/Sources/IrohaSwift/TxBuilder.swift`, `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift` |
| 2 - Android + mock harness reuse | Align Android HTTP client + mock harness with the same retry/idempotency policy; share fixtures with Swift/JS | Android Networking TL / SDK Program Lead | In Progress (policy locked; fixtures shared) | `docs/source/torii/mock_harness_workshop.md`, `java/iroha_android/src/test/java/org/hyperledger/iroha/android/client/websocket/ToriiWebSocketSubscriptionTests.java` |
| 3 - JS confirmation | Keep JS wrappers aligned with the shared retry/backoff profile and NRPC fixtures | JS Lead | Completed | `docs/source/sdk/js/torii_retry_policy.md`, `javascript/iroha_js/src/toriiClient.js` |

Stages 2-3 reuse the Torii mock harness fixtures validated in Stage 1 so all SDK
clients exercise the same `/v1/pipeline` behaviours.

## Gates & Reporting

- **CI gates:** `ci/xcode-swift-parity` and the Android/JS mock harness jobs must
  stay green before a rollout proceeds past the next stage.
- **Telemetry:** exporters feed the parity dashboard (`mobile_parity` schema) and
  the shared pipeline metadata feed so regressions surface in dashboards and
  status digests.
- **Docs:** each SDK must publish a staging/rollout guide; Swift and Torii
  references are listed above, and Android/JS guides inherit the same sequence.

## Decision Log

- **2026-02 Torii roadmap sync:** confirmed the stage ordering above, agreed
  that Swift remains the reference client for `/v1/pipeline` retries/polling,
  and recorded the requirement that all SDKs cite the same fixtures when
  publishing evidence.
- **Follow-ups:** Android keeps the sequencing but finishes queue persistence +
  telemetry wiring before marking Stage 2 "complete"; JS mirrors any retry
  tweaks via the `torii_retry_policy.md` contract.
