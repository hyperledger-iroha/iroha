---
lang: ja
direction: ltr
source: docs/source/status/soranet_testnet_weekly_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e87ff17c92df5f22db362da249242d7570e9c6e7e53c6d39452061d846cd1757
source_last_modified: "2026-01-03T18:07:58.606006+00:00"
translation_last_reviewed: 2026-01-22
title: SoraNet Testnet 週間ダイジェスト テンプレート
summary: SNNet-10 の週次ステータス更新向けチェックリスト。
---

<!-- 日本語訳: docs/source/status/soranet_testnet_weekly_digest.md -->

# SoraNet Testnet 週間ダイジェスト — {{ week_start }} の週

## ネットワークスナップショット

- リレー数: {{ relay_count }} (目標 {{ target_relay_count }})
- PQ 対応リレー: {{ pq_relays }} ({{ pq_ratio }}%)
- ガードローテーション平均年齢: {{ guard_rotation_hours }} 時間
- 今週の brownout 件数: {{ brownout_count }}

## 主要メトリクス

| 指標 | 値 | 目標 | メモ |
|--------|-------|--------|-------|
| `soranet_privacy_circuit_events_total{kind="downgrade"}` (per min) | {{ downgrade_rate }} | 0 | {{ downgrade_notes }} |
| `sorafs_orchestrator_policy_events_total{outcome="brownout"}` (per 30 min) | {{ brownout_rate }} | <0.05 | {{ policy_notes }} |
| PoW 中央解答時間 | {{ pow_median_ms }} ms | ≤300 ms | {{ pow_notes }} |
| 回線 RTT p95 | {{ rtt_p95_ms }} ms | ≤200 ms | {{ rtt_notes }} |

## コンプライアンス & GAR

- オプトアウトカタログのバージョン: {{ opt_out_tag }}
- 今週受領したオペレーターのコンプライアンス確認: {{ compliance_forms }}
- 未解決アクション: {{ compliance_actions }}

## インシデント & ドリル

- [ ] brownout ドリル実施 (日時・結果)
- [ ] ダウングレード連絡テンプレートの演習 (記録へのリンク)
- [ ] ロールバック予行演習の実施 (概要)

## 今後のマイルストーン

- {{ milestone_one }}
- {{ milestone_two }}

## メモ & 決定事項

- {{ decision_one }}
- {{ decision_two }}
