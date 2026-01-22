---
lang: ja
direction: ltr
source: docs/source/norito_stage1_cutover.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7f02eb4d7d5fa1a5ffef5c285cd1722799dcd0fd1fce852cd12eb6b9cf477f51
source_last_modified: "2026-01-03T18:07:57.754211+00:00"
translation_last_reviewed: 2026-01-22
---

<!-- 日本語訳: docs/source/norito_stage1_cutover.md -->

## Norito Stage-1 切り替え（2026年3月）

このメモは、Stage-1（JSON 構造インデックス）と CRC64 GPU の閾値調整の最新結果をまとめたもの。
ベンチマークは Apple Silicon 上で次のコマンドにより取得した:

```
cargo run -p norito --example stage1_cutover --release --features bench-internal \
  > benchmarks/norito_stage1/cutover.csv
```

ヘルパーは `bytes,scalar_ns,kernel_ns,ns_per_byte_*` の行を出力する。主な結果:
- Scalar と SIMD のクロスオーバーは **6–8 KiB** 付近になり、~4 KiB では SIMD が約 5% 遅いが、8 KiB 以降は約 2 倍高速。
- GPU Stage-1 の起動オーバーヘッドは、分割 CRC64/Stage-1 カーネルでこのホスト上 **~192 KiB** 付近で償却される。カットオフをそれ以上に保つことでスラッシングを避けつつ、大きなドキュメントが helper dylib を利用できる。
- CRC64 GPU ヘルパーも同じ 192 KiB ガードの恩恵を受け、テストや下流ツールがスタブアクセラレータを注入できるよう、ヘルパーのパスを env で上書き可能になった。

結果としてのデフォルト:
- `build_struct_index` の小入力カットオフを **4096 bytes** に引き上げ。
- Stage-1 GPU 最小値: **192 KiB**（`NORITO_STAGE1_GPU_MIN_BYTES` で上書き）。
- CRC64 GPU 最小値: **192 KiB**（`NORITO_GPU_CRC64_MIN_BYTES` で上書き）。
- CRC64 GPU ローダーは `NORITO_CRC64_GPU_LIB` を受け付け、カスタム helper を指す（スタブ使用のパリティテストで利用）。

生の測定値は `benchmarks/norito_stage1/cutover.csv` を参照。
