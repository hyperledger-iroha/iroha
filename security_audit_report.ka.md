<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# უსაფრთხოების აუდიტის ანგარიში

თარიღი: 2026-03-26

## რეზიუმე

ეს აუდიტი ფოკუსირებული იყო მიმდინარე ხეზე ყველაზე მაღალი რისკის ზედაპირებზე: Torii HTTP/API/auth ნაკადები, P2P ტრანსპორტი, საიდუმლო დამუშავების API-ები, SDK სატრანსპორტო მცველები და დანართის სადეზინფექციო გზა.

აღმოვაჩინე 6 სამოქმედო საკითხი:

- 2 მაღალი სიმძიმის აღმოჩენა
- 4 საშუალო სიმძიმის აღმოჩენა

ყველაზე მნიშვნელოვანი პრობლემებია:

1. Torii ამჟამად აღრიცხავს შემომავალი მოთხოვნის სათაურებს ყოველი HTTP მოთხოვნისთვის, რომელსაც შეუძლია გამოავლინოს მატარებლის ნიშნები, API ტოკენები, ოპერატორის სესიის/ჩატვირთვის ნიშნები და გადაგზავნილი mTLS მარკერები ჟურნალებში.
2. მრავალი საჯარო Torii მარშრუტი და SDK კვლავ მხარს უჭერს სერვერზე დაუმუშავებელი `private_key` მნიშვნელობების გაგზავნას, რათა Torii შეძლოს ხელი მოაწეროს აბონენტის სახელით.
3. რამდენიმე „საიდუმლო“ გზა განიხილება როგორც ჩვეულებრივი მოთხოვნის ორგანო, მათ შორის კონფიდენციალური სათესლე დერივაცია და კანონიკური მოთხოვნის ავტორიზაცია ზოგიერთ SDK-ში.

## მეთოდი

- Torii, P2P, კრიპტო/VM და SDK საიდუმლო დამუშავების გზების სტატიკური მიმოხილვა
- მიზნობრივი ვალიდაციის ბრძანებები:
  - `cargo check -p iroha_torii --lib --message-format short` -> საშვი
  - `cargo check -p iroha_p2p --message-format short` -> საშვი
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> საშვი
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> საშვი, მხოლოდ დუბლიკატი ვერსიის გაფრთხილებები
- არ არის დასრულებული ამ პასში:
  - სრული სამუშაო სივრცის აშენება/ტესტი/კლიპი
  - Swift/Gradle სატესტო კომპლექტები
  - CUDA/Metal გაშვების ვალიდაცია

## დასკვნები

### SA-001 მაღალი: Torii აღრიცხავს მგრძნობიარე მოთხოვნის სათაურებს გლობალურადგავლენა: ნებისმიერ განლაგებას, რომელსაც გემები ითხოვენ მიკვლევას, შეუძლია გაჟონოს მატარებლის/API/ოპერატორის ნიშნები და შესაბამისი ავტორიზაციის მასალა აპლიკაციის ჟურნალებში.

მტკიცებულება:

- `crates/iroha_torii/src/lib.rs:20752` რთავს `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753` საშუალებას აძლევს `DefaultMakeSpan::default().include_headers(true)`
- სენსიტიური სათაურის სახელები აქტიურად გამოიყენება სხვაგან იმავე სერვისში:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

რატომ აქვს ამას მნიშვნელობა:

- `include_headers(true)` აღრიცხავს სრულ შემომავალი სათაურის მნიშვნელობებს ტრასირების სპინებში.
- Torii იღებს ავთენტიფიკაციის მასალებს სათაურებში, როგორიცაა `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` და `x-forwarded-client-cert`.
- ჟურნალის ჩაძირვის კომპრომისი, ჟურნალის გამართვის შეგროვება ან მხარდაჭერის ნაკრები შეიძლება გახდეს რწმუნებათა სიგელის გამჟღავნების მოვლენა.

რეკომენდირებული გამოსწორება:

- შეწყვიტეთ სრული მოთხოვნის სათაურების ჩართვა წარმოების საზღვრებში.
- დაამატეთ მკაფიო რედაქცია უსაფრთხოების მიმართ მგრძნობიარე სათაურებისთვის, თუ სათაურის ჟურნალი ჯერ კიდევ საჭიროა გამართვისთვის.
- მოთხოვნის/პასუხის აღრიცხვა ნაგულისხმევად განიხილება, როგორც საიდუმლოების შემცველი, თუ მონაცემები დადებითად არ არის ჩამოთვლილი.

### SA-002 მაღალი: საჯარო Torii API-ები კვლავ იღებენ დაუმუშავებელ პირად გასაღებებს სერვერის მხრიდან ხელმოწერისთვის

გავლენა: კლიენტებს ურჩევენ გადასცენ ნედლეული პირადი გასაღებები ქსელში, რათა სერვერმა შეძლოს ხელი მოაწეროს მათ სახელით, შექმნას არასაჭირო საიდუმლოების გამოვლენის არხი API, SDK, პროქსი და სერვერის მეხსიერების ფენებზე.

მტკიცებულება:- მმართველობის მარშრუტის დოკუმენტაცია ცალსახად აქვეყნებს სერვერის ხელმოწერას:
  - `crates/iroha_torii/src/gov.rs:495`
- მარშრუტის განხორციელება აანალიზებს მოწოდებულ პირად გასაღებს და ხელს აწერს სერვერის მხარეს:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK-ები აქტიურად ახდენენ `private_key` სერიებს JSON სხეულებში:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

შენიშვნები:

- ეს ნიმუში არ არის იზოლირებული ერთი მარშრუტის ოჯახში. ამჟამინდელი ხე შეიცავს იგივე მოხერხებულობის მოდელს მმართველობის, ოფლაინ ნაღდი ფულის, გამოწერების და სხვა აპების წინაშე მდგარ DTO-ებში.
- მხოლოდ HTTPS-ის სატრანსპორტო შემოწმებები ამცირებს შემთხვევითი ტექსტის ტრანსპორტს, მაგრამ ისინი არ წყვეტენ სერვერის მხარეს საიდუმლო დამუშავებას ან ჟურნალის/მეხსიერების ზემოქმედების რისკს.

რეკომენდირებული გამოსწორება:

- გააუქმეთ ყველა მოთხოვნის DTO, რომლებიც ატარებენ დაუმუშავებელ `private_key` მონაცემებს.
- მოითხოვეთ კლიენტებისგან ხელი მოაწერონ ადგილობრივად და წარადგინონ ხელმოწერები ან სრულად ხელმოწერილი გარიგებები/კონვერტები.
- წაშალეთ `private_key` მაგალითები OpenAPI/SDK-ებიდან თავსებადობის ფანჯრის შემდეგ.

### SA-003 საშუალო: კონფიდენციალური გასაღების დერივაცია აგზავნის საიდუმლო სათესლე მასალას Torii-ზე და უბრუნებს მას.

გავლენა: კონფიდენციალური გასაღების წარმოშობის API აქცევს სათესლე მასალას ნორმალურ მოთხოვნის/პასუხის დატვირთვის მონაცემებად, ზრდის სათესლე მასალის გამჟღავნების შანსს პროქსიების, შუა პროგრამების, ჟურნალების, კვალის, ავარიის შესახებ ანგარიშების ან კლიენტის არასწორად გამოყენების მეშვეობით.

მტკიცებულება:- მოთხოვნა პირდაპირ იღებს სათესლე მასალას:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- პასუხის სქემა ეხმიანება თესლს, როგორც ექვსკუთხედში, ასევე ფუძე64-ში:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- დამმუშავებელი აშკარად ხელახლა შიფრავს და აბრუნებს თესლს:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK ავლენს ამას, როგორც ქსელის რეგულარულ მეთოდს და აგრძელებს გამოხმაურებას პასუხების მოდელში:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

რეკომენდირებული გამოსწორება:

- უპირატესობა მიანიჭეთ გასაღების ლოკალურ დერივაციას CLI/SDK კოდში და მთლიანად წაშალეთ დისტანციური დერივაციის მარშრუტი.
- თუ მარშრუტი უნდა დარჩეს, არასოდეს დააბრუნოთ თესლი პასუხში და მონიშნეთ თესლის შემცველი ორგანოები, როგორც მგრძნობიარე ყველა სატრანსპორტო მცველში და ტელემეტრიულ/საჭრელ ბილიკზე.

### SA-004 საშუალო: SDK ტრანსპორტის მგრძნობელობის ამოცნობას აქვს ბრმა წერტილები არა-`private_key` საიდუმლო მასალისთვის

გავლენა: ზოგიერთი SDK აიძულებს HTTPS-ს დაუმუშავებელი `private_key` მოთხოვნისთვის, მაგრამ მაინც დაუშვებს უსაფრთხოების მიმართ მგრძნობიარე მოთხოვნის სხვა მასალას გადაადგილდეს დაუცველ HTTP-ზე ან შეუსაბამო ჰოსტებში.

მტკიცებულება:- Swift განიხილავს კანონიკური მოთხოვნის ავტორიზაციის სათაურებს, როგორც მგრძნობიარე:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- მაგრამ Swift ჯერ კიდევ მხოლოდ სხეულის შესატყვისია `"private_key"`-ზე:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- კოტლინი ამოიცნობს მხოლოდ `authorization` და `x-api-token` სათაურებს, შემდეგ უბრუნდება იგივე `"private_key"` სხეულის ევრისტიკას:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android-ს აქვს იგივე შეზღუდვა:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java კანონიკური მოთხოვნის ხელმომწერები ქმნიან დამატებით ავტორიზაციის სათაურებს, რომლებიც არ არის კლასიფიცირებული, როგორც სენსიტიური მათი სატრანსპორტო მცველების მიერ:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

რეკომენდირებული გამოსწორება:

- შეცვალეთ სხეულის ევრისტიკული სკანირება მკაფიო მოთხოვნის კლასიფიკაციით.
- მოიხსენიეთ კანონიკური ავტორიზაციის სათაურები, სათაური/ფრაზის ველები, ხელმოწერილი მუტაციის სათაურები და ნებისმიერი მომავალი საიდუმლოების შემცველი ველი, როგორც მგრძნობიარე კონტრაქტით და არა ქვესტრიქონების შესატყვისით.
- შეინახეთ მგრძნობელობის წესები Swift-ში, Kotlin-სა და Java-ში.

### SA-005 საშუალო: დანართი "Sandbox" არის მხოლოდ ქვეპროცესი პლუს `setrlimit`ზემოქმედება: დანართის სადეზინფექციო საშუალება აღწერილია და მოხსენებულია, როგორც "ქვიშის ყუთი", მაგრამ დანერგვა არის მხოლოდ მიმდინარე ბინარის ჩანგალი/შესრულება რესურსის ლიმიტებით. პარსერი ან არქივის ექსპლოიტი კვლავ შესრულდება იგივე მომხმარებლის, ფაილური სისტემის ხედით და გარემოს ქსელის/პროცესის პრივილეგიებით, როგორც Torii.

მტკიცებულება:

- გარე გზა აღნიშნავს შედეგს, როგორც ქვიშის ყუთში ბავშვის ქვირითის შემდეგ:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- ბავშვი ნაგულისხმევია მიმდინარე შესრულებადზე:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- ქვეპროცესი პირდაპირ გადადის `AttachmentSanitizerMode::InProcess`-ზე:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- გამოყენებული ერთადერთი გამკვრივება არის CPU/მისამართების სივრცე `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

რეკომენდირებული გამოსწორება:

- ან განახორციელეთ რეალური OS sandbox (მაგალითად, namespaces/seccomp/landlock/jail-style იზოლაცია, პრივილეგიების დაცემა, ქსელის გარეშე, შეზღუდული ფაილური სისტემა) ან შეწყვიტეთ შედეგის ეტიკეტირება, როგორც `sandboxed`.
- განიხილეთ მიმდინარე დიზაინი, როგორც "ქვეპროცესის იზოლაცია" და არა "სანბოქსი" API-ებში, ტელემეტრიაში და დოკუმენტებში, სანამ არ იქნება ნამდვილი იზოლაცია.

### SA-006 საშუალო: სურვილისამებრ P2P TLS/QUIC ტრანსპორტირებს სერთიფიკატის დადასტურების გამორთვასგავლენა: როდესაც `quic` ან `p2p_tls` ჩართულია, არხი უზრუნველყოფს დაშიფვრას, მაგრამ არ ახდენს დისტანციური საბოლოო წერტილის ავთენტიფიკაციას. გზაზე აქტიურ თავდამსხმელს მაინც შეუძლია გადასცეს ან შეწყვიტოს არხი, დაამარცხებს უსაფრთხოების ნორმალური მოლოდინების ოპერატორებს, რომლებიც დაკავშირებულია TLS/QUIC-თან.

მტკიცებულება:

- QUIC ცალსახად ადასტურებს ნებადართული სერტიფიკატის დადასტურებას:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC ვერიფიკატორი უპირობოდ იღებს სერვერის სერთიფიკატს:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP ტრანსპორტი იგივეს აკეთებს:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

რეკომენდირებული გამოსწორება:

- ან გადაამოწმეთ თანატოლების სერთიფიკატები, ან დაამატეთ არხის მკაფიო კავშირი უფრო მაღალ დონეზე ხელმოწერილი ხელის ჩამორთმევასა და სატრანსპორტო სესიას შორის.
- თუ ამჟამინდელი ქცევა მიზანმიმართულია, გადაარქვით მახასიათებელს სახელი/დოკუმენტირება, როგორც არაავთენტიფიცირებული დაშიფრული ტრანსპორტი, რათა ოპერატორებმა ის არ შეცდომით შეცდომით შეასრულონ TLS თანატოლთა ავთენტიფიკაცია.

## რეკომენდირებული გამოსწორების ორდერი1. სასწრაფოდ დააფიქსირეთ SA-001 სათაურის ჟურნალის რედაქტირებით ან გამორთვით.
2. შეიმუშავეთ და გაგზავნეთ SA-002-ის მიგრაციის გეგმა ისე, რომ დაუმუშავებელი პირადი გასაღებები შეაჩერონ API საზღვრის გადაკვეთა.
3. ამოიღეთ ან შეზღუდეთ დისტანციური კონფიდენციალური გასაღების დერივაციის მარშრუტი და დაახარისხეთ თესლის შემცველი ორგანოები, როგორც მგრძნობიარე.
4. გაასწორეთ SDK ტრანსპორტის მგრძნობელობის წესები Swift/Kotlin/Java-ში.
5. გადაწყვიტეთ, სჭირდება თუ არა დანართი სანიტარიული ქვიშის ყუთი თუ პატიოსანი სახელის შეცვლა/გადაკეთება.
6. გაარკვიეთ და გაამკაცრეთ P2P TLS/QUIC საფრთხის მოდელი, სანამ ოპერატორები ჩართავენ იმ ტრანსპორტებს, რომლებიც ელოდება ავთენტიფიცირებულ TLS-ს.

## დადასტურების შენიშვნები

- `cargo check -p iroha_torii --lib --message-format short` გავიდა.
- `cargo check -p iroha_p2p --message-format short` გავიდა.
- `cargo deny check advisories bans sources --hide-inclusion-graph` გავიდა ქვიშის ყუთის გარეთ გაშვების შემდეგ; მან გამოუშვა დუბლიკატი ვერსიის გაფრთხილებები, მაგრამ იტყობინება `advisories ok, bans ok, sources ok`.
- ფოკუსირებული Torii ტესტი კონფიდენციალური წარმომავლობა-გასაღების მარშრუტისთვის დაიწყო ამ აუდიტის დროს, მაგრამ არ დასრულებულა ანგარიშის დაწერამდე; დასკვნა მხარდაჭერილია პირდაპირი წყაროს ინსპექტირებით, მიუხედავად იმისა.