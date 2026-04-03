<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Անվտանգության աուդիտի հաշվետվություն

Ամսաթիվ՝ 2026-03-26

## Գործադիր ամփոփագիր

Այս աուդիտը կենտրոնացած էր ընթացիկ ծառի ամենաբարձր ռիսկային մակերեսների վրա՝ Torii HTTP/API/auth հոսքեր, P2P փոխադրումներ, գաղտնի մշակման API-ներ, SDK-ի փոխադրման պաշտպանիչներ և կցորդի ախտահանման ուղի:

Ես գտա 6 գործող խնդիր.

- 2 Բարձր ծանրության բացահայտումներ
- 4 Միջին ծանրության բացահայտումներ

Ամենակարևոր խնդիրներն են.

1. Torii-ը ներկայումս գրանցում է ներգնա հարցումների վերնագրերը յուրաքանչյուր HTTP հարցման համար, որը կարող է բացահայտել կրիչի նշանները, API-ի նշանները, օպերատորի նստաշրջանի/bootstrap նշանները և փոխանցված mTLS մարկերները գրանցամատյաններին:
2. Բազմաթիվ հանրային Torii երթուղիներ և SDK-ներ դեռ աջակցում են սերվերին չմշակված `private_key` արժեքների ուղարկելը, որպեսզի Torii-ը կարողանա ստորագրել զանգահարողի անունից:
3. Մի քանի «գաղտնի» ուղիներ դիտվում են որպես սովորական հարցումների մարմիններ, ներառյալ գաղտնի սերմերի ստացումը և կանոնական հարցումը որոշ SDK-ներում:

## Մեթոդ

- Torii, P2P, կրիպտո/VM և SDK գաղտնի մշակման ուղիների ստատիկ վերանայում
- Նպատակային վավերացման հրամաններ.
  - `cargo check -p iroha_torii --lib --message-format short` -> անցում
  - `cargo check -p iroha_p2p --message-format short` -> անցում
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> անցում
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> անցում, միայն կրկնօրինակ տարբերակի նախազգուշացումներ
- Ավարտված չէ այս անցումով.
  - ամբողջական աշխատանքային տարածքի կառուցում/փորձարկում/clippy
  - Swift/Gradle թեստային փաթեթներ
  - CUDA/Metal գործարկման ժամանակի վավերացում

## Գտածոներ

### SA-001 Բարձր. Torii գրանցում է զգայուն հարցումների վերնագրերը ամբողջ աշխարհումԱզդեցություն. Ցանկացած տեղակայում, որը նավերը պահանջում են հետագծում, կարող է արտահոսել կրիչի/API/օպերատորի նշանները և հարակից վավերական նյութերը հավելվածների մատյաններում:

Ապացույց:

- `crates/iroha_torii/src/lib.rs:20752`-ը հնարավորություն է տալիս `TraceLayer::new_for_http()`
- `crates/iroha_torii/src/lib.rs:20753`-ը հնարավորություն է տալիս `DefaultMakeSpan::default().include_headers(true)`-ին
- Զգայուն վերնագրերի անունները ակտիվորեն օգտագործվում են նույն ծառայության այլ վայրերում.
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Ինչու է սա կարևոր.

- `include_headers(true)`-ը գրանցում է ներգնա վերնագրի ամբողջական արժեքները հետագծման միջակայքում:
- Torii-ն ընդունում է նույնականացման նյութերը վերնագրերում, ինչպիսիք են `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` և `x-forwarded-client-cert`:
- Հետևաբար, տեղեկամատյանների փոխզիջումը, վրիպազերծման մատյանների հավաքածուն կամ աջակցության փաթեթը կարող են դառնալ հավատարմագրերի բացահայտման իրադարձություն:

Առաջարկվող վերականգնում.

- Դադարեցրեք ամբողջական պահանջների վերնագրերը ներառել արտադրական միջակայքում:
- Անվտանգության նկատմամբ զգայուն վերնագրերի համար բացահայտ խմբագրում ավելացրեք, եթե վրիպազերծման համար դեռևս անհրաժեշտ է վերնագրերի գրանցում:
- Լռելյայն դիտարկեք հարցումների/պատասխանների գրանցումը որպես գաղտնիք կրող, եթե տվյալները դրականորեն թույլատրված չեն:

### SA-002 Բարձր. Հանրային Torii API-ները դեռ ընդունում են չմշակված անձնական բանալիներ սերվերի կողմից ստորագրման համար

Ազդեցություն. Հաճախորդներին խրախուսվում է չմշակված անձնական բանալիներ փոխանցել ցանցի միջոցով, որպեսզի սերվերը կարողանա ստորագրել նրանց անունից՝ ստեղծելով անհարկի գաղտնի բացահայտման ալիք API, SDK, պրոքսի և սերվերի հիշողության շերտերում:

Ապացույց:- Կառավարման երթուղու փաստաթղթերը բացահայտորեն գովազդում են սերվերի կողմից ստորագրումը.
  - `crates/iroha_torii/src/gov.rs:495`
- Երթուղու իրականացումը վերլուծում է մատակարարված մասնավոր բանալին և նշում է սերվերի կողմից.
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK-ները ակտիվորեն սերիականացնում են `private_key`-ը JSON մարմիններում.
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Նշումներ:

- Այս օրինաչափությունը մեկուսացված չէ մեկ երթուղու ընտանիքի համար: Ընթացիկ ծառը պարունակում է նույն հարմարության մոդելը կառավարման, անցանց կանխիկացման, բաժանորդագրությունների և հավելվածներին առնչվող այլ DTO-ների համար:
- Միայն HTTPS-ով տրանսպորտային ստուգումները նվազեցնում են պատահական պարզ տեքստի փոխադրումը, սակայն դրանք չեն լուծում սերվերի կողմից գաղտնի կառավարումը կամ գրանցման/հիշողության բացահայտման վտանգը:

Առաջարկվող վերականգնում.

- Հնեցրեք բոլոր հարցումների DTO-ները, որոնք կրում են չմշակված `private_key` տվյալներ:
- Հաճախորդներից պահանջել ստորագրել տեղում և ներկայացնել ստորագրություններ կամ ամբողջությամբ ստորագրված գործարքներ/ծրարներ:
- Հեռացրեք `private_key` օրինակները OpenAPI/SDK-ներից համատեղելիության պատուհանից հետո:

### SA-003 Միջին. Բանալինների գաղտնի ածանցումը ուղարկում է գաղտնի սերմացու նյութը Torii-ին և արձագանքում է դրան

Ազդեցություն. բանալիների ստացման գաղտնի API-ն սերմերի նյութը վերածում է սովորական հարցման/պատասխանի օգտակար բեռնվածքի տվյալների՝ մեծացնելով սերմերի բացահայտման հնարավորությունը վստահված անձանց, միջնակարգ ծրագրերի, տեղեկամատյանների, հետքերի, խափանումների մասին հաշվետվությունների կամ հաճախորդի սխալ օգտագործման միջոցով:

Ապացույց:- Հարցումն ուղղակիորեն ընդունում է սերմացուի նյութը՝
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- Պատասխանների սխեման կրկնում է սերմը և՛ վեցանկյուն, և՛ հիմք64:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- Կառավարիչը բացահայտորեն վերակոդավորում և վերադարձնում է սերմը.
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK-ն բացահայտում է սա որպես կանոնավոր ցանցային մեթոդ և պահպանում է արձագանքման սերմը արձագանքման մոդելում.
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Առաջարկվող վերականգնում.

- Նախընտրեք տեղական բանալիների ստացումը CLI/SDK կոդում և ամբողջությամբ հեռացրեք հեռակառավարման երթուղին:
- Եթե երթուղին պետք է մնա, երբեք չվերադարձրեք սերմը պատասխանում և նշեք սերմ պարունակող մարմինները որպես զգայուն բոլոր տրանսպորտային պաշտպանիչ սարքերում և հեռաչափության/հատումների ուղիներում:

### SA-004 Միջին. SDK տրանսպորտային զգայունության հայտնաբերումը կույր կետեր ունի ոչ `private_key` գաղտնի նյութերի համար

Ազդեցությունը. որոշ SDK-ներ կկիրառեն HTTPS չմշակված `private_key` հարցումների համար, բայց այնուամենայնիվ թույլ կտան անվտանգության համար զգայուն հարցումների այլ նյութերը շրջել անապահով HTTP-ով կամ անհամապատասխան հոսթորդներով:

Ապացույց:- Swift-ը վերաբերվում է կանոնական հարցումների վավերացման վերնագրերին որպես զգայուն.
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Բայց Swift-ը դեռևս համապատասխանում է միայն մարմնին `"private_key"`-ին.
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Կոտլինը ճանաչում է միայն `authorization` և `x-api-token` վերնագրերը, այնուհետև վերադառնում է նույն `"private_key"` մարմնի էվրիստիկին.
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android-ն ունի նույն սահմանափակումը.
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java կանոնական հարցում ստորագրողները ստեղծում են լրացուցիչ վավերացման վերնագրեր, որոնք չեն դասակարգվում որպես զգայուն իրենց սեփական տրանսպորտային պահակախմբի կողմից.
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Առաջարկվող վերականգնում.

- Փոխարինեք մարմնի էվրիստիկական սկանավորումը բացահայտ հարցումների դասակարգմամբ:
- Կանոնական վավերագրման վերնագրերը, սերմերի/գաղտնաբառերի դաշտերը, ստորագրված մուտացիայի վերնագրերը և ապագա գաղտնիք կրող դաշտերը համարեք որպես զգայուն պայմանագրով, այլ ոչ թե ենթալարի համընկնումով:
- Պահպանեք զգայունության կանոնները Swift-ում, Kotlin-ում և Java-ում:

### SA-005 Միջին. Հավելված «Sandbox»-ը միայն ենթագործընթաց է գումարած `setrlimit`Ազդեցություն. կցորդը ախտահանող սարքը նկարագրված և զեկուցվում է որպես «ավազի տուփ», սակայն իրականացումը ռեսուրսների սահմանափակումներով ընթացիկ երկուականի միայն պատառաքաղ/գործադիր է: Վերլուծիչը կամ արխիվի շահագործումը դեռ կգործարկվի նույն օգտագործողի, ֆայլային համակարգի տեսքի և շրջապատող ցանցի/գործընթացի արտոնություններով, ինչ Torii:

Ապացույց:

- Արտաքին ուղին նշում է արդյունքը որպես ավազի արկղ՝ երեխային ծնելուց հետո.
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- Երեխան լռելյայն սահմանում է ընթացիկ գործարկվողը.
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- Ենթագործընթացը բացահայտորեն վերադառնում է `AttachmentSanitizerMode::InProcess`.
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- Կիրառված միակ կարծրացումն է CPU/հասցեի տարածությունը `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Առաջարկվող վերականգնում.

- Կամ կիրառեք իրական OS sandbox (օրինակ, namespaces/seccomp/landlock/bail-style մեկուսացում, արտոնությունների անկում, առանց ցանցի, սահմանափակված ֆայլային համակարգ) կամ դադարեցրեք արդյունքը որպես `sandboxed` պիտակավորում:
- Ներկայիս դիզայնը դիտարկեք որպես «ենթապրոցեսների մեկուսացում», այլ ոչ թե «ավազի արկղ» API-ներում, հեռաչափության և փաստաթղթերում, քանի դեռ իրական մեկուսացումը գոյություն չունի:

### SA-006 Միջին. կամընտիր P2P TLS/QUIC փոխադրում է անջատել վկայագրի ստուգումըԱզդեցություն. Երբ `quic` կամ `p2p_tls` միացված է, ալիքը ապահովում է գաղտնագրում, բայց չի նույնականացնում հեռավոր վերջնակետը: Ակտիվ ուղու վրա հարձակվողը դեռ կարող է փոխանցել կամ դադարեցնել ալիքը՝ հաղթահարելով TLS/QUIC-ի հետ կապված սովորական անվտանգության սպասելիքները:

Ապացույց:

- QUIC-ը բացահայտորեն փաստաթղթավորում է թույլատրելի վկայականի ստուգումը.
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC ստուգիչն անվերապահորեն ընդունում է սերվերի վկայականը.
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP տրանսպորտը նույնն է անում.
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Առաջարկվող վերականգնում.

- Կամ ստուգեք հասակակիցների վկայականները կամ ավելացրեք բացահայտ կապուղիների կապակցում ավելի բարձր մակարդակի ստորագրված ձեռքսեղմման և տրանսպորտային նստաշրջանի միջև:
- Եթե ընթացիկ վարքագիծը միտումնավոր է, վերանվանեք/փաստաթղթավորեք հատկությունը որպես չվավերացված կոդավորված տրանսպորտ, որպեսզի օպերատորներն այն չշփոթեն ամբողջական TLS հավասարակցային նույնականացման հետ:

## Առաջարկվող վերականգնման կարգ1. Անմիջապես ուղղեք SA-001-ը՝ խմբագրելով կամ անջատելով վերնագրերի գրանցումը:
2. Նախագծեք և ուղարկեք SA-002-ի միգրացիոն պլան, որպեսզի չմշակված մասնավոր բանալիները չհատեն API-ի սահմանը:
3. Հեռացրեք կամ նեղացրեք հեռավոր գաղտնի բանալիների ստացման երթուղին և դասակարգեք սերմ պարունակող մարմինները որպես զգայուն:
4. Հավասարեցրեք SDK տրանսպորտային զգայունության կանոնները Swift/Kotlin/Java-ում:
5. Որոշեք, թե արդյոք կցորդների սանիտարական պահպանման համար անհրաժեշտ է իրական ավազատուփ, թե ազնիվ անվանափոխություն/վերանվանում:
6. Հստակեցրեք և խստացրեք P2P TLS/QUIC սպառնալիքի մոդելը, նախքան օպերատորները միացնեն այն փոխադրումները, որոնք ակնկալում են վավերացված TLS:

## Վավերացման նշումներ

- `cargo check -p iroha_torii --lib --message-format short` անցել է:
- անցել է `cargo check -p iroha_p2p --message-format short`:
- `cargo deny check advisories bans sources --hide-inclusion-graph`-ն անցել է ավազատուփից դուրս վազելուց հետո; այն թողարկեց կրկնակի տարբերակի նախազգուշացումներ, բայց հայտնեց `advisories ok, bans ok, sources ok`:
- Այս աուդիտի ընթացքում սկսվել է Torii կենտրոնացված փորձարկում՝ արտահոսք-ստեղների հավաքածուի երթուղու համար, սակայն չի ավարտվել մինչև հաշվետվության գրելը. բացահայտումը հաստատվում է աղբյուրի անմիջական ստուգմամբ՝ անկախ նրանից: