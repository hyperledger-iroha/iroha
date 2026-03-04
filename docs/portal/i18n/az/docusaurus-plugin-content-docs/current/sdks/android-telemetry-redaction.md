---
slug: /sdks/android-telemetry
lang: az
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Android Telemetry Redaction Plan
sidebar_label: Android Telemetry
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::Qeyd Kanonik Mənbə
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Telemetriya Redaksiya Planı (AND7)

## Əhatə dairəsi

Bu sənəd təklif olunan telemetriya redaksiyası siyasətini və imkanlarını əks etdirir
yol xəritəsi elementi **AND7** tələb etdiyi kimi Android SDK üçün artefaktlar. Uyğunlaşır
uçotu zamanı Rust node bazası ilə mobil cihazlar
cihaza xas məxfilik zəmanətləri. Çıxış üçün əvvəlcədən oxunuş kimi xidmət edir
Fevral 2026 SRE idarəetmə icmalı.

Məqsədlər:

- Paylaşılan müşahidə qabiliyyətinə çatan hər bir Android yayımlanan siqnalı kataloq edin
  arxa uçlar (OpenTelemetry izləri, Norito kodlu qeydlər, ölçülərin ixracı).
- Rust bazası və sənəd redaksiyasından fərqli olan sahələri təsnif edin və ya
  saxlama nəzarətləri.
- Dəstək qruplarının cavab verə bilməsi üçün aktivləşdirmə və sınaq işlərini təsvir edin
  redaksiya ilə bağlı xəbərdarlıqlara deterministik olaraq.

## Siqnal İnventarizasiyası (Qaralama)

Kanala görə qruplaşdırılmış planlaşdırılmış alətlər. Bütün sahə adları Android-i izləyir
SDK telemetriya sxemi (`org.hyperledger.iroha.android.telemetry.*`). Könüllü
sahələr `?` ilə qeyd olunur.

| Siqnal ID | Kanal | Əsas sahələr | PII/PHI Təsnifatı | Redaksiya / Saxlama | Qeydlər |
|----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | İz aralığı | `authority_hash`, `route`, `status_code`, `latency_ms` | Səlahiyyət ictimaidir; marşrutda heç bir sirr yoxdur | İxrac etməzdən əvvəl hashed səlahiyyəti (`blake2b_256`) buraxın; 7 gün saxlamaq | Güzgülər Rust `torii.http.request`; hashing mobil ləqəb məxfiliyini təmin edir. |
| `android.torii.http.retry` | Hadisə | `route`, `retry_count`, `error_code`, `backoff_ms` | Heç biri | Redaksiya yoxdur; 30 gün saxla | Deterministik təkrar yoxlamalar üçün istifadə olunur; Rust sahələri ilə eynidir. |
| `android.pending_queue.depth` | Ölçmə metrik | `queue_type`, `depth` | Heç biri | Redaksiya yoxdur; 90 gün saxlamaq | Rust `pipeline.pending_queue_depth` uyğun gəlir. |
| `android.keystore.attestation.result` | Hadisə | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias ​​(törəmə), cihaz metadata | Təxəllüsü deterministik etiketlə əvəz edin, markanı enum bucket | üçün redaktə edin AND2 attestasiyasına hazır olmaq üçün tələb olunur; Pas qovşaqları cihaz metadatasını yaymır. |
| `android.keystore.attestation.failure` | Sayğac | `alias_label`, `failure_reason` | Təxəllüs redaksiyasından sonra heç biri | Redaksiya yoxdur; 90 gün saxlamaq | Xaos təlimlərini dəstəkləyir; alias_label hash edilmiş ləqəbdən əldə edilmişdir. |
| `android.telemetry.redaction.override` | Hadisə | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Aktyor rolu operativ PII | kimi təsnif edilir Sahə ixracı maskalı rol kateqoriyası; audit jurnalı ilə 365 gün saxlayın | Rustda yoxdur; operatorlar dəstək vasitəsilə ləğvetmələri fayl etməlidirlər. |
| `android.telemetry.export.status` | Sayğac | `backend`, `status` | Heç biri | Redaksiya yoxdur; 30 gün saxla | Rust ixracatçı status sayğacları ilə paritet. |
| `android.telemetry.redaction.failure` | Sayğac | `signal_id`, `reason` | Heç biri | Redaksiya yoxdur; 30 gün saxla | Rust `streaming_privacy_redaction_fail_total` güzgü üçün tələb olunur. |
| `android.telemetry.device_profile` | Ölçü | `profile_id`, `sdk_level`, `hardware_tier` | Cihaz metadata | Kobud kovalar buraxın (SDK əsas, aparat səviyyəsi); 30 gün saxla | OEM xüsusiyyətlərini ifşa etmədən paritet tablosunu aktivləşdirir. |
| `android.telemetry.network_context` | Hadisə | `network_type`, `roaming` | Daşıyıcı PII | ola bilər `carrier_name`-i tamamilə buraxın; digər sahələri 7 gün saxla | `ClientConfig.networkContextProvider` təmizlənmiş snapshot təqdim edir ki, proqramlar abunəçi məlumatlarını ifşa etmədən şəbəkə növü + rouminq yaysın; paritet panelləri siqnalı Rust `peer_host`-in mobil analoqu kimi qəbul edir. |
| `android.telemetry.config.reload` | Hadisə | `source`, `result`, `duration_ms` | Heç biri | Redaksiya yoxdur; 30 gün saxla | Güzgülər Rust konfiqurasiyasının yenidən yüklənməsi aralığı. |
| `android.telemetry.chaos.scenario` | Hadisə | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Cihaz profili paketlənib | `device_profile` ilə eyni; 30 gün saxla | AND7 hazırlığı üçün tələb olunan xaos məşqləri zamanı qeyd edilib. |
| `android.telemetry.redaction.salt_version` | Ölçü | `salt_epoch`, `rotation_id` | Heç biri | Redaksiya yoxdur; 365 gün saxlamaq | Blake2b duzunun dönməsini izləyir; Android hash dövrü Rust qovşaqlarından ayrıldıqda paritet xəbərdarlığı. |
| `android.crash.report.capture` | Hadisə | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Qəza barmaq izi + metadata prosesi | Paylaşılan redaksiya duzu ilə `crash_id` hash, bucket watchdog vəziyyəti, ixracdan əvvəl yığın çərçivələrini buraxın; 30 gün saxla | `ClientConfig.Builder.enableCrashTelemetryHandler()` çağırıldıqda avtomatik aktivləşdirilir; cihazı müəyyən edən izləri ifşa etmədən paritet tablosunu qidalandırır. |
| `android.crash.report.upload` | Sayğac | `crash_id`, `backend`, `status`, `retry_count` | Qəza barmaq izi | Hashed `crash_id` təkrar istifadə edin, yalnız statusu buraxın; 30 gün saxla | `ClientConfig.crashTelemetryReporter()` və ya `CrashTelemetryHandler.recordUpload` vasitəsilə buraxın, beləliklə yükləmələr digər telemetriya ilə eyni Sigstore/OLTP zəmanətlərini paylaşır. |

### İcra qarmaqları

- `ClientConfig` indi manifestdən əldə edilən telemetriya məlumatlarını vasitəsilə ötürür
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`, avtomatik qeydiyyatdan keçir
  `TelemetryObserver` belə hashed səlahiyyətlilər və duz göstəriciləri sifarişli müşahidəçilər olmadan axır.
  Baxın `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  və altındakı yoldaş sinifləri
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Müraciətlər zəng edə bilər
  Qeydiyyatdan keçmək üçün `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)`
  əks əsaslı `AndroidNetworkContextProvider`, icra zamanı `ConnectivityManager` sorğularını verir
  və tərtib vaxtı Android təqdim etmədən `android.telemetry.network_context` hadisəsini yayır
  asılılıqlar.
- Vahid testləri `TelemetryOptionsTests` və `TelemetryObserverTests`
  (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) hashingi qoruyur
  köməkçiləri üstəgəl ClientConfig inteqrasiya çəngəlindən istifadə edərək reqressiyaları dərhal üzə çıxarır.
- Aktivləşdirmə dəsti/laboratoriyaları indi bu sənədi saxlayaraq psevdokod əvəzinə konkret API-lərə istinad edir və
  göndərmə SDK ilə uyğunlaşdırılmış runbook.

> **Əməliyyat qeydi:** sahibi/status iş vərəqi burada yaşayır
> `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` və olmalıdır
> hər AND7 yoxlama məntəqəsi zamanı bu cədvəlin yanında yenilənir.

## Paritet icazəli siyahıları və sxem fərqi iş axını

İdarəetmə ikili icazəli siyahı tələb edir ki, Android ixracı heç vaxt identifikatorları sızdırmasın
ki, Rust xidmətləri qəsdən üzə çıxır. Bu bölmə runbook girişini əks etdirir
(`docs/source/android_runbook.md` §2.3) lakin AND7 redaksiya planını saxlayır
öz-özünə.

| Kateqoriya | Android ixracatçıları | Rust xidmətləri | Doğrulama çəngəl |
|----------|-------------------|---------------|-----------------|
| Səlahiyyət / marşrut konteksti | Blake2b-256 vasitəsilə `authority`/`alias` hash edin və ixracdan əvvəl xam Torii host adlarını buraxın; duz fırlanmasını sübut etmək üçün `android.telemetry.redaction.salt_version` buraxın. | Korrelyasiya üçün tam Torii host adlarını və peer ID-lərini buraxın. | `docs/source/sdk/android/readiness/schema_diffs/` altında ən son diaqram fərqindəki `android.torii.http.request` ilə `torii.http.request` qeydlərini müqayisə edin, sonra duz dövrlərini təsdiqləmək üçün `scripts/telemetry/check_redaction_status.py`-i işə salın. |
| Cihaz və imzalayan şəxsiyyəti | Bucket `hardware_tier`/`device_profile`, hash nəzarətçi ləqəbləri və heç vaxt seriya nömrələrini ixrac etməyin. | Emit validator `peer_id`, nəzarətçi `public_key` və növbə heşlərini sözbəsöz. | `docs/source/sdk/mobile_device_profile_alignment.md` ilə uyğunlaşdırın, `java/iroha_android/run_tests.sh` daxilində ləqəb həshing testlərini həyata keçirin və laboratoriyalar zamanı arxiv növbəsi-inspektor çıxışlarını həyata keçirin. |
| Şəbəkə metadata | Yalnız ixrac `network_type` + `roaming`; `carrier_name` buraxın. | Peer hostname/TLS son nöqtə metadatasını saxlayın. | Hər bir sxem fərqini `readiness/schema_diffs/`-də saxlayın və Grafana-in “Şəbəkə Konteksti” vidceti daşıyıcı sətirləri göstərərsə xəbərdar edin. |
| Override / xaos sübut | Emit `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` maskalı aktyor rolları ilə. | Maskasız ləğvetmə təsdiqlərini yaymaq; xaosa xüsusi span yoxdur. | Məşqlərdən sonra `docs/source/sdk/android/readiness/and7_operator_enablement.md`-i çarpaz yoxlayın, tokenləri ləğv etmək + xaos artefaktları maskalanmamış Rust hadisələri ilə yanaşı oturur. |

İş axını:

1. Hər bir manifest/ixracatçı dəyişikliyindən sonra işə salın
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` və JSON-u `docs/source/sdk/android/readiness/schema_diffs/` altına yerləşdirin.
2. Yuxarıdakı cədvəllə müqayisədə fərqi nəzərdən keçirin. Android yalnız Rust sahəsi yayırsa
   (və ya əksinə), AND7 hazırlığı səhvini yazın və həm bu planı, həm də
   runbook.
3. Həftəlik əməliyyatların nəzərdən keçirilməsi zamanı icra edin
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   və hazırlıq iş vərəqində duz dövrü plus schema-diff vaxt damğasını qeyd edin.
4. `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`-də hər hansı sapmaları qeyd edin
   beləliklə, idarəetmə paketləri paritet qərarları qəbul edir.

> **Sxem arayışı:** kanonik sahə identifikatorları mənşəlidir
> `android_telemetry_redaction.proto` (Android SDK qurulması zamanı materiallaşdırılıb
> Norito deskriptorları ilə yanaşı). Sxem `authority_hash`-i ifşa edir,
> `alias_label`, `attestation_digest`, `device_brand_bucket` və
> `actor_role_masked` sahələri indi SDK və telemetriya ixracatçılarında istifadə olunur.

`authority_hash` qeydə alınmış Torii səlahiyyət dəyərinin sabit 32 baytlıq həzmidir.
protoda. `attestation_digest` kanonik attestasiya bəyanatını çəkir
barmaq izi, `device_brand_bucket` isə xam Android marka simini
təsdiq edilmiş siyahı (`generic`, `oem`, `enterprise`). `actor_role_masked` daşıyır
əvəzinə redaksiyanın ləğvi aktyor kateqoriyası (`support`, `sre`, `audit`)
xam istifadəçi identifikatoru.

### Crash Telemetry Export AlignmentCrash telemetry indi eyni OpenTelemetry ixracatçılarını və mənşəyi paylaşır
boru kəməri Torii şəbəkə siqnalları kimi idarəçilik təqibini bağlayaraq
dublikat ixracatçılar. Qəza idarəedicisi `android.crash.report.capture`-i qidalandırır
Hashed `crash_id` ilə hadisə (Blake2b-256 artıq redaksiya duzundan istifadə edir
`android.telemetry.redaction.salt_version` tərəfindən izlənilir), proses vəziyyəti vedrələri,
və sanitarlaşdırılmış ANR nəzarətçisi metadata. Yığın izləri cihazda qalır və yalnızdır
əvvəl `has_native_trace` və `anr_watchdog_bucket` sahələrinə ümumiləşdirilmişdir
ixrac edin ki, heç bir PII və ya OEM sətri cihazı tərk etsin.

Qəzanın yüklənməsi `android.crash.report.upload` əks girişini yaradır,
SRE-yə heç bir şey öyrənmədən arxa uçun etibarlılığını yoxlamağa imkan verir
istifadəçi və ya yığın izi. Hər iki siqnal Torii ixracatçısını təkrar istifadə etdiyi üçün onlar miras alırlar
eyni Sigstore imzalama, saxlama siyasəti və xəbərdarlıq qarmaqları artıq müəyyən edilmişdir
AND7 üçün. Dəstək runbooks buna görə də hash edilmiş qəza identifikatorunu əlaqələndirə bilər
sifarişli qəza kəməri olmadan Android və Rust sübut paketləri arasında.

`ClientConfig.Builder.enableCrashTelemetryHandler()` vasitəsilə işləyicini bir dəfə aktivləşdirin
telemetriya seçimləri və yuvalar konfiqurasiya edilmişdir; qəza yükləmə körpüləri təkrar istifadə edə bilər
`ClientConfig.crashTelemetryReporter()` (və ya `CrashTelemetryHandler.recordUpload`)
eyni imzalanmış boru kəmərində backend nəticələrini yaymaq.

## Siyasət Deltaları və Rust Baseline

Azaltma addımları ilə Android və Rust telemetriya siyasətləri arasındakı fərqlər.

| Kateqoriya | Rust Baseline | Android Siyasəti | Azaltma / Təsdiqləmə |
|----------|---------------|----------------|-------------------------|
| Səlahiyyət / həmyaşıd identifikatorları | Sadə səlahiyyət sətirləri | `authority_hash` (Blake2b-256, fırlanan duz) | `iroha_config.telemetry.redaction_salt` vasitəsilə dərc edilən paylaşılan duz; paritet testi dəstək işçiləri üçün geri çevrilə bilən xəritələşdirməni təmin edir. |
| Host / şəbəkə metadata | Node hostnames/IPs ixrac | Şəbəkə növü + yalnız rouminq | Şəbəkə sağlamlığı panelləri host adları əvəzinə əlçatanlıq kateqoriyalarından istifadə etmək üçün yeniləndi. |
| Cihazın xüsusiyyətləri | N/A (server tərəfi) | Bucketed profil (SDK 21/23/29+, səviyyə `emulator`/`consumer`/`enterprise`) | Xaos məşqləri vedrə xəritəsini yoxlayır; daha incə detallara ehtiyac olduqda runbook sənədlərinin yüksəldilməsi yolunu dəstəkləyin. |
| Redaksiya ləğv edilir | dəstəklənmir | Norito kitabçasında (`actor_role_masked`, `reason`) saxlanılan əl ilə ləğvetmə nişanı | Ləğv etmək üçün imzalanmış sorğu tələb olunur; audit jurnalı 1 il saxlanılır. |
| Attestasiya izləri | Yalnız SRE vasitəsilə server attestasiyası | SDK təmizlənmiş attestasiya xülasəsini yayır | Rust attestasiya validatoruna qarşı attestasiya hashlərini çarpaz yoxlayın; hashed ləqəb sızmanın qarşısını alır. |

Doğrulama yoxlama siyahısı:

- Əvvəllər hashed/maskalı sahələri yoxlayan hər bir siqnal üçün redaksiya vahidi testləri
  ixracatçının təqdimatı.
- Sxem fərqi aləti (Pas qovşaqları ilə paylaşılır) sahə paritetini təsdiqləmək üçün hər gecə işləyir.
- Xaos məşqi skript məşqləri iş axınını ləğv edir və audit qeydini təsdiqləyir.

## İcra Tapşırıqları (SRE-dən əvvəl İdarəetmə)

1. **İnventar Təsdiqi** — Yuxarıdakı cədvəli faktiki Android SDK ilə çarpaz yoxlayın
   cihaz qarmaqları və Norito sxem tərifləri. Sahiblər: Android
   Müşahidə qabiliyyəti TL, LLM.
2. **Telemetri Şeması Fərq** — Pas göstəricilərinə qarşı paylaşılan fərq alətini işə salın
   SRE baxışı üçün paritet artefaktlar hazırlayın. Sahib: SRE məxfilik rəhbəri.
3. **Runbook Qaralama (Tamamlandı 2026-02-03)** — `docs/source/android_runbook.md`
   indi başdan sona ləğvetmə iş prosesini (Bölmə 3) və genişləndirilmişi sənədləşdirir
   eskalasiya matrisi üstəgəl rol məsuliyyətləri (Bölmə 3.1), CLI-ni birləşdirir
   köməkçilər, insident sübutları və xaos skriptləri idarəetmə siyasətinə qayıdır.
   Sahiblər: Sənədlər/Dəstək redaktəsi ilə LLM.
4. **Enablement Content** — Brifinq slaydlarını, laboratoriya təlimatlarını və
   Fevral 2026 sessiyası üçün bilik yoxlama sualları. Sahiblər: Sənədlər/Dəstək
   Menecer, SRE aktivləşdirmə komandası.

## İş axını və Runbook qarmaqlarını aktivləşdirin

### 1. Yerli + CI tüstü əhatəsi

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` Torii qum qutusunu fırladır, kanonik çoxmənbəli SoraFS qurğusunu təkrarlayır (`ci/check_sorafs_orchestrator_adoption.sh`-ə həvalə edir) və sintetik Android telemetriyasını toxumlayır.
  - Trafikin yaradılması `scripts/telemetry/generate_android_load.py` tərəfindən idarə olunur, o, `artifacts/android/telemetry/load-generator.log` altında sorğu/cavab stenoqramını qeyd edir və başlıqları, yolun dəyişdirilməsini və ya quru işləmə rejimini qeyd edir.
  - Köməkçi SoraFS tablosunu/xülasələrini `${WORKDIR}/sorafs/`-ə köçürür ki, AND7 məşqləri mobil müştərilərə toxunmazdan əvvəl çox mənbəli pariteti sübut edə bilsin.
- CI eyni alətdən təkrar istifadə edir: `ci/check_android_dashboard_parity.sh` `scripts/telemetry/compare_dashboards.py`-i `dashboards/grafana/android_telemetry_overview.json`, Rust arayış panelinə və `dashboards/data/android_rust_dashboard_allowances.json`-də icazə faylına qarşı işlədir, imzalanmış fərq snapshot I08NI00000 verir.
- Xaos məşqləri `docs/source/sdk/android/telemetry_chaos_checklist.md` izləyir; nümunə-env skripti üstəgəl tablosunun pariteti yoxlanışı AND7 yanma auditini təmin edən “hazır” sübut paketini təşkil edir.

### 2. Buraxılış və audit izini ləğv edin

- `scripts/android_override_tool.py` redaksiyanın ləğvi üçün qanuni CLI-dir. `apply` imzalanmış sorğunu qəbul edir, manifest paketini verir (defolt olaraq `telemetry_redaction_override.to`) və `docs/source/sdk/android/telemetry_override_log.md`-ə heşlənmiş işarə cərgəsi əlavə edir. `revoke` ləğvetmə vaxt damğasını həmin cərgəyə qarşı möhürləyir və `digest` idarəetmə üçün tələb olunan təmizlənmiş JSON snapşotunu yazır.
- `docs/source/android_support_playbook.md`-də qeyd olunan uyğunluq tələbinə uyğun gələn Markdown cədvəlinin başlığı mövcud olmadıqda CLI audit jurnalını dəyişdirməkdən imtina edir. `scripts/tests/test_android_override_tool_cli.py`-də bölmənin əhatə dairəsi cədvəl analizatorunu, manifest emitentlərini və səhvlərin idarə edilməsini qoruyur.
- Operatorlar hər dəfə ləğvetmə tətbiq edildikdə yaradılan manifest, yenilənmiş jurnaldan çıxarışı, **və ** `docs/source/sdk/android/readiness/override_logs/` altında həzm JSON-u əlavə edirlər; jurnal bu plandakı idarəetmə qərarına görə 365 gün tarixi saxlayır.

### 3. Sübutların tutulması və saxlanması

- Hər bir məşq və ya insident aşağıdakıları ehtiva edən `artifacts/android/telemetry/` altında strukturlaşdırılmış paket yaradır:
  - `generate_android_load.py`-dən yük generatoru transkripti və aqreqat sayğacları.
  - İdarə panelinin paritet fərqi (`android_vs_rust-<stamp>.json`) və `ci/check_android_dashboard_parity.sh` tərəfindən buraxılan ehtiyat hash.
  - Ləğv et jurnalının deltası (əgər ləğv edilibsə), müvafiq manifest və yenilənmiş həzm JSON.
- SRE yanma hesabatı həmin artefaktlara və `android_sample_env.sh` tərəfindən kopyalanan SoraFS tablosuna istinad edir və AND7 hazırlığının nəzərdən keçirilməsinə telemetriya heşlərindən → tablosundan → ləğvetmə statusundan deterministik zəncir verir.

## Çapraz SDK Cihaz Profilinin Hizalanması

Panellər Android-in `hardware_tier`-ni kanonik formata çevirir
`mobile_profile_class`-də müəyyən edilmişdir
`docs/source/sdk/mobile_device_profile_alignment.md` belə AND7 və IOS7 telemetriyası
eyni kohortları müqayisə edin:

- `lab` - `hardware_tier = emulator` kimi yayılır, Swift-ə uyğun gəlir
  `device_profile_bucket = simulator`.
- `consumer` — `hardware_tier = consumer` kimi buraxılır (SDK əsas şəkilçisi ilə)
  və Swift-in `iphone_small`/`iphone_large`/`ipad` vedrələri ilə qruplaşdırılıb.
- `enterprise` - `hardware_tier = enterprise` kimi yayılır, Swift ilə uyğunlaşır
  `mac_catalyst` paketi və gələcək idarə olunan/iOS masaüstü iş vaxtları.

İstənilən yeni səviyyə hizalanma sənədinə və diaqram fərqlərinin artefaktlarına əlavə edilməlidir
tablosuna istifadə etməzdən əvvəl.

## İdarəetmə və Bölüşdürmə

- **Öncədən oxunan paket** — Bu sənəd və əlavə artefaktlar (sxem fərqi,
  runbook diff, hazırlıq göyərtəsinin konturları) SRE idarəçiliyinə paylanacaq
  poçt siyahısı **2026-02-05**-dən gec olmayaraq.
- **Əlaqə döngəsi** — İdarəetmə zamanı toplanmış şərhlər daxil olacaq
  `AND7` JIRA epik; blokerlər `status.md` və həftəlik Android-də üzə çıxır
  stand-up qeydləri.
- **Nəşriyyat** — Təsdiq edildikdən sonra siyasət xülasəsi ilə əlaqələndiriləcək
  `docs/source/android_support_playbook.md` və paylaşılan tərəfindən istinad edilir
  `docs/source/telemetry.md`-də telemetriya ilə bağlı FAQ.

## Audit və Uyğunluq Qeydləri

- Siyasət mobil abunəçi məlumatlarını silməklə GDPR/CCPA tələblərinə cavab verir
  ixracdan əvvəl; Hashed səlahiyyət duzu rübdə bir fırlanır və saxlanılır
  paylaşılan sirlər anbarı.
- Aktivləşdirmə artefaktları və runbook yeniləmələri uyğunluq reyestrinə daxil edilir.
- Rüblük rəylər təsdiq edir ki, ləğvetmələr qapalı olaraq qalır (köhnə giriş yoxdur).

## İdarəetmə Nəticəsi (2026-02-12)

**2026-02-12** tarixində SRE idarəetmə sessiyası Android redaksiyasını təsdiqlədi
dəyişdirilmədən siyasət. Əsas qərarlar (bax
`docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Siyasətlərin qəbulu.** Hashed səlahiyyəti, cihaz profilinin toplanması və
  daşıyıcı adlarının buraxılması təsdiq edilmişdir. Vasitəsilə duz fırlanma izlənilməsi
  `android.telemetry.redaction.salt_version` rüblük audit bəndinə çevrilir.
- **Təsdiqləmə planı.** Vahid/inteqrasiya əhatə dairəsi, gecəlik sxem fərqləri və
  rüblük xaos məşqləri təsdiqləndi. Fəaliyyət elementi: tablosunu dərc edin
  hər məşqdən sonra paritet hesabatı.
- **İdarəetməni ləğv edin.** Norito-də qeydə alınmış ləğvetmə nişanları
  365 günlük saxlama pəncərəsi. Dəstək mühəndisliyi ləğvetmə jurnalına sahib olacaq
  aylıq əməliyyatlar sinxronizasiyası zamanı həzm icmalı.

## İzləmə Status

1. **Cihaz profilinin düzülməsi (2026-03-01 tarixinə qədər).** ✅ Tamamlandı — paylaşılan
   `docs/source/sdk/mobile_device_profile_alignment.md`-də xəritəçəkmə necə olduğunu müəyyən edir
   Android `hardware_tier` dəyərlərin kanonik `mobile_profile_class` ilə xəritəsi
   paritet panelləri və schema diff alətləri tərəfindən istehlak edilir.

## Qarşıdan gələn SRE İdarəetmə Qısacası (Q22026)Yol xəritəsinin **AND7** bəndi tələb edir ki, növbəti SRE idarəetmə sessiyası a
qısa Android telemetriya redaksiyasının əvvəlcədən oxunması. Bu bölməni canlı kimi istifadə edin
qısa; hər şura iclasından əvvəl onu yeniləyin.

### Hazırlıq yoxlama siyahısı

1. **Sübut paketi** — ən son sxem fərqini, tablosuna ekran görüntülərini ixrac edin,
   və log həzmini ləğv edin (aşağıdakı matrisə baxın) və onları tarixli işarənin altına qoyun
   qovluq (məsələn
   `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) əvvəl
   dəvət göndərir.
2. **Qazma xülasəsi** — ən son xaos məşq jurnalını əlavə edin
   `android.telemetry.redaction.failure` metrik snapshot; Alertmanager təmin edin
   annotasiyalar eyni vaxt damğasına istinad edir.
3. **Auditi ləğv et** — bütün aktiv ləğvetmələrin Norito-də qeyd olunduğunu təsdiqləyin
   reyestrdə və yığıncaqda ümumiləşdirilir. Son istifadə tarixlərini və
   müvafiq hadisə identifikatorları.
4. **Gündəlik qeydi** — iclasdan 48 saat əvvəl SRE sədrinə zəng edin
   tələb olunan hər hansı qərarları vurğulayan qısa keçid (yeni siqnallar, saxlama
   dəyişikliklər və ya siyasət yeniləmələrini ləğv edin).

### Sübut matrisi

| Artefakt | Məkan | Sahibi | Qeydlər |
|----------|----------|-------|-------|
| Sxem fərqi vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetriya alətləri DRI | Görüşdən <72 saat əvvəl yaradılmalıdır. |
| Dashboard diff ekran görüntüləri | `docs/source/sdk/android/readiness/dashboards/<date>/` | Müşahidə qabiliyyəti TL | `sorafs.fetch.*`, `android.telemetry.*` və Alertmanager snapşotlarını daxil edin. |
| Dijesti ləğv edin | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Dəstək mühəndisliyi | Ən son `telemetry_override_log.md`-ə qarşı `scripts/android_override_tool.sh digest` (həmin kataloqda README-ə baxın) işə salın; tokenlər paylaşmadan əvvəl heşlənmiş qalır. |
| Xaos məşq qeydi | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA avtomatlaşdırılması | KPI xülasəsini əlavə edin (dayanma sayı, təkrar cəhd nisbəti, istifadəni ləğv edin). |

### Şura üçün açıq suallar

- İndi 365 gündən ləğv edilmiş saxlama pəncərəsini qısaltmalıyıq
  həzm avtomatlaşdırılmışdır?
- `android.telemetry.device_profile` yeni paylaşılanı qəbul etməlidir
  Növbəti buraxılışda `mobile_profile_class` etiketləri və ya Swift/JS-i gözləyin
  Eyni dəyişikliyi göndərmək üçün SDK-lar?
- Torii-dən sonra regional məlumat rezidentliyi üçün əlavə təlimat tələb olunur
  Norito-RPC hadisələri Android-ə enir (NRPC-3 təqibi)?

### Telemetriya Sxema Fərqləri Proseduru

Sxem fərq alətini hər buraxılış namizədi üçün ən azı bir dəfə (və Android
alətlər dəyişiklikləri) beləliklə, SRE şurası ilə yanaşı təzə paritet artefaktları alır
tablosuna fərq:

1. Müqayisə etmək istədiyiniz Android və Rust telemetriya sxemlərini ixrac edin. CI üçün konfiqurasiyalar canlıdır
   `configs/android_telemetry.json` və `configs/rust_telemetry.json` altında.
2. `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json` yerinə yetirin.
   - Alternativ olaraq öhdəlikləri (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) keçirin
     konfiqurasiyaları birbaşa git-dən çəkin; skript heşləri artefaktın içinə yapışdırır.
3. Yaradılmış JSON-u hazırlıq paketinə əlavə edin və onu `status.md` + ilə əlaqələndirin
   `docs/source/telemetry.md`. Fərq əlavə edilmiş/çıxarılmış sahələri və saxlama deltalarını vurğulayır
   auditorlar aləti təkrar oynatmadan redaksiyanın paritetini təsdiq edə bilərlər.
4. Fərq icazə verilən fərqliliyi aşkar etdikdə (məsələn, yalnız Android-in ləğvi siqnalları),
   `ci/check_android_dashboard_parity.sh` tərəfindən istinad edilən müavinət faylı və əsaslandırmanı qeyd edin
   schema-diff kataloqu README.

> **Arxiv qaydaları:** ən son beş fərqi aşağıda saxlayın
> `docs/source/sdk/android/readiness/schema_diffs/` və köhnə şəkilləri bura köçürün
> `artifacts/android/telemetry/schema_diffs/` beləliklə, idarəetmə rəyçiləri həmişə ən son məlumatları görürlər.