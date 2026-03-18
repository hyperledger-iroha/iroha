---
lang: hy
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

# Connect Session Preview Runbook (IOS7 / JS4)

Այս runbook-ը փաստագրում է փուլային, վավերացման և վերջից մինչև վերջ ընթացակարգը
Միացման նախադիտման նիստերի քանդում, ինչպես պահանջվում է ճանապարհային քարտեզի նշաձողերով **IOS7**
և **JS4** (`roadmap.md:1340`, `roadmap.md:1656`): Հետևեք այս քայլերին ամեն անգամ
դուք ցուցադրում եք Connect Strawman (`docs/source/connect_architecture_strawman.md`),
գործադրեք SDK-ի ճանապարհային քարտեզներում խոստացված հերթի/հեռաչափության կեռիկները կամ հավաքեք
ապացույցներ `status.md`-ի համար:

## 1. Նախնական թռիչքի ստուգաթերթ

| Նյութ | Մանրամասն | Հղումներ |
|------|---------|------------|
| Torii վերջնակետ + Միացման քաղաքականություն | Հաստատեք Torii հիմնական URL-ը, `chain_id` և Միացման քաղաքականությունը (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`): Լուսանկարեք JSON-ի նկարը runbook տոմսում: | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Հարմարավետ + կամուրջ տարբերակներ | Նկատի ունեցեք Norito հարմարանքների հեշը և կամուրջը, որը դուք կօգտագործեք (Swift-ը պահանջում է `NoritoBridge.xcframework`, JS-ը՝ `@iroha/iroha-js` ≥ `bootstrapConnectPreviewSession` առաքված տարբերակը): | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Հեռաչափության վահանակներ | Համոզվեք, որ գծապատկերների կառավարման վահանակները հասանելի են `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` և այլն (Grafana Grafana Norito նկարներ): | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Ապացույցների թղթապանակներ | Ընտրեք այնպիսի ուղղություն, ինչպիսին է `docs/source/status/swift_weekly_digest.md` (շաբաթական ամփոփում) և `docs/source/sdk/swift/connect_risk_tracker.md` (ռիսկերի հետագծում): Պահպանեք տեղեկամատյանները, չափումների սքրինշոթները և հաստատումները `docs/source/sdk/swift/readiness/archive/<date>/connect/`-ում: | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Bootstrap նախադիտման նիստը

1. **Վավերացնել քաղաքականությունը + քվոտաները։** Զանգել՝
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Չհաջողվեց գործարկել, եթե `queue_max`-ը կամ TTL-ը տարբերվում են ձեր պլանավորած կազմաձևից
   փորձարկում.
2. **Ստեղծեք դետերմինիստական SID/URI-ներ։** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` օգնականը կապում է SID/URI սերունդը Torii-ին
   նիստի գրանցում; օգտագործեք այն նույնիսկ այն ժամանակ, երբ Swift-ը կքշի WebSocket շերտը:
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
   - Սահմանեք `register: false`-ը չոր գործարկման QR/խորը հղման սցենարների համար:
   - Պահպանեք վերադարձված `sidBase64Url`, խորը հղման URL-ները և `tokens` բլբակը
     ապացույցների թղթապանակ; Կառավարման վերանայումն ակնկալում է այս արտեֆակտները:
3. **Տարածեք գաղտնիքները։** Կիսվեք խորը կապի URI-ով դրամապանակի օպերատորի հետ
   (swift dApp նմուշ, Android դրամապանակ կամ QA զրահ): Երբեք մի կպցրեք չմշակված նշաններ
   զրույցի մեջ; օգտագործել գաղտնագրված պահոցը, որը փաստագրված է միացման փաթեթում:

## 3. Վարել նիստը1. **Բացեք WebSocket-ը։** Swift-ի հաճախորդները սովորաբար օգտագործում են.
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
   Հղում `docs/connect_swift_integration.md` լրացուցիչ տեղադրման համար (կամուրջ
   ներմուծում, համաժամանակյա ադապտերներ):
2. **Հաստատել + նշանի հոսքերը։** DApps-ը զանգահարում է `ConnectSession.requestSignature(...)`,
   մինչդեռ դրամապանակները պատասխանում են `approveSession` / `reject`-ի միջոցով: Յուրաքանչյուր հաստատում պետք է գրանցվի
   հեշավորված կեղծանունը + թույլտվությունները՝ համապատասխանելու Connect կառավարման կանոնադրությանը:
3. **Զորավարժությունների հերթ + վերսկսման ուղիներ։** Միացնել ցանցի միացումը կամ կասեցնել
   դրամապանակ՝ ապահովելու սահմանափակ հերթի և վերարտադրման կեռիկներ գրանցամատյանների գրառումները: JS/Android
   SDK-ները թողարկում են `ConnectQueueError.overflow(limit)` /
   `.expired(ttlMs)` երբ նրանք գցում են շրջանակներ; Սվիֆթը պետք է նույնը մեկ անգամ դիտարկի
   IOS7 հերթի փայտամած հողերը (`docs/source/connect_architecture_strawman.md`):
   Առնվազն մեկ վերամիացում ձայնագրելուց հետո գործարկեք
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (կամ փոխանցեք արտահանման գրացուցակը, որը վերադարձվել է `ConnectSessionDiagnostics`-ի կողմից) և
   կցեք ցուցադրված աղյուսակը/JSON-ը runbook տոմսին: CLI-ն նույնն է կարդում
   `state.json` / `metrics.ndjson` զույգ, որը արտադրում է `ConnectQueueStateTracker`,
   այնպես որ կառավարման վերանայողները կարող են հետևել փորձնական ապացույցներին՝ առանց պատվիրված գործիքների:

## 4. Հեռաչափություն և դիտելիություն

- ** Չափիչները գրավելու համար.
  - `connect.queue_depth{direction}` չափիչ (պետք է մնա քաղաքականության սահմանաչափից ցածր):
  - `connect.queue_dropped_total{reason="overflow|ttl"}` հաշվիչ (միայն ոչ զրոյական
    սխալ ներարկման ժամանակ):
  - `connect.resume_latency_ms` հիստոգրամ (գրանցեք p95-ը ստիպելուց հետո
    նորից միացնել):
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-ի հատուկ `swift.connect.session_event` և
    `swift.connect.frame_latency` արտահանումներ (`docs/source/sdk/swift/telemetry_redaction.md`):
- **Գրաֆիկական վահանակներ.** Թարմացրեք Connect board-ի էջանիշերը ծանոթագրությունների մարկերներով:
  Կցեք սքրինշոթներ (կամ JSON արտահանումներ) ապացույցների թղթապանակին հումքի կողքին
  OTLP/Prometheus նկարներ՝ նկարահանված հեռաչափություն արտահանող CLI-ի միջոցով:
- **Զգուշացում.** Եթե որևէ Sev1/2 շեմ գործարկվի (ըստ `docs/source/android_support_playbook.md` §5-ի),
  էջեք SDK ծրագրի առաջատարը և փաստաթղթավորեք PagerDuty միջադեպի ID-ն runbook-ում
  տոմսը շարունակելուց առաջ:

## 5. Մաքրում և վերադարձ

1. **Ջնջեք փուլային նիստերը։** Միշտ ջնջեք նախադիտման նիստերը, որպեսզի հերթի խորությունը
   ահազանգերը մնում են իմաստալից.
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Միայն Swift-ով փորձնական գործարկումների համար զանգահարեք նույն վերջնակետը՝ Rust/CLI օգնականի միջոցով:
2. **Մաքրեք ամսագրերը** Հեռացրեք շարունակվող հերթերի ամսագրերը
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB խանութներ և այլն), ուստի
   հաջորդ վազքը սկսում է մաքուր: Անհրաժեշտության դեպքում գրանցեք ֆայլի հեշը նախքան ջնջումը
   վրիպազերծել վերարտադրման խնդիրը:
3. **Ֆայլի միջադեպի նշումներ։** Ամփոփեք վազքը հետևյալում.
   - `docs/source/status/swift_weekly_digest.md` (դելտաների բլոկ),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (մաքրել կամ իջեցնել CR-2
     երբ հեռաչափությունը տեղադրվի),
   - JS SDK փոփոխության մատյան կամ բաղադրատոմս, եթե նոր վարքագիծը վավերացվել է:
4. **Աճացնել ձախողումները.
   - Հերթի հորդացում առանց ներարկված անսարքությունների ⇒ վրիպակ ներկայացնել SDK-ի դեմ, որի
     քաղաքականությունը շեղվել է Torii-ից:
   - Վերսկսել սխալները ⇒ կցել `connect.queue_depth` + `connect.resume_latency_ms`
     դեպքի հաղորդման կադրերը:
   - Կառավարման անհամապատասխանություններ (նշանները կրկին օգտագործված են, TTL-ը գերազանցվել է) ⇒ բարձրացնել SDK-ի հետ
     Ծրագիրը Առաջնորդում և նշում է `roadmap.md` հաջորդ վերանայման ժամանակ:

## 6. Ապացույցների ստուգաթերթ| Արտեֆակտ | Գտնվելու վայրը |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Վահանակի արտահանումներ (`connect.queue_depth` և այլն) | `.../metrics/` ենթաթղթապանակ |
| PagerDuty / միջադեպի ID-ներ | `.../notes.md` |
| Մաքրման հաստատում (Torii ջնջում, ամսագրի սրբում) | `.../cleanup.log` |

Այս ստուգաթերթի լրացումը բավարարում է «փաստաթղթերը/գործադիրները թարմացվել են» ելքի չափանիշը
IOS7/JS4-ի համար և կառավարման վերանայողներին տալիս է դետերմինիստական հետք յուրաքանչյուրի համար
Միացնել նախադիտման նիստը: