<!-- Japanese translation of docs/source/benchmarks.md -->

---
lang: ja
direction: ltr
source: docs/source/benchmarks.md
status: complete
translator: manual
---

# ベンチマークレポート

## Enum とトレイトオブジェクトディスパッチの比較

- コンパイル時間（デバッグビルド）: 16.58 秒
- 実行時間（Criterion、数値が小さいほど高速）:
  - `enum`: 平均 386ps
 - `trait_object`: 平均 1.56ns

これらの測定値は、enum ベースのディスパッチとボックス化されたトレイトオブジェクト実装を比較するマイクロベンチマークに基づいています。

## Poseidon CUDA batching

The Poseidon benchmark (`crates/ivm/benches/bench_poseidon.rs`) now includes workloads that exercise both single-hash permutations and the new batched helpers. Run the suite with:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion will record results under `target/criterion/poseidon*_many`. When a GPU worker is available, export the JSON summaries (for example copy `target/criterion/**/new/benchmark.json` to `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`) so downstream teams can compare CPU vs CUDA throughput for each batch size. Until the dedicated GPU lane goes live, the benchmark falls back to the SIMD/CPU implementation and still provides useful regression data for batch performance.
