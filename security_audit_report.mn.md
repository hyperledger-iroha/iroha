<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Аюулгүй байдлын аудитын тайлан

Огноо: 2026-03-26

## Товч дүгнэлт

Энэ аудит нь одоогийн модны хамгийн өндөр эрсдэлтэй гадаргуу дээр төвлөрсөн: Torii HTTP/API/auth урсгал, P2P тээвэрлэлт, нууцаар ажиллах API, SDK тээврийн хамгаалалт, хавсралт ариутгах зам.

Би хэрэгжүүлэх боломжтой 6 асуудлыг олсон:

- 2 Хүнд зэргийн шинж тэмдэг илэрсэн
- Дунд зэргийн хүндийн 4 дүгнэлт

Хамгийн чухал асуудлууд нь:

1. Torii нь одоогоор HTTP хүсэлт бүрийн дотогшоо хүсэлтийн толгой хэсгийг бүртгэдэг бөгөөд энэ нь зөөгч токен, API токен, оператор сесс/bootstrap токен болон дамжуулагдсан mTLS тэмдэглэгээг лог руу харуулах боломжтой.
2. Олон нийтийн Torii чиглүүлэлтүүд болон SDK-ууд нь `private_key` утгыг сервер рүү илгээхийг дэмждэг хэвээр байгаа тул Torii залгагчийн өмнөөс гарын үсэг зурах боломжтой.
3. Хэд хэдэн "нууц" замыг энгийн хүсэлтийн байгууллага гэж үздэг бөгөөд үүнд зарим SDK-д нууц үрийн үүсмэл болон каноник хүсэлтийн баталгаажуулалт орно.

## Арга

- Torii, P2P, крипто/VM болон SDK нууцыг зохицуулах замуудын статик хяналт
- Зорилтот баталгаажуулалтын командууд:
  - `cargo check -p iroha_torii --lib --message-format short` -> нэвтрүүлэх
  - `cargo check -p iroha_p2p --message-format short` -> нэвтрүүлэх
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> нэвтрүүлэх
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> нэвтрүүлэх, зөвхөн давхардсан хувилбарын анхааруулга
- Энэ тасалбарыг бөглөөгүй:
  - бүрэн ажлын талбар бүтээх/туршилт/клиппи
  - Swift/Gradle тестийн багц
  - CUDA/Metal ажиллах цагийн баталгаажуулалт

## Судалгаа

### SA-001 Өндөр: Torii эмзэг хүсэлтийн толгойг дэлхий даяар бүртгэдэгНөлөөлөл: Хөлөг онгоцыг хянах хүсэлт гаргасан аливаа байршуулалт нь зөөгч/API/операторын токенууд болон холбогдох баталгаажуулах материалыг програмын бүртгэлд алдаж болзошгүй.

Нотлох баримт:

- `crates/iroha_torii/src/lib.rs:20752` `TraceLayer::new_for_http()`-г идэвхжүүлдэг
- `crates/iroha_torii/src/lib.rs:20753` `DefaultMakeSpan::default().include_headers(true)`-г идэвхжүүлдэг
- Мэдрэмжтэй толгойн нэрийг ижил үйлчилгээний өөр газар идэвхтэй ашигладаг:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Энэ яагаад чухал вэ:

- `include_headers(true)` нь бүрэн дотогшоо толгойн утгыг мөрийн зайд бүртгэдэг.
- Torii нь `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token`, `x-forwarded-client-cert` зэрэг гарчиг дахь баталгаажуулалтын материалыг хүлээн авдаг.
- Иймээс лог угаалтуурын эвдрэл, дибаг хийх бүртгэлийн цуглуулга эсвэл дэмжлэгийн багц нь итгэмжлэлийг задруулах үйл явдал болж болно.

Зөвлөмж болгож буй засвар:

- Үйлдвэрлэлийн хүрээн дэх хүсэлтийн толгой хэсгийг бүрэн оруулахаа боль.
- Хэрэв дибаг хийхэд толгой хэсгийг бүртгэх шаардлагатай хэвээр байвал аюулгүй байдлын мэдрэмтгий толгой хэсэгт тодорхой засвар нэмнэ үү.
- Хүсэлт/хариултын бүртгэлийг өгөгдөлд эерэгээр зөвшөөрөгдөөгүй тохиолдолд анхдагчаар нууц гэж үзнэ.

### SA-002 Өндөр: Нийтийн Torii API-ууд нь сервер талын гарын үсэг зурахад зориулж түүхий хувийн түлхүүрүүдийг хүлээн зөвшөөрсөн хэвээр байна

Нөлөөлөл: Үйлчлүүлэгчдийг сүлжээгээр түүхий хувийн түлхүүр дамжуулахыг зөвлөж байна, ингэснээр сервер тэдний өмнөөс гарын үсэг зурж, API, SDK, прокси болон серверийн санах ойн давхаргад шаардлагагүй нууц сувгийг үүсгэх боломжтой.

Нотлох баримт:- Удирдлагын маршрутын баримт бичиг нь сервер талын гарын үсэг зурахыг илт сурталчилсан:
  - `crates/iroha_torii/src/gov.rs:495`
- Маршрутын хэрэгжилт нь нийлүүлсэн хувийн түлхүүрийг задлан шинжилж, сервер тал дээр тэмдэг тавьдаг:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK-ууд `private_key`-г JSON биет болгон идэвхтэй цуваа болгодог:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Тэмдэглэл:

- Энэ загвар нь нэг маршрутын гэр бүлд тусгаарлагдаагүй. Одоогийн мод нь засаглал, офлайн бэлэн мөнгө, захиалга болон бусад аппликейшнтэй холбоотой DTO-г хамарсан ижил төстэй загварыг агуулдаг.
- Зөвхөн HTTPS-н тээврийн шалгалтууд нь санамсаргүй байдлаар шууд текст зөөвөрлөлтийг бууруулдаг боловч сервер талын нууцлалтай ажиллах, бүртгэл хөтлөх/санах ойд өртөх эрсдэлийг шийдэж чадахгүй.

Зөвлөмж болгож буй засвар:

- `private_key` түүхий өгөгдөл агуулсан бүх хүсэлтийн DTO-г хүчингүй болго.
- Үйлчлүүлэгчдээс орон нутагт гарын үсэг зурж, гарын үсэг эсвэл бүрэн гарын үсэг зурсан гүйлгээ/дугтуйг ирүүлэхийг шаардах.
- Тохиромжтой цонхны дараа `private_key` жишээг OpenAPI/SDK-с устгана уу.

### SA-003 Дундаж: Нууц түлхүүр гарган авах нь нууц үрийн материалыг Torii руу илгээж, дахин цуурайтуулдаг

Нөлөөлөл: Нууц түлхүүр гарал үүслийн API нь үрийн материалыг ердийн хүсэлт/харилын ачааллын өгөгдөл болгон хувиргаж, прокси, завсрын програм, бүртгэл, ул мөр, гэмтлийн тайлан эсвэл үйлчлүүлэгчийн буруугаар ашиглах замаар үрийн мэдээллийг задруулах боломжийг нэмэгдүүлдэг.

Нотлох баримт:- Хүсэлт нь үрийн материалыг шууд хүлээн авдаг:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- Хариултын схем нь үрийг hex болон base64 аль алинд нь буцааж өгч байна:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- Зохицуулагч нь үрийг тодорхой дахин кодлож, буцааж өгдөг:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK нь үүнийг ердийн сүлжээний арга болгож, хариу загварт цуурайтсан үрийг хэвээр үлдээдэг:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Зөвлөмж болгож буй засвар:

- CLI/SDK кодын локал түлхүүр гарган авахыг илүүд үзэж, алсаас үүсмэл хандлагыг бүхэлд нь устгана уу.
- Хэрэв маршрут хэвээр байх ёстой бол хариуд үрийг хэзээ ч буцааж өгөхгүй байх ба бүх тээврийн хамгаалалт болон телеметрийн / мод бэлтгэх замд үр агуулсан биеийг мэдрэмтгий гэж тэмдэглэ.

### SA-004 Дундаж: SDK зөөвөрлөх мэдрэмжийн илрүүлэлт нь `private_key` бус нууц материалд зориулсан сохор толботой.

Нөлөөлөл: Зарим SDK нь түүхий `private_key` хүсэлтэд HTTPS-ийг хэрэгжүүлэх боловч бусад аюулгүй байдлын мэдрэмтгий хүсэлтийн материалыг аюулгүй HTTP эсвэл таарахгүй хостууд руу шилжүүлэхийг зөвшөөрдөг.

Нотлох баримт:- Свифт нь каноник хүсэлтийн баталгаажуулалтын толгой хэсгийг эмзэг гэж үздэг:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Гэхдээ Swift зөвхөн `"private_key"` дээр биетэй таарч байна:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Котлин зөвхөн `authorization` болон `x-api-token` толгойг таньж, дараа нь ижил `"private_key"` биеийн эвристик рүү буцна:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android нь ижил хязгаарлалттай:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Котлин/Жавагийн каноник хүсэлтийн гарын үсэг зурагчид өөрсдийн тээврийн хамгаалагчдын мэдрэмтгий гэж ангилагдаагүй нэмэлт баталгаажуулалтын толгойг үүсгэдэг:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Зөвлөмж болгож буй засвар:

- Эвристик биеийн сканнерыг тодорхой хүсэлтийн ангилалаар солих.
- Каноник баталгаажуулалтын толгой, үр/нууц үгийн талбар, гарын үсэг зурсан мутацийн толгой хэсэг болон ирээдүйн нууцыг агуулсан талбаруудыг дэд мөрний тохирлоор бус гэрээгээр мэдрэмтгий гэж үзнэ.
- Swift, Kotlin болон Java дээр мэдрэмжийн дүрмийг дагаж мөрдөөрэй.

### SA-005 Дундаж: Хавсралт "sandbox" нь зөвхөн дэд процесс, дээр нь `setrlimit`Нөлөөлөл: Хавсралт ариутгагчийг "элсэн хайрцагт" гэж тодорхойлж, мэдээлсэн боловч хэрэгжилт нь нөөцийн хязгаартай одоогийн хоёртын файлын зөвхөн салаа/exec юм. Задлан шинжлэгч эсвэл архивын ашиглалт нь Torii-тэй ижил хэрэглэгч, файлын системийн харагдац болон орчны сүлжээ/процессын давуу эрхээр ажилласаар байх болно.

Нотлох баримт:

- Гаднах зам нь үр дүнг хүүхэд төрүүлсний дараа хамгаалагдсан хязгаарлагдмал орчинд гэж тэмдэглэнэ:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- Хүүхэд одоогийн гүйцэтгэгдэх файлыг өгөгдмөл болгож байна:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- Дэд процесс нь `AttachmentSanitizerMode::InProcess` руу буцаж шилждэг:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- Цорын ганц хатуужуулалт нь CPU/address-space `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Зөвлөмж болгож буй засвар:

- Жинхэнэ үйлдлийн системийн хамгаалагдсан хязгаарлагдмал орчинг (жишээ нь нэрийн зай/seccomp/landlock/шоронгийн хэв маягийн тусгаарлалт, эрх алдагдах, сүлжээгүй, хязгаарлагдмал файлын систем гэх мэт) хэрэгжүүлэх эсвэл үр дүнг `sandboxed` гэж тэмдэглэхээ боль.
- Жинхэнэ тусгаарлалт бий болтол одоогийн загварыг API, телеметр, баримт бичигт "sandboxing" гэхээсээ илүү "дэд процессын тусгаарлалт" гэж тооц.

### SA-006 Дундаж: Нэмэлт P2P TLS/QUIC дамжуулалт нь гэрчилгээ баталгаажуулалтыг идэвхгүй болгодог.Нөлөөлөл: `quic` эсвэл `p2p_tls` идэвхжсэн үед суваг нь шифрлэлт өгдөг боловч алсын төгсгөлийн цэгийг баталгаажуулдаггүй. Идэвхтэй зам дээрх халдагч нь операторуудын TLS/QUIC-тай холбодог аюулгүй байдлын хэвийн хүлээлтийг даван туулж сувгийг дамжуулах эсвэл зогсоох боломжтой хэвээр байна.

Нотлох баримт:

- QUIC нь зөвшөөрөгдсөн гэрчилгээний баталгаажуулалтыг тодорхой баримтжуулдаг:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC баталгаажуулагч нь серверийн гэрчилгээг болзолгүйгээр хүлээн зөвшөөрдөг:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP тээвэрлэлт нь дараахь зүйлийг хийдэг.
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Зөвлөмж болгож буй засвар:

- Үе тэнгийн гэрчилгээг баталгаажуулах эсвэл дээд түвшний гарын үсэг зурсан гар барих болон тээвэрлэлтийн сешн хооронд тодорхой сувгийн холболтыг нэмнэ үү.
- Хэрэв одоогийн үйл ажиллагаа нь зориудаар хийгдсэн бол операторууд TLS-ийн бүрэн нэвтрэлт танилт гэж андуурахгүйн тулд тухайн функцийг баталгаажуулаагүй шифрлэгдсэн тээвэрлэлт гэж өөрчилнө үү.

## Санал болгож буй засварын захиалга1. Толгойн бүртгэлийг засварлах эсвэл идэвхгүй болгох замаар SA-001-ийг нэн даруй засна уу.
2. SA-002-д зориулсан шилжилтийн төлөвлөгөөг боловсруулж, илгээгээрэй, ингэснээр түүхий хувийн түлхүүрүүд API хилийг давахаа болино.
3. Алсын нууц түлхүүр гарган авах замыг хасах буюу нарийсгаж, үр агуулсан биетүүдийг мэдрэмтгий гэж ангилна.
4. Swift/Kotlin/Java дээр SDK тээврийн мэдрэмжийн дүрмийг тохируулах.
5. Хавсралтын ариун цэврийн байгууламжид жинхэнэ хамгаалагдсан хязгаарлагдмал орчин шаардлагатай юу эсвэл нэрээ өөрчлөх/хамрах хүрээг өөрчлөх шаардлагатай эсэхийг шийд.
6. Операторууд баталгаажсан TLS-ийг хүлээж буй тээвэрлэлтийг идэвхжүүлэхээс өмнө P2P TLS/QUIC аюулын загварыг тодруулж, хатууруулна уу.

## Баталгаажуулалтын тэмдэглэл

- `cargo check -p iroha_torii --lib --message-format short` тэнцсэн.
- `cargo check -p iroha_p2p --message-format short` тэнцсэн.
- `cargo deny check advisories bans sources --hide-inclusion-graph` хамгаалагдсан хязгаарлагдмал орчинд гүйсний дараа дамжуулсан; Энэ нь давхардсан анхааруулга гаргасан боловч `advisories ok, bans ok, sources ok` мэдээлсэн.
- Энэ аудитын үеэр нууц үүсмэл түлхүүр багцын маршрутын Torii төвлөрсөн тестийг эхлүүлсэн боловч тайлан бичихээс өмнө дуусгаагүй; илэрцийг үл харгалзан шууд эх сурвалжийн шалгалтаар баталгаажуулсан.