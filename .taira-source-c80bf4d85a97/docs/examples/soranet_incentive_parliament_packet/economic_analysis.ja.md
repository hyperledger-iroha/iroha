---
lang: ja
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

# 経済分析 - 2025-10 -> 2025-11 Shadow Run

ソース artefact: `docs/examples/soranet_incentive_shadow_run.json` (同ディレクトリに署名 + 公開鍵)。シミュレーションは relay ごとに 60 epoch を再生し、報酬エンジンは `reward_config.json` に記録された `RewardConfig` に固定されています。

## 分配サマリ

- **総支払い:** 360 の報酬 epoch に対して 5,160 XOR。
- **公平性エンベロープ:** ジニ係数 0.121; 上位 relay シェア 23.26%
  (30% の governance guardrail を大きく下回る)。
- **可用性:** フリート平均 96.97%、全 relays が 94% 以上。
- **帯域:** フリート平均 91.20%、最低パフォーマーは 87.23%
  (計画メンテナンス中); 罰則は自動適用。
- **コンプライアンスノイズ:** warning epoch 9 件と suspend 3 件が観測され、
  payout 減額に変換; 12-warning 上限を超えた relay はなし。
- **運用衛生:** 欠落 config/bonds/重複によるメトリクス snapshots の欠落なし;
  計算エラーの出力なし。

## 観察

- suspensions は relays がメンテナンスモードに入った epochs に対応。payout エンジンは
  その epochs に 0 payout を出しつつ、shadow-run JSON に監査証跡を残した。
- warning 罰則で影響 payout が 2% 減少; uptime/bandwidth の重み (650/350 per mille) により
  分配は収束を維持。
- 帯域の分散は匿名化された guard heatmap を追跡。最低パフォーマー
  (`6666...6666`) は期間中 620 XOR を維持し、0.6x フロアを上回った。
- レイテンシ感度アラート (`SoranetRelayLatencySpike`) は期間中 warning 閾値を下回り、
  関連ダッシュボードは `dashboards/grafana/soranet_incentives.json` に保存。

## GA 前の推奨アクション

1. 毎月 shadow replays を継続し、フリート構成が変わる場合は artefact セットと本分析を更新。
2. roadmap に記載された Grafana アラートスイートで自動支払いをゲート
   (`dashboards/alerts/soranet_incentives_rules.yml`); 更新時にはスクリーンショットを
   governance minutes に添付。
3. 基本報酬、uptime/bandwidth 重み、または compliance 罰則が >=10% 変化した場合は
   経済ストレステストを再実行。
