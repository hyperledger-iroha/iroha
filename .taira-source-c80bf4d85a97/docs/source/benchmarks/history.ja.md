---
lang: ja
direction: ltr
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2025-11-21T15:32:12.874170+00:00"
translation_last_reviewed: 2026-01-21
---

<!-- 日本語訳: docs/source/benchmarks/history.md -->

# GPU ベンチマークキャプチャ履歴 (FASTPQ WP5-B)

このファイルは `python3 scripts/fastpq/update_benchmark_history.py` により生成される。
FASTPQ Stage 7 の WP5-B 成果物として、ラップ済み GPU ベンチマークアーティファクト、
Poseidon microbench マニフェスト、`benchmarks/` 配下の補助スイープを追跡する。
新しいバンドルの追加やテレメトリ更新が必要な場合は、基礎キャプチャを更新して
スクリプトを再実行する。

## 範囲と更新プロセス

- 新しい GPU キャプチャを生成またはラップ（`scripts/fastpq/wrap_benchmark.py`）し、
  キャプチャマトリクスに追加してジェネレータを再実行し、テーブルを更新する。
- Poseidon microbench のデータがある場合は、
  `scripts/fastpq/export_poseidon_microbench.py` でエクスポートし、
  `scripts/fastpq/aggregate_poseidon_microbench.py` でマニフェストを再構築する。
- Merkle threshold スイープは JSON 出力を `benchmarks/merkle_threshold/` に保存する。
  ジェネレータは既知ファイルを一覧化し、監査で CPU と GPU の可用性を照合できる。

## FASTPQ Stage 7 GPU ベンチマーク

| Bundle | Backend | Mode | GPU backend | GPU available | Device class | GPU | LDE ms (CPU/GPU/SU) | Poseidon ms (CPU/GPU/SU) |
|-------|---------|------|-------------|---------------|--------------|-----|----------------------|---------------------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | cuda | gpu | cuda-sm80 | yes | xeon-rtx | NVIDIA RTX 6000 Ada | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | metal | gpu | none | yes | apple-m4 | Apple GPU 40-core | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | metal | gpu | metal | yes | apple-m2-ultra | Apple M2 Ultra | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | metal | gpu | metal | yes | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | metal | gpu | metal | yes | apple-m2-ultra | Apple M2 Ultra | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | opencl | gpu | opencl | yes | neoverse-mi300 | AMD Instinct MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> 列: `Backend` はバンドル名から導出。`Mode`/`GPU backend`/`GPU available` は
> ラップ済み `benchmarks` ブロックからコピーし、CPU フォールバックや GPU
> 検出失敗（例: `Mode=gpu` でも `gpu_backend=none`）を可視化する。SU = スピードアップ比 (CPU/GPU)。

## Poseidon Microbench スナップショット

`benchmarks/poseidon/manifest.json` は、各 Metal バンドルからエクスポートされた
デフォルト vs スカラーの Poseidon microbench を集約する。以下の表は
ジェネレータで更新されるため、CI とガバナンスレビューはラップ済み FASTPQ レポートを
展開せずに履歴の速度向上を比較できる。

| Summary | Bundle | Timestamp | Default ms | Scalar ms | Speedup |
|---------|--------|-----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## Merkle Threshold スイープ

`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
で収集した参照キャプチャは `benchmarks/merkle_threshold/` に保存される。リスト項目は
スイープ実行時にホストが Metal デバイスを公開したかを示す。GPU 有効キャプチャは
`metal_available=true` を報告する必要がある。

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

Apple Silicon のキャプチャ (`takemiyacStudio.lan_25.0.0_arm64`) は
`docs/source/benchmarks.md` で使用する正準 GPU ベースライン。macOS 14 のエントリは
Metal デバイスを公開できない環境向けの CPU 専用ベースラインとして残す。

## Row-Usage スナップショット

`scripts/fastpq/check_row_usage.py` で取得した witness デコードは、
transfer gadget の行効率を証明する。JSON アーティファクトを
`artifacts/fastpq_benchmarks/` に保持すれば、ジェネレータが監査向けに
記録済み transfer 比率を要約する。

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` — batches=2, transfer_ratio avg=0.629 (min=0.625, max=0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` — batches=2, transfer_ratio avg=0.619 (min=0.613, max=0.625)
