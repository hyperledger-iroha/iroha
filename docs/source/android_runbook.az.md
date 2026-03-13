---
lang: az
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T15:38:09.507154+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Əməliyyatlar Runbook

Bu runbook Android SDK-nı idarə edən operatorları və dəstək mühəndislərini dəstəkləyir
AND7 və ondan kənar üçün yerləşdirmələr. SLA üçün Android Support Playbook ilə cütləşdirin
təriflər və eskalasiya yolları.

> **Qeyd:** Hadisə prosedurlarını yeniləyərkən, paylaşılanları da yeniləyin
> problemlərin aradan qaldırılması matrisi (`docs/source/sdk/android/troubleshooting.md`).
> ssenari cədvəli, SLA-lar və telemetriya arayışları bu runbook ilə uyğunlaşdırılır.

## 0. Sürətli başlanğıc (peycerlər işə salındıqda)

Təfərrüata dalmadan əvvəl Sev1/Sev2 xəbərdarlığı üçün bu ardıcıllığı əlinizdə saxlayın
aşağıdakı bölmələr:

1. **Aktiv konfiqurasiyanı təsdiq edin:** `ClientConfig` manifest yoxlama məbləğini çəkin
   tətbiqin işə salınması zamanı yayılır və onu daxil edilmiş manifestlə müqayisə edin
   `configs/android_client_manifest.json`. Əgər hashlər bir-birindən ayrılırsa, buraxılışları dayandırın və
   telemetriya/dəyişmələrə toxunmazdan əvvəl konfiqurasiya drift biletini təqdim edin (bax §1).
2. **Sxem fərq qapısını işə salın:** `telemetry-schema-diff` CLI-ni yerinə yetirin
   qəbul edilən snapshot
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   İstənilən `policy_violations` çıxışını Sev2 kimi qəbul edin və ixracı bloklayana qədər
   uyğunsuzluq başa düşülür (bax §2.6).
3. **İdarə panelləri + CLI statusunu yoxlayın:** Android Telemetriya Redaksiyasını açın və
   İhracatçı Sağlamlıq panolarını, ardından çalıştırın
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. Əgər
   Səlahiyyətlilər zəmin altındadır və ya ixracat səhvi, ekran görüntüləri və
   Hadisə sənədi üçün CLI çıxışı (bax §2.4–§2.5).
4. **Qeyd etmələrə qərar verin:** Yalnız yuxarıdakı addımlardan sonra və hadisə/sahibi ilə
   qeydə alındı, `scripts/android_override_tool.sh` vasitəsilə məhdud yalnış yazın
   və onu `telemetry_override_log.md`-də daxil edin (bax §3). Defolt müddəti: <24 saat.
5. **Əlaqə siyahısına görə artırın:** Zəng zamanı Android və Müşahidə oluna bilən TL-ni səhifələyin
   (§8-də kontaktlar), sonra §4.1-dəki eskalasiya ağacını izləyin. Əgər attestasiya və ya
   StrongBox siqnalları iştirak edir, ən son paketi çəkin və kəməri işə salın
   ixracı yenidən aktivləşdirməzdən əvvəl §7-dən yoxlayın.

## 1. Konfiqurasiya və Yerləşdirmə

- **ClientConfig mənbəsi:** Android müştərilərinin Torii son nöqtəsini, TLS-ni yükləməsini təmin edin
  siyasətlər və `iroha_config`-dən əldə edilən manifestlərdən təkrar cəhd düymələri. Doğrulayın
  proqramın işə salınması zamanı dəyərlər və aktiv manifestin yoxlama cəmi.
  İcra arayışı: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  iplər `TelemetryOptions`-dən `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (əlavə olaraq yaradılan `TelemetryObserver`) belə hash edilmiş səlahiyyətlilər avtomatik olaraq buraxılır.
- **İsti yenidən yükləmə:** `iroha_config`-i götürmək üçün konfiqurasiya nəzarətçisindən istifadə edin
  proqramın yenidən başlaması olmadan yeniləmələr. Uğursuz yenidən yükləmələr yaymalıdır
  `android.telemetry.config.reload` hadisəsi və eksponensial ilə təkrar cəhd edin
  geri çəkilmə (maksimum 5 cəhd).
- **Geri qaytarma davranışı:** Konfiqurasiya olmadıqda və ya etibarsız olduqda, geri qayıdın
  təhlükəsiz defoltları (yalnız oxumaq üçün rejim, gözlənilən növbə təqdimatı yoxdur) və istifadəçinin üzünü göstərin
  tələsik. Hadisəni izləmək üçün qeyd edin.

### 1.1 Yenidən yükləmə diaqnostikasını konfiqurasiya edin- Konfiqurasiya izləyicisi ilə `android.telemetry.config.reload` siqnalları verir
  `source`, `result`, `duration_ms` və isteğe bağlı `digest`/`error` sahələri (bax.
  `configs/android_telemetry.json` və
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  Tətbiq olunan manifest üçün tək `result:"success"` hadisəsini gözləyin; təkrarlanır
  `result:"error"` qeydləri izləyicinin 5 geri çəkilmə cəhdini tükəndiyini göstərir
  50ms-dən başlayaraq.
- Hadisə zamanı kollektordan ən son yenidən yükləmə siqnalını çəkin
  (OTLP/span mağazası və ya redaksiya statusunun son nöqtəsi) və `digest` + daxil edin
  Hadisə sənədində `source`. Həzmi müqayisə edin
  `configs/android_client_manifest.json` və buraxılış manifestinə paylanmışdır
  operatorlar.
- İzləyici səhvlər yaymağa davam edərsə, təkrar istehsal etmək üçün hədəf qoşqudan istifadə edin
  şübhəli manifest ilə təhlil uğursuzluğu:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  SRE üçün sınaq çıxışını və uğursuzluq manifestini hadisə paketinə əlavə edin
  onu bişmiş konfiqurasiya sxemindən fərqləndirə bilər.
- Yenidən yükləmə telemetriyası olmadıqda, aktiv `ClientConfig`-in
  telemetriya sink və OTLP kollektor hələ də qəbul edir
  `android.telemetry.config.reload` ID; əks halda onu Sev2 telemetriyası kimi qəbul edin
  reqressiya (§2.4 ilə eyni yol) və siqnal qayıdana qədər buraxılışları dayandırın.

### 1.2 Deterministik əsas ixrac paketləri
- Proqram ixracı indi hər ixrac duzu + nonce, `kdf_kind` və `kdf_work_factor` ilə v3 paketləri yayır.
  İxracatçı Argon2id-ə (64 MiB, 3 iterasiya, paralellik = 2) üstünlük verir və geri qayıdır.
  Argon2id cihazda mövcud olmadıqda 350 k iterasiya mərtəbəsi ilə PBKDF2-HMAC-SHA256. Paket
  AAD hələ də ləqəblə bağlanır; parol ifadələri v3 ixracı üçün ən azı 12 simvoldan ibarət olmalıdır
  idxalçı tamamilə sıfır duz/nəfəs olmayan toxumları rədd edir.
  `KeyExportBundle.decode(Base64|bytes)`, orijinal parol ilə idxal edin və v3-ə yenidən ixrac edin
  sərt yaddaş formatına keçin. İdxalçı tamamilə sıfır və ya təkrar istifadə edilmiş duz/qeyri-neft cütlərini rədd edir; həmişə
  köhnə ixracları cihazlar arasında təkrar istifadə etmək əvəzinə paketləri fırladın.
- `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests`-də mənfi yol testləri
  imtina. İstifadədən sonra parol ifadəsi massivlərini silin və həm paket versiyasını, həm də `kdf_kind`-i çəkin
  bərpa uğursuz olduqda hadisə qeydlərində.

## 2. Telemetriya və Redaksiya

> Tez istinad: bax
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> aktivləşdirmə zamanı istifadə edilən sıxlaşdırılmış komanda/ərəfəsində yoxlama siyahısı üçün
> seanslar və insident körpüləri.- **Siqnal inventar:** `docs/source/sdk/android/telemetry_redaction.md`-ə baxın
  yayılan spanların/metriklərin/hadisələrin tam siyahısı üçün və
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  sahibi/təsdiqləmə təfərrüatları və gözlənilməz boşluqlar üçün.
- **Kanonik sxem fərqi:** Təsdiqlənmiş AND7 şəklidir
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  Rəyçilər görə bilməsi üçün hər yeni CLI əməliyyatı bu artefaktla müqayisə edilməlidir
  qəbul edilmiş `intentional_differences` və `android_only_signals` hələ də
  sənədləşdirilmiş siyasət cədvəllərinə uyğundur
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. CLI indi əlavə edir
  `policy_violations` hər hansı qəsdən fərq olmadıqda a
  `status:"accepted"`/`"policy_allowlisted"` (və ya yalnız Android qeydləri itirildikdə
  onların qəbul edilmiş statusu), buna görə də boş olmayan pozuntuları Sev2 kimi qəbul edin və dayandırın
  ixrac edir. Aşağıdakı `jq` fraqmentləri arxivdə əl ilə ağıl yoxlanışı kimi qalır
  artefaktlar:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  Bu əmrlərdən hər hansı bir çıxışı lazım olan sxem reqresiyası kimi qəbul edin
  Telemetriya ixracı davam etməzdən əvvəl AND7 hazırlıq səhvi; `field_mismatches`
  `telemetry_schema_diff.md` §5-ə uyğun olaraq boş qalmalıdır. Köməkçi indi yazır
  `artifacts/android/telemetry/schema_diff.prom` avtomatik; keçmək
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (və ya set
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) səhnələşdirmə/istehsal hostlarında işləyərkən
  beləliklə, `telemetry_schema_diff_run_status` ölçü cihazı `policy_violation`-ə çevrilir
  CLI sürüşməni aşkar edərsə avtomatik.
- **CLI köməkçisi:** `scripts/telemetry/check_redaction_status.py` yoxlayır
  Defolt olaraq `artifacts/android/telemetry/status.json`; `--status-url`-ə keçir
  yerli nüsxəni oflayn olaraq yeniləmək üçün sorğu quruluşu və `--write-cache`
  matkaplar. `--min-hashed 214` istifadə edin (və ya təyin edin
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) idarəetməni tətbiq etmək
  hər status sorğusu zamanı hashed səlahiyyətlilərə söz.
- **Səlahiyyət heşinqi:** Bütün səlahiyyətlilər Blake2b-256 ilə hashing edilir.
  rüblük fırlanma duzu təhlükəsiz sirlər anbarında saxlanılır. Fırlanmalar baş verir
  hər rübün ilk bazar ertəsi 00:00 UTC. İxracatçının götürdüyünü yoxlayın
  `android.telemetry.redaction.salt_version` metrikasını yoxlayaraq yeni duz.
- **Cihaz profili qutuları:** Yalnız `emulator`, `consumer` və `enterprise`
  səviyyələr ixrac olunur (SDK əsas versiyası ilə birlikdə). İdarə panelləri bunları müqayisə edir
  Rust əsas göstəricilərinə qarşı hesablanır; >10% fərq xəbərdarlıqları artırır.
- **Şəbəkə metadatası:** Android yalnız `network_type` və `roaming` bayraqlarını ixrac edir.
  Daşıyıcı adları heç vaxt yayılmır; operatorlar abunəçi tələb etməməlidir
  hadisə jurnallarında məlumat. Dezinfeksiya edilmiş snapshot olaraq yayılır
  `android.telemetry.network_context` hadisəsi, ona görə də proqramların qeydiyyatdan keçməsini təmin edin a
  `NetworkContextProvider` (ya vasitəsilə
  `ClientConfig.Builder.setNetworkContextProvider(...)` və ya rahatlıq
  `enableAndroidNetworkContext(...)` köməkçisi) Torii zəngləri verilməzdən əvvəl.
- **Grafana göstərici:** `Android Telemetry Redaction` idarə paneli
  yuxarıdakı CLI çıxışı üçün kanonik vizual yoxlama - təsdiqləyin
  `android.telemetry.redaction.salt_version` paneli cari duz dövrünə uyğun gəlir
  və `android_telemetry_override_tokens_active` vidceti sıfırda qalır
  heç bir məşq və ya insident olmadıqda. Panellərdən hər hansı biri sürüşərsə, artırın
  CLI skriptləri reqressiyanı bildirməzdən əvvəl.

### 2.1 İxrac boru kəməri iş axını1. **Konfiqurasiya paylanması.** `ClientConfig.telemetry.redaction` yivlidir
   `iroha_config` və `ConfigWatcher` tərəfindən yenidən yüklənmişdir. Hər bir yenidən yükləmə qeyd edir
   manifest həzm üstəgəl duz dövrü - hadisələrdə və zamanı həmin xətti tutmaq
   məşqlər.
2. **Alətlər.** SDK komponentləri aralıqlar/metrikalar/hadisələr yayır.
   `TelemetryBuffer`. Bufer hər bir faydalı yükü cihaz profili ilə və
   cari duz dövrü beləliklə ixracatçı hashing daxilolmalarını deterministik şəkildə yoxlaya bilsin.
3. **Redaksiya filtri.** `RedactionFilter` `authority`, `alias` və
   cihazı tərk etməzdən əvvəl cihaz identifikatorları. Uğursuzluqlar yayılır
   `android.telemetry.redaction.failure` və ixrac cəhdini bloklayın.
4. **İxracatçı + kollektor.** Sanitarlaşdırılmış faydalı yüklər Android vasitəsilə göndərilir
   `android-otel-collector` yerləşdirilməsi üçün OpenTelemetry ixracatçısı. The
   kollektor fanatları izlərə (Tempo), ölçülərə (Prometheus) və Norito çıxış edir
   log yuvaları.
5. **Müşahidə qarmaqları.** `scripts/telemetry/check_redaction_status.py` oxuyur
   kollektor sayğacları (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) və status paketini yaradır
   bu runbook boyunca istinad edilir.

### 2.2 Doğrulama qapıları

- **Sxem fərqi:** Çalışın
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  təzahür edəndə dəyişir. Hər qaçışdan sonra hər birini təsdiqləyin
  `intentional_differences[*]` və `android_only_signals[*]` girişi möhürlənib
  `status:"accepted"` (və ya hashed/bucketed üçün `status:"policy_allowlisted"`
  sahələri) əlavə etməzdən əvvəl `telemetry_schema_diff.md` §3-də tövsiyə edildiyi kimi
  hadisələr və xaos laboratoriya hesabatlarına artefakt. Təsdiqlənmiş snapshotdan istifadə edin
  (`android_vs_rust-20260305.json`) qoruyucu barmaqlıq kimi və təzə buraxılan
  JSON təqdim edilməzdən əvvəl:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  `$LATEST` ilə müqayisə edin
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  icazəli siyahının dəyişməz qaldığını sübut etmək. Çatışmayan və ya boş `status`
  girişlər (məsələn, `android.telemetry.redaction.failure` və ya
  `android.telemetry.redaction.salt_version`) indi reqressiya və kimi qəbul edilir
  baxış bağlanmazdan əvvəl həll edilməlidir; CLI qəbul ediləni üzə çıxarır
  birbaşa dövlət, buna görə də təlimat §3.4 çarpaz istinad yalnız zaman tətbiq edilir
  qeyri-`accepted` statusunun niyə göründüyünü izah edir.

  **Kanonik AND7 siqnalları (03-05-2026 snapshot)**| Siqnal | Kanal | Status | İdarəetmə qeydi | Doğrulama çəngəl |
  |--------|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` | Hadisə | `accepted` | Güzgülər manifestləri ləğv edir və `telemetry_override_log.md` girişlərinə uyğun olmalıdır. | §3-ə uyğun olaraq `android_telemetry_override_tokens_active` və arxiv manifestlərinə baxın. |
  | `android.telemetry.network_context` | Hadisə | `accepted` | Android daşıyıcı adlarını qəsdən redaktə edir; yalnız `network_type` və `roaming` ixrac edilir. | Tətbiqlərin `NetworkContextProvider`-ni qeydiyyatdan keçirməsini və `Android Telemetry Overview`-də Torii trafikinə uyğun tədbir həcmini təsdiqləyin. |
  | `android.telemetry.redaction.failure` | Sayğac | `accepted` | Hashing uğursuz olduqda yayır; idarəetmə indi diff artefakt sxemində açıq status metadata tələb edir. | `Redaction Compliance` idarə paneli paneli və `check_redaction_status.py`-dən CLI çıxışı məşqlər zamanı istisna olmaqla sıfırda qalmalıdır. |
  | `android.telemetry.redaction.salt_version` | Ölçü | `accepted` | İxracatçının cari rüblük duz dövrünü istifadə etdiyini sübut edir. | Grafana-in duz vidcetini sirrlər zirvəsi dövrü ilə müqayisə edin və sxem fərqlərinin `status:"accepted"` annotasiyasını saxlamasını təmin edin. |

  Yuxarıdakı cədvəldə hər hansı bir giriş `status`-i aşağı salırsa, fərq artefakt olmalıdır
  bərpa edilmiş **və** `telemetry_schema_diff.md` AND7-dən əvvəl yeniləndi
  idarəetmə paketi yayılır. Yenilənmiş JSON-u daxil edin
  `docs/source/sdk/android/readiness/schema_diffs/` və onu linkdən əlaqələndirin
  təkrar işə səbəb olan insident, xaos laboratoriyası və ya aktivləşdirmə hesabatı.
- **CI/vahid əhatə dairəsi:** `ci/run_android_tests.sh` əvvəl keçməlidir
  nəşriyyat quruluşları; paket həyata keçirməklə hashing/dəyişmə davranışını tətbiq edir
  nümunə yükləri ilə telemetriya ixracatçıları.
- **Enjektorun ağlı başında olma yoxlanışı:** İstifadə edin
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` məşqlərdən əvvəl
  uğursuz enjeksiyon işlərini təsdiq etmək və mühafizəçilərin hashing zamanı yanğın xəbərdarlığı
  yıxılırlar. Həmişə bir dəfə doğrulama ilə injektoru `--clear` ilə təmizləyin
  tamamlayır.

### 2.3 Mobil ↔ Rust telemetriya pariteti yoxlama siyahısı

Android ixracatçıları və Rust node xidmətlərini uyğunlaşdırarkən
sənədləşdirilmiş müxtəlif redaktə tələbləri
`docs/source/sdk/android/telemetry_redaction.md`. Aşağıdakı cədvəl kimi xidmət edir
AND7 yol xəritəsi girişində istinad edilən ikili icazəli siyahı - istənilən vaxt onu yeniləyin
schema diff sahələri təqdim edir və ya silir.| Kateqoriya | Android ixracatçıları | Rust xidmətləri | Doğrulama çəngəl |
|----------|-------------------|---------------|-----------------|
| Səlahiyyət / marşrut konteksti | Blake2b-256 vasitəsilə `authority`/`alias` hash edin və ixracdan əvvəl xam Torii host adlarını buraxın; duz fırlanmasını sübut etmək üçün `android.telemetry.redaction.salt_version` buraxın. | Korrelyasiya üçün tam Torii host adlarını və peer ID-lərini buraxın. | `readiness/schema_diffs/` altında ən son diaqram fərqindəki `android.torii.http.request` ilə `torii.http.request` qeydlərini müqayisə edin, sonra `scripts/telemetry/check_redaction_status.py` işlətməklə `android.telemetry.redaction.salt_version`-in klaster duzuna uyğun olduğunu təsdiqləyin. |
| Cihaz və imzalayan şəxsiyyəti | Bucket `hardware_tier`/`device_profile`, hash nəzarətçi ləqəbləri və heç vaxt seriya nömrələrini ixrac etməyin. | Cihaz metadatası yoxdur; qovşaqlar validator `peer_id` və nəzarətçi `public_key` hərfi yayır. | `docs/source/sdk/mobile_device_profile_alignment.md`-də xəritələri əks etdirin, laboratoriyalar zamanı `PendingQueueInspector` çıxışlarını yoxlayın və `ci/run_android_tests.sh` daxilindəki ləqəb hashing testlərinin yaşıl qalmasını təmin edin. |
| Şəbəkə metadata | Yalnız `network_type` + `roaming` booleanları ixrac edin; `carrier_name` buraxıldı. | Rust həmyaşıd host adlarını və tam TLS son nöqtə metadatasını saxlayır. | Ən son fərq JSON-u `readiness/schema_diffs/`-də saxlayın və Android tərəfinin hələ də `carrier_name`-i buraxdığını təsdiqləyin. Grafana-in “Şəbəkə Konteksti” vidceti hər hansı daşıyıcı sətirləri göstərərsə xəbərdar olun. |
| Override / xaos sübut | Maskalı aktyor rolları ilə `android.telemetry.redaction.override` və `android.telemetry.chaos.scenario` hadisələrini yayımlayın. | Rust xidmətləri rolu maskalamadan və xaosa xüsusi aralıqlar olmadan ləğvetmə təsdiqlərini verir. | Hər məşqdən sonra `docs/source/sdk/android/readiness/and7_operator_enablement.md`-i çarpaz yoxlayın ki, maskalanmamış Rust hadisələri ilə yanaşı ləğv edilən tokenlər və xaos artefaktları arxivləşsin. |

Paritet iş prosesi:

1. Hər manifest və ya ixracatçı dəyişikliyindən sonra işə salın
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   beləliklə, JSON artefaktı və əks olunmuş ölçülər dəlil paketinə düşür
   (köməkçi hələ də standart olaraq `artifacts/android/telemetry/schema_diff.prom` yazır).
2. Yuxarıdakı cədvəllə müqayisədə fərqi nəzərdən keçirin; əgər Android indi bir sahə yayırsa
   yalnız Rust-da icazə verilir (və ya əksinə), AND7 hazırlığı səhvini yazın və yeniləyin
   redaktə planı.
3. Həftəlik yoxlamalar zamanı qaçın
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   duz dövrlərinin Grafana vidcetinə uyğun olduğunu təsdiqləmək və epoxa qeyd etmək üçün
   çağırış jurnalı.
4. İstənilən deltaları qeyd edin
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` belə
   idarəetmə paritet qərarlarını yoxlaya bilər.

### 2.4 Müşahidə oluna bilən panellər və xəbərdarlıq hədləri

İdarə panellərini və xəbərdarlıqları AND7 sxemi fərq təsdiqləri ilə uyğunlaşdırın
`scripts/telemetry/check_redaction_status.py` çıxışını nəzərdən keçirərək:

- `Android Telemetry Redaction` — Duz dövrü vidceti, işarə ölçüsünü ləğv edin.
- `Redaction Compliance` — `android.telemetry.redaction.failure` sayğacı və
  injektor trend panelləri.
- `Exporter Health` — `android.telemetry.export.status` dərəcəsinin pozulması.
- `Android Telemetry Overview` — cihaz profili vedrələri və şəbəkə kontekstinin həcmi.

Aşağıdakı həddlər sürətli istinad kartını əks etdirir və tətbiq edilməlidir
hadisəyə reaksiya və məşqlər zamanı:| Metrik / panel | Həddi | Fəaliyyət |
|----------------|-----------|--------|
| `android.telemetry.redaction.failure` (`Redaction Compliance` lövhəsi) | >0 yuvarlanan 15 dəqiqəlik pəncərə üzərində | Uğursuz siqnalı araşdırın, injektoru təmizləyin, CLI çıxışı + Grafana ekran görüntüsünü qeyd edin. |
| `android.telemetry.redaction.salt_version` (`Android Telemetry Redaction` lövhəsi) | Sirləri-tonoz duz dövrü ilə fərqlənir | Relizləri dayandırın, sirlərin fırlanması ilə əlaqələndirin, fayl AND7 qeydi. |
| `android.telemetry.export.status{status="error"}` (`Exporter Health` lövhəsi) | >1% ixrac | Kollektorun sağlamlığını yoxlayın, CLI diaqnostikasını çəkin, SRE-ə yüksəldin. |
| `android.telemetry.device_profile{tier="enterprise"}` vs Rust pariteti (`Android Telemetry Overview`) | Dəyişiklik Rust bazasından >10% | Fayl idarəçiliyinə təqib edin, qurğu hovuzlarını yoxlayın, diaqram fərqi artefaktını şərh edin. |
| `android.telemetry.network_context` həcmi (`Android Telemetry Overview`) | Torii trafik mövcud olarkən sıfıra enir | `NetworkContextProvider` qeydiyyatını təsdiq edin, sahələrin dəyişməməsini təmin etmək üçün diff sxemini təkrar edin. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | Təsdiqlənmiş ləğv/qazma pəncərəsi xaricində sıfır olmayan | Tokeni insidentlə əlaqələndirin, həzmi bərpa edin, §3-də iş axını vasitəsilə ləğv edin. |

### 2.5 Operator hazırlığı və işə salınma izi

Yol xəritəsi bəndi AND7 xüsusi operator kurrikuluma çağırır, belə ki, dəstək, SRE və
buraxılış maraqlı tərəfləri runbook getməzdən əvvəl yuxarıdakı paritet cədvəllərini başa düşürlər
GA. Konturdan istifadə edin
Kanonik logistika üçün `docs/source/sdk/android/telemetry_readiness_outline.md`
(gündəm, aparıcılar, qrafik) və `docs/source/sdk/android/readiness/and7_operator_enablement.md`
ətraflı yoxlama siyahısı, sübut bağlantıları və fəaliyyət jurnalı üçün. Aşağıdakıları saxlayın
telemetriya planı dəyişdikdə fazalar sinxronlaşdırılır:| Faza | Təsvir | Sübut paketi | Əsas sahib |
|-------|-------------|-----------------|---------------|
| Əvvəlcədən oxunan paylama | Əvvəlcədən oxunmuş siyasəti, `telemetry_redaction.md` və sürətli arayış kartını brifinqdən ən azı beş iş günü əvvəl göndərin. Konturun ünsiyyət jurnalında təsdiqləri izləyin. | `docs/source/sdk/android/telemetry_readiness_outline.md` (Session Logistics + Communications Log) və `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`-də arxivləşdirilmiş e-poçt. | Sənədlər/Dəstək meneceri |
| Canlı hazırlıq sessiyası | 60 dəqiqəlik təlim keçirin (siyasətlə bağlı dərin dalış, runbook araşdırması, idarə panelləri, xaos laboratoriyası demosu) və asinxron izləyicilər üçün qeydi davam etdirin. | Qeyd + slaydlar konturun §2-də çəkilmiş istinadlarla `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` altında saxlanılır. | LLM (və AND7 sahibi fəaliyyət göstərir) |
| Xaos laboratoriya icrası | Canlı seansdan dərhal sonra `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md`-dən ən azı C2 (əsr etmə) + C6 (növbənin təkrarı) işə salın və aktivləşdirmə dəstinə qeydlər/skrinşotlar əlavə edin. | `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` və `/screenshots/<YYYY-MM>/` daxilində ssenari hesabatları və ekran görüntüləri. | Android Observability TL + SRE on-zəng |
| Bilik yoxlanışı və davamiyyət | Viktorina təqdimatlarını toplayın, <90% bal toplayan hər kəsi düzəldin və davamiyyət/viktorina statistikasını qeyd edin. Tez istinad suallarını paritet yoxlama siyahısına uyğun saxlayın. | `docs/source/sdk/android/readiness/forms/responses/`-də sorğu ixracı, `scripts/telemetry/generate_and7_quiz_summary.py` vasitəsilə hazırlanmış Markdown/JSON xülasəsi və `and7_operator_enablement.md` daxilində davamiyyət cədvəli. | Dəstək mühəndisliyi |
| Arxiv və təqiblər | Aktivləşdirmə dəstinin fəaliyyət jurnalını yeniləyin, artefaktları arxivə yükləyin və `status.md`-də tamamlanmanı qeyd edin. Sessiya zamanı buraxılmış istənilən remediasiya və ya ləğvetmə tokenləri `telemetry_override_log.md`-ə kopyalanmalıdır. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (fəaliyyət jurnalı), `.../archive/<YYYY-MM>/checklist.md` və §3-də istinad edilən ləğvetmə jurnalı. | LLM (və AND7 sahibi fəaliyyət göstərir) |

Kurrikuluma yenidən baxıldıqda (rüblük və ya əsas sxem dəyişikliklərindən əvvəl) yeniləyin
yeni sessiya tarixi ilə kontur, iştirakçı siyahısını cari saxlayın və
viktorina xülasəsini JSON/Markdown artefaktlarını bərpa edin ki, idarəetmə paketləri edə bilsin
ardıcıl sübutlara istinad edin. AND7 üçün `status.md` girişi ilə əlaqələndirilməlidir
hər aktivləşdirmə sprinti bağlandıqdan sonra ən son arxiv qovluğu.

### 2.6 Sxem fərqlərinin icazə siyahıları və siyasət yoxlamaları

Yol xəritəsi açıq şəkildə ikili icazəli siyahı siyasətini çağırır (mobil redaksiyalar vs
Pas saxlama) altında yerləşdirilən `telemetry-schema-diff` CLI tərəfindən həyata keçirilir
`tools/telemetry-schema-diff`. Qeydə alınmış hər bir fərq artefakt
`docs/source/sdk/android/readiness/schema_diffs/` hansı sahələrin olduğunu sənədləşdirməlidir
Android-də hashed/bucketed, Rust-da hansı sahələr hash edilməmiş qalır və olub-olmaması
icazə verilməyən hər hansı bir siqnal quruluşa sürüşdü. Bu qərarları qəbul edin
işlətməklə birbaşa JSON-da:

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```Yekun `jq` hesabat təmiz olduqda əməliyyatsız olaraq qiymətləndirilir. İstənilən çıxışı müalicə edin
bu əmrdən Sev2 hazırlığı səhvi kimi: məskunlaşmış `policy_violations`
massiv o deməkdir ki, CLI yalnız Android siyahısında olmayan bir siqnal aşkar edib
nə də sənədləşdirilmiş yalnız Rust istisna siyahısında
`docs/source/sdk/android/telemetry_schema_diff.md`. Bu baş verdikdə, dayanın
ixrac edin, AND7 bileti verin və fərqi yalnız siyasət modulundan sonra yenidən işə salın
və manifest snapshotları düzəldildi. Nəticədə JSON-u saxlayın
Tarix şəkilçisi və qeydi ilə `docs/source/sdk/android/readiness/schema_diffs/`
hadisə və ya laboratoriya hesabatı daxilindəki yol, beləliklə idarəetmə yoxlamaları təkrarlaya bilsin.

**Hashing və saxlama matrisi**

| Signal.field | Android ilə işləmə | Pasla mübarizə | Allowlist tag |
|-------------|-----------------|---------------|---------------|
| `torii.http.request.authority` | Blake2b-256 hashed (`representation: "blake2b_256"`) | İzləmə üçün sözlə saxlanılır | `policy_allowlisted` (mobil hash) |
| `attestation.result.alias` | Blake2b-256 hashed | Düz mətn ləqəbi (attestasiya arxivi) | `policy_allowlisted` |
| `attestation.result.device_tier` | Bucketed (`representation: "bucketed"`) | Düz səviyyəli simli | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | Yoxdur — Android ixracatçıları sahəni tamamilə tərk edirlər | Redaktə etmədən təqdim | `rust_only` (`telemetry_schema_diff.md` sənədinin 3-cü bəndində sənədləşdirilib) |
| `android.telemetry.redaction.override.*` | Maskalı aktyor rolları ilə yalnız Android siqnal | Ekvivalent siqnal buraxılmadı | `android_only` (`status:"accepted"` qalmalıdır) |

Yeni siqnallar görünəndə onları **və** sxemə fərq siyasəti moduluna əlavə edin
yuxarıdakı cədvəl, beləliklə runbook CLI-də göndərilən icra məntiqini əks etdirir.
Hər hansı bir Android siqnalı açıq `status`-i buraxırsa və ya
`policy_violations` massivi boş deyil, ona görə də bu yoxlama siyahısını aşağıdakılarla sinxronlaşdırın
`telemetry_schema_diff.md` §3 və burada istinad edilən ən son JSON snapshotları
`telemetry_redaction_minutes_*.md`.

## 3. İş axınını ləğv edin

Reqressiyaları və ya məxfiliyi hashing edərkən ləğvetmələr "şüşə sındırmaq" seçimidir
xəbərdarlıqlar müştəriləri bloklayır. Onları yalnız tam qərar yolunu qeyd etdikdən sonra tətbiq edin
hadisədə dok.1. **Drift və əhatə dairəsini təsdiqləyin.** PagerDuty xəbərdarlığını və ya diaqram fərqini gözləyin
   atəş qapısı, sonra qaç
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`-dən
   uyğunsuz səlahiyyətləri sübut edin. CLI çıxışını və Grafana ekran görüntülərini əlavə edin
   hadisə qeydinə.
2. **İmzalanmış sorğu hazırlayın.** Doldurun
   Bilet id ilə `docs/examples/android_override_request.json`, sorğuçu,
   istifadə müddəti və əsaslandırma. Faylı hadisə artefaktlarının yanında saxlayın
   uyğunluq girişləri yoxlaya bilər.
3. **Qeydiyyatı buraxın.** Çağırın
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   Köməkçi ləğv işarəsini çap edir, manifest yazır və cərgə əlavə edir
   Markdown audit jurnalına. Çatda nişanı heç vaxt yerləşdirməyin; birbaşa çatdırın
   ləğvetməni tətbiq edən Torii operatorlarına.
4. **Effektə nəzarət edin.** Beş dəqiqə ərzində təkini yoxlayın
   `android.telemetry.redaction.override` hadisə buraxıldı, kollektor
   statusun son nöqtəsi `override_active=true`-i göstərir və insident sənədi bunları siyahıya alır
   bitmə. Android Telemetriya Baxış panelinin “Tokenləri ləğv et
   eyni üçün aktiv” paneli (`android_telemetry_override_tokens_active`).
   token sayın və hər 10 dəqiqədən bir CLI statusunu işə salmağa davam edin
   hashing stabilləşdirir.
5. **Ləğv edin və arxivləşdirin.** Yumşaldılma yeri düşən kimi qaçın
  `scripts/android_override_tool.sh revoke --token <token>` belə ki, audit jurnalı
  ləğvetmə vaxtını tutur, sonra icra edir
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  idarəetmənin gözlədiyi sanitarlaşdırılmış görüntünü yeniləmək üçün. əlavə edin
  manifest, digest JSON, CLI transkriptləri, Grafana anlıq görüntüləri və NDJSON jurnalı
  `--event-log` vasitəsilə istehsal olunur
  `docs/source/sdk/android/readiness/screenshots/<date>/` və çarpaz bağlayın
  `docs/source/sdk/android/telemetry_override_log.md`-dən giriş.

24 saatı aşan ləğvetmələr SRE Direktoru və Uyğunluq təsdiqini tələb edir və
növbəti həftəlik AND7 icmalında vurğulanmalıdır.

### 3.1 Eskalasiya matrisini ləğv edin

| Vəziyyət | Maksimum müddət | Təsdiq edənlər | Tələb olunan bildirişlər |
|-----------|--------------|-----------|------------------------|
| Tək kirayəçi araşdırması (hashed səlahiyyət uyğunsuzluğu, müştəri Sev2) | 4 saat | Dəstək mühəndisi + Zəng üzrə SRE | Bilet `SUP-OVR-<id>`, `android.telemetry.redaction.override` hadisə, insident qeydi |
| Donanma miqyasında telemetriya kəsilməsi və ya SRE tələb olunan reproduksiya | 24 saat | Zəng üzrə SRE + Proqram Rəhbəri | PagerDuty qeydi, log girişini ləğv edin, `status.md`-də yeniləmə |
| Uyğunluq/məhkəmə sorğusu və ya 24 saatı aşan hər hansı bir hal | Açıqca ləğv edilənə qədər | SRE Direktoru + Uyğunluq üzrə aparıcı | İdarəetmə poçt siyahısı, ləğv jurnalı, AND7 həftəlik status |

#### Rol öhdəlikləri| Rol | Məsuliyyətlər | SLA / Qeydlər |
|------|------------------|-------------|
| Zəng üzrə Android telemetriyası (Hadisə Komandiri) | Sürücünün aşkarlanması, ləğvetmə alətini yerinə yetirin, insident sənədində təsdiqləri qeyd edin və ləğvetmənin müddəti bitmədən baş verdiyinə əmin olun. | 5 dəqiqə ərzində PagerDuty-ni qəbul edin və hər 15 dəqiqədən bir tərəqqi qeyd edin. |
| Android Observability TL (Haruka Yamamoto) | Drift siqnalını yoxlayın, ixracatçı/kollektor vəziyyətini təsdiqləyin və operatorlara təhvil verilməzdən əvvəl ləğvetmə manifestində imzalayın. | 10 dəqiqə ərzində körpüyə qoşulun; əlçatan olmadıqda səhnələşdirmə klasterinin sahibinə həvalə edin. |
| SRE əlaqəsi (Liam O'Connor) | Manifesti kollektorlara tətbiq edin, geridə qalan işlərə nəzarət edin və Torii tərəfdən azaldılması üçün Release Engineering ilə əlaqələndirin. | Dəyişiklik sorğusunda hər bir `kubectl` hərəkətini qeyd edin və hadisə sənədinə əmr transkriptlərini yapışdırın. |
| Uyğunluq (Sofiya Martins / Daniel Park) | 30 dəqiqədən çox olan ləğvetmələri təsdiqləyin, audit jurnalının sırasını yoxlayın və tənzimləyici/müştəri mesajlaşması ilə bağlı məsləhət verin. | `#compliance-alerts`-də təsdiq göndərin; istehsal hadisələri üçün ləğv edilməzdən əvvəl uyğunluq qeydi təqdim edin. |
| Sənədlər/Dəstək Meneceri (Priya Deshpande) | `docs/source/sdk/android/readiness/…` altında arxiv manifestləri/CLI çıxışı edin, ləğvetmə jurnalını səliqəli saxlayın və boşluqlar yaranarsa, təqib laboratoriyalarını planlaşdırın. | Hadisəni bağlamazdan əvvəl sübutların saxlanmasını (13 ay) və faylları AND7 təqiblərini təsdiqləyir. |

Hər hansı ləğvetmə nişanı müddəti a olmadan yaxınlaşarsa, dərhal artırın
sənədləşdirilmiş ləğv planı.

## 4. Hadisəyə Cavab

- **Xəbərdarlıqlar:** PagerDuty xidməti `android-telemetry-primary` redaktəni əhatə edir
  uğursuzluqlar, ixracatçıların kəsilməsi və vedrə sürüşməsi. SLA pəncərələrində təsdiq edin
  (dəstək kitabçasına baxın).
- **Diaqnostika:** Toplama üçün `scripts/telemetry/check_redaction_status.py`-i işə salın
  cari ixracatçı sağlamlığı, son xəbərdarlıqlar və hashed səlahiyyət göstəriciləri. Daxil et
  hadisə qrafikində çıxış (`incident/YYYY-MM-DD-android-telemetry.md`).
- ** İdarə panelləri:** Android Telemetriya Redaksiyasına, Android Telemetriyasına nəzarət edin
  İcmal, Redaksiya Uyğunluğu və İxracatçı Sağlamlığının idarə panelləri. Tutmaq
  hadisə qeydləri üçün skrinşotlar və hər hansı duz-versiyaya və ya ləğv etməyə şərh əlavə edin
  hadisəni bağlamadan əvvəl işarə sapmaları.
- **Koordinasiya:** İxracatçı məsələləri, Uyğunluq üçün Release Engineering ilə məşğul olun
  ləğvetmə/PII sualları və Sev 1 hadisələri üçün Proqram Rəhbəri.

### 4.1 Eskalasiya axını

Android insidentləri Android ilə eyni şiddət səviyyələrindən istifadə etməklə sınaqdan keçirilir
Dəstək Playbook (§2.1). Aşağıdakı cədvəl kimin və necə səhifələnməli olduğunu ümumiləşdirir
Hər cavab verənin körpüyə sürətlə qoşulması gözlənilir.| Ciddilik | Təsir | İlkin cavab verən (≤5 dəq) | İkinci dərəcəli eskalasiya (≤10dəq) | Əlavə bildirişlər | Qeydlər |
|----------|--------|----------------------------|--------------------------------|--------------------------|-------|
| Sev1 | Müştəri ilə üzləşən kəsinti, məxfiliyin pozulması və ya məlumat sızması | Zəng üzrə Android telemetriyası (`android-telemetry-primary`) | Torii zəng + Proqram Rəhbəri | Uyğunluq + SRE İdarəetmə (`#sre-governance`), klaster sahibləri (`#android-staging`) | Müharibə otağına dərhal başlayın və əmr qeydləri üçün ortaq sənəd açın. |
| Sev2 | Donanmanın deqradasiyası, sui-istifadənin qarşısının alınması və ya uzun müddət təkrar oynatma gecikməsi | Zəng üzrə Android telemetriyası | Android Foundations TL + Sənədlər/Dəstək Meneceri | Proqram Rəhbəri, Release Engineering əlaqə | Ləğv etmələr 24 saatı keçərsə, Uyğunluğa yüksəldin. |
| Sev3 | Tək kirayəçi məsələsi, laboratoriya məşqi və ya məsləhət xəbərdarlığı | Dəstək mühəndisi | Zəng zamanı Android (isteğe bağlı) | Sənədlər/maarifləndirmə üçün dəstək | Əhatə dairəsi genişlənərsə və ya bir neçə kirayəçi təsir edərsə, Sev2-yə çevirin. |

| Pəncərə | Fəaliyyət | Sahib(lər) | Sübut/Qeydlər |
|--------|--------|----------|----------------|
| 0-5dəq | PagerDuty-ni qəbul edin, insident komandiri (IC) təyin edin və `incident/YYYY-MM-DD-android-telemetry.md` yaradın. Linki və `#android-sdk-support`-də bir sətirli statusu buraxın. | Zəng üzrə SRE / Dəstək mühəndisi | Digər insident qeydləri ilə yanaşı edilən PagerDuty ack + insident stubunun skrinşotu. |
| 5-15 dəq | `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`-i işə salın və xülasəni hadisə sənədinə yapışdırın. Ping Android Observability TL (Haruka Yamamoto) və Dəstək aparıcısı (Priya Deshpande). | IC + Android Observability TL | CLI çıxışı JSON-nu əlavə edin, idarə paneli URL-lərinin açıldığını qeyd edin və diaqnostikanın kimə məxsus olduğunu qeyd edin. |
| 15-25 dəq | `android-telemetry-stg`-də reproduksiya etmək üçün quruluşçu klaster sahiblərini (müşahidə üçün Haruka Yamamoto, SRE üçün Liam O'Connor) cəlb edin. Semptom paritetini təsdiqləmək üçün `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` ilə toxum yükləyin və Pixel + emulyatorundan növbə boşluqlarını çəkin. | Səhnələşdirmə klaster sahibləri | Sanitasiya edilmiş `pending.queue` + `PendingQueueInspector` çıxışını hadisə qovluğuna yükləyin. |
| 25–40 dəq | Ləğv etmələrə, Torii azaltmağa və ya StrongBox geri qaytarılmasına qərar verin. Əgər PII ifşası və ya qeyri-deterministik heshing şübhələnirsə, `#compliance-alerts` vasitəsilə Uyğunluğu (Sofia Martins, Daniel Park) səhifəsini açın və eyni hadisə başlığında Proqram Rəhbərini xəbərdar edin. | IC + Uyğunluq + Proqram Rəhbəri | Linki ləğv edən tokenlər, Norito manifestləri və təsdiq şərhləri. |
| ≥40dəq | 30 dəqiqəlik status yeniləmələrini təmin edin (PagerDuty qeydləri + `#android-sdk-support`). Əgər aktiv deyilsə, döyüş otağı körpüsünü planlaşdırın, azaldılmış ETA-nı sənədləşdirin və Release Engineering (Aleksey Morozov) kollektor/SDK artefaktlarını gəzdirmək üçün gözləmə rejimində olduğundan əmin olun. | IC | Zaman möhürlü yeniləmələr və hadisə faylında saxlanılan və növbəti həftəlik yeniləmə zamanı `status.md`-də ümumiləşdirilmiş qərar jurnalları. |- Bütün eskalasiyalar, Android Dəstək Kitabçasındakı “Sahibi / Növbəti yeniləmə vaxtı” cədvəlindən istifadə edərək hadisə sənədində əks etdirilməlidir.
- Əgər başqa bir hadisə artıq açıqdırsa, mövcud döyüş otağına qoşulun və yenisini yaratmaq əvəzinə Android kontekstini əlavə edin.
- Hadisə runbook boşluqlarına toxunduqda, AND7 JIRA epikində sonrakı tapşırıqlar yaradın və `telemetry-runbook` işarələyin.

## 5. Xaos və Hazırlıq Təlimləri

- Təfərrüatlı ssenariləri yerinə yetirin
  `docs/source/sdk/android/telemetry_chaos_checklist.md` rüblük və ondan əvvəl
  əsas buraxılışlar. Laboratoriya hesabat şablonu ilə nəticələri qeyd edin.
- Sübutları (skrinşotlar, qeydlər) altında saxlayın
  `docs/source/sdk/android/readiness/screenshots/`.
- `telemetry-lab` etiketi ilə AND7 eposunda remediasiya biletlərini izləyin.
- Ssenari xəritəsi: C1 (redaksiya xətası), C2 (əxz etmə), C3 (ixracatçının uğursuzluğu), C4
  (Drift konfiqurasiyası ilə `run_schema_diff.sh` istifadə edərək diaqram fərq qapısı), C5
  (`generate_android_load.sh` vasitəsilə səpilmiş cihaz profili əyri), C6 (Torii fasiləsi
  + növbə təkrarı), C7 (attestasiyadan imtina). Bu nömrələməni uyğunlaşdırın
  `telemetry_lab_01.md` və təlimlər əlavə edərkən xaos yoxlama siyahısı.

### 5.1 Redaction drift & override drill (C1/C2)

1. Vasitəsilə bir hashing uğursuzluğu daxil edin
   `scripts/telemetry/inject_redaction_failure.sh` və PagerDuty-ni gözləyin
   xəbərdarlıq (`android.telemetry.redaction.failure`). -dən CLI çıxışını çəkin
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` üçün
   hadisə rekordu.
2. `--clear` ilə nasazlığı aradan qaldırın və siqnalın aradan qaldırılmasını təsdiq edin
   10 dəqiqə; duz/səlahiyyət panellərinin Grafana ekran görüntülərini əlavə edin.
3. İstifadə edərək imzalanmış ləğv sorğusu yaradın
   `docs/examples/android_override_request.json`, ilə tətbiq edin
   `scripts/android_override_tool.sh apply` və xaric edilməmiş nümunəni yoxlayın
   mərhələlərdə ixracatçının faydalı yükünü yoxlamaq (arayın
   `android.telemetry.redaction.override`).
4. `scripts/android_override_tool.sh revoke --token <token>` ilə ləğvi ləğv edin,
   ləğvetmə nişanı hash və bilet istinadını əlavə edin
   `docs/source/sdk/android/telemetry_override_log.md` və JSON həzm edin
   `docs/source/sdk/android/readiness/override_logs/` altında. Bu bağlayır
   Xaos yoxlama siyahısında C2 ssenarisi və idarəetmə sübutlarını təzə saxlayır.

### 5.2 İxracatçının söndürülməsi və növbənin təkrar oxunması (C3/C6)1. Səhnə kollektorunu aşağı salın (`kubectl miqyası
   deploy/android-otel-collector --replicas=0`) ixracatçını simulyasiya etmək üçün
   qəhvəyi. Status CLI vasitəsilə bufer göstəricilərini izləyin və xəbərdarlıqların atəşə tutulmasını təsdiqləyin
   15 dəqiqə işarəsi.
2. Kollektoru bərpa edin, yığılmış vəsaitin boşaldılmasını təsdiqləyin və kollektor jurnalını arxivləşdirin
   təkrar oynatma tamamlanmasını göstərən parça.
3. Həm səhnələşdirmə Pixelində, həm də emulyatorda ScenarioC6: quraşdırın
   `examples/android/operator-console`, təyyarə rejimini dəyişin, demonu təqdim edin
   köçürmələr, sonra təyyarə rejimini söndürün və növbə dərinliyi ölçülərini izləyin.
4. Gözləyən hər növbəni çəkin (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :core:siniflər >/dev/null`), and run `java -cp build/classes
   org.hyperledger.iroha.android.tools.PendingQueueInspector --fayl
   /tmp/.queue --json > queue-replay-.json`. Deşifrə əlavə edin
   zərflər və hashləri laboratoriya jurnalına təkrarlayın.
5. Xaos hesabatını ixracatçının kəsilmə müddəti, əvvəl/sonra növbə dərinliyi ilə yeniləyin,
   və `android_sdk_offline_replay_errors`-in 0 qaldığını təsdiqləyir.

### 5.3 Staging klaster xaos skripti (android-telemetry-stg)

Səhnə klasterinin sahibləri Haruka Yamamoto (Android Observability TL) və Liam O'Connor
(SRE) hər dəfə məşq proqramı təyin edildikdə bu skriptə əməl edin. Ardıcıllığı saxlayır
iştirakçılar telemetriya xaosunun yoxlanış siyahısına uyğun olaraq buna zəmanət verirdilər
artefaktlar idarəetmə üçün ələ keçirilir.

**İştirakçılar**

| Rol | Məsuliyyətlər | Əlaqə |
|------|------------------|---------|
| Android zəngli IC | Qazmağı idarə edir, PagerDuty qeydlərini əlaqələndirir, əmrlər jurnalına sahibdir | PagerDuty `android-telemetry-primary`, `#android-sdk-support` |
| Staging klaster sahibləri (Haruka, Liam) | Qapı dəyişdirmə pəncərələri, `kubectl` əməliyyatları, snapshot klaster telemetriyası | `#android-staging` |
| Sənədlər/Dəstək meneceri (Priya) | Sübutları qeyd edin, laboratoriyaların yoxlama siyahısını izləyin, izləmə biletlərini dərc edin | `#docs-support` |

**Uçuşdan əvvəl koordinasiya**

- Təlimdən 48 saat əvvəl, planlaşdırılanları sadalayan dəyişiklik sorğusu göndərin
  ssenarilər (C1–C7) və klaster sahibləri üçün linki `#android-staging`-ə yapışdırın
  ziddiyyətli yerləşdirmələri blok edə bilər.
- Ən son `ClientConfig` hash və `kubectl toplayın --kontekst staging pods əldə edin
  -n android-telemetry-stg` çıxışı baza vəziyyətini təyin edin, sonra saxla
  hər ikisi `docs/source/sdk/android/readiness/labs/reports/<date>/` altında.
- Cihazın əhatə dairəsini (Pixel + emulator) təsdiqləyin və təmin edin
  `ci/run_android_tests.sh` laboratoriya zamanı istifadə olunan alətləri tərtib etdi
  (`PendingQueueInspector`, telemetriya injektorları).

**İcra nəzarət məntəqələri**

- `#android-sdk-support`-də “xaos başlanğıcı” elan edin, körpünün qeydinə başlayın,
  və `docs/source/sdk/android/telemetry_chaos_checklist.md`-i belə görünən saxlayın
  hər bir əmr katib üçün nəql edilmişdir.
- Səhnə sahibinin hər bir injektor hərəkətini əks etdirməsi (`kubectl scale`, ixracatçı)
  yenidən işə salınır, generatorları yükləyir) beləliklə, həm Müşahidə, həm də SRE addımı təsdiqləyir.
- `scripts/telemetry/check_redaction_status.py-dən çıxışı çəkin
  --status-url https://android-telemetry-stg/api/redaction/status` hər birindən sonra
  ssenari və hadisə sənədinə yapışdırın.

**Bərpa**- Bütün injektorlar təmizlənənə qədər körpünü tərk etməyin (`inject_redaction_failure.sh --clear`,
  `kubectl scale ... --replicas=1`) və Grafana idarə panelləri yaşıl statusu göstərir.
- Sənədlər/Dəstək arxivləri növbə zibilləri, CLI qeydləri və ekran görüntüləri
  `docs/source/sdk/android/readiness/screenshots/<date>/` və arxivi işarələyir
  dəyişiklik sorğusu bağlanmazdan əvvəl yoxlama siyahısı.
- İstənilən ssenari üçün `telemetry-chaos` etiketi ilə izləmə biletlərini qeyd edin
  uğursuz və ya gözlənilməz ölçülər çıxarın və onlara `status.md`-də istinad edin
  növbəti həftəlik baxış zamanı.

| Zaman | Fəaliyyət | Sahib(lər) | Artefakt |
|------|--------|----------|----------|
| T−30min | `android-telemetry-stg` sağlamlığını yoxlayın: `kubectl --context staging get pods -n android-telemetry-stg`, gözləyən təkmilləşdirmələri təsdiqləyin və kollektor versiyalarını qeyd edin. | Haruka | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20min | Toxum əsas yükü (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) və stdout-u ələ keçirin. | Liam | `readiness/labs/reports/<date>/load-generator.log` |
| T−15min | `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md`-i `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`-ə kopyalayın, yerinə yetiriləcək ssenariləri siyahıya salın (C1–C7) və yazıçılar təyin edin. | Priya Deshpande (Dəstək) | Məşq başlamazdan əvvəl qeydə alınan insident. |
| T−10min | Pixel + emulyatorunu onlayn olaraq təsdiqləyin, ən son SDK quraşdırılıb və `ci/run_android_tests.sh` `PendingQueueInspector`-i tərtib edib. | Haruka, Liam | `readiness/screenshots/<date>/device-checklist.png` |
| T−5min | Böyütmə körpüsünü başladın, ekran yazmağa başlayın və `#android-sdk-support`-də “xaos başlanğıcı” elan edin. | IC / Sənədlər / Dəstək | Səsyazma `readiness/archive/<month>/` altında saxlanıldı. |
| +0dəq | `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md`-dən seçilmiş ssenarini yerinə yetirin (adətən C2 + C6). Laboratoriya bələdçisini görünən saxlayın və baş verən kimi əmr çağırışlarını çağırın. | Haruka sürücülər, Liam güzgülər nəticələri | Real vaxtda hadisə faylına əlavə edilmiş qeydlər. |
| +15dəq | Metrikləri toplamaq üçün fasilə verin (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) və Grafana skrinşotlarını götürün. | Haruka | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25dəq | Bütün vurulmuş nasazlıqları bərpa edin (`inject_redaction_failure.sh --clear`, `kubectl scale ... --replicas=1`), növbələri təkrarlayın və xəbərdarlıqların bağlanmasını təsdiqləyin. | Liam | `readiness/labs/reports/<date>/recovery.log` |
| +35dəq | Məlumat: hadisə sənədini hər ssenari üzrə keçdi/uğursuz yeniləyin, təqibləri siyahıya salın və artefaktları git-ə itələyin. Arxiv yoxlama siyahısının tamamlana biləcəyi barədə Sənədlərə/Dəstəkə bildirin. | IC | Hadisə sənədi yeniləndi, `readiness/archive/<month>/checklist.md` işarələndi. |

- İxracatçılar sağlam olana və bütün xəbərdarlıqlar təmizlənənə qədər səhnə sahiblərini körpüdə saxlayın.
- Xam növbə zibillərini `docs/source/sdk/android/readiness/labs/reports/<date>/queues/`-də saxlayın və hadisə jurnalında onların hashlərinə istinad edin.
- Əgər ssenari uğursuz olarsa, dərhal `telemetry-chaos` etiketli JIRA bileti yaradın və onu `status.md` ilə çarpazlaşdırın.
- Avtomatlaşdırma köməkçisi: `ci/run_android_telemetry_chaos_prep.sh` yük generatorunu, status görüntülərini və növbə ixrac santexnikasını əhatə edir. Səhnə girişi mövcud olduqda `ANDROID_TELEMETRY_DRY_RUN=false` və `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (s. `ANDROID_PENDING_QUEUE_INSPECTOR=false`-dən yalnız JSON emissiyası atlanmalı olduqda istifadə edin (məsələn, JDK yoxdur). **Həmişə köməkçini işə salmazdan əvvəl gözlənilən duz identifikatorlarını ixrac edin** `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` və `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` parametrlərini təyin edin, beləliklə, çəkilmiş telemetriya Rust əsas xəttindən uzaqlaşarsa, daxil edilmiş `check_redaction_status.py` zəngləri tez uğursuz olur.

## 6. Sənədləşdirmə və Aktivləşdirmə- **Operatorun aktivləşdirilməsi dəsti:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  runbook, telemetriya siyasəti, laboratoriya təlimatı, arxiv yoxlama siyahısı və bilikləri əlaqələndirir
  tək AND7-yə hazır paketə yoxlayır. SRE hazırlayarkən ona istinad edin
  idarəetmə əvvəlcədən oxuyur və ya rüblük yeniləməni planlaşdırır.
- **Aktivləşdirmə seansları:** 60 dəqiqəlik aktivləşdirmə qeydi 2026-02-18 tarixində işləyir
  rüblük yeniləmələrlə. Materiallar altında yaşayır
  `docs/source/sdk/android/readiness/`.
- **Bilik yoxlanışı:** İşçilər hazırlıq forması vasitəsilə ≥90% bal toplamalıdırlar. Mağaza
  `docs/source/sdk/android/readiness/forms/responses/` ilə nəticələnir.
- **Yeniləmələr:** Telemetriya sxemləri, idarə panelləri və ya siyasətləri ləğv etdikdə
  dəyişdirin, bu runbook, dəstək kitabçası və `status.md`-i eyni şəkildə yeniləyin
  PR.
- **Həftəlik baxış:** Hər Rust buraxılış namizədindən sonra (və ya ən azı həftəlik) yoxlayın
  `java/iroha_android/README.md` və bu runbook hələ də cari avtomatlaşdırmanı əks etdirir,
  qurğunun fırlanma prosedurları və idarəetmə gözləntiləri. Nəzərdən keçirin
  `status.md` beləliklə, Foundations mərhələ auditi sənədlərin təzəliyini izləyə bilər.

## 7. StrongBox Attestasiya Qoşqu- **Məqsəd:** Cihazları təqdim etməzdən əvvəl aparat tərəfindən dəstəklənən attestasiya paketlərini yoxlayın
  StrongBox hovuzu (AND2/AND6). Qoşqu əldə edilmiş sertifikat zəncirlərini istehlak edir və onları yoxlayır
  istehsal kodunun icra etdiyi eyni siyasətdən istifadə edərək etibarlı köklərə qarşı.
- **İstinad:** Tam olaraq `docs/source/sdk/android/strongbox_attestation_harness_plan.md`-ə baxın
  capture API, ləqəb həyat dövrü, CI/Buildkite məftilləri və sahiblik matrisi. Bu plan kimi davranın
  yeni laboratoriya texniklərini işə qəbul edərkən və ya maliyyə/uyğunluq artefaktlarını yeniləyərkən həqiqət mənbəyi.
- **İş axını:**
  1. Cihazda attestasiya paketi toplayın (ləqəb, `challenge.hex` və `chain.pem`
     leaf→root order) seçin və onu iş stansiyasına köçürün.
  2. `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root  proqramını işə salın
     [--trust-root-dir ] --require-strongbox --çıxış ` müvafiq istifadə edərək
     Google/Samsung kökü (kataloqlar bütün satıcı paketlərini yükləməyə imkan verir).
  3. JSON xülasəsini xam attestasiya materialı ilə birlikdə arxivləşdirin
     `artifacts/android/attestation/<device-tag>/`.
- **Paket formatı:** `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` izləyin
  tələb olunan fayl düzeni üçün (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **Etibarlı köklər:** Təchizatçı tərəfindən təchiz edilmiş PEM-ləri cihazın laboratoriya sirləri mağazasından əldə edin; çox keçmək
  `--trust-root` arqumentləri və ya `--trust-root-dir` nöqtəsi olduqda ankerləri saxlayan kataloqa
  zəncir Google olmayan lövbərdə bitir.
- **CI qoşqu:** Arxivləşdirilmiş paketləri toplu yoxlamaq üçün `scripts/android_strongbox_attestation_ci.sh` istifadə edin
  laboratoriya maşınlarında və ya CI qaçış maşınlarında. Skript `artifacts/android/attestation/**`-i skan edir və onu işə salır
  yenilənmiş `result.json` yazı, sənədləşdirilmiş faylları ehtiva edən hər bir kataloq üçün qoşqu
  xülasələr yerindədir.
- **CI zolağı:** Yeni paketləri sinxronlaşdırdıqdan sonra müəyyən edilmiş Buildkite addımını yerinə yetirin
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  İş `scripts/android_strongbox_attestation_ci.sh` yerinə yetirir, ilə xülasə yaradır
  `scripts/android_strongbox_attestation_report.py`, hesabatı `artifacts/android_strongbox_attestation_report.txt`-ə yükləyir,
  və quruluşu `android-strongbox/report` kimi şərh edir. Hər hansı bir uğursuzluğu dərhal araşdırın və
  Quraşdırma URL-ni cihaz matrisindən əlaqələndirin.
- **Hesabat:** JSON çıxışını idarəetmə rəylərinə əlavə edin və cihazın matrisa girişini yeniləyin
  Attestasiya tarixi ilə `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.
- **Sınaq məşq:** Aparat mövcud olmadıqda, `scripts/android_generate_mock_attestation_bundles.sh`-i işə salın
  (`scripts/android_mock_attestation_der.py` istifadə edir) deterministik test paketləri və ortaq saxta kök yaratmaq üçün CI və sənədlər qoşqudan sona qədər istifadə edə bilər.
- **Koddaxili qoruyucu barmaqlıqlar:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests `boş və mübahisəli olanı əhatə edir
  attestasiya regenerasiyası (StrongBox/TEE metadata) və `android.keystore.attestation.failure` emissiyaları
  problem uyğunsuzluğunda, beləliklə, yeni paketləri göndərməzdən əvvəl keş/temetriya reqressiyaları tutulur.

## 8. Əlaqələr

- ** Zəng üzrə Mühəndisliyə Dəstək:** `#android-sdk-support`
- **SRE İdarəetmə:** `#sre-governance`
- **Sənədlər/Dəstək:** `#docs-support`
- **Eskalasiya Ağacı:** Android Dəstək Kitabına baxın §2.1

## 9. Problemlərin aradan qaldırılması ssenariləriYol xəritəsi bəndi AND7-P2 dəfələrlə səhifəni açan üç hadisə sinfini çağırır
Zəng zamanı Android: Torii/şəbəkə fasilələri, StrongBox sertifikatlaşdırma uğursuzluqları və
`iroha_config` manifest sürüşməsi. Sənəd verməzdən əvvəl müvafiq yoxlama siyahısından keçin
Sev1/2 təqibləri və sübutları `incident/<date>-android-*.md`-də arxivləşdirin.

### 9.1 Torii & Şəbəkə Taymoutları

**Siqnallar**

- `android_sdk_submission_latency`, `android_sdk_pending_queue_depth`-də xəbərdarlıqlar,
  `android_sdk_offline_replay_errors` və Torii `/v2/pipeline` xəta dərəcəsi.
- `operator-console` vidjetləri (nümunələr/android) dayanmış növbənin boşaldılmasını və ya
  təkrar cəhdlər eksponensial geriləmədə qalıb.

**Dərhal cavab**

1. PagerDuty (`android-networking`) ilə tanış olun və insident jurnalına başlayın.
2. Grafana anlıq görüntülərini (Təqdimat gecikməsi + növbənin dərinliyi) əhatə edən çəkin
   son 30 dəqiqə.
3. Cihaz qeydlərindən (`ConfigWatcher`) aktiv `ClientConfig` hashını qeyd edin
   yenidən yükləmə uğurlu və ya uğursuz olduqda manifest həzmini çap edir).

**Diaqnostika**

- **Növbə sağlamlığı:** Konfiqurasiya edilmiş növbə faylını bir quruluş cihazından və ya cihazdan çəkin
  emulator (`adb qabığı  pişik faylları/pending.queue> kimi işləyir
  /tmp/pending.queue`). ilə zərfləri deşifrə edin
  `OfflineSigningEnvelopeCodec` -də təsvir olunduğu kimi
  təsdiq etmək üçün `docs/source/sdk/android/offline_signing.md#4-queueing--replay`
  geriləmə operatorun gözləntilərinə uyğun gəlir. Şifrədən çıxarılan hashləri əlavə edin
  hadisə.
- **Hash inventar:** Növbə faylını endirdikdən sonra müfəttiş köməkçisini işə salın
  hadisə artefaktları üçün kanonik hashları/ləqəbləri tutmaq üçün:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  Hadisəyə `queue-inspector.json` və gözəl çap edilmiş stdout əlavə edin
  və Ssenari D üçün AND7 laboratoriya hesabatından əlaqələndirin.
- **Torii bağlantısı:** SDK-nı istisna etmək üçün HTTP nəqliyyat kəmərini yerli olaraq işə salın
  reqressiyalar: `ci/run_android_tests.sh` məşqləri
  `HttpClientTransportTests`, `HttpClientTransportHarnessTests` və
  `ToriiMockServerTests`. Buradakı uğursuzluqlar müştəri səhvini göstərir
  Torii kəsilməsi.
- **Xəta inyeksiya məşqi:** Səhnələndirici Pixel (StrongBox) və AOSP-də
  emulyator, gözləyən növbə artımını bərpa etmək üçün əlaqəni dəyişdirin:
  `adb shell cmd connectivity airplane-mode enable` → iki demo təqdim edin
  operator-konsol vasitəsilə əməliyyatlar → `adb shell cmd qoşulma təyyarə rejimi
  disable` → verify the queue drains and `android_sdk_offline_replay_errors`
  0 olaraq qalır. Təkrarlanan əməliyyatların qeyd heşlərini.
- **Xəbərdarlıq pariteti:** Həddini tənzimləyərkən və ya Torii dəyişikliyindən sonra yerinə yetirin
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` buna görə də Prometheus qaydaları qalır
  tablosuna uyğunlaşdırılmışdır.

**Bərpa**

1. Əgər Torii deqradasiyaya uğrayıbsa, Torii-i çağırın və təkrar oxumağa davam edin.
   `/v2/pipeline` trafiki qəbul etdikdən sonra növbə.
2. Təsirə məruz qalan müştəriləri yalnız imzalanmış `iroha_config` manifestləri vasitəsilə yenidən konfiqurasiya edin. The
   `ClientConfig` qaynar yenidən yüklənən müşahidəçi hadisədən əvvəl müvəffəqiyyət qeydi çıxarmalıdır
   bağlaya bilər.
3. Hadisəni təkrar oynatmadan əvvəl/sonra növbə ölçüsü və hashlərlə yeniləyin
   hər hansı kəsilmiş əməliyyatlar.

### 9.2 StrongBox və Attestasiya Uğursuzluqları

**Siqnallar**- `android_sdk_strongbox_success_rate` və ya haqqında xəbərdarlıqlar
  `android.keystore.attestation.failure`.
- `android.keystore.keygen` telemetriyası indi tələb olunanı qeyd edir
  `KeySecurityPreference` və istifadə olunan marşrut (`strongbox`, `hardware`,
  `software`) StrongBox üstünlükləri daxil olduqda `fallback=true` bayrağı ilə
  TEE/proqram təminatı. STRONGBOX_REQUIRED sorğular indi səssiz əvəzinə tez uğursuz olur
  TEE açarlarının qaytarılması.
- `KeySecurityPreference.STRONGBOX_ONLY` cihazlarına istinad edən dəstək biletləri
  proqram düymələrinə qayıdır.

**Dərhal cavab**

1. PagerDuty-ni (`android-crypto`) qəbul edin və təsirlənmiş ləqəb etiketini çəkin
   (duzlu hash) üstəgəl cihaz profil kovası.
2. Cihaz üçün attestasiya matrisi girişini yoxlayın
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` və
   son təsdiqlənmiş tarixi qeyd edin.

**Diaqnostika**

- **Paketin yoxlanılması:** Çalışın
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  nasazlığın cihazla bağlı olub-olmadığını təsdiqləmək üçün arxivləşdirilmiş attestasiyada
  səhv konfiqurasiya və ya siyasət dəyişikliyi. Yaradılmış `result.json`-i əlavə edin.
- **Çağırış bərpası:** Çağırışlar keşlənmir. Hər bir çağırış tələbi yenisini bərpa edir
  `(alias, challenge)` tərəfindən attestasiya və keşlər; problemsiz zənglər keşi təkrar istifadə edir. Dəstəklənmir
- **CI taraması:** Hər dəfə `scripts/android_strongbox_attestation_ci.sh` icra edin
  saxlanılan paket yenidən təsdiqlənir; bu, təqdim edilən sistemli problemlərdən qoruyur
  yeni etibar lövbərləri tərəfindən.
- **Cihaz qazması:** StrongBox olmadan (və ya emulyatoru məcbur etməklə) aparatda
  SDK-nı yalnız StrongBox tələb etmək üçün təyin edin, demo əməliyyatı təqdim edin və təsdiqləyin
  telemetriya ixracatçısı `android.keystore.attestation.failure` hadisəsini yayır
  gözlənilən səbəblə. təmin etmək üçün StrongBox-a malik Pixel-də təkrarlayın
  xoşbəxt yol yaşıl qalır.
- **SDK reqressiya yoxlanışı:** `ci/run_android_tests.sh`-i işə salın və ödəyin
  attestasiyaya yönəlmiş dəstlərə diqqət (`AndroidKeystoreBackendDetectionTests`,
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  Keş/çağrı ayırma üçün `KeystoreKeyProviderTests`). Buradakı uğursuzluqlar
  müştəri tərəfində reqressiyanı göstərir.

**Bərpa**

1. Təchizatçı sertifikatları fırladıbsa və ya
   cihaz bu yaxınlarda böyük bir OTA aldı.
2. Yenilənmiş paketi `artifacts/android/attestation/<device>/`-ə yükləyin və
   matris girişini yeni tarixlə yeniləyin.
3. Əgər StrongBox istehsalda əlçatmazdırsa, inverride iş prosesinə əməl edin
   Bölmə 3 və bərpa müddətini sənədləşdirin; uzunmüddətli yumşaldılmasını tələb edir
   cihazın dəyişdirilməsi və ya satıcının təmiri.

### 9.2a Deterministik İxracın Bərpası

- **Formatlar:** Cari ixraclar v3-dür (ixrac başına duz/qeydiyyat + Argon2id, kimi qeydə alınıb
- **Şifrə siyasəti:** v3 ≥12 simvoldan ibarət parol ifadələrini tətbiq edir. İstifadəçilər daha qısa təmin edərsə
  parol ifadələri, onlara uyğun parol ifadəsi ilə yenidən ixrac etməyi tapşırın; v0/v1 idxallarıdır
  azaddır, lakin idxaldan dərhal sonra v3 kimi yenidən paketlənməlidir.
- **Təyrimə/təkrar istifadə qoruyucuları:** Dekoderlər sıfır/qısa duz və ya uzunluqları rədd edir və təkrarlanır
  `salt/nonce reuse` xətaları kimi duz/qeyri-keçmiş cütlərin səthi. Təmizləmək üçün ixracı bərpa edin
  gözətçi; təkrar istifadə etməyə məcbur etməyin.
  Açarı yenidən nəmləndirmək üçün `SoftwareKeyProvider.importDeterministic(...)`
  `exportDeterministic(...)` v3 paketini yaymaq üçün masaüstü alətlər yeni KDF-ni qeyd edir
  parametrlər.### 9.3 Manifest və Konfiqurasiya Uyğunsuzluqları

**Siqnallar**

- `ClientConfig` yenidən yükləmə xətaları, uyğun olmayan Torii host adları və ya telemetriya
  AND7 fərq aləti ilə işarələnmiş diaqram fərqləri.
- Operatorlar eyni cihazlarda müxtəlif təkrar cəhd/geri söndürmə düymələri haqqında məlumat verir
  donanma.

**Dərhal cavab**

1. Android qeydlərində çap edilmiş `ClientConfig` həzmini çəkin və
   buraxılış manifestindən gözlənilən həzm.
2. Müqayisə üçün işləyən node konfiqurasiyasını boşaltın:
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

**Diaqnostika**

- **Sxem fərqi:** `scripts/telemetry/run_schema_diff.sh --android-config-i işə salın
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  Norito fərq hesabatını yaratmaq üçün Prometheus mətn faylını yeniləyin və əlavə edin
  JSON artefaktı plus hadisəyə dair göstəricilər sübutu və AND7 telemetriya hazırlığı jurnalı.
- **Manifest doğrulaması:** `iroha_cli runtime capabilities` (və ya iş vaxtı) istifadə edin
  audit əmri) qovşağın reklam edilmiş kripto/ABI hashlərini əldə etmək və təmin etmək
  onlar mobil manifestə uyğun gəlir. Uyğunsuzluq düyünün geri çəkildiyini təsdiqləyir
  Android manifestini yenidən nəşr etmədən.
- **SDK reqressiya yoxlanışı:** `ci/run_android_tests.sh` əhatə edir
  `ClientConfigNoritoRpcTests`, `ClientConfig.ValidationTests` və
  `HttpClientTransportStatusTests`. Uğursuzluqlar göndərilən SDK-nın edə bilməyəcəyini göstərir
  hazırda yerləşdirilmiş manifest formatını təhlil edin.

**Bərpa**

1. Manifesti icazə verilmiş boru kəməri vasitəsilə bərpa edin (adətən
   `iroha_cli runtime Capabilities` → imzalanmış Norito manifest → konfiqurasiya paketi) və
   onu operator kanalı vasitəsilə yenidən yerləşdirin. Heç vaxt `ClientConfig` redaktə etməyin
   cihazda üstünlük təşkil edir.
2. Düzəliş edilmiş manifest yerləşdikdən sonra `ConfigWatcher` “yenidən yüklə ok” yazısına baxın
   hər donanma səviyyəsində mesaj göndərin və hadisəni yalnız telemetriyadan sonra bağlayın
   schema diff hesabat pariteti.
3. Manifest hashını, diaqram fərqinin artefakt yolunu və insident bağlantısını qeyd edin
   Audit üçün Android bölməsi altında `status.md`.

## 10. Operatorun aktivləşdirilməsi üzrə kurikulum

Yol xəritəsi elementi **AND7** təkrarlanan təlim paketi tələb edir ki, operatorlar,
dəstək mühəndisləri və SRE telemetriya/redaksiya yeniləmələrini olmadan qəbul edə bilər
fərziyyə. Bu bölməni ilə cütləşdirin
`docs/source/sdk/android/readiness/and7_operator_enablement.md`, ehtiva edir
ətraflı yoxlama siyahısı və artefakt bağlantıları.

### 10.1 Sessiya modulları (60 dəqiqəlik brifinq)

1. **Telemetri arxitekturası (15 dəq).** İxracatçı buferində gəzin,
   redaksiya filtri və sxem fərqi alətləri. Demo
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` plus
   `scripts/telemetry/check_redaction_status.py` beləliklə iştirakçılar paritetin necə olduğunu görürlər
   icra olunur.
2. **Runbook + xaos laboratoriyaları (20 dəqiqə).** Bu runbook-un 2-9 bölmələrini vurğulayın,
   `readiness/labs/telemetry_lab_01.md`-dən bir ssenarini məşq edin və necə olduğunu göstərin
   artefaktları `readiness/labs/reports/<stamp>/` altında arxivləşdirmək.
3. **Qeyd et + uyğunluq iş axını (10 dəq).** Bölmə 3-ü ləğv edin,
   `scripts/android_override_tool.sh` nümayiş etdirin (tətbiq et/ləğv et/həzm et) və
   `docs/source/sdk/android/telemetry_override_log.md` və ən sonunu yeniləyin
   JSON-u həzm edin.
4. **Sual-Cavab/bilik yoxlanışı (15dəq).** Daxildə sürətli arayış kartından istifadə edin
   Sualları bağlamaq üçün `readiness/cards/telemetry_redaction_qrc.md`
   `readiness/and7_operator_enablement.md`-də izləmələri çəkin.### 10.2 Aktiv ritmi və sahibləri

| Aktiv | Kadans | Sahib(lər) | Arxiv yeri |
|-------|---------|----------|------------------|
| Qeydə alınmış keçid (Böyütmə/Komandalar) | Rüblük və ya hər duz fırlanmadan əvvəl | Android Observability TL + Sənədlər/Dəstək meneceri | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (qeydiyyat + yoxlama siyahısı) |
| Slayd göyərtəsi və sürətli istinad kartı | Siyasət/runbook dəyişdikdə yeniləyin | Sənədlər/Dəstək meneceri | `docs/source/sdk/android/readiness/deck/` və `/cards/` (ixrac PDF + Markdown) |
| Bilik yoxlanışı + davamiyyət vərəqi | Hər canlı sessiyadan sonra | Dəstək mühəndisliyi | `docs/source/sdk/android/readiness/forms/responses/` və `and7_operator_enablement.md` davamiyyət bloku |
| Sual və Cavabların geridə qalması / fəaliyyət jurnalı | yuvarlanan; hər seansdan sonra yenilənir | LLM (fəaliyyət göstərən DRI) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 Sübut və əks əlaqə dövrəsi

- Sessiya artefaktlarını (skrinşotlar, insident təlimləri, viktorina ixracı) mağazada saxlayın
  xaos sınaqları üçün istifadə edilən eyni tarixli kataloq beləliklə idarəetmə hər ikisini yoxlaya bilsin
  hazırlığı birlikdə izləyir.
- Sessiya başa çatdıqda, linklərlə `status.md` (Android bölməsi) yeniləyin
  arxiv kataloqu və hər hansı açıq təqibləri qeyd edin.
- Canlı Sual-Cavabdan çıxan suallar məsələlərə və ya sənədə çevrilməlidir
  bir həftə ərzində sorğuları çəkmək; yol xəritəsi dastanlarına (AND7/AND8) istinad edin
  bilet təsviri, belə ki, sahiblər uyğunlaşır.
- SRE sinxronizasiyası arxiv yoxlama siyahısını və sadalanan sxem fərqi artefaktını nəzərdən keçirir
  Bölmə 2.3 kurrikulumun rüb üçün bağlandığını elan etməzdən əvvəl.