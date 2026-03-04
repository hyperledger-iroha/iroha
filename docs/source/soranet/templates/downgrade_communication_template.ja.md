---
lang: ja
direction: ltr
source: docs/source/soranet/templates/downgrade_communication_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0385a25caa5cdefcab7288602522ad165523dc14cd3eb56114f188c119b55679
source_last_modified: "2026-01-03T18:08:01.997331+00:00"
translation_last_reviewed: 2026-01-22
title: SoraNet ダウングレード連絡テンプレート
summary: 一時的な SoraNet ダウングレードについてオペレーターと SDK 利用者へ通知するための定型文。
---

<!-- 日本語訳: docs/source/soranet/templates/downgrade_communication_template.md -->

**件名:** SoraNet の地域 {{ region }} ダウングレード ({{ incident_id }})

**概要:**

- **期間:** {{ start_time }} UTC – {{ expected_end_time }} UTC
- **範囲:** {{ region }} の回線は {{ root_cause }} の復旧作業中、一時的に direct mode へフォールバックします。
- **影響:** SoraFS フェッチのレイテンシが増加し匿名性が低下しますが、GAR の強制は継続されます。

**オペレーター対応:**

1. 復旧を告知するまで、公開済みの override (`transport_policy=direct-only`) を適用してください。
2. brownout ダッシュボード (`sorafs_orchestrator_policy_events_total`, `soranet_privacy_circuit_events_total`) を監視してください。
3. GAR ログブックに緩和手順を記録してください。

**SDK / クライアント向けメッセージ:**

- ステータスページのバナー: "{{ region }} の SoraNet 回線は一時的にダウングレードされています。トラフィックはプライベートですが匿名ではありません。"
- API ヘッダー: `Soranet-Downgrade: region={{ region }}; incident={{ incident_id }}`

**次回更新:** {{ follow_up_time }} UTC もしくはそれ以前。

問い合わせはガバナンスブリッジ (`#soranet-incident`) に誘導してください。
