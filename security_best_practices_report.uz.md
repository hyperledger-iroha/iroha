<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: security_best_practices_report.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6055090fbfab93113392610b3eb78ef0a0e97d6fef4a94292c5609ec5562e925
source_last_modified: "2026-03-26T06:12:11.487656+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Xavfsizlik bo'yicha eng yaxshi amaliyotlar hisoboti

Sana: 2026-03-25

## Ijroiya xulosasi

Joriy ish maydoniga nisbatan oldingi Torii/Soracloud hisobotini yangiladim
kodni kiritdi va ko'rib chiqishni eng yuqori xavfli server, SDK va
kripto/seriyalash sirtlari. Ushbu audit dastlab uchtasini tasdiqladi
kirish/autentifikatsiya muammolari: ikkita yuqori jiddiylik va bitta o'rtacha jiddiylik.
Ushbu uchta topilma hozirgi daraxtda tuzatish orqali yopildi
quyida tasvirlangan. Tashish va ichki jo'natish tekshiruvi tasdiqlandi
to'qqizta qo'shimcha o'rtacha jiddiylikdagi muammolar: bitta chiquvchi P2P identifikatori
bo'shliq, bitta chiquvchi P2P TLS standartini pasaytiruvchi standart, ikkita Torii ishonch chegarasi xatosi
webhook yetkazib berish va MCP ichki jo'natish, bitta o'zaro SDK sezgir-transport
Swift, Java/Android, Kotlin va JS mijozlaridagi bo'shliq, bitta SoraFS
ishonchli-proksi/mijoz-IP siyosati bo'shlig'i, bitta SoraFS mahalliy proksi
bog'lash/autentifikatsiya bo'shlig'i, bir xil telemetriyali geo-qidiruv ochiq matnli zaxira,
va bitta operator-auth masofaviy IP-noto'g'ri ochilgan qulflash/stavkani kalitlash bo'shlig'i. Bular
keyingi topilmalar ham joriy daraxtda yopiq.To'rtta avval Soracloudning xom shaxsiy kalitlarga oid topilmalari haqida xabar bergan edik
HTTP, ichki proksi-faqat mahalliy o'qish, o'lchovsiz umumiy ish vaqti
qayta tiklash va masofaviy IP biriktirma ijarasi endi joriy kodda mavjud emas.
Ular quyida yangilangan kod havolalari bilan yopiq/o'rniga qo'yilgan deb belgilangan.Bu to'liq qizil jamoa mashqlari emas, balki kodga asoslangan audit bo'lib qoldi.
Men tashqi kirish mumkin bo'lgan Torii kirish va so'rov-auth yo'llarini birinchi o'ringa qo'ydim, keyin
joyida tekshirilgan IVM, `iroha_crypto`, `norito`, Swift/Android/JS SDK
soʻrov imzolash yordamchilari, tengdosh telemetriya geo yoʻli va SoraFS
ish stantsiyasi proksi yordamchilari va SoraFS pin/shlyuz mijoz-IP siyosati
yuzalar. Ushbu kirish/auth, chiqish siyosati bo'yicha jonli tasdiqlangan muammo yo'q,
peer-telemetry geo, namunali P2P transport standartlari, MCP-dispatch, namunali SDK
transport, operator-auth lokavt/stavka kaliti, SoraFS ishonchli-proksi/mijoz-IP
siyosat yoki mahalliy proksi boʻlimi ushbu hisobotdagi tuzatishlardan keyin qoladi.
Kuzatuv qattiqlashuvi, shuningdek, muvaffaqiyatsiz yopilgan ishga tushirish haqiqati to'plamini kengaytirdi
namunali IVM CUDA/Metal tezlatgich yo'llari; bu ish yangilikni tasdiqlamadi
muvaffaqiyatsiz ochiq muammo. Namuna olingan metall Ed25519
Bir nechta ref10 driftini tuzatgandan so'ng, endi bu xostda imzo yo'li tiklandi
Metall/CUDA portlaridagi nuqtalar: tekshirishda ijobiy tayanch nuqtasi bilan ishlash,
`d2` doimiysi, aniq `fe_sq2` kamaytirish yo'li, adashgan yakuniy
`fe_mul` ko'chirish qadami va oyoq-qo'llarni harakatga keltiradigan operatsiyadan keyingi maydonni normallashtirish etishmayotgan
chegaralar skalyar narvon bo'ylab siljiydi. Fokuslangan Metall regressiya qamrovi hozir
imzo quvurini yoqilgan holda ushlab turadi va `[true, false]` ni tasdiqlaydiprotsessor mos yozuvlar yo'liga qarshi tezlatgich. Namuna olingan boshlang'ich haqiqat hozirda o'rnatiladi
shuningdek, jonli vektorni bevosita tekshirib ko'ring (`vadd64`, `vand`, `vxor`, `vor`) va
Ushbu backendlardan oldin Metall va CUDA-da bir martalik AES ommaviy yadrolari
faol qoling. Keyinchalik qaramlikni skanerlash ettita jonli uchinchi tomon topilmalarini qo'shdi
orqada qoldi, lekin joriy daraxt o'shandan beri ikkala faol `tar`ni olib tashladi
`xtask` Rust `tar` qaramligidan voz kechish va o'rniga
`iroha_crypto` `libsodium-sys-stable` OpenSSL tomonidan qo'llab-quvvatlanadigan interop testlari
ekvivalentlari. Joriy daraxt ham to'g'ridan-to'g'ri PQ bog'liqliklarini almashtirdi
`soranet_pq`, `iroha_crypto` va `ivm` va `ivm` koʻchirilgan.
`pqcrypto-dilithium` / `pqcrypto-kyber` gacha
`pqcrypto-mldsa` / `pqcrypto-mlkem` mavjud ML-DSA ni saqlagan holda /
ML-KEM API yuzasi. Keyinchalik o'sha kunlik bog'liqlik o'tkazmasi ish maydonini mahkamladi
`reqwest` / `rustls` versiyalari yamoqli yamoq relizlar uchun
Joriy hal qilishda sobit `0.103.10` liniyasida `rustls-webpki`. yagona
Qolgan qaramlik siyosati istisnolari ikkita o'tish davri bo'lib qolmaydi
makro qutilar (`derivative`, `paste`), ular hozirda aniq qabul qilinadi
`deny.toml`, chunki xavfsiz yangilanish yo'q va ularni olib tashlash talab qilinadi
bir nechta yuqori oqimlarni almashtirish yoki sotish. Theqolgan tezlatgich ishi
aks ettirilgan CUDA tuzatishining ish vaqtini tekshirish va kengaytirilgan CUDA haqiqati yoqilgan
jonli CUDA drayverini qo'llab-quvvatlaydigan xost, to'g'riligi yoki muvaffaqiyatsizligi tasdiqlangan emas
joriy daraxtdagi muammo.

## Yuqori jiddiylik

### SEC-05: Ilovaning kanonik soʻrovini tekshirish multisig chegaralarini chetlab oʻtdi (2026-03-24 yopilgan)

Ta'sir:

- Multisig tomonidan boshqariladigan hisobning har qanday bitta a'zo kaliti avtorizatsiya qilishi mumkin
  chegara yoki vazn talab qilishi kerak bo'lgan ilovalarga tegishli so'rovlar
  kvorum.
- Bu `verify_canonical_request` ga ishonadigan har bir so'nggi nuqtaga, shu jumladan
  Soracloud imzolangan mutatsiyaga kirish, kontentga kirish va imzolangan hisob qaydnomasi ZK
  qo'shimcha ijara.

Dalil:

- `verify_canonical_request` multisig kontrollerni to'liq a'zoga kengaytiradi
  ochiq kalitlar ro'yxati va so'rovni tasdiqlovchi birinchi kalitni qabul qiladi
  chegara yoki to'plangan vaznni baholamasdan imzo:
  `crates/iroha_torii/src/app_auth.rs:198-210`.
- Haqiqiy multisig siyosat modeli ham `threshold`, ham og'irligi bor.
  aʼzolarni qabul qiladi va chegarasi umumiy ogʻirlikdan oshgan siyosatlarni rad etadi:
  `crates/iroha_data_model/src/account/controller.rs:92-95`,
  `crates/iroha_data_model/src/account/controller.rs:163-178`,
  `crates/iroha_data_model/src/account/controller.rs:188-196`.
- Yordamchi Soracloud mutatsiyasiga kirish uchun ruxsat yo'lida
  `crates/iroha_torii/src/lib.rs:2141-2157`, kontentga kirish hisobiga kirish
  `crates/iroha_torii/src/content.rs:359-360` va ilova ijarasi
  `crates/iroha_torii/src/lib.rs:7962-7968`.

Nima uchun bu muhim:- So'rovni imzolagan shaxs HTTP qabul qilish uchun hisob vakolati sifatida qabul qilinadi,
  ammo amalga oshirish multisig hisoblarini "har qanday singl" ga jimgina pasaytiradi
  a'zosi yolg'iz harakat qilishi mumkin."
- Bu chuqur himoyalangan HTTP imzo qatlamini avtorizatsiyaga aylantiradi
  multisig bilan himoyalangan hisoblar uchun chetlab o'tish.

Tavsiya:

- Yoki agacha app-auth sathida multisig tomonidan boshqariladigan hisoblarni rad eting
  tegishli guvoh formati mavjud yoki protokolni HTTP so'rovi uchun kengaytiring
  chegarani qondiradigan to'liq multisig guvohlar to'plamini olib boradi va tasdiqlaydi
  vazn.
- Soracloud mutatsion vositachi dasturi, kontentni autentifikatsiya qilish va ZKni qamrab oluvchi regressiyalarni qo'shing
  chegaradan past multisig imzolari uchun qo'shimchalar.

Tuzatish holati:

- Joriy kodda yopilgan multisig tomonidan boshqariladigan hisoblarda muvaffaqiyatsiz yopilgan
  `crates/iroha_torii/src/app_auth.rs`.
- Tekshiruvchi endi "har qanday a'zo imzolashi mumkin" semantikasini qabul qilmaydi
  multisig HTTP avtorizatsiyasi; multisig so'rovlari agacha rad etiladi
  chegarani qondiradigan guvohlik formati mavjud.
- Regressiya qamrovi endi multisigni rad etish uchun maxsus ishni o'z ichiga oladi
  `crates/iroha_torii/src/app_auth.rs`.

## Yuqori jiddiylik

### SEC-06: Ilovaning kanonik soʻrov imzolari cheksiz muddatga takrorlanishi mumkin edi (2026-03-24 yopilgan)

Ta'sir:- Imzolangan xabarda yo'qligi sababli yozib olingan haqiqiy so'rovni takrorlash mumkin
  vaqt tamg'asi, vaqt belgisi, muddati tugashi yoki keshni takrorlash.
- Bu holatni o'zgartiruvchi Soracloud mutatsion so'rovlarini takrorlashi va qayta chiqarishi mumkin
  hisob bilan bog'langan kontent/ilova operatsiyalari asl mijozdan ancha keyin
  ularni niyat qilgan.

Dalil:

- Torii ilovaning kanonik so'rovini faqat deb belgilaydi
  `METHOD + path + sorted query + body hash` dyuym
  `crates/iroha_torii/src/app_auth.rs:1-17` va
  `crates/iroha_torii/src/app_auth.rs:74-89`.
- Tekshiruvchi faqat `X-Iroha-Account` va `X-Iroha-Signature` ni qabul qiladi va shunday qiladi
  yangilikni talab qilmang yoki takroriy keshni saqlamang:
  `crates/iroha_torii/src/app_auth.rs:137-218`.
- JS, Swift va Android SDK yordamchilari bir xil takrorlashga moyil sarlavhani yaratadi
  hech qanday/vaqt tamg'asi maydonlarisiz juftlash:
  `javascript/iroha_js/src/canonicalRequest.js:50-82`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift:41-68`, va
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java:67-106`.
- Torii operator-imzo yo'li allaqachon kuchliroq naqshdan foydalanadi.
  Ilovaga qarashli yoʻl yoʻq: vaqt tamgʻasi, vaqt belgisi va keshni takrorlash
  `crates/iroha_torii/src/operator_signatures.rs:1-21` va
  `crates/iroha_torii/src/operator_signatures.rs:266-294`.

Nima uchun bu muhim:

- HTTPSning o'zi teskari proksi-server, disk raskadrovka jurnali tomonidan takrorlashni oldini olmaydi,
  buzilgan mijoz xosti yoki haqiqiy so'rovlarni yozib oladigan har qanday vositachi.
- Xuddi shu sxema barcha asosiy mijoz SDK-larida amalga oshirilganligi sababli, takrorlash
  zaiflik faqat server uchun emas, balki tizimli.

Tavsiya:- Ilovani tasdiqlash so'rovlariga imzolangan yangilik materialini kamida vaqt belgisi bilan qo'shing
  va nonce va cheklangan takroriy kesh bilan eskirgan yoki qayta foydalanilgan kortejlarni rad eting.
- Ilovaning kanonik so'rov formatini Torii va SDK'lar uchun aniq versiyasi.
  eski ikki sarlavhali sxemani xavfsiz tarzda bekor qiling.
- Soracloud mutatsiyalari, kontenti uchun takrorlashni rad etishni isbotlovchi regressiyalarni qo'shing
  kirish va CRUD biriktirilishi.

Tuzatish holati:

- Joriy kodda yopilgan. Torii endi to'rt sarlavhali sxemani talab qiladi
  (`X-Iroha-Account`, `X-Iroha-Signature`, `X-Iroha-Timestamp-Ms`,
  `X-Iroha-Nonce`) va imzolaydi/tasdiqlaydi
  `METHOD + path + sorted query + body hash + timestamp + nonce` dyuym
  `crates/iroha_torii/src/app_auth.rs`.
- Yangilikni tekshirish endi chegaralangan soat-qiyshayish oynasini qo'llaydi va tasdiqlaydi
  nonce shaklga ega bo'ladi va xotirada qayta o'ynash keshi bilan qayta ishlatilgan noncesni rad etadi
  tugmalar `crates/iroha_config/src/parameters/{defaults,actual,user}.rs` orqali yuzaga chiqadi.
- JS, Swift va Android yordamchilari endi bir xil to'rt sarlavhali formatni chiqaradi
  `javascript/iroha_js/src/canonicalRequest.js`,
  `IrohaSwift/Sources/IrohaSwift/CanonicalRequest.swift`, va
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/CanonicalRequestSigner.java`.
- Regressiya qamrovi endi ijobiy imzoni tekshirishni va takrorlashni o'z ichiga oladi,
  eskirgan vaqt tamg'asi va yangilikni rad etish holatlari
  `crates/iroha_torii/src/app_auth.rs`.

## Oʻrtacha ogʻirlik

### SEC-07: mTLS qo'llanilishi soxta yo'naltirilgan sarlavhaga ishondi (2026-03-24 yopilgan)

Ta'sir:- Agar Torii to'g'ridan-to'g'ri bo'lsa, `require_mtls` ga asoslangan o'rnatishlarni chetlab o'tish mumkin
  kirish mumkin yoki oldingi proksi-server mijoz tomonidan taqdim etilganini ajratmaydi
  `x-forwarded-client-cert`.
- Muammo konfiguratsiyaga bog'liq, lekin ishga tushirilganda u da'vo qilinganga aylanadi
  oddiy sarlavha tekshiruviga mijoz-sertifikat talabi.

Dalil:

- Norito-RPC darvozasi qo'ng'iroq qilish orqali `require_mtls`-ni qo'llaydi
  `norito_rpc_mtls_present`, bu faqat tekshiradi
  `x-forwarded-client-cert` mavjud va bo'sh emas:
  `crates/iroha_torii/src/lib.rs:1897-1926`.
- Operator-auth bootstrap/login oqimlari `check_common` chaqiruvi, bu faqat rad etadi
  `mtls_present(headers)` noto'g'ri bo'lsa:
  `crates/iroha_torii/src/operator_auth.rs:562-570`.
- `mtls_present`, shuningdek, faqat bo'sh bo'lmagan `x-forwarded-client-cert` tekshiruvidir
  `crates/iroha_torii/src/operator_auth.rs:1212-1216`.
- O'sha operator-auth ishlovchilar hali ham marshrut sifatida ko'rinadi
  `crates/iroha_torii/src/lib.rs:16658-16672`.

Nima uchun bu muhim:

- Yo'naltirilgan sarlavha konventsiyasi faqat Torii orqasida o'tirganda ishonchli bo'ladi.
  sarlavhani ajratadigan va qayta yozadigan qattiqlashtirilgan proksi-server. Kod tasdiqlanmaydi
  bu joylashtirish taxminining o'zi.
- Teskari proksi gigienasiga jimgina bog'liq bo'lgan xavfsizlikni boshqarish oson
  sahnalashtirish, kanareyka yoki voqea-javob marshrutini o'zgartirish paytida noto'g'ri sozlang.

Tavsiya:- Iloji bo'lsa, to'g'ridan-to'g'ri transport-davlat ijrosini afzal ko'ring. Agar proksi bo'lishi kerak bo'lsa
  foydalanilgan bo'lsa, autentifikatsiya qilingan proksi-Torii kanaliga ishoning va ruxsatlar ro'yxatini talab qiling
  yoki xom sarlavha mavjudligi o'rniga o'sha proksi-serverdan imzolangan attestatsiya.
- `require_mtls` bevosita Torii tinglovchilari uchun xavfli ekanligi haqidagi hujjat.
- Norito-RPC da soxta `x-forwarded-client-cert` kiritish uchun salbiy testlarni qo'shing
  va operator-auth bootstrap marshrutlari.

Tuzatish holati:

- Sozlangan proksi-serverga yo'naltirilgan sarlavha ishonchini ulash orqali joriy kodda yopildi
  Faqat xom sarlavha mavjudligi o'rniga CIDR'lar.
- `crates/iroha_torii/src/limits.rs` endi birgalikda taqdim etadi
  `has_trusted_forwarded_header(...)` darvozasi va ikkalasi ham Norito-RPC
  (`crates/iroha_torii/src/lib.rs`) va operator-auth
  (`crates/iroha_torii/src/operator_auth.rs`) uni qo'ng'iroq qiluvchi TCP peer bilan ishlating
  manzil.
- `iroha_config` endi ikkalasi uchun ham `mtls_trusted_proxy_cidrs`ni ochib beradi
  operator-auth va Norito-RPC; sukut bo'yicha - faqat orqaga qaytish.
- Regressiya qamrovi endi soxta `x-forwarded-client-cert` kiritishni rad etadi.
  operator-auth va umumiy chegaralar yordamchisida ishonchsiz masofadan boshqarish pulti.

## Oʻrtacha ogʻirlik

### SEC-08: Chiquvchi P2P terishlari autentifikatsiya qilingan kalitni mo'ljallangan peer identifikatoriga bog'lamadi (2026-03-25 yopilgan)

Ta'sir:- `X` peer-ga chiquvchi terish kaliti boshqa `Y` tengdoshi kabi yakunlanishi mumkin.
  muvaffaqiyatli ilova-qatlam qo'l siqish imzolandi, chunki qo'l siqish
  "ushbu ulanishdagi kalit" autentifikatsiya qilingan, lekin kalit mavjudligini hech qachon tekshirmagan
  tarmoq aktyori erishmoqchi bo'lgan tengdosh identifikatori.
- Ruxsat berilgan qatlamlarda keyingi topologiya/ruxsat berilgan ro'yxat tekshiruvlari hali ham o'chiriladi
  noto'g'ri kalit, shuning uchun bu birinchi navbatda almashtirish/etishish xatosi edi
  to'g'ridan-to'g'ri konsensus taqlid xatosidan ko'ra. Ommaviy qatlamlarda u ruxsat berishi mumkin edi
  buzilgan manzil, DNS javobi yoki o'rni so'nggi nuqtasi boshqasini almashtiradi
  chiquvchi telefonda kuzatuvchining identifikatori.

Dalil:

- Chiqib ketgan tengdosh davlat mo'ljallangan `peer_id` ni saqlaydi
  `crates/iroha_p2p/src/peer.rs:5153-5179`, lekin eski qo'l siqish oqimi
  imzoni tekshirishdan oldin bu qiymatni tushirdi.
- `GetKey::read_their_public_key` imzolangan qo'l siqish yukini tasdiqladi va
  keyin darhol e'lon qilingan masofaviy ochiq kalitdan `Peer` ni yaratdi.
  `crates/iroha_p2p/src/peer.rs:6266-6355` da, hech qanday taqqoslashsiz
  `peer_id` dastlab `connecting(...)` ga yetkazib berilgan.
- Xuddi shu transport to'plami TLS/QUIC sertifikatini aniq o'chirib qo'yadi
  `crates/iroha_p2p/src/transport.rs` da P2P uchun tekshirish, shuning uchun majburiy
  mo'ljallangan peer identifikatori uchun dastur sathining autentifikatsiya qilingan kaliti muhim hisoblanadi
  chiquvchi ulanishlarda identifikatsiyani tekshirish.

Nima uchun bu muhim:- Dizayn ataylab tengdoshlarning autentifikatsiyasini transportdan yuqoriga suradi
  qatlam, bu qo'l siqish kaliti yagona mustahkam identifikatsiya bog'lovchi tekshirish qiladi
  chiquvchi terishlarda.
- Ushbu tekshiruvsiz tarmoq qatlami jimgina "muvaffaqiyatli" davolashi mumkin
  autentifikatsiya qilingan ba'zi tengdoshlar" so'zi "biz terilgan tengdoshga yetib keldi"
  Bu zaifroq kafolatdir va topologiya/obro' holatini buzishi mumkin.

Tavsiya:

- Belgilangan `peer_id`ni imzolangan qoʻl siqish bosqichlari orqali olib boring va
  Agar tekshirilgan masofaviy kalit unga mos kelmasa, yopilmaydi.
- To'g'ri imzolangan qo'l siqishni isbotlovchi qaratilgan regressiyani saqlang
  oddiy imzolangan qoʻl siqish hali ham muvaffaqiyatli boʻlganda notoʻgʻri kalit rad etiladi.

Tuzatish holati:

- Joriy kodda yopilgan. `ConnectedTo` va quyi oqimdagi qoʻl siqish hozir
  kutilgan chiquvchi `PeerId` ni olib, va
  `GetKey::read_their_public_key` mos kelmaydigan autentifikatsiya qilingan kalitni rad etadi
  `HandshakePeerMismatch`, `crates/iroha_p2p/src/peer.rs`.
- Yo'naltirilgan regressiya qamrovi endi o'z ichiga oladi
  `outgoing_handshake_rejects_unexpected_peer_identity` va mavjud
  ijobiy `handshake_v1_defaults_to_trust_gossip` yo'li
  `crates/iroha_p2p/src/peer.rs`.

### SEC-09: HTTPS/WSS veb-huk yetkazib berish ulanish vaqtida tekshirilgan xost nomlarini qayta hal qildi (2026-03-25 yopilgan)

Ta'sir:- Xavfsiz vebhuk yetkazib berish tasdiqlangan maqsadli DNS javoblari vebhukga qarshi
  chiqish siyosati, lekin keyin tekshirilgan manzillarni tashlab, mijozga ruxsat bering
  stek haqiqiy HTTPS yoki WSS ulanishi vaqtida xost nomini qayta hal qiladi.
- Tasdiqlash va ulanish vaqti o'rtasida DNS-ga ta'sir qilishi mumkin bo'lgan tajovuzkor
  avval ruxsat berilgan xost nomini bloklangan shaxsiy yoki
  faqat operator uchun moʻljallangan manzilni belgilang va CIDR-ga asoslangan veb-huk himoyasini chetlab oʻting.

Dalil:

- Chiqish qo'riqchisi nomzodning manzillarini hal qiladi va filtrlaydi
  `crates/iroha_torii/src/webhook.rs:1746-1829` va xavfsiz yetkazib berish yo'llari
  o'sha tekshirilgan manzillar ro'yxatini HTTPS/WSS yordamchilariga o'tkazing.
- Eski HTTPS yordamchisi asl URL manziliga nisbatan umumiy mijozni yaratdi
  host `crates/iroha_torii/src/webhook.rs` va ulanishni bog'lamadi
  tekshirilgan manzillar to'plamiga, ya'ni DNS ruxsati yana ichida sodir bo'ldi
  HTTP mijozi.
- Qadimgi WSS yordamchisi ham `tokio_tungstenite::connect_async(url)` deb nomlangan
  o'rniga xostni qayta hal qilgan asl xost nomiga qarshi
  allaqachon tasdiqlangan manzilni qayta ishlatish.

Nima uchun bu muhim:

- Belgilangan manzil bitta bo'lsa, belgilangan manzilga ruxsat beruvchi ro'yxatlar ishlaydi
  mijoz aslida ulanadi.
- Siyosat tasdiqlangandan keyin qayta hal qilish a da DNS qayta ulash / TOCTOU bo'shlig'ini yaratadi
  operatorlar SSRF uslubidagi saqlash uchun ishonishi mumkin bo'lgan yo'l.

Tavsiya:- Tekshirilgan DNS javoblarini saqlab qolgan holda haqiqiy HTTPS ulanish yoʻliga mahkamlang
  SNI / sertifikatni tekshirish uchun asl xost nomi.
- WSS uchun TCP rozetkasini to'g'ridan-to'g'ri tekshirilgan manzilga ulang va TLSni ishga tushiring
  host nomiga asoslangan qo'ng'iroq qilish o'rniga ushbu oqim orqali websocket qo'l siqish
  qulaylik ulagichi.

Tuzatish holati:

- Joriy kodda yopilgan. `crates/iroha_torii/src/webhook.rs` endi olinadi
  `https_delivery_dns_override(...)` va
  Tekshirilgan manzillar to'plamidan `websocket_pinned_connect_addr(...)`.
- HTTPS yetkazib berish endi `reqwest::Client::builder().resolve_to_addrs(...)` dan foydalanadi
  Shunday qilib, TCP ulanishi mavjud bo'lganda, asl xost nomi TLS uchun ko'rinadigan bo'lib qoladi
  allaqachon tasdiqlangan manzillarga mahkamlangan.
- WSS yetkazib berish endi tekshirilgan manzilga xom `TcpStream` ni ochadi va amalga oshiradi
  Ushbu oqim orqali `tokio_tungstenite::client_async_tls_with_config(...)`,
  bu siyosatni tekshirishdan keyin ikkinchi DNS qidiruvidan qochadi.
- Endi regressiya qamroviga kiradi
  `https_delivery_dns_override_pins_vetted_domain_addresses`,
  `https_delivery_dns_override_skips_ip_literals`, va
  `websocket_pinned_connect_addr_pins_secure_delivery_when_guarded` dyuym
  `crates/iroha_torii/src/webhook.rs`.

### SEC-10: MCP ichki marshrutni jo'natish muhri qo'yilgan orqaga qaytish va meros qilib olingan ruxsatnomalar ro'yxati (2026-03-25 yopilgan)

Ta'sir:- Torii MCP yoqilganda, ichki asboblarni yuborish har bir so'rovni shunday yozdi
  haqiqiy qo'ng'iroq qiluvchidan qat'iy nazar orqaga qaytish. Qo'ng'iroq qiluvchi CIDRga ishonadigan marshrutlar
  imtiyoz yoki cheklovchi bypass shuning uchun MCP trafigini ko'rishi mumkin
  `127.0.0.1`.
- Muammo konfiguratsiya bilan bog'liq edi, chunki MCP sukut bo'yicha o'chirilgan va
  ta'sirlangan marshrutlar hali ham ruxsat etilgan ro'yxatga yoki shunga o'xshash qayta ishlash ishonchiga bog'liq
  siyosat, lekin u MCP ni imtiyozlarni oshirish ko'prigiga aylantirdi
  tugmalar birgalikda ishga tushirildi.

Dalil:

- `dispatch_route(...)` ichida `crates/iroha_torii/src/mcp.rs` ilgari kiritilgan
  uchun `x-iroha-remote-addr: 127.0.0.1` va sintetik orqa aylanish `ConnectInfo`
  har bir ichki yuborilgan so'rov.
- `iroha.parameters.get` MCP yuzasida faqat o'qish rejimida ochiladi va
  `/v1/parameters` qo'ng'iroq qiluvchining IP manziliga tegishli bo'lsa, oddiy autentifikatsiyani chetlab o'tadi.
  `crates/iroha_torii/src/lib.rs:5879-5888` da sozlangan ruxsatnomalar ro'yxati.
- `apply_extra_headers(...)` ham ixtiyoriy `headers` yozuvlarini qabul qildi
  MCP qo'ng'iroq qiluvchisi, shunday qilib ajratilgan ichki ishonch sarlavhalari kabi
  `x-iroha-remote-addr` va `x-forwarded-client-cert` aniq emas edi
  himoyalangan.

Nima uchun bu muhim:- Ichki ko'prik qatlamlari asl ishonch chegarasini saqlab qolishi kerak. O'zgartirish
  orqaga qaytish bilan haqiqiy qo'ng'iroq qiluvchi har bir MCP qo'ng'iroq qiluvchiga samarali munosabatda bo'ladi
  ichki mijoz so'rov ko'prikdan o'tgandan keyin.
- Xato nozik, chunki tashqi ko'rinadigan MCP profili hali ham ko'rinishi mumkin
  faqat o'qish uchun, ichki HTTP marshruti esa imtiyozli manbani ko'radi.

Tavsiya:

- Tashqi `/v1/mcp` so'rovi allaqachon qabul qilingan qo'ng'iroq qiluvchi IP-ni saqlang
  Torii masofaviy manzilli oraliq dasturidan va `ConnectInfo` dan sintez qiling
  loopback o'rniga bu qiymat.
- `x-iroha-remote-addr` va kabi faqat kirish uchun ishonchli sarlavhalarni ishlating
  `x-forwarded-client-cert` zaxiralangan ichki sarlavhalar sifatida MCP qo'ng'iroq qiluvchilar qila olmaydi
  `headers` argumenti orqali ularni kontrabanda qilish yoki bekor qilish.

Tuzatish holati:

- Joriy kodda yopilgan. `crates/iroha_torii/src/mcp.rs` endi kelib chiqadi
  ichki yuborilgan masofaviy IP tashqi so'rovdan kiritilgan
  `x-iroha-remote-addr` sarlavhasi va `ConnectInfo` ni shu realdan sintez qiladi
  orqaga qaytish o'rniga qo'ng'iroq qiluvchi IP.
- `apply_extra_headers(...)` endi ikkala `x-iroha-remote-addr` va
  `x-forwarded-client-cert` zaxiralangan ichki sarlavhalar sifatida, shuning uchun MCP qo'ng'iroq qiluvchilar
  asbob argumentlari orqali orqaga qaytish/kirish-proksi ishonchini soxtalashtira olmaydi.
- Endi regressiya qamroviga kiradi
  `dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks`
  `dispatch_route_blocks_remote_addr_spoofing_from_extra_headers`, va
  `apply_extra_headers_blocks_reserved_internal_headers` dyuym
  `crates/iroha_torii/src/mcp.rs`.### SEC-11: SDK mijozlari xavfli yoki oʻzaro xostlar oʻrtasida tashishlar boʻyicha nozik soʻrov materiallariga ruxsat berdi (2026-03-25 yopilgan)

Ta'sir:

- Namuna olingan Swift, Java/Android, Kotlin va JS mijozlari bunday qilmadi
  doimiy ravishda barcha nozik so'rov shakllarini transportga sezgir sifatida ko'rib chiqing.
  Yordamchiga qarab, qo'ng'iroq qiluvchilar tashuvchi/API-token sarlavhalarini jo'natishlari mumkin
  `private_key*` JSON maydonlari yoki ilovaning kanonik-auth imzolash materiallari
  oddiy `http` / `ws` yoki o'zaro xost mutlaq URL bekor qilish orqali.
- JS mijozida, xususan, `canonicalAuth` sarlavhalari keyin qo'shilgan
  `_request(...)` transport tekshiruvlarini tugatdi va faqat tanaga tegishli `private_key`
  JSON umuman sezgir transport sifatida hisoblanmadi.

Dalil:- Endi Svift qo'riqchini markazlashtiradi
  `IrohaSwift/Sources/IrohaSwift/TransportSecurity.swift` va undan qo'llaydi
  `IrohaSwift/Sources/IrohaSwift/NoritoRpcClient.swift`,
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift`, va
  `IrohaSwift/Sources/IrohaSwift/ConnectClient.swift`; bu o'tishdan oldin
  bu yordamchilar bitta transport-siyosat eshigini baham ko'rishmadi.
- Java/Android endi xuddi shu siyosatni markazlashtiradi
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/TransportSecurity.java`
  va uni `NoritoRpcClient.java`, `ToriiRequestBuilder.java`,
  `OfflineToriiClient.java`, `SubscriptionToriiClient.java`,
  `stream/ToriiEventStreamClient.java`, va
  `websocket/ToriiWebSocketClient.java`.
- Kotlin endi bu siyosatni aks ettiradi
  `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/client/TransportSecurity.kt`
  va uni mos keladigan JVM mijozi/request-builder/event-stream/dan qo'llaydi.
  veb-rozetka sirtlari.
- JS `ToriiClient._request(...)` endi `canonicalAuth` va JSON jismlarini davolashadi
  ichida sezgir transport materiali sifatida `private_key*` maydonlarini o'z ichiga oladi
  `javascript/iroha_js/src/toriiClient.js` va telemetriya hodisasi shakli
  `javascript/iroha_js/index.d.ts` endi `hasSensitiveBody` /
  `allowInsecure` foydalanilganda `hasCanonicalAuth`.

Nima uchun bu muhim:

- Mobil, brauzer va mahalliy ishlab chiquvchi yordamchilar ko'pincha o'zgaruvchan ishlab chiquvchi/stagingga ishora qiladilar
  asosiy URL manzillar. Agar mijoz sezgir so'rovlarni sozlanganlarga qo'ymasa
  sxema/xost, qulaylik mutlaq URL bekor qilish yoki oddiy-HTTP bazasi mumkin
  SDK ni maxfiy-exfiltratsiya yoki imzolangan so'rovni pasaytirish yo'liga aylantiring.
- Xavf egasining tokenlariga qaraganda kengroqdir. Xom `private_key` JSON va yangi
  kanonik-auth imzolari ham sim va xavfsizlik sezgir
  transport siyosatini jimgina chetlab o'tmasligi kerak.

Tavsiya:- Har bir SDKda transport tekshiruvini markazlashtiring va uni tarmoq kiritish-chiqarishdan oldin qo'llang
  barcha nozik soʻrov shakllari uchun: autentifikatsiya sarlavhalari, ilovaning kanonik-auth imzosi,
  va xom `private_key*` JSON.
- `allowInsecure` ni faqat aniq mahalliy/dev escape lyuk sifatida saqlang va emissiya qiling
  qo'ng'iroq qiluvchilar uni tanlaganlarida telemetriya.
- Faqatgina emas, balki umumiy so'rovlar yaratuvchilarga qaratilgan regressiyalarni qo'shing
  yuqori darajadagi qulaylik usullari, shuning uchun kelajakdagi yordamchilar bir xil qo'riqchini meros qilib olishadi.

Tuzatish holati:- Joriy kodda yopilgan. Namuna olingan Swift, Java/Android, Kotlin va JS
  mijozlar endi sezgir uchun xavfsiz yoki o'zaro bog'liq transportlarni rad etadi
  Agar qo'ng'iroq qiluvchi faqat hujjatlashtirilgan ishlab chiqaruvchiga qo'shilmasa, yuqoridagi shakllarni so'rang
  xavfsiz rejim.
- Tezkor yo'naltirilgan regressiyalar endi xavfsiz bo'lmagan Norito-RPC auth sarlavhalarini qamrab oladi,
  xavfsiz bo'lmagan Connect websocket transporti va xom-`private_key` Torii so'rovi
  jismlar.
- Kotlinga yo'naltirilgan regressiyalar endi xavfsiz bo'lmagan Norito-RPC auth sarlavhalarini qamrab oladi,
  oflayn/obuna `private_key` korpuslari, SSE auth sarlavhalari va veb-rozetkasi
  auth sarlavhalari.
- Java/Android yo'naltirilgan regressiyalar endi xavfsiz bo'lmagan Norito-RPC autentsiyasini qamrab oladi
  sarlavhalar, oflayn/obuna `private_key` korpuslari, SSE auth sarlavhalari va
  umumiy Gradle jabduqlari orqali websocket auth sarlavhalari.
- JS yo'naltirilgan regressiyalar endi xavfsiz va o'zaro xostlarni qamrab oladi
  `private_key`-tana soʻrovlari hamda xavfsiz boʻlmagan `canonicalAuth` soʻrovlari
  `javascript/iroha_js/test/transportSecurity.test.js`, esa
  `javascript/iroha_js/test/toriiCanonicalAuth.test.js` endi ijobiy ishlaydi
  xavfsiz asosiy URL manzilidagi kanonik-auth yo'li.

### SEC-12: SoraFS mahalliy QUIC proksi-server mijozning autentifikatsiyasisiz orqaga qaytmaydigan ulanishlarni qabul qildi (2026-03-25 yopilgan)

Ta'sir:- `LocalQuicProxyConfig.bind_addr` avvalroq `0.0.0.0`, a
  LAN IP yoki "mahalliy" ni ochib beradigan boshqa qayta bo'lmagan manzil
  masofadan turib QUIC tinglovchisi sifatida ish stantsiyasining proksi-serveri.
- O'sha tinglovchi mijozlarni autentifikatsiya qilmagan. Mumkin bo'lgan har qanday tengdosh
  QUIC/TLS seansini yakunlang va versiyaga mos keladigan qoʻl siqishini yuboring
  keyin `tcp`, `norito`, `car` yoki `kaigi` oqimlarini oching.
  ko'prik rejimlari sozlangan.
- `bridge` rejimida operator noto'g'ri konfiguratsiyasini masofaviy TCPga aylantirdi.
  operator ish stantsiyasida o'rni va mahalliy fayl oqimi yuzasi.

Dalil:

- `LocalQuicProxyConfig::parsed_bind_addr(...)` dyuym
  `crates/sorafs_orchestrator/src/proxy.rs` ilgari faqat rozetkani tahlil qilgan
  manzil va orqaga qaytmaydigan interfeyslarni rad etmadi.
- Xuddi shu fayldagi `spawn_local_quic_proxy(...)` a bilan QUIC serverini ishga tushiradi
  o'z-o'zidan imzolangan sertifikat va `.with_no_client_auth()`.
- `handle_connection(...)` `ProxyHandshakeV1` har qanday mijozni qabul qiladi
  versiya yagona qo'llab-quvvatlanadigan protokol versiyasiga mos keldi va keyin kiritildi
  ilovalar oqimi tsikli.
- `handle_tcp_stream(...)` orqali o'zboshimchalik bilan `authority` qiymatlarini teradi
  `TcpStream::connect(...)`, `handle_norito_stream(...)` esa,
  `handle_car_stream(...)` va `handle_kaigi_stream(...)` mahalliy fayllarni oqimlash
  sozlangan spool/kesh kataloglaridan.

Nima uchun bu muhim:- O'z-o'zidan imzolangan sertifikat faqat mijoz bo'lsa, server identifikatorini himoya qiladi
  tekshirishni tanlaydi. Bu mijozning haqiqiyligini tasdiqlamaydi. Bir marta proksi
  ulanishdan tashqari ulanish mumkin edi, qo'l siqish yo'li faqat versiyaga teng edi
  qabul qilish.
- API va hujjatlar ushbu yordamchini mahalliy ish stantsiyasining proksi-server sifatida tavsiflaydi
  brauzer/SDK integratsiyasi, shuning uchun masofadan kirish mumkin bo'lgan ulanish manzillariga ruxsat berildi
  mo'ljallangan masofaviy xizmat ko'rsatish rejimi emas, balki ishonch chegarasining nomuvofiqligi.

Tavsiya:

- `bind_addr`-da yopilmagan, shuning uchun joriy yordamchi bo'lolmaydi.
  mahalliy ish stantsiyasidan tashqarida.
- Agar proksi-serverni masofaviy ta'sir qilish mahsulot talabiga aylansa, kiriting
  o'rniga birinchi navbatda mijozning aniq autentifikatsiyasi/qobiliyatini qabul qilish
  qo'riqchini bo'shashtirish.

Tuzatish holati:

- Joriy kodda yopilgan. `crates/sorafs_orchestrator/src/proxy.rs` hozir
  `ProxyError::BindAddressNotLoopback` bilan orqaga qaytmaydigan ulanish manzillarini rad etadi
  QUIC tinglovchisi boshlanishidan oldin.
- Konfiguratsiya maydonining hujjatlari
  `docs/source/sorafs/developer/orchestrator.md` va
  `docs/portal/docs/sorafs/orchestrator-config.md` endi hujjatlar
  `bind_addr` faqat orqaga qaytish sifatida.
- Endi regressiya qamroviga kiradi
  `spawn_local_quic_proxy_rejects_non_loopback_bind_addr` va mavjud
  ijobiy mahalliy ko'prik testi
  `proxy::tests::tcp_stream_bridge_transfers_payload` dyuym
  `crates/sorafs_orchestrator/src/proxy.rs`.

### SEC-13: chiquvchi P2P TLS-over-TCP sukut bo'yicha jimgina ochiq matnga tushirildi (yopiq 2026-03-25)

Ta'sir:- `network.tls_enabled=true`-ni yoqish aslida faqat TLS-ni qo'llamadi
  chiquvchi transport, agar operatorlar ham aniqlamasa va o'rnatmasa
  `tls_fallback_to_plain=false`.
- Shunday qilib, chiquvchi yo'lda har qanday TLS qo'l siqish xatosi yoki vaqt tugashi
  sukut bo'yicha kadrni ochiq matnli TCP ga tushirdi, bu esa transportni olib tashladi
  yo'lda hujumchilarga yoki noto'g'ri xatti-harakatlarga qarshi maxfiylik va yaxlitlik
  o'rta qutilar.
- Imzolangan ilovaning qo'l siqishi hali ham tengdosh identifikatorini tasdiqladi, shuning uchun
  Bu tengdoshlarni aylanib o'tish emas, balki transport siyosatini pasaytirish edi.

Dalil:

- `tls_fallback_to_plain` standart sifatida `true` da
  `crates/iroha_config/src/parameters/user.rs`, shuning uchun qaytarish faol edi
  operatorlar uni konfiguratsiyada aniq bekor qilmasalar.
- `Connecting::connect_tcp(...)` da `crates/iroha_p2p/src/peer.rs` urinishlari
  `tls_enabled` o'rnatilganda TLS tering, lekin TLS xatolarida yoki vaqt tugashida u
  ogohlantirishni qayd qiladi va istalgan vaqtda ochiq matnli TCP ga qaytadi
  `tls_fallback_to_plain` yoqilgan.
- Operatorga qaragan namuna konfiguratsiyasi `crates/iroha_kagami/src/wizard.rs` va
  `docs/source/p2p*.md` da jamoat P2P transport hujjatlari ham e'lon qilingan
  standart xatti-harakat sifatida ochiq matn zaxirasi.

Nima uchun bu muhim:- Operatorlar TLSni yoqsa, xavfsizroq kutish muvaffaqiyatsiz tugadi: agar TLS bo'lsa
  seansni o'rnatib bo'lmaydi, terish jimgina emas, balki muvaffaqiyatsiz bo'lishi kerak
  transport himoyasini yo'qotish.
- Sukut bo'yicha pasaytirishni yoqilgan holda qoldirish, joylashtirishni sezgir qiladi
  tarmoq yo'lidagi g'ayrioddiyliklar, proksi-server aralashuvi va faol qo'l siqish buzilishi a
  tarqatish paytida o'tkazib yuborish oson bo'lgan usul.

Tavsiya:

- To'g'ridan-to'g'ri matnni aniq moslik tugmasi sifatida saqlang, lekin uni sukut bo'yicha qiling
  `false`, shuning uchun `network.tls_enabled=true`, agar operator tanlamasa, faqat TLS degan ma'noni anglatadi.
  pasaytirish xatti-harakatlariga.

Tuzatish holati:

- Joriy kodda yopilgan. `crates/iroha_config/src/parameters/user.rs` hozir
  sukut bo'yicha `tls_fallback_to_plain` - `false`.
- Standart konfiguratsiya moslamasining surati, Kagami namuna konfiguratsiyasi va sukut bo'yicha o'xshash
  P2P/Torii test yordamchilari endi bu qattiqlashtirilgan ish vaqti sukutini aks ettiradi.
- Ikki nusxadagi `docs/source/p2p*.md` hujjatlari endi ochiq matnni qayta tiklashni quyidagicha tasvirlaydi.
  jo'natilgan standart o'rniga ochiq-oydin opt-in.

### SEC-14: Tengdosh telemetriya geo qidiruvi jimgina ochiq matnli uchinchi tomon HTTP-ga qaytdi (2026-03-25 yopilgan)

Ta'sir:- `torii.peer_geo.enabled=true` ni aniq yakuniy nuqtasiz yoqish
  Torii o'rnatilgan ochiq matnga tengdosh xost nomlarini yuborish uchun
  `http://ip-api.com/json/...` xizmati.
- Bu autentifikatsiya qilinmagan uchinchi tomon HTTP-ga tengdosh telemetriya maqsadlarini sizdirdi
  qaramlik va har qanday yo'lda tajovuzkor yoki buzilgan so'nggi nuqta tasmasini soxtalashtirishga ruxsat bering
  joylashuv metamaʼlumotlari Torii ga qaytadi.
- Bu xususiyat qo'shilgan edi, lekin umumiy telemetriya hujjatlari va namuna konfiguratsiyasi
  o'rnatilgan standartni e'lon qildi, bu esa xavfsiz bo'lmagan joylashtirish naqshini yaratdi
  Ehtimol, bir marta operatorlar geo-qidiruvlarni faollashtirgan.

Dalil:

- `crates/iroha_torii/src/telemetry/peers/monitor.rs` ilgari belgilangan
  `DEFAULT_GEO_ENDPOINT = "http://ip-api.com/json"` va
  `construct_geo_query(...)` har doim bu standartdan foydalangan
  `GeoLookupConfig.endpoint` `None` edi.
- Tengdosh telemetriya monitori istalgan vaqtda `collect_geo(...)` ni chiqaradi
  `geo_config.enabled` da to'g'ri
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`, shuning uchun ochiq matn
  Qayta tiklash faqat sinov uchun emas, balki jo'natilgan ish vaqti kodida mavjud edi.
- Konfiguratsiya sukut bo'yicha `crates/iroha_config/src/parameters/defaults.rs` va
  `crates/iroha_config/src/parameters/user.rs` `endpoint` o'rnatilmagan holda qoldiring va
  `docs/source/telemetry*.md` plus-da takrorlangan telemetriya hujjatlari
  `docs/source/references/peer.template.toml` aniq hujjatlashtirilgan
  Operatorlar ushbu xususiyatni faollashtirganda o'rnatilgan zaxira.

Nima uchun bu muhim:- Tengdosh telemetriyasi ochiq matnli HTTP orqali tengdosh xost nomlarini jimgina chiqmasligi kerak
  operatorlar qulaylik bayrog'ini o'girgandan so'ng, uchinchi tomon xizmatiga.
- Yashirin ishonchsiz sukut ham o'zgarishlarni ko'rib chiqishga putur etkazadi: operatorlar mumkin
  tashqi tomondan kiritilganligini sezmasdan geo qidiruvni yoqing
  uchinchi tomon metama'lumotlarini oshkor qilish va autentifikatsiya qilinmagan javoblarni qayta ishlash.

Tavsiya:

- O'rnatilgan geo so'nggi nuqta standartini olib tashlang.
- Tengdosh geo-qidiruvlar yoqilganda aniq HTTPS so'nggi nuqtasini talab qiling va
  aks holda qidiruvlarni o'tkazib yuboring.
- Yo'qolgan yoki HTTPS bo'lmagan so'nggi nuqtalarning yopilmaganligini isbotlovchi qaratilgan regressiyalarni saqlang.

Tuzatish holati:

- Joriy kodda yopilgan. `crates/iroha_torii/src/telemetry/peers/monitor.rs`
  endi `MissingEndpoint` bilan etishmayotgan so'nggi nuqtalarni rad etadi, HTTPS bo'lmaganlarni rad etadi
  `InsecureEndpoint` bilan so'nggi nuqtalar va o'rniga tengdoshlar uchun geo qidiruvni o'tkazib yuboradi
  jimgina ochiq matnli o'rnatilgan xizmatga qaytish.
- `crates/iroha_config/src/parameters/user.rs` endi yashirin in'ektsiya qilmaydi
  tahlil qilish vaqtida so'nggi nuqta, shuning uchun o'rnatilmagan konfiguratsiya holati hamma uchun ochiq bo'lib qoladi
  ish vaqtini tekshirish usuli.
- Ikki nusxadagi telemetriya hujjatlari va kanonik namuna konfiguratsiyasi
  `docs/source/references/peer.template.toml` endi buni bildiradi
  `torii.peer_geo.endpoint` HTTPS bilan aniq sozlanishi kerak
  funksiya yoqilgan.
- Endi regressiya qamroviga kiradi
  `construct_geo_query_requires_explicit_endpoint`,
  `construct_geo_query_rejects_non_https_endpoint`,
  `collect_geo_requires_explicit_endpoint_when_enabled`, va
  `collect_geo_rejects_non_https_endpoint` dyuym
  `crates/iroha_torii/src/telemetry/peers/monitor.rs`.### SEC-15: SoraFS pin va shlyuz siyosatida ishonchli proksi-serverdan xabardor mijoz-IP ruxsati yo'q (yopiq 2026-03-25)

Ta'sir:

- Torii teskari proksilangan o'rnatishlar avval SoraFS ni buzishi mumkin edi.
  CIDR saqlash pinini baholashda proksi soket manziliga qo'ng'iroq qiluvchilar
  ruxsat beruvchi ro'yxatlar, har bir mijoz uchun saqlash pinlari va shlyuz mijozi
  barmoq izlari.
- Bu `/v1/sorafs/storage/pin` va SoraFS da suiiste'mol nazoratini zaiflashtirdi.
  qilish orqali umumiy teskari proksi topologiyalarida shlyuzni yuklab olish yuzalarini
  bir nechta mijozlar bitta chelak yoki bitta ruxsat berilgan ro'yxat identifikatoriga ega.
- Standart yo'riqnoma hali ham ichki masofaviy metama'lumotlarni in'ektsiya qiladi, shuning uchun bu a emas edi
  yangi tasdiqlanmagan kirish chetlab o'tish, lekin bu haqiqiy ishonch chegarasi bo'shlig'i edi
  proksi-xabarli joylashtirishlar va ishlov beruvchi chegarasidan oshib ketish uchun
  ichki masofaviy IP sarlavhasi.

Dalil:- `crates/iroha_config/src/parameters/user.rs` va
  `crates/iroha_config/src/parameters/actual.rs` ilgari umumiy bo'lmagan
  `torii.transport.trusted_proxy_cidrs` tugmasi, shuning uchun proksi-serverdan xabardor kanonik mijoz
  IP ruxsatini umumiy Torii kirish chegarasida sozlash mumkin emas edi.
- `inject_remote_addr_header(...)`, `crates/iroha_torii/src/lib.rs` da
  avval dan ichki `x-iroha-remote-addr` sarlavhasini ustiga yozgan
  Faqat `ConnectInfo`, bu esa mijozning IP-meta-ma'lumotlarini yo'qotdi.
  haqiqiy teskari proksi-serverlar.
- `PinSubmissionPolicy::enforce(...)` dyuym
  `crates/iroha_torii/src/sorafs/pin.rs` va
  `gateway_client_fingerprint(...)`, `crates/iroha_torii/src/sorafs/api.rs`
  da ishonchli-proksi-xabarli kanonik-IP ruxsati qadamini baham ko'rmadi
  ishlov beruvchi chegarasi.
- `crates/iroha_torii/src/sorafs/pin.rs`-da saqlash pinini o'zgartirish ham kalitli
  faqat ko'taruvchi tokenda token mavjud bo'lganda, bu bir nechta ma'noni anglatadi
  bitta yaroqli pin tokenini baham ko'rgan proksi-mijozlar bir xil stavkaga majburlangan
  chelak, hatto ularning mijoz IP-lari ajratilgandan keyin ham.

Nima uchun bu muhim:

- Teskari proksi-serverlar Torii uchun oddiy joylashtirish namunasidir. Agar ish vaqti
  ishonchli proksi-serverni ishonchsiz qo'ng'iroq qiluvchi IP-dan doimiy ravishda ajrata olmaydi
  ruxsat ro'yxatlari va har bir mijozga drossellar operatorlar nimani o'ylayotganini anglatadi
  degani.
- SoraFS pin va shlyuz yo'llari aniq suiiste'molga sezgir yuzalardir, shuning uchun
  proksi IP-ga qo'ng'iroq qiluvchilarni to'sib qo'yish yoki haddan tashqari ishonchli eskirish
  metama'lumotlar bazaviy yo'nalish hali ham bo'lsa ham operatsion ahamiyatga ega
  boshqa kirishni talab qiladi.

Tavsiya:- Umumiy Torii `trusted_proxy_cidrs` konfiguratsiya yuzasini qo'shing va muammoni hal qiling.
  `ConnectInfo` dan bir marta kanonik mijoz IP va oldindan yuborilgan har qanday
  sarlavha faqat rozetkaning tengdoshi ruxsat etilgan ro'yxatda bo'lsa.
- SoraFS ishlov beruvchi yo'llari ichida o'sha kanonik-IP ruxsatini qayta ishlating.
  ichki sarlavhaga ko'r-ko'rona ishonish.
- Token va kanonik mijoz IP-si bo'yicha umumiy tokenli saqlash pinini blokirovka qilish doirasi
  ikkalasi ham mavjud bo'lganda.

Tuzatish holati:

- Joriy kodda yopilgan. `crates/iroha_config/src/parameters/defaults.rs`,
  `crates/iroha_config/src/parameters/user.rs`, va
  `crates/iroha_config/src/parameters/actual.rs` endi ochiladi
  `torii.transport.trusted_proxy_cidrs`, sukut bo'yicha uni bo'sh ro'yxatga kiritadi.
- `crates/iroha_torii/src/lib.rs` endi kanonik mijoz IP-ni hal qiladi
  `limits::ingress_remote_ip(...)` kirish o'rta dasturi ichida va qayta yozadi
  ichki `x-iroha-remote-addr` sarlavhasi faqat ishonchli proksi-serverlardan.
- `crates/iroha_torii/src/sorafs/pin.rs` va
  `crates/iroha_torii/src/sorafs/api.rs` endi kanonik mijoz IP manzillarini hal qiladi
  saqlash-pin uchun ishlov beruvchi chegarasida `state.trusted_proxy_nets` ga qarshi
  siyosat va shlyuz mijozi barmoq izlari, shuning uchun to'g'ridan-to'g'ri ishlov beruvchi yo'llari mumkin emas
  o'ta ishonchli eskirgan uzatilgan IP metama'lumotlari.
- Saqlash pinini blokirovka qilish endi `token + kanonik tomonidan umumiy tashuvchi tokenlarini kalitlarga beradi.
  mijoz IP` ikkalasi ham mavjud bo'lganda, har bir mijoz uchun chelaklarni birgalikda saqlash
  pin tokenlari.
- Endi regressiya qamroviga kiradi
  `limits::tests::ingress_remote_ip_preserves_trusted_forwarded_header`,
  `limits::tests::ingress_remote_ip_ignores_forwarded_header_from_untrusted_peer`,
  `sorafs::pin::tests::rate_key_scopes_shared_tokens_by_ip`,
  `storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy`,
  `car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy` va
  konfiguratsiya moslamasi
  `torii_transport_trusted_proxy_cidrs_default_to_empty`.### SEC-16: AOK qilingan masofaviy IP sarlavhasi yo'qolganda, operatorni autentifikatsiya qilish blokirovkasi va tezlikni cheklash umumiy anonim paqirga qaytdi (2026-03-25 yopilgan)

Ta'sir:

- Operator autentsiyasini qabul qilish allaqachon qabul qilingan IP soketini olgan, ammo
  qulflash/stavka chegarasi kaliti uni e'tiborsiz qoldirdi va so'rovlarni umumiy so'rovga yig'di
  Ichki `x-iroha-remote-addr` sarlavhasi bo'lganda `"anon"` paqir
  yo'q.
- Bu standart routerda yangi ommaviy kirish aylanmasi emas edi, chunki
  kirish o'rta dasturi ushbu ishlov beruvchilar ishga tushishidan oldin ichki sarlavhani qayta yozadi.
  Bu hali ham tor ichki uchun haqiqiy muvaffaqiyatsiz ochiq ishonch chegarasi bo'shliq edi
  ishlov berish yo'llari, to'g'ridan-to'g'ri testlar va kelajakdagi har qanday marshrut
  Inyeksion vosita dasturidan oldin `OperatorAuth`.
- Bunday hollarda bir qo'ng'iroq qiluvchining tarif limiti byudjetini iste'mol qilishi yoki boshqasini blokirovka qilishi mumkin
  manba IP tomonidan ajratilishi kerak bo'lgan qo'ng'iroq qiluvchilar.

Dalil:- `OperatorAuth::check_common(...)` dyuym
  `crates/iroha_torii/src/operator_auth.rs` allaqachon qabul qilingan
  `remote_ip: Option<IpAddr>`, lekin u ilgari `auth_key(headers)` deb nomlangan
  va transport IP-ni butunlay tashlab yubordi.
- Ilgari `crates/iroha_torii/src/operator_auth.rs` da `auth_key(...)`
  faqat `limits::REMOTE_ADDR_HEADER` tahlil qilindi va aks holda `"anon"` qaytarildi.
- `crates/iroha_torii/src/limits.rs`-dagi umumiy Torii yordamchisi allaqachon mavjud edi.
  `effective_remote_ip(sarlavhalar,
  masofaviy)`, in'ektsiya qilingan kanonik sarlavhani afzal ko'radi, lekin qaytib tushadi
  to'g'ridan-to'g'ri ishlov beruvchi chaqiruvlari o'rta dasturni chetlab o'tganda qabul qilingan soket IP.

Nima uchun bu muhim:

- Lokaut va tarif chegarasi holati bir xil samarali qo'ng'iroq qiluvchining identifikatoriga kalit bo'lishi kerak
  Torii ning qolgan qismi siyosat qarorlari uchun foydalanadi. Birgalikda ishlashga qaytish
  anonim paqir yetishmayotgan ichki metadata hopni o'zaro mijozga aylantiradi
  haqiqiy qo'ng'iroq qiluvchiga effektni mahalliylashtirish o'rniga shovqin.
- Operator auth - bu suiiste'mollikka sezgir chegara, shuning uchun hatto o'rtacha jiddiylik
  paqir-to'qnashuv muammosi aniq yopishga arziydi.

Tavsiya:

- Operator-auth kalitini `limits::effective_remote_ip(sarlavhalar,
  remote_ip)` shuning uchun kiritilgan sarlavha mavjud bo'lganda ham yutadi, lekin to'g'ridan-to'g'ri
  ishlov beruvchi chaqiruvlari `"anon"` o'rniga transport manziliga qaytadi.
- `"anon"` ni faqat oxirgi zaxira sifatida saqlang, agar ichki sarlavha va
  transport IP mavjud emas.

Tuzatish holati:- Joriy kodda yopilgan. `crates/iroha_torii/src/operator_auth.rs` endi qo'ng'iroq qilmoqda
  `check_common(...)` dan `auth_key(headers, remote_ip)` va `auth_key(...)`
  endi blokirovka/stavka chegarasi kalitini dan oladi
  `limits::effective_remote_ip(headers, remote_ip)`.
- Endi regressiya qamroviga kiradi
  `operator_auth_key_uses_remote_ip_when_internal_header_missing` va
  `operator_auth_key_prefers_injected_header_over_transport_remote_ip` dyuym
  `crates/iroha_torii/src/operator_auth.rs`.

## Avvalgi hisobotdagi yopiq yoki o'rnini bosgan topilmalar

- Ilgari xom-xususiy kalit Soracloud topilmasi: yopiq. Hozirgi mutatsiyaning kirib borishi
  qatordagi `authority` / `private_key` maydonlarini rad etadi
  `crates/iroha_torii/src/soracloud.rs:5305-5308`, HTTP imzolovchini bog'laydi
  `crates/iroha_torii/src/soracloud.rs:5310-5315` da mutatsiya kelib chiqishi va
  imzolangan serverni yuborish o'rniga tranzaksiya ko'rsatmalari loyihasini qaytaradi
  `crates/iroha_torii/src/soracloud.rs:5556-5565` da tranzaksiya.
- Ilgari faqat ichki mahalliy o'qiladigan proksi-server ijrosi topildi: yopiq. Ommaviy
  marshrut o'lchamlari endi umumiy bo'lmagan va yangilash/xususiy-yangilash ishlov beruvchilarini o'tkazib yuboradi
  `crates/iroha_torii/src/soracloud.rs:8445-8463` va ish vaqti rad etadi
  jamoat bo'lmagan mahalliy o'qiladigan marshrutlar
  `crates/irohad/src/soracloud_runtime.rs:5906-5923`.
- Ilgari ommaviy ish vaqti o'lchovsiz qayta tiklash: yozilganidek yopildi. Ommaviy
  Ish vaqtiga kirish endi tarif chegaralarini va parvoz chegaralarini joriy qiladi
  Umumiy marshrutni hal qilishdan oldin `crates/iroha_torii/src/lib.rs:8837-8852`
  `crates/iroha_torii/src/lib.rs:8858-8860`.
- Ilgari masofaviy-IP biriktirma-ijaraga olish: yopiq. Endi qo'shimcha ijara
  tasdiqlangan hisob qaydnomasini talab qiladi
  `crates/iroha_torii/src/lib.rs:7962-7968`.
  Ilova ijarasi SEC-05 va SEC-06 ni meros qilib olgan; o'sha meros
  yuqoridagi joriy ilova autentsiyasi tuzatishlari bilan yopilgan.## Bog'liqlik topilmalari- `cargo deny check advisories bans sources --hide-inclusion-graph` hozir ishlaydi
  bevosita kuzatilgan `deny.toml` qarshi va endi uch jonli xabar
  yaratilgan ish maydoni qulf faylidan qaramlik topilmalari.
- `tar` maslahatlari endi faol qaramlik grafigida mavjud emas:
  `xtask/src/mochi.rs` endi belgilangan argument bilan `Command::new("tar")` dan foydalanadi
  vektor va `iroha_crypto` endi `libsodium-sys-stable` ni tortmaydi
  Ushbu cheklarni OpenSSL ga almashtirgandan so'ng Ed25519 interop testlarini o'tkazadi.
- Joriy topilmalar:
  - `RUSTSEC-2024-0388`: `derivative` ta'mirlanmagan.
  - `RUSTSEC-2024-0436`: `paste` ta'mirlanmagan.
- Ta'sir triaji:
  - Ilgari xabar qilingan `tar` maslahatlari faollar uchun yopiq
    qaramlik grafigi. `cargo tree -p xtask -e normal -i tar`,
    `cargo tree -p iroha_crypto -e all -i tar`, va
    `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` endi hammasi muvaffaqiyatsiz
    "paket identifikatori spetsifikatsiyasi ... hech qanday paketga mos kelmadi" va
    `cargo deny` endi `RUSTSEC-2026-0067` haqida xabar bermaydi yoki
    `RUSTSEC-2026-0068`.
  - Ilgari xabar qilingan to'g'ridan-to'g'ri PQ almashtirish bo'yicha maslahatlar endi yopiq
    hozirgi daraxt. `crates/soranet_pq/Cargo.toml`,
    `crates/iroha_crypto/Cargo.toml` va `crates/ivm/Cargo.toml` endi bog'liq
    `pqcrypto-mldsa` / `pqcrypto-mlkem` va tegilgan ML-DSA / ML-KEM da
    ish vaqti testlari migratsiyadan keyin ham o'tadi.
  - Ilgari xabar qilingan `rustls-webpki` maslahati endi faol emas
    joriy qaror. Ish maydoni endi `reqwest` / `rustls` ni`rustls-webpki`ni `0.103.10` da saqlaydigan yamoqli yamoq relizlari, ya'ni
    maslahat doirasidan tashqarida.
  - `derivative` va `paste` to'g'ridan-to'g'ri ish maydoni manbasida ishlatilmaydi.
    Ular ostidagi BLS / arkworks stek orqali o'tishli kirishadi
    `w3f-bls` va boshqa bir nechta yuqori oqim qutilari, shuning uchun ularni olib tashlash uchun
    mahalliy so'l tozalashdan ko'ra yuqori oqim yoki qaramlik stekidagi o'zgarishlar.
    Hozirgi daraxt endi ushbu ikkita maslahatni aniq qabul qiladi
    `deny.toml` sabablari qayd etilgan.

## Qoplama eslatmalari- Server/ish vaqti/konfiguratsiya/tarmoq: SEC-05, SEC-06, SEC-07, SEC-08, SEC-09,
  SEC-10, SEC-12, SEC-13, SEC-14, SEC-15 va SEC-16 tasdiqlandi
  audit va hozir joriy daraxtda yopilgan. Qo'shimcha qattiqlashuv
  joriy daraxt endi Connect websocket/sessiyani qabul qilish muvaffaqiyatsiz tugadi
  o'rniga ichki AOK qilingan masofaviy IP sarlavhasi yo'q bo'lganda yopiladi
  bu shartni orqaga qaytarish uchun sukut bo'yicha.
- IVM/kripto/seriyalashtirish: bu auditdan qo'shimcha tasdiqlangan topilma yo'q
  tilim. Ijobiy dalillar sirli asosiy materiallarni nolga kiritishni o'z ichiga oladi
  `crates/iroha_crypto/src/confidential.rs:53-60` va qayta tinglashdan xabardor Soranet PoW
  `crates/iroha_crypto/src/soranet/pow.rs:823-879` da imzolangan chipta tekshiruvi.
  Keyingi qattiqlashuv endi noto'g'ri shakllangan tezlatgich chiqishini ikkiga bo'lishdan ham rad etadi
  namunali Norito yo'llari: `crates/norito/src/lib.rs` tezlashtirilgan JSONni tasdiqlaydi
  `TapeWalker` dan oldingi 1-bosqich lentalari ofsetlarni bekor qiladi va endi ham talab qiladi
  bilan tenglikni isbotlash uchun dinamik ravishda yuklangan Metal/CUDA Stage-1 yordamchilari
  faollashtirishdan oldin skalyar strukturaviy-indeks quruvchisi va
  `crates/norito/src/core/gpu_zstd.rs` GPU tomonidan bildirilgan chiqish uzunliklarini tasdiqlaydi
  kodlash/dekodlash buferlarini kesishdan oldin. `crates/norito/src/core/simd_crc64.rs`
  endi dinamik ravishda yuklangan GPU CRC64 yordamchilarini ham o'z-o'zini sinab ko'radi
  `hardware_crc64` oldin kanonik qayta tiklash ularga ishonadi, shuning uchun noto'g'ri
  Norito nazorat summasini jimgina o'zgartirish o'rniga yordamchi kutubxonalar yopilmaydixulq-atvor. Noto'g'ri yordamchi natijalar endi vahima chiqarish o'rniga qaytariladi
  tuzadi yoki drift nazorat summasi pariteti. IVM tomonida, namunali tezlatgich
  startap eshiklari endi CUDA Ed25519 `signature_kernel`, CUDA BN254 ni ham qamrab oladi
  qo'shish/sub/mul yadrolari, CUDA `sha256_leaves` / `sha256_pairs_reduce`, jonli
  CUDA vektor/AES ommaviy yadrolari (`vadd64`, `vand`, `vxor`, `vor`,
  `aesenc_batch`, `aesdec_batch`) va mos keladigan metall
  Ushbu yo'llar ishonchli bo'lishidan oldin `sha256_leaves`/vector/AES ommaviy yadrolari. The
  namunali Metall Ed25519 imzo yo'li endi qaytib keldi
  Ushbu xostda o'rnatilgan jonli tezlatgich ichida: oldingi paritet xatosi edi
  skaler narvon bo'ylab ref10 a'zolar bilan bog'langan normalizatsiyani tiklash orqali o'rnatiladi,
  va yo'naltirilgan Metall regressiya endi `[s]B`, `[h](-A)`,
  ikkitadan quvvatli zinapoya va to'liq `[true, false]` to'plamini tekshirish
  CPU mos yozuvlar yo'liga qarshi metall ustida. Oynali CUDA manbasi o'zgaradi
  `--features cuda --tests` ostida kompilyatsiya qiling va CUDA ishga tushirish haqiqati hozir o'rnatiladi
  jonli Merkle bargi/juft yadrolari protsessordan chiqib ketsa, yopilmaydi
  mos yozuvlar yo'li. Ish vaqti CUDA tekshiruvi bunda xost bilan cheklangan bo'lib qoladi
  muhit.
- SDK/misollar: SEC-11 transportga yo'naltirilgan namunalar davomida tasdiqlangan
  Swift, Java/Android, Kotlin va JS mijozlari orqali o'tish va hokazotopilma hozirgi daraxtda yopildi. JS, Swift va Android
  kanonik so'rov yordamchilari ham yangisiga yangilandi
  tazelikdan xabardor to'rt sarlavhali sxema.
  Namuna olingan QUIC oqimli transport tekshiruvi ham jonli efirga chiqmadi
  joriy daraxtda ish vaqtini topish: `StreamingClient::connect(...)`,
  `StreamingServer::bind(...)` va qobiliyat-muzokara yordamchilari
  hozirda faqat `crates/iroha_p2p` va test kodidan foydalaniladi
  `crates/iroha_core`, shuning uchun ushbu yordamchida ruxsat beruvchi o'z-o'zidan imzolangan tasdiqlovchi
  yo'l hozirda yuborilgan kirish joyidan ko'ra sinov/yordamchi uchundir.
- Misollar va mobil namunaviy ilovalar faqat spot-check darajasida ko'rib chiqildi va
  to'liq tekshirilgan deb hisoblanmasligi kerak.

## Tasdiqlash va qoplash bo'shliqlari- `cargo deny check advisories bans sources --hide-inclusion-graph` hozir ishlaydi
  bevosita kuzatilgan `deny.toml` bilan. Ushbu joriy sxema ostida,
  `bans` va `sources` toza, `advisories` esa beshta ishlamayapti.
  yuqorida sanab o'tilgan qaramlik natijalari.
- O'tkazilgan yopiq `tar` topilmalari uchun qaramlik grafikini tozalash tekshiruvi:
  `cargo tree -p xtask -e normal -i tar`,
  `cargo tree -p iroha_crypto -e all -i tar`, va
  `cargo tree -p iroha_crypto -e all -i libsodium-sys-stable` hozir hammasi hisobot
  "paket identifikatori spetsifikatsiyasi ... hech qanday paketga mos kelmadi" esa
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p xtask create_archive_packages_bundle_directory -- --nocapture`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo check -p xtask`,
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_verify -- --nocapture`,
  va
  `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 cargo test -p iroha_crypto ed25519_sign -- --nocapture`
  hammasi o'tdi.
- `bash scripts/fuzz_smoke.sh` endi haqiqiy libFuzzer seanslarini orqali ishlaydi
  `cargo +nightly fuzz`, lekin IVM skriptning yarmi tugamadi
  bu o'tish, chunki `tlv_validate` uchun birinchi tungi qurilish hali ham mavjud edi
  topshirishda taraqqiyot. O'sha vaqtdan beri bu qurilishni amalga oshirish uchun etarli darajada yakunlandi
  libFuzzer ikkilik faylini bevosita yaratdi:
  `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
  endi libFuzzer ishga tushirish tsikliga etib boradi va 200 ta yugurishdan so'ng toza chiqadi
  bo'sh korpus. Norito yarmini tuzatgandan so'ng muvaffaqiyatli yakunlandi
  jabduqlar/manifest drifti va `json_from_json_equiv` fuzz-target kompilyatsiyasi
  tanaffus.
- Torii remediatsiya tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo check -p iroha_torii --lib`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib --no-run`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_accepts_valid_signature -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib verify_rejects_replayed_nonce -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_key_ -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-operator-auth-key CARGO_TARGET_DIR=/tmp/iroha-codex-target-operator-auth-key cargo test -p iroha_torii --lib operator_auth_rejects_forwarded_mtls_from_untrusted_proxy -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p iroha_torii --lib trusted_forwarded_header_requires_proxy_membership -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo check -p iroha_torii --lib --features app_api_https`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook-https cargo test -p iroha_torii --lib https_delivery_dns_override_ --features app_api_https -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-webhook2 cargo test -p iroha_torii --lib websocket_pinned_connect_addr_pins_secure_delivery_when_guarded -- --nocapture`- `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_preserves_inbound_remote_addr_for_internal_allowlist_checks -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib dispatch_route_blocks_remote_addr_spoofing_from_extra_headers -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-mcp cargo test -p iroha_torii --lib apply_extra_headers_blocks_reserved_internal_headers -- --nocapture`
  - torroq `--no-default-features --features app_api,app_api_https` Torii
    test matritsasi hali ham DAda bog'liq bo'lmagan mavjud kompilyatsiya xatolariga ega /
    Soracloud-gated lib-test kodi, shuning uchun bu o'tish jo'natilganni tasdiqladi
    sukut bo'yicha MCP yo'li va `app_api_https` veb-huk yo'li
    to'liq minimal xususiyatli qamrovga da'vo.
- Ishonchli proksi/SoraFS remediatsiya tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib limits::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib sorafs::pin::tests:: -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_requires_token_and_respects_allowlist_and_rate_limit -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib storage_pin_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limits_repeated_clients -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_torii --lib car_range_rate_limit_uses_forwarded_client_ip_from_trusted_proxy -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-trusted-proxy CARGO_TARGET_DIR=/tmp/iroha-codex-target-trusted-proxy cargo test -p iroha_config --test fixtures torii_transport_trusted_proxy_cidrs_default_to_empty -- --nocapture`
- P2P remediatsiya tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib --no-run`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib outgoing_handshake_rejects_unexpected_peer_identity -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p cargo test -p iroha_p2p --lib handshake_v1_defaults_to_trust_gossip -- --nocapture`
- SoraFS mahalliy proksi remediatsiyasini tekshirish endi quyidagilarni o'z ichiga oladi:
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-proxy cargo test -p sorafs_orchestrator spawn_local_quic_proxy_rejects_non_loopback_bind_addr -- --nocapture`
  - `/tmp/iroha-codex-target-proxy/debug/deps/sorafs_orchestrator-b3be10a343598c7b --exact proxy::tests::tcp_stream_bridge_transfers_payload --nocapture`
- P2P TLS-standart remediatsiya tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-config-tls2 cargo test -p iroha_config tls_fallback_defaults_to_tls_only -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib start_rejects_tls_without_feature_when_tls_only_outbound -- --nocapture`
  - `CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-p2p-sec13 cargo test -p iroha_p2p --lib tls_only_dial_requires_p2p_tls_feature_when_no_fallback -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_HOME=/tmp/iroha-cargo-home-sec13 CARGO_TARGET_DIR=/tmp/iroha-codex-target-torii-connect-gating cargo test -p iroha_torii --test connect_gating --no-run`
- SDK tomonida remediatsiya tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `node --test javascript/iroha_js/test/canonicalRequest.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `cd IrohaSwift && swift test --filter CanonicalRequestTests`
  - `cd IrohaSwift && swift test --filter 'NoritoRpcClientTests/testCallRejectsInsecureAuthorizationHeader'`
  - `cd IrohaSwift && swift test --filter 'ConnectClientTests/testBuildsConnectWebSocketRequestRejectsInsecureTransport'`
  - `cd IrohaSwift && swift test --filter 'ToriiClientTests/testCreateSubscriptionPlanRejectsInsecureTransportForPrivateKeyBody'`
  - `cd kotlin && ./gradlew :core-jvm:test --tests org.hyperledger.iroha.sdk.client.TransportSecurityClientTest --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ./gradlew android:compileDebugUnitTestJavaWithJavac --console=plain`
  - `cd java/iroha_android && JAVA_HOME=$(/usr/libexec/java_home -v 21) ANDROID_HOME=~/Library/Android/sdk ANDROID_SDK_ROOT=~/Library/Android/sdk ANDROID_HARNESS_MAINS=org.hyperledger.iroha.android.client.NoritoRpcClientTests,org.hyperledger.iroha.android.client.OfflineToriiClientTests,org.hyperledger.iroha.android.client.SubscriptionToriiClientTests,org.hyperledger.iroha.android.client.stream.ToriiEventStreamClientTests,org.hyperledger.iroha.android.client.websocket.ToriiWebSocketClientTests ./gradlew :core:test --tests org.hyperledger.iroha.android.GradleHarnessTests --console=plain`
  - `node --test javascript/iroha_js/test/transportSecurity.test.js javascript/iroha_js/test/toriiCanonicalAuth.test.js`
  - `node --test javascript/iroha_js/test/toriiSubscriptions.test.js`
- Norito nazorat tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `python3 scripts/check_norito_bindings_sync.py`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_out_of_bounds_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito validate_accel_rejects_non_structural_offsets -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_encode_rejects_invalid_success_length -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target cargo test -p norito try_gpu_decode_rejects_invalid_success_length -- --nocapture`
  - `bash scripts/fuzz_smoke.sh` (Norito maqsadlari `json_parse_string`,
    `json_parse_string_ref`, `json_skip_value` va `json_from_json_equiv`jabduqlar/maqsad tuzatishlaridan keyin o'tdi)
- IVM fuzz kuzatuv tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `cd crates/ivm/fuzz && ./target/aarch64-apple-darwin/release/tlv_validate -runs=200 -rss_limit_mb=3072 -max_total_time=15`
- IVM tezlatgichni kuzatish tekshiruvi endi quyidagilarni o'z ichiga oladi:
  - `xcrun -sdk macosx metal -c crates/ivm/src/metal_ed25519.metal -o /tmp/metal_ed25519.air`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo check -p ivm --features cuda --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda-check cargo check -p ivm --features cuda --tests`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo check -p ivm --features metal --tests`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_bitwise_single_vector_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_aes_batch_matches_scalar -- --nocapture`
  - `NORITO_KOTLIN_SKIP_TESTS=1 NORITO_JAVA_SKIP_TESTS=1 CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_ed25519_batch_matches_cpu -- --nocapture`
  - `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-metal cargo test -p ivm --features metal --lib metal_sha256_leaves_matches_cpu -- --nocapture`
- Fokuslangan CUDA lib-testini bajarish ushbu xostda muhit bilan cheklangan bo'lib qoladi:
  `CARGO_TARGET_DIR=/tmp/iroha-codex-target-ivm-cuda2 cargo test -p ivm --features cuda --lib selftest_covers_ -- --nocapture`
  CUDA drayver belgilari (`cu*`) mavjud emasligi sababli hali ham ulana olmaydi.
- Focused Metal ish vaqtini tekshirish endi buning tezlatgichida to'liq ishlaydi
  xost: namunali Ed25519 imzo quvuri ishga tushirish orqali yoqilgan qoladi
  o'z-o'zini sinab ko'radi va `metal_ed25519_batch_matches_cpu` `[true, false]`ni tasdiqlaydi
  to'g'ridan-to'g'ri metall ustida CPU mos yozuvlar yo'liga qarshi.
- Men to'liq ish joyini Rust sinovini, to'liq `npm test` yoki
  Ushbu tuzatish vaqtida to'liq Swift/Android to'plamlari.

## Tartibga solishning ustuvor vazifalari

### Keyingi transh- To'g'ridan-to'g'ri qabul qilingan o'tish uchun yuqori oqimlarni almashtirishni kuzatib boring
  `derivative` / `paste` makro qarz va `deny.toml` istisnolarini olib tashlang
  xavfsiz yangilanishlar BLS / Halo2 / PQ / ni beqarorlashtirmasdan mavjud bo'ladi.
  UI qaramlik steklari.
- To'liq tungi IVM fuzz-smoke skriptini issiq keshda qayta ishga tushiring.
  `tlv_validate` / `kotodama_lower` yonidagi barqaror qayd etilgan natijalarga ega
  hozir yashil Norito maqsadlari. To'g'ridan-to'g'ri `tlv_validate` ikkilik ishga tushirish endi tugadi,
  lekin to'liq skript tungi tutun hali ham ajoyib.
- CUDA drayveri bilan xostda yo'naltirilgan CUDA lib-testining o'z-o'zini sinab ko'rish qismini qayta ishga tushiring
  kutubxonalar o'rnatilgan, shuning uchun kengaytirilgan CUDA ishga tushirish haqiqati to'plami tasdiqlangan
  `cargo check` va aks ettirilgan Ed25519 normalizatsiya tuzatishidan tashqari
  yangi vektor/AES ishga tushirish problari ish vaqtida amalga oshiriladi.
- Kengroq JS/Swift/Android/Kotlin to'plamlarini bir-biriga bog'liq bo'lmagan to'plam darajasidan keyin qayta ishga tushiring
  Ushbu filialdagi blokerlar tozalanadi, shuning uchun yangi kanonik-so'rov va
  transport-xavfsizlik qo'riqchilari yuqorida yo'naltirilgan yordamchi testlardan tashqari qamrab olingan.
- Uzoq muddatli ilova-auth multisig hikoyasi qolishi kerakmi yoki yo'qligini hal qiling
  muvaffaqiyatsiz yopildi yoki birinchi darajali HTTP multisig guvoh formatini o'sadi.

### Monitor- `ivm` apparat tezlashuvi/xavfsiz yo'llarini diqqat bilan ko'rib chiqishni davom eting
  va qolgan `norito` oqim/kripto chegaralari. JSON bosqichi-1
  va GPU zstd yordamchi uzatishlari endi yopilish uchun qotib qolgan
  relizlar tuziladi va namunali IVM tezlatkich ishga tushirish haqiqati to'plamlari endi
  kengroq, ammo kengroq xavfli/determinizm tekshiruvi hali ham ochiq.