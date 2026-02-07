---
lang: zh-hans
direction: ltr
source: docs/source/sdk/swift/connect_risk_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9de1817f80dce556b77c2f20f1947263546d7fadd78ef7f6ea371ea44ec16c05
source_last_modified: "2025-12-29T18:16:36.067623+00:00"
translation_last_reviewed: 2026-02-07
---

# Swift Connect Risk Tracker - Apr 25 2026

The Swift SDK still treats several Connect subsystems as scaffolding. This tracker keeps the
roadmap's "publish weekly status delta + Connect risk" requirement visible (`roadmap.md:1786`)
until the risks are retired.

| ID | Risk | Impact | Evidence | Mitigation / Next Steps | Status |
|----|------|--------|----------|-------------------------|--------|
| CR-1 | **Codec parity depends on JSON fallback** | Risk shrank now that `ConnectCodec` fails closed whenever the Norito bridge is absent—the SDK no longer emits JSON-encoded frames, and tests exercise the bridge-backed control/ciphertext fixtures directly. Residual exposure is limited to packaging mistakes that omit `NoritoBridge.xcframework`. | `IrohaSwift/Sources/IrohaSwift/ConnectCodec.swift:4`, `IrohaSwift/Tests/IrohaSwiftTests/ConnectFramesTests.swift:1`, `docs/source/sdk/swift/index.md:496` | Keep the xcframework bundling checklist front-and-centre (`docs/source/sdk/swift/reproducibility_checklist.md`, README) so Carthage/SPM releases always ship the bridge; Connect status exports now cite the fail-closed tests to prove parity. | Medium (mitigated Apr 25 2026) |
| CR-2 | **Transport scaffolding lacks queue/flow control telemetry** | Closed — queue/journal + metrics ship via `ConnectQueueJournal`, `ConnectQueueStateTracker`, and the replay recorder, and inbound flow control now enforces per-direction windows with token grant/consume (`ConnectFlowControlWindow` + `ConnectFlowController`). Tests cover snapshot/metric persistence and flow-control exhaustion/grant; `ConnectSession` consumes tokens on ciphertext frames before decrypting. | `IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectQueueDiagnostics.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectReplayRecorder.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectFlowControl.swift:1`, `docs/source/sdk/swift/index.md:590` | Keep weekly digests confirming queue metrics export; extend flow-control grants when wallet-side windows are exposed. | Low |
| CR-3 | **Key custody & attestation not implemented** | Mitigated — `ConnectKeyStore` now persists Connect keypairs with an attestation bundle (SHA-256 digest, device label, created-at) and file-backed storage by default, closing the “raw bridge only” gap. Tests cover round-trip persistence and digest generation; docs point wallets at the keystore before approvals. | `docs/source/sdk/swift/index.md:445`, `IrohaSwift/Sources/IrohaSwift/ConnectKeyStore.swift:1` | Hook the keystore into wallet approval flows and extend attestation to hardware-backed storage when available; ensure weekly digests confirm keystore usage in Connect sessions. | Low |

> **Tracking cadence:** review weekly during the Swift Connect stand-up; drop items only after the
code, docs, and telemetry dashboards prove the mitigation is live.
