---
lang: am
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO ልጓም

የIroha 3 የመልቀቂያ መስመር ለወሳኝ የNexus መንገዶች ግልጽ SLOs ይይዛል፡

- የማጠናቀቂያ ማስገቢያ ቆይታ (NX-18 cadence)
- የማረጋገጫ ማረጋገጫ (የምስክር ወረቀት፣ የጄዲጂ ማረጋገጫዎች፣ የድልድይ ማረጋገጫዎች)
- የማጠናቀቂያ ነጥብ አያያዝ (የአክሱም መንገድ ተኪ በማረጋገጫ መዘግየት)
- ክፍያ እና የመያዣ መንገዶች (ከፋይ/ስፖንሰር እና ቦንድ/ስላሽ ፍሰቶች)

#በጀቶች

በጀቶች በ `benchmarks/i3/slo_budgets.json` ውስጥ ይኖራሉ እና በቀጥታ ወደ አግዳሚ ወንበር ካርታ
በ I3 ስብስብ ውስጥ ያሉ ሁኔታዎች. አላማዎች በየጥሪ p99 ኢላማዎች ናቸው፡

- ክፍያ/ማስያዝ፡ 50ms በአንድ ጥሪ (`fee_payer`፣ `fee_sponsor`፣ `staking_bond`፣ `staking_slash`)
የምስክር ወረቀት/ጄዲጂ/ድልድይ ማረጋገጫ፡ 80ms (`commit_cert_verify`፣ `jdg_attestation_verify`፣
  `bridge_proof_verify`)
- የምስክር ወረቀት መሰብሰብ፡ 80ms (`commit_cert_assembly`)
- የመዳረሻ መርሐግብር: 50ms (`access_scheduler`)
- የማጠናቀቂያ ነጥብ ተኪ፡ 120ms (`torii_proof_endpoint`)

የተቃጠለ-ተመን ፍንጮች (`burn_rate_fast`/`burn_rate_slow`) 14.4/6.0 ኮድ
ባለብዙ-መስኮት ሬሾዎች ለ paging vs. የቲኬት ማንቂያዎች።

##መታጠቅ

ማሰሪያውን በ`cargo xtask i3-slo-harness` በኩል ያሂዱ፡

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

ውጤቶች፡

- `bench_report.json|csv|md` — ጥሬ I3 የቤንች ስብስብ ውጤቶች (git hash + scenarios)
- `slo_report.json|md` — የ SLO ግምገማ ከ ማለፊያ/ውድቀት/የበጀት ሬሾ በአንድ ኢላማ

ማሰሪያው የበጀት ፋይሉን ይበላል እና `benchmarks/i3/slo_thresholds.json` ያስፈጽማል
በቤንች ሩጫ ወቅት ዒላማው ወደ ኋላ ሲመለስ በፍጥነት ለመክሸፍ።

## ቴሌሜትሪ እና ዳሽቦርዶች

የመጨረሻ: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- የማረጋገጫ ማረጋገጫ: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana ማስጀመሪያ ፓነሎች በ `dashboards/grafana/i3_slo.json` ውስጥ ይኖራሉ። Prometheus
የተቃጠለ ፍጥነት ማንቂያዎች በ `dashboards/alerts/i3_slo_burn.yml` ውስጥ ቀርበዋል
ከመጋገር በላይ ያለው በጀት (የመጨረሻ 2s፣ የማረጋገጫ ማረጋገጫ 80ms፣ የማረጋገጫ የመጨረሻ ነጥብ ፕሮክሲ
120 ሚሰ)

## ተግባራዊ ማስታወሻዎች

- በምሽት ምሽት ማሰሪያውን ያካሂዱ; `artifacts/i3_slo/<stamp>/slo_report.md` አትም
  ለአስተዳደራዊ ማስረጃ ከቤንች ቅርሶች ጋር።
- በጀት ካልተሳካ፣ ሁኔታውን ለመለየት የቤንች ማርክን ይጠቀሙ፣ ከዚያ ይቦርሹ
  ከቀጥታ መለኪያዎች ጋር ለማዛመድ ወደ ተዛማጅ Grafana ፓነል/ማስጠንቀቂያ።
- የማረጋገጫ የመጨረሻ ነጥብ SLOs በየመንገድ ለማስቀረት የማረጋገጫ መዘግየትን እንደ ፕሮክሲ ይጠቀማሉ
  ካርዲናሊቲ ማፈንዳት; የቤንችማርክ ኢላማ (120 ሚሴ) ከማቆየት/DoS ጋር ይዛመዳል
  በማረጋገጫ ኤፒአይ ላይ የጥበቃ መንገዶች።