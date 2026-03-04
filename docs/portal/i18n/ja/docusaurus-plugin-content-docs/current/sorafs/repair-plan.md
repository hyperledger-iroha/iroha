---
id: repair-plan
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note 正規ソース
ミラー `docs/source/sorafs_repair_plan.md`。 Sphinx セットが廃止されるまで、両方のバージョンを同期させてください。
:::

## ガバナンスの決定ライフサイクル
1. エスカレートされた修理はスラッシュ提案草案を作成し、紛争ウィンドウを開きます。
2. ガバナンス有権者は、紛争期間中に承認/拒否の投票を提出します。
3. `escalated_at_unix + dispute_window_secs` では、決定は決定論的に計算されます。つまり、最小投票者数、承認が拒否を上回り、承認率が定足数のしきい値を満たしています。
4. 決定が承認された場合には、異議申し立ての窓口が開かれます。 `approved_at_unix + appeal_window_secs` より前に記録された控訴は、決定を控訴済みとしてマークします。
5. ペナルティの上限はすべての提案に適用されます。上限を超える投稿は拒否されます。

## ガバナンス エスカレーション ポリシー
エスカレーション ポリシーは、`iroha_config` の `governance.sorafs_repair_escalation` から取得され、すべての修復スラッシュ提案に適用されます。

|設定 |デフォルト |意味 |
|----------|----------|----------|
| `quorum_bps` | 6667 |集計された投票数のうちの最小支持率（ベーシスポイント）。 |
| `minimum_voters` | 3 |決定を解決するために必要な個別の投票者の最小数。 |
| `dispute_window_secs` | 86400 |エスカレーション後、投票が確定するまでの時間 (秒)。 |
| `appeal_window_secs` | 604800 |承認後、異議申し立てが受け付けられるまでの時間 (秒)。 |
| `max_penalty_nano` | 1,000,000,000 |修復エスカレーションに許可される最大のスラッシュ ペナルティ (ナノ XOR)。 |

- スケジューラが生成するプロポーザルの上限は `max_penalty_nano` です。上限を超える監査人の提出は拒否されます。
- 投票レコードは決定論的な順序付け (`voter_id` ソート) で `repair_state.to` に保存されるため、すべてのノードが同じ決定タイムスタンプと結果を導き出します。