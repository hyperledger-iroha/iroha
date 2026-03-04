---
lang: uz
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T14:35:37.551676+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Boshqaruv kitobi

Ushbu o'yin kitobi Sora tarmog'ini saqlaydigan kundalik marosimlarni o'z ichiga oladi
boshqaruv kengashi birlashtirildi. U nufuzli havolalarni jamlaydi
repository, shuning uchun individual marosimlar qisqacha qolishi mumkin, operatorlar esa har doim
kengroq jarayon uchun yagona kirish nuqtasiga ega.

## Kengash marosimlari

- **Fiksturni boshqarish** – [Sora Parlament qarorini tasdiqlash](sorafs/signing_ceremony.md) ga qarang.
  Parlamentning infratuzilma paneli hozirda zanjirdagi tasdiqlash oqimi uchun
  SoraFS chunker yangilanishlarini ko'rib chiqishda quyidagicha.
- **Ovoz berish natijalarini e'lon qilish** - qarang
  Bosqichma-bosqich CLI uchun [Governance Vote Tally](governance_vote_tally.md)
  ish jarayoni va hisobot shabloni.

## Operatsion Runbooks

- **API integratsiyalari** – [Governance API reference](governance_api.md)
  REST/gRPC sirtlari kengash xizmatlari tomonidan, jumladan, autentifikatsiya
  talablar va sahifalash qoidalari.
- **Telemetriya asboblar paneli** - ostidagi Grafana JSON ta'riflari
  `docs/source/grafana_*` “Boshqaruv cheklovlari” va “Rejalashtiruvchi”ni belgilaydi
  TEU” taxtalari. Har bir nashrdan soʻng JSON-ni Grafana-ga eksport qiling, bu esa bir xilda qoladi
  kanonik tartib bilan.

## Ma'lumotlar mavjudligini nazorat qilish

### Saqlash sinflari

DA manifestlarini tasdiqlovchi parlament panellari majburiy saqlashga havola qilishi kerak
ovoz berishdan oldin siyosat. Quyidagi jadval orqali amalga oshirilgan standart sozlamalar aks ettirilgan
`torii.da_ingest.replication_policy`, shuning uchun sharhlovchilar mos kelmasligini aniqlashlari mumkin
TOML manbasini qidirish.【docs/source/da/replication_policy.md:1】

| Boshqaruv tegi | Blob sinf | Issiq saqlash | Sovuqni ushlab turish | Kerakli nusxalar | Saqlash sinfi |
|----------------|------------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24 soat | 14d | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6 soat | 7d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 soat | 180d | 3 | `cold` |
| `da.default` | _barcha boshqa sinflar_ | 6 soat | 30d | 3 | `warm` |

Infratuzilma paneli to'ldirilgan shablonni dan biriktirishi kerak
`docs/examples/da_manifest_review_template.md` har bir byulletenga shunday manifest
dayjest, saqlash yorlig'i va Norito artefaktlari boshqaruvda bog'langan bo'lib qoladi
rekord.

### Imzolangan manifest tekshiruvi

Ovoz berish byulletenini kun tartibiga kiritishdan oldin kengash xodimlari manifest ekanligini isbotlashlari kerak
Ko'rib chiqilayotgan baytlar Parlament konvertiga va SoraFS artefaktiga mos keladi. Foydalanish
Ushbu dalillarni to'plash uchun mavjud vositalar:1. Torii (`iroha app da get-blob --storage-ticket <hex>`) dan manifest toʻplamini oling
   yoki shunga o'xshash SDK yordamchisi) shuning uchun hamma bir xil baytlarni xeshlaydi
   shlyuzlar.
2. Imzolangan konvert bilan manifest stub tekshiruvchini ishga tushiring:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   Bu BLAKE3 manifest dayjestini qayta hisoblab chiqadi va tasdiqlaydi
   `chunk_digest_sha3_256` va ichiga o'rnatilgan har bir Ed25519 imzosini tekshiradi
   `manifest_signatures.json`. `docs/source/sorafs/manifest_pipeline.md` ga qarang
   qo'shimcha CLI imkoniyatlari uchun.
3. Dayjest, `chunk_digest_sha3_256`, profil dastagi va imzo qo‘yuvchilar ro‘yxatidan nusxa ko‘chiring.
   ko'rib chiqish shabloni. QAYD: agar tekshiruvchi “profil mos kelmasligi” haqida xabar bersa yoki a
   imzo etishmayapti, ovoz berishni to'xtating va tuzatilgan konvertni so'rang.
4. Tasdiqlovchi chiqishini (yoki CI artefaktini saqlang
   `ci/check_sorafs_fixtures.sh`) Norito `.to` foydali yuk bilan birga auditorlar
   ichki shlyuzlarga kirmasdan dalillarni takrorlashi mumkin.

Olingan audit to'plami parlamentga har bir xesh va imzoni qayta yaratishga imkon berishi kerak
manifest issiq saqlash joyidan aylantirilgandan keyin ham tekshiring.

### Tekshirish ro'yxatini ko'rib chiqing

1. Parlament tomonidan tasdiqlangan manifest konvertini torting (qarang
   `docs/source/sorafs/signing_ceremony.md`) va BLAKE3 dayjestini yozib oling.
2. Manifestning `RetentionPolicy` bloki jadvaldagi tegga mos kelishini tekshiring
   yuqorida; Torii nomuvofiqliklarni rad etadi, ammo kengash buni qo'lga kiritishi kerak.
   auditorlar uchun dalillar.【docs/source/da/replication_policy.md:32】
3. Taqdim etilgan Norito foydali yuki bir xil saqlash tegiga havola qilishini tasdiqlang
   va qabul chiptasida ko'rinadigan blob sinfi.
4. Siyosat tekshiruvini tasdiqlovchi hujjatni ilova qiling (CLI chiqishi, `torii.da_ingest.replication_policy`
   dump yoki CI artefakt) ko'rib chiqish paketiga kiriting, shunda SRE qarorni takrorlashi mumkin.
5. Taklifga bog'liq bo'lsa, rejalashtirilgan subsidiya kranlarini yoki ijaraga tuzatishlarni yozib oling
   `docs/source/sorafs_reserve_rent_plan.md`.

### Eskalatsiya matritsasi

| So'rov turi | Egalik paneli | Qo'shish uchun dalillar | Belgilangan muddatlar va telemetriya | Adabiyotlar |
|-------------|--------------|-------------------|-----------------------|------------|
| Subsidiya / ijara to'lovi | Infratuzilma + G'aznachilik | To'ldirilgan DA paketi, `reserve_rentd` dan ijara deltasi, yangilangan zaxira proyeksiyasi CSV, kengash ovoz berish daqiqalari | G'aznachilik yangilanishini yuborishdan oldin ijara ta'siriga e'tibor bering; Moliya keyingi hisob-kitob oynasida kelishib olishi uchun 30d bufer telemetriyasini o'z ichiga oladi | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| Moderatsiya olib tashlash / muvofiqlik harakati | Moderatsiya + Muvofiqlik | Muvofiqlik chiptasi (`ComplianceUpdateV1`), isbot tokenlari, imzolangan manifest dayjesti, shikoyat holati | Gateway muvofiqligi SLAga rioya qiling (24 soat ichida tasdiqlang, to'liq olib tashlash ≤72 soat). Harakatni ko'rsatadigan `TransparencyReportV1` parchasini ilova qiling. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| Favqulodda muzlatish / orqaga qaytarish | Parlament moderatorlik paneli | Oldindan tasdiqlash paketi, yangi muzlatish tartibi, orqaga qaytarish manifest dayjesti, hodisalar jurnali | Muzlatish to'g'risidagi bildirishnomani zudlik bilan e'lon qiling va keyingi boshqaruv oralig'ida qayta tiklash referendumini belgilang; favqulodda vaziyatni oqlash uchun bufer to'yinganligi + DA replikatsiya telemetriyasini o'z ichiga oladi. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |Qabul qilish chiptalarini aniqlashda jadvaldan foydalaning, shunda har bir panel aniq ma'lumot oladi
o'z vakolatlarini bajarish uchun zarur bo'lgan artefaktlar.

### Yetkazib berish haqida hisobot berish

Har bir DA-10 qarori quyidagi artefaktlar bilan birga yuborilishi kerak (ularni
Ovoz berishda havola qilingan boshqaruv DAG yozuvi):

- To'ldirilgan Markdown paketi
  `docs/examples/da_manifest_review_template.md` (endi imzo va
  eskalatsiya bo'limlari).
- Imzolangan Norito manifest (`.to`) va `manifest_signatures.json` konvert
  yoki CI tekshiruvi jurnallari, bu olib kirishni tasdiqlaydi.
- Har qanday shaffoflik yangilanishlari harakat tufayli:
  - `TransparencyReportV1` deltasi olib tashlash yoki muvofiqlikka asoslangan muzlatish uchun.
  - Ijara/zaxira kitobi deltasi yoki subsidiyalar uchun `ReserveSummaryV1` surati.
- Ko'rib chiqish paytida to'plangan telemetriya suratlariga havolalar (replikatsiya chuqurligi,
  bufer bo'sh joy, moderatsiya qoldig'i) shuning uchun kuzatuvchilar shartlarni o'zaro tekshirishlari mumkin
  haqiqatdan keyin.

## Moderatsiya va eskalatsiya

Muvofiqlikdan keyin shlyuzni olib tashlash, subsidiyalarni qaytarish yoki DAni muzlatish
quvur liniyasi `docs/source/sorafs_gateway_compliance_plan.md` da tasvirlangan va
`docs/source/sorafs_moderation_panel_plan.md` da apellyatsiya vositalari. Panellar quyidagilarga ega bo'lishi kerak:

1. Dastlabki muvofiqlik chiptasini (`ComplianceUpdateV1` yoki
   `ModerationAppealV1`) va tegishli isbot tokenlarini biriktiring.【docs/source/sorafs_gateway_compliance_plan.md:20】
2. So‘rov moderatsiya apellyatsiya yo‘lini chaqirishini tasdiqlang (fuqarolar paneli
   ovoz berish) yoki favqulodda parlamentni muzlatish; ikkala oqim ham manifestni keltirishi kerak
   yangi shablonda olingan dayjest va saqlash tegi.【docs/source/sorafs_moderation_panel_plan.md:1】
3. Ko'tarilish muddatlarini sanab o'ting (apellyatsiya berish/oshkor qilish oynalari, favqulodda vaziyatlar
   muzlatish muddati) va kuzatuv qaysi kengash yoki panelga tegishli ekanligini ko'rsating.
4. Telemetriya snapshotini suratga oling (bufer bo'sh joy, moderatsiya qoldig'i)
   harakatni oqlang, shunda quyi oqim auditlari qarorni jonli bilan moslashtirishi mumkin
   davlat.

Muvofiqlik va moderatsiya panellari haftalik shaffoflik hisobotlarini sinxronlashtirishi kerak
hisob-kitob router operatorlari bilan, shuning uchun olib tashlash va subsidiyalar bir xil ta'sir qiladi
manifestlar to'plami.

## Hisobot shablonlari

Endi barcha DA-10 sharhlari imzolangan Markdown paketini talab qiladi. Nusxalash
`docs/examples/da_manifest_review_template.md`, manifest metamaʼlumotlarini toʻldiring,
saqlashni tekshirish jadvali va panel ovozlari xulosasini kiriting, so‘ngra to‘ldirilganni mahkamlang
Hujjat (qoʻshimcha havola qilingan Norito/JSON artefaktlari) Governance DAG yozuviga.
Panellar paketni boshqaruv protokollarida bog'lashi kerak, shuning uchun kelajakda olib tashlash yoki
subsidiyalarni yangilash asl manifest dayjestini qayta ishga tushirmasdan keltirishi mumkin
butun marosim.

## Hodisa va bekor qilish ish jarayoni

Favqulodda harakatlar endi zanjirda amalga oshiriladi. Bir armatura ozod bo'lishi kerak bo'lganda
orqaga qaytdi, boshqaruv chiptasini topshiring va parlamentni qaytarish taklifini oching
ilgari tasdiqlangan manifest dayjestiga ishora. Infratuzilma paneli
ovoz berishni boshqaradi va yakunlangandan so'ng Nexus ish vaqti orqaga qaytishni nashr etadi
quyi oqimdagi mijozlar iste'mol qiladigan hodisa. Mahalliy JSON artefaktlari talab qilinmaydi.

## O'yin kitobini joriy qilish- Boshqaruvga mos keladigan yangi runbook fayli qachon paydo bo'lsa, ushbu faylni yangilang
  ombori.
- Bu yerda yangi marosimlarni o'zaro bog'lang, shuning uchun kengash indeksi aniq bo'lib qoladi.
- Agar havola qilingan hujjat harakatlansa (masalan, yangi SDK yo'li), havolani yangilang
  eskirgan ko'rsatkichlardan qochish uchun bir xil tortish so'rovining bir qismi sifatida.