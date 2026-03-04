---
lang: dz
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

# མཐུད་ལམ་དུས་ཚོད་སྔོན་ལྟ་ རན་བུཀ་ (IOS7 / JS4)

རན་དེབ་འདི་གིས་ འཁྲབ་སྟོན་དང་ བདེན་དཔྱད་འབད་ནི་གི་དོན་ལུ་ མཐའ་མཇུག་ལས་མཇུག་ཚུན་གྱི་བྱ་རིམ་ཚུ་ ཡིག་ཆ་བཟོཝ་ཨིན།
འབྲེལ་འཐུད་ཀྱི་སྔོན་ལྟའི་དུས་ཚོད་ཚུ་ ལམ་གྱི་ས་ཁྲ་གི་གནས་རིམ་ཚུ་གིས་ དགོས་མཁོ་དང་འཁྲིལ་ཏེ་ **IOS7** གིས་ བཤུབ་བཏང་ནི།
དང་ **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). གོ་རིམ་འདི་ཚུ་ ག་དུས་འབད་རུང་ རྗེས་སུ་འཇུག་དགོ།
ཁྱོད་ཀྱིས་ མཐུད་ལམ་གྱི་ མེ་ཏོག་ (`docs/source/connect_architecture_strawman.md`) འདི་ བཀྲམ་སྟོན་འབདཝ་ཨིན།
བང་རིམ་/བརྒྱུད་འཕྲིན་གྱི་ ཧུཀ་ཚུ་ ཨེསི་ཌི་ཀེ་ ལམ་སྟོན་ནང་ ཡང་ན་ བསྡུ་ལེན་འབད།
སྒྲུབ་བྱེད་ `status.md` གི་སྒྲུབ་བྱེད་།

## 1. སྔོན་མའི་ཚོད་ལྟའི་ཐོ་ཡིག།

| རྣམ་གྲངས་ | ཁ་གསལ་ | གཞི་བསྟུན་ཚུ་ |
|--------------------------------------- |
| Torii མཇུག་པོ་ + མཐུད་པའི་སྲིད་བྱུས། | Torii གཞི་རྟེན་ཡུ་ཨར་ཨེལ་དང་ `chain_id`, དང་ མཐུད་བྱེད་སྲིད་བྱུས་ (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`) ངེས་དཔྱད་འབད། རན་བུག་གི་ཤོག་བྱང་ནང་ ཇེ་ཨེསི་ཨོ་ཨེན་ པར་ལེན་འདི་ བཟུང་། | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| fixture + ཟམ་གྱི་ཐོན་རིམ། | ཁྱོད་ཀྱིས་ལག་ལེན་འཐབ་ནི་ཨིན་པའི་ Norito བརྟན་བཞུགས་ཧེ་ཤི་དང་ཟམ་བཟོ་བསྐྲུན་ (Swift དགོཔ་ཨིན་ `NoritoBridge.xcframework` དགོཔ་ཨིན་, JS ལུ་ `@iroha/iroha-js` ≥ ཐོན་རིམ་འདི་ `bootstrapConnectPreviewSession` གཏང་དགོཔ་ཨིན།) | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| བརྒྱུད་འཕྲིན་གྱི་ བརྡ་བཀོད་ཚུ་ | `connect.queue_depth`, Norito, Norito, ལ་སོགས་པ་ཚུ་ ལྷོད་ཚུགསཔ་ཨིན་ དེ་ཡང་ ལྷོད་ཚུགསཔ་ཨིན་ (Grafana `Android/Swift Connect` བུར་གཞོང་ +ཕྱིར་འདྲེན་འབད་ནི། Prometheus པར་ལེན་ཚུ། | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md`, .
| སྒྲུབ་བྱེད་ཀྱི་སྣོད་འཛིན་ཚུ་ | འགྲོ་ཡུལ་ཅིག་ དཔེར་ན་ `docs/source/status/swift_weekly_digest.md` (བདུན་ཕྲག་རེའི་ བཞུ་བཅོས་) དང་ `docs/source/sdk/swift/connect_risk_tracker.md` (ཉེན་ཁ་རྗེས་འདེད་) བཟུམ་གྱི་ འདམ་ཁ་རྐྱབ། དྲན་ཐོ་དང་ མེ་ཊིགསི་གསལ་གཞི་ དེ་ལས་ ངོས་ལེན་ཚུ་ `docs/source/sdk/swift/readiness/archive/<date>/connect/` གི་འོག་ལུ་ གསོག་འཇོག་འབད། | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. སྔོན་ལྟའི་ལས་རིམ།

1. **སྲིད་བྱུས་བདེན་དཔང་འབད་ + quotas.** འབོད་བརྡ་:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   ཁྱོད་ཀྱིས་འཆར་གཞི་བརྩམ་ཡོད་པའི་རིམ་སྒྲིག་ལས་ `queue_max` ཡང་ན་ TTL འདི་ གཡོག་བཀོལ་མ་ཚུགས།
   ཆོས་རྒྱུགས།
2. **གཏན་འབེབས་ SID/URIs བསྡུ་སྒྲིག་འབད།** `@iroha/iroha-js` དང་།
   `bootstrapConnectPreviewSession` གྲོགས་རམ་གྱི་འབྲེལ་མཐུན་ SID/URI མི་རབས་ལས་ Torii
   ལཱ་ཡུན་ཐོ་བཀོད་འབད་ནི། སུའིཕཊི་གིས་ ཝེབ་སོ་ཀེཊི་བང་རིམ་འདི་ འདྲེན་འབད་འོང་པའི་སྐབས་ལུ་ཡང་ ལག་ལེན་འཐབ།
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
   - སྐམ་གཡོག་བཀོལ་བའི་ ཀིའུ་ཨར་/གཏིང་ཟབ་པའི་གནས་སྟངས་ཚུ་ལུ་ `register: false` གཞི་སྒྲིག་འབད།
   - སླར་ལོག་འབད་མི་ `sidBase64Url` དང་ གཏིང་ཟབ་པའི་ཡུ་ཨར་ཨེལ་ དེ་ལས་ Torii blob ནང་ལུ་ བཀོད་སྒྲིག་འབད།
     བདེན་དཔང་སྣོད་ཐོ།; གཞུང་སྐྱོང་བསྐྱར་ཞིབ་འདི་གིས་ འ་ནི་དངོས་པོ་ཚུ་ རེ་བ་བསྐྱེདཔ་ཨིན།
3. **གསང་བ་ཚུ་བཀྲམ་སྤེལ་འབདཝ་ཨིན།** དངུལ་ཁུག་བཀོལ་སྤྱོད་པ་དང་གཅིག་ཁར་ གཏིང་ཟབ་པའི་ལིངཀ་ཡུ་ཨར་ཨའི་འདི་ བརྗེ་སོར་འབད།
   (Swift dAP གི་དཔེ་ཚད་ Android དངུལ་ཁང་ ཡང་ན་ QA ཧར་ནིསི)། ཊོ་ཀེན་ཚུ་ ནམ་ཡང་ སྦྱར་མ་བཏུབ།
   ཁ་སླབ་ནང་ལུ་; ལྕོགས་ཅན་ཐུམ་སྒྲིལ་ནང་ ཡིག་ཐོག་ལུ་བཀོད་ཡོད་པའི་ གསང་བཟོས་འབད་ཡོད་པའི་ གྱང་འདི་ལག་ལེན་འཐབ།

## 3. ཚོགས་འདུ་བཏང་བ།1. **ཝེབ་སོ་ཀེཊི་ཁ་ཕྱེ།** སུའིཕཊི་མཁོ་སྤྲོད་པ་ཚུ་ སྤྱིར་བཏང་ལུ་ལག་ལེན་འཐབ་ཨིན།
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
   ཁ་སྐོང་གཞི་སྒྲིག་ (ཟམ་པ་) གི་དོན་ལུ་ གཞི་བསྟུན་ `docs/connect_swift_integration.md`
   ནང་འདྲེན་, དུས་མཉམ་དུ་མཐུན་སྒྲིག)།
2. **ཆ་འཇོག་ + རྟགས་བཞུར་རྒྱུན་ཚུ་ འཕེན་དགོ།
   ཝལ་ཊི་ཚུ་གིས་ `approveSession` / `reject` བརྒྱུད་དེ་ ལན་གསལ་འབདཝ་ཨིན། ཆ་འཇོག་རེ་རེ་གིས་ ནང་བསྐྱོད་འབད་དགོ།
   མཐུད་འབྲེལ་གཞུང་གི་བཀའ་ཁྲིམས་དང་མཐུན་སྒྲིག་འབད་ནིའི་གནང་བ་ ཧ་ཤེད་གཞན་ + གནང་བ་ཚུ།
3. **ལུས་སྦྱོང་གྲལ་ཐིག་ + སླར་འབྱུང་འགྲུལ་ལམ་ཚུ་ བཏོན་གཏང་།** ཡོངས་འབྲེལ་མཐུད་ལམ་སོར་བསྒྱུར་འབད་ ཡང་ན་ འདི་བཀག་བཞག།
   མཐའ་མཚམས་ཡོད་པའི་གྱལ་དང་ བསྐྱར་རྩེད་ཀྱི་ ཧུཀ་གི་ དྲན་ཐོ་ཐོ་བཀོད་ཚུ་ ངེས་གཏན་བཟོ་ནི་ལུ་ warlet. ཇེ་ཨེསི་/ཨེན་ཌོའིཌ་
   ཨེསི་ཌི་ཀེ་ཨེསི་གིས་ `ConnectQueueError.overflow(limit)` བཏོན་གཏང་།
   Norito གིས་ གཞི་ཁྲམ་ཚུ་བཏོན་བཏང་པའི་སྐབས་; སུའིཕཊ་གིས་ ཚར་གཅིག་ དེ་བཟུམ་ཅིག་སྦེ་ བལྟ་དགོ།
   IOS7 གྱལ་རིམ་གྱི་ས་ཆ་ (`docs/source/connect_architecture_strawman.md`).
   ཁྱོད་ཀྱིས་ཉུང་མཐའ་གཅིག་སླར་མཐུད་འབད་བའི་ཤུལ་ལས་ གཡོག་བཀོལ།
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (ཡང་ན་ Torii གིས་སླར་ལོག་འབད་མི་ ཕྱིར་འདྲེན་སྣོད་ཐོ་འདི་ བརྒྱུད་དེ་འགྱོཝ་ཨིན།) དང་།
   རན་དེབ་ཀྱི་ཤོག་འཛིན་ལུ་ བརྡ་སྟོན་འབད་ཡོད་པའི་ཐིག་ཁྲམ་/JSON མཉམ་སྦྲགས་འབད། CLI གིས་ དེ་དང་འདྲ་བར་ལྷག་ཡོད།
   Torii / `metrics.ndjson` ཆ་གཅིག་ Torii གིས་ བསྐྲུན་ཡོད།
   དེ་འབདཝ་ལས་ གཞུང་སྐྱོང་བསྐྱར་ཞིབ་འབད་མི་ཚུ་གིས་ ལག་ཆས་མེད་པར་ དམག་སྦྱོང་སྒྲུབ་བྱེད་ཚུ་ འཚོལ་ཞིབ་འབད་ཚུགས།

## 4. བརྒྱུད་འཕྲིན་དང་བལྟ་ཚུགས།

- **པར་ལེན་ནི་:** ལུ་འཚམས།
  - `connect.queue_depth{direction}` འཇལ་ཚད་ (སྲིད་བྱུས་ཁ་དོག་འོག་ལུ་སྡོད་དགོ།)
  - `connect.queue_dropped_total{reason="overflow|ttl"}` གྱངས་ཁ་ (ཀླད་ཀོར་མ་ཡིན་པ་རྐྱངམ་གཅིག
    སྐྱོན་བཀོད་པའི་སྐབས།
  - `connect.resume_latency_ms` ཧིསི་ཊོ་གཱརམ་ (a བཙན་ཤེད་འབད་བའི་ཤུལ་ལས་ p95 དྲན་ཐོ་བཀོད།
    བསྐྱར་མཐུན)།
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - སུའིཕཊ་-དམིགས་བསལ་ `swift.connect.session_event` དང་།
    `swift.connect.frame_latency` ཕྱིར་གཏོང་ (`docs/source/sdk/swift/telemetry_redaction.md`).
- **གནད་སྡུད་བོརཌི་ཚུ་:** མཆན་འགྲེལ་རྟགས་ཚུ་དང་བཅས་ མཐུད་སྦྲེལ་གྱི་དེབ་རྟགས་ཚུ་དུས་མཐུན་བཟོ།
  གསལ་གཞི་གི་པར་ཚུ་(ཡང་ན་ཇེ་ཨེསི་ཨོ་ཨེན་ཕྱིར་འདྲེན་) རའུ་གི་སྦོ་ལོགས་ཁར་ སྒྲུབ་བྱེད་སྣོད་འཛིན་ལུ་མཉམ་སྦྲགས་འབད།
  OTLP/Prometheus པར་ལེན་ཚུ་ ཊེ་ལི་མི་ཊི་ཕྱིར་ཚོང་པ་ སི་ཨེལ་ཨའི་བརྒྱུད་དེ་ འཐེན་ཡོདཔ་ཨིན།
- **Alerting:** གལ་ཏེ་ རིམ་པ་གང་རུང་གིས་ གང་རུང་ཅིག་གིས་ སྐུལ་མ་ (`docs/source/android_support_playbook.md` §༥)
  ཤོག་ངོས་ཨེསི་ཌི་ཀེ་ལས་རིམ་འགོ་འཁྲིད་འབད་ཞིནམ་ལས་ རྔོན་དེབ་ནང་ པེ་ཇར་ཌུཊི་བྱུང་ལས་ཨའི་ཌི་འདི་ ཡིག་ཐོག་ལུ་བཀོད།
  འཕྲོ་མཐུད་མ་འབད་བའི་ཧེ་མ་ ཊིཀ།

## 5. གཙང་སྦྲ་དང་རོལ་དབྱངས།

1. ** སྟེགས་རིས་ཀྱི་ལཱ་ཡུན་ཚུ་བཏོན་གཏང་།** སྔོན་ལྟ་ལཱ་ཡུན་ཚུ་ཨ་རྟག་ར་རྩ་བསྐྲད་གཏང་བསུ།
   ཉེན་བརྡ་ཚུ་ དོན་དག་ཅན་སྦེ་སྡོད་ནི།
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   སུའིཕཊི་-རྐྱངམ་ཅིག་བརྟག་དཔྱད་གཡོག་བཀོལ་ནིའི་དོན་ལུ་ རཱསིཊི་/སི་ཨེལ་ཨའི་གྲོགས་རམ་བརྒྱུད་དེ་ མཐའ་ཐིག་གཅིགཔོ་འདི་ལུ་ ཁ་བཏང་།
2. **Purge thep.** གནས་ཡུན་བརྟན་ཏོག་ཏོ་ཡོད་པའི་གྲལ་ཐིག་དུས་དེབ་ཚུ་རྩ་བསྐྲད་གཏང་།
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB ཚོང་ཁང་སོགས་) དེ་ལས་
   ཤུལ་མམ་གྱི་གཡོག་བཀོལ་འགོ་བཙུགས། བཏོན་གཏང་ནི་གི་ཧེ་མར་ ཡིག་སྣོད་ཧེཤ་ དྲན་ཐོ་བཀོད།
   བསྐྱར་རྩེད་ཀྱི་གནད་དོན་ཅིག་རྐྱེན་སེལ་འབད།
3. **ཡིག་སྣོད་བྱུང་རྐྱེན་དྲན་འཛིན་ཚུ།** གཡོག་བཀོལ་འདི་ བཅུད་བསྡུས་འབད།
   - `docs/source/status/swift_weekly_digest.md` (ཌེལ་ཊསི་སྡེབ་ཚན་)།
   - `docs/source/sdk/swift/connect_risk_tracker.md` (གསལ་པོ་ཡང་ན་མར་ཕབ་ཀྱི་CR-22)
     ཚར་གཅིག་ བརྡ་བརྒྱུདཔ་འདི་ ས་སྒོ་ནང་ཡོདཔ་ཨིན།)
   - སྤྱོད་ལམ་གསརཔ་འདི་ བདེན་དཔྱད་འབད་བ་ཅིན་ ཇེ་ཨེསི་ཨེསི་ཌི་ཀེ་ བསྒྱུར་བཅོས་ཡང་ན་ བཟོ་ཐངས་འདི་ཨིན།
4. **ཡར་འཕར་གྱི་འཐུས་ཤོར་:**
   - བཙུགས་ཡོད་པའི་འཛོལ་བ་མེད་པར་ ཀིའུ་ཨོ་སིལ་ཕོལོ་། ⇒ ག་གིས་ ཨེསི་ཌི་ཀེ་ལུ་ འབུཔ་ཅིག་ཡིག་སྣོད་ཅིག་ཡིག་སྣོད་ནང་བཙུགས་ནི།
     སྲིད་བྱུས་འདི་ Torii ལས་ཁ་སྟོར་ཡོདཔ་ཨིན།
   - བསྐྱར་བཟོའི་འཛོལ་བ་ཚུ་ ⇒ `connect.queue_depth` + `connect.resume_latency_ms` མཉམ་སྦྲགས།
     བྱུང་རྐྱེན་གྱི་སྙན་ཞུ་ལུ་ པར་ལེན་འབདཝ་ཨིན།
   - གཞུང་སྐྱོང་མ་མཐུན་མི་ཚུ་ (ཊོ་ཀེན་ཚུ་ལོག་ལག་ལེན་འཐབ་ཡོདཔ་, ཊི་ཊི་ཨེལ་ལྷག་ཡོདཔ་) ⇒ ཨེསི་ཌི་ཀེ་དང་གཅིག་ཁར་ཡར་འཐེན།
     ཤུལ་མའི་བསྐྱར་ཞིབ་ཀྱི་སྐབས་ལུ་ ལས་རིམ་འགོ་འཁྲིདཔ་དང་ མཆན་འགྲེལ།

## 6. སྒྲུབ་བྱེད་ཀྱི་དཔྱད་གཞི།| ཅ་ཆས། | ས་གནས་ |
|-----------------------------------------------------------------------------------------------------------------
| SID/deeplink/toins JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| ཌེཤ་བོརཌ་ཕྱིར་འདྲེན་ (`connect.queue_depth`, ལ་སོགས་པ་ཚུ་) | `.../metrics/` ཡན་ལག་སྣོད་འཛིན་ |
| PagerDuty / བྱུང་རྐྱེན་ IDs | Torii |
| གཙང་སྦྲའི་བདེན་དཔང་ (Torii བཏོན་གཏང་, དུས་དེབ་འཕྱག་འབག་) | `.../cleanup.log` |

ཞིབ་དཔྱད་ཐོ་ཡིག་འདི་མཇུག་བསྡུ་མི་འདི་གིས་ “docs/runbooks དུས་མཐུན་བཟོ་ནི་” ཕྱིར་ཐོན་ཚད་གཞི་འདི་ གྲུབ་ཚུགསཔ་ཨིན།
IOS7/JS4 གི་དོན་ལུ་ དང་ གཞུང་སྐྱོང་བསྐྱར་ཞིབ་པ་ཚུ་ལུ་ ག་ར་གི་དོན་ལུ་ ཐག་བཅད་ཀྱི་ལམ་ཅིག་བྱིནམ་ཨིན།
སྔོན་ལྟ་ལཱ་ཡུན་མཐུད།