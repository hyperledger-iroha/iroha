---
lang: az
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2025-12-29T18:16:35.966039+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO Qoşqu

Iroha 3 buraxılış xətti kritik Nexus yolları üçün açıq SLOları daşıyır:

- final slotunun müddəti (NX‑18 kadans)
- sübutun yoxlanılması (təhsil sertifikatları, JDG attestasiyaları, körpü sübutları)
- sübut son nöqtə ilə işləmə (doğrulama gecikməsi ilə Axum yolu proksisi)
- ödəniş və staking yolları (ödəyici/sponsor və istiqraz/slash axınları)

## Büdcələr

Büdcələr `benchmarks/i3/slo_budgets.json`-də yaşayır və birbaşa dəzgahla xəritələnir
I3 dəstindəki ssenarilər. Məqsədlər hər zəng üçün p99 hədəfləridir:

- Ödəniş/bağlama: hər zəng üçün 50 ms (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Təhlil sertifikatı / JDG / körpü yoxlayın: 80ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Sertifikat yığımını yerinə yetirin: 80ms (`commit_cert_assembly`)
- Giriş planlayıcısı: 50ms (`access_scheduler`)
- Provok son nöqtəsi: 120ms (`torii_proof_endpoint`)

Yanma dərəcəsi göstərişləri (`burn_rate_fast`/`burn_rate_slow`) 14.4/6.0-ı kodlayır
peyjinq və bilet xəbərdarlığı üçün çox pəncərə nisbətləri.

## Qoşqu

Qoşqu `cargo xtask i3-slo-harness` vasitəsilə işə salın:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Çıxışlar:

- `bench_report.json|csv|md` - xam I3 dəzgah dəstinin nəticələri (git hash + ssenarilər)
- `slo_report.json|md` - hər hədəfə görə keçdi/uğursuz/büdcə nisbəti ilə SLO qiymətləndirməsi

Qoşqu büdcə faylını istehlak edir və `benchmarks/i3/slo_thresholds.json`-i tətbiq edir
skamyada qaçış zamanı hədəf geriyə doğru getdikdə sürətli uğursuzluq.

## Telemetriya və idarə panelləri

- Sonluq: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Sübutun yoxlanılması: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana başlanğıc panelləri `dashboards/grafana/i3_slo.json`-də yaşayır. Prometheus
Yanma dərəcəsi ilə bağlı xəbərdarlıqlar `dashboards/alerts/i3_slo_burn.yml`-də verilir
yuxarıda göstərilən büdcələr (sonluq 2s, sübut doğrulama 80ms, sübut son nöqtə proxy
120ms).

## Əməliyyat qeydləri

- Gecələr qoşqu işlətmək; nəşr `artifacts/i3_slo/<stamp>/slo_report.md`
  idarəetmə sübutları üçün dəzgah artefaktları ilə yanaşı.
- Büdcə uğursuz olarsa, ssenarini müəyyən etmək üçün skamyadan istifadə edin, sonra qazın
  canlı ölçülərlə əlaqələndirmək üçün uyğun Grafana panelinə/xəbərdarlığına.
- Proof son nöqtə SLO-ları hər marşrutdan qaçmaq üçün doğrulama gecikməsindən proxy kimi istifadə edir
  kardinallıq zərbəsi; etalon hədəf (120ms) saxlama/DoS ilə uyğun gəlir
  sübut API-də qoruyucu barmaqlıqlar.