---
lang: uz
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1c9ce5a256683152440986a028d1e57ed298bbbb189196af866128962228caa5
source_last_modified: "2026-01-05T09:28:11.847834+00:00"
translation_last_reviewed: 2026-02-07
title: Android Telemetry Redaction Plan
sidebar_label: Android Telemetry
slug: /sdks/android-telemetry
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android telemetriyasini tuzatish rejasi (AND7)

## Qo'llash doirasi

Ushbu hujjat taklif qilingan telemetriyani tahrirlash siyosati va faolligini qamrab oladi
**AND7** yoʻl xaritasi bandiga koʻra Android SDK uchun artefaktlar. U tekislanadi
hisobga olish paytida Rust tugunining asosiy chizig'i bilan mobil asboblar
qurilmaga xos maxfiylik kafolatlari. Chiqish uchun oldindan o'qish vazifasini bajaradi
Fevral 2026 SRE boshqaruvi sharhi.

Maqsadlar:

- Birgalikda kuzatilishi mumkin bo'lgan Android tomonidan chiqarilgan har bir signalni kataloglang
  backends (OpenTelemetry izlari, Norito kodlangan jurnallar, o'lchovlarni eksport qilish).
- Rust bazaviy va hujjat tahriridan farq qiluvchi maydonlarni tasniflash yoki
  ushlab turish nazorati.
- Yordam guruhlari javob berishi uchun faollashtirish va sinov ishlarini belgilang
  redaktsiya bilan bog'liq ogohlantirishlarga aniq.

## Signal inventarizatsiyasi (qoralama)

Kanal bo'yicha guruhlangan rejalashtirilgan asboblar. Barcha maydon nomlari Androidga mos keladi
SDK telemetriya sxemasi (`org.hyperledger.iroha.android.telemetry.*`). Ixtiyoriy
maydonlar `?` bilan belgilangan.

| Signal ID | Kanal | Asosiy maydonlar | PII/PHI tasnifi | Tahrirlash / Saqlash | Eslatmalar |
|----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Izlanish oralig'i | `authority_hash`, `route`, `status_code`, `latency_ms` | Hokimiyat ommaviydir; marshrut hech qanday sirni o'z ichiga olmaydi | Eksportdan oldin xeshlangan vakolatni (`blake2b_256`) chiqaring; 7 kun davomida saqlang | Mirrors Rust `torii.http.request`; xeshlash mobil taxallusning maxfiyligini ta'minlaydi. |
| `android.torii.http.retry` | Tadbir | `route`, `retry_count`, `error_code`, `backoff_ms` | Yo'q | Hech qanday tahrir yo'q; 30 kun saqlang | Deterministik qayta tekshiruvlar uchun foydalaniladi; Rust maydonlari bilan bir xil. |
| `android.pending_queue.depth` | O'lchov ko'rsatkichi | `queue_type`, `depth` | Yo'q | Hech qanday tahrir yo'q; 90 kun saqlang | Rust `pipeline.pending_queue_depth`ga mos keladi. |
| `android.keystore.attestation.result` | Tadbir | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Taxallus (oliy nom), qurilma metama'lumotlari | Taxallusni deterministik yorliq bilan almashtiring, tovar belgisini paqir | AND2 attestatsiyasiga tayyorligi uchun talab qilinadi; Zang tugunlari qurilma metama'lumotlarini chiqarmaydi. |
| `android.keystore.attestation.failure` | Hisoblagich | `alias_label`, `failure_reason` | Taxallus tahrirlanganidan keyin hech biri | Hech qanday tahrir yo'q; 90 kun saqlang | Xaos mashqlarini qo'llab-quvvatlaydi; alias_label xeshlangan taxallusdan olingan. |
| `android.telemetry.redaction.override` | Tadbir | `override_id`, `actor_role_masked`, `reason`, `expires_at` | Aktyor roli operativ PII | Dala eksporti niqoblangan rol toifasi; audit jurnali bilan 365 kun saqlang | Rustda mavjud emas; operatorlar qo'llab-quvvatlash orqali bekor qilishni fayl qilishlari kerak. |
| `android.telemetry.export.status` | Hisoblagich | `backend`, `status` | Yo'q | Hech qanday tahrir yo'q; 30 kun saqlang | Rust eksportchisi holati hisoblagichlari bilan tenglik. |
| `android.telemetry.redaction.failure` | Hisoblagich | `signal_id`, `reason` | Yo'q | Hech qanday tahrir yo'q; 30 kun saqlang | Rust `streaming_privacy_redaction_fail_total` aks ettirish uchun talab qilinadi. |
| `android.telemetry.device_profile` | O'lchagich | `profile_id`, `sdk_level`, `hardware_tier` | Qurilma metama'lumotlari | Dag'al chelaklarni chiqaradi (SDK asosiy, apparat darajasi); 30 kun saqlang | OEM xususiyatlarini oshkor qilmasdan paritet asboblar panelini yoqadi. |
| `android.telemetry.network_context` | Tadbir | `network_type`, `roaming` | Tashuvchi PII | bo'lishi mumkin `carrier_name`ni butunlay tashlab yuboring; boshqa maydonlarni 7 kun ushlab turing | `ClientConfig.networkContextProvider` tozalangan suratni taqdim etadi, shunda ilovalar abonent maʼlumotlarini oshkor qilmasdan tarmoq turini + roumingni chiqarishi mumkin; paritet asboblar paneli signalni Rust `peer_host` ning mobil analogi sifatida ko'radi. |
| `android.telemetry.config.reload` | Tadbir | `source`, `result`, `duration_ms` | Yo'q | Hech qanday tahrir yo'q; 30 kun saqlang | Mirrors Rust konfiguratsiyasini qayta yuklash oraliqlari. |
| `android.telemetry.chaos.scenario` | Tadbir | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | Qurilma profili chelaklangan | `device_profile` bilan bir xil; 30 kun saqlang | AND7 tayyorligi uchun zarur bo'lgan tartibsizlik mashqlari paytida qayd etilgan. |
| `android.telemetry.redaction.salt_version` | O'lchagich | `salt_epoch`, `rotation_id` | Yo'q | Hech qanday tahrir yo'q; 365 kun saqlang | Blake2b tuzining aylanishini kuzatadi; Android xesh davri Rust tugunlaridan ajralib chiqqanda paritet ogohlantirishi. |
| `android.crash.report.capture` | Tadbir | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Barmoq izi xatosi + metamaʼlumotlar jarayoni | `crash_id` xash, umumiy redaktsiya tuzi, paqir kuzatuv holati, eksportdan oldin stek ramkalarini tashlab yuborish; 30 kun saqlang | `ClientConfig.Builder.enableCrashTelemetryHandler()` chaqirilganda avtomatik ravishda yoqiladi; paritet asboblar panelini qurilmani identifikatsiya qiluvchi izlarni ochmasdan ta'minlaydi. |
| `android.crash.report.upload` | Hisoblagich | `crash_id`, `backend`, `status`, `retry_count` | Barmoq izi buzilishi | `crash_id` xeshini qayta ishlatish, faqat holatini chiqarish; 30 kun saqlang | `ClientConfig.crashTelemetryReporter()` yoki `CrashTelemetryHandler.recordUpload` orqali yuboring, shuning uchun yuklamalar boshqa telemetriya kabi bir xil Sigstore/OLTP kafolatlariga ega. |

### Amalga oshirish uchun ilgaklar

- `ClientConfig` endi manifest orqali olingan telemetriya ma'lumotlarini uzatadi.
  `setTelemetryOptions(...)`/`setTelemetrySink(...)`, avtomatik ro'yxatdan o'tish
  `TelemetryObserver` shunday xeshlangan vakolatlar va tuz ko'rsatkichlari maxsus kuzatuvchilarsiz oqadi.
  Qarang: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  va ostidagi hamrohlik sinflari
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Ilovalar qo'ng'iroq qilishlari mumkin
  Ro'yxatdan o'tish uchun `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)`
  aks ettirishga asoslangan `AndroidNetworkContextProvider`, ish vaqtida `ConnectivityManager` so'rovi
  va kompilyatsiya vaqti Androidni kiritmasdan `android.telemetry.network_context` hodisasini chiqaradi
  bog'liqliklar.
- `TelemetryOptionsTests` va `TelemetryObserverTests` birlik sinovlari
  (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) xeshni himoya qiladi
  yordamchilar va ClientConfig integratsiya kancasi, shuning uchun regressiyalar darhol yuzaga chiqadi.
- Faollashtirish to'plami/laboratoriyalari endi psevdokod o'rniga aniq API-larni keltirib, ushbu hujjatni va
  yuk tashish SDK bilan moslashtirilgan runbook.

> **Operatsiyalar haqida eslatma:** egasi/holat ish varag'i yashaydi
> `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` va bo'lishi kerak
> har bir AND7 nazorat punktida ushbu jadval bilan birga yangilanadi.

## Paritetga ruxsat berilgan roʻyxatlar va sxema-diff ish jarayoni

Boshqaruv ikkita ruxsat etilgan roʻyxatni talab qiladi, shuning uchun Android eksporti hech qachon identifikatorlarni sizdirmaydi
bu Rust xizmatlari qasddan yuzaga. Ushbu bo'lim runbook yozuvini aks ettiradi
(`docs/source/android_runbook.md` §2.3) lekin AND7 redaktsiya rejasini saqlaydi
o'z-o'zidan.

| Kategoriya | Android eksportchilari | Rust xizmatlari | Tasdiqlash kancasi |
|----------|-------------------|---------------|-----------------|
| Vakolat / marshrut konteksti | Blake2b-256 orqali `authority`/`alias` xash va eksportdan oldin xom Torii xost nomlarini tashlang; tuz aylanishini isbotlash uchun `android.telemetry.redaction.salt_version` chiqaradi. | Korrelyatsiya uchun to'liq Torii xost nomlari va tengdosh identifikatorlarini chiqaring. | `docs/source/sdk/android/readiness/schema_diffs/` ostidagi so'nggi sxema farqidagi `android.torii.http.request` va `torii.http.request` yozuvlarini solishtiring, so'ngra tuz davrlarini tasdiqlash uchun `scripts/telemetry/check_redaction_status.py` ni ishga tushiring. |
| Qurilma va imzolovchi identifikatori | Paqir `hardware_tier`/`device_profile`, xesh-kontroller taxalluslari va hech qachon seriya raqamlarini eksport qilmang. | Emit validator `peer_id`, kontroller `public_key` va navbat xeshlarini so'zma-so'z. | `docs/source/sdk/mobile_device_profile_alignment.md` bilan moslang, `java/iroha_android/run_tests.sh` ichidagi taxallusni xeshlash testlarini o'tkazing va laboratoriya mashg'ulotlarida navbat-inspektor natijalarini arxivlang. |
| Tarmoq metama'lumotlari | Faqat eksport qilish `network_type` + `roaming`; `carrier_name` tushiring. | Tengdosh host nomi/TLS so‘nggi nuqta metama’lumotlarini saqlang. | Har bir sxema farqini `readiness/schema_diffs/` da saqlang va Grafana ning “Tarmoq konteksti” vidjetida tashuvchi satrlari ko‘rsatilsa, ogohlantiring. |
| bekor qilish / tartibsizlik dalillar | Emit `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` niqobli aktyor rollari bilan. | Niqobsiz bekor qilish tasdiqlarini chiqarish; betartiblikka xos oraliqlar yo'q. | Mashqlardan so'ng `docs/source/sdk/android/readiness/and7_operator_enablement.md` ni o'zaro tekshirib ko'ring, tokenlarni bekor qilish + tartibsizlik artefaktlari maskalanmagan Rust hodisalari bilan bir qatorda joylashishiga ishonch hosil qiling. |

Ish jarayoni:

1. Har bir manifest/eksportchi o'zgarishidan keyin ishga tushiring
   `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` va JSONni `docs/source/sdk/android/readiness/schema_diffs/` ostiga joylashtiring.
2. Yuqoridagi jadvaldagi farqni ko'rib chiqing. Agar Android faqat Rust maydonini chiqarsa
   (yoki aksincha), AND7 tayyorligi xatosini yozing va ushbu rejani ham, rejani ham yangilang
   runbook.
3. Haftalik operatsiyalarni ko'rib chiqish paytida, bajaring
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   va tayyorlik ish varag'ida tuz davri plus sxemasi-farq vaqt tamg'asini qayd qiling.
4. `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` da har qanday og'ishlarni yozib oling
   shuning uchun boshqaruv paketlari paritet qarorlarni qabul qiladi.

> **Sxemaga havola:** kanonik maydon identifikatorlari kelib chiqadi
> `android_telemetry_redaction.proto` (Android SDK yaratish jarayonida materiallashtirilgan
> Norito deskriptorlari bilan birga). Sxema `authority_hash` ni ochib beradi,
> `alias_label`, `attestation_digest`, `device_brand_bucket` va
> `actor_role_masked` maydonlari endi SDK va telemetriya eksportchilarida ishlatiladi.

`authority_hash` qayd etilgan Torii vakolat qiymatining 32 baytlik qattiq dayjestidir.
protoda. `attestation_digest` kanonik attestatsiya bayonotini oladi
barmoq izi, `device_brand_bucket` esa Android brendining xom ashyo qatorini
tasdiqlangan raqam (`generic`, `oem`, `enterprise`). `actor_role_masked` tashiydi
o'rniga redaktsiyani bekor qiluvchi aktyor toifasi (`support`, `sre`, `audit`)
xom foydalanuvchi identifikatori.

### Telemetriyani eksportga moslashtirish buzilishiCrash telemetry endi OpenTelemetry eksportchilari va kelib chiqishi bilan bir xil
quvur liniyasi Torii tarmoq signallari sifatida boshqaruv kuzatuvini yopadi.
takroriy eksportchilar. Avariyaga ishlov beruvchi `android.crash.report.capture` ni ta'minlaydi
`crash_id` xeshli hodisa (Blake2b-256 allaqachon redaktsiya tuzidan foydalangan holda)
`android.telemetry.redaction.salt_version` tomonidan kuzatilgan), jarayon holati paqirlari,
va sanitarizatsiya qilingan ANR kuzatuvchisi metama'lumotlari. Stack izlari qurilmada qoladi va faqat
oldin `has_native_trace` va `anr_watchdog_bucket` maydonlariga jamlangan
eksport qiling, shuning uchun PII yoki OEM satrlari qurilmadan chiqmaydi.

Nosozlikni yuklash `android.crash.report.upload` hisoblagich yozuvini yaratadi,
SRE-ga hech narsa o'rganmasdan backend ishonchliligini tekshirish imkonini beradi
foydalanuvchi yoki stek izi. Ikkala signal ham Torii eksportchisini qayta ishlatganligi sababli ular meros oladi
bir xil Sigstore imzolash, saqlash siyosati va ogohlantirish ilgaklari allaqachon belgilangan
AND7 uchun. Shuning uchun qo'llab-quvvatlovchi runbooks xeshlangan nosozlik identifikatorini o'zaro bog'lashi mumkin
Android va Rust dalillar to'plamlari o'rtasida buyurtma bo'yicha halokatli quvur liniyasisiz.

Ishlovchini `ClientConfig.Builder.enableCrashTelemetryHandler()` orqali bir marta yoqing
telemetriya imkoniyatlari va lavabolar sozlangan; halokatli yuklash ko'priklari qayta ishlatilishi mumkin
`ClientConfig.crashTelemetryReporter()` (yoki `CrashTelemetryHandler.recordUpload`)
bir xil imzolangan quvur liniyasida backend natijalarini chiqarish.

## Siyosat Deltalari va Rust Baseline

Android va Rust telemetriya siyosatlari oʻrtasidagi farqlarni yumshatish bosqichlari bilan.

| Kategoriya | Rust Baseline | Android siyosati | Yumshatish / Tasdiqlash |
|----------|---------------|----------------|-------------------------|
| Vakolat / tengdosh identifikatorlari | Oddiy hokimiyat satrlari | `authority_hash` (Blake2b-256, aylantirilgan tuz) | `iroha_config.telemetry.redaction_salt` orqali chop etilgan umumiy tuz; paritet testi qo'llab-quvvatlash xodimlari uchun teskari xaritalashni ta'minlaydi. |
| Xost / tarmoq metama'lumotlari | Eksport qilingan tugun xost nomlari/IP-lar | Tarmoq turi + faqat rouming | Xost nomlari o‘rniga mavjudlik toifalaridan foydalanish uchun tarmoq sog‘lig‘i boshqaruv paneli yangilandi. |
| Qurilma xususiyatlari | Yo'q (server tomonida) | Chelaklangan profil (SDK 21/23/29+, `emulator`/`consumer`/`enterprise`) | Xaos mashqlari chelak xaritasini tasdiqlaydi; nozikroq tafsilotlar kerak bo'lganda, runbook hujjatlarini ko'tarish yo'lini qo'llab-quvvatlang. |
| Tahrirlash bekor qilinadi | Qo'llab-quvvatlanmaydi | Norito kitobida saqlangan qo'lda bekor qilish tokeni (`actor_role_masked`, `reason`) | Qayta belgilash imzolangan so'rovni talab qiladi; audit jurnali 1 yil saqlanadi. |
| Attestatsiya izlari | Faqat SRE orqali server attestatsiyasi | SDK sanitarlashtirilgan attestatsiya xulosasini chiqaradi | Rust attestatsiya validatoriga qarshi attestatsiya xeshlarini o‘zaro tekshirish; xeshlangan taxallus oqishning oldini oladi. |

Tekshirish ro'yxati:

- Oldin xeshlangan/maskelangan maydonlarni tekshiradigan har bir signal uchun redaktsiya birligi sinovlari
  eksportchi taqdimnomasi.
- Sxema farqlash vositasi (Rust tugunlari bilan birgalikda) maydonlar paritetini tasdiqlash uchun har kecha ishlaydi.
- Chaos mashqlari skripti mashqlari ish jarayonini bekor qiladi va audit jurnalini tasdiqlaydi.

## Amalga oshirish vazifalari (SREdan oldingi boshqaruv)

1. **Inventarizatsiyani tasdiqlash** — Yuqoridagi jadvalni haqiqiy Android SDK bilan o‘zaro tekshirish
   asboblar kancalari va Norito sxema ta'riflari. Egalari: Android
   Kuzatish mumkinligi TL, LLM.
2. **Telemetriya sxemasi farqi** — Rust koʻrsatkichlariga nisbatan umumiy farq vositasini ishga tushiring.
   SRE tekshiruvi uchun paritet artefaktlarni ishlab chiqarish. Egasi: SRE maxfiylik yetakchisi.
3. **Runbook loyihasi (2026-02-03 tugallangan)** — `docs/source/android_runbook.md`
   endi end-to-end bekor qilish ish jarayonini hujjatlashtiradi (3-bo'lim) va kengaytirilgan
   eskalatsiya matritsasi va rol mas'uliyati (3.1-bo'lim), CLIni bog'lash
   yordamchilar, voqea dalillari va boshqaruv siyosatiga qaytariladigan tartibsizlik skriptlari.
   Egalari: Hujjatlar/Yordam tahriri bilan LLM.
4. **Kontentni yoqish** — Brifing slaydlarini, laboratoriya ko‘rsatmalarini va
   2026 yil fevral sessiyasi uchun bilimlarni tekshirish savollari. Egalari: Hujjatlar/Yordam
   Menejer, SRE faollashtirish jamoasi.

## Ish oqimi va Runbook ilgaklarini yoqish

### 1. Mahalliy + CI tutun qoplamasi

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` Torii sinov muhitini aylantiradi, kanonik ko'p manbali SoraFS moslamasini qayta o'ynaydi (`ci/check_sorafs_orchestrator_adoption.sh` ga topshiriladi) va sintetik Android telemetriyasini urug'laydi.
  - Trafik yaratish `scripts/telemetry/generate_android_load.py` tomonidan boshqariladi, u `artifacts/android/telemetry/load-generator.log` ostida so'rov/javob transkriptini yozib oladi va sarlavhalar, yo'lni bekor qilish yoki quruq ishga tushirish rejimini hurmat qiladi.
  - Yordamchi SoraFS jadvalini/xulosalarini `${WORKDIR}/sorafs/` ga ko'chiradi, shuning uchun AND7 mashqlari mobil mijozlarga tegmasdan oldin ko'p manbali paritetni isbotlashi mumkin.
- CI xuddi shu asbobdan qayta foydalanadi: `ci/check_android_dashboard_parity.sh` `scripts/telemetry/compare_dashboards.py` ni `dashboards/grafana/android_telemetry_overview.json`, Rust mos yozuvlar paneli va `dashboards/data/android_rust_dashboard_allowances.json` da ruxsatnoma fayliga qarshi ishlaydi va imzolangan farqni chiqaradi I0100000
- Xaos mashqlari `docs/source/sdk/android/telemetry_chaos_checklist.md` dan keyin; sample-env skripti va asboblar panelidagi paritet tekshiruvi AND7 yonish auditini ta'minlovchi "tayyor" dalillar to'plamini tashkil qiladi.

### 2. Emissiya va audit izini bekor qilish

- `scripts/android_override_tool.py` - redaktsiyani bekor qilish va bekor qilish uchun kanonik CLI. `apply` imzolangan so‘rovni qabul qiladi, manifest to‘plamini chiqaradi (sukut bo‘yicha `telemetry_redaction_override.to`) va `docs/source/sdk/android/telemetry_override_log.md` ga xeshlangan token qatorini qo‘shadi. `revoke` o'sha qatorga qarshi bekor qilish vaqt tamg'asini qo'yadi va `digest` boshqaruv uchun zarur bo'lgan sanitarlashtirilgan JSON snapshotini yozadi.
- `docs/source/android_support_playbook.md` da ko'rsatilgan muvofiqlik talabiga mos keladigan Markdown jadval sarlavhasi mavjud bo'lmasa, CLI audit jurnalini o'zgartirishni rad etadi. `scripts/tests/test_android_override_tool_cli.py`-da birlik qamrovi jadval tahlilini, manifest emitentlarini va xatolarni qayta ishlashni himoya qiladi.
- Operatorlar har doim bekor qilish amalga oshirilganda yaratilgan manifest, yangilangan jurnal parchasini, **va ** dayjest JSONni `docs/source/sdk/android/readiness/override_logs/` ostida biriktiradilar; jurnal ushbu rejadagi boshqaruv qaroriga ko'ra 365 kunlik tarixni saqlaydi.

### 3. Dalillarni qo'lga olish va saqlash

- Har bir mashq yoki hodisa `artifacts/android/telemetry/` ostida tuzilgan to'plamni ishlab chiqaradi, o'z ichiga oladi:
  - `generate_android_load.py` dan yuk generatori transkripti va agregat hisoblagichlari.
  - Boshqaruv paneli pariteti farqi (`android_vs_rust-<stamp>.json`) va `ci/check_android_dashboard_parity.sh` tomonidan chiqarilgan ruxsat xeshi.
  - Jurnal deltasini bekor qilish (agar bekor qilish berilgan bo'lsa), tegishli manifest va yangilangan JSON dayjesti.
- SRE yonish hisoboti ushbu artefaktlarga hamda `android_sample_env.sh` tomonidan koʻchirilgan SoraFS koʻrsatkich paneliga havola qiladi va AND7 tayyorligini tekshirishga telemetriya xeshlari → asboblar paneli → bekor qilish holatidan deterministik zanjir beradi.

## Cross-SDK Device Profile Alignment

Boshqaruv paneli Android-ning `hardware_tier`-ni kanonik formatga tarjima qiladi.
`mobile_profile_class` da belgilangan
`docs/source/sdk/mobile_device_profile_alignment.md` shunday AND7 va IOS7 telemetriyasi
bir xil guruhlarni solishtiring:

- `lab` - Swift-ga mos keladigan `hardware_tier = emulator` sifatida chiqariladi
  `device_profile_bucket = simulator`.
- `consumer` - `hardware_tier = consumer` sifatida chiqariladi (SDK asosiy qo'shimchasi bilan)
  va Swiftning `iphone_small`/`iphone_large`/`ipad` chelaklari bilan guruhlangan.
- `enterprise` - `hardware_tier = enterprise` sifatida chiqariladi, Swift bilan mos keladi
  `mac_catalyst` paqir va kelajakda boshqariladigan/iOS ish stoli ish vaqtlari.

Har qanday yangi darajani tekislash hujjatiga va sxema farqlari artefaktlariga qo'shish kerak
asboblar paneli uni ishlatishdan oldin.

## Boshqaruv va taqsimlash

- **Oldindan oʻqilgan paket** — Ushbu hujjat va ilova artefaktlari (sxema farqi,
  runbook diff, tayyorlik pastki konturi) SRE boshqaruviga tarqatiladi
  pochta ro'yxati **2026-02-05** dan kechiktirmay.
- **Fikr-mulohaza zanjiri** — Boshqaruv davomida to‘plangan sharhlar quyidagi ma’lumotlarga kiritiladi
  `AND7` JIRA epik; blokerlar `status.md` va Android haftaliklarida chiqariladi
  stend-up yozuvlari.
- **Nashriyot** — Tasdiqlangandan so'ng, siyosat xulosasi quyidagi manzildan bog'lanadi
  `docs/source/android_support_playbook.md` va baham ko'rilgan
  `docs/source/telemetry.md` da telemetriya haqida tez-tez so'raladigan savollar.

## Audit va muvofiqlik eslatmalari

- Siyosat mobil abonent maʼlumotlarini oʻchirish orqali GDPR/CCPA talablariga javob beradi
  eksport qilishdan oldin; hashlangan hokimiyat tuzi har chorakda aylanadi va saqlanadi
  umumiy sirlar ombori.
- Yoqish artefaktlari va runbook yangilanishlari muvofiqlik registrida qayd etiladi.
- Har choraklik ko'rib chiqishlar bekor qilishlar yopiq tsiklda qolishini tasdiqlaydi (eskirgan kirish yo'q).

## Boshqaruv natijalari (2026-02-12)

SRE boshqaruvi sessiyasi **2026-02-12** Android tahririni tasdiqladi
o'zgartirishlarsiz siyosat. Asosiy qarorlar (qarang
`docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Siyosatni qabul qilish.** Hashed vakolatlari, qurilma profilini paqirlash va
  tashuvchi nomlari qoldirilganligi tasdiqlandi. Tuz aylanishini kuzatish orqali
  `android.telemetry.redaction.salt_version` har choraklik audit elementiga aylanadi.
- **Tasdiqlash rejasi.** Birlik/integratsiya qamrovi, tungi sxema farqlari va
  har chorakda tartibsizlik mashqlari tasdiqlandi. Harakat elementi: asboblar panelini nashr qilish
  har bir mashqdan keyin paritet hisoboti.
- **Boshqaruvni bekor qilish.** Norito tomonidan qayd etilgan bekor qilish tokenlari
  365 kunlik saqlash oynasi. Yordam muhandisligi bekor qilish jurnaliga egalik qiladi
  oylik operatsiyalarni sinxronlash paytida dayjest ko'rib chiqish.

## Kuzatuv holati

1. **Qurilma profilini moslashtirish (muddati: 2026-03-01).** ✅ Tugallangan — ulashilgan
   `docs/source/sdk/mobile_device_profile_alignment.md` da xaritalash qanday qilib aniqlanishini belgilaydi
   Android `hardware_tier` qiymatlari `mobile_profile_class` kanoniga mos keladi
   paritet asboblar paneli va sxema diff asboblari tomonidan iste'mol qilinadi.

## Kelgusi SRE boshqaruvi qisqacha bayoni (22026-yil)“Yoʻl xaritasi” bandi **AND7** SRE boshqaruvining keyingi sessiyasi a qabul qilinishini talab qiladi
qisqacha Android telemetriya redaktsiyasini oldindan o'qish. Ushbu bo'limni tirik sifatida foydalaning
qisqacha; har bir kengash yig'ilishidan oldin uni yangilab turing.

### Tayyorgarlik nazorati ro'yxati

1. **Dalillar to‘plami** — so‘nggi farq sxemasini, asboblar panelidagi skrinshotlarni eksport qilish,
   va jurnallar dayjestini bekor qiling (quyidagi matritsaga qarang) va ularni sanasi ostida joylashtiring
   papka (masalan
   `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) oldin
   taklifnoma yuborish.
2. **Matkap xulosasi** — eng so'nggi tartibsizlik mashqlari jurnalini va ilova qiling
   `android.telemetry.redaction.failure` metrik surat; Alertmanager-ga ishonch hosil qiling
   izohlar bir xil vaqt tamg'asiga ishora qiladi.
3. **Override audit** — barcha faol bekor qilishlar Norito da qayd etilganligini tasdiqlang
   ro'yxatga olish kitobi va yig'ilish palubasida umumlashtiriladi. Yaroqlilik muddati va ni kiriting
   tegishli hodisa identifikatorlari.
4. **Kun tartibiga eslatma** — yig'ilishdan 48 soat oldin SRE raisiga xabar bering.
   qisqacha havola, kerakli qarorlarni ta'kidlaydi (yangi signallar, saqlash
   o'zgartirishlar yoki siyosat yangilanishlarini bekor qilish).

### Dalillar matritsasi

| Artefakt | Manzil | Egasi | Eslatmalar |
|----------|----------|-------|-------|
| Sxema farqi va Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetriya asboblari DRI | Uchrashuvdan <72 soat oldin yaratilishi kerak. |
| Boshqaruv panelidagi farq skrinshotlari | `docs/source/sdk/android/readiness/dashboards/<date>/` | Kuzatish qobiliyati TL | `sorafs.fetch.*`, `android.telemetry.*` va Alertmanager suratlarini qoʻshing. |
| Dijestni bekor qilish | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Muhandislikni qo'llab-quvvatlash | `scripts/android_override_tool.sh digest` (ushbu katalogdagi README-ga qarang) so'nggi `telemetry_override_log.md`-ga qarshi ishga tushiring; tokenlar almashishdan oldin xeshlangan holda qoladi. |
| Chaos repetisiya jurnali | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA avtomatlashtirish | KPI xulosasini qo'shing (to'xtashlar soni, qayta urinish nisbati, foydalanishni bekor qilish). |

### Kengash uchun ochiq savollar

- Endi bekor qilish muddatini 365 kundan qisqartirishimiz kerakmi?
  dayjest avtomatlashtirilganmi?
- `android.telemetry.device_profile` yangi almashishni qabul qilishi kerak
  Keyingi nashrda `mobile_profile_class` teglari yoki Swift/JS ni kuting
  SDK'lar bir xil o'zgarishni jo'natadimi?
- Torii dan bir marta mintaqaviy ma'lumotlar rezidentligi uchun qo'shimcha ko'rsatmalar talab qilinadimi?
  Norito-RPC hodisalari Androidga tushadimi (NRPC-3 kuzatuvi)?

### Telemetriya sxemasini farqlash tartibi

Sxema diff vositasini har bir reliz nomzodiga kamida bir marta ishga tushiring (va Android
asboblar o'zgaradi) shuning uchun SRE kengashi yangi paritet artefaktlarni oladi
asboblar panelidagi farq:

1. Taqqoslamoqchi bo‘lgan Android va Rust telemetriya sxemalarini eksport qiling. CI uchun konfiguratsiyalar mavjud
   `configs/android_telemetry.json` va `configs/rust_telemetry.json` ostida.
2. `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json` ni bajaring.
   - Shu bilan bir qatorda majburiyatlarni (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) o'tkazing
     konfiguratsiyalarni to'g'ridan-to'g'ri git-dan tortib oling; skript artefakt ichidagi xeshlarni mahkamlaydi.
3. Yaratilgan JSONni tayyorlik toʻplamiga biriktiring va uni `status.md` + dan bogʻlang.
   `docs/source/telemetry.md`. Farq qo'shilgan/olib tashlangan maydonlarni va saqlash deltalarini ta'kidlaydi
   auditorlar redaktsiya paritetini asbobni takrorlamasdan tasdiqlashlari mumkin.
4. Farq ruxsat etilgan farqni aniqlasa (masalan, faqat Android uchun bekor qilish signallari),
   `ci/check_android_dashboard_parity.sh` tomonidan havola qilingan nafaqa fayli va mantiqiy asosga e'tibor bering
   README schema-diff katalogi.

> **Arxiv qoidalari:** eng oxirgi beshta farqni ostida saqlang
> `docs/source/sdk/android/readiness/schema_diffs/` va eski suratlarni oʻtkazing
> `artifacts/android/telemetry/schema_diffs/`, shuning uchun boshqaruv sharhlovchilari har doim eng so'nggi ma'lumotlarni ko'rishadi.