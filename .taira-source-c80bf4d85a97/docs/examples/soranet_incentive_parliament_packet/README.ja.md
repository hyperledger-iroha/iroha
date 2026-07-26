---
lang: ja
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

# SoraNet Relay Incentive Parliament Packet

このバンドルは、Sora Parliament が自動 relay 支払い (SNNet-7) を承認するために必要な artefacts をまとめたものです:

- `reward_config.json` - Norito でシリアライズ可能な報酬エンジン設定で、`iroha app sorafs incentives service init` により取り込める状態。`budget_approval_id` はガバナンス議事録に記載された hash と一致します。
- `shadow_daemon.json` - replay harness (`shadow-run`) と本番 daemon が使用する受益者および bond のマッピング。
- `economic_analysis.md` - 2025-10 -> 2025-11 shadow シミュレーションの公平性サマリ。
- `rollback_plan.md` - 自動支払いを無効化するための運用 playbook。
- 付随 artefacts: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Integrity Checks

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

パーラメント議事録に記録された値と digests を比較してください。shadow-run の署名は
`docs/source/soranet/reports/incentive_shadow_run.md` の説明に従って検証します。

## Updating the Packet

1. 報酬の重み、ベース支払い、または承認 hash が変わるたびに `reward_config.json` を更新します。
2. 60 日間の shadow シミュレーションを再実行し、`economic_analysis.md` を新しい結果で更新し、JSON + 分離署名ペアをコミットします。
3. 更新済みバンドルを Parliament に提出し、再承認を求める際には Observatory ダッシュボードのエクスポートを添付します。
