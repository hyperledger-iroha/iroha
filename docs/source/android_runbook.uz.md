---
lang: uz
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

# Android SDK Operations Runbook

Ushbu runbook operatorlar va Android SDK boshqaruvchi muhandislarni qo'llab-quvvatlaydi
AND7 va undan keyingi versiyalar uchun joylashtirish. SLA uchun Android Support Playbook bilan ulang
ta'riflar va kuchayish yo'llari.

> **Eslatma:** Voqea tartib-qoidalarini yangilashda, shuningdek, umumiy ma'lumotlarni yangilang
> muammolarni bartaraf etish matritsasi (`docs/source/sdk/android/troubleshooting.md`) shuning uchun
> stsenariy jadvali, SLA’lar va telemetriya ma’lumotnomalari ushbu ish kitobiga mos keladi.

## 0. Tez boshlash (peyjerlar ishga tushganda)

Batafsil ma'lumotga sho'ng'ishdan oldin Sev1/Sev2 ogohlantirishlari uchun ushbu ketma-ketlikni qulay saqlang
quyidagi bo'limlar:

1. **Faol konfiguratsiyani tasdiqlang:** `ClientConfig` manifest nazorat summasini yozib oling
   ilova ishga tushganda chiqariladi va uni qadalgan manifest bilan solishtiring
   `configs/android_client_manifest.json`. Agar xeshlar ajralib chiqsa, nashrlarni to'xtating va
   telemetriya/bekorlarni teginishdan oldin konfiguratsiya drift chiptasini rasmiylashtiring (§1-ga qarang).
2. **Diff gate sxemasini ishga tushiring:** `telemetry-schema-diff` CLI-ni quyidagiga qarshi bajaring
   qabul qilingan surat
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   Har qanday `policy_violations` chiqishini Sev2 sifatida ko'rib chiqing va oxirigacha eksportni bloklang
   nomuvofiqlik tushuniladi (2.6-bandga qarang).
3. **Boshqaruv paneli + CLI holatini tekshiring:** Android Telemetriyani tahrirlashni oching va
   Exporter Health boards, keyin ishga tushiring
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. Agar
   organlari qavat ostida yoki eksport xato, qo'lga ekran tasvirlari va
   Hodisa hujjati uchun CLI chiqishi (qarang: §2.4–§2.5).
4. **Bekor qilish to'g'risida qaror qabul qiling:** Faqat yuqoridagi amallardan keyin va hodisa/egasi bilan
   qayd etilgan, `scripts/android_override_tool.sh` orqali cheklangan bekor qilish
   va uni `telemetry_override_log.md` tizimiga kiriting (3-§ga qarang). Birlamchi amal qilish muddati: <24 soat.
5. **Har bir kontakt roʻyxatini koʻpaytirish:** Qoʻngʻiroq boʻyicha Android va Observability TL sahifasi
   (§8-dagi kontaktlar), keyin §4.1-dagi eskalatsiya daraxtiga amal qiling. Agar attestatsiya yoki
   StrongBox signallari ishtirok etadi, eng so'nggi to'plamni torting va jabduqni ishga tushiring
   eksportni qayta yoqishdan oldin §7 dan tekshiradi.

## 1. Konfiguratsiya va joylashtirish

- **ClientConfig manbasi:** Android mijozlari Torii oxirgi nuqtasi, TLS yuklanishiga ishonch hosil qiling
  siyosatlar va `iroha_config`-dan olingan manifestlardan qayta urinib ko'ring. Tasdiqlash
  ilovani ishga tushirish vaqtidagi qiymatlar va faol manifestning log nazorat summasi.
  Amalga oshirish uchun ma'lumotnoma: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  iplar `TelemetryOptions` dan `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (shuningdek, yaratilgan `TelemetryObserver`) shuning uchun xeshlangan vakolatlar avtomatik ravishda chiqariladi.
- **Issiq qayta yuklash:** `iroha_config` ni olish uchun konfiguratsiya kuzatuvchisidan foydalaning
  ilovalarni qayta ishga tushirmasdan yangilanishlar. Muvaffaqiyatsiz qayta yuklashlar chiqarishi kerak
  `android.telemetry.config.reload` hodisasi va eksponensial bilan qayta urinish
  orqaga qaytish (maksimal 5 urinish).
- **Orqaga qaytish harakati:** Agar konfiguratsiya mavjud boʻlmasa yoki yaroqsiz boʻlsa, ga qayting
  xavfsiz standart sozlamalar (faqat o'qish rejimi, kutilayotgan navbatni topshirish yo'q) va foydalanuvchini ko'rsatish
  taklif. Kuzatuv uchun voqeani yozib oling.

### 1.1 Qayta yuklash diagnostikasini sozlash- Konfiguratsiya kuzatuvchisi `android.telemetry.config.reload` signallarini chiqaradi
  `source`, `result`, `duration_ms` va ixtiyoriy `digest`/`error` maydonlari (qarang.
  `configs/android_telemetry.json` va
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  Har bir qo'llaniladigan manifestda bitta `result:"success"` hodisasini kuting; takrorlanadi
  `result:"error"` yozuvlari kuzatuvchining 5 ta orqaga qaytish urinishlarini tugatganligini ko'rsatadi.
  50ms dan boshlanadi.
- Hodisa paytida kollektordan so'nggi qayta yuklash signalini oling
  (OTLP/span do'koni yoki redaktsiya holatining so'nggi nuqtasi) va `digest` + ni qayd qiling
  Hodisa hujjatida `source`. Dijest bilan solishtiring
  `configs/android_client_manifest.json` va nashr manifestiga tarqatildi
  operatorlar.
- Agar kuzatuvchi xatoliklarni chiqarishda davom etsa, ko'paytirish uchun mo'ljallangan jabduqni ishga tushiring
  shubhali manifest bilan tahlil xatosi:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  Sinov chiqishi va muvaffaqiyatsiz manifestni hodisa to'plamiga SRE qilib biriktiring
  uni pishirilgan konfiguratsiya sxemasidan farq qilishi mumkin.
- Qayta yuklash telemetriyasi yo'q bo'lsa, faol `ClientConfig` ni tasdiqlang
  telemetriya sink va OTLP kollektori hali ham qabul qiladi
  `android.telemetry.config.reload` ID; aks holda uni Sev2 telemetriyasi sifatida qabul qiling
  regressiya (§2.4 bilan bir xil yo'l) va signal qaytguncha pauza bo'shatish.

### 1.2 Deterministik kalit eksport paketlari
- Dasturiy ta'minot eksporti endi har bir eksport tuzi + nonce, `kdf_kind` va `kdf_work_factor` bilan v3 to'plamlarini chiqaradi.
  Eksportchi Argon2id ni afzal ko'radi (64 MiB, 3 iteratsiya, parallelizm = 2) va qaytadi.
  PBKDF2-HMAC-SHA256, agar qurilmada Argon2id mavjud bo'lmasa, 350 k iteratsiya qavatiga ega. To'plam
  AAD hali ham taxallus bilan bog'lanadi; v3 eksporti uchun parollar kamida 12 belgidan iborat bo'lishi kerak
  import qiluvchi barcha nol tuz/nonce urug'larni rad etadi.
  `KeyExportBundle.decode(Base64|bytes)`, asl parol bilan import qiling va v3 ga qayta eksport qiling
  qattiq xotira formatiga o'ting. Import qiluvchi nol yoki qayta ishlatiladigan tuz/nontime juftlarini rad etadi; har doim
  qurilmalar o'rtasida eski eksportlarni qayta ishlatish o'rniga to'plamlarni aylantiring.
- `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests` da salbiy yo'l testlari
  rad etish. Foydalanishdan keyin parolli iboralar massivlarini tozalang va paket versiyasini va `kdf_kind` ni yozib oling
  tiklash muvaffaqiyatsizlikka uchragan voqea qaydlarida.

## 2. Telemetriya va redaktsiya

> Tezkor ma'lumot: qarang
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> faollashtirish vaqtida foydalaniladigan siqilgan buyruq/eshik nazorat ro'yxati uchun
> seanslar va hodisa ko'prigi.- **Signal inventarizatsiyasi:** `docs/source/sdk/android/telemetry_redaction.md` ga qarang
  chiqarilgan oraliqlar/ko'rsatkichlar/hodisalar to'liq ro'yxati va
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  egasi/tasdiqlash tafsilotlari va ajoyib bo'shliqlar uchun.
- **Kanonik sxema farqi:** Tasdiqlangan AND7 surati
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  Har bir yangi CLI ishini ushbu artefakt bilan solishtirish kerak, shunda sharhlovchilar ko'rishlari mumkin
  qabul qilingan `intentional_differences` va `android_only_signals` hali ham
  da hujjatlashtirilgan siyosat jadvallariga mos keling
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. Endi CLI qo'shadi
  `policy_violations` qasddan farq yo'q bo'lganda a
  `status:"accepted"`/`"policy_allowlisted"` (yoki faqat Android yozuvlari yo'qolganda)
  ularning qabul qilingan holati), shuning uchun bo'sh bo'lmagan buzilishlarni Sev2 deb hisoblang va to'xtating
  eksport. Quyidagi `jq` snippetlari arxivda qo'lda aqlni tekshirish sifatida qoladi
  artefaktlar:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  Ushbu buyruqlardan olingan har qanday natijani kerak bo'lgan sxema regressiyasi sifatida ko'rib chiqing
  Telemetriya eksporti davom etgunga qadar AND7 tayyorligi xatosi; `field_mismatches`
  `telemetry_schema_diff.md` §5 bo'yicha bo'sh qolishi kerak. Yordamchi hozir yozadi
  `artifacts/android/telemetry/schema_diff.prom` avtomatik ravishda; o'tish
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (yoki o'rnatilgan
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) sahnalashtirish/ishlab chiqarish xostlarida ishlayotganda
  Shunday qilib, `telemetry_schema_diff_run_status` o'lchagich `policy_violation` ga aylanadi
  Agar CLI driftni aniqlasa, avtomatik ravishda.
- **CLI yordamchisi:** `scripts/telemetry/check_redaction_status.py` tekshiradi
  Sukut bo'yicha `artifacts/android/telemetry/status.json`; `--status-url` ga o'ting
  mahalliy nusxani oflayn rejimda yangilash uchun so'rov bosqichi va `--write-cache`
  matkaplar. `--min-hashed 214` dan foydalaning (yoki o'rnating
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) boshqaruvni amalga oshirish
  har bir status so'rovi paytida xeshlangan organlarga qavat.
- **Authority heshing:** Barcha organlar Blake2b-256 yordamida xeshlangan.
  xavfsiz sirlar omborida saqlanadi chorak aylanma tuz. Aylanishlar sodir bo'ladi
  har chorakning birinchi dushanba kuni soat 00:00 UTC da. Eksportchi qabul qilishini tasdiqlang
  `android.telemetry.redaction.salt_version` ko'rsatkichini tekshirish orqali yangi tuz.
- **Qurilma profili chelaklari:** Faqat `emulator`, `consumer` va `enterprise`
  darajalar eksport qilinadi (SDK asosiy versiyasi bilan birga). Boshqaruv panellari ularni taqqoslaydi
  Rustning asosiy ko'rsatkichlari bilan hisoblashadi; >10% farq ogohlantirishlarni oshiradi.
- **Tarmoq metamaʼlumotlari:** Android faqat `network_type` va `roaming` bayroqlarini eksport qiladi.
  Operator nomlari hech qachon ko'rsatilmaydi; operatorlar abonentni talab qilmasligi kerak
  hodisalar jurnallarida ma'lumotlar. Sanitatsiya qilingan oniy rasm sifatida chiqariladi
  `android.telemetry.network_context` hodisasi, shuning uchun ilovalar roʻyxatdan oʻtganiga ishonch hosil qiling a
  `NetworkContextProvider` (yoki orqali
  `ClientConfig.Builder.setNetworkContextProvider(...)` yoki qulaylik
  `enableAndroidNetworkContext(...)` yordamchisi) Torii qo'ng'iroqlari chiqarilishidan oldin.
- **Grafana ko'rsatkichi:** `Android Telemetry Redaction` asboblar paneli
  Yuqoridagi CLI chiqishi uchun kanonik vizual tekshirish - tasdiqlang
  `android.telemetry.redaction.salt_version` paneli joriy tuz davriga mos keladi
  va `android_telemetry_override_tokens_active` vidjeti nolda qoladi
  hech qanday mashqlar yoki hodisalar bajarilmaganda. Har bir panel siljib ketsa, kuchaytiring
  CLI skriptlari regressiya haqida xabar berishdan oldin.

### 2.1 Eksport quvurlari ish jarayoni1. **Konfiguratsiya taqsimoti.** `ClientConfig.telemetry.redaction` dan ulangan
   `iroha_config` va `ConfigWatcher` tomonidan qayta yuklangan. Har bir qayta yuklash qayd qiladi
   manifest digest plyus tuz davri - bu chiziqni hodisalarda va davomida qo'lga kiriting
   mashqlar.
2. **Instrumentation.** SDK komponentlari oraliqlar/ko‘rsatkichlar/hodisalar chiqaradi
   `TelemetryBuffer`. Bufer har bir foydali yukni qurilma profili va
   joriy tuz davri, shuning uchun eksportchi xeshlash ma'lumotlarini aniq tekshirishi mumkin.
3. **Redaktsiya filtri.** `RedactionFilter` xeshlari `authority`, `alias` va
   qurilma identifikatorlari qurilmani tark etishidan oldin. Muvaffaqiyatsizliklar chiqaradi
   `android.telemetry.redaction.failure` va eksport urinishini bloklang.
4. **Eksportchi + kollektor.** Sanitizatsiya qilingan foydali yuklar Android orqali yuboriladi
   `android-otel-collector` o'rnatish uchun OpenTelemetry eksportchisi. The
   kollektor fanatlari izlar (Temp), ko'rsatkichlar (Prometheus) va Norito chiqadi
   log lavabolar.
5. **Kuzatilish ilgaklari.** `scripts/telemetry/check_redaction_status.py` o'qiydi
   kollektor hisoblagichlari (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) va holat to'plamini ishlab chiqaradi
   ushbu runbook davomida havola qilingan.

### 2.2 Tasdiqlash eshiklari

- **Sxema farqi:** Ishga tushirish
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  namoyon bo'lganda o'zgaradi. Har bir yugurishdan keyin har birini tasdiqlang
  `intentional_differences[*]` va `android_only_signals[*]` yozuvlari muhrlangan
  `status:"accepted"` (yoki xeshlangan/chelaklangan uchun `status:"policy_allowlisted"`)
  maydonlar) biriktirishdan oldin `telemetry_schema_diff.md` §3 da tavsiya qilinganidek
  hodisalar va xaos laboratoriya hisobotlari uchun artefakt. Tasdiqlangan suratdan foydalaning
  (`android_vs_rust-20260305.json`) to'siq sifatida va yangi chiqarilgan
  JSON topshirilishidan oldin:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  `$LATEST` bilan solishtiring
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  ruxsat etilgan ro'yxat o'zgarishsiz qolganligini isbotlash. Yo'qolgan yoki bo'sh `status`
  yozuvlar (masalan, `android.telemetry.redaction.failure` yoki
  `android.telemetry.redaction.salt_version`) endi regressiya va sifatida qaraladi
  ko'rib chiqish tugashidan oldin hal qilinishi kerak; CLI qabul qilinganni yuzaga chiqaradi
  to'g'ridan-to'g'ri davlat, shuning uchun qo'llanma §3.4 o'zaro havola faqat qachon amal qiladi
  nega `accepted` bo'lmagan holat paydo bo'lishini tushuntiradi.

  **Kanonik AND7 signallari (2026-03-05 surati)**| Signal | Kanal | Holati | Boshqaruv eslatmasi | Tasdiqlash kancasi |
  |--------|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` | Tadbir | `accepted` | Ko'zgular manifestlarni bekor qiladi va `telemetry_override_log.md` yozuvlariga mos kelishi kerak. | `android_telemetry_override_tokens_active` ni tomosha qiling va §3 bo'yicha manifestlarni arxivlang. |
  | `android.telemetry.network_context` | Tadbir | `accepted` | Android tashuvchi nomlarini ataylab o'zgartiradi; faqat `network_type` va `roaming` eksport qilinadi. | Ilovalar `NetworkContextProvider` ni roʻyxatdan oʻtkazganiga ishonch hosil qiling va `Android Telemetry Overview` da Torii trafigiga mos keladigan voqea hajmini tasdiqlang. |
  | `android.telemetry.redaction.failure` | Hisoblagich | `accepted` | Xeshlash muvaffaqiyatsiz bo'lganda chiqaradi; boshqaruv endi diff artefakt sxemasida aniq status metama'lumotlarini talab qiladi. | `Redaction Compliance` asboblar paneli paneli va `check_redaction_status.py` dan CLI chiqishi mashqlar paytidan tashqari nolda qolishi kerak. |
  | `android.telemetry.redaction.salt_version` | O'lchagich | `accepted` | Eksport qiluvchi joriy choraklik tuz davridan foydalanishini isbotlaydi. | Grafana tuz vidjetini sirlar ombori davri bilan solishtiring va sxema farqlari `status:"accepted"` izohini saqlab qolishiga ishonch hosil qiling. |

  Agar yuqoridagi jadvaldagi biron bir yozuv `status` ni tushirsa, farq artefakt bo'lishi kerak.
  qayta tiklangan **va** `telemetry_schema_diff.md` AND7 dan oldin yangilangan
  boshqaruv paketi tarqatiladi. Yangilangan JSONni qo'shing
  `docs/source/sdk/android/readiness/schema_diffs/` va uni dan bog'lang
  voqea, tartibsizlik laboratoriyasi yoki qayta ishga tushirishga sabab bo'lgan faollashtirish hisoboti.
- **CI/birlik qamrovi:** `ci/run_android_tests.sh` oldin o'tishi kerak
  nashriyot tuzilmalari; to'plam mashq qilish orqali xeshlash/bekor qilish xatti-harakatlarini amalga oshiradi
  namuna yuklari bilan telemetriya eksportchilari.
- **Injektorning sog'lig'ini tekshirish:** Foydalanish
  Mashqlar oldidan `scripts/telemetry/inject_redaction_failure.sh --dry-run`
  nosozlik inyeksiya ishlarini tasdiqlash va qo'riqchilarni xeshlashda yong'in haqida ogohlantiradi
  qoqilib ketishadi. Injektorni har doim bir marta tekshirish `--clear` bilan tozalang
  yakunlaydi.

### 2.3 Mobile ↔ Rust telemetriya pariteti nazorat roʻyxati

Android eksportchilari va Rust tugun xizmatlarini bir xilda saqlang
hujjatlashtirilgan turli tahrirlash talablari
`docs/source/sdk/android/telemetry_redaction.md`. Quyidagi jadval sifatida xizmat qiladi
AND7 yo'l xaritasi yozuvida havola qilingan ikki tomonlama ruxsat etilgan ro'yxat - uni istalgan vaqtda yangilang
schema diff maydonlarni kiritadi yoki olib tashlaydi.| Kategoriya | Android eksportchilari | Rust xizmatlari | Tasdiqlash kancasi |
|----------|-------------------|---------------|-----------------|
| Vakolat / marshrut konteksti | Blake2b-256 orqali `authority`/`alias` xash va eksportdan oldin xom Torii xost nomlarini tashlang; tuz aylanishini isbotlash uchun `android.telemetry.redaction.salt_version` chiqaradi. | Korrelyatsiya uchun to'liq Torii xost nomlari va tengdosh identifikatorlarini chiqaring. | `readiness/schema_diffs/` ostidagi so'nggi sxema farqidagi `android.torii.http.request` va `torii.http.request` yozuvlarini solishtiring, so'ngra `scripts/telemetry/check_redaction_status.py` ishga tushirish orqali `android.telemetry.redaction.salt_version` klaster tuziga mos kelishini tasdiqlang. |
| Qurilma va imzolovchi identifikatori | Paqir `hardware_tier`/`device_profile`, xesh-kontroller taxalluslari va hech qachon seriya raqamlarini eksport qilmang. | Qurilmaning metamaʼlumotlari yoʻq; tugunlar validator `peer_id` va kontroller `public_key` so'zma-so'z chiqaradi. | `docs/source/sdk/mobile_device_profile_alignment.md`-dagi xaritalarni aks ettiring, laboratoriya mashg'ulotlari davomida `PendingQueueInspector` natijalarini tekshirib ko'ring va `ci/run_android_tests.sh` ichidagi taxallus xeshlash testlari yashil bo'lib qolishiga ishonch hosil qiling. |
| Tarmoq metama'lumotlari | Faqat `network_type` + `roaming` mantiqiy qiymatlarni eksport qilish; `carrier_name` o'chirildi. | Rust tengdosh xost nomlarini va toʻliq TLS soʻnggi nuqta metamaʼlumotlarini saqlaydi. | Eng so'nggi farq JSONni `readiness/schema_diffs/` da saqlang va Android tomoni hali ham `carrier_name` ni o'tkazib yuborayotganini tasdiqlang. Grafana ning "Tarmoq konteksti" vidjeti har qanday tashuvchi qatorlarini ko'rsatsa, ogohlantiring. |
| bekor qilish / tartibsizlik dalillar | Maskali aktyor rollari bilan `android.telemetry.redaction.override` va `android.telemetry.chaos.scenario` voqealarini emit. | Rust xizmatlari rolni niqoblashsiz va tartibsizliklarga xos oraliqlarsiz bekor qilish ruxsatlarini chiqaradi. | Har bir mashqdan keyin `docs/source/sdk/android/readiness/and7_operator_enablement.md` ni oʻzaro tekshirib koʻring, tokenlar va tartibsizlik artefaktlari maskalanmagan Rust hodisalari bilan birga arxivlanganligiga ishonch hosil qiling. |

Paritet ish jarayoni:

1. Har bir manifest yoki eksportchi o'zgarishidan keyin ishga tushiring
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   shuning uchun JSON artefakti va aks ettirilgan ko'rsatkichlar ikkalasi ham dalillar to'plamiga tushadi
   (yordamchi hali ham sukut bo'yicha `artifacts/android/telemetry/schema_diff.prom` deb yozadi).
2. Yuqoridagi jadvalga nisbatan farqni ko'rib chiqing; agar Android endi maydonni chiqaradi
   faqat Rust-da ruxsat berilgan (yoki aksincha), AND7-ga tayyorlik xatosini va yangilashni yozing
   tahrirlash rejasi.
3. Haftalik tekshiruvlar vaqtida yuguring
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   tuz davrlari Grafana vidjetiga mos kelishini tasdiqlash uchun va davrni qayd qiling.
   chaqiruv jurnali.
4. Barcha deltalarni yozib oling
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` shunday
   boshqaruv paritet qarorlarni tekshirishi mumkin.

### 2.4 Kuzatuv paneli va ogohlantirish chegaralari

Boshqaruv panellari va ogohlantirishlarni qachon AND7 sxemasi farqlarini tasdiqlash bilan moslab turing
`scripts/telemetry/check_redaction_status.py` chiqishini ko'rib chiqish:

- `Android Telemetry Redaction` - Tuz davri vidjeti, token o'lchagichni bekor qilish.
- `Redaction Compliance` — `android.telemetry.redaction.failure` hisoblagich va
  injektor trend panellari.
- `Exporter Health` — `android.telemetry.export.status` stavkalarining buzilishi.
- `Android Telemetry Overview` - qurilma profili paqirlari va tarmoq konteksti hajmi.

Quyidagi chegaralar tezkor ma'lumot kartasini aks ettiradi va ular bajarilishi kerak
hodisaga javob berish va mashqlar paytida:| Metrik / panel | Ostona | Harakat |
|----------------|-----------|--------|
| `android.telemetry.redaction.failure` (`Redaction Compliance` taxtasi) | >0 dumalab 15min oyna ustida | Ishlamay qolgan signalni tekshirib ko'ring, injektorni tozalang, CLI chiqishi + Grafana skrinshotini qayd qiling. |
| `android.telemetry.redaction.salt_version` (`Android Telemetry Redaction` taxtasi) | Sirlardan farq qiladi-qnoz tuzi davri | Relizlarni to'xtating, sirlarni aylantirish bilan muvofiqlashtiring, fayl AND7 eslatmasi. |
| `android.telemetry.export.status{status="error"}` (`Exporter Health` taxtasi) | >1% eksport | Kollektorning sog'lig'ini tekshiring, CLI diagnostikasini oling, SREga o'ting. |
| `android.telemetry.device_profile{tier="enterprise"}` va Rust pariteti (`Android Telemetry Overview`) | Tafovut >10% Rustning asosiy ko'rsatkichidan | Fayl boshqaruvini kuzatib boring, armatura hovuzlarini tekshiring, sxema farqiga izoh bering. |
| `android.telemetry.network_context` hajmi (`Android Telemetry Overview`) | Torii trafik mavjud bo'lganda nolga tushadi | `NetworkContextProvider` roʻyxatdan oʻtishni tasdiqlang, maydonlar oʻzgarmasligini taʼminlash uchun diff sxemasini qayta ishga tushiring. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | Noldan tashqari tasdiqlangan bekor qilish/burg'ulash oynasi | Tokenni voqeaga bog‘lang, dayjestni qayta tiklang, 3-§da ish jarayoni orqali bekor qiling. |

### 2.5 Operatorning tayyorligi va ishga tushirilishi

Yo'l xaritasi AND7 bandi maxsus operator o'quv dasturini chaqiradi, shuning uchun qo'llab-quvvatlash, SRE va
nashrning manfaatdor tomonlari yuqoridagi paritet jadvallarini runbook ishga tushishidan oldin tushunishadi
GA. Konturdan foydalaning
Kanonik logistika uchun `docs/source/sdk/android/telemetry_readiness_outline.md`
(kun tartibi, taqdimotchilar, vaqt jadvali) va `docs/source/sdk/android/readiness/and7_operator_enablement.md`
batafsil nazorat ro'yxati, dalillar havolalari va harakatlar jurnali uchun. Quyidagilarni saqlang
Telemetriya rejasi o'zgarganda fazalar sinxronlashtiriladi:| Bosqich | Tavsif | Dalillar to'plami | Asosiy egasi |
|-------|-------------|-----------------|---------------|
| O'qishdan oldin tarqatish | Oldindan oʻqilgan siyosatni, `telemetry_redaction.md` va tezkor maʼlumot kartasini brifingdan kamida besh ish kuni oldin yuboring. Tasdiqlashlarni konturning xabarlar jurnalida kuzatib boring. | `docs/source/sdk/android/telemetry_readiness_outline.md` (Session Logistics + Communications Log) va `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` da arxivlangan elektron pochta. | Hujjatlar/Yordam menejeri |
| Jonli tayyorgarlik sessiyasi | 60 daqiqalik treningni o'tkazing (siyosat chuqur sho'ng'in, yugurish kitobi, asboblar paneli, betartiblik laboratoriyasi demosi) va asinxron tomoshabinlar uchun yozib olishni davom ettiring. | Yozib olish + slaydlar konturning 2-bandida olingan havolalar bilan `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` ostida saqlanadi. | LLM (AND7 egasi vazifasini bajaruvchi) |
| Xaos laboratoriya ijrosi | Jonli seansdan so'ng darhol `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` dan kamida C2 (bekor qilish) + C6 (navbat takrorlash) ni ishga tushiring va faollashtirish to'plamiga jurnallar/skrinshotlarni biriktiring. | `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` va `/screenshots/<YYYY-MM>/` ichidagi stsenariy hisobotlari va skrinshotlar. | Android Observability TL + SRE on-call |
| Bilimlarni tekshirish va davomat | Viktorina topshiriqlarini to'plang, <90% ball to'plagan har bir kishini tuzating va davomat/viktorina statistikasini yozib oling. Tezkor savollarni paritetni tekshirish roʻyxatiga moslab qoʻying. | `docs/source/sdk/android/readiness/forms/responses/` da viktorina eksporti, `scripts/telemetry/generate_and7_quiz_summary.py` orqali ishlab chiqarilgan Markdown/JSON xulosasi va `and7_operator_enablement.md` ichidagi davomatlar jadvali. | Muhandislikni qo'llab-quvvatlash |
| Arxiv va kuzatuvlar | Faollashtirish to'plamining harakatlar jurnalini yangilang, artefaktlarni arxivga yuklang va `status.md` da yakunlanishiga e'tibor bering. Seans davomida chiqarilgan har qanday tuzatish yoki bekor qilish tokenlari `telemetry_override_log.md` ga ko'chirilishi kerak. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (harakat jurnali), `.../archive/<YYYY-MM>/checklist.md` va §3 da havola qilingan bekor qilish jurnali. | LLM (AND7 egasi vazifasini bajaruvchi) |

O'quv rejasi qayta ishlansa (har chorakda yoki asosiy sxema o'zgarishidan oldin) yangilang
yangi sessiya sanasi bilan kontur, ishtirokchilar ro'yxatini joriy saqlang va
boshqaruv paketlari uchun JSON/Markdown artefaktlari viktorina xulosasini qayta yarating
izchil dalillarga havola. AND7 uchun `status.md` yozuvi bilan bog'lanishi kerak
Har bir faollashtirish sprinti yopilgandan so'ng so'nggi arxiv papkasi.

### 2.6 Sxema farqlari ruxsat etilgan roʻyxatlar va siyosat tekshiruvlari

Yo'l xaritasi ikkita ruxsat etilgan ro'yxat siyosatini aniq ko'rsatadi (mobil tahrirlar va boshqalar
Zangni ushlab turish) ostida joylashgan `telemetry-schema-diff` CLI tomonidan amalga oshiriladi.
`tools/telemetry-schema-diff`. Har bir diff artefakt qayd etilgan
`docs/source/sdk/android/readiness/schema_diffs/` qaysi maydonlar ekanligini hujjatlashtirishi kerak
Android-da xeshlangan/paqirlangan, Rust-da qaysi maydonlar xeshlanmagan va yo'qmi
har qanday ruxsat etilmagan signal qurilishga kirdi. Ushbu qarorlarni qabul qiling
to'g'ridan-to'g'ri JSONda ishga tushirish orqali:

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
```Yakuniy `jq` hisobot toza bo'lsa, no-op deb baholanadi. Har qanday chiqishni davolash
ushbu buyruqdan Sev2 tayyorligi xatosi sifatida: to'ldirilgan `policy_violations`
massiv CLI faqat Android ro'yxatida bo'lmagan signalni aniqlaganligini anglatadi
na hujjatlashtirilgan faqat Rust-dan ozod qilish ro'yxatida
`docs/source/sdk/android/telemetry_schema_diff.md`. Bu sodir bo'lganda, to'xtating
eksport qilish, AND7 chiptasini topshirish va farqni faqat siyosat modulidan keyin qayta ishga tushirish
va manifest snapshotlari tuzatildi. Olingan JSON-ni saqlang
Sana qo'shimchasi va eslatma bilan `docs/source/sdk/android/readiness/schema_diffs/`
hodisa yoki laboratoriya hisoboti ichidagi yo'l, shuning uchun boshqaruv tekshiruvlarni takrorlashi mumkin.

**Xeshlash va saqlash matritsasi**

| Signal.field | Android bilan ishlash | Zang bilan ishlov berish | Ruxsat berilgan roʻyxat tegi |
|-------------|-----------------|---------------|---------------|
| `torii.http.request.authority` | Blake2b-256 xeshlangan (`representation: "blake2b_256"`) | Kuzatuv uchun so'zma-so'z saqlanadi | `policy_allowlisted` (mobil xesh) |
| `attestation.result.alias` | Blake2b-256 xeshlangan | Oddiy matn taxalluslari (attestatsiya arxivi) | `policy_allowlisted` |
| `attestation.result.device_tier` | Paqirlangan (`representation: "bucketed"`) | Oddiy darajali qator | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | Yo'q - Android eksportchilari maydonni butunlay tark etadilar | Tahrirsiz taqdim etish | `rust_only` (`telemetry_schema_diff.md` ning 3-bandida hujjatlashtirilgan) |
| `android.telemetry.redaction.override.*` | Masklangan aktyor rollari bilan faqat Android uchun signal | Ekvivalent signal chiqarilmadi | `android_only` (qolishi kerak `status:"accepted"`) |

Yangi signallar paydo bo'lganda, ularni sxema diff siyosat moduliga **va** qo'shing
yuqoridagi jadval, shuning uchun runbook CLI da yuborilgan majburlash mantiqini aks ettiradi.
Sxema endi faqat Android uchun signal aniq `status` ni o'tkazib yuborsa yoki bajarilmaydi
`policy_violations` massivi bo'sh emas, shuning uchun ushbu nazorat ro'yxati bilan sinxronlashtiring.
`telemetry_schema_diff.md` §3 va eng so'nggi JSON snapshotlariga havola qilingan
`telemetry_redaction_minutes_*.md`.

## 3. Ish jarayonini bekor qilish

Regressiya yoki maxfiylikni xeshlashda bekor qilish "shisha sindirish" opsiyasidir
ogohlantirishlar bloklangan mijozlar. Ularni faqat to'liq qaror yo'lini yozib olgandan keyin qo'llang
voqeada doc.1. **Drift va qamrovni tasdiqlang.** PagerDuty ogohlantirishi yoki diagramma farqini kuting
   olov uchun darvoza, keyin yugur
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` gacha
   vakolatlarning mos kelmasligini isbotlash. CLI chiqishi va Grafana skrinshotlarini biriktiring
   voqea yozuviga.
2. **Imzolangan so‘rov tayyorlang.** To‘ldiring
   `docs/examples/android_override_request.json` chipta identifikatori, so'rovchi,
   amal qilish muddati va asoslash. Faylni voqea artefaktlari yonida saqlang
   muvofiqlik kirishlarni tekshirishi mumkin.
3. **Bekor qilish.** Chaqiruv
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   Yordamchi bekor qilish tokenini chop etadi, manifestni yozadi va qator qo'shadi
   Markdown audit jurnaliga. Hech qachon tokenni chatda joylashtirmang; to'g'ridan-to'g'ri etkazib bering
   bekor qilishni qo'llaydigan Torii operatorlariga.
4. **Effektni kuzatib boring.** Besh daqiqa ichida bittasini tekshiring
   `android.telemetry.redaction.override` hodisa emissiya qilindi, kollektor
   holat so'nggi nuqtasi `override_active=true` ni ko'rsatadi va voqea hujjati ro'yxatini ko'rsatadi
   muddati tugashi. Android Telemetry Overview boshqaruv panelidagi “Tokenlarni bekor qilish
   faol” paneli (`android_telemetry_override_tokens_active`).
   tokenni hisoblang va har 10 daqiqada CLI holatini ishga tushirishda davom eting
   xeshlash barqarorlashadi.
5. **Bekor qilish va arxivlash.** Yumshatish tushishi bilanoq yuguring
  `scripts/android_override_tool.sh revoke --token <token>` shuning uchun audit jurnali
  bekor qilish vaqtini ushlaydi, keyin bajaring
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  boshqaruv kutgan sanitarlashtirilgan suratni yangilash uchun. ni biriktiring
  manifest, dayjest JSON, CLI transkriptlari, Grafana suratlari va NDJSON jurnali
  to `--event-log` orqali ishlab chiqarilgan
  `docs/source/sdk/android/readiness/screenshots/<date>/` va o'zaro bog'lang
  `docs/source/sdk/android/telemetry_override_log.md` dan kirish.

24 soatdan ortiq vaqtni bekor qilish SRE direktori va Muvofiqlik tasdiqini talab qiladi va
keyingi haftalik AND7 sharhida ta'kidlanishi kerak.

### 3.1 Eskalatsiya matritsasini bekor qilish

| Vaziyat | Maksimal muddat | Tasdiqlovchilar | Kerakli bildirishnomalar |
|----------|--------------|-----------|------------------------|
| Yagona ijarachi tekshiruvi (hashed vakolatlari mos kelmasligi, mijoz Sev2) | 4 soat | Yordam muhandisi + SRE on-call | Chipta `SUP-OVR-<id>`, `android.telemetry.redaction.override` voqea, voqea jurnali |
| Filo boʻylab telemetriya uzilishi yoki SRE soʻralgan koʻpaytirish | 24 soat | Qo'ng'iroq bo'yicha SRE + Dastur rahbari | PagerDuty eslatmasi, jurnal yozuvini bekor qilish, `status.md` da yangilash |
| Muvofiqlik/sud-tibbiyot ekspertizasi so'rovi yoki 24 soatdan ortiq har qanday holat | Aniq bekor qilinmaguncha | SRE direktori + Muvofiqlik yetakchisi | Boshqaruv pochta roʻyxati, bekor qilish jurnali, AND7 haftalik holati |

#### Rol mas'uliyati| Rol | Mas'uliyat | SLA / Eslatmalar |
|------|------------------|-------------|
| Qo'ng'iroq bo'yicha Android telemetriyasi (Incident Commander) | Drayvni aniqlash, bekor qilish vositalarini bajaring, voqea hujjatiga tasdiqlashlarni yozib oling va bekor qilish muddati tugashidan oldin sodir bo'lishini ta'minlang. | PagerDuty-ni 5 daqiqa ichida tasdiqlang va har 15 daqiqada taraqqiyotni qayd qiling. |
| Android Observability TL (Haruka Yamamoto) | Drift signalini tasdiqlang, eksportchi/kollektor holatini tasdiqlang va operatorlarga topshirishdan oldin bekor qilish manifestida imzo cheking. | 10 daqiqa ichida ko'prikka qo'shiling; agar mavjud bo'lmasa, staging klaster egasiga vakil qiling. |
| SRE aloqasi (Liam O'Konnor) | Manifestni kollektorlarga qo'llang, orqada qolganlarni kuzatib boring va Torii tomonidagi yumshatishlar uchun Release Engineering bilan muvofiqlashtiring. | Har bir `kubectl` harakatini o'zgartirish so'roviga kiriting va buyruq transkriptlarini voqea hujjatiga joylashtiring. |
| Muvofiqlik (Sofiya Martins / Daniel Park) | 30 daqiqadan ko'proq vaqtni bekor qilishni tasdiqlang, audit jurnali qatorini tekshiring va tartibga soluvchi/mijoz xabarlari bo'yicha maslahat bering. | `#compliance-alerts` da e'tirof etish; ishlab chiqarish hodisalari uchun bekor qilishdan oldin muvofiqlik eslatmasini topshiring. |
| Hujjatlar/Yordam menejeri (Priya Deshpande) | `docs/source/sdk/android/readiness/…` ostida arxiv manifestlari/CLI chiqishi, bekor qilish jurnalini tartibli saqlang va boʻshliqlar yuzaga kelsa, kuzatuv laboratoriyalarini rejalashtiring. | Hodisani yopishdan oldin dalillarni saqlashni (13 oy) va AND7 kuzatuvlarini tasdiqlaydi. |

Agar bekor qilish tokeni muddati tugashiga a holda yaqinlashsa, darhol oshiring
hujjatlashtirilgan bekor qilish rejasi.

## 4. Hodisaga javob

- **Ogohlantirishlar:** PagerDuty xizmati `android-telemetry-primary` tahrirlashni qamrab oladi
  nosozliklar, eksportchilarning uzilishlari va chelakning siljishi. SLA oynalarida tasdiqlash
  (qo'llab-quvvatlash o'yin kitobiga qarang).
- **Diagnostika:** yig‘ish uchun `scripts/telemetry/check_redaction_status.py` ni ishga tushiring
  joriy eksportchi salomatligi, so'nggi ogohlantirishlar va xeshlangan vakolat ko'rsatkichlari. O'z ichiga oladi
  hodisa vaqt jadvalidagi chiqish (`incident/YYYY-MM-DD-android-telemetry.md`).
- **Boshqaruv paneli:** Android Telemetry Redaction, Android Telemetry-ni kuzatib boring
  Umumiy koʻrinish, redaktsiyaga muvofiqlik va eksportchi salomatligi asboblar paneli. Qo‘lga olish
  voqea yozuvlari uchun skrinshotlar va har qanday tuz-versiya yoki bekor qilish izohi
  hodisani yopishdan oldin token og'ishlari.
- **Muvofiqlashtirish:** Eksportchi masalalari bo'yicha Release Engineering bilan shug'ullanish, Muvofiqlik
  bekor qilish/PII savollari va Sev 1 hodisalari uchun dastur rahbari.

### 4.1 Eskalatsiya oqimi

Android hodisalari Android bilan bir xil jiddiylik darajalari yordamida triajlanadi
Qo'llab-quvvatlash o'yin kitobi (§2.1). Quyidagi jadvalda kim va qanday sahifalash kerakligi ko'rsatilgan
Tezda har bir javob beruvchi ko'prikka qo'shilishi kutilmoqda.| Jiddiylik | Ta'sir | Birlamchi javob beruvchi (≤5min) | Ikkilamchi eskalatsiya (≤10min) | Qo'shimcha bildirishnomalar | Eslatmalar |
|----------|--------|--------------------------------|--------------------------------|--------------------------|-------|
| Sev1 | Mijoz bilan bog'liq uzilishlar, maxfiylik buzilishi yoki ma'lumotlar sizib chiqishi | Qo'ng'iroq bo'yicha Android telemetriyasi (`android-telemetry-primary`) | Torii qo'ng'iroq bo'yicha + Dastur rahbari | Muvofiqlik + SRE boshqaruvi (`#sre-governance`), bosqichma-bosqich klaster egalari (`#android-staging`) | Darhol urush xonasini ishga tushiring va buyruqlar jurnali uchun umumiy hujjatni oching. |
| Sev2 | Filoning degradatsiyasi, noto'g'ri foydalanishni bekor qilish yoki uzoq vaqt davomida qayta o'ynash kechikishi | Qo'ng'iroq bo'yicha Android telemetriyasi | Android Foundations TL + Hujjatlar/Yordam menejeri | Dastur rahbari, reliz muhandislik aloqasi | Agar bekor qilish 24 soatdan oshsa, Muvofiqlik darajasiga o'ting. |
| Sev3 | Yagona ijarachi masalasi, laboratoriya mashg'ulotlari yoki maslahat ogohlantirishi | Yordamchi muhandis | Qo'ng'iroq bo'yicha Android (ixtiyoriy) | Hujjatlar/Ogohlantirish uchun yordam | Qo'llanish doirasi kengaysa yoki bir nechta ijarachilarga ta'sir qilsa, Sev2 ga aylantiring. |

| Oyna | Harakat | Ega(lar)i | Dalillar/eslatmalar |
|--------|--------|----------|----------------|
| 0–5 daqiqa | PagerDuty-ni tan oling, hodisa qo'mondoni (IC) tayinlang va `incident/YYYY-MM-DD-android-telemetry.md` ni yarating. `#android-sdk-support` da havolani va bir qatorli holatini qoldiring. | Qo'ng'iroq bo'yicha SRE / Yordam muhandisi | Boshqa hodisalar jurnallari bilan bir qatorda PagerDuty ack + hodisa stubining skrinshoti. |
| 5–15 daqiqa | `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` ni ishga tushiring va xulosani voqea hujjatiga joylashtiring. Ping Android Observability TL (Haruka Yamamoto) va qo'llab-quvvatlash yetakchisi (Priya Deshpande) real vaqtda uzatish uchun. | IC + Android Observability TL | CLI chiqish JSON-ni biriktiring, boshqaruv paneli URL-manzillari ochilganiga e'tibor bering va diagnostika kimga tegishli ekanligini belgilang. |
| 15–25 daqiqa | `android-telemetry-stg` da koʻpaytirish uchun klaster egalarini (kuzatish uchun Xaruka Yamamoto, SRE uchun Liam O'Konnor) jalb qiling. Semptomlar paritetini tasdiqlash uchun `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` bilan urug'larni yuklang va Pixel + emulyatoridan navbat qoldiqlarini oling. | Staging klaster egalari | Sanitatsiya qilingan `pending.queue` + `PendingQueueInspector` chiqishini voqea jildiga yuklang. |
| 25–40 daqiqa | Qayta belgilash, Torii kamaytirish yoki StrongBoxni qayta tiklash haqida qaror qabul qiling. Agar PII taʼsir qilish yoki deterministik boʻlmagan xeshlashda gumon qilinsa, `#compliance-alerts` orqali Muvofiqlik (Sofiya Martins, Daniel Park) sahifasiga oʻting va xuddi shu voqea doirasida dastur rahbariga xabar bering. | IC + Muvofiqlik + Dastur rahbari | Havolani bekor qilish tokenlari, Norito manifestlari va tasdiqlash sharhlari. |
| ≥40min | 30 daqiqalik holat yangilanishlarini taqdim eting (PagerDuty eslatmalari + `#android-sdk-support`). Agar hali faol bo'lmasa, jangovar xona ko'prigini rejalashtiring, ETAni yumshatishni hujjatlang va Release Engineering (Aleksey Morozov) kollektor/SDK artefaktlarini aylantirish uchun kutish rejimida ekanligiga ishonch hosil qiling. | IC | Vaqt tamgʻasi boʻlgan yangilanishlar va voqea faylida saqlanadigan qaror jurnallari va keyingi haftalik yangilanish vaqtida `status.md` da umumlashtiriladi. |- Barcha eskalatsiyalar voqea hujjatida Android qoʻllab-quvvatlash kitobidagi “Egasi / Keyingi yangilash vaqti” jadvalidan foydalangan holda aks ettirilishi kerak.
- Agar boshqa voqea allaqachon ochiq bo'lsa, mavjud urush xonasiga qo'shiling va yangisini yaratish o'rniga Android kontekstini qo'shing.
- Voqea runbook bo'shliqlariga tegsa, AND7 JIRA eposida keyingi vazifalarni yarating va `telemetry-runbook` tegini yarating.

## 5. Xaos va Tayyorgarlik mashqlari

- Batafsil ko'rsatilgan stsenariylarni bajaring
  `docs/source/sdk/android/telemetry_chaos_checklist.md` har chorakda va undan oldin
  asosiy relizlar. Natijalarni laboratoriya hisoboti shabloni bilan qayd qiling.
- Dalillarni (skrinshotlar, jurnallar) ostida saqlang
  `docs/source/sdk/android/readiness/screenshots/`.
- `telemetry-lab` yorlig'i bilan AND7 dostonidagi tuzatish chiptalarini kuzatib boring.
- Stsenariy xaritasi: C1 (redaktsiya xatosi), C2 (bekor qilish), C3 (eksportchining ishdan chiqishi), C4
  (Drift konfiguratsiyasi bilan `run_schema_diff.sh` yordamida farqli eshik sxemasi), C5
  (`generate_android_load.sh` orqali qurilma profilining egri chizig'i), C6 (Torii kutish vaqti tugadi
  + navbatni takrorlash), C7 (attestatsiyani rad etish). Bu raqamlashni moslab turing
  `telemetry_lab_01.md` va mashqlarni qo'shishda tartibsizlik nazorat ro'yxati.

### 5.1 Redaction drift & override drill (C1/C2)

1. Xeshlash xatosini orqali kiriting
   `scripts/telemetry/inject_redaction_failure.sh` va PagerDuty-ni kuting
   ogohlantirish (`android.telemetry.redaction.failure`). dan CLI chiqishini yozib oling
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` uchun
   voqea yozuvi.
2. `--clear` yordamida nosozlikni o'chiring va ogohlantirishning ichida hal qilinishini tasdiqlang
   10 daqiqa; tuz/vakolat panellarining Grafana skrinshotlarini biriktiring.
3. Imzolangan bekor qilish so'rovini yarating
   `docs/examples/android_override_request.json`, bilan qo'llang
   `scripts/android_override_tool.sh apply` va o'tkazilmagan namunani tomonidan tekshiring
   bosqichma-bosqich eksportchining foydali yukini tekshirish (
   `android.telemetry.redaction.override`).
4. `scripts/android_override_tool.sh revoke --token <token>` yordamida bekor qilishni bekor qiling,
   bekor qilish tokeni xesh va chipta havolasini qo'shing
   `docs/source/sdk/android/telemetry_override_log.md` va JSON-ni sindiring
   `docs/source/sdk/android/readiness/override_logs/` ostida. Bu yopadi
   C2 stsenariysi tartibsizlik nazorat ro'yxatida va boshqaruv dalillarini yangilab turadi.

### 5.2 Eksportchining ishdan chiqishi va navbatni takrorlash mashqlari (C3/C6)1. Staging kollektorini kichiklashtiring (`kubectl shkalasi
   deploy/android-otel-collector --replicas=0`) eksportchini simulyatsiya qilish uchun
   jigarrang. Status CLI orqali bufer ko'rsatkichlarini kuzatib boring va ogohlantirishlarning yonishini tasdiqlang
   15 daqiqalik belgi.
2. Kollektorni qayta tiklang, qoldiq drenajini tasdiqlang va kollektor jurnalini arxivlang
   takror ijro tugallanishini ko'rsatadigan parcha.
3. Staging Pixel va emulyatorda ScenarioC6: o'rnatishga amal qiling
   `examples/android/operator-console`, samolyot rejimini o'zgartiring, demoni yuboring
   oʻtkazmalarni oʻtkazing, keyin samolyot rejimini oʻchiring va navbat chuqurligi koʻrsatkichlarini tomosha qiling.
4. Har bir kutilayotgan navbatni torting (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :yadro:sinflar >/dev/null`), and run `java -cp qurish/sinflar
   org.hyperledger.iroha.android.tools.PendingQueueInspector --fayl
   /tmp/.queue --json > queue-replay-.json`. Ilova dekodlangan
   konvertlar va laboratoriya jurnaliga xeshlarni takrorlang.
5. Xaos hisobotini eksportchi uzilish muddati, oldin/keyin navbat chuqurligi bilan yangilang,
   va `android_sdk_offline_replay_errors` 0 qolganligini tasdiqlash.

### 5.3 Staging klaster xaos skripti (android-telemetry-stg)

Sahnalashtiruvchi klaster egalari Xaruka Yamamoto (Android Observability TL) va Liam O'Konnor
(SRE) har doim mashq rejalashtirilganda ushbu skriptga amal qiling. Ketma-ketlik saqlanib qoladi
ishtirokchilar telemetriya xaos nazorat ro'yxatiga mos kelishini kafolatlashdi
artefaktlar boshqaruv uchun qo'lga olinadi.

**Ishtirokchilar**

| Rol | Mas'uliyat | Aloqa |
|------|------------------|---------|
| Android qo'ng'iroq bo'yicha IC | Matkapni boshqaradi, PagerDuty qaydlarini muvofiqlashtiradi, buyruqlar jurnaliga egalik qiladi | PagerDuty `android-telemetry-primary`, `#android-sdk-support` |
| Staging klaster egalari (Haruka, Liam) | Darvoza oynalarini oʻzgartirish, `kubectl` amallarini ishga tushirish, snapshot klaster telemetriyasi | `#android-staging` |
| Hujjatlar/Yordam menejeri (Priya) | Dalillarni yozib oling, laboratoriya nazorat ro'yxatini kuzatib boring, keyingi chiptalarni nashr eting | `#docs-support` |

**Parvozdan oldin muvofiqlashtirish**

- Mashqdan 48 soat oldin, rejalashtirilganlar ro'yxatini o'zgartirish so'rovini yuboring
  stsenariylar (C1–C7) va klaster egalari uchun havolani `#android-staging`-ga joylashtiring
  ziddiyatli joylashtirishlarni bloklashi mumkin.
- Eng so'nggi `ClientConfig` xesh va `kubectl --kontekst staging pods olish to'plamini to'plang
  Asosiy holatni o'rnatish uchun -n android-telemetry-stg` chiqishi, keyin esa saqlang
  ikkalasi ham `docs/source/sdk/android/readiness/labs/reports/<date>/` ostida.
- Qurilmaning qamrovini tasdiqlang (Pixel + emulyator) va ishonch hosil qiling
  `ci/run_android_tests.sh` laboratoriya davomida ishlatiladigan asboblarni tuzdi
  (`PendingQueueInspector`, telemetriya injektorlari).

**Ijro nazorat punktlari**

- `#android-sdk-support` da "tartibsizlik boshlanishi" e'lon qiling, ko'prik yozishni boshlang,
  va `docs/source/sdk/android/telemetry_chaos_checklist.md` ko'rinadigan qilib turing
  har bir amr kotib uchun rivoyat qilinadi.
- Har bir injektor harakatini sahnalashtiruvchi egasi aks ettirsin (`kubectl scale`, eksportchi
  qayta ishga tushiriladi, generatorlarni yuklaydi) shuning uchun ham Kuzatish, ham SRE qadamni tasdiqlaydi.
- `scripts/telemetry/check_redaction_status.py dan olingan ma'lumotlarni yozib oling
  --status-url https://android-telemetry-stg/api/redaction/status` har biridan keyin
  stsenariyni yarating va uni voqea hujjatiga joylashtiring.

**Qayta tiklash**- Barcha injektorlar tozalanmaguncha ko'prikni tark etmang (`inject_redaction_failure.sh --clear`,
  `kubectl scale ... --replicas=1`) va Grafana asboblar paneli yashil holatni ko'rsatadi.
- Hujjatlar/Yordam arxivlari navbatlari, CLI jurnallari va skrinshotlar ostida
  `docs/source/sdk/android/readiness/screenshots/<date>/` va arxivni belgilang
  o'zgartirish so'rovi yopilishidan oldin nazorat ro'yxati.
- Har qanday stsenariy uchun `telemetry-chaos` yorlig'i bilan kuzatuv chiptalarini ro'yxatdan o'tkazing
  muvaffaqiyatsiz yoki kutilmagan ko'rsatkichlarni ishlab chiqaring va ularga `status.md` da havola qiling
  keyingi haftalik ko'rib chiqish paytida.

| Vaqt | Harakat | Ega(lar)i | Artefakt |
|------|--------|----------|----------|
| T−30min | `android-telemetry-stg` sog'lig'ini tekshiring: `kubectl --context staging get pods -n android-telemetry-stg`, kutilayotgan yangilanishlar yo'qligini tasdiqlang va kollektor versiyalariga e'tibor bering. | Haruka | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T−20min | Urug'ning asosiy yuki (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) va stdoutni tortib oling. | Liam | `readiness/labs/reports/<date>/load-generator.log` |
| T−15min | `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` ni `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md` ga nusxalash, ishga tushiriladigan stsenariylarni ro‘yxatlash (C1–C7) va yozuvchilarni tayinlash. | Priya Deshpande (Qo'llab-quvvatlash) | Repetitsiya boshlanishidan oldin sodir bo'lgan voqea belgisi. |
| T−10min | Pixel + emulyatorini onlayn tasdiqlang, so'nggi SDK o'rnatilgan va `ci/run_android_tests.sh` `PendingQueueInspector` ni kompilyatsiya qilgan. | Haruka, Liam | `readiness/screenshots/<date>/device-checklist.png` |
| T−5min | Masshtab koʻprigini ishga tushiring, ekran yozishni boshlang va `#android-sdk-support` da “tartibsizlik boshlanishi”ni eʼlon qiling. | IC / Hujjatlar / Yordam | Yozuv `readiness/archive/<month>/` ostida saqlangan. |
| +0min | `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (odatda C2 + C6) dan tanlangan stsenariyni bajaring. Laboratoriya qo'llanmasini ko'rinadigan holda saqlang va ular sodir bo'lganda buyruq chaqiruvlarini chaqiring. | Haruka haydaydi, Liam natijalarni aks ettiradi | Voqea fayliga real vaqtda biriktirilgan jurnallar. |
| +15 daqiqa | Koʻrsatkichlarni yigʻish uchun pauza qiling (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) va Grafana skrinshotlarini oling. | Haruka | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25 daqiqa | AOK qilingan nosozliklarni tiklang (`inject_redaction_failure.sh --clear`, `kubectl scale ... --replicas=1`), navbatlarni takrorlang va ogohlantirishlarni yopishni tasdiqlang. | Liam | `readiness/labs/reports/<date>/recovery.log` |
| +35 daqiqa | Xulosa: voqea hujjatini har bir stsenariy bo'yicha o'tish/muvaffaqiyatsiz yangilash, kuzatuvlarni ro'yxatga olish va artefaktlarni git-ga surish. Arxivni tekshirish roʻyxatini toʻldirish mumkinligi haqida Docs/Supportga xabar bering. | IC | Hodisa hujjati yangilandi, `readiness/archive/<month>/checklist.md` belgilandi. |

- Eksportchilar sog'lom bo'lgunga qadar va barcha ogohlantirishlar o'chirilgunga qadar sahna egalarini ko'prikda saqlang.
- Navbatdagi xom ashyolarni `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` da saqlang va hodisalar jurnalida ularning xeshlariga havola qiling.
- Agar stsenariy bajarilmasa, darhol `telemetry-chaos` etiketli JIRA chiptasini yarating va uni `status.md` dan oʻzaro bogʻlang.
- Avtomatlashtirish yordamchisi: `ci/run_android_telemetry_chaos_prep.sh` yuk generatorini, holat suratlarini va navbatni eksport qilish sanitariya-tesisatini o'rab oladi. Skript har bir navbat faylini nusxalashi, `<label>.sha256` chiqarishi va `PendingQueueInspector` ni ishga tushirishi uchun `PendingQueueInspector` `PendingQueueInspector` va `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (va hokazo.) `PendingQueueInspector` ni oʻrnating. `ANDROID_PENDING_QUEUE_INSPECTOR=false` dan faqat JSON emissiyasini o'tkazib yuborish kerak bo'lganda foydalaning (masalan, JDK mavjud emas). **Yordamchini ishga tushirishdan oldin har doim kutilgan tuz identifikatorlarini eksport qiling** `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` va `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` ni o'rnating, shuning uchun o'rnatilgan `check_redaction_status.py` qo'ng'iroqlari, agar olingan telemetriya Rust asosiy chizig'idan farq qilsa, tezda muvaffaqiyatsiz bo'ladi.

## 6. Hujjatlashtirish va yoqish- **Operatorni yoqish to'plami:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  runbook, telemetriya siyosati, laboratoriya qo'llanmasi, arxiv nazorat ro'yxati va bilimlarni bog'laydi
  bitta AND7-tayyor paketga tekshiradi. SREni tayyorlashda unga murojaat qiling
  boshqaruvni oldindan o'qish yoki choraklik yangilanishni rejalashtirish.
- **Yoqish seanslari:** 60 daqiqalik yoqish yozuvi 2026-02-18 da ishlaydi
  har chorakda yangilanishlar bilan. Materiallar ostida yashaydi
  `docs/source/sdk/android/readiness/`.
- **Bilimlarni tekshirish:** Xodimlar tayyorlik shakli orqali ≥90% ball to'plashlari kerak. Do'kon
  natijalari `docs/source/sdk/android/readiness/forms/responses/`.
- **Yangilanishlar:** Telemetriya sxemalari, asboblar paneli yoki siyosatlarni bekor qilganda
  o'zgartiring, ushbu runbook, qo'llab-quvvatlash o'yin kitobi va `status.md`ni bir xilda yangilang
  PR.
- **Haftalik ko'rib chiqish:** Har bir Rust-reliz nomzodidan so'ng (yoki kamida haftada bir marta) tekshiring
  `java/iroha_android/README.md` va bu runbook hali ham joriy avtomatlashtirishni aks ettiradi,
  armatura aylanish tartib-qoidalari va boshqaruvning taxminlari. Sharhni yozib oling
  `status.md`, shuning uchun Foundations muhim bosqich auditi hujjatlarning yangiligini kuzatishi mumkin.

## 7. StrongBox attestatsiya jabduqlari- **Maqsad:** Qurilmalarni reklama qilishdan oldin apparat ta'minoti bilan ta'minlangan attestatsiya to'plamlarini tasdiqlang.
  StrongBox puli (AND2/AND6). Jabduqlar olingan sertifikat zanjirlarini iste'mol qiladi va ularni tekshiradi
  ishlab chiqarish kodi bajaradigan siyosatdan foydalangan holda ishonchli ildizlarga qarshi.
- **Ma'lumotnoma:** To'liq ma'lumot uchun `docs/source/sdk/android/strongbox_attestation_harness_plan.md` ga qarang
  Capture API, taxallusning hayot aylanishi, CI/Buildkite simlari va egalik matritsasi. Ushbu rejani shunday deb hisoblang
  yangi laboratoriya texniklarini ishga qabul qilish yoki moliya/muvofiqlik artefaktlarini yangilashda haqiqat manbai.
- **Ish jarayoni:**
  1. Qurilmada attestatsiya to‘plamini yig‘ing (taxallus, `challenge.hex` va `chain.pem`
     barg→ ildiz tartibi) va uni ish stantsiyasiga nusxalash.
  2. `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root  dasturini ishga tushiring.
     [--trust-root-dir ] --require-strongbox --output ` tegishli kod yordamida
     Google/Samsung ildizi (kataloglar butun sotuvchi paketlarini yuklash imkonini beradi).
  3. JSON xulosasini xom attestatsiya materiallari bilan birga arxivlang
     `artifacts/android/attestation/<device-tag>/`.
- **To‘plam formati:** `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` ga rioya qiling
  kerakli fayl tartibi uchun (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **Ishonchli ildizlar:** Qurilma laboratoriyasi sirlari do‘konidan sotuvchi tomonidan taqdim etilgan PEMlarni oling; bir nechta o'tish
  `--trust-root` argumentlari yoki `--trust-root-dir` ni langarlarni saqlaydigan katalogga belgilang
  zanjir Google bo'lmagan langarda tugaydi.
- **CI jabduqlari:** Arxivlangan paketlarni paketli tekshirish uchun `scripts/android_strongbox_attestation_ci.sh` dan foydalaning
  laboratoriya mashinalarida yoki CI yuguruvchilarda. Skript `artifacts/android/attestation/**` ni skanerlaydi va uni chaqiradi
  hujjatlashtirilgan fayllarni o'z ichiga olgan har bir katalog uchun jabduqlar, yangilangan `result.json` yozish
  xulosalar joyida.
- **CI qatori:** Yangi paketlarni sinxronlashtirgandan so'ng, Buildkite-da belgilangan qadamni bajaring
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  Ish `scripts/android_strongbox_attestation_ci.sh` ni bajaradi, bilan xulosa hosil qiladi
  `scripts/android_strongbox_attestation_report.py`, hisobotni `artifacts/android_strongbox_attestation_report.txt` ga yuklaydi,
  va qurilishni `android-strongbox/report` sifatida izohlaydi. Har qanday nosozliklarni darhol tekshirib ko'ring va
  qurilma matritsasidan qurish URL manzilini bog'lang.
- **Hisobot:** JSON chiqishini boshqaruv tekshiruvlariga ilova qiling va qurilma matritsasi yozuvini yangilang
  Attestatsiya sanasi bilan `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.
- **Soxta mashq:** Uskunalar mavjud bo'lmaganda, `scripts/android_generate_mock_attestation_bundles.sh` ni ishga tushiring
  (`scripts/android_mock_attestation_der.py` dan foydalanadi) deterministik test toʻplamlarini va umumiy soxta ildizni zarb qilish uchun, CI va docs jabduqni oxirigacha ishlatishi mumkin.
- **Kod ichidagi himoya panjaralari:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` boʻsh va eʼtirozli boʻlganlarni qamrab oladi
  attestatsiya regeneratsiyasi (StrongBox/TEE metamaʼlumotlari) va `android.keystore.attestation.failure` chiqaradi
  muammoga mos kelmasligi sababli kesh/temetriya regressiyalari yangi to'plamlarni jo'natishdan oldin ushlanadi.

## 8. Kontaktlar

- **Qo'ng'iroq bo'yicha muhandislikni qo'llab-quvvatlash:** `#android-sdk-support`
- **SRE boshqaruvi:** `#sre-governance`
- **Hujjatlar/Yordam:** `#docs-support`
- **Eskalatsiya daraxti:** Android qo'llab-quvvatlash bo'limiga qarang §2.1

## 9. Muammolarni bartaraf etish stsenariylariYo'l xaritasi AND7-P2 elementi qayta-qayta sahifaga tushadigan uchta hodisa sinfini chaqiradi
Android qo'ng'irog'i: Torii/tarmoq vaqtlari, StrongBox attestatsiyasidagi nosozliklar va
`iroha_config` manifest drifti. Ariza topshirishdan oldin tegishli nazorat ro'yxatidan o'ting
Sev1/2 kuzatish va dalillarni `incident/<date>-android-*.md` da arxivlash.

### 9.1 Torii va tarmoqning kutish vaqti

**Signallar**

- `android_sdk_submission_latency`, `android_sdk_pending_queue_depth` da ogohlantirishlar,
  `android_sdk_offline_replay_errors` va Torii `/v1/pipeline` xato darajasi.
- `operator-console` vidjetlari (misollar/android) toʻxtab qolgan navbatni koʻrsatuvchi yoki
  qayta urinishlar eksponentsial orqaga yopishib qolgan.

**Darhol javob**

1. PagerDuty (`android-networking`) ni tan oling va hodisalar jurnalini boshlang.
2. Grafana suratlarini (yuborish kechikishi + navbat chuqurligi) qamrab oladi.
   oxirgi 30 daqiqa.
3. Qurilma jurnallaridan (`ConfigWatcher`) faol `ClientConfig` xeshini yozib oling
   qayta yuklash muvaffaqiyatli yoki muvaffaqiyatsiz bo'lganda manifest dayjestini chop etadi).

**Diagnostika**

- **Navbat holati:** Sozlangan navbat faylini sahnalash qurilmasidan yoki qurilmadan tortib oling
  emulyator (`adb qobig'i  mushuk fayllari/pending.queue> sifatida ishlaydi
  /tmp/pending.queue`). bilan konvertlarni dekodlash
  `OfflineSigningEnvelopeCodec` da tavsiflanganidek
  tasdiqlash uchun `docs/source/sdk/android/offline_signing.md#4-queueing--replay`
  kechikish operator kutganlariga mos keladi. dekodlangan xeshlarni biriktiring
  voqea.
- **Xesh inventarizatsiyasi:** Navbat faylini yuklab olgach, inspektor yordamchisini ishga tushiring
  hodisa artefaktlari uchun kanonik xesh/taxalluslarni olish uchun:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  Hodisaga `queue-inspector.json` va chiroyli bosilgan stdoutni biriktiring
  va uni D stsenariysi uchun AND7 laboratoriya hisobotidan bog'lang.
- **Torii ulanishi:** SDK ni istisno qilish uchun HTTP transport jabduqlarini mahalliy sifatida ishga tushiring
  regressiyalar: `ci/run_android_tests.sh` mashqlari
  `HttpClientTransportTests`, `HttpClientTransportHarnessTests` va
  `ToriiMockServerTests`. Bu yerdagi nosozliklar a emas, balki mijoz xatosini bildiradi
  Torii uzilish.
- **Noto'g'ri in'ektsiya repetisiyasi:** Pixel (StrongBox) va AOSP sahnasida
  emulyator, kutilayotgan navbat o'sishini ko'paytirish uchun ulanishni almashtiring:
  `adb shell cmd connectivity airplane-mode enable` → ikkita demoni yuboring
  operator-konsol orqali tranzaktsiyalar → `adb shell cmd ulanish samolyot-rejimi
  disable` → verify the queue drains and `android_sdk_offline_replay_errors`
  qoladi 0. Qayta o'tkazilgan tranzaksiyalarning rekord xeshlari.
- **Ogohlantirish pariteti:** Eshiklarni sozlashda yoki Torii o'zgargandan so'ng, bajaring
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` shuning uchun Prometheus qoidalari qoladi
  asboblar paneli bilan moslashtirilgan.

**Qayta tiklash**

1. Agar Torii buzilgan bo'lsa, Torii qo'ng'irog'ini ishga tushiring va qayta o'ynashni davom eting.
   navbat bir marta `/v1/pipeline` trafikni qabul qiladi.
2. Ta'sir qilingan mijozlarni faqat imzolangan `iroha_config` manifestlari orqali qayta sozlang. The
   `ClientConfig` issiq qayta yuklash kuzatuvchisi voqeadan oldin muvaffaqiyat jurnalini chiqarishi kerak
   yopishi mumkin.
3. Hodisani takrorlashdan oldin/keyin navbat hajmi va xeshlari bilan yangilang
   har qanday to'xtatilgan tranzaktsiyalar.

### 9.2 StrongBox va attestatsiyadagi xatolar

**Signallar**- `android_sdk_strongbox_success_rate` yoki bo'yicha ogohlantirishlar
  `android.keystore.attestation.failure`.
- `android.keystore.keygen` telemetriyasi endi so'ralganni yozib oladi
  `KeySecurityPreference` va foydalanilgan marshrut (`strongbox`, `hardware`,
  `software`) StrongBox afzalligi tushganda `fallback=true` bayrog'i bilan
  TEE/dasturiy ta'minot. STRONGBOX_REQUIRED so‘rovlari endi jimgina emas, tez bajarilmaydi
  TEE kalitlarini qaytarish.
- `KeySecurityPreference.STRONGBOX_ONLY` qurilmalariga havola qilingan chiptalarni qo'llab-quvvatlash
  dasturiy ta'minot kalitlariga qaytish.

**Darhol javob**

1. PagerDuty (`android-crypto`) ni tan oling va ta'sirlangan taxallus yorlig'ini oling
   (tuzlangan hash) ortiqcha qurilma profili paqir.
2. Qurilma uchun attestatsiya matritsasi yozuvini tekshiring
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` va
   oxirgi tasdiqlangan sanani yozib oling.

**Diagnostika**

- **To‘plamni tekshirish:** Ishga tushirish
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  Arxivlangan attestatsiyada nosozlik qurilmadan kelib chiqqanligini tasdiqlash uchun
  noto'g'ri konfiguratsiya yoki siyosat o'zgarishi. Yaratilgan `result.json` ni biriktiring.
- **Challenge regen:** Qiyinchiliklar keshda saqlanmaydi. Har bir chaqiruv so'rovi yangisini tiklaydi
  `(alias, challenge)` tomonidan attestatsiya va keshlar; muammosiz qo'ng'iroqlar keshni qayta ishlatadi. Qo'llab-quvvatlanmaydi
- **CI supurgi:** Har bir `scripts/android_strongbox_attestation_ci.sh` ni bajaring
  saqlangan to'plam qayta tasdiqlanadi; bu kiritilgan tizimli muammolardan himoya qiladi
  yangi ishonch langarlari tomonidan.
- **Device matkap:** StrongBox bo'lmagan apparatda (yoki emulyatorni majburlash orqali),
  SDK ni faqat StrongBox talab qiladigan qilib sozlang, demo tranzaksiyani yuboring va tasdiqlang
  telemetriya eksportchisi `android.keystore.attestation.failure` hodisasini chiqaradi
  kutilgan sabab bilan. Bunga ishonch hosil qilish uchun StrongBox-ga mos keladigan Pixel-da takrorlang
  baxtli yo'l yashil bo'lib qoladi.
- **SDK regressiya tekshiruvi:** `ci/run_android_tests.sh` dasturini ishga tushiring va to‘lang
  attestatsiyaga yo'naltirilgan to'plamlarga e'tibor (`AndroidKeystoreBackendDetectionTests`,
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  kesh/qidiruv ajratish uchun `KeystoreKeyProviderTests`). Bu erda muvaffaqiyatsizliklar
  mijoz tomonidagi regressiyani ko'rsatadi.

**Qayta tiklash**

1. Agar sotuvchi sertifikatlarni aylantirgan bo'lsa yoki sertifikatlash paketlarini qayta tiklang
   qurilma yaqinda katta OTA oldi.
2. Yangilangan to'plamni `artifacts/android/attestation/<device>/` va
   matritsa yozuvini yangi sana bilan yangilang.
3. Agar StrongBox ishlab chiqarishda mavjud bo'lmasa, unda bekor qilish ish jarayoniga rioya qiling
   3-bo'lim va qayta tiklash muddatini hujjatlash; uzoq muddatli yumshatishni talab qiladi
   qurilmani almashtirish yoki sotuvchini tuzatish.

### 9.2a Deterministik eksportni tiklash

- **Formatlar:** Joriy eksportlar v3 (eksport uchun tuz/kesish + Argon2id, sifatida qayd etilgan)
- **Parol iborasi siyosati:** v3 ≥12 ta belgidan iborat parol iboralarini qo‘llaydi. Agar foydalanuvchilar qisqaroq etkazib berishsa
  parollar, ularga mos parol bilan qayta eksport qilishni ko'rsating; v0/v1 importlari
  ozod qilingan, lekin importdan so'ng darhol v3 sifatida qayta o'ralgan bo'lishi kerak.
- **Buzg'unchilik/qayta foydalanish muhofazasi:** Dekoderlar nol/qisqa tuz yoki bo'lmagan uzunliklarni rad etadi va takrorlanadi
  tuz/nonce juftliklar `salt/nonce reuse` xatolar sifatida yuzaga keladi. Tozalash uchun eksportni qayta tiklang
  qo'riqchi; qayta ishlatishga majburlamang.
  Kalitni qayta tiklash uchun `SoftwareKeyProvider.importDeterministic(...)`, keyin
  `exportDeterministic(...)` v3 to'plamini chiqaradi, shunda ish stoli asboblari yangi KDFni yozib oladi
  parametrlari.### 9.3 Manifest va konfiguratsiya nomuvofiqligi

**Signallar**

- `ClientConfig` qayta yuklashda xatoliklar, mos kelmaydigan Torii xost nomlari yoki telemetriya
  AND7 diff vositasi tomonidan belgilangan sxema farqlari.
- Operatorlar bir xil qurilmalarda turli xil qayta urinish/orqaga o'chirish tugmalari haqida xabar berishadi
  flot.

**Darhol javob**

1. Android jurnallarida chop etilgan `ClientConfig` dayjestini yozib oling va
   reliz manifestidan kutilgan dayjest.
2. Taqqoslash uchun ishlaydigan tugun konfiguratsiyasini o'chirib tashlang:
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

**Diagnostika**

- **Sxema farqi:** `scripts/telemetry/run_schema_diff.sh --android-config-ni ishga tushiring
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  Norito farq hisobotini yaratish uchun Prometheus matn faylini yangilang va ilova qiling
  JSON artefakt plyus koʻrsatkichlari hodisaga dalil va AND7 telemetriyaga tayyorlik jurnali.
- **Manifest tekshiruvi:** `iroha_cli runtime capabilities` (yoki ish vaqti) dan foydalaning
  audit buyrug'i) tugunning reklama qilingan kripto/ABI xeshlarini olish va ta'minlash
  ular mobil manifestga mos keladi. Mos kelmaslik tugunning orqaga qaytarilganligini tasdiqlaydi
  Android manifestini qayta chiqarmasdan.
- **SDK regressiya tekshiruvi:** `ci/run_android_tests.sh` qamrab oladi
  `ClientConfigNoritoRpcTests`, `ClientConfig.ValidationTests` va
  `HttpClientTransportStatusTests`. Xatolar yuborilgan SDK qila olmasligini bildiradi
  joriy qilingan manifest formatini tahlil qilish.

**Qayta tiklash**

1. Manifestni ruxsat etilgan quvur liniyasi orqali qayta yarating (odatda
   `iroha_cli runtime Capabilities` → imzolangan Norito manifest → konfiguratsiya paketi) va
   uni operator kanali orqali qayta joylashtiring. Hech qachon `ClientConfig` ni tahrirlamang
   qurilmada bekor qiladi.
2. Tuzatilgan manifest tushgandan soʻng, `ConfigWatcher` “qayta yuklash OK”ni koʻring.
   har bir flot darajasida xabar va voqeani faqat telemetriyadan keyin yoping
   schema diff hisobotlar pariteti.
3. Manifest xeshini, sxema farqli artefakt yo'lini va hodisa havolasini yozib oling
   Auditorlik uchun Android bo'limi ostida `status.md`.

## 10. Operatorni yoqish bo'yicha o'quv dasturi

“Yo‘l xaritasi” bandi **AND7** takrorlanadigan o‘quv paketini talab qiladi, shuning uchun operatorlar
qo'llab-quvvatlash muhandislari va SRE telemetriya/redaktsiya yangilanishlarini qabul qilmasdan qabul qilishi mumkin
taxmin qilish. Ushbu bo'lim bilan bog'lang
O'z ichiga olgan `docs/source/sdk/android/readiness/and7_operator_enablement.md`
batafsil nazorat ro'yxati va artefakt havolalari.

### 10.1 Seans modullari (60 daqiqalik brifing)

1. **Telemetriya arxitekturasi (15 daqiqa).** Eksport qiluvchi bufer orqali o'ting,
   redaktsiya filtri va sxema diff asboblari. Namoyish
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` plus
   `scripts/telemetry/check_redaction_status.py`, shuning uchun ishtirokchilar paritet qanday ekanligini ko'rishadi
   amalga oshirildi.
2. **Runbook + chaos labs (20daq).** Ushbu runbookning 2–9-qismlarini ajratib ko‘rsating,
   `readiness/labs/telemetry_lab_01.md` dan bitta stsenariyni takrorlang va qanday qilib ko'rsating
   artefaktlarni `readiness/labs/reports/<stamp>/` ostida arxivlash.
3. **Bekor qilish + muvofiqlik ish jarayoni (10 daqiqa).** 3-bo‘limni bekor qilishni ko‘rib chiqing,
   `scripts/android_override_tool.sh` ni ko'rsatish (qo'llash/bekor qilish/dijest) va
   `docs/source/sdk/android/telemetry_override_log.md` va eng yangisini yangilang
   JSONni hazm qilish.
4. **Savol-javob / bilimlarni tekshirish (15 daqiqa).** Tezkor ma'lumot kartasidan foydalaning
   Savollarni aniqlash uchun `readiness/cards/telemetry_redaction_qrc.md`
   `readiness/and7_operator_enablement.md` da kuzatuvlarni yozib oling.### 10.2 Obyekt ritmi va egalari

| Aktiv | Kadens | Ega(lar)i | Arxiv joylashuvi |
|-------|---------|----------|------------------|
| Yozib olingan ko'rsatmalar (Zoom/Jamoalar) | Har chorakda yoki har bir tuz aylanishidan oldin | Android Observability TL + Hujjatlar/Yordam menejeri | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (yozuv + nazorat ro'yxati) |
| Slayd pastki va tezkor ma'lumot kartasi | Siyosat/runbook o'zgarganda yangilang | Hujjatlar/Yordam menejeri | `docs/source/sdk/android/readiness/deck/` va `/cards/` (PDF eksport + Markdown) |
| Bilimlarni tekshirish + davomat varaqasi | Har bir jonli seansdan keyin | Muhandislikni qo'llab-quvvatlash | `docs/source/sdk/android/readiness/forms/responses/` va `and7_operator_enablement.md` davomat bloki |
| Savol-javoblar ro'yxati / harakatlar jurnali | Rolling; har bir seansdan keyin yangilanadi | LLM (harakat qiluvchi DRI) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 Dalillar va mulohazalar davri

- Seans artefaktlarini (skrinshotlar, hodisa mashqlari, viktorina eksporti) saqlang
  Boshqaruv ikkalasini ham tekshirishi uchun tartibsizlik mashqlari uchun ishlatiladigan bir xil sanali katalog
  tayyorlik izlari birgalikda.
- Seans tugagach, havolalar bilan `status.md` (Android bo'limi) yangilang.
  arxiv katalogini oching va har qanday ochiq kuzatuvlarga e'tibor bering.
- Jonli savol-javobdagi ajoyib savollar muammo yoki hujjatga aylantirilishi kerak
  bir hafta ichida so'rovlarni qabul qilish; da yo'l xaritasi epiklariga (AND7/AND8) murojaat qiling
  chipta tavsifi, shuning uchun egalar bir xilda qoladilar.
- SRE sinxronizatsiyasi arxiv nazorat ro'yxatini va ro'yxatda keltirilgan sxema farqi artefaktini ko'rib chiqadi
  O'quv rejasini chorak uchun yopiq deb e'lon qilishdan oldin 2.3-bo'lim.