---
lang: uz
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-12-29T18:16:35.913527+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Xaos va nosozliklarni takrorlash rejasini ulash (IOS3 / IOS7)

Ushbu o'yin kitobi IOS3/IOS7 ni qondiradigan takrorlanadigan tartibsizlik mashqlarini belgilaydi
yo'l xaritasi harakati _"qo'shma tartibsizlik mashqlarini rejalashtirish"_ (`roadmap.md:1527`). U bilan bog'lang
Connect oldindan ko'rish ish kitobi (`docs/runbooks/connect_session_preview_runbook.md`)
o'zaro SDK demolarini o'tkazishda.

## Maqsadlar va muvaffaqiyat mezonlari
- Birgalikda ulanishni qayta urinish/o'chirish siyosatini, oflayn navbat chegaralarini va
  ishlab chiqarish kodini o'zgartirmasdan, boshqariladigan nosozliklar ostida telemetriya eksportchilari.
- Deterministik artefaktlarni suratga olish (`iroha connect queue inspect` chiqishi,
  `connect.*` ko'rsatkichlari oniy tasvirlari, Swift/Android/JS SDK jurnallari) shuning uchun boshqaruv
  har bir mashqni tekshirish.
- Hamyonlar va dApps konfiguratsiya o'zgarishlariga (manifest drifts, tuz
  aylanish, attestatsiyadagi nosozliklar) kanonik `ConnectError` ni qoplash orqali
  kategoriya va redaktsiya uchun xavfsiz telemetriya hodisalari.

## Old shartlar
1. **Atrof-muhit bootstrap**
   - Torii demo stekini ishga tushiring: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Kamida bitta SDK namunasini ishga tushiring (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Asboblar**
   - SDK diagnostikasini yoqish (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     Swift-da `ConnectSessionDiagnostics`; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     Android/JS da ekvivalentlari).
   - CLI `iroha connect queue inspect --sid <sid> --metrics` hal qilinganligiga ishonch hosil qiling
     SDK tomonidan ishlab chiqarilgan navbat yo'li (`~/.iroha/connect/<sid>/state.json` va
     `metrics.ndjson`).
   - Simli telemetriya eksportchilari, shuning uchun quyidagi vaqt seriyalari ko'rinadi
     Grafana va `scripts/swift_status_export.py telemetry` orqali: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Dalillar papkalari** – `artifacts/connect-chaos/<date>/` yarating va saqlang:
   - xom jurnallar (`*.log`), ko'rsatkichlar oniy tasvirlari (`*.json`), asboblar paneli eksporti
     (`*.png`), CLI chiqishlari va PagerDuty identifikatorlari.

## Stsenariy matritsasi| ID | Xato | Inyeksiya bosqichlari | Kutilayotgan signallar | Dalil |
|----|-------|-----------------|------------------|----------|
| C1 | WebSocket uzilishi va qayta ulanish | `/v1/connect/ws` ni proksi-server orqasiga oʻrang (masalan, `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) yoki xizmatni vaqtincha bloklang (≤60s uchun `kubectl scale deploy/torii --replicas=0`). Oflayn navbatlar toʻldirilishi uchun hamyonni kadrlar yuborishda davom eting. | `connect.reconnects_total` oshadi, `connect.resume_latency_ms` ko'tariladi, lekin - O'chirish oynasi uchun asboblar panelidagi izoh.- Qayta ulanish + drenaj xabarlari bilan jurnaldan ko'chirma namunasi. |
| C2 | Oflayn navbatning to'lib ketishi / TTL muddati | Navbat chegaralarini qisqartirish uchun namunani tuzating (Swift: `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`ni `ConnectSessionDiagnostics` ichida instantiate qiling; Android/JS mos keladigan konstruktorlardan foydalanadi). dApp soʻrovlarni qoʻyishda davom etayotganda hamyonni ≥2× `retentionInterval` uchun toʻxtatib turing. | `connect.queue_dropped_total{reason="overflow"}` va `{reason="ttl"}` ortishi, `connect.queue_depth` platolari yangi chegarada, SDK yuzasi `ConnectError.QueueOverflow(limit: 4)` (yoki `.QueueExpired`). `iroha connect queue inspect` 100% da `warn/drop` suv belgilari bilan `state=Overflow`ni ko'rsatadi. | - Metrik hisoblagichlarning skrinshoti.- CLI JSON chiqishi to'lib toshgan.- `ConnectError` qatorini o'z ichiga olgan Swift/Android jurnali parchasi. |
| C3 | Manifest drift / qabul rad | Ulanish manifestini o'zgartirish hamyonlarga xizmat qiladi (masalan, `docs/connect_swift_ios.md` namunasi manifestini o'zgartiring yoki `--connect-manifest-path` bilan `--connect-manifest-path` bilan `chain_id` yoki `chain_id` yoki Grafana ko'rsatgan holda Grafana boshlang). dApp so'rovini tasdiqlang va siyosat orqali hamyonni rad etishiga ishonch hosil qiling. | Torii `/v1/connect/session` uchun `manifest_mismatch` bilan `HTTP 409`ni qaytaradi, SDKlar `ConnectError.Authorization.manifestMismatch(manifestVersion)` chiqaradi, telemetriya `connect.manifest_mismatch_total` ni oshiradi va empty qoladi (`state=Idle`). | - Torii jurnalidan ko'chirma nomuvofiqlikni aniqlashni ko'rsatmoqda.- Aniqlangan xatoning SDK skrinshoti.- Sinov davomida navbatda turgan kadrlar yo'qligini isbotlovchi ko'rsatkichlar snapshoti. |
| C4 | Kalitni aylantirish / tuzli versiyadagi zarba | Seans o'rtasida Connect tuzi yoki AEAD tugmachasini aylantiring. Dev steklarida Torii ni `CONNECT_SALT_VERSION=$((old+1))` bilan qayta ishga tushiring (`docs/source/sdk/android/telemetry_schema_diff.md` da Android redaktsiya tuzi testini aks ettiradi). Tuz aylanishi tugaguniga qadar hamyonni oflayn holatda saqlang, keyin davom eting. | Birinchi davom ettirish urinishi `ConnectError.Authorization.invalidSalt` bilan muvaffaqiyatsiz tugadi, navbatlar tozalanadi (dApp `salt_version_mismatch` sababi bilan keshlangan kadrlarni tashlaydi), telemetriya `android.telemetry.redaction.salt_version` (Android) va `swift.connect.session_event{event="salt_rotation"}` ni chiqaradi. SID yangilanishidan keyingi ikkinchi seans muvaffaqiyatli yakunlandi. | - Tuz davridan oldin/keyin bo'lgan asboblar panelidagi izoh.- Yaroqsiz tuz xatosi va keyingi muvaffaqiyatni o'z ichiga olgan jurnallar.- `iroha connect queue inspect` chiqishi `state=Stalled`, keyin esa yangi `state=Active`. || C5 | Attestatsiya / StrongBox xatosi | Android hamyonlarida `ConnectApproval` ni `attachments[]` + StrongBox attestatsiyasini kiritish uchun sozlang. dApp-ga topshirishdan oldin attestatsiya simidan foydalaning (`scripts/android_keystore_attestation.sh` `--inject-failure strongbox-simulated`) yoki attestatsiya JSON-ni o'zgartiring. | DApp `ConnectError.Authorization.invalidAttestation` bilan tasdiqlashni rad etadi, Torii muvaffaqiyatsizlik sababini qayd qiladi, eksportchilar `connect.attestation_failed_total` bilan urishadi va navbat qoidabuzar yozuvni tozalaydi. Swift/JS dApps seansni saqlab turganda xatolikni qayd qiladi. | - In'ektsiya qilingan nosozlik identifikatoriga ega jabduqlar jurnali.- SDK xato jurnali + telemetriya hisoblagichini yozib olish.- Navbat noto'g'ri ramkani olib tashlaganligidan dalolat beradi (`recordsRemoved > 0`). |

## Ssenariy tafsilotlari

### C1 — WebSocket uzilishi va qayta ulanish
1. Torii ni proksi-server (toxiproxy, Envoy yoki `kubectl port-forward`) orqasiga oʻrang.
   butun tugunni o'ldirmasdan mavjudlikni o'zgartirishingiz mumkin.
2. 45 soniyalik uzilishni ishga tushiring:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Telemetriya asboblar paneli va `scripts/swift_status_export.py telemetriyasini kuzating.
   --json-out artefacts/connect-chaos//c1_metrics.json`.
4. Chiqindilarni to'xtatish navbatining holati:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Muvaffaqiyat = bitta qayta ulanishga urinish, chegaralangan navbat o'sishi va avtomatik
   proksi-server tiklangandan keyin drenajlang.

### C2 - Oflayn navbatning to'lib ketishi / TTL muddati tugashi
1. Mahalliy qurilishlarda navbat chegaralarini qisqartirish:
   - Swift: namunangizdagi `ConnectQueueJournal` ishga tushirgichni yangilang
     (masalan, `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` o'tish.
   - Android/JS: qurishda ekvivalent konfiguratsiya ob'ektini o'tkazing
     `ConnectQueueJournal`.
2. Hamyonni (simulyator fonida yoki qurilmaning samolyot rejimi) ≥60 soniyaga to'xtatib turing
   dApp esa `ConnectClient.requestSignature(...)` qo'ng'iroqlarini chiqaradi.
3. `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) yoki JS dan foydalaning
   dalillar to'plamini eksport qilish uchun diagnostika yordamchisi (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Muvaffaqiyat = hisoblagichlarning ko'payishi, SDK sirtlari `ConnectError.QueueOverflow`
   bir marta, va hamyon davom etgandan keyin navbat tiklanadi.

### C3 - Manifest drift / qabulni rad etish
1. Qabul qilish manifestining nusxasini tuzing, masalan:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Torii ni `--connect-manifest-path /tmp/manifest_drift.json` bilan ishga tushiring (yoki
   matkap uchun docker compose/k8s konfiguratsiyasini yangilang).
3. Seansni hamyondan boshlashga urinish; HTTP 409 ni kuting.
4. Torii + SDK jurnallari va `connect.manifest_mismatch_total` ni yozib oling
   telemetriya asboblar paneli.
5. Muvaffaqiyat = navbat o'sishi holda rad, plus hamyon birgalikda ko'rsatadi
   taksonomiya xatosi (`ConnectError.Authorization.manifestMismatch`).### C4 - Kalitni aylantirish / tuz bo'shlig'i
1. Telemetriyadan joriy tuz versiyasini yozib oling:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Torii ni yangi tuz bilan qayta ishga tushiring (`CONNECT_SALT_VERSION=$((OLD+1))` yoki yangilang
   konfiguratsiya xaritasi). Qayta ishga tushirish tugamaguncha hamyonni oflayn rejimda saqlang.
3. Hamyonni davom ettiring; birinchi rezyume yaroqsiz-tuz xatosi bilan muvaffaqiyatsiz bo'lishi kerak
   va `connect.queue_dropped_total{reason="salt_version_mismatch"}` bosqichlari.
4. Seans katalogini o'chirish orqali ilovani keshlangan kadrlarni tushirishga majburlang
   (`rm -rf ~/.iroha/connect/<sid>` yoki platformaga xos kesh tozalanadi), keyin
   sessiyani yangi tokenlar bilan qayta boshlang.
5. Muvaffaqiyat = telemetriya tuz zarbasini ko'rsatadi, noto'g'ri rezyume hodisasi qayd etiladi
   bir marta va keyingi sessiya qo'lda aralashuvisiz muvaffaqiyatli o'tadi.

### C5 - Attestatsiya / StrongBox xatosi
1. `scripts/android_keystore_attestation.sh` yordamida attestatsiya toʻplamini yarating
   (imzo bitini aylantirish uchun `--inject-failure strongbox-simulated` sozlang).
2. Hamyonga ushbu paketni `ConnectApproval` API orqali biriktiring; dApp
   foydali yukni tasdiqlashi va rad etishi kerak.
3. Telemetriyani tekshiring (`connect.attestation_failed_total`, Swift/Android hodisasi
   ko'rsatkichlar) va navbat zaharlangan yozuvni tashlaganligiga ishonch hosil qiling.
4. Muvaffaqiyat = rad etish yomon ma'qullash bilan ajralib turadi, navbatlar sog'lom bo'lib qoladi,
   va attestatsiya jurnali matkap dalillari bilan saqlanadi.

## Dalillarni tekshirish ro'yxati
- `artifacts/connect-chaos/<date>/c*_metrics.json` dan eksport qiladi
  `scripts/swift_status_export.py telemetry`.
- `iroha connect queue inspect` dan CLI chiqishlari (`c*_queue.txt`).
- Vaqt belgilari va SID xeshlari bilan SDK + Torii jurnallari.
- Har bir stsenariy uchun izohli asboblar paneli skrinshotlari.
- Agar Sev1/2 signallari ishga tushirilsa, PagerDuty / hodisa identifikatorlari.

To'liq matritsani chorakda bir marta to'ldirish yo'l xaritasi darvozasini qondiradi va
Swift/Android/JS Connect ilovalari aniq javob berishini ko'rsatadi
eng yuqori xavfli ishlamay qolish usullari bo'ylab.