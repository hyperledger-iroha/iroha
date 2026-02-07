---
lang: ja
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO ハーネス

Iroha 3 リリース ラインには、重要な Nexus パスの明示的な SLO が含まれています。

- ファイナリティ スロット期間 (NX‑18 ケイデンス)
- 証拠検証 (コミット証明書、JDG 証明書、ブリッジ証明)
- エンドポイント処理の証明 (検証レイテンシによる Axum パス プロキシ)
- 料金とステーキングパス (支払者/スポンサーおよびボンド/スラッシュフロー)

## 予算

予算は `benchmarks/i3/slo_budgets.json` に存在し、ベンチに直接マッピングされます。
I3 スイートのシナリオ。目標はコールごとの p99 ターゲットです。

- 料金/ステーキング: 通話あたり 50 ミリ秒 (`fee_payer`、`fee_sponsor`、`staking_bond`、`staking_slash`)
- コミット証明書/JDG/ブリッジ検証: 80ms (`commit_cert_verify`、`jdg_attestation_verify`、
  `bridge_proof_verify`)
- コミット証明書アセンブリ: 80ms (`commit_cert_assembly`)
- アクセススケジューラ: 50ms (`access_scheduler`)
- プルーフエンドポイントプロキシ: 120ms (`torii_proof_endpoint`)

バーンレートヒント (`burn_rate_fast`/`burn_rate_slow`) は 14.4/6.0 をエンコードします
ページングとチケット アラートのマルチウィンドウ比率。

## ハーネス

`cargo xtask i3-slo-harness` 経由でハーネスを実行します。

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

出力:

- `bench_report.json|csv|md` — 生の I3 ベンチ スイートの結果 (git ハッシュ + シナリオ)
- `slo_report.json|md` — ターゲットごとの合否/予算比による SLO 評価

ハーネスは予算ファイルを消費し、`benchmarks/i3/slo_thresholds.json` を適用します。
ベンチ実行中に、ターゲットが退行したときにフェイルファストを実行します。

## テレメトリとダッシュボード

- ファイナリティ: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- 証拠検証: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana スターター パネルは `dashboards/grafana/i3_slo.json` に存在します。 Prometheus
バーンレート アラートは、`dashboards/alerts/i3_slo_burn.yml` で提供されます。
ベイクインされた上記の予算 (ファイナリティ 2 秒、証明検証 80 ミリ秒、証明エンドポイント プロキシ)
120ミリ秒）。

## 操作上の注意事項

- ナイトリーでハーネスを実行します。パブリッシュ `artifacts/i3_slo/<stamp>/slo_report.md`
  ガバナンスの証拠としてベンチの成果物と並べてください。
- 予算が達成できない場合は、ベンチマークダウンを使用してシナリオを特定し、ドリルします。
  一致する Grafana パネル/アラートに取り込み、ライブ メトリックと関連付けます。
- 証明エンドポイント SLO は、検証レイテンシーをプロキシとして使用して、ルートごとの遅延を回避します。
  カーディナリティの爆発。ベンチマーク ターゲット (120 ミリ秒) は保持/DoS と一致します
  証明 API のガードレール。