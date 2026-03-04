<!-- Hebrew translation of docs/source/benchmarks.md -->

---
lang: he
direction: rtl
source: docs/source/benchmarks.md
status: complete
translator: manual
---

<div dir="rtl">

# דו״ח מדדי ביצועים

## הפניית Enum לעומת הפניית אובייקט־תכונה

- זמן קומפילציה (בנייה Debug): ‎16.58 שניות
- זמן ריצה (Criterion – ערך נמוך עדיף):
  - `enum`: ממוצע ‎386ps
  - `trait_object`: ממוצע ‎1.56ns

המדידות נאספו ממיקרו-בנץ׳ המשווה בין מנגנון הפניה המבוסס על enum לבין מימוש בעזרת אובייקט־תכונה (trait object) ממופה לקופסה.

## Poseidon CUDA batching

The Poseidon benchmark (`crates/ivm/benches/bench_poseidon.rs`) now includes workloads that exercise both single-hash permutations and the new batched helpers. Run the suite with:

```bash
cargo bench -p ivm bench_poseidon -- --save-baseline poseidon_cuda
```

Criterion will record results under `target/criterion/poseidon*_many`. When a GPU worker is available, export the JSON summaries (לדוגמה להעתיק את `target/criterion/**/new/benchmark.json` אל `benchmarks/poseidon/criterion_poseidon2_many_cuda.json`) so downstream teams can compare CPU vs CUDA throughput for each batch size. Until the dedicated GPU lane goes live, the benchmark falls back to the SIMD/CPU implementation and still provides useful regression data for batch performance.

</div>
