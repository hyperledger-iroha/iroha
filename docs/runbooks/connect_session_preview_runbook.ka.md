---
lang: ka
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

ეს წიგნაკი ადასტურებს ინსცენირების, ვალიდაციის და ბოლომდე მიყვანის პროცედურას
დაკავშირების წინასწარი გადახედვის სესიების დანგრევა, როგორც ეს მოითხოვს საგზაო რუქის ეტაპებს **IOS7**
და **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). მიჰყევით ამ ნაბიჯებს ნებისმიერ დროს
თქვენ დემო Connect strawman (`docs/source/connect_architecture_strawman.md`),
განახორციელეთ SDK-ის საგზაო რუქებში დაპირებული რიგი/ტელემეტრიის კაკვები, ან შეაგროვეთ
მტკიცებულება `status.md`-ისთვის.

## 1. წინასწარი ფრენის ჩამონათვალი

| ნივთი | დეტალები | ლიტერატურა |
|------|---------|------------|
| Torii ბოლო წერტილი + დაკავშირების პოლიტიკა | დაადასტურეთ Torii საბაზისო URL, `chain_id` და დაკავშირების პოლიტიკა (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). გადაიღეთ JSON სნეპშოტი runbook ბილეთში. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| არმატურა + ხიდის ვერსიები | გაითვალისწინეთ Norito მოწყობილობების ჰეში და ხიდის კონსტრუქცია, რომელსაც გამოიყენებთ (Swift მოითხოვს `NoritoBridge.xcframework`, JS მოითხოვს `@iroha/iroha-js` ≥ ვერსიას, რომლითაც გაიგზავნება `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| ტელემეტრიის დაფები | დარწმუნდით, რომ დაფები, რომლებზეც დიაგრამა `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event` და ა.შ. ხელმისაწვდომია (Grafana Grafana Norito კადრები). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| მტკიცებულებათა საქაღალდეები | აირჩიეთ დანიშნულების ადგილი, როგორიცაა `docs/source/status/swift_weekly_digest.md` (კვირის შეჯამება) და `docs/source/sdk/swift/connect_risk_tracker.md` (რისკების ტრეკერი). შეინახეთ ჟურნალები, მეტრიკის ეკრანის ანაბეჭდები და დადასტურებები `docs/source/sdk/swift/readiness/archive/<date>/connect/`-ში. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. ჩატვირთეთ წინასწარი გადახედვის სესია

1. **პოლიტიკის დამოწმება + კვოტები.** დარეკეთ:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   გაშვება ვერ მოხერხდება, თუ `queue_max` ან TTL განსხვავდება კონფიგურაციისგან, რომელიც დაგეგმეთ
   ტესტი.
2. **შექმენით დეტერმინისტული SID/URI.** `@iroha/iroha-js`
   `bootstrapConnectPreviewSession` დამხმარე აკავშირებს SID/URI თაობას Torii-თან
   სესიის რეგისტრაცია; გამოიყენეთ ის მაშინაც კი, როცა Swift ამოძრავებს WebSocket ფენას.
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
   - დააყენეთ `register: false` მშრალი გაშვების QR/ღრმა ბმულის სცენარებზე.
   - განაგრძეთ დაბრუნებული `sidBase64Url`, ღრმა ბმული URL და `tokens` ბლოგები
     მტკიცებულების საქაღალდე; მმართველობის მიმოხილვა მოელის ამ არტეფაქტებს.
3. **გაავრცელეთ საიდუმლოებები.** გაუზიარეთ deeplink URI საფულის ოპერატორს
   (swift dApp ნიმუში, Android საფულე ან QA აღკაზმულობა). არასოდეს ჩასვათ ნედლეული ჟეტონები
   ჩატში; გამოიყენეთ დაშიფრული სარდაფი, რომელიც დოკუმენტირებულია ჩართვის პაკეტში.

## 3. მართეთ სესია1. **გახსენით WebSocket.** Swift კლიენტები ჩვეულებრივ იყენებენ:
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
   მითითება `docs/connect_swift_integration.md` დამატებითი დაყენებისთვის (ხიდი
   იმპორტი, კონკურენტული გადამყვანები).
2. **დამტკიცება + ნიშნის ნაკადები.** DApps რეკავს `ConnectSession.requestSignature(...)`,
   ხოლო საფულეები პასუხობენ `approveSession` / `reject` მეშვეობით. ყოველი დამტკიცება უნდა იყოს შესული
   ჰეშირებული მეტსახელი + Connect-ის მმართველობის წესდების შესატყვისი ნებართვები.
3. **სავარჯიშოების რიგი + განახლების ბილიკები.** ქსელის კავშირის გადართვა ან შეჩერება
   საფულე, რათა უზრუნველყოს შემოსაზღვრული რიგი და ხელახლა დაკვრა კაკვების ჟურნალის ჩანაწერები. JS/Android
   SDK-ები ასხივებენ `ConnectQueueError.overflow(limit)` /
   `.expired(ttlMs)` კადრების ჩამოყრისას; სვიფტმა იგივე უნდა დააკვირდეს ერთხელ
   IOS7 რიგის ხარაჩოების მიწები (`docs/source/connect_architecture_strawman.md`).
   მას შემდეგ, რაც ჩაწერთ მინიმუმ ერთ ხელახლა დაკავშირებას, გაუშვით
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (ან გაიარეთ `ConnectSessionDiagnostics`-ის მიერ დაბრუნებული ექსპორტის დირექტორია) და
   მიამაგრეთ გამოსახული ცხრილი/JSON runbook ბილეთს. CLI იგივეს კითხულობს
   `state.json` / `metrics.ndjson` წყვილი, რომელსაც `ConnectQueueStateTracker` აწარმოებს,
   ასე რომ, მმართველობის მიმომხილველებს შეუძლიათ მოძებნონ სავარჯიშო მტკიცებულება შეკვეთილი ინსტრუმენტების გარეშე.

## 4. ტელემეტრია და დაკვირვება

- ** მეტრიკა გადასაღებად:**
  - `connect.queue_depth{direction}` ლიანდაგი (უნდა დარჩეს პოლიტიკის ზღვარზე ქვემოთ).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` მრიცხველი (მხოლოდ ნულოვანი
    შეცდომით ინექციის დროს).
  - `connect.resume_latency_ms` ჰისტოგრამა (ჩაწერეთ p95 იძულებით
    ხელახლა დაკავშირება).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-ის სპეციფიკური `swift.connect.session_event` და
    `swift.connect.frame_latency` ექსპორტი (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Dashboards:** განაახლეთ Connect board სანიშნეები ანოტაციის მარკერებით.
  მიამაგრეთ ეკრანის ანაბეჭდები (ან JSON ექსპორტი) მტკიცებულების საქაღალდეში ნედლეულის გვერდით
  OTLP/Prometheus სნეპშოტები ამოღებული ტელემეტრიის ექსპორტიორის CLI-ით.
- **გაფრთხილება:** თუ რაიმე Sev1/2 ზღურბლები ამოქმედდება (`docs/source/android_support_playbook.md` §5-ზე),
  გვერდი SDK პროგრამის წამყვანი და დაასაბუთეთ PagerDuty ინციდენტის ID წიგნში
  ბილეთი გაგრძელებამდე.

## 5. გასუფთავება და დაბრუნება

1. **დადგმული სესიების წაშლა.** ყოველთვის წაშალეთ სესიების გადახედვისას რიგის სიღრმე
   სიგნალიზაცია რჩება მნიშვნელოვანი:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   მხოლოდ Swift-ის სატესტო გაშვებისთვის, გამოიძახეთ იგივე საბოლოო წერტილი Rust/CLI დამხმარე საშუალებით.
2. ** ჟურნალების გასუფთავება. ** წაშალეთ მუდმივი რიგის ჟურნალები
   (`ApplicationSupport/ConnectQueue/<sid>.to`, IndexedDB მაღაზიები და ა.შ.) ასე რომ
   შემდეგი გაშვება იწყება სუფთად. საჭიროების შემთხვევაში ჩაწერეთ ფაილის ჰეში წაშლამდე
   გამეორების პრობლემის გამართვა.
3. **ჩაწერეთ ინციდენტის შენიშვნები.** შეაჯამეთ გაშვება შემდეგში:
   - `docs/source/status/swift_weekly_digest.md` (დელტას ბლოკი),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (გასუფთავება ან შემცირება CR-2
     ტელემეტრიის დაყენების შემდეგ),
   - JS SDK ცვლილებების ჟურნალი ან რეცეპტი, თუ ახალი ქცევა დადასტურდა.
4. ** წარუმატებლობის ესკალაცია:**
   - რიგის გადადინება შეყვანილი ხარვეზების გარეშე ⇒ შეიტანეთ შეცდომა SDK-ის წინააღმდეგ, რომლის
     პოლიტიკა განსხვავდებოდა Torii-დან.
   - განაახლეთ შეცდომები ⇒ მიამაგრეთ `connect.queue_depth` + `connect.resume_latency_ms`
     ინციდენტის მოხსენების კადრები.
   - მმართველობის შეუსაბამობები (ჟეტონები ხელახლა გამოიყენება, TTL გადაჭარბებულია) ⇒ გაზრდა SDK-ით
     პროგრამა წარმართავს და ანოტირებს `roadmap.md` მომდევნო გადასინჯვისას.

## 6. მტკიცებულებათა ჩამონათვალი| არტეფაქტი | მდებარეობა |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| დაფის ექსპორტი (`connect.queue_depth` და ა.შ.) | `.../metrics/` ქვესაქაღალდე |
| PagerDuty / ინციდენტის ID | `.../notes.md` |
| გასუფთავების დადასტურება (Torii წაშლა, ჟურნალის წაშლა) | `.../cleanup.log` |

ამ საკონტროლო სიის შევსება აკმაყოფილებს „docs/runbooks განახლებულია“ გასასვლელის კრიტერიუმს
IOS7/JS4-ისთვის და მმართველობის მიმომხილველებს აძლევს განმსაზღვრელ კვალს ყველასთვის
გადახედვის სესიის დაკავშირება.