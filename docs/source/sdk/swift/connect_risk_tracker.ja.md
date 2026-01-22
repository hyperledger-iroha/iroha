---
lang: ja
direction: ltr
source: docs/source/sdk/swift/connect_risk_tracker.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 942518163a04c38fdef0c4c65a62b358d1d6db5f0105d6013741b12524c60199
source_last_modified: "2026-02-03T09:04:02.697509+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/sdk/swift/connect_risk_tracker.md -->

# Swift Connect リスクトラッカー - 2026年4月25日

Swift SDK は依然として Connect のいくつかのサブシステムを足場段階として扱っている。
このトラッカーはロードマップの「週次ステータス差分 + Connect リスクの公開」要件
（`roadmap.md:1786`）がリスク解消まで可視化されるよう保持する。

| ID | リスク | 影響 | 証拠 | 緩和策 / 次のステップ | ステータス |
|----|------|--------|----------|-------------------------|--------|
| CR-1 | **Codec parity depends on JSON fallback** | `ConnectCodec` が Norito ブリッジ不在時に fail-closed となるためリスクは縮小。SDK は JSON エンコードのフレームを出力せず、テストはブリッジ対応の control/ciphertext フィクスチャを直接検証する。残存リスクは `NoritoBridge.xcframework` を欠落させるパッケージングミスに限られる。 | `IrohaSwift/Sources/IrohaSwift/ConnectCodec.swift:4`, `IrohaSwift/Tests/IrohaSwiftTests/ConnectFramesTests.swift:1`, `docs/source/sdk/swift/index.md:496` | xcframework のバンドルチェックリスト（`docs/source/sdk/swift/reproducibility_checklist.md`, README）を最前面に維持し、Carthage/SPM のリリースが常にブリッジを同梱するようにする。Connect ステータス出力は fail-closed テストを引用してパリティ証拠とする。 | 中 (2026-04-25 で緩和) |
| CR-2 | **Transport scaffolding lacks queue/flow control telemetry** | 解消 — `ConnectQueueJournal`, `ConnectQueueStateTracker`, replay recorder により queue/journal + metrics が出荷済み。inbound flow control は `ConnectFlowControlWindow` + `ConnectFlowController` による方向別ウィンドウの token grant/consume を強制。テストでスナップショット/メトリクスの永続化と flow-control の枯渇/付与を検証。`ConnectSession` は復号前に ciphertext フレームで token を消費する。 | `IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectQueueDiagnostics.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectReplayRecorder.swift:1`, `IrohaSwift/Sources/IrohaSwift/ConnectFlowControl.swift:1`, `docs/source/sdk/swift/index.md:590` | 週次ダイジェストで queue メトリクスのエクスポートを確認し、ウォレット側ウィンドウ公開時に flow-control の grant を拡張する。 | 低 |
| CR-3 | **Key custody & attestation not implemented** | 緩和 — `ConnectKeyStore` は Connect キーペアをアテステーションバンドル（SHA‑256 digest, デバイスラベル, 作成時刻）付きで永続化し、デフォルトでファイルバックのストレージを使用するため「raw bridge のみ」のギャップを解消。テストは永続化の round‑trip と digest 生成をカバーし、ドキュメントは承認前に keystore を参照するよう案内。 | `docs/source/sdk/swift/index.md:445`, `IrohaSwift/Sources/IrohaSwift/ConnectKeyStore.swift:1` | keystore をウォレットの承認フローに統合し、利用可能な場合はハードウェアバックのストレージへアテステーションを拡張。週次ダイジェストで Connect セッションの keystore 使用を確認する。 | 低 |

> **追跡頻度:** Swift Connect のスタンドアップで週次レビュー。コード、ドキュメント、
テレメトリダッシュボードが緩和の稼働を示すまで項目を削除しない。
