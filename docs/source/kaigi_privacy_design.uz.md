---
lang: uz
direction: ltr
source: docs/source/kaigi_privacy_design.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6b7ffca7e960376a2959357cd865d8dab5afa1dfcb959adbc688b6db60977c8f
source_last_modified: "2026-01-05T09:28:12.022066+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kaigi Maxfiylik va Relay dizayni

Ushbu hujjat nol bilimni joriy qiluvchi maxfiylikka qaratilgan evolyutsiyani qamrab oladi
ishtirok dalillar va determinizm yoki qurbon qilmasdan piyoz uslubidagi o'rni
buxgalteriya hisobining tekshirilishi.

# Umumiy ko'rinish

Dizayn uchta qatlamni o'z ichiga oladi:

- **Roster maxfiyligi** – xost ruxsatnomalari va hisob-kitoblarni izchil saqlagan holda zanjirdagi ishtirokchilarning identifikatorlarini yashirish.
- **Foydalanish shaffofligi** – xostlarga har bir segment tafsilotlarini oshkor qilmasdan o‘lchovli foydalanishni qayd qilish imkonini beradi.
- **Overlay releys** – tarmoq kuzatuvchilari qaysi ishtirokchilar muloqot qilayotganini bilib ololmasligi uchun ko‘p tarmoqli tengdoshlar orqali paketlarni tashish.

Barcha qo'shimchalar birinchi navbatda Norito bo'lib qoladi, ABI 1-versiyasi ostida ishlaydi va heterojen qurilmalarda deterministik tarzda bajarilishi kerak.

#gol

1. Ishtirokchilarni nol ma'lumotga ega bo'lgan dalillardan foydalangan holda qabul qiling / chiqarib yuboring, shuning uchun buxgalteriya hisobi hech qachon xom hisob identifikatorlarini ko'rsatmaydi.
2. Kuchli buxgalteriya kafolatlarini saqlang: har bir qo'shilish, ketish va foydalanish hodisasi hali ham deterministik tarzda mos kelishi kerak.
3. Nazorat/ma'lumotlar kanallari uchun piyoz marshrutlarini tavsiflovchi va zanjirda tekshirilishi mumkin bo'lgan ixtiyoriy reley manifestlarini taqdim eting.
4. Maxfiylikni talab qilmaydigan joylashtirishlar uchun zaxira (to'liq shaffof ro'yxat) ishlayotgan bo'lsin.

# Tahdid modelining xulosasi

- **Dushmanlar:** Tarmoq kuzatuvchilari (ISP), qiziq tekshiruvchilar, zararli relay operatorlari va yarim halol xostlar.
- **Himoyalangan aktivlar:** Ishtirokchi identifikatori, ishtirok etish vaqti, har bir segmentdan foydalanish/hisob-kitob tafsilotlari va tarmoq marshrutlash metamaʼlumotlari.
- **Taxminlar:** Xostlar hali ham zanjirdan tashqari haqiqiy ishtirokchi to'plamini o'rganadilar; daftar tengdoshlari dalillarni aniq tekshiradi; ustki o'rni ishonchsiz, lekin tezligi cheklangan; HPKE va SNARK primitivlari kodlar bazasida allaqachon mavjud.

# Ma'lumotlar modelidagi o'zgarishlar

Barcha turlar `iroha_data_model::kaigi` da yashaydi.

```rust
/// Commitment to a participant identity (Poseidon hash of account + domain salt).
pub struct KaigiParticipantCommitment {
    pub commitment: FixedBinary<32>,
    pub alias_tag: Option<String>,
}

/// Nullifier unique to each join action, prevents double-use of proofs.
pub struct KaigiParticipantNullifier {
    pub digest: FixedBinary<32>,
    pub issued_at_ms: u64,
}

/// Relay path description used by clients to set up onion routing.
pub struct KaigiRelayManifest {
    pub hops: Vec<KaigiRelayHop>,
    pub expiry_ms: u64,
}

pub struct KaigiRelayHop {
    pub relay_id: AccountId,
    pub hpke_public_key: FixedBinary<32>,
    pub weight: u8,
}
```

`KaigiRecord` quyidagi maydonlarni oladi:

- `roster_commitments: Vec<KaigiParticipantCommitment>` - maxfiylik rejimi yoqilgandan so'ng ochiq `participants` ro'yxatini almashtiradi. Klassik o'rnatishlar migratsiya paytida ikkalasini ham to'ldirishi mumkin.
- `nullifier_log: Vec<KaigiParticipantNullifier>` – faqat qoʻshish uchun moʻljallangan, metamaʼlumotlarni chegaralangan holda saqlash uchun aylanma oyna bilan yopilgan.
- `room_policy: KaigiRoomPolicy` – seans uchun tomoshabinning autentifikatsiya pozitsiyasini tanlaydi (`Public` xonalari faqat o‘qish uchun releni aks ettiradi; `Authenticated` xonalari paketlarni yuborishdan oldin tomoshabin chiptalarini talab qiladi).
- `relay_manifest: Option<KaigiRelayManifest>` - Norito bilan kodlangan tuzilgan manifest, shuning uchun hops, HPKE kalitlari va og'irliklari JSON shimlarisiz kanonik bo'lib qoladi.
- `privacy_mode: KaigiPrivacyMode` enum (pastga qarang).

```rust
pub enum KaigiPrivacyMode {
    Transparent,
    ZkRosterV1,
}
```

`NewKaigi` mos keladigan ixtiyoriy maydonlarni oladi, shuning uchun xostlar yaratilish vaqtida maxfiylikni tanlay oladi.


- Maydonlar kanonik kodlashni amalga oshirish uchun `#[norito(with = "...")]` yordamchilaridan foydalanadi (butun sonlar uchun kichik endian, joylashuv bo'yicha saralangan hops).
- `KaigiRecord::from_new` yangi vektorlarni bo'shatadi va taqdim etilgan reley manifestini nusxalaydi.

# Yo'riqnoma sirtini o'zgartirish

## Demo tez ishga tushirish yordamchisi

Ad-hoc demolar va birgalikda ishlash testlari uchun CLI endi ochiladi
`iroha kaigi quickstart`. Bu:- `--domain`/`--host` orqali bekor qilinmasa, CLI konfiguratsiyasidan (`wonderland` domen + hisob) qayta foydalanadi.
- `--call-name` o'tkazib yuborilganda vaqt tamg'asiga asoslangan qo'ng'iroq nomini yaratadi va faol Torii so'nggi nuqtasiga qarshi `CreateKaigi` yuboradi.
- Ixtiyoriy ravishda avtomatik ravishda xostga qo'shiladi (`--auto-join-host`), shunda tomoshabinlar darhol ulanishi mumkin.
- Torii URL manzili, qoʻngʻiroq identifikatorlari, maxfiylik/xona siyosati, nusxa koʻchirishga tayyor qoʻshilish buyrugʻi va spool yoʻlini sinovchilar kuzatishi kerak boʻlgan JSON xulosasini chiqaradi (masalan, `storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/*.norito`). Blobni davom ettirish uchun `--summary-out path/to/file.json` dan foydalaning.

Ushbu yordamchi ishlayotgan `irohad --sora` tuguniga bo'lgan ehtiyojni **o'zgartirmaydi**: maxfiylik marshrutlari, spool fayllari va relay manifestlari buxgalteriya hisobi bilan ta'minlangan bo'lib qoladi. Tashqi partiyalar uchun vaqtinchalik xonalarni yig'ishda u oddiygina qozonni kesadi.

### Bir buyruqli demo skript

Tezroq yo'l uchun qo'shimcha skript mavjud: `scripts/kaigi_demo.sh`.
U siz uchun quyidagilarni bajaradi:

1. To'plamdagi `defaults/nexus/genesis.json` ni `target/kaigi-demo/genesis.nrt` ga imzolaydi.
2. Imzolangan blok bilan `irohad --sora` ni ishga tushiradi (`target/kaigi-demo/irohad.log` ostidagi jurnallar) va Torii `http://127.0.0.1:8080/status`ni ochishini kutadi.
3. `iroha kaigi quickstart --auto-join-host --summary-out target/kaigi-demo/kaigi_summary.json` ishlaydi.
4. JSON xulosasiga yo'lni va spool katalogini (`storage/streaming/soranet_routes/exit-<relay-id>/kaigi-stream/`) chop etadi, shuning uchun uni tashqi testerlar bilan baham ko'rishingiz mumkin.

Atrof-muhit o'zgaruvchilari:

- `TORII_URL` - so'rov uchun Torii so'nggi nuqtasini bekor qilish (standart `http://127.0.0.1:8080`).
- `RUN_DIR` — ishchi katalogni bekor qilish (standart `target/kaigi-demo`).

`Ctrl+C` tugmasini bosib namoyishni to'xtating; skriptdagi tuzoq `irohad` ni avtomatik ravishda tugatadi. Spool fayllari va xulosa diskda qoladi, shuning uchun jarayon tugagandan so'ng artefaktlarni topshirishingiz mumkin.

## `CreateKaigi`

- `privacy_mode`ni xost ruxsatnomalariga qarshi tasdiqlaydi.
- Agar `relay_manifest` taʼminlangan boʻlsa, zanjirdagi manifestlar tekshirilishi mumkin boʻlib qolishi uchun ≥3 hops, noldan farqli ogʻirliklar, HPKE kaliti mavjudligi va oʻziga xoslikni taʼminlang.
- SDK/CLI (`public` va `authenticated`) dan `room_policy` ma'lumotlarini tasdiqlang va uni SoraNet ta'minotiga tarqating, shunda rele keshlari to'g'ri GAR toifalarini (`stream.kaigi.public`00X00) ochib beradi. Xostlar buni `iroha kaigi create --room-policy …`, JS SDK ning `roomPolicy` maydoni orqali yoki Swift mijozlari taqdim etishdan oldin Norito foydali yukini yig‘ganda `room_policy` o‘rnatish orqali ulanadi.
- Bo'sh majburiyat / bekor qiluvchi jurnallarni saqlaydi.

## `JoinKaigi`

Parametrlar:

- `proof: ZkProof` (Norito bayt o'rami) – Groth16 dalili qo'ng'iroq qiluvchi `(account_id, domain_salt)` ni bilishini tasdiqlovchi Poseidon xeshi taqdim etilgan `commitment` ga teng.
- `commitment: FixedBinary<32>`
- `nullifier: FixedBinary<32>`
- `relay_hint: Option<KaigiRelayHop>` – keyingi hop uchun har bir ishtirokchi uchun ixtiyoriy bekor qilish.

Amalga oshirish bosqichlari:

1. `record.privacy_mode == Transparent` bo'lsa, joriy xatti-harakatlarga qayting.
2. Groth16 isbotini elektron registrdagi `KAIGI_ROSTER_V1` yozuviga qarshi tekshiring.
3. `record.nullifier_log` da `nullifier` chiqmaganligiga ishonch hosil qiling.
4. Majburiyat/nokor yozuvlarni qo'shish; agar `relay_hint` ta'minlangan bo'lsa, ushbu ishtirokchi uchun rele manifest ko'rinishini tuzating (zanjirda emas, faqat xotirada seans holatida saqlanadi).## `LeaveKaigi`

Shaffof rejim joriy mantiqqa mos keladi.

Shaxsiy rejim talab qiladi:

1. Qo'ng'iroq qiluvchi `record.roster_commitments` da majburiyatni bilishini isbotlash.
2. Bir martalik ta'tilni tasdiqlovchi bekor qiluvchi yangilanish.
3. Majburiyat/nokor yozuvlarni olib tashlang. Tekshiruv strukturaviy oqishning oldini olish uchun qattiq saqlash oynalari uchun qabr toshlarini saqlaydi.

## `RecordKaigiUsage`

Foydali yukni kengaytiradi:

- `usage_commitment: FixedBinary<32>` - xom-ashyodan foydalanish to'plamiga sodiqlik (davomiylik, gaz, segment identifikatori).
- Deltaning shifrlangan jurnallarga mos kelishini tasdiqlovchi ixtiyoriy ZK isboti.

Xostlar hali ham shaffof jamlanmalarni yuborishlari mumkin; maxfiylik rejimi faqat majburiyat maydonini majburiy qiladi.

# Tekshiruv va sxemalar

- `iroha_core::smartcontracts::isi::kaigi::privacy` endi to'liq ro'yxatni bajaradi
  sukut bo'yicha tekshirish. Bu `zk.kaigi_roster_join_vk` (qo'shilish) va
  `zk.kaigi_roster_leave_vk` (barglar) konfiguratsiyadan,
  WSV da mos keladigan `VerifyingKeyRef` ni qidiradi (yozuvning mavjudligini ta'minlash
  `Active`, backend/sxema identifikatorlari mos keladi va majburiyatlar mos keladi), to'lovlar
  bayt hisobi va konfiguratsiya qilingan ZK backendiga jo'natish.
- `kaigi_privacy_mocks` xususiyati deterministik stub tekshiruvchini saqlab qoladi, shuning uchun
  birlik/integratsiya testlari va cheklangan CI ishlari Halo2 backendsiz ishlashi mumkin.
  Ishlab chiqarish tuzilmalari haqiqiy dalillarni qo'llash uchun xususiyatni o'chirib qo'yishi kerak.
- Agar `kaigi_privacy_mocks` yoqilgan bo'lsa, kassa kompilyatsiya vaqtida xatolik chiqaradi.
  sinovdan o'tmagan, `debug_assertions` tuzilmasi, tasodifiy chiqarish ikkilik fayllarining oldini olish
  stub bilan jo'natishdan.
- Operatorlar (1) boshqaruv orqali o'rnatilgan ro'yxat tekshiruvini ro'yxatdan o'tkazishlari kerak va
  (2) `zk.kaigi_roster_join_vk`, `zk.kaigi_roster_leave_vk` va
  `zk.kaigi_usage_vk` `iroha_config` da xostlar ularni ish vaqtida hal qilishlari mumkin.
  Kalitlar mavjud bo'lmaguncha, maxfiylik qo'shiladi, tark etadi va foydalanish qo'ng'iroqlari bajarilmaydi
  deterministik tarzda.
- `crates/kaigi_zk` endi ro'yxatga qo'shilish/barglar va foydalanish uchun Halo2 sxemalarini yuboradi
  qayta ishlatiladigan kompressorlar bilan bir qatorda majburiyatlar (`commitment`, `nullifier`,
  `usage`). Ro'yxat sxemalari Merkle ildizini ochib beradi (to'rtta kichik endian
  64-bitli a'zolar) qo'shimcha ommaviy kirishlar sifatida, shuning uchun xost isbotni o'zaro tekshirishi mumkin
  tekshirishdan oldin saqlangan ro'yxat ildiziga qarshi. Foydalanish majburiyatlari
  `(davomiylik, gaz,
  segment)` daftardagi xeshga.
- `Join` elektron kirishlari: `(commitment, nullifier, domain_salt)` va shaxsiy
  `(account_id)`. Umumiy maʼlumotlarga `commitment`, `nullifier` va
  Ro'yxat majburiyatlari daraxti uchun Merkle ildizining to'rtta a'zosi (ro'yxat
  zanjirdan tashqari qoladi, lekin ildiz transkriptga bog'langan).
- Determinizm: biz Poseidon parametrlarini, sxema versiyalarini va indekslarini tuzatamiz.
  ro'yxatga olish kitobi. Har qanday o'zgarish `KaigiPrivacyMode` dan `ZkRosterV2` ga mos keladi.
  testlar/oltin fayllar.

# Piyoz marshrutlash qoplamasi

## Estafetani ro'yxatdan o'tkazish- Relaylar `kaigi_relay::<relay_id>` domen metama'lumotlari yozuvlari sifatida o'z-o'zini ro'yxatdan o'tkazadi, shu jumladan HPKE kalit materiali va tarmoqli kengligi sinfi.
- `RegisterKaigiRelay` ko'rsatmasi domen metama'lumotlaridagi deskriptorni saqlab qoladi, `KaigiRelayRegistered` xulosasini chiqaradi (HPKE barmoq izi va tarmoqli kengligi klassi bilan) va kalitlarni aniq aylantirish uchun qayta chaqirilishi mumkin.
- Boshqaruv domen metamaʼlumotlari (`kaigi_relay_allowlist`) orqali ruxsat etilgan roʻyxatlarni tanlaydi va yangi yoʻllarni qabul qilishdan oldin roʻyxatdan oʻtish/manifest yangilanishlari aʼzolikni majbur qiladi.

## Manifest Yaratilish

- Xostlar mavjud relelardan ko'p martali yo'llarni (minimal uzunlik 3) quradilar. Manifest qatlamli konvertni shifrlash uchun zarur bo'lgan AccountIds va HPKE ochiq kalitlari ketma-ketligini kodlaydi.
- zanjirda saqlangan `relay_manifest` hop deskriptorlari va amal qilish muddatini o'z ichiga oladi (Norito kodli `KaigiRelayManifest`); haqiqiy efemer kalitlar va har bir sessiya uchun ofsetlar HPKE yordamida hisobdan tashqari almashiladi.

## Signal va media

- SDP/ICE almashinuvi Kaigi metama'lumotlari orqali davom etadi, lekin har bir hop uchun shifrlangan. Tekshiruvchilar faqat HPKE shifrlangan matn va sarlavha indekslarini ko'radi.
- Media paketlar muhrlangan foydali yuklarga ega QUIC yordamida o'rni orqali harakatlanadi. Har bir hop keyingi hop manzilini o'rganish uchun bitta qatlamni parolini hal qiladi; Yakuniy qabul qiluvchi barcha qatlamlarni olib tashlaganidan keyin media oqimini oladi.

## Ishdan chiqish

- Mijozlar `ReportKaigiRelayHealth` yo'riqnomasi orqali reley holatini kuzatib boradi, bu domen metama'lumotlarida (`kaigi_relay_feedback::<relay_id>`) imzolangan fikr-mulohazalarni davom ettiradi, `KaigiRelayHealthUpdated` translyatsiya qiladi va boshqaruv/xostlarga joriy mavjudligi haqida fikr yuritish imkonini beradi. O'z o'rni bajarilmasa, xost yangilangan manifest chiqaradi va `KaigiRelayManifestUpdated` hodisasini qayd qiladi (pastga qarang).
- Xostlar `SetKaigiRelayManifest` ko'rsatmasi orqali buxgalteriya kitobidagi manifest o'zgarishlarni qo'llaydi, bu saqlangan yo'lni almashtiradi yoki uni butunlay tozalaydi. Tozalash `hop_count = 0` bilan xulosa chiqaradi, shuning uchun operatorlar to'g'ridan-to'g'ri marshrutizatsiyaga qaytishni kuzatishi mumkin.
- Prometheus ko'rsatkichlari (`kaigi_relay_registered_total`, `kaigi_relay_registration_bandwidth_class`, `kaigi_relay_manifest_updates_total`, `kaigi_relay_manifest_hop_count`, `kaigi_relay_health_reports_total`, `kaigi_relay_health_reports_total`, Prometheus, Prometheus `kaigi_relay_failover_hop_count`) endi operator panellari uchun sirt releyining uzilishi, sog'liq holati va o'chirilish kadensi.

# Tadbir

`DomainEvent` variantlarini kengaytiring:

- `KaigiRosterSummary` - anonim hisoblar va joriy ro'yxat bilan chiqariladi
  ro'yxat o'zgarganda root (root shaffof rejimda `None`).
- `KaigiRelayRegistered` - reley registratsiyasi yaratilgan yoki yangilanganda chiqariladi.
- `KaigiRelayManifestUpdated` - rele manifesti o'zgarganda chiqariladi.
- `KaigiRelayHealthUpdated` - xostlar `ReportKaigiRelayHealth` orqali relay sog'ligi haqida hisobot yuborganda chiqariladi.
- `KaigiUsageSummary` - har bir foydalanish segmentidan so'ng chiqariladi, faqat yig'indini ko'rsatadi.

Voqealar Norito bilan ketma-ketlashtirilib, faqat majburiyat xeshlari va hisoblarini ochib beradi.CLI asboblari (`iroha kaigi …`) operatorlar seanslarni ro'yxatdan o'tkazishlari uchun har bir ISIni o'rab oladi,
ro'yxat yangilanishlarini yuboring, relay holati haqida xabar bering va qo'lda ishlov bermasdan foydalanishni yozib oling.
Relay manifestlari va maxfiylik dalillari oʻtkazilgan JSON/hex fayllardan yuklanadi
CLI-ning oddiy topshirish yo'li, bu skript shartnomasini osonlashtiradi
sahnalashtirilgan muhitda qabul qilish.

# Gaz hisobi

- `crates/iroha_core/src/gas.rs` da yangi konstantalar:
  - `BASE_KAIGI_JOIN_ZK`, `BASE_KAIGI_LEAVE_ZK` va `BASE_KAIGI_USAGE_ZK`
    Halo2 tekshirish vaqtlariga nisbatan kalibrlangan (ro'yxat uchun ≈1,6ms
    qo'shiladi/barglar, Apple M2 Ultra-da foydalanish uchun ≈1,2ms). Qo'shimcha to'lovlar davom etmoqda
    `PER_KAIGI_PROOF_BYTE` orqali isbot bayt hajmi bilan o'lchov.
- `RecordKaigiUsage` majburiyat hajmi va dalillarni tekshirish asosida qo'shimcha haq to'laydi.
- Kalibrlash jabduqlari konfidensial infratuzilmani belgilangan urug'lar bilan qayta ishlatadi.

# Sinov strategiyasi

- `KaigiParticipantCommitment`, `KaigiRelayManifest` uchun Norito kodlash/dekodlashni tasdiqlovchi birlik testlari.
- Kanonik tartibni ta'minlaydigan JSON ko'rinishi uchun oltin testlar.
- Mini-tarmoqni aylantiruvchi integratsiya testlari (qarang
  Joriy qamrov uchun `crates/iroha_core/tests/kaigi_privacy.rs`):
  - Soxta dalillardan foydalangan holda shaxsiy qo'shilish/chiqish tsikllari (`kaigi_privacy_mocks` xususiyat bayrog'i).
  - Metama'lumotlar hodisalari orqali tarqatiladigan relay manifest yangilanishlari.
- Xostning noto'g'ri konfiguratsiyasini qamrab oluvchi UI testlarini sinab ko'ring (masalan, maxfiylik rejimida etishmayotgan rele manifest).
- Cheklangan muhitda birlik/integratsiya testlarini o'tkazishda (masalan, Codex
  sandbox), Norito ulanishini chetlab o'tish uchun `NORITO_SKIP_BINDINGS_SYNC=1` eksport qiling
  sinxronizatsiya tekshiruvi `crates/norito/build.rs` tomonidan amalga oshiriladi.

# Migratsiya rejasi

1. ✅ `KaigiPrivacyMode::Transparent` sukut boʻyicha maʼlumotlar modeli qoʻshimchalarini yuboring.
2. ✅ Simli ikki yoʻl tekshiruvi: ishlab chiqarish `kaigi_privacy_mocks` ni oʻchiradi,
   `zk.kaigi_roster_vk` ni hal qiladi va konvertni haqiqiy tekshirishni amalga oshiradi; testlar mumkin
   deterministik stublar uchun xususiyatni hali ham yoqing.
3. ✅ Maxsus `kaigi_zk` Halo2 qutisi, kalibrlangan gaz va simli simlar taqdim etildi
   Haqiqiy dalillarni oxirigacha ishlatish uchun integratsiya qamrovi (masxarabozlar endi faqat sinovdan o'tkaziladi).
4. ⬜ Barcha iste'molchilar majburiyatlarni tushungandan so'ng, shaffof `participants` vektorini bekor qiling.

# Ochiq savollar

- Merkle daraxtining barqarorligi strategiyasini aniqlang: zanjirdagi va zanjirsiz (hozirgi moyillik: zanjirli ildiz majburiyatlari bilan zanjirdan tashqari daraxt). *(KPG-201 da kuzatilgan.)*
- Rele manifestlari ko'p yo'lni (bir vaqtning o'zida ortiqcha yo'llar) qo'llab-quvvatlashi kerakligini aniqlang. *(KPG-202 da kuzatilgan.)*
- Estafeta obro'si uchun boshqaruvni aniqlang - bizga qisqartirish kerakmi yoki shunchaki yumshoq taqiqlar? *(KPG-203 da kuzatilgan.)*

`KaigiPrivacyMode::ZkRosterV1` ni ishlab chiqarishda yoqishdan oldin bu elementlarni hal qilish kerak.