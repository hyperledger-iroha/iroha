---
lang: uz
direction: ltr
source: docs/source/connect_architecture_strawman.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1a6bcc6bca3d7f70b82e35734b71d706ac46d8dc9c728351fabbd8a61dd3f31
source_last_modified: "2026-01-05T09:28:12.003461+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ulanish sessiyasi arxitekturasi Strawman (Swift / Android / JS)

Ushbu strawman taklifi Nexus Connect ish oqimlari uchun umumiy dizaynni tavsiflaydi
Swift, Android va JavaScript SDK-larida. ni qo'llab-quvvatlash uchun mo'ljallangan
2026   fevral SDK bo‘yicha seminar va amalga oshirishdan oldin ochiq savollarni yozib oling.

> Oxirgi yangilangan: 29.01.2026  
> Mualliflar: Swift SDK Lead, Android Networking TL, JS Lead  
> Holat: kengash ko‘rib chiqish loyihasi (tahdid modeli + ma’lumotlarni saqlash moslashuvi 2026-03-12 qo‘shilgan)

## Maqsadlar

1. Hamyonni ↔ dApp seansining hayot aylanishini, jumladan ulanishni yuklash,
   tasdiqlash, imzolash so'rovlari va parchalanish.
2. Hamma tomonidan ulashiladigan Norito konvert sxemasini (ochish/tasdiqlash/imzolash/nazorat qilish) belgilang
   SDK va `connect_norito_bridge` bilan paritetni ta'minlang.
3. Tashish (WebSocket/WebRTC), shifrlash o'rtasida mas'uliyatni taqsimlash
   (Norito Ulanish ramkalari + kalit almashinuvi) va dastur qatlamlari (SDK jabhalari).
4. Ish stoli/mobil platformalarida, jumladan, deterministik xatti-harakatni ta'minlash
   oflayn buferlash va qayta ulanish.

## Seansning hayot aylanishi (yuqori daraja)

```
┌────────────┐      ┌─────────────┐      ┌────────────┐
│  dApp SDK  │←────→│  Connect WS │←────→│ Wallet SDK │
└────────────┘      └─────────────┘      └────────────┘
      │                    │                    │
      │ 1. open (app→wallet) frame (metadata, permissions, chain_id)
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 2. route frame     │
      │                    │────────────────────│
      │                    │                    │
      │                    │     3. approve frame (wallet pk, account,
      │                    │        permissions, proof/attest)
      │<────────────────────────────────────────│
      │                    │                    │
      │ 4. sign request    │                    │
      │────────────────────────────────────────>│
      │                    │                    │
      │                    │ 5. sign result     │
      │                    │────────────────────│
      │                    │                    │
      │ 6. control frames for reject/close, error propagation, heartbeats.
```

## Konvert/Norito sxemasi

Barcha SDKlar `connect_norito_bridge` da belgilangan kanonik Norito sxemasidan foydalanishi SHART:

- `EnvelopeV1` (ochish / tasdiqlash / imzolash / boshqarish)
- `ConnectFrameV1` (AEAD foydali yuki bilan shifrlangan matnli ramkalar)
- Boshqarish kodlari:
  - `open_ext` (metadata, ruxsatlar)
  - `approve_ext` (hisob, ruxsatlar, dalillar, imzo)
  - `reject`, `close`, `ping/pong`, `error`

Swift oldindan jo'natilgan to'ldiruvchi JSON enkoderlari (`ConnectCodec.swift`). 2026 yil aprel holatiga ko'ra SDK
har doim Norito ko'prigidan foydalanadi va XCFramework yo'qolganda yopilmaydi, lekin bu strawman
hali ham ko'prik integratsiyasiga olib kelgan mandatni egallab turibdi:

| Funktsiya | Tavsif | Holati |
|----------|-------------|--------|
| `connect_norito_encode_control_open_ext` | dApp ochiq ramka | Ko'prikda amalga oshirildi |
| `connect_norito_encode_control_approve_ext` | Hamyonni tasdiqlash | Amalga oshirildi |
| `connect_norito_encode_envelope_sign_request_tx/raw` | Imzo so'rovlari | Amalga oshirildi |
| `connect_norito_encode_envelope_sign_result_ok/err` | Natijalarni imzolash | Amalga oshirildi |
| `connect_norito_decode_*` | Hamyonlar/dApps uchun tahlil qilish | Amalga oshirildi |

### Majburiy ish

- Swift: `ConnectCodec` JSON yordamchilarini ko'prik qo'ng'iroqlari va sirt bilan almashtiring
  umumiy Norito turlaridan foydalangan holda yoziladigan o'ramlar (`ConnectFrame`, `ConnectEnvelope`). ✅ (2026 yil aprel)
- Android/JS: bir xil o'ramlar mavjudligiga ishonch hosil qiling; xato kodlari va metadata kalitlarini tekislang.
- Umumiy: doimiy kalit hosilasi bilan hujjat shifrlash (X25519 kalit almashinuvi, AEAD)
  Norito spetsifikatsiyasi bo'yicha va Rust ko'prigi yordamida namunaviy integratsiya testlarini taqdim eting.

## Transport shartnomasi- Asosiy transport: WebSocket (`/v1/connect/ws?sid=<session_id>`).
- Majburiy emas kelajak: WebRTC (TBD) - boshlang'ich strawman uchun imkoniyatdan tashqarida.
- Qayta ulanish strategiyasi: to'liq jitter bilan eksponensial orqaga qaytish (baza 5s, maksimal 60s); Swift, Android va JS bo'ylab umumiy konstantalar, shuning uchun qayta urinishlar oldindan aytish mumkin bo'lib qoladi.
- Ping/pong kadansi: qayta ulanishdan oldin uchta o'tkazib yuborilgan tennis uchun tolerantlik bilan 30s yurak urishi; JS brauzerni qisqartirish qoidalarini qondirish uchun minimal intervalni 15 soniyagacha qisqartiradi.
- Ilgaklar: Android hamyoni SDK uyg'onish uchun ixtiyoriy FCM integratsiyasini taqdim etadi, JS esa so'rovga asoslangan bo'lib qoladi (brauzerni surish ruxsatlari uchun hujjatlashtirilgan cheklovlar).
- SDK majburiyatlari:
  - Ping/pong yurak urishlarini saqlang (mobil telefonda batareyalarni zaryadsizlantirishdan saqlaning).
  - Oflayn rejimda chiquvchi kadrlarni buferlash (chegaralangan navbat, dApp uchun davom etadi).
- Voqealar oqimining API-ni taqdim eting (Swift Combine `AsyncStream`, Android Flow, JS async iter).
- Ilgaklarni sirtdan ulang va qo'lda qayta obuna bo'lishiga ruxsat bering.
- Telemetriya redaktsiyasi: faqat sessiya darajasidagi hisoblagichlarni chiqaradi (`sid` xesh, yo'nalish,
  ketma-ketlik oynasi, navbat chuqurligi) Connect telemetriyasida hujjatlashtirilgan tuzlar bilan
  rahbar; sarlavhalar/kalitlar hech qachon jurnallarda yoki disk raskadrovka satrlarida ko'rinmasligi kerak.

## Shifrlash va kalitlarni boshqarish

### Seans identifikatorlari va tuzlari

- `sid` `BLAKE2b-256("iroha-connect|sid|" || chain_id || app_ephemeral_pk || nonce16)` dan olingan 32 baytli identifikator.  
  DApps uni `/v1/connect/session` ga qo'ng'iroq qilishdan oldin hisoblab chiqadi; hamyonlar buni `approve` ramkalarida aks ettiradi, shuning uchun har ikki tomon ham jurnallar va telemetriyani doimiy ravishda kaliti mumkin.
- Xuddi shu tuz har bir kalit hosila bosqichini oziqlantiradi, shuning uchun SDK hech qachon xost platformasidan olingan entropiyaga tayanmaydi.

### Efemer kalit bilan ishlash

- Har bir seans yangi X25519 kalit materialidan foydalanadi.  
  Swift uni `ConnectCrypto` orqali Keychain/Secure Enclave-da saqlaydi, Android hamyonlari sukut bo'yicha StrongBox (TEE tomonidan qo'llab-quvvatlanadigan kalit do'konlariga tushadi) va JS xavfsiz kontekstli WebCrypto nusxasini yoki mahalliy `iroha_js_host` plaginini talab qiladi.
- Ochiq ramkalar dApp efemer ochiq kaliti va ixtiyoriy attestatsiya to'plamini o'z ichiga oladi. Hamyonni tasdiqlash hamyonning ochiq kalitini va muvofiqlik oqimi uchun zarur bo‘lgan har qanday apparat sertifikatini qaytaradi.
- Attestatsiyaning foydali yuklari qabul qilingan sxema bo'yicha:  
  `attestation { platform, evidence_b64, statement_hash }`.  
  Brauzerlar blokni o'tkazib yuborishi mumkin; mahalliy hamyonlar uni apparat kalitlari ishlatilayotganda o'z ichiga oladi.

### Yo'nalish tugmalari va AEAD

- Umumiy sirlar HKDF-SHA256 (Rust ko'prigi yordamchilari orqali) va domendan ajratilgan ma'lumotlar qatorlari bilan kengaytirilgan:
  - `iroha-connect|k_app` → ilova→hamyon trafiki.
  - `iroha-connect|k_wallet` → hamyon → ilova trafiki.
- AEAD v1 konvert uchun ChaCha20-Poly1305 (`connect_norito_bridge` har bir platformada yordamchilarni ko'rsatadi).  
  Bog'langan ma'lumotlar `("connect:v1", sid, dir, seq_le, kind=ciphertext)` ga teng, shuning uchun sarlavhalarni buzish aniqlanadi.
- Nonces 64-bitli ketma-ketlik hisoblagichidan (`nonce[0..4]=0`, `nonce[4..12]=seq_le`) olingan. Birgalikda yordamchi testlar BigInt/UInt konversiyalari SDKlarda bir xil harakat qilishini ta'minlaydi.

### Aylantirish va tiklash qo'l siqish- Aylanish ixtiyoriy bo‘lib qoladi, lekin protokol aniqlangan: ketma-ketlik hisoblagichlari o‘rash himoyasiga yaqinlashganda dApps `Control::RotateKeys` ramkasini chiqaradi, hamyonlar yangi ochiq kalit va imzolangan tasdiq bilan javob beradi va har ikki tomon ham sessiyani yopmasdan darhol yangi yo‘nalish kalitlarini chiqaradi.
- Hamyon tomonidagi kalit yo‘qolishi bir xil qo‘l siqishni va undan so‘ng `resume` boshqaruvini ishga tushiradi, shuning uchun dApps eskirgan kalitga mo‘ljallangan keshlangan shifrlangan matnni tozalashni biladi.

Tarixiy CryptoKit zaxiralari uchun `docs/connect_swift_ios.md` ga qarang; Kotlin va JS `docs/connect_kotlin_ws*.md` ostida mos havolalarga ega.

## Ruxsatlar va dalillar

- Ruxsat manifestlari ko'prik orqali eksport qilingan umumiy Norito tuzilmasi bo'ylab aylanib chiqishi kerak.  
  Maydonlar:
  - `methods` — fe'llar (`sign_transaction`, `sign_raw`, `submit_proof`, …).  
  - `events` — dApp ilovasini biriktirishga ruxsat berilgan obunalar.  
  - `resources` - ixtiyoriy hisob/aktiv filtrlari, shuning uchun hamyonlar kirishni qamrab oladi.  
  - `constraints` - zanjir identifikatori, TTL yoki maxsus siyosat tugmalari hamyon imzolashdan oldin amalga oshiradi.
- Ruxsatlar bilan bir qatorda muvofiqlik metama'lumotlari:
  - Majburiy emas `attachments[]` Norito biriktirma havolalarini o'z ichiga oladi (KYC to'plamlari, regulyator kvitantsiyalari).  
  - `compliance_manifest_id` so'rovni avval tasdiqlangan manifest bilan bog'laydi, shuning uchun operatorlar kelib chiqishini tekshirishlari mumkin.
- Hamyon javoblari kelishilgan kodlardan foydalanadi:
  - `user_declined`, `permissions_mismatch`, `compliance_failed`, `internal_error`.  
  Ularning har birida UI ko‘rsatmalari uchun `localized_message` hamda mashinada o‘qiladigan `reason_code` bo‘lishi mumkin.
- Tasdiqlash ramkalari tanlangan hisob/nazoratchi, ruxsat aks-sadosi, isbot to‘plami (ZK isboti yoki attestatsiyasi) va har qanday siyosat o‘tish tugmalarini (masalan, `offline_queue_enabled`) o‘z ichiga oladi.  
  Rad etishlar bir xil sxemani bo'sh `proof` bilan aks ettiradi, ammo audit uchun `sid` ni yozib oladi.

## SDK jabhalari

| SDK | Taklif etilgan API | Eslatmalar |
|-----|--------------|-------|
| Swift | `ConnectClient`, `ConnectSession`, `ConnectRequest`, `ConnectApproval` | To'ldirgichlarni yozilgan o'ramlar + sinxron oqimlar bilan almashtiring. |
| Android | Kotlin coroutines + ramkalar uchun muhrlangan sinflar | Portativlik uchun Swift tuzilishi bilan tekislang. |
| JS | Async iteratorlar + ramka turlari uchun TypeScript raqamlari | Bundler uchun qulay SDK (brauzer/tugun) taqdim eting. |

### Umumiy xatti-harakatlar- `ConnectSession` hayot aylanishini tartibga soladi:
  1. WebSocket-ni o'rnating, qo'l siqishni bajaring.
  2. Ochiq/tasdiqlovchi kadrlarni almashish.
  3. Belgi so'rovlarini/javoblarini ko'rib chiqing.
  4. Ilova qatlamiga hodisalarni chiqarish.
- Yuqori darajadagi yordamchilarni taqdim eting:
  - `requestSignature(tx, metadata)`
  - `approveSession(account, permissions)`
  - `reject(reason)`
  - `cancelRequest(hash)` - hamyon tomonidan tan olingan boshqaruv ramkasini chiqaradi.
- Xatolarni qayta ishlash: Norito xato kodlarini SDK-ga xos xatolarga xaritalash; o'z ichiga oladi
  umumiy taksonomiyadan foydalangan holda UI uchun domenga xos kodlar (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, I180NI08000). Swiftning asosiy amaliyoti + telemetriya qoʻllanmasi [`connect_error_taxonomy.md`](connect_error_taxonomy.md) da ishlaydi va Android/JS pariteti uchun maʼlumotnoma hisoblanadi.
- Navbat chuqurligi uchun telemetriya ilgaklarini chiqaring, hisoblarni qayta ulash va so'rovning kechikishi (`connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`).
 
## Tartib raqamlari va oqimni boshqarish

- Har bir yo'nalishda seans ochilganda noldan boshlanadigan 64 bitli `sequence` hisoblagichi mavjud. Birgalikda yordamchi turlari qisqichlarni qisadi va hisoblagich o'ralishidan ancha oldin `ConnectError.sequenceOverflow` + tugmani aylantirish bilan qo'l siqishni ishga tushiradi.
- Nonces va tegishli ma'lumotlar ketma-ketlik raqamiga ishora qiladi, shuning uchun dublikatlarni foydali yuklarni tahlil qilmasdan rad etish mumkin. SDK'lar `{sid, dir, seq, payload_hash}` ni o'z jurnallarida saqlashi kerak, bu qayta ulanishlar bo'ylab detuplikatsiyani aniqlaydi.
- Hamyonlar mantiqiy oyna orqali orqa bosimni reklama qiladi (`FlowControl` boshqaruv ramkalari). DApps faqat oyna tokeni mavjud bo'lganda navbatdan chiqariladi; hamyonlar quvurlarni chegaralangan holda saqlash uchun shifrlangan matnni qayta ishlagandan keyin yangi tokenlarni chiqaradi.
- Muzokaralarni davom ettirish aniq: har ikki tomon qayta ulangandan so'ng `Control::Resume { seq_app_max, seq_wallet_max, queue_depths }` chiqaradi, shunda kuzatuvchilar qancha ma'lumot qayta yuborilganligini va jurnallarda bo'shliqlar mavjudligini tekshirishlari mumkin.
- Mojarolar (masalan, bir xil `(sid, dir, seq)`, lekin turli xil xeshlarga ega ikkita foydali yuk) `ConnectError.Internal` darajasiga ko'tariladi va jim ajralishning oldini olish uchun yangi `sid` ni majburlaydi.

## Tahdid modeli va ma'lumotlarni saqlash moslashuvi- **Ko'rib chiqilgan sirtlar:** WebSocket transporti, Norito ko'prik kodlash/dekodlash,
  jurnal qat'iyligi, telemetriya eksportchilari va ilovalarga qaragan qayta qo'ng'iroqlar.
- **Asosiy maqsadlar:** sessiya sirlarini himoya qilish (X25519 kalitlari, olingan AEAD kalitlari,
  nonce/sequence counters) jurnallar/telemetriyadagi oqishlardan, qayta o'ynashni oldini olish va
  pasaytirish hujumlari, jurnallar va anomaliya hisobotlarini majburiy saqlash.
- **Kodifikatsiyalangan yumshatish choralari:**
  - Jurnallar faqat shifrlangan matndan iborat; saqlangan metadata xeshlar, uzunlik bilan cheklangan
    maydonlar, vaqt belgilari va tartib raqamlari.
  - Telemetriya foydali yuklari har qanday sarlavha/foydali kontentni o'zgartiradi va faqat o'z ichiga oladi
    `sid` plyus agregat hisoblagichlarning tuzlangan xeshlari; tahrirlash roʻyxati ulashildi
    audit pariteti uchun SDKlar o'rtasida.
  - Sessiya jurnallari sukut bo'yicha 7 kundan keyin aylantiriladi va eskiradi. Hamyonlar ochiladi
    `connectLogRetentionDays` tugmasi (SDK standart 7) va xatti-harakatni hujjatlashtiring
    shuning uchun tartibga solinadigan joylashtirishlar qattiqroq oynalarni o'rnatishi mumkin.
  - Bridge API-dan noto'g'ri foydalanish (bog'lanishlar etishmayotgan, shifrlangan matn, noto'g'ri ketma-ketlik)
    xom yuklarni yoki kalitlarni aks ettirmasdan yozilgan xatolarni qaytaradi.

Tekshiruvdan kutilayotgan savollar `docs/source/sdk/swift/connect_workshop.md` da kuzatiladi
va kengash bayonnomasida hal qilinadi; bir marta yopilgan somonchi bo'ladi
loyihadan qabul qilingangacha ko'tariladi.

## Oflayn buferlash va qayta ulanishlar

### Jurnal shartnomasi

Har bir SDK har bir seansda faqat qo'shimchalar jurnalini yuritadi, shuning uchun dApp va hamyon
oflayn holatda freymlarni navbatga qo‘yishi, ma’lumotlarni yo‘qotmasdan davom ettirishi va dalillar keltirishi mumkin
telemetriya uchun. Shartnoma Norito ko'prik turlarini aks ettiradi, shuning uchun bir xil bayt
vakillik mobil/JS steklarida saqlanib qoladi.- Jurnallar xeshlangan sessiya identifikatori (`sha256(sid)`) ostida yashaydi, ikkitasini chiqaradi
  har bir sessiya uchun fayllar: `app_to_wallet.queue` va `wallet_to_app.queue`. Swift foydalanadi
  sandboxed fayl o'rami, Android fayllarni `Room`/`FileChannel` orqali saqlaydi,
  va JS IndexedDB ga yozadi; barcha formatlar ikkilik va endian-barqaror.
- Har bir yozuv `ConnectJournalRecordV1` sifatida seriyalanadi:
  - `direction: u8` (`0 = app→wallet`, `1 = wallet→app`)
  - `sequence: u64`
  - `payload_hash: [u8; 32]` (Blake3 shifrlangan matn + sarlavhalar)
  - `ciphertext_len: u32`
  - `received_at_ms: u64`
  - `expires_at_ms: u64`
  - `ciphertext: [u8; ciphertext_len]` (aniq Norito ramkasi allaqachon AEAD bilan o'ralgan)
- Jurnallar shifrlangan matnni so'zma-so'z saqlaydi. Biz hech qachon foydali yukni qayta shifrlaymiz; AEAD
  sarlavhalar allaqachon yo'nalish tugmachalarini autentifikatsiya qiladi, shuning uchun qat'iylik kamayadi
  qo'shilgan yozuvni sinxronlash.
- Xotiradagi `ConnectQueueState` strukturasi fayl metama'lumotlarini aks ettiradi (chuqurlik,
  bayt ishlatilgan, eng qadimgi/yangi ketma-ketlik). U telemetriya eksport qiluvchilarni va
  `FlowControl` yordamchi.
- Jurnallar hajmi sukut bo'yicha 32 kvadrat / 1MiB; qalpoqchani urib chiqarib yuboradi
  eng qadimgi yozuvlar (`reason=overflow`). `ConnectFeatureConfig.max_queue_len`
  har bir joylashtirish uchun ushbu standart sozlamalarni bekor qiladi.
- Jurnallar ma'lumotlarni 24 soat davomida saqlaydi (`expires_at_ms`). Fon GC eskirganini olib tashlaydi
  segmentlarni ishtiyoq bilan ajratadi, shuning uchun diskdagi iz chegaralangan bo'lib qoladi.
- Avariya xavfsizligi: xabar berishdan oldin xotira oynasini qo'shing, fsync va yangilang
  qo'ng'iroq qiluvchi. Ishga tushganda, SDK katalogni skanerlaydi, nazorat summalarini qayd qiladi,
  va `ConnectQueueState` ni qayta tiklang. Korruptsiya huquqbuzarlik rekordini keltirib chiqaradi
  o'tkazib yuborilgan, telemetriya orqali belgilab qo'yilgan va ixtiyoriy ravishda qo'llab-quvvatlash chiqindilari uchun karantinga qo'yilgan.
- Chunki shifrlangan matn allaqachon Norito maxfiylik konvertini qondiradi, yagona
  qayd etilgan qo'shimcha metama'lumotlar xeshlangan sessiya identifikatoridir. Qo'shimcha talab qiladigan ilovalar
  maxfiylik jurnallarni saqlaydigan `telemetry_opt_in = false` ni tanlashi mumkin, ammo
  navbat chuqurligi eksportini o'zgartiradi va jurnallarda xeshlangan `sid` almashishni o'chiradi.
- SDKlar `ConnectQueueObserver` ni ochib beradi, shuning uchun hamyonlar/dApps navbat chuqurligini tekshirishi mumkin,
  drenajlar va GC natijalari; bu ilgak UI statuslarini jurnallarni tahlil qilmasdan beradi.

### Semantikani takrorlang va davom ettiring

1. Qayta ulanganda, SDK `{seq_app_max bilan `Control::Resume` chiqaradi,
   seq_wallet_max, queued_app, queued_wallet, journal_hash}`. Xesh - bu
   Faqat qo'shimchalar uchun jurnalning Bleyk3 dayjesti, shuning uchun mos kelmaydigan tengdoshlar driftni aniqlay oladi.
2. Qabul qiluvchi tengdosh rezyume yukini uning holati, so'rovlari bilan taqqoslaydi
   bo'shliqlar mavjud bo'lganda qayta uzatish va takrorlangan kadrlarni orqali tan oladi
   `Control::ResumeAck`.
3. Qayta tinglangan kadrlar har doim kiritish tartibiga rioya qiladi (`sequence` keyin yozish vaqti).
   Wallet SDK’lari `FlowControl` tokenlarini chiqarish orqali teskari bosimni qo‘llashi SHART (shuningdek,
   jurnali) shuning uchun dApps oflayn rejimda navbatni to'ldira olmaydi.
4. Jurnallar shifrlangan matnni so'zma-so'z saqlaydi, shuning uchun takrorlash oddiygina yozilgan baytlarni pompalaydi.
   transport va dekoder orqali orqaga. Har bir SDK uchun qayta kodlashga ruxsat berilmaydi.

### Qayta ulanish oqimi1. Transport WebSocket-ni qayta tiklaydi va yangi ping oralig'i bo'yicha muzokaralar olib boradi.
2. dApp hamyonning orqa bosimiga rioya qilgan holda navbatda turgan kadrlarni tartibda takrorlaydi
   (`ConnectSession.nextControlFrame()` `FlowControl` tokenlarini beradi).
3. Hamyon buferlangan natijalarni parolini hal qiladi, ketma-ketlik monotonligini tekshiradi va
   tasdiqlar/natijalar kutilayotgan takrorlanadi.
4. Ikkala tomon ham `seq_app_max`, `seq_wallet_max`, `seq_app_max`,
   va telemetriya uchun navbat chuqurligi.
5. Ikki nusxadagi ramkalar (mos keladigan `sequence` + `payload_hash`) tan olinadi va o'chiriladi; ziddiyatlar `ConnectError.Internal` ni ko'taradi va seansni majburiy qayta ishga tushirishni boshlaydi.

### Muvaffaqiyatsizlik rejimlari

- Agar sessiya eskirgan deb hisoblansa (`offline_timeout_ms`, standart 5 daqiqa),
  buferlangan ramkalar tozalanadi va SDK `ConnectError.sessionExpired` ni ko'taradi.
- Jurnal buzilgan taqdirda, SDKlar bitta Norito dekodini tuzatishga harakat qiladi; yoqilgan
  muvaffaqiyatsizlik ular jurnalni tashlab, `connect.queue_repair_failed` telemetriyasini chiqaradi.
- Ketma-ketlik mos kelmasligi `ConnectError.replayDetected` ni ishga tushiradi va yangisini majburlaydi
  qo'l siqish (sessiyani yangi `sid` bilan qayta boshlash).

### Oflayn buferlash rejasi va operator boshqaruvlari

Seminarning topshirilishi hujjatlashtirilgan rejani talab qiladi, shuning uchun har bir SDK bir xil yuboriladi
oflayn xatti-harakatlar, tuzatish oqimi va dalillar yuzasi. Quyidagi reja
Swift (`ConnectSessionDiagnostics`), Androidda keng tarqalgan
(`ConnectDiagnosticsSnapshot`) va JS (`ConnectQueueInspector`).

| Davlat | Trigger | Avtomatik javob | Qo'lda bekor qilish | Telemetriya bayrog'i |
|-------|---------|-------------------|-----------------|----------------|
| `Healthy` | Navbatdan foydalanish  5/min | Yangi belgi so'rovlarini to'xtating, oqimni boshqarish tokenlarini yarim tezlikda chiqaring | Ilovalar `clearOfflineQueue(.app|.wallet)` deb nomlanishi mumkin; SDK re-hydrates holatini tengdoshidan bir marta onlayn | `connect.queue_state=\"throttled\"`, `connect.queue_watermark` o'lchagich |
| `Quarantined` | Foydalanish ≥ `disk_watermark_drop` (standart 85%), buzilish ikki marta aniqlandi yoki `offline_timeout_ms` oshib ketdi | Buferlashni to'xtating, `ConnectError.QueueQuarantined` ko'taring, operator tasdiqini talab qiling | `ConnectSessionDiagnostics.forceReset()` | to'plamini eksport qilgandan keyin jurnallarni o'chiradi `connect.queue_state=\"quarantined\"`, `connect.queue_quarantine_total` hisoblagich |- Eshiklar `ConnectFeatureConfig` da yashaydi (`disk_watermark_warn`,
  `disk_watermark_drop`, `max_disk_bytes`, `offline_timeout_ms`). Uy egasi bo'lganda
  qiymatni o'tkazib yuborsa, SDK'lar sukut bo'yicha qaytadi va konfiguratsiyalar uchun ogohlantirishni qayd qiladi
  telemetriyadan tekshirish mumkin.
- SDKlar `ConnectQueueObserver` va diagnostika yordamchilarini ko'rsatadi:
  - Swift: `ConnectSessionDiagnostics.snapshot()` `{holat, chuqurlik, baytlar,
    sabab}` and `exportJournalBundle(url:)` qoʻllab-quvvatlash uchun ikkala navbatda ham davom etadi.
  - Android: `ConnectDiagnostics.snapshot()` + `exportJournalBundle(path)`.
  - JS: `ConnectQueueInspector.read()` bir xil struktura va blob tutqichini qaytaradi
    bu UI kodi Torii qo'llab-quvvatlash vositalariga yuklanishi mumkin.
- Ilova `offline_queue_enabled=false` ni o'zgartirganda, SDK darhol o'chib ketadi va
  ikkala jurnalni tozalang, holatni `Disabled` deb belgilang va terminalni chiqaring
  telemetriya hodisasi. Foydalanuvchiga qaragan afzallik Norito da aks ettirilgan
  tasdiqlash ramkasi, shuning uchun tengdoshlar buferlangan freymlarni davom ettirish mumkinligini bilishadi.
- Operatorlar `connect queue inspect --sid <sid>` (SDK atrofidagi CLI paketi) ni ishga tushiradilar.
  diagnostika) xaos testlari paytida; bu buyruq holat o'tishlarini chop etadi,
  suv belgisi tarixi va dalillarni davom ettiring, shuning uchun boshqaruv sharhlari bunga bog'liq emas
  platformaga xos asboblar.

### Dalillar to'plamining ish jarayoni

Yordam va muvofiqlik guruhlari audit paytida deterministik dalillarga tayanadi
oflayn xatti-harakatlar. Shuning uchun har bir SDK bir xil uch bosqichli eksportni amalga oshiradi:

1. `exportJournalBundle(..)` yozadi `{app_to_wallet,wallet_to_app}.queue` plus a
   qurilish xeshini, xususiyat bayroqlarini va diskdagi suv belgilarini tavsiflovchi manifest.
2. `exportQueueMetrics(..)` oxirgi 1000 ta telemetriya namunalarini chiqaradi, shuning uchun asboblar paneli
   oflayn rejimda qayta qurish mumkin. Namunalar xeshlangan sessiya identifikatorini o'z ichiga oladi
   foydalanuvchi ro'yxatdan o'tdi.
3. CLI yordamchisi ikkala eksportni ham ziplaydi va imzolangan Norito metamaʼlumotlar faylini biriktiradi.
   (`ConnectQueueEvidenceV1`) shuning uchun Torii qabul qilish paketni SoraFS da arxivlashi mumkin.

Tekshirish muvaffaqiyatsiz bo'lgan to'plamlar `connect.evidence_invalid` bilan rad etiladi
telemetriya, shuning uchun SDK jamoasi eksportchini qayta ishlab chiqarishi va tuzatishi mumkin.

## Telemetriya va diagnostika- Umumiy OpenTelemetry eksportchilari orqali Norito JSON hodisalarini emitent. Majburiy ko'rsatkichlar:
  - `ConnectQueueState` tomonidan oziqlangan `connect.queue_depth{direction}` (o'lchagich).
  - `connect.queue_bytes{direction}` (o'lchagich) diskda qo'llab-quvvatlanadigan iz uchun.
  - `overflow|ttl|repair` uchun `connect.queue_dropped_total{reason}` (hisoblagich).
  - `connect.offline_flush_total{direction}` (hisoblagich) navbatda turganda oshadi
    transportsiz drenajlash; nosozliklar ortishi `connect.offline_flush_failed`.
  - `connect.replay_success_total`/`connect.replay_error_total`.
  - `connect.resume_latency_ms` gistogrammasi (qayta ulanish va barqaror
    davlat) plus `connect.resume_attempts_total`.
  - `connect.session_duration_ms` gistogrammasi (tugallangan sessiya uchun).
  - `connect.error` tuzilgan voqealar `code`, `fatal`, `telemetry_profile`.
- Eksportchilar `{platform, sdk_version, feature_hash}` yorliqlarini shunday qilib qo'yishlari SHART
  asboblar paneli SDK tuzilishi bo'yicha bo'linishi mumkin. Xeshlangan `sid` ixtiyoriy va faqat
  telemetriyaga kirish rost bo'lganda chiqariladi.
- SDK darajasidagi ilgaklar bir xil hodisalarni yuzaga keltiradi, shuning uchun ilovalar batafsil ma'lumotlarni eksport qilishi mumkin:
  - Tezkor: `ConnectSession.addObserver(_:) -> ConnectEvent`.
  - Android: `Flow<ConnectEvent>`.
  - JS: asinxron iterator yoki qayta qo'ng'iroq qilish.
- CI gating: Swift vazifalari `make swift-ci` bilan ishlaydi, Android `./gradlew sdkConnectCi` dan foydalanadi,
  va JS `npm run test:connect` ishlaydi, shuning uchun telemetriya/boshqaruv paneli oldin yashil bo'lib qoladi.
  birlashtirish Connect o'zgarishlar.
- Strukturaviy jurnallarga xeshlangan `sid`, `seq`, `queue_depth` va `sid_epoch` kiradi.
  operatorlar mijozlar muammolarini o'zaro bog'lashlari uchun qiymatlar. Ta'mirlash muvaffaqiyatsiz bo'lgan jurnallar chiqaradi
  `connect.queue_repair_failed{reason}` hodisalari, shuningdek, ixtiyoriy halokat yoʻli.

### Telemetriya ilgaklari va boshqaruv dalillari

- `connect.queue_state` yo'l xaritasi xavf ko'rsatkichi sifatida ikki barobar. Boshqaruv paneli guruhi
  `{platform, sdk_version}` tomonidan va shtatdagi vaqtni ko'rsating, shunda boshqaruv namuna olishi mumkin
  bosqichli ishlab chiqarishni tasdiqlashdan oldin oylik burg'ulash dalillari.
- `connect.queue_watermark` va `connect.queue_bytes` Connect xavf reytingini beradi
  (`risk.connect.offline_buffer`), SRE dan ortiq bo'lganda avtomatik ravishda sahifalarni ochadi
  Seanslarning 5% `Throttled` da >10 daqiqa vaqt sarflaydi.
- Eksportchilar `feature_hash` ni har bir hodisaga biriktiradilar, shunda auditor asboblari tasdiqlaydi.
  Norito kodek + oflayn rejasi ko'rib chiqilgan tuzilishga mos keladi. SDK CI muvaffaqiyatsiz tugadi
  telemetriya noma'lum xesh haqida xabar berganida tez.
- Somonchi hali ham tahdid modeli qo'shimchasini talab qiladi; ko'rsatkichlar oshib ketganda
  siyosat chegaralari, SDKlar `connect.policy_violation` hodisalarini chiqaradi
  qoidabuzar sid (xeshlangan), holat va hal qilingan harakat (`drain|purge|quarantine`).
- `exportQueueMetrics` orqali olingan dalillar bir xil SoraFS nom maydoniga tushadi
  Connect runbook artefaktlari sifatida kengash sharhlovchilari har bir mashqni kuzatishi mumkin
  ichki jurnallarni talab qilmasdan muayyan telemetriya namunalariga qaytish.

## Ramkaga egalik va mas'uliyat| Ramka / Boshqarish | Egasi | Domen ketma-ketligi | Jurnal davom etdimi? | Telemetriya belgilari | Eslatmalar |
|----------------|-------|-----------------|-------------------|------------------|-------|
| `Control::Open` | dApp | `seq_app` | ✅ (`app_to_wallet`) | `event=open` | Metadata + ruxsat bitmapini olib yuradi; hamyonlar so'rovlar oldidan so'nggi ochilganni takrorlaydi. |
| `Control::Approve` | Hamyon | `seq_wallet` | ✅ (`wallet_to_app`) | `event=approve` | Hisob, dalillar, imzolarni o'z ichiga oladi. Bu yerda meta-maʼlumotlar versiyasi oʻsishi qayd etilgan. |
| `Control::Reject` | Hamyon | `seq_wallet` | ✅ | `event=reject`, `reason` | Ixtiyoriy mahalliylashtirilgan xabar; dApp kutilayotgan belgi so'rovlarini o'chirib tashlaydi. |
| `Control::Close` (init) | dApp | `seq_app` | ✅ | `event=close`, `initiator=app` | Hamyon o'zining `Close` bilan tan oladi. |
| `Control::Close` (ack) | Hamyon | `seq_wallet` | ✅ | `event=close`, `initiator=wallet` | Yirtilishni tasdiqlaydi; GC ikkala tomon ham freymni saqlab qolganda jurnallarni olib tashlaydi. |
| `SignRequest` | dApp | `seq_app` | ✅ | `event=sign_request`, `payload_hash` | Takroriy nizolarni aniqlash uchun foydali yuk xeshi qayd etildi. |
| `SignResult` | Hamyon | `seq_wallet` | ✅ | `event=sign_result`, `status=ok|err` | Imzolangan baytlarning BLAKE3 xeshini o'z ichiga oladi; muvaffaqiyatsizliklar `ConnectError.Signing` ni oshiradi. |
| `Control::Error` | Hamyon (ko'pchilik) / dApp (transport) | mos keladigan egasi domeni | ✅ | `event=error`, `code` | Fatal xatolar seansni qayta boshlashga majbur qiladi; telemetriya belgilari `fatal=true`. |
| `Control::RotateKeys` | Hamyon | `seq_wallet` | ✅ | `event=rotate_keys`, `reason` | Yangi yo'nalish tugmachalarini e'lon qiladi; dApp `RotateKeysAck` (ilova tomonida jurnal) bilan javob beradi. |
| `Control::Resume` / `ResumeAck` | Ikkala | faqat mahalliy domen | ✅ | `event=resume`, `direction=app|wallet` | Navbat chuqurligi + ketma-ketlik holatini umumlashtiradi; xashed jurnali dayjest tashxisga yordam beradi. |

- Yo'nalish shifrlash kalitlari har bir rol uchun simmetrik bo'lib qoladi (`app→wallet`, `wallet→app`).
  Hamyonni aylantirish takliflari `Control::RotateKeys` va dApps orqali e'lon qilinadi.
  `Control::RotateKeysAck` chiqarish orqali tan oling; ikkala ramka ham diskka tegishi kerak
  Qayta o'ynash bo'shliqlarini oldini olish uchun tugmalarni almashtirishdan oldin.
- Metadata ilovasi (piktogrammalar, mahalliylashtirilgan nomlar, muvofiqlik dalillari) tomonidan imzolangan
  hamyon va dApp tomonidan keshlangan; yangilanishlar yangi tasdiqlash ramkasini talab qiladi
  oshirilgan `metadata_version`.
- Yuqoridagi egalik matritsasi SDK hujjatlaridan olingan, shuning uchun CLI/web/automation
  mijozlar bir xil shartnoma va asboblar standartlariga rioya qilishadi.

## Ochiq savollar

1. **Session kashfiyoti**: WalletConnect kabi QR kodlari / tarmoqdan tashqari qo‘l siqish kerakmi? (Kelajakdagi ish.)
2. **Multisig**: Ko‘p belgilarni tasdiqlash qanday ifodalanadi? (Bir nechta imzoni qo'llab-quvvatlash uchun belgi natijasini kengaytiring.)
3. **Muvofiqlik**: Qaysi maydonlar tartibga solinadigan oqimlar uchun majburiydir (har bir yo‘l xaritasi bo‘yicha)? (Muvofiqlik bo'yicha guruh ko'rsatmalarini kuting.)
4. **SDK packaging**: Umumiy kodni (masalan, Norito Connect kodeklari) platformalararo qutiga kiritishimiz kerakmi? (TBD.)

## Keyingi qadamlar- Bu somonni SDK kengashiga yuboring (2026 yil fevral yig'ilishi).
- Ochiq savollar bo'yicha fikr-mulohazalarni to'plang va shunga mos ravishda hujjatni yangilang.
- SDK bo'yicha amalga oshirishni rejalashtirish (Swift IOS7, Android AND7, JS Connect bosqichlari).
- Yo'l xaritasi issiq ro'yxati orqali taraqqiyotni kuzatib boring; strawman ratifikatsiya qilingandan so'ng `status.md` yangilang.