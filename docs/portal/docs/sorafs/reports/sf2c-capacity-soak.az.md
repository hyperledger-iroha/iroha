---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09567fc0280e726bdd1f2f1289dc98547ac70db9b19324ef5e413c2cff34de80
source_last_modified: "2025-12-29T18:16:35.201180+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SF-2c Tutumunun Yığılması Islatma Hesabatı

Tarix: 21-03-2026

## Əhatə dairəsi

Bu hesabat deterministik SoraFS tutumun hesablanmasını və ödəməni qeyd edir
SF-2c yol xəritəsi treki altında tələb olunan testlər.

- **30 günlük multi-provayder islatmaq:** tərəfindən həyata keçirilir
  `capacity_fee_ledger_30_day_soak_deterministic` in
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Qoşqu beş provayderi təcəssüm etdirir, 30 yaşayış pəncərəsini əhatə edir və
  mühasibat dəftərinin cəminin müstəqil hesablanmış arayışla uyğun olduğunu təsdiq edir
  proyeksiya. Test Blake3 həzmini (`capacity_soak_digest=...`) yayır.
  CI kanonik görüntünü çəkə və fərqləndirə bilər.
- **Çatdırılma cəzaları:** Tətbiq edir
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (eyni fayl). Test tətil hədlərini, soyuducuları, girov kəsiklərini,
  və kitab sayğacları deterministik olaraq qalır.

## İcra

Yerli olaraq islatma təsdiqləmələrini həyata keçirin:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Testlər standart noutbukda bir saniyədən az müddətdə tamamlanır və heç bir tələb yoxdur
xarici qurğular.

## Müşahidə qabiliyyəti

Torii indi ödəniş kitabçaları ilə yanaşı provayderin kredit şəkillərini də nümayiş etdirir, beləliklə tablosuna
aşağı balanslara və cərimə zərbələrinə qapıla bilər:

- REST: `GET /v2/sorafs/capacity/state` `credit_ledger[*]` qeydlərini qaytarır
  islatma testində təsdiqlənmiş kitab sahələrini əks etdirin. Bax
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Grafana idxal: `dashboards/grafana/sorafs_capacity_penalties.json`
  ixrac edilən tətil sayğacları, cərimə məbləğləri və bağlanmış girovlar
  işçilər canlı mühitlə sızma əsaslarını müqayisə edə bilər.

## Təqib

- Islatma testini (tüstü səviyyəli) təkrar etmək üçün CI-də həftəlik buraxılışları planlaşdırın.
- İstehsal telemetrindən sonra Grafana lövhəsini Torii sıyrılma hədəfləri ilə genişləndirin
  ixrac canlanır.