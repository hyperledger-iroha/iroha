---
lang: ka
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

# დააკავშირეთ ქაოსი და გაუმართაობის რეპეტიციის გეგმა (IOS3 / IOS7)

ეს სათამაშო წიგნი განსაზღვრავს განმეორებად ქაოსის წვრთნებს, რომლებიც აკმაყოფილებს IOS3/IOS7-ს
საგზაო რუკა ქმედება _"დაგეგმეთ ერთობლივი ქაოსის რეპეტიცია"_ (`roadmap.md:1527`). დააწყვილეთ იგი
Connect preview runbook (`docs/runbooks/connect_session_preview_runbook.md`)
cross-SDK დემოს დადგმისას.

## მიზნები და წარმატების კრიტერიუმები
- განახორციელეთ გაზიარებული Connect-ის განმეორებითი ცდის/დაბრუნების პოლიტიკა, ხაზგარეშე რიგის ლიმიტები და
  ტელემეტრიის ექსპორტიორები კონტროლირებადი ხარვეზების ქვეშ წარმოების კოდის მუტაციის გარეშე.
- გადაიღეთ დეტერმინისტული არტეფაქტები (`iroha connect queue inspect` გამომავალი,
  `connect.*` მეტრიკის სნეპშოტები, Swift/Android/JS SDK ჟურნალები), რათა მმართველობამ შეძლოს
  აუდიტი ყოველი სავარჯიშო.
- დაამტკიცეთ, რომ საფულეები და dApps პატივს სცემენ კონფიგურაციის ცვლილებებს (მანიფესტი დრიფტები, მარილი
  როტაცია, ატესტაციის წარუმატებლობა) კანონიკური `ConnectError`-ის ზედაპირით
  კატეგორიის და რედაქციისთვის უსაფრთხო ტელემეტრიული მოვლენები.

## წინაპირობები
1. **გარემოს ჩამტვირთავი **
   - დაიწყეთ დემო Torii სტეკი: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - გაუშვით მინიმუმ ერთი SDK ნიმუში (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **ინსტრუმენტაცია**
   - ჩართეთ SDK დიაგნოსტიკა (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` სვიფტში; `ConnectQueueJournal` + `ConnectQueueJournalTests`
     ეკვივალენტები Android/JS-ში).
   - დარწმუნდით, რომ CLI `iroha connect queue inspect --sid <sid> --metrics` მოგვარდება
     SDK-ის მიერ წარმოებული რიგის გზა (`~/.iroha/connect/<sid>/state.json` და
     `metrics.ndjson`).
   - მავთულის ტელემეტრიის ექსპორტიორები, ასე რომ, შემდეგი დროის სერიები ჩანს
     Grafana და `scripts/swift_status_export.py telemetry` მეშვეობით: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **მტკიცებულებების საქაღალდეები** – შექმენით `artifacts/connect-chaos/<date>/` და შეინახეთ:
   - დაუმუშავებელი ჟურნალი (`*.log`), მეტრიკის სნეპშოტები (`*.json`), დაფის ექსპორტი
     (`*.png`), CLI გამომავალი და PagerDuty ID.

## სცენარის მატრიცა| ID | ბრალია | ინექციის ნაბიჯები | მოსალოდნელი სიგნალები | მტკიცებულება |
|----|-------|----------------|-----------------|---------|
| C1 | WebSocket გათიშვა და ხელახლა დაკავშირება | შემოახვიეთ `/v1/connect/ws` პროქსის მიღმა (მაგ., `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) ან დროებით დაბლოკეთ სერვისი (`kubectl scale deploy/torii --replicas=0` ≤60 წმ.). აიძულეთ საფულე გააგრძელოს ჩარჩოების გაგზავნა ისე, რომ ხაზგარეშე რიგები გაივსოს. | `connect.reconnects_total` იზრდება, `connect.resume_latency_ms` იზრდება, მაგრამ რჩება - დაფის ანოტაცია გათიშვის ფანჯრისთვის.- ჟურნალის ამონაწერის ნიმუში ხელახლა დაკავშირებით + გადინების შეტყობინებებით. |
| C2 | ხაზგარეშე რიგის გადადინება / TTL ვადა | შეასწორეთ ნიმუში რიგის ლიმიტების შესამცირებლად (Swift: Instantiate `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` `ConnectSessionDiagnostics`-ში; Android/JS გამოიყენეთ შესაბამისი კონსტრუქტორები). შეაჩერეთ საფულე ≥2× `retentionInterval`-ისთვის, სანამ dApp განაგრძობს მოთხოვნებს რიგში. | `connect.queue_dropped_total{reason="overflow"}` და `{reason="ttl"}` ზრდა, `connect.queue_depth` პლატოები ახალ ლიმიტზე, SDK-ების ზედაპირი `ConnectError.QueueOverflow(limit: 4)` (ან `.QueueExpired`). `iroha connect queue inspect` აჩვენებს `state=Overflow`-ს `warn/drop` ჭვირნიშნით 100%. | - მეტრულ მრიცხველების სკრინშოტი.- CLI JSON გამომავალი ჩაწერის ზედმეტად.- Swift/Android ჟურნალის ფრაგმენტი, რომელიც შეიცავს `ConnectError` ხაზს. |
| C3 | მანიფესტი დრიფტი / დაშვების უარყოფა | შეცვალეთ Connect მანიფესტი, რომელიც ემსახურება საფულეებს (მაგ., შეცვალეთ `docs/connect_swift_ios.md` ნიმუშის მანიფესტი, ან დაიწყეთ Torii `--connect-manifest-path`-ით, რომელიც მიუთითებს ასლზე, ​​სადაც `chain_id` ან `permissions`). მოითხოვეთ dApp-მა დამტკიცება და დარწმუნდით, რომ საფულე უარყოფს პოლიტიკას. | Torii აბრუნებს `HTTP 409`-ს `/v1/connect/session`-ისთვის `manifest_mismatch`-ით, SDK-ები ასხივებენ `ConnectError.Authorization.manifestMismatch(manifestVersion)`, ტელემეტრია ამაღლებს `connect.manifest_mismatch_total`-ს, ხოლო რიგები ცარიელი რჩება. (`state=Idle`). | - Torii ჟურნალის ამონაწერი, რომელიც გვიჩვენებს შეუსაბამობის აღმოჩენას.- SDK-ის ეკრანის ანაბეჭდი ზედაპირზე გამოჩენილი შეცდომის შესახებ.- მეტრიკის სნეპშოტი, რომელიც ადასტურებს, რომ ტესტის დროს რიგი კადრები არ არის. |
| C4 | გასაღების როტაცია / მარილის ვერსიის მუწუკი | შეატრიალეთ Connect salt ან AEAD გასაღები სესიის შუა პერიოდში. დეველოპერის სტეკებში გადატვირთეთ Torii `CONNECT_SALT_VERSION=$((old+1))`-ით (ასახავს Android-ის რედაქციის მარილის ტესტს `docs/source/sdk/android/telemetry_schema_diff.md`-ში). შეინახეთ საფულე ხაზგარეშე, სანამ მარილის როტაცია არ დასრულდება, შემდეგ განაახლეთ. | რეზიუმეს პირველი მცდელობა წარუმატებელია `ConnectError.Authorization.invalidSalt`-ით, რიგები იშლება (dApp ჩამოაგდებს ქეშურ კადრებს `salt_version_mismatch` მიზეზით), ტელემეტრია ასხივებს `android.telemetry.redaction.salt_version` (Android) და `swift.connect.session_event{event="salt_rotation"}`. მეორე სესია SID განახლების შემდეგ წარმატებით დასრულდა. | - საინფორმაციო დაფის ანოტაცია მარილის ეპოქით ადრე/შემდეგ.- ჟურნალები, რომლებიც შეიცავს invalid-salt შეცდომას და შემდგომ წარმატებას.- `iroha connect queue inspect` გამომავალი აჩვენებს `state=Stalled`, რასაც მოჰყვება ახალი `state=Active`. || C5 | ატესტაცია / StrongBox წარუმატებლობა | Android საფულეებზე, დააკონფიგურირეთ `ConnectApproval`, რომ შეიცავდეს `attachments[]` + StrongBox ატესტაციას. გამოიყენეთ ატესტაციის აღკაზმულობა (`scripts/android_keystore_attestation.sh` `--inject-failure strongbox-simulated`-თან ერთად) ან შეცვალეთ ატესტაციის JSON dApp-ისთვის გადაცემამდე. | DApp უარყოფს დამტკიცებას `ConnectError.Authorization.invalidAttestation`-ით, Torii აღრიცხავს წარუმატებლობის მიზეზს, ექსპორტიორები აჯამებენ `connect.attestation_failed_total` და რიგი ასუფთავებს შეურაცხმყოფელ ჩანაწერს. Swift/JS dApps წერს შეცდომას სესიის გატარებისას. | - დამაგრების ჟურნალი შეყვანილი წარუმატებლობის ID-ით.- SDK შეცდომების ჟურნალი + ტელემეტრიის მრიცხველის აღბეჭდვა.- მტკიცებულება იმისა, რომ რიგმა ამოიღო ცუდი ჩარჩო (`recordsRemoved > 0`). |

## სცენარის დეტალები

### C1 - WebSocket გათიშვა და ხელახლა დაკავშირება
1. შემოახვიეთ Torii პროქსის (toxiproxy, Envoy, ან `kubectl port-forward`) უკან ასე
   თქვენ შეგიძლიათ გადართოთ ხელმისაწვდომობა მთელი კვანძის მოკვლის გარეშე.
2. გამორთვა 45 წამში:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. დააკვირდით ტელემეტრიის დაფებს და `სკრიპტებს/swift_status_export.py ტელემეტრიას
   --json-out artifacts/connect-chaos//c1_metrics.json`.
4. ნაგავსაყრელის რიგის მდგომარეობა გათიშვისთანავე:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. წარმატება = ხელახლა დაკავშირების ერთჯერადი მცდელობა, შემოსაზღვრული რიგის ზრდა და ავტომატური
   გადინება პროქსის აღდგენის შემდეგ.

### C2 — ხაზგარეშე რიგის გადინება / TTL ვადა
1. რიგების ზღურბლების შემცირება ლოკალურ შენობებში:
   - Swift: განაახლეთ `ConnectQueueJournal` ინიციალატორი თქვენს ნიმუშში
     (მაგ., `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     გაიაროს `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: კონსტრუირებისას გადაიტანეთ ექვივალენტური კონფიგურაციის ობიექტი
     `ConnectQueueJournal`.
2. შეაჩერეთ საფულე (სიმულატორის ფონი ან მოწყობილობის თვითმფრინავის რეჟიმი) ≥60 წმ.
   ხოლო dApp გასცემს `ConnectClient.requestSignature(...)` ზარებს.
3. გამოიყენეთ `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) ან JS
   დიაგნოსტიკური დამხმარე მტკიცებულებების ნაკრების ექსპორტისთვის (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. წარმატება = გადინების მრიცხველების ზრდა, SDK ზედაპირები `ConnectError.QueueOverflow`
   ერთხელ და რიგი აღდგება საფულის განახლების შემდეგ.

### C3 - მანიფესტი დრიფტი / დაშვების უარყოფა
1. გააკეთეთ დაშვების მანიფესტის ასლი, მაგ.:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. გაუშვით Torii `--connect-manifest-path /tmp/manifest_drift.json`-ით (ან
   განაახლეთ docker compose/k8s კონფიგურაცია საბურღისთვის).
3. სცადეთ სესიის დაწყება საფულედან; ველით HTTP 409.
4. გადაიღეთ Torii + SDK ჟურნალები პლუს `connect.manifest_mismatch_total`
   ტელემეტრიის დაფა.
5. წარმატება = უარი რიგის ზრდის გარეშე, პლუს საფულე აჩვენებს გაზიარებულს
   ტაქსონომიის შეცდომა (`ConnectError.Authorization.manifestMismatch`).### C4 - გასაღების როტაცია / მარილის ნაკაწრი
1. ჩაიწერეთ მარილის მიმდინარე ვერსია ტელემეტრიიდან:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. გადატვირთეთ Torii ახალი მარილით (`CONNECT_SALT_VERSION=$((OLD+1))` ან განაახლეთ
   კონფიგურაციის რუკა). შეინახეთ საფულე ხაზგარეშე გადატვირთვის დასრულებამდე.
3. განაახლეთ საფულე; პირველი რეზიუმე უნდა ჩავარდეს არასწორი მარილის შეცდომით
   და `connect.queue_dropped_total{reason="salt_version_mismatch"}` მატება.
4. აიძულეთ აპი ჩამოაგდოს ქეშირებული ფრეიმები სესიების დირექტორიას წაშლით
   (`rm -rf ~/.iroha/connect/<sid>` ან პლატფორმის სპეციფიკური ქეში გასუფთავებულია), მაშინ
   განაახლეთ სესია ახალი ჟეტონებით.
5. წარმატება = ტელემეტრია აჩვენებს მარილის ნაკაწრს, არასწორი რეზიუმეს მოვლენა არის ჩაწერილი
   ერთხელ, და შემდეგი სესია წარმატებულია ხელით ჩარევის გარეშე.

### C5 — ატესტაცია / StrongBox მარცხი
1. შექმენით ატესტაციის ნაკრები `scripts/android_keystore_attestation.sh`-ის გამოყენებით
   (დააყენეთ `--inject-failure strongbox-simulated` ხელმოწერის ბიტის გადასაბრუნებლად).
2. სთხოვეთ საფულეს მიამაგროს ეს პაკეტი მისი `ConnectApproval` API-ით; dApp
   უნდა დაადასტუროს და უარყოს დატვირთვა.
3. გადაამოწმეთ ტელემეტრია (`connect.attestation_failed_total`, Swift/Android ინციდენტი
   მეტრიკა) და დარწმუნდით, რომ რიგმა ჩამოაგდო მოწამლული ჩანაწერი.
4. წარმატება = უარყოფა იზოლირებულია ცუდი მოწონებით, რიგები რჩება ჯანმრთელი,
   ხოლო ატესტაციის ჟურნალი ინახება საბურღი მტკიცებულებებთან ერთად.

## მტკიცებულებათა ჩამონათვალი
- `artifacts/connect-chaos/<date>/c*_metrics.json` ექსპორტს
  `scripts/swift_status_export.py telemetry`.
- CLI გამომავალი (`c*_queue.txt`) `iroha connect queue inspect`-დან.
- SDK + Torii ჟურნალი დროის შტამპებით და SID ჰეშებით.
- დაფის ეკრანის ანაბეჭდები ანოტაციებით თითოეული სცენარისთვის.
- PagerDuty / ინციდენტის პირადობის მოწმობები, თუ Sev1/2 გაფრთხილებები გაშვებულია.

სრული მატრიცის შევსება კვარტალში ერთხელ აკმაყოფილებს საგზაო რუქის კარიბჭეს და
გვიჩვენებს, რომ Swift/Android/JS Connect განხორციელებები განმსაზღვრელად რეაგირებენ
ყველაზე მაღალი რისკის მარცხის რეჟიმებში.