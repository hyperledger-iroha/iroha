---
lang: mn
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

# Холбох сессийг урьдчилан үзэх Runbook (IOS7 / JS4)

Энэхүү runbook нь үе шатыг тогтоох, баталгаажуулах, баталгаажуулах төгсгөл хүртэлх процедурыг баримтжуулдаг
**IOS7** замын зураглалын чухал үе шатуудын шаардлагын дагуу урьдчилан үзэх сешнүүдийг холбоно уу
болон **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Эдгээр алхмуудыг хүссэн үедээ дагана уу
та Connect strawman-г үзүүлэв (`docs/source/connect_architecture_strawman.md`),
SDK замын зурагт амласан дараалал/телеметрийн дэгээг ашиглах, эсвэл цуглуул
`status.md`-ийн нотолгоо.

## 1. Нислэгийн өмнөх шалгах хуудас

| Зүйл | Дэлгэрэнгүй | Ашигласан материал |
|------|---------|------------|
| Torii төгсгөлийн цэг + Холбох бодлого | Torii үндсэн URL, `chain_id` болон Холбох бодлогыг (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) баталгаажуулна уу. Runbook тасалбарт JSON агшин зуурын зургийг аваарай. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Бэхэлгээ + гүүрний хувилбарууд | Таны ашиглах Norito бэхэлгээний хэш болон гүүрийг анхаарна уу (Swift-д `NoritoBridge.xcframework`, JS-д `@iroha/iroha-js` ≥ `bootstrapConnectPreviewSession`-ийг илгээсэн хувилбар шаардлагатай). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Телеметрийн хяналтын самбар | `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` гэх мэт диаграммыг ашиглах боломжтой (Grafana Norito) Prometheus хормын хувилбарууд). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Нотлох баримт хавтас | `docs/source/status/swift_weekly_digest.md` (долоо хоног тутмын тойм) болон `docs/source/sdk/swift/connect_risk_tracker.md` (эрсдэл хянагч) зэрэг очих газраа сонго. Лог, хэмжүүрийн дэлгэцийн агшин, талархалыг `docs/source/sdk/swift/readiness/archive/<date>/connect/` доор хадгална уу. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Урьдчилан үзэх сессийг ачаална уу

1. **Бодлого + квотыг баталгаажуулах.** Дуудлага хийх:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Хэрэв `queue_max` эсвэл TTL нь таны төлөвлөж байсан тохиргооноос өөр байвал ажиллуулна уу.
   тест.
2. ** Тодорхойлогч SID/URI үүсгэх.** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` туслах нь SID/URI үүслийг Torii руу холбодог.
   хуралдааны бүртгэл; Свифт WebSocket давхаргыг жолоодох үед ч үүнийг ашиглаарай.
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
   - `register: false`-г хуурай ажиллуулах QR/гүн холбоос хувилбарт тохируулна уу.
   - Буцаагдсан `sidBase64Url`, гүнзгий холбоосын URL-ууд болон `tokens` блокуудыг хадгалах
     нотлох баримт хавтас; засаглалын тойм эдгээр олдворуудыг хүлээж байна.
3. **Нууц түгээх.** Deeplink URI-г түрийвчний оператортой хуваалцаарай
   (swift dApp дээж, Android түрийвч эсвэл QA бэхэлгээ). Түүхий жетоныг хэзээ ч бүү буулга
   чат руу; идэвхжүүлэх багцад бичигдсэн шифрлэгдсэн санг ашиглах.

## 3. Сессийг жолоодох1. **WebSocket-г нээнэ үү.** Swift үйлчлүүлэгчид ихэвчлэн дараахыг ашигладаг:
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
   Нэмэлт тохиргооны лавлагаа `docs/connect_swift_integration.md` (гүүр
   импорт, зэрэгцээ адаптер).
2. **Батлах + тэмдгийн урсгал.** DApps `ConnectSession.requestSignature(...)`,
   харин түрийвч `approveSession` / `reject`-ээр хариу өгдөг. Зөвшөөрөл бүрийг бүртгэх ёстой
   холбосон нэр + зөвшөөрлүүд нь Connect-ийн засаглалын дүрэмд нийцнэ.
3. **Дасгалын дараалал + үргэлжлүүлэх зам.** Сүлжээний холболтыг асаах/унтраах эсвэл түр зогсоох
   хэтэвч хязгаарлагдмал дараалал болон дахин тоглуулах дэгээ бүртгэлийн оруулгуудыг хангах. JS/Android
   SDK нь `ConnectQueueError.overflow(limit)` ялгаруулдаг /
   `.expired(ttlMs)` тэд хүрээ буулгах үед; Свифт ижил зүйлийг нэг удаа ажиглах ёстой
   IOS7-ийн дарааллын шатуудын газар (`docs/source/connect_architecture_strawman.md`).
   Та дор хаяж нэг дахин холболтыг бичсэний дараа ажиллуулна уу
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (эсвэл `ConnectSessionDiagnostics`-ээр буцаасан экспортын лавлахыг дамжуулж) болон
   харуулсан хүснэгт/JSON-г runbook тасалбарт хавсаргана уу. CLI нь мөн адил уншдаг
   `ConnectQueueStateTracker` үйлдвэрлэдэг `state.json` / `metrics.ndjson` хос,
   Тиймээс засаглалын тоймчид захиалгат багаж хэрэгсэлгүйгээр өрөмдлөгийн нотлох баримтыг хайж олох боломжтой.

## 4. Телеметр ба ажиглалт

- **Багах хэмжүүр:**
  - `connect.queue_depth{direction}` хэмжигч (бодлогын хязгаараас доогуур байх ёстой).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` тоолуур (зөвхөн тэг биш
    алдаа шахах үед).
  - `connect.resume_latency_ms` гистограмм (хүчээр оруулсны дараа p95-г бичнэ үү.
    дахин холбох).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift тусгай `swift.connect.session_event` болон
    `swift.connect.frame_latency` экспорт (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Хяналтын самбар:** Холболтын самбарын хавчуургыг тэмдэглэгээний тэмдэглэгээгээр шинэчилнэ үү.
  Дэлгэцийн агшинг (эсвэл JSON экспортыг) нотлох баримт хавтсанд түүхийн хажууд хавсаргана уу
  Телеметрийн экспортлогч CLI-аар татсан OTLP/Prometheus агшин зуурын зургууд.
- **Сэрэмжлүүлэг:** Хэрэв ямар нэгэн Sev1/2 босго өдөөгдвөл (`docs/source/android_support_playbook.md` §5-ын дагуу),
  SDK хөтөлбөрийн удирдагчийг хуудсаж, PagerDuty ослын ID-г runbook-д баримтжуулна
  үргэлжлүүлэхийн өмнө тасалбар.

## 5. Цэвэрлэх & Эргүүлэх

1. **Үе шаттай сешнүүдийг устгана уу.** Урьдчилан үзэх сешнүүдийг үргэлж устга, ингэснээр гүн дараалалд оруулна.
   дохиолол нь утга учиртай хэвээр байна:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Зөвхөн Swift-н туршилтын хувьд Rust/CLI туслагчаар дамжуулан ижил төгсгөлийн цэг рүү залгана уу.
2. **Тэмдэглэлүүдийг цэвэрлэх.** Үргэлжлүүлсэн дарааллын журналуудыг устгана уу
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB дэлгүүр гэх мэт) тиймээс
   дараагийн гүйлт цэвэрхэн эхэлнэ. Шаардлагатай бол устгахын өмнө файлын хэшийг тэмдэглэ
   дахин тоглуулах асуудлыг дибаг хийх.
3. **Ойл явдлын тэмдэглэлийг файлд оруулна уу.** Гүйлгээг нэгтгэн дүгнэнэ үү:
   - `docs/source/status/swift_weekly_digest.md` (гурвалжин блок),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2-г арилгах эсвэл бууруулах
     телеметрийг суулгасны дараа),
   - JS SDK өөрчлөлтийн бүртгэл эсвэл шинэ зан үйлийг баталгаажуулсан жор.
4. **Алдааг нэмэгдүүлэх:**
   - Тарилгын алдаагүй дарааллын халилт ⇒ SDK-ийн эсрэг алдаа гаргах
     бодлого нь Torii-ээс ялгаатай.
   - Үргэлжлүүлэх алдаа ⇒ `connect.queue_depth` + `connect.resume_latency_ms` хавсаргана уу
     ослын тайлангийн агшин зуурын зургууд.
   - Засаглалын таарамжгүй байдал (токенуудыг дахин ашигласан, TTL хэтэрсэн) ⇒ SDK-тэй хамт өсгөх
     Хөтөлбөрийг удирдаж, дараагийн засварын үеэр `roadmap.md`-д тайлбар оруулна уу.

## 6. Нотлох баримт шалгах хуудас| Олдвор | Байршил |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Самбарын экспорт (`connect.queue_depth` гэх мэт) | `.../metrics/` дэд хавтас |
| PagerDuty / ослын ID | `.../notes.md` |
| Цэвэрлэгээний баталгаажуулалт (Torii устгах, журналыг арчих) | `.../cleanup.log` |

Энэхүү хяналтын хуудсыг бөглөх нь "docs/runbooks шинэчлэгдсэн" гарах шалгуурыг хангана.
IOS7/JS4-д зориулагдсан бөгөөд засаглалын тоймчдод тус бүрээр тодорхойлогч мөрийг өгдөг
Урьдчилан үзэх сессийг холбоно уу.