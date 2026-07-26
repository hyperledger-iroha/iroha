---
lang: az
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 dəzgah dəsti

Iroha 3 dəzgah dəsti, ödəniş zamanı etibar etdiyimiz isti yolları dəfələrlə qiymətləndirir.
şarj, sübut yoxlaması, planlaşdırma və sübut son nöqtələri. kimi çalışır
Deterministik qurğularla `xtask` əmri (sabit toxumlar, sabit əsas material,
və sabit sorğu yükləri) buna görə də nəticələr hostlar arasında təkrarlana bilər.

## Paketi işə salmaq

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

Bayraqlar:

- `--iterations` ssenari nümunəsi üzrə təkrarlamalara nəzarət edir (defolt: 64).
- `--sample-count` medianı hesablamaq üçün hər bir ssenarini təkrarlayır (defolt: 5).
- `--json-out|--csv-out|--markdown-out` çıxış artefaktlarını seçin (hamısı isteğe bağlıdır).
- `--threshold` medianı baza sərhədləri ilə müqayisə edir (`--no-threshold` təyin edin
  keçmək).
- `--flamegraph-hint` Markdown hesabatını `cargo flamegraph` ilə şərh edir
  bir ssenarini profil etmək üçün əmr.

CI yapışqan `ci/i3_bench_suite.sh`-də yaşayır və yuxarıdakı yollara defolt olaraq təyin olunur; təyin edin
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` gecə vaxtı iş vaxtını tənzimləmək üçün.

## Ssenarilər

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — ödəyiciyə qarşı sponsor debeti
  və çatışmazlıqdan imtina.
- `staking_bond` / `staking_slash` — istiqraz və ya bağlama növbəsi
  kəsmək.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  öhdəçilik sertifikatları, JDG sertifikatları və körpü üzərində imza yoxlaması
  sübut yükləri.
- `commit_cert_assembly` — öhdəçilik sertifikatları üçün yığım.
- `access_scheduler` — münaqişədən xəbərdar olan giriş-dəsti planlaşdırma.
- `torii_proof_endpoint` — Axum sübut son nöqtənin təhlili + yoxlama gedişi.

Hər bir ssenari iterasiya üçün median nanosaniyələri, ötürmə qabiliyyətini və a
sürətli reqressiyalar üçün deterministik ayırma sayğacı. Eşiklər yaşayır
`benchmarks/i3/thresholds.json`; hardware dəyişdikdə orada sərhədləri qabar və
hesabatla yanaşı yeni artefaktı yerinə yetirin.

## Problemlərin aradan qaldırılması

- Səs-küylü reqressiyaların qarşısını almaq üçün sübut toplayarkən CPU tezliyini/qubernatorunu pin edin.
- Kəşfiyyat işləri üçün `--no-threshold`-dən istifadə edin, sonra baza səviyyəsinə çatdıqdan sonra yenidən aktivləşdirin
  təzələndi.
- Tək bir ssenarini profil etmək üçün `--iterations 1` təyin edin və altında yenidən işə salın
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.