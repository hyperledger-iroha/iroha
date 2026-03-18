---
lang: ja
direction: ltr
source: docs/source/sdk/swift/readiness/screenshots/2026-03-05/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 34c22822e5a4c8fb7ed5d4e411a80f01ad20f51fdd2310a859bdd7304f0d1bc7
source_last_modified: "2026-03-06T08:23:12.591006+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/sdk/swift/readiness/screenshots/2026-03-05/README.md -->

# スクリーンショット索引 — Swift テレメトリ準備セッション (2026-03-05)

リポジトリにはメタデータのみを保存し、バイナリアーティファクトは
`s3://sora-readiness/swift/telemetry/20260305/` に置く。

| ファイル | セグメント | 備考 |
|------|---------|-------|
| `s3://sora-readiness/swift/telemetry/20260305/20260305-policy-hash-slide.png` | ポリシー深掘り | dry-run 後に追加されたハッシュ済み権限 + salt ローテーションの記述を含むスライド 12 の抜粋。 |
| `s3://sora-readiness/swift/telemetry/20260305/20260305-mobile-parity-dashboard.png` | ダッシュボード | chaos 実行前の正常ゲージを示す `mobile_parity.swift` エクスポーターブロックのキャプチャ。 |
| `s3://sora-readiness/swift/telemetry/20260305/20260305-connect-latency-spike.png` | カオスデモ | シナリオ E のレイテンシスパイク時にヒストグラム + アラートが発報している様子。 |
| `s3://sora-readiness/swift/telemetry/20260305/20260305-connect-alert-clear.png` | カオスデモ | エクスポーターのカウンタがベースラインへ戻るアラート復旧タイムライン。 |
| `s3://sora-readiness/swift/telemetry/20260305/20260305-quiz-summary.png` | 知識チェック | Google Form サマリのスクリーンショット（平均 96%、最小 92%）。 |
