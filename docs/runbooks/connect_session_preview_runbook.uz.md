---
lang: uz
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d04af2ad3ae5cc6a9254236f5627850aab7c6517308e9f3a09650cbc1490168
source_last_modified: "2026-01-05T18:22:23.401292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Seansni oldindan ko'rish kitobini ulash (IOS7 / JS4)

Ushbu runbook bosqichma-bosqich, tekshirish va yakuniy protsedurani hujjatlashtiradi
**IOS7** yoʻl xaritasi bosqichlarida talab qilinganidek, oldindan koʻrish seanslarini ulash
va **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Istalgan vaqtda ushbu amallarni bajaring
siz Connect strawman (`docs/source/connect_architecture_strawman.md`) ni namoyish qilasiz,
SDK yo'l xaritalarida va'da qilingan navbat/telemetriya ilgaklarini ishlating yoki to'plang
`status.md` uchun dalil.

## 1. Parvozdan oldingi nazorat ro'yxati

| Element | Tafsilotlar | Adabiyotlar |
|------|---------|------------|
| Torii oxirgi nuqta + Ulanish siyosati | Torii asosiy URL manzilini, `chain_id` va ulanish siyosatini tasdiqlang (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). Runbook chiptasida JSON snapshotini oling. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Fikstur + ko'prik versiyalari | Siz foydalanadigan Norito armatura xesh va koʻprik qurilishiga eʼtibor bering (Swift uchun `NoritoBridge.xcframework`, JS uchun `@iroha/iroha-js` ≥ `bootstrapConnectPreviewSession` yetkazib berilgan versiya talab qilinadi). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Telemetriya asboblar paneli | `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` va boshqalar diagrammalariga kirish mumkin bo'lgan asboblar paneliga ishonch hosil qiling (Grafana Norito Prometheus suratlar). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Dalillar papkalari | `docs/source/status/swift_weekly_digest.md` (haftalik dayjest) va `docs/source/sdk/swift/connect_risk_tracker.md` (xavf kuzatuvchisi) kabi manzilni tanlang. Jurnallar, koʻrsatkichlar skrinshotlari va tasdiqlarni `docs/source/sdk/swift/readiness/archive/<date>/connect/` ostida saqlang. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Ko'rib chiqish seansini yuklash

1. **Siyosat + kvotalarni tasdiqlash.** Qo‘ng‘iroq qiling:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Agar `queue_max` yoki TTL siz rejalashtirgan konfiguratsiyadan farq qilsa, ishga tushirish muvaffaqiyatsiz tugadi.
   sinov.
2. **Deterministik SID/URI-larni yarating.** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` yordamchisi SID/URI avlodini Torii bilan bog'laydi
   sessiyani ro'yxatdan o'tkazish; Swift WebSocket qatlamini boshqarganda ham undan foydalaning.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - `register: false`-ni quruq QR/chuqur ulanish stsenariylariga sozlang.
   - Qaytarilgan `sidBase64Url`, chuqur havola URL-manzillari va `tokens` blokini saqlang.
     dalillar papkasi; boshqaruv tekshiruvi ushbu artefaktlarni kutmoqda.
3. **Sirlarni tarqating.** Deeplink URI-ni hamyon operatori bilan baham ko'ring
   (tezkor dApp namunasi, Android hamyoni yoki QA jabduqlari). Hech qachon xom tokenlarni joylashtirmang
   chatda; faollashtirish paketida hujjatlashtirilgan shifrlangan ombordan foydalaning.

## 3. Sessiyani boshqaring1. **WebSocket-ni oching.** Swift mijozlari odatda quyidagilardan foydalanadilar:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   Qo'shimcha sozlash uchun `docs/connect_swift_integration.md` havolasi (ko'prik
   import, parallel adapterlar).
2. **Tasdiqlash + belgilar oqimi.** DApps `ConnectSession.requestSignature(...)` raqamiga qo‘ng‘iroq qiladi,
   hamyonlar esa `approveSession` / `reject` orqali javob beradi. Har bir tasdiqlash jurnalga kirishi kerak
   xeshlangan taxallus + Connect boshqaruv xartiyasiga mos keladigan ruxsatlar.
3. **Mashq navbati + davom ettirish yoʻllari.** Tarmoqqa ulanishni oʻchirib qoʻying yoki toʻxtatib qoʻying.
   chegaralangan navbatni ta'minlash va kancalar jurnali yozuvlarini takrorlash uchun hamyon. JS/Android
   SDKlar `ConnectQueueError.overflow(limit)` chiqaradi /
   `.expired(ttlMs)` ular freymlarni tushirganda; Swift bir marta xuddi shunday kuzatishi kerak
   IOS7 navbatdagi iskala erlari (`docs/source/connect_architecture_strawman.md`).
   Kamida bitta qayta ulanishni yozganingizdan so'ng, ishga tushiring
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (yoki `ConnectSessionDiagnostics` tomonidan qaytarilgan eksport katalogidan o'ting) va
   ko'rsatilgan jadval/JSONni runbook chiptasiga biriktiring. CLI xuddi shunday o'qiydi
   `ConnectQueueStateTracker` ishlab chiqaradigan `state.json` / `metrics.ndjson` juftligi,
   shuning uchun boshqaruvni ko'rib chiquvchilar maxsus asboblarsiz burg'ulash dalillarini kuzatishi mumkin.

## 4. Telemetriya va kuzatuvchanlik

- **Qo'lga olish uchun ko'rsatkichlar:**
  - `connect.queue_depth{direction}` o'lchagich (qo'riqnoma chegarasidan past bo'lishi kerak).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` hisoblagichi (faqat nolga teng
    noto'g'ri in'ektsiya paytida).
  - `connect.resume_latency_ms` gistogrammasi (majburlagandan keyin p95 ni yozib oling.
    qayta ulaning).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-ga xos `swift.connect.session_event` va
    `swift.connect.frame_latency` eksporti (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Boshqaruv paneli:** Annotatsiya belgilari bilan Connect board xatcho‘plarini yangilang.
  Skrinshotlarni (yoki JSON eksportlarini) raw bilan birga dalillar jildiga biriktiring
  Telemetriya eksportchisi CLI orqali olingan OTLP/Prometheus suratlari.
- **Ogohlantirish:** Agar Sev1/2 chegaralari ishga tushsa (`docs/source/android_support_playbook.md` §5 bo'yicha),
  SDK dasturi rahbari sahifasini oching va PagerDuty hodisasi identifikatorini runbookda hujjatlang
  davom etishdan oldin chipta.

## 5. Tozalash va Orqaga qaytarish

1. **Bosqichli seanslarni o‘chirish.** Navbat chuqurligi uchun har doim oldindan ko‘rish seanslarini o‘chirib tashlang
   signallar mazmunli bo'lib qoladi:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Faqat Swift sinovlari uchun Rust/CLI yordamchisi orqali bir xil so'nggi nuqtaga qo'ng'iroq qiling.
2. **Jurnallarni tozalash.** Doimiy navbatdagi jurnallarni olib tashlang
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB do'konlari va boshqalar) shuning uchun
   keyingi yugurish toza boshlanadi. Agar kerak bo'lsa, o'chirishdan oldin fayl xeshini yozib oling
   takrorlash muammosini tuzatish.
3. **Fayl hodisasi qaydlari.** Ishni sarhisob qiling:
   - `docs/source/status/swift_weekly_digest.md` (deltalar bloki),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2 ni tozalash yoki pastga tushirish
     telemetriya o'rnatilgandan so'ng),
   - agar yangi xatti-harakatlar tasdiqlangan bo'lsa, JS SDK o'zgarishlar jurnali yoki retsepti.
4. **Muvaffaqiyatsizliklarni kuchaytirish:**
   - Inyeksion nosozliklarsiz navbatning to'lib ketishi ⇒ SDK ga qarshi xato xabari
     siyosat Torii dan ajralib chiqdi.
   - Davom etish xatolari ⇒ `connect.queue_depth` + `connect.resume_latency_ms` ni biriktiring
     voqea hisobotiga lavhalar.
   - Boshqaruvdagi nomuvofiqliklar (tokenlar qayta ishlatilgan, TTL oshib ketgan) ⇒ SDK bilan oshirish
     Dastur rahbari va keyingi tahrirda `roadmap.md` ga izoh bering.

## 6. Dalillarni tekshirish ro'yxati| Artefakt | Manzil |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Boshqaruv paneli eksporti (`connect.queue_depth` va boshqalar) | `.../metrics/` pastki papkasi |
| PagerDuty / voqea identifikatorlari | `.../notes.md` |
| Tozalashni tasdiqlash (Torii o'chirish, jurnalni tozalash) | `.../cleanup.log` |

Ushbu nazorat roʻyxatini toʻldirish “docs/runbooks yangilangan” chiqish mezoniga javob beradi
IOS7/JS4 uchun va boshqaruv sharhlovchilariga har biri uchun deterministik iz beradi
Ko‘rib chiqish seansini ulaning.