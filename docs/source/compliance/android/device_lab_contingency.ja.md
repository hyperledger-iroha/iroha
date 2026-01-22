---
lang: ja
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 461ea741be813740f0c99f4af35a2b23f5d98fe22292211e740f1448f522d617
source_last_modified: "2026-02-13T09:32:33.722418+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/compliance/android/device_lab_contingency.md -->

# デバイスラボ緊急対応ログ

Android デバイスラボのコンティンジェンシープランを発動した場合は必ず記録する。
コンプライアンスレビューと将来のレディネス監査に十分な詳細を含めること。

| 日付 | トリガー | 実施内容 | フォローアップ | 担当 |
|------|---------|---------------|------------|-------|
| 2026-02-11 | Pixel 8 Pro レーン障害と Pixel 8a 配送遅延によりキャパシティが 78% まで低下（`android_strongbox_device_matrix.md` 参照）。 | Pixel 7 レーンを主要 CI ターゲットに昇格し、共用 Pixel 6 フリートを借用。retail-wallet サンプル向けに Firebase Test Lab の smoke test を予約し、AND6 計画に基づき外部 StrongBox ラボを起動。 | Pixel 8 Pro 用の USB‑C ハブ交換（期限 2026-02-15）。Pixel 8a 到着を確認し、キャパシティレポートを再ベースライン。 | ハードウェアラボリード |
| 2026-02-13 | Pixel 8 Pro ハブ交換と Galaxy S24 承認により、キャパシティを 85% へ回復。 | Pixel 7 レーンをセカンダリへ戻し、Buildkite ジョブ `android-strongbox-attestation` をタグ `pixel8pro-strongbox-a` と `s24-strongbox-a` で再有効化。レディネスマトリクスと証拠ログを更新。 | Pixel 8a 配送 ETA を監視（未確定）。予備ハブの在庫記録を維持。 | ハードウェアラボリード |
