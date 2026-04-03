<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: security_audit_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4343a8f72823a2539a2e79e80a1c7162d455e03b450e2fd613f01ac5c61aec63
source_last_modified: "2026-03-26T13:01:48.980087+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Xavfsizlik auditi hisoboti

Sana: 2026-03-26

## Ijroiya xulosasi

Ushbu audit joriy daraxtdagi eng xavfli sirtlarga e'tibor qaratdi: Torii HTTP/API/auth oqimlari, P2P transporti, maxfiy ishlov berish API'lari, SDK transport himoyachilari va biriktirma dezinfektsiyalash yo'li.

Men hal qilinishi mumkin bo'lgan 6 ta muammoni topdim:

- 2 ta yuqori og'irlikdagi topilmalar
- 4 ta o'rtacha og'irlikdagi topilmalar

Eng muhim muammolar quyidagilardir:

1. Torii hozirda har bir HTTP soʻrovi uchun kiruvchi soʻrov sarlavhalarini qayd qiladi, ular tashuvchi tokenlari, API tokenlari, operator sessiyasi/bootstrap tokenlari va jurnallarga yoʻnaltirilgan mTLS markerlarini koʻrsatishi mumkin.
2. Bir nechta ommaviy Torii marshrutlari va SDKlar hali ham serverga xom `private_key` qiymatlarini yuborishni qo'llab-quvvatlaydi, shuning uchun Torii qo'ng'iroq qiluvchi nomidan imzo qo'yishi mumkin.
3. Bir nechta "maxfiy" yo'llar oddiy so'rov organlari sifatida ko'rib chiqiladi, jumladan, ba'zi SDK'larda maxfiy urug'larni olish va kanonik so'rov autentsiyasi.

## Usul

- Torii, P2P, kripto/VM va SDK maxfiy ishlov berish yo'llarini statik ko'rib chiqish
- Maqsadli tekshirish buyruqlari:
  - `cargo check -p iroha_torii --lib --message-format short` -> o'tish
  - `cargo check -p iroha_p2p --message-format short` -> o'tish
  - `cargo test -p iroha_torii --lib confidential_derive_keyset_endpoint_roundtrip -- --nocapture` -> o'tish
  - `cargo deny check advisories bans sources --hide-inclusion-graph` -> o'tish, faqat takroriy versiya haqida ogohlantirishlar
- Ushbu o'tishda to'ldirilmagan:
  - to'liq ish maydoni qurish/test/klip
  - Swift/Gradle test to'plamlari
  - CUDA/Metal ish vaqtini tekshirish

## Topilmalar

### SA-001 Yuqori: Torii global miqyosda nozik so'rov sarlavhalarini qayd qiladiTa'sir: Kemalarni kuzatishni talab qiladigan har qanday joylashtirish tashuvchi/API/operator tokenlari va tegishli autentifikatsiya materiallarini ilovalar jurnallariga oqib chiqishi mumkin.

Dalil:

- `crates/iroha_torii/src/lib.rs:20752` `TraceLayer::new_for_http()` ni yoqadi
- `crates/iroha_torii/src/lib.rs:20753` `DefaultMakeSpan::default().include_headers(true)` ni yoqadi
- Nozik sarlavha nomlari bir xil xizmatning boshqa joylarida faol ishlatiladi:
  - `crates/iroha_torii/src/operator_auth.rs:40`
  - `crates/iroha_torii/src/operator_auth.rs:41`
  - `crates/iroha_torii/src/operator_auth.rs:42`
  - `crates/iroha_torii/src/operator_auth.rs:43`

Nima uchun bu muhim:

- `include_headers(true)` to'liq kiruvchi sarlavha qiymatlarini kuzatuv oraliqlariga yozib oladi.
- Torii, `Authorization`, `x-api-token`, `x-iroha-operator-session`, `x-iroha-operator-token` va `x-forwarded-client-cert` kabi sarlavhalarda autentifikatsiya materiallarini qabul qiladi.
- Shunday qilib, jurnalning sink kompromisi, disk raskadrovka jurnali to'plami yoki qo'llab-quvvatlash to'plami hisobga olish ma'lumotlarini oshkor qilish hodisasiga aylanishi mumkin.

Tavsiya etilgan tuzatish:

- Ishlab chiqarish oralig'ida to'liq so'rov sarlavhalarini qo'shishni to'xtating.
- Agar nosozliklarni tuzatish uchun sarlavhalar jurnali hali ham zarur bo'lsa, xavfsizlikka sezgir sarlavhalar uchun aniq tahrirni qo'shing.
- Agar ma'lumotlar ijobiy ro'yxatga kiritilmagan bo'lsa, so'rov/javob jurnalini sukut bo'yicha maxfiy deb hisoblang.

### SA-002 Oliy: Ommaviy Torii API'lari server tomonida imzolash uchun hali ham xom shaxsiy kalitlarni qabul qiladi

Ta'sir: Mijozlarga tarmoq orqali xom shaxsiy kalitlarni uzatish tavsiya etiladi, shunda server ularning nomidan tizimga kirishi va API, SDK, proksi-server va server xotirasi qatlamlarida keraksiz maxfiy ta'sir qilish kanalini yaratishi mumkin.

Dalil:- Boshqaruv marshruti hujjatlari server tomonida imzolashni aniq reklama qiladi:
  - `crates/iroha_torii/src/gov.rs:495`
- Marshrutni amalga oshirish taqdim etilgan shaxsiy kalitni tahlil qiladi va server tomonini belgilaydi:
  - `crates/iroha_torii/src/gov.rs:1088`
  - `crates/iroha_torii/src/gov.rs:1091`
  - `crates/iroha_torii/src/gov.rs:1123`
  - `crates/iroha_torii/src/gov.rs:1125`
- SDK'lar `private_key` ni JSON jismlariga faol ravishda seriyalashtiradi:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:47`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:260`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/OfflineToriiClient.kt:261`

Eslatmalar:

- Bu naqsh bir marshrut oilasiga ajratilmagan. Joriy daraxt boshqaruv, oflayn naqd pul, obunalar va boshqa ilovalarga tegishli DTOlarda bir xil qulaylik modelini o'z ichiga oladi.
- HTTPS-faqat transport tekshiruvlari tasodifiy ochiq matnni tashishni kamaytiradi, lekin ular server tomonida maxfiy ishlov berish yoki jurnalga kirish/xotiraga ta'sir qilish xavfini hal qilmaydi.

Tavsiya etilgan tuzatish:

- Xom `private_key` ma'lumotlarini tashuvchi barcha so'rov DTOlarini bekor qiling.
- Mijozlardan mahalliy imzo qo'yishni va imzolar yoki to'liq imzolangan bitimlar/konvertlarni taqdim etishlarini talab qilish.
- Moslik oynasidan keyin `private_key` misollarini OpenAPI/SDK dan olib tashlang.

### SA-003 Medium: Maxfiy kalitni olish Torii ga maxfiy materialni yuboradi va uni qayta aks ettiradi

Ta'siri: Maxfiy kalitlarni chiqarish APIsi urug'lik materialini oddiy so'rov/javob yuk ma'lumotlariga aylantiradi, bu esa proksi-serverlar, vositachi dasturlar, jurnallar, izlar, ishdan chiqish hisobotlari yoki mijozdan noto'g'ri foydalanish orqali urug'larni oshkor qilish imkoniyatini oshiradi.

Dalil:- So'rov to'g'ridan-to'g'ri urug'lik materialini qabul qiladi:
  - `crates/iroha_torii/src/routing.rs:2736`
  - `crates/iroha_torii/src/routing.rs:2738`
  - `crates/iroha_torii/src/routing.rs:2740`
- Javob sxemasi urug'ni ham hex, ham base64da aks ettiradi:
  - `crates/iroha_torii/src/routing.rs:2745`
  - `crates/iroha_torii/src/routing.rs:2746`
  - `crates/iroha_torii/src/routing.rs:2747`
- Ishlovchi urug'ni aniq qayta kodlaydi va qaytaradi:
  - `crates/iroha_torii/src/routing.rs:2797`
  - `crates/iroha_torii/src/routing.rs:2801`
  - `crates/iroha_torii/src/routing.rs:2802`
  - `crates/iroha_torii/src/routing.rs:2804`
- Swift SDK buni oddiy tarmoq usuli sifatida ochib beradi va javob modelida aks-sadoni saqlab qoladi:
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4716`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4717`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4718`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11912`
  - `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:11926`

Tavsiya etilgan tuzatish:

- CLI/SDK kodida mahalliy kalit hosilasini afzal ko'ring va masofaviy derivatsiya marshrutini butunlay olib tashlang.
- Agar marshrut qolishi kerak bo'lsa, javobda hech qachon urug'ni qaytarmang va barcha transport himoyasi va telemetriya/kadozlash yo'llarida urug'li tanalarni sezgir deb belgilang.

### SA-004 Medium: SDK transport sezgirligini aniqlash `private_key` bo'lmagan maxfiy material uchun ko'r nuqtalarga ega

Ta'sir: Ba'zi SDK'lar xom `private_key` so'rovlari uchun HTTPS-ni qo'llaydi, lekin shunga qaramay, boshqa xavfsizlikka sezgir so'rov materiallariga xavfsiz bo'lmagan HTTP orqali yoki mos kelmaydigan xostlarga o'tishga ruxsat beradi.

Dalil:- Swift kanonik so'rovni tasdiqlash sarlavhalarini sezgir deb hisoblaydi:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:4`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:7`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:8`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:9`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:10`
- Ammo Swift hali ham faqat `"private_key"` da tanaga mos keladi:
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:18`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:66`
  - `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift:69`
- Kotlin faqat `authorization` va `x-api-token` sarlavhalarini taniydi, keyin bir xil `"private_key"` tana evristikasiga qaytadi:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:50`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:53`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:58`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt:61`
- Java/Android bir xil cheklovga ega:
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:24`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:100`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java:104`
- Kotlin/Java kanonik so'rov imzolovchilari o'zlarining transport qo'riqchilari tomonidan sezgir deb tasniflanmagan qo'shimcha autentifikatsiya sarlavhalarini yaratadilar:
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:17`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:18`
  - `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/CanonicalRequestSigner.kt:51`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:25`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:26`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:27`
  - `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:28`

Tavsiya etilgan tuzatish:

- Evristik tanani skanerlashni aniq so'rov tasnifi bilan almashtiring.
- Kanonik autentifikatsiya sarlavhalari, urugʻlik/parol maydonlari, imzolangan mutatsiya sarlavhalari va kelajakdagi maxfiy maʼlumotlarga ega boʻlgan maydonlarni pastki qator moslashuvi boʻyicha emas, balki shartnoma boʻyicha sezgir deb hisoblang.
- Swift, Kotlin va Java bo'ylab sezgirlik qoidalarini bir xilda saqlang.

### SA-005 O'rta: "Sandbox" biriktirma faqat kichik jarayon va `setrlimit`Ta'sir: biriktirma dezinfektsiyalash vositasi "qum qutisi" deb ta'riflanadi va xabar qilinadi, lekin joriy qilish resurs chegaralari bo'lgan joriy ikkilik faylning vilkalari/execsidir. Tahlil qiluvchi yoki arxiv ekspluatatsiyasi hali ham Torii kabi bir xil foydalanuvchi, fayl tizimi ko'rinishi va atrof-muhit tarmog'i/jarayon imtiyozlari bilan bajariladi.

Dalil:

- Tashqi yo'l bola tug'ilgandan so'ng, natijani qum qutisi sifatida belgilaydi:
  - `crates/iroha_torii/src/zk_attachments.rs:756`
  - `crates/iroha_torii/src/zk_attachments.rs:760`
  - `crates/iroha_torii/src/zk_attachments.rs:776`
  - `crates/iroha_torii/src/zk_attachments.rs:782`
- Bola joriy bajariladigan faylga sukut saqlaydi:
  - `crates/iroha_torii/src/zk_attachments.rs:913`
  - `crates/iroha_torii/src/zk_attachments.rs:919`
- Pastki jarayon aniq `AttachmentSanitizerMode::InProcess` ga o'tadi:
  - `crates/iroha_torii/src/zk_attachments.rs:1794`
  - `crates/iroha_torii/src/zk_attachments.rs:1803`
- Qo'llaniladigan yagona qattiqlashuv protsessor/manzil maydoni `setrlimit`:
  - `crates/iroha_torii/src/zk_attachments.rs:1845`
  - `crates/iroha_torii/src/zk_attachments.rs:1850`
  - `crates/iroha_torii/src/zk_attachments.rs:1851`
  - `crates/iroha_torii/src/zk_attachments.rs:1872`

Tavsiya etilgan tuzatish:

- Yoki haqiqiy OS sinov muhitini (masalan, nomlar maydoni/seccomp/landlock/jail-uslubi izolyatsiyasi, imtiyozlarni yo'qotish, tarmoqsiz, cheklangan fayl tizimi) amalga oshiring yoki natijani `sandboxed` deb belgilashni to'xtating.
- Haqiqiy izolyatsiya mavjud bo'lgunga qadar joriy dizaynni API, telemetriya va hujjatlarda "sandboxing" emas, balki "subprocess izolyatsiyasi" sifatida ko'ring.

### SA-006 O'rta: ixtiyoriy P2P TLS/QUIC transportlari sertifikatni tekshirishni o'chirib qo'yadiTa'sir: `quic` yoki `p2p_tls` yoqilganda, kanal shifrlashni ta'minlaydi, lekin masofaviy so'nggi nuqtani autentifikatsiya qilmaydi. Faol yo'lda tajovuzkor operatorlar TLS/QUIC bilan bog'langan oddiy xavfsizlik talablarini yengib, kanalni uzatishi yoki tugatishi mumkin.

Dalil:

- QUIC ruxsat beruvchi sertifikatni tekshirishni aniq hujjatlashtiradi:
  - `crates/iroha_p2p/src/transport.rs:12`
  - `crates/iroha_p2p/src/transport.rs:13`
  - `crates/iroha_p2p/src/transport.rs:14`
  - `crates/iroha_p2p/src/transport.rs:15`
- QUIC tekshiruvi server sertifikatini so'zsiz qabul qiladi:
  - `crates/iroha_p2p/src/transport.rs:33`
  - `crates/iroha_p2p/src/transport.rs:35`
  - `crates/iroha_p2p/src/transport.rs:44`
  - `crates/iroha_p2p/src/transport.rs:112`
  - `crates/iroha_p2p/src/transport.rs:114`
  - `crates/iroha_p2p/src/transport.rs:115`
- TLS-over-TCP transporti xuddi shunday qiladi:
  - `crates/iroha_p2p/src/transport.rs:229`
  - `crates/iroha_p2p/src/transport.rs:232`
  - `crates/iroha_p2p/src/transport.rs:241`
  - `crates/iroha_p2p/src/transport.rs:279`
  - `crates/iroha_p2p/src/transport.rs:281`
  - `crates/iroha_p2p/src/transport.rs:282`

Tavsiya etilgan tuzatish:

- Yoki tengdosh sertifikatlarini tasdiqlang yoki yuqori darajadagi imzolangan qoʻl siqish va transport seansi oʻrtasida aniq kanal bogʻlanishini qoʻshing.
- Agar joriy xatti-harakatlar qasddan amalga oshirilgan bo'lsa, operatorlar uni to'liq TLS autentifikatsiyasi deb xato qilmasliklari uchun xususiyatni autentifikatsiya qilinmagan shifrlangan transport sifatida qayta nomlang/hujjatlang.

## Tavsiya etilgan tuzatish buyrug'i1. Sarlavhalar jurnalini o'zgartirish yoki o'chirish orqali SA-001 ni darhol tuzating.
2. SA-002 uchun migratsiya rejasini ishlab chiqing va yuboring, shunda xom shaxsiy kalitlar API chegarasini kesib o'tishni to'xtatadi.
3. Masofaviy maxfiy kalitlarni olish yo'lini olib tashlang yoki toraytiring va urug'li jismlarni sezgir deb tasniflang.
4. Swift/Kotlin/Java bo'ylab SDK transport sezgirligi qoidalarini tekislang.
5. Qo'shimcha sanitariya uchun haqiqiy qum qutisi yoki halol nomini o'zgartirish/ko'lamini o'zgartirish kerakligini hal qiling.
6. Operatorlar autentifikatsiya qilingan TLSni kutayotgan transportlarni yoqishdan oldin P2P TLS/QUIC tahdid modelini aniqlang va mustahkamlang.

## Tasdiqlash eslatmalari

- `cargo check -p iroha_torii --lib --message-format short` o'tdi.
- `cargo check -p iroha_p2p --message-format short` o'tdi.
- `cargo deny check advisories bans sources --hide-inclusion-graph` qum qutisidan tashqarida yugurgandan keyin o'tdi; u ikki nusxadagi ogohlantirishlarni chiqardi, lekin `advisories ok, bans ok, sources ok` haqida xabar berdi.
- Ushbu audit davomida konfidensial olingan kalitlar to'plamiga qaratilgan Torii testi boshlandi, lekin hisobot yozilgunga qadar tugallanmagan; topilma to'g'ridan-to'g'ri manba tekshiruvi tomonidan qo'llab-quvvatlanadi.