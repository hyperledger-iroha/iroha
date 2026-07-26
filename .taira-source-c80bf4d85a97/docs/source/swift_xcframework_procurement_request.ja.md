---
lang: ja
direction: ltr
source: docs/source/swift_xcframework_procurement_request.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 57e5497c8a43ac0dc46a21678c3bff6892a700f5f00702c63d93c2a21f5a548d
source_last_modified: "2026-01-27T09:17:52.324120+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/swift_xcframework_procurement_request.md -->

# 調達依頼 – StrongBox 予備端末

- **申請者:** Swift QA Lead (qa-swift@sora.org)
- **日付:** 2026-01-27
- **目的:** XCFramework smoke harness（`xcframework-smoke/strongbox` レーン）向けに
  StrongBox 対応 iPhone をホットスペアとして提供し、主要デバイスが保守中の際の
  ダウンタイムを回避する。
- **推奨端末:** iPhone 15 Pro（128 GB, SIM フリー）— StrongBox サポートを維持し、
  iOS 18 への将来性を確保。
- **見積コスト:** $1,199 USD（Apple Store 法人価格、税別）。
- **付属品:** USB‑C to USB‑C ケーブル（同梱） + 予備 USB‑C 電源アダプタ（20W）— $19 USD。
- **見積総額:** $1,218 USD + 現地税と配送費。

## 根拠
- `docs/source/swift_xcframework_device_matrix.md` に記載された必須 StrongBox レーンの
  カバレッジを確保する。
- バッテリー劣化、OS 再インストール、ハードウェア故障による CI ダウンタイムを緩和する。
- リリース週にハーネス拡張が必要な場合、並列 smoke 実行を可能にする。

## 次のステップ
1. 本文書を参照した調達チケット（`PROC-STRONGBOX-2026`）を提出する。
2. Cupertino ラボラック 3（slot A12 spare）への納品を調整し、到着後に
   `docs/source/swift_xcframework_hardware_plan.md` を更新する。
3. 新端末に DeviceKit ラベル `ios15p-strongbox-spare` を付与し、MDM に登録し、
   次回 Buildkite 実行で `ci/xcframework-smoke:strongbox:device_tag` メタデータが
   表示されることを確認してから予備端末を ready と宣言する。
