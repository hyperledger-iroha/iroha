---
lang: uz
direction: ltr
source: docs/source/ministry/policy_jury_ballots.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff3faabda5f1c277f545b7edbbc93f3b58dee65cec943cfd464a026b2984a146
source_last_modified: "2025-12-29T18:16:35.979378+00:00"
translation_last_reviewed: 2026-02-07
title: Policy Jury Sortition & Ballots
translator: machine-google-reviewed
---

“Yoʻl xaritasi” bandi **MINFO-5 — Siyosat hakamlari ovoz berish uchun asboblar toʻplami** portativ formatni talab qiladi
deterministik hakamlar hay'atini tanlash va muhrlangan majburiyat uchun → byulletenlarni ko'rsatish.  The
`iroha_data_model::ministry::jury` moduli endi uchta Norito foydali yuklarni yuboradi
butun ovoz berish jarayonini qamrab oladi:

1. **`PolicyJurySortitionV1`** – oʻyin metamaʼlumotlarini qayd etadi (taklif identifikatori,
   davra identifikatori, shaxsni tasdiqlovchi snapshot dayjesti, tasodifiy mayoq),
   qo'mita hajmi, tanlangan hakamlar hay'ati va avtomatik uchun ishlatiladigan kutish ro'yxati
   uzilish.  Har bir asosiy uyaga `PolicyJuryFailoverPlan` kirishi mumkin
   kutish ro'yxati darajasiga ishora qilib, uning inoyatidan keyin ko'tarilishi kerak
   davr uzilishlari.  Tuzilma qasddan deterministik, shuning uchun auditorlar
   durangni takrorlashi va bir xil POP dan manifestni qayta tiklashi mumkin
   surat + mayoq.
2. **`PolicyJuryBallotCommitV1`** – ovoz berishdan oldin yozilgan muhrlangan majburiyat
   oshkor qilinadi.  U davra/taklif/hakamlar identifikatorlarini saqlaydi
   Blake2b‑256 hakamlar a'zosi identifikatori dayjesti + ovoz berish tanlovi + noaniq kortej, suratga olish
   vaqt tamg'asi va ovoz berish rejimi (`plaintext` yoki `zk-envelope`
   `zk-ballot` xususiyati faol).  `PolicyJuryBallotCommitV1::verify_reveal`
   saqlangan dayjest ochilgan foydali yuk bilan mos kelishini ta'minlaydi.
3. **`PolicyJuryBallotRevealV1`** – o'z ichiga olgan ommaviy oshkora ob'ekt
   ovoz berish tanlovi, bajarilish vaqtida foydalanilmagan vaqt va ixtiyoriy ZK isboti URI.
   Oshkorlar kamida 16 baytlik nonce talab qiladi, shuning uchun boshqaruv ushbu muammoni hal qilishi mumkin
   majburiyat, hatto hakamlar hay'ati xavfli kanallar orqali ishlaganda ham majburiydir.

`PolicyJurySortitionV1::validate` yordamchisi qo'mita o'lchamlarini belgilaydi,
takroriy aniqlash (qo'mitada ham, sud majlisida ham hakamlar hay'ati paydo bo'lishi mumkin emas
kutish roʻyxati), buyurtma qilingan kutish roʻyxati darajalari va yaroqli oʻzgarish moslamalari.  Saylov byulleteni
tekshirish tartiblari taklif yoki davra identifikatorlari bo'lganda `PolicyJuryBallotError` ni oshiradi
drift, hakamlar hay'ati noto'g'ri nonce bilan oshkor qilishga harakat qilganda yoki a
`zk-envelope` majburiyati o'z hujjatida mos keladigan dalillarni taqdim eta olmaydi
oshkor qilish.

### Mijozlar bilan integratsiya

- Boshqaruv vositalari tartiblash manifestini saqlab turishi va uni o'z ichiga olishi kerak
  siyosat paketlari, shuning uchun kuzatuvchilar POP snapshot dayjestini qayta hisoblashlari va
  tasodifiy mayoq plus nomzodlar to'plami bir xil olib kelishini tasdiqlang
  hakamlar hay'ati topshiriqlari.
- Hakamlar a'zosi mijozlari darhol keyin `PolicyJuryBallotCommitV1` yozishadi
  ularning ovozi uchun nonce hosil qiladi.  Olingan majburiyat baytlari bo'lishi mumkin
  Torii ga tayanch 64 qiymati sifatida taqdim etilgan yoki bevosita Norito ichiga kiritilgan
  voqealar.
- Oshkora bosqichida hakamlar hay'ati `PolicyJuryBallotRevealV1` chiqaradi.  Operatorlar
  oldin foydali yukni `PolicyJuryBallotCommitV1::verify_reveal` ga boqing
  ovoz berishni qabul qilish, oshkor qilish almashtirilmasligi yoki o'zgartirilmasligini ta'minlash.
- `zk-ballot` funksiyasi yoqilganda, hakamlar hay'ati deterministik ma'lumotlarni biriktirishi mumkin.
  dalil URI'lar (masalan, `sorafs://proofs/pj-2026-02/juror-5`) shuning uchun quyi oqim
  auditorlar tomonidan havola qilingan nol ma'lumotli guvohlar to'plamini olishlari mumkin
  majburiyat.Barcha uchta tuzilma `Encode`, `Decode` va `IntoSchema` ni hosil qiladi, ya'ni ular
ISI oqimlari, CLI asboblari, SDKlar va boshqaruv REST API uchun mavjud.
Kanonik Rust uchun `crates/iroha_data_model/src/ministry/jury.rs` ga qarang
ta'riflar va yordamchi usullar.

### Saralash manifestlari uchun CLI qo'llab-quvvatlashi

“Yo‘l xaritasi” bandi **MINFO-5**, shuningdek, boshqaruvni amalga oshirish uchun takrorlanadigan vositalarni talab qiladi.
har bir referendum paketi chop etilishidan oldin tekshirilishi mumkin bo'lgan siyosat-haylar ro'yxatini yuboring.
Ish maydoni endi `cargo xtask ministry-jury sortition` buyrug'ini ko'rsatadi:

```bash
cargo xtask ministry-jury sortition \
  --roster docs/examples/ministry/policy_jury_roster_example.json \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --beacon 22b1e48d47123f5c9e3f0cc0c8e34aa3c5f9c49a2cbb70559d3cb0ddc1a6ef01 \
  --committee-size 3 \
  --waitlist-size 2 \
  --drawn-at 2026-01-15T09:00:00Z \
  --waitlist-ttl-hours 72 \
  --out artifacts/ministry/policy_jury_sortition.json
```

- `--roster` deterministik PoP ro'yxatini qabul qiladi (JSON misoli:
  `docs/examples/ministry/policy_jury_roster_example.json`).  Har bir kirish
  `juror_id`, `pop_identity`, vazni va ixtiyoriyligini e'lon qiladi
  `grace_period_secs`.  Nomaqbul yozuvlar avtomatik ravishda filtrlanadi.
- `--beacon` boshqaruvda olingan 32 baytlik tasodifiy mayoqni kiritadi
  daqiqa.  CLI mayoqni to'g'ridan-to'g'ri ChaCha20 RNG ga ulaydi, shuning uchun auditorlar
  chizilgan bayt-baytni takrorlashi mumkin.
- `--committee-size`, `--waitlist-size` va `--waitlist-ttl-hours`
  o'tirgan hakamlar hay'ati soni, o'chirilish buferi va amal qilish muddati tugashi
  kutish ro'yxatidagi yozuvlarga.  Slot uchun muvaffaqiyatsiz daraja mavjud bo'lganda, buyruq
  mos keladigan kutish ro'yxati darajasiga ishora qiluvchi `PolicyJuryFailoverPlan` qayd qiladi.
- `--drawn-at` saralash uchun devor soati vaqt tamg'asini yozadi; asbob
  uni manifest uchun Unix millisekundlariga aylantiradi.

Yaratilgan manifest toʻliq tasdiqlangan `PolicyJurySortitionV1` foydali yukidir.
Katta o'rnatishlar odatda `artifacts/ministry/` ostida chiqishni saqlaydi, shuning uchun
ko'rib chiqish paneli bilan birga to'g'ridan-to'g'ri referendum paketlariga to'planishi mumkin
xulosa.  Tasviriy chiqishda mavjud
`docs/examples/ministry/policy_jury_sortition_example.json`, shuning uchun SDK guruhlari mumkin
o'zlarining Norito dekoderlarini mahalliy ravishda butun o'yinni takrorlamasdan ishlating.

### Saylov byulletenlarini topshirish/ko'rsatish yordamchilari

Hakamlar hay'ati mijozlari majburiyatni bajarish uchun deterministik vositalarga muhtoj → oqimni ochish.
Xuddi shu `cargo xtask ministry-jury` buyrug'i endi quyidagi yordamchilarni ochib beradi:

```bash
cargo xtask ministry-jury ballot commit \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --juror citizen:ada \
  --choice approve \
  --nonce-hex aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899 \
  --committed-at 2026-02-01T13:00:00Z \
  --out artifacts/ministry/policy_jury_commit_ada.json \
  --reveal-out artifacts/ministry/policy_jury_reveal_ada.json

cargo xtask ministry-jury ballot verify \
  --commit artifacts/ministry/policy_jury_commit_ada.json \
  --reveal artifacts/ministry/policy_jury_reveal_ada.json
```

- `ballot commit` `PolicyJuryBallotCommitV1` JSON foydali yukini chiqaradi.  Qachon
  `--out` qo'yilmaydi, buyruq stdout majburiyatini chop etadi.  Agar
  `--reveal-out` ta'minlanadi, asbob moslikni ham yozadi
  `PolicyJuryBallotRevealV1`, taqdim etilganlardan bir marta foydalanish va qo'llash
  ixtiyoriy `--revealed-at` vaqt tamg'asi (birlamchi `--committed-at` yoki
  joriy vaqt).
- `--nonce-hex` har qanday juft uzunlikdagi ≥16 bayt olti burchakli qatorni qabul qiladi.  O'tkazib yuborilganda
  yordamchi `OsRng` yordamida 32 baytlik nonce hosil qiladi, bu skriptni osonlashtiradi
  maxsus tasodifiy santexnika holda hakamlar ish oqimlari.
- `--choice` katta-kichik harflarni sezmaydi va `approve`, `reject` yoki `abstain` ni qabul qiladi.

`ballot verify` majburiyat/oshkor juftligini oʻzaro tekshiradi
`PolicyJuryBallotCommitV1::verify_reveal`, dumaloq identifikatorni kafolatlaydi,
taklif identifikatori, hakamlar aʼzosi identifikatori, nonce va ovoz berish tanlovi oshkor etilishidan oldin bir xil boʻladi
Torii ga qabul qilingan.  Tekshirish paytida yordamchi nolga teng bo'lmagan holat bilan chiqadi
muvaffaqiyatsiz bo'lib, CI yoki mahalliy hakamlar portaliga ulanishni xavfsiz qiladi.