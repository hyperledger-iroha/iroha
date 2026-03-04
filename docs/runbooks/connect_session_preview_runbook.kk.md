---
lang: kk
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

# Қосылу сеансын алдын ала қарау Runbook (IOS7 / JS4)

Бұл жұмыс кітабы кезеңге, тексеруге және тексеруге арналған ақырғы процедураны құжаттайды
жою Алдын ала қарау сеанстарын жол картасының маңызды кезеңдеріне сәйкес қосу **IOS7**
және **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Кез келген уақытта осы қадамдарды орындаңыз
сіз Connect strawman демонстрациясын жасайсыз (`docs/source/connect_architecture_strawman.md`),
SDK жол карталарында уәде етілген кезек/телеметриялық ілмектерді қолданыңыз немесе жинаңыз
`status.md` дәлелі.

## 1. Алдын ала ұшуды тексеру тізімі

| Элемент | Мәліметтер | Әдебиеттер |
|------|---------|------------|
| Torii соңғы нүкте + Қосылу саясаты | Torii негізгі URL мекенжайын, `chain_id` және Қосылу саясатын (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) растаңыз. Runbook билетінде JSON суретін түсіріңіз. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Арматура + көпір нұсқалары | Сіз пайдаланатын Norito арматура хэшіне және көпір құрылысына назар аударыңыз (Swift үшін `NoritoBridge.xcframework` қажет, JS үшін `@iroha/iroha-js` ≥ `bootstrapConnectPreviewSession` жеткізілген нұсқасы қажет). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Телеметриялық бақылау тақталары | `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event`, т.б. диаграммалардың қол жетімді екеніне көз жеткізіңіз (Grafana Norito Prometheus суреті). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Дәлелдеме қалталары | `docs/source/status/swift_weekly_digest.md` (апталық дайджест) және `docs/source/sdk/swift/connect_risk_tracker.md` (тәуекел трекері) сияқты бағытты таңдаңыз. Журналдарды, метриканың скриншоттарын және растауларды `docs/source/sdk/swift/readiness/archive/<date>/connect/` астында сақтаңыз. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Алдын ала қарау сеансын жүктеңіз

1. **Саясатты растау + квоталар.** Қоңырау шалыңыз:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   `queue_max` немесе TTL сіз жоспарлаған конфигурациядан өзгеше болса, іске қосу сәтсіз аяқталады.
   сынақ.
2. **Дерминирленген SID/URI жасау.** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` көмекшісі SID/URI генерациясын Torii жүйесіне байланыстырады
   сессияны тіркеу; оны Swift WebSocket қабатын басқарғанда да пайдаланыңыз.
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
   - `register: false` QR/терең сілтеме сценарийлерін құрғақ орындау үшін орнатыңыз.
   - Қайтарылған `sidBase64Url`, терең сілтеме URL мекенжайлары және `tokens` блогын сақтау
     дәлелдемелер папкасы; басқару шолуы осы артефактілерді күтеді.
3. **Құпияларды таратыңыз.** Deeplink URI мекенжайын әмиян операторымен бөлісіңіз
   (swift dApp үлгісі, Android әмиян немесе QA құрылғысы). Ешқашан шикі белгілерді қоймаңыз
   чатта; қосу пакетінде құжатталған шифрланған қойманы пайдаланыңыз.

## 3. Сеансты жүргізіңіз1. **WebSocket ашыңыз.** Swift клиенттері әдетте мыналарды пайдаланады:
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
   Қосымша орнату үшін `docs/connect_swift_integration.md` сілтемесі (көпір
   импорт, параллельдік адаптерлер).
2. **Бекіту + белгі ағындары.** DApps `ConnectSession.requestSignature(...)` нөміріне қоңырау шалады,
   әмияндар `approveSession` / `reject` арқылы жауап береді. Әрбір мақұлдау тіркелуі керек
   хэштелген бүркеншік ат + Connect басқару жарғысына сәйкес келетін рұқсаттар.
3. **Жаттығу кезегі + жалғастыру жолдары.** Желі қосылымын ауыстырыңыз немесе тоқтатыңыз.
   әмиян шектелген кезекті қамтамасыз ету және ілгектер журналының жазбаларын қайталау. JS/Android
   SDKs `ConnectQueueError.overflow(limit)` шығарады /
   `.expired(ttlMs)` олар кадрларды түсіргенде; Свифт бір рет бақылауы керек
   IOS7 кезегі орман алқаптары (`docs/source/connect_architecture_strawman.md`).
   Кем дегенде бір қайта қосылуды жазғаннан кейін іске қосыңыз
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (немесе `ConnectSessionDiagnostics` арқылы қайтарылған экспорттық каталогты өткізіңіз) және
   көрсетілген кестені/JSON-ды runbook билетіне тіркеңіз. CLI бірдей оқиды
   `ConnectQueueStateTracker` шығаратын `state.json` / `metrics.ndjson` жұбы,
   сондықтан басқаруды тексерушілер арнайы құралдарсыз бұрғылау дәлелдерін бақылай алады.

## 4. Телеметрия және бақылау мүмкіндігі

- **Түсіруге арналған көрсеткіштер:**
  - `connect.queue_depth{direction}` көрсеткіші (саясат шегінен төмен тұруы керек).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` есептегіші (тек нөл емес
    ақаулық инъекция кезінде).
  - `connect.resume_latency_ms` гистограммасы (мәжбүрлеуден кейін p95 жазыңыз.
    қайта қосыңыз).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-спецификалық `swift.connect.session_event` және
    `swift.connect.frame_latency` экспорты (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Бақылау тақталары:** Қосылу тақтасының бетбелгілерін аннотация маркерлерімен жаңартыңыз.
  Скриншоттарды (немесе JSON экспорттарын) шикі деректермен қатар дәлелдер қалтасына тіркеңіз
  CLI телеметрия экспорттаушысы арқылы алынған OTLP/Prometheus суреттері.
- **Ескерту:** Кез келген Sev1/2 шектері іске қосылса (`docs/source/android_support_playbook.md` §5 бойынша),
  SDK бағдарлама жетекшісін бетке түсіріңіз және runbook ішіндегі PagerDuty оқиғасының идентификаторын құжаттаңыз
  жалғастырмас бұрын билет.

## 5. Тазалау және кері қайтару

1. **Кезеңдік сеанстарды жою.** Кезең тереңдігі үшін алдын ала қарау сеанстарын әрқашан жойыңыз.
   дабыл мәнді болып қалады:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Тек Swift сынақтары үшін Rust/CLI көмекшісі арқылы бірдей соңғы нүктеге қоңырау шалыңыз.
2. **Журналдарды тазалау.** Кез келген тұрақты кезек журналдарын алып тастаңыз
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB дүкендері, т.б.) сондықтан
   келесі жүгіру таза басталады. Қажет болса, жою алдында файл хэшін жазып алыңыз
   қайта ойнату мәселесін жөндеу.
3. **Файл оқиғасы туралы жазбалар.** Іске қосуды қорытындылаңыз:
   - `docs/source/status/swift_weekly_digest.md` (дельталар блогы),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (CR-2 нұсқасын тазалау немесе төмендету
     телеметрия орнатылғаннан кейін),
   - JS SDK өзгерту журналы немесе жаңа әрекет расталған болса, рецепт.
4. **Сәтсіздіктерді күшейту:**
   - Инъекциялық ақауларсыз кезектің толып кетуі ⇒ SDK-ге қарсы қате жібереді
     саясат Torii-тен алшақтады.
   - Жалғастыру қателері ⇒ `connect.queue_depth` + `connect.resume_latency_ms` тіркеңіз
     оқиға есебінің суреттері.
   - Басқарудағы сәйкессіздіктер (токендер қайта пайдаланылды, TTL асып кетті) ⇒ SDK арқылы арттыру
     Бағдарлама жетекшісі және келесі қайта қарау кезінде `roadmap.md` түсіндірмесі.

## 6. Дәлелдерді тексеру парағы| Артефакт | Орналасқан жері |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Бақылау тақтасының экспорты (`connect.queue_depth`, т.б.) | `.../metrics/` ішкі қалтасы |
| PagerDuty / оқиға идентификаторлары | `.../notes.md` |
| Тазалауды растау (Torii жою, журналды сүрту) | `.../cleanup.log` |

Бұл бақылау тізімін толтыру "docs/runbooks жаңартылды" шығу шартын қанағаттандырады
IOS7/JS4 үшін және басқару шолушыларына әрқайсысы үшін детерминирленген із береді
Алдын ала қарау сеансын қосыңыз.