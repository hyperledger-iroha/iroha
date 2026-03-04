---
lang: ja
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 ベンチ スイート

Iroha 3 ベンチ スイートは、ステーキング中に依存するホット パスを倍増します。
課金、プルーフ検証、スケジューリング、プルーフエンドポイント。として実行されます
決定論的フィクスチャを使用した `xtask` コマンド (固定シード、固定キー マテリアル、
安定したリクエスト ペイロード）により、ホスト間で結果を再現できます。

## スイートの実行

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

フラグ:

- `--iterations` は、シナリオ サンプルごとの反復数を制御します (デフォルト: 64)。
- `--sample-count` は、各シナリオを繰り返して中央値を計算します (デフォルト: 5)。
- `--json-out|--csv-out|--markdown-out` 出力アーティファクトを選択します (すべてオプション)。
- `--threshold` は中央値をベースライン境界と比較します (`--no-threshold` を設定)
  スキップします）。
- `--flamegraph-hint` は、Markdown レポートに `cargo flamegraph` の注釈を付けます
  シナリオをプロファイリングするコマンド。

CI グルーは `ci/i3_bench_suite.sh` に存在し、デフォルトでは上記のパスになります。セット
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` は、nightlies でのランタイムを調整します。

## シナリオ

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — 支払者とスポンサーの借方
  そして不足分の拒否。
- `staking_bond` / `staking_slash` — キューの結合/結合解除ありまたはなし
  斬りつける。
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  コミット証明書、JDG 証明書、ブリッジを介した署名検証
  証拠のペイロード。
- `commit_cert_assembly` — コミット証明書のダイジェスト アセンブリ。
- `access_scheduler` — 競合を認識するアクセスセットのスケジューリング。
- `torii_proof_endpoint` — Axum の証明エンドポイント解析 + 検証往復。

すべてのシナリオで、反復ごとのナノ秒の中央値、スループット、および
迅速な回帰のための決定論的割り当てカウンター。しきい値が存在する
`benchmarks/i3/thresholds.json`;ハードウェアが変更されると、そこにバンプ境界が発生します。
レポートと一緒に新しいアーティファクトをコミットします。

## トラブルシューティング

- ノイズの多い回帰を避けるために、証拠を収集するときに CPU 周波数/ガバナを固定します。
- 探索的な実行には `--no-threshold` を使用し、ベースラインが設定されたら再度有効にします。
  リフレッシュされました。
- 単一のシナリオをプロファイリングするには、`--iterations 1` を設定し、以下で再実行します。
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`。