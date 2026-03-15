---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Capacity Marketplace Validation
tags: [SF-2c, acceptance, checklist]
summary: Acceptance checklist covering provider onboarding, dispute workflows, and treasury reconciliation gating the SoraFS capacity marketplace general availability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Tutum bazarının yoxlanış siyahısı

**İnceləmə pəncərəsi:** 2026-03-18 → 2026-03-24  
**Proqram sahibləri:** Saxlama Komandası (`@storage-wg`), İdarəetmə Şurası (`@council`), Xəzinədarlıq Gildiyası (`@treasury`)  
**Əhatə dairəsi:** SF-2c GA üçün tələb olunan provayder boru kəmərləri, mübahisələrin həlli axınları və xəzinə razılaşması prosesləri.

Xarici operatorlar üçün bazarı işə salmazdan əvvəl aşağıdakı yoxlama siyahısı nəzərdən keçirilməlidir. Hər bir sıra auditorların təkrarlaya biləcəyi deterministik sübutlarla (testlər, qurğular və ya sənədlər) əlaqələndirilir.

## Qəbul Yoxlama Siyahısı

### Provayderin işə qəbulu

| Yoxlayın | Doğrulama | Sübut |
|-------|------------|----------|
| Reyestr kanonik qabiliyyət bəyannamələrini qəbul edir | İnteqrasiya testi tətbiq API vasitəsilə `/v1/sorafs/capacity/declare` tapşırıqlarını yerinə yetirir, imzaların idarə edilməsini, metadata ələ keçirilməsini və qovşaq reyestrinə ötürülməsini yoxlayır. | `crates/iroha_torii/src/routing.rs:7654` |
| Ağıllı müqavilə uyğun olmayan faydalı yükləri rədd edir | Vahid testi davam etməzdən əvvəl provayder identifikatorlarının və qəbul edilmiş GiB sahələrinin imzalanmış bəyannamə ilə uyğunluğunu təmin edir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI kanonik onboarding artefaktları yayır | CLI qoşqu deterministik Norito/JSON/Base64 çıxışlarını yazır və gedişləri təsdiqləyir ki, operatorlar bəyannamələri oflayn rejimdə həyata keçirə bilsinlər. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Operator təlimatı qəbul iş prosesini və idarəetmə qoruyucularını əks etdirir | Sənədlər bəyannamə sxemini, siyasət defoltlarını və şura üçün nəzərdən keçirmə addımlarını sadalayır. | `../storage-capacity-marketplace.md` |

### Mübahisələrin həlli

| Yoxlayın | Doğrulama | Sübut |
|-------|------------|----------|
| Mübahisə qeydləri kanonik faydalı yük həzmi ilə davam edir Vahid sınağı mübahisəni qeydə alır, saxlanan faydalı yükü deşifrə edir və kitab determinizminə zəmanət vermək üçün gözlənilən statusu təsdiqləyir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| CLI mübahisə generatoru kanonik sxemə uyğun gəlir | CLI testi Base64/Norito çıxışlarını və `CapacityDisputeV1` üçün JSON xülasələrini əhatə edir və sübut paketlərinin hashini deterministik şəkildə təmin edir. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Təkrar testi mübahisə/cəza determinizmini sübut edir | İki dəfə təkrarlanan sübut uğursuzluğu telemetriyası eyni kitab, kredit və mübahisə snapshotlarını yaradır, beləliklə, kəsiklər həmyaşıdları arasında deterministikdir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Runbook sənədlərinin eskalasiyası və ləğvi axını | Əməliyyat təlimatı şuranın iş axını, sübut tələbləri və geri qaytarma prosedurlarını əks etdirir. | `../dispute-revocation-runbook.md` |

### Xəzinədarlığın uzlaşması

| Yoxlayın | Doğrulama | Sübut |
|-------|------------|----------|
| Ledger hesablama uyğun gəlir 30 günlük islatmaq proyeksiya | Islatma testi, gözlənilən ödəmə arayışından fərqli olaraq, 30 hesablaşma pəncərəsində beş provayderi əhatə edir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Mühasibat kitabının ixrac uzlaşması hər gecə qeydə alınır | `capacity_reconcile.py` ödəniş kitabçası gözləntilərini yerinə yetirilmiş XOR transfer ixracı ilə müqayisə edir, Prometheus ölçülərini yayır və Alertmanager vasitəsilə xəzinədarlığın təsdiqini təmin edir. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Faturalandırma panelləri yerüstü cərimələr və hesablama telemetriyası | Grafana idxal planları GiB·saat hesablanması, tətil sayğacları və zəng zamanı görünmə üçün bağlanmış girov. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Nəşr edilmiş hesabat arxivləri metodologiya və təkrar əmrləri islatmaq | Hesabat təfərrüatları auditorlar üçün əhatə dairəsi, icra əmrləri və müşahidə qarmaqları haqqında məlumat verir. | `./sf2c-capacity-soak.md` |

## İcra qeydləri

Hesabdan çıxmazdan əvvəl doğrulama dəstini yenidən işə salın:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operatorlar onboarding/mübahisə sorğusu yüklərini `sorafs_manifest_stub capacity {declaration,dispute}` ilə bərpa etməli və nəticədə JSON/Norito baytlarını idarəetmə bileti ilə birlikdə arxivləşdirməlidir.

## Qeydiyyatdan Çıxış Artefaktları

| Artefakt | Yol | blake2b-256 |
|----------|------|-------------|
| Provayderin işə qəbulu təsdiq paketi | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Mübahisələrin həlli təsdiq paketi | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Xəzinədarlığın uzlaşma təsdiq paketi | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Bu artefaktların imzalanmış nüsxələrini buraxılış paketi ilə birlikdə saxlayın və onları idarəetmə dəyişikliyi qeydində əlaqələndirin.

## Təsdiqlər

- Saxlama Qrupunun Rəhbəri — @storage-tl (24-03-2026)  
- İdarəetmə Şurasının katibi — @council-sec (2026-03-24)  
- Xəzinədarlıq Əməliyyatları Rəhbəri — @treasury-ops (2026-03-24)