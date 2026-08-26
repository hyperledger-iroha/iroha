<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
title: `/v1/pipeline` Rollout Sequencing (SDKs)
summary: Locked sequencing and evidence requirements for aligning `/v1/pipeline` adoption across Swift, Android, JS, and Torii.
---

# `/v1/pipeline` Rollout Sequencing (SDKs)

This record captures the completed cross-SDK adoption of `/v1/pipeline`. The
first-release contract is now fixed: exact V1 input, one signed-byte dispatch,
HTTP `202` as the sole admission success, and authoritative status resolution.
Pre-release retry, downgrade, and endpoint-selection controls are retired.

## Final Sequence

| Stage | Scope | Owner(s) | Status | Evidence |
|-------|-------|----------|--------|----------|
| 0 - Torii staging validation | Run the staging validation checklist and record the rollout artefacts | Torii PM / SRE | Completed | `specs/torii/pipeline_staging_validation.md` |
| 1 - Swift default path | Submit exact V1 bytes once to `/v1/pipeline/transactions`, accept only `202`, and resolve finality from authoritative global state | Swift Lead / Torii delegate | Completed | `specs/sdk/swift/pipeline_adoption_guide.md`, `IrohaSwift/Sources/IrohaSwift/TxBuilder.swift`, `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift` |
| 2 - Kotlin default + Java mirror | Enforce the same one-shot admission and authoritative-finality contract in the default Android SDK and its temporary Java mirror | Android Networking TL / SDK Program Lead | In Progress | `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/HttpClientTransport.kt`, `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/HttpClientTransport.java` |
| 3 - JS confirmation | Enforce exact V1, one-shot `202` admission; retry only safe status reads and stream reconnections | JS Lead | Completed | `specs/sdk/js/torii_retry_policy.md`, `javascript/iroha_js/src/toriiClient.js` |

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

- **2026-02 Torii roadmap sync:** confirmed the stage ordering and shared fixtures.
- **2026-08 first-release hard cut:** removed the pre-release retry/downgrade
  narrative. Swift remains the reference for one-shot admission and bounded
  status polling; Kotlin is the default Android implementation and Java mirrors it.
- **Follow-ups:** finish Kotlin/Java exact-`202` parity and keep status-read retry
  policy aligned without making transaction POST replayable.
