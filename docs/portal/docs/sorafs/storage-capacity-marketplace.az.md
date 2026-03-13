---
lang: az
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 868edd6aa7401c64b8757db188edb13aa8e6ca8959966b6fea02e44bc298c6b7
source_last_modified: "2026-01-05T09:28:11.910794+00:00"
translation_last_reviewed: 2026-02-07
id: storage-capacity-marketplace
title: SoraFS Storage Capacity Marketplace
sidebar_label: Capacity Marketplace
description: SF-2c plan for the capacity marketplace, replication orders, telemetry, and governance hooks.
translator: machine-google-reviewed
---

:::Qeyd Kanonik Mənbə
:::

# SoraFS Yaddaş Tutumu Marketplace (SF-2c Qaralama)

SF-2c yol xəritəsi elementi saxlama yeri olan idarə olunan bazarı təqdim edir
provayderlər öhdəlik götürdüklərini bəyan edirlər, təkrarlama sifarişləri alırlar və haqq qazanırlar
çatdırılma mövcudluğu ilə mütənasibdir. Bu sənəd çatdırılanları əhatə edir
ilk buraxılış üçün tələb olunur və onları hərəkətə keçirə bilən treklərə ayırır.

## Məqsədlər

- Ekspres provayderin tutum öhdəlikləri (ümumi baytlar, hər zolaqlı limitlər, istifadə müddəti)
  idarəetmə, SoraNet nəqliyyatı və Torii tərəfindən istehlak edilə bilən yoxlanıla bilən formada.
- Elan edilmiş tutum, pay və paya görə provayderlər arasında sancaqlar ayırın
  deterministik davranışı qoruyarkən siyasət məhdudiyyətləri.
- Sayğacların saxlanması (replikasiya müvəffəqiyyəti, iş vaxtı, bütövlük sübutları) və
  ödəniş paylanması üçün telemetriya ixracı.
- Ləğv etmə və mübahisə proseslərini təmin edin ki, provayderlər vicdansız ola bilsinlər
  cəzalandırılır və ya çıxarılır.

## Domen Konseptləri

| Konsepsiya | Təsvir | İlkin Çatdırılma |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito provayder ID-sini, chunker profil dəstəyini, qəbul edilmiş GiB, zolağa xüsusi məhdudiyyətlər, qiymət göstərişləri, stake öhdəliyi və müddəti təsvir edən faydalı yük. | `sorafs_manifest::capacity`-də sxem + doğrulayıcı. |
| `ReplicationOrder` | Artıqlıq səviyyəsi və SLA göstəriciləri daxil olmaqla, bir və ya daha çox provayderə açıq CID təyin edən idarəetmə tərəfindən verilmiş təlimat. | Norito sxemi Torii + ağıllı müqavilə API ilə paylaşıldı. |
| `CapacityLedger` | Zəncirdə/zəncirdənkənar reyestrdə aktiv tutum bəyannaməsi, təkrarlama sifarişləri, performans göstəriciləri və haqq hesablanması izlənir. | Ağıllı müqavilə modulu və ya deterministik snapshot ilə zəncirdənkənar xidmət stub. |
| `MarketplacePolicy` | Minimum payı, audit tələblərini və cərimə əyrilərini müəyyən edən idarəetmə siyasəti. | `sorafs_manifest` + idarəetmə sənədində konfiqurasiya strukturu. |

### Həyata keçirilmiş sxemlər (Status)

## İş Dağılımı

### 1. Sxem və Registry Layer

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1` təyin edin. | Saxlama Komandası / İdarəetmə | Norito istifadə edin; semantik versiya və qabiliyyət istinadları daxildir. |
| `sorafs_manifest`-də parser + validator modullarını tətbiq edin. | Saxlama Komandası | Monoton şəxsiyyətləri, tutum hədlərini, pay tələblərini tətbiq edin. |
| Profil üçün `min_capacity_gib` ilə chunker reyestrinin metadatasını genişləndirin. | Tooling WG | Müştərilərə hər profil üçün minimum avadanlıq tələblərini yerinə yetirməyə kömək edir. |
| Qəbul barmaqlıqlarını və cərimə cədvəlini əks etdirən layihə `MarketplacePolicy` sənədi. | İdarəetmə Şurası | Siyasət defoltları ilə yanaşı sənədlərdə dərc edin. |

#### Sxem Tərifləri (İcra olunur)

- `CapacityDeclarationV1` hər provayder üçün imzalanmış tutum öhdəliklərini, o cümlədən kanonik chunker tutacaqları, qabiliyyət arayışları, isteğe bağlı zolaq qapaqları, qiymət göstərişləri, etibarlılıq pəncərələri və metadata alır. Təsdiqləmə sıfır olmayan payı, kanonik tutacaqları, təkrarlanan ləqəbləri, elan edilmiş cəmi daxilində hər zolaqlı qapaqları və monoton GiB uçotunu təmin edir.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` manifestləri artıqlıq hədəfləri, SLA hədləri və hər bir tapşırıq zəmanəti ilə idarəetmə tərəfindən verilmiş tapşırıqlara bağlayır; validatorlar Torii və ya reyestr sifarişi qəbul etməzdən əvvəl kanonik chunker tutacaqlarını, unikal provayderləri və son tarix məhdudiyyətlərini tətbiq edir.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` ödəniş paylanmasını təmin edən dövr anlıq görüntülərini (istifadə olunan GiB, təkrarlama sayğacları, iş vaxtı/PoR faizləri ilə müqayisədə elan edilmiş) ifadə edir. Sərhəd yoxlamaları bəyannamələr daxilində istifadəni və 0 – 100% daxilində faizləri saxlayır.【crates/sorafs_manifest/src/capacity.rs:476】
- Paylaşılan köməkçilər (`PricingScheduleV1`, `PricingScheduleV1`, zolaq/təyinat/SLA təsdiqləyiciləri) CI və aşağı axın alətlərinin təkrar istifadə edə biləcəyi ilə bağlı deterministik açar doğrulama və xəta hesabatını təmin edir.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` indi deterministik Norito arxasında provayder bəyannamələrini və ödəniş kitabçası qeydlərini birləşdirərək `/v2/sorafs/capacity/state` vasitəsilə zəncir üzərindəki görüntünü təqdim edir. JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Təsdiqləmə əhatə dairəsi kanonik idarəetmə tətbiqi, dublikatın aşkarlanması, hər zolaqlı sərhədlər, replikasiya təyini qoruyucuları və telemetriya diapazonunun yoxlanılması ilə məşğul olur ki, reqressiyalar dərhal CI-də üzə çıxsın.【crates/sorafs_manifest/src/capacity.rs:792】
- Operator alətləri: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` insan tərəfindən oxuna bilən spesifikasiyaları kanonik Norito faydalı yüklərə, base64 bloblara və JSON xülasələrinə çevirir ki, operatorlar `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` və replikasiya sifarişi ilə yerli düzəlişləri hazırlaya bilsinlər. validation.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 İstinad qurğuları `fixtures/sorafs_manifest/replication_order/`-də (`order_v1.json`, `order_v1.to`) yaşayır və 00.0.05 vasitəsilə yaradılır.

### 2. İdarəetmə Təyyarəsinin İnteqrasiyası

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` Norito JSON faydalı yükləri olan işləyiciləri əlavə edin. | Torii Komandası | Mirror validator məntiqi; Norito JSON köməkçilərini təkrar istifadə edin. |
| `CapacityDeclarationV1` snapşotlarını orkestrator tablosunun metadatasına və şlüz gətirmə planlarına təbliğ edin. | Tooling WG / Orkestr qrupu | `provider_metadata`-i tutum istinadları ilə genişləndirin ki, çox mənbəli hesablama zolaq məhdudiyyətlərinə riayət etsin. |
| Tapşırıqları və əvəzetmə göstərişlərini idarə etmək üçün replikasiya sifarişlərini orkestrator/şluz müştərilərinə çatdırın. | Şəbəkə TL / Gateway komandası | Scoreboard builder idarəetmə tərəfindən imzalanmış replikasiya sifarişlərini istehlak edir. |
| CLI alətləri: `sorafs_cli`-i `capacity declare`, `capacity telemetry`, `capacity orders import` ilə genişləndirin. | Tooling WG | Deterministik JSON + skorbord nəticələrini təmin edin. |

### 3. Bazar Siyasəti və İdarəetmə

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| `MarketplacePolicy` (minimum pay, cərimə çarpanları, audit kadansı) təsdiq edin. | İdarəetmə Şurası | Sənədlərdə dərc edin, təftiş tarixçəsini çəkin. |
| Parlamentin bəyannamələri təsdiq edə, yeniləyə və ləğv edə bilməsi üçün idarəetmə qarmaqları əlavə edin. | İdarəetmə Şurası / Ağıllı Müqavilə komandası | Norito hadisələri + manifest qəbulundan istifadə edin. |
| Telemetred SLA pozuntuları ilə əlaqəli cəza cədvəlini (haqqın azaldılması, istiqrazların kəsilməsi) həyata keçirin. | İdarəetmə Şurası / Xəzinədarlıq | `DealEngine` hesablaşma çıxışları ilə uyğunlaşdırın. |
| Sənəd mübahisəsi prosesi və eskalasiya matrisi. | Sənədlər / İdarəetmə | Disput runbook + CLI köməkçiləri üçün keçid. |

### 4. Ölçmə və Haqqın Paylanması

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| `CapacityTelemetryV1` qəbul etmək üçün Torii ölçmə qəbulunu genişləndirin. | Torii Komandası | GiB-saatları, PoR müvəffəqiyyətini, iş vaxtını təsdiqləyin. |
| Sifariş üzrə istifadə + SLA statistikası haqqında hesabat vermək üçün `sorafs_node` ölçmə boru kəmərini yeniləyin. | Saxlama Komandası | Replikasiya sifarişləri və chunker tutacaqları ilə uyğunlaşdırın. |
| Hesablaşma boru kəməri: telemetriya + replikasiya məlumatlarını XOR-də nominal ödənişlərə çevirin, idarəetməyə hazır xülasələr hazırlayın və uçotun vəziyyətini qeyd edin. | Xəzinədarlıq / Saxlama Komandası | Deal Engine / Treasury ixracına daxil olun. |
| Ölçmə sağlamlığı üçün idarə panellərini/xəbərdarlıqlarını ixrac edin (alınma gecikməsi, köhnə telemetriya). | Müşahidə qabiliyyəti | SF-6/SF-7 tərəfindən istinad edilən Grafana paketini genişləndirin. |

- Torii indi `/v2/sorafs/capacity/telemetry` və `/v2/sorafs/capacity/state` (JSON + Norito) ifşa edir, beləliklə operatorlar epox telemetriya snapshotlarını təqdim edə, müfəttişlər isə audit və ya kanonik göstəriciləri əldə edə bilsinlər. qablaşdırma.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` inteqrasiyası replikasiya sifarişlərinin eyni son nöqtə vasitəsilə əldə edilməsini təmin edir; CLI köməkçiləri (`sorafs_cli capacity telemetry --from-file telemetry.json`) indi deterministik hashing və ləqəb həlli ilə avtomatlaşdırma işlərindən telemetriyanı təsdiqləyir/nəşr edir.
- Ölçmə snapshotları `metering` snapşotuna bərkidilmiş `CapacityTelemetrySnapshot` daxiletmələrini yaradır və Prometheus ixracları idxala hazır Grafana lövhəsini I18NI0000003X monitorunda qidalandırır. hesablama, proqnozlaşdırılan nano-SORA haqları və real vaxtda SLA uyğunluğu.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Ölçmələrin hamarlanması aktiv olduqda, snapşot `smoothed_gib_hours` və `smoothed_por_success_bps`-dən ibarətdir ki, operatorlar EMA trendli dəyərləri idarəetmənin ödənişlər üçün istifadə etdiyi xam sayğaclarla müqayisə edə bilsin.【crates/sorafs_node/src/metering.rs:401】:

### 5. Mübahisə və Ləğv Edilmə

| Tapşırıq | Sahib(lər) | Qeydlər |
|------|----------|-------|
| `CapacityDisputeV1` yükünü müəyyən edin (şikayətçi, sübut, hədəf provayder). | İdarəetmə Şurası | Norito sxemi + doğrulayıcı. |
| Mübahisələri bildirmək və cavab vermək üçün CLI dəstəyi (sübut əlavələri ilə). | Tooling WG | Sübut paketinin deterministik hashingini təmin edin. |
| Təkrarlanan SLA pozuntuları üçün avtomatlaşdırılmış yoxlamalar əlavə edin (mübahisə üçün avtomatik sürətləndirmə). | Müşahidə qabiliyyəti | Xəbərdarlıq hədləri və idarəetmə qarmaqları. |
| Sənədin ləğvi oyun kitabı (güzəşt müddəti, bağlanmış məlumatların evakuasiyası). | Sənədlər / Yaddaş Komandası | Siyasət sənədi və operator runbook-a keçid. |

## Test və CI Tələbləri- Bütün yeni sxem təsdiqləyiciləri üçün vahid testləri (`sorafs_manifest`).
- Simulyasiya edən inteqrasiya testləri: bəyannamə → təkrarlama sifarişi → ölçmə → ödəniş.
- Nümunə tutumlu bəyannamələrin/telemetriyanın bərpası və imzaların sinxron qalmasını təmin etmək üçün CI iş axını (`ci/check_sorafs_fixtures.sh`-i genişləndirin).
- Reyestr API üçün testləri yükləyin (10k provayderi, 100k sifarişi simulyasiya edin).

## Telemetriya və İdarə Panelləri

- İdarə paneli panelləri:
  - Elan edilmiş tutum provayderə görə istifadə edilənə qarşı.
  - Replikasiya sifarişi və orta tapşırıq gecikməsi.
  - SLA uyğunluğu (iş vaxtı %, PoR müvəffəqiyyət dərəcəsi).
  - Epoxa görə haqq hesablanması və cərimələr.
- Xəbərdarlıqlar:
  - Təchizatçı minimum öhdəlik götürülmüş imkandan aşağıdır.
  - Replikasiya sırası ilişib > SLA.
  - Ölçmə boru kəmərinin nasazlıqları.

## Sənədləşdirmə Çatdırılmaları

- Gücün bəyan edilməsi, öhdəliklərin yenilənməsi və istifadənin monitorinqi üçün operator təlimatı.
- Bəyannamələrin təsdiqi, sərəncamların verilməsi, mübahisələrin həlli üçün idarəetmə təlimatı.
- Tutumun son nöqtələri və replikasiya sifarişi formatı üçün API arayışı.
- Tərtibatçılar üçün Marketplace FAQ.

## GA Hazırlıq Yoxlama Siyahısı

Yol xəritəsi elementi **SF-2c** mühasibat uçotu üzrə konkret sübutlar əsasında istehsalın yayılmasını təmin edir,
mübahisələrin həlli və işə qəbul. Qəbul meyarlarını saxlamaq üçün aşağıdakı artefaktlardan istifadə edin
həyata keçirilməsi ilə sinxronlaşdırılır.

### Gecə uçotu və XOR uzlaşması
- Eyni pəncərə üçün tutum vəziyyəti snapshot və XOR kitab ixracını ixrac edin, sonra işə salın:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Köməkçi çatışmayan/artıq ödənilmiş hesablaşmalar və ya cərimələr üzrə sıfırdan fərqlənir və Prometheus buraxır
  mətn faylının xülasəsi.
- Xəbərdarlıq `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml`-də)
  uzlaşma metrikləri boşluqları bildirdikdə yanğınlar; tablosuna altında yaşayır
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- JSON xülasəsini və hashləri `docs/examples/sorafs_capacity_marketplace_validation/` altında arxivləşdirin
  idarəetmə paketləri ilə yanaşı.

### Mübahisə və sübutları kəsmək
- `sorafs_manifest_stub capacity dispute` vasitəsilə mübahisələri fayl (testlər:
  `cargo test -p sorafs_car --test capacity_cli`) beləliklə faydalı yüklər kanonik olaraq qalır.
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` və cəzanı işə salın
  suites (`record_capacity_telemetry_penalises_persistent_under_delivery`) mübahisələri sübut etmək və
  kəsiklər deterministik şəkildə təkrarlanır.
- Sübutların tutulması və eskalasiyası üçün `docs/source/sorafs/dispute_revocation_runbook.md`-i izləyin;
  Xəbərdarlıq təsdiqlərini təsdiqləmə hesabatına qaytarın.

### Provayderin işə düşməsi və tüstüdən çıxma testləri
- `sorafs_manifest_stub capacity ...` ilə bəyannamə/temetriya artefaktlarını bərpa edin və təkrar oxuyun
  təqdim etməzdən əvvəl CLI testləri (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Torii (`/v2/sorafs/capacity/declare`) vasitəsilə təqdim edin, sonra `/v2/sorafs/capacity/state` plus yazın
  Grafana ekran görüntüləri. `docs/source/sorafs/capacity_onboarding_runbook.md`-də çıxış axını izləyin.
- Arxivdə imzalanmış artefaktlar və uzlaşma çıxışları
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Asılılıqlar və ardıcıllıq

1. SF-2b-ni bitirin (qəbul siyasəti) — bazar yoxlanılmış provayderlərə əsaslanır.
2. Torii inteqrasiyasından əvvəl sxem + reyestr qatını (bu sənəd) həyata keçirin.
3. Ödənişləri aktivləşdirməzdən əvvəl ölçmə boru kəmərini tamamlayın.
4. Son addım: ölçmə məlumatları mərhələlərdə yoxlanıldıqdan sonra idarəetmə tərəfindən idarə olunan ödəniş paylanmasını aktivləşdirin.

Tərəqqi bu sənədə istinadlarla yol xəritəsində izlənilməlidir. Hər bir əsas bölmə (sxem, idarəetmə müstəvisi, inteqrasiya, ölçmə, mübahisələrin həlli) funksiyanın tam statusuna çatdıqdan sonra yol xəritəsini yeniləyin.