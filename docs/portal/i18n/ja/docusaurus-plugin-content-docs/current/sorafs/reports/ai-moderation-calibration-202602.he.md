---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/reports/ai-moderation-calibration-202602.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fb5ace5acddc31fd9721cea5d539677ebdf307094e319ec444c227f12e1597f0
source_last_modified: "2025-11-14T04:43:22.174556+00:00"
translation_last_reviewed: 2026-01-30
---

# AI モデレーション校正レポート - 2026年2月

このレポートは **MINFO-1** 向けの初回校正アーティファクトをまとめたもの。
dataset、manifest、scoreboard は 2026-02-05 に作成され、2026-02-10 に省庁評議会で
レビューされ、ガバナンス DAG の高さ `912044` にアンカーされた。

## Dataset manifest

- **Dataset reference:** `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`
- **Slug:** `ai-moderation-calibration-202602`
- **Entries:** manifest 480, chunk 12,800, metadata 920, audio 160
- **Label mix:** safe 68%, suspect 19%, escalate 13%
- **Artefact digest:** `9c4f86a3c099a48d0e3d7cfbf14d22bb9492960c41cba3858f0722519ff612ab`
- **Distribution:** `sora://datasets/ministry/ai-moderation/calibration/2026-02.tar.zst`

完全な manifest は `docs/examples/ai_moderation_calibration_manifest_202602.json` にあり、
ガバナンス署名と release 時点で取得した runner hash を含む。

## Scoreboard サマリー

校正は opset 17 と決定論的 seed パイプラインで実行された。完全な scoreboard JSON
(`docs/examples/ai_moderation_calibration_scorecard_202602.json`) は hash と telemetry
digest を記録しており、以下の表で主要メトリクスを示す。

| モデル (ファミリー) | Brier | ECE | AUROC | Precision@Quarantine | Recall@Escalate |
| ------------------- | ----- | --- | ----- | -------------------- | --------------- |
| ViT-H/14 Safety (vision) | 0.141 | 0.031 | 0.987 | 0.964 | 0.912 |
| LLaVA-1.6 34B Safety (multimodal) | 0.118 | 0.028 | 0.978 | 0.942 | 0.904 |
| Perceptual ensemble (perceptual) | 0.162 | 0.047 | 0.953 | 0.883 | 0.861 |

統合メトリクス: `Brier = 0.126`, `ECE = 0.034`, `AUROC = 0.982`。校正期間の判定分布は
pass 91.2%、quarantine 6.8%、escalate 2.0%で、manifest サマリーに記載された
ポリシー期待値と一致する。false-positive backlog はゼロのままで、drift score (7.1%) も
20% のアラートしきい値を大きく下回った。

## しきい値とサインオフ

- `thresholds.quarantine = 0.42`
- `thresholds.escalate = 0.78`
- Governance motion: `MINFO-2026-02-07`
- Signed by `ministry-council-seat-03` at `2026-02-10T11:33:12Z`

CI は署名済みバンドルを `artifacts/ministry/ai_moderation/2026-02/` に保存し、
moderation runner のバイナリと一緒に保管した。上記の manifest digest と scoreboard
hashes は監査や異議申し立てで参照しなければならない。

## ダッシュボードとアラート

モデレーション SRE は Grafana ダッシュボード
`dashboards/grafana/ministry_moderation_overview.json` と Prometheus アラートルール
`dashboards/alerts/ministry_moderation_rules.yml` を導入すること
(テストカバレッジは `dashboards/alerts/tests/ministry_moderation_rules.test.yml`)。
これらのアーティファクトは ingest の停止、drift の急増、quarantine キューの増加に
対するアラートを発し、
[AI Moderation Runner Specification](../../ministry/ai-moderation-runner.md) に記載された
監視要件を満たす。
