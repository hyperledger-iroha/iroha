---
lang: uz
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 34fd39f47a276eaf175ddb014b3baaa94bba1c3a9b017600a6014c9592ad4cdb
source_last_modified: "2026-01-18T15:31:35.202347+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

sarlavha: Ma'lumotlar mavjudligi tahdid modeli
sidebar_label: tahdid modeli
tavsif: Sora Nexus maʼlumotlari mavjudligi uchun tahdid tahlili, yumshatish va qoldiq xavflar.
---

::: Eslatma Kanonik manba
:::

# Sora Nexus Ma'lumotlar mavjudligi tahdid modeli

_Oxirgi ko'rib chiqilgan: 2026-01-19 — Keyingi rejalashtirilgan ko'rib chiqish: 2026-04-19_

Xizmat ko'rsatish darajasi: Ma'lumotlar mavjudligi bo'yicha ishchi guruh (<=90 kun). Har bir qayta ko'rib chiqish kerak
faol yumshatish chiptalari va simulyatsiya artefaktlariga havolalar bilan `status.md` da paydo bo'ladi.

## Maqsad va qamrov

Ma'lumotlar mavjudligi (DA) dasturi Taikai eshittirishlarini, Nexus qatorli bloblarni va
Vizantiya, tarmoq va operator xatolari ostida qayta tiklanadigan boshqaruv artefaktlari.
Ushbu tahdid modeli DA-1 (arxitektura va tahdid modeli) uchun muhandislik ishlarini birlashtiradi.
va quyi oqimdagi DA vazifalari uchun asos bo'lib xizmat qiladi (DA-2 dan DA-10gacha).

Amaldagi komponentlar:
- Torii DA kengaytmasi va Norito metamaʼlumotlar yozuvchilari.
- SoraFS tomonidan qo'llab-quvvatlanadigan blob saqlash daraxtlari (issiq/sovuq qatlamlar) va replikatsiya siyosatlari.
- Nexus blok majburiyatlari (sim formatlari, isbotlar, engil mijoz API'lari).
- DA foydali yuklarga xos bo'lgan PDP/PoTR ijro ilgaklari.
- Operatorning ish oqimlari (pinning, ko'chirish, kesish) va kuzatuv quvurlari.
- DA operatorlari va tarkibini qabul qiladigan yoki chiqarib yuboradigan boshqaruv tasdiqlari.

Ushbu hujjat uchun amal qilmaydi:
- To'liq iqtisodni modellashtirish (DA-7 ish oqimida olingan).
- SoraFS tahdid modeli allaqachon qamrab olingan SoraFS asosiy protokollari.
- Mijoz SDK ergonomikasi tahdid yuzasidan mulohazalar tashqarisida.

## Arxitekturaga umumiy nuqtai

1. **Yuborish:** Mijozlar bloblarni Torii DA ingest API orqali yuboradi. Tugun
   bo'laklarni parchalaydi, Norito manifestlarini kodlaydi (blob turi, chiziq, davr, kodek bayroqlari),
   va parchalarni issiq SoraFS darajasida saqlaydi.
2. **Reklama:** Pin niyatlari va replikatsiya bo‘yicha maslahatlar xotiraga tarqaladi
   provayderlar ro'yxatga olish kitobi (SoraFS bozor) orqali siyosat teglari bilan
   issiq/sovuqni saqlash maqsadlarini davlat.
3. **Majburiyat:** Nexus sekvenserlari blob majburiyatlarini oʻz ichiga oladi (CID + ixtiyoriy KZG
   ildizlar) kanonik blokda. Engil mijozlar majburiyat hash va tayanadi
   mavjudligini tekshirish uchun reklama qilingan metama'lumotlar.
4. **Replikatsiya:** Saqlash tugunlari tayinlangan aktsiyalarni/bo'laklarni tortib oladi, PDP/PoTRni qondiradi
   muammolar va siyosat bo'yicha issiq va sovuq darajalar o'rtasidagi ma'lumotlarni targ'ib qilish.
5. **Fetch:** Iste'molchilar ma'lumotlarni SoraFS yoki DA-dan xabardor shlyuzlar orqali tekshirib oladilar.
   isbotlar va nusxalar yo'qolganda tuzatish so'rovlarini ko'tarish.
6. **Boshqaruv:** Parlament va DA nazorat qo'mitasi operatorlarni tasdiqlaydi,
   ijara jadvallari va majburiyatlarning kuchayishi. Boshqaruv artefaktlari saqlanadi
   jarayonning shaffofligini ta'minlash uchun bir xil DA yo'li orqali.

## Aktivlar va egalar

Ta'sir shkalasi: **Kritik** buxgalteriya kitobining xavfsizligini/hayotiyligini buzadi; **Yuqori** DA ni bloklaydi
to'ldirish yoki mijozlar; **O'rtacha** sifatni pasaytiradi, lekin qayta tiklanadigan bo'lib qoladi;
**Past** cheklangan effekt.

| Aktiv | Tavsif | Butunlik | Mavjudligi | Maxfiylik | Egasi |
| --- | --- | --- | --- | --- | --- |
| DA bloblari (bo'laklar + manifestlar) | SoraFS da saqlangan Taikai, lane, boshqaruv bloklari | Tanqidiy | Tanqidiy | O'rtacha | DA WG / Saqlash jamoasi |
| Norito DA namoyon bo'ladi | Bloblarni tavsiflovchi terilgan metama'lumotlar | Tanqidiy | Yuqori | O'rtacha | Asosiy protokol WG |
| Majburiyatlarni bloklash | Nexus bloklari ichidagi CID + KZG ildizlari | Tanqidiy | Yuqori | Past | Asosiy protokol WG |
| PDP/PoTR jadvallari | DA replikalari uchun amal qilish kadensi | Yuqori | Yuqori | Past | Saqlash jamoasi |
| Operator reestri | Tasdiqlangan saqlash provayderlari va siyosatlari | Yuqori | Yuqori | Past | Boshqaruv Kengashi |
| Ijara va rag'batlantirish yozuvlari | DA ijarasi va jarimalar uchun daftar yozuvlari | Yuqori | O'rtacha | Past | G'aznachilik WG |
| Kuzatuv paneli | DA SLOs, replikatsiya chuqurligi, ogohlantirishlar | O'rtacha | Yuqori | Past | SRE / Kuzatish mumkinligi |
| Ta'mirlash niyatlari | Yo'qolgan bo'laklarni regidratatsiya qilish uchun so'rovlar | O'rtacha | O'rtacha | Past | Saqlash jamoasi |

## Dushmanlar va imkoniyatlar

| Aktyor | Imkoniyatlar | Motivatsiyalar | Eslatmalar |
| --- | --- | --- | --- |
| Zararli mijoz | Noto'g'ri shakllangan bloblarni yuboring, eskirgan manifestlarni takrorlang, qabul qilishda DoSni sinab ko'ring. | Taikai eshittirishlarini buzing, noto'g'ri ma'lumotlarni kiriting. | Imtiyozli kalitlar yo'q. |
| Vizantiya saqlash tugunlari | Belgilangan nusxalarni tashlab yuboring, PDP/PoTR dalillarini soxtalashtiring, boshqalar bilan til biriktiring. | DA saqlashni qisqartiring, ijaradan qoching, ma'lumotlarni garovga oling. | Yaroqli operator hisob ma'lumotlariga ega. |
| Buzilgan sekvenser | Majburiyatlarni o'tkazib yuboring, bloklarda ikkilantiring, blob metama'lumotlarini qayta tartiblang. | DA taqdimotlarini yashirish, nomuvofiqlik yaratish. | Konsensus ko'pchilik tomonidan cheklangan. |
| Insayder operator | Boshqaruvga kirish huquqini suiiste'mol qilish, saqlash siyosatini buzish, hisob ma'lumotlarini sizib chiqish. | Iqtisodiy foyda, sabotaj. | Issiq/sovuq darajadagi infratuzilmaga kirish. |
| Tarmoq raqibi | Bo'lim tugunlari, replikatsiyani kechiktirish, MITM trafigini kiritish. | Mavjudlikni kamaytiring, SLOlarni yomonlashtiring. | TLSni sindira olmaydi, lekin havolalarni tashlab/sekinlashtirishi mumkin. |
| Kuzatuvga tajovuzkor | Boshqaruv paneli/ogohlantirishlarni buzish, hodisalarni bostirish. | DA uzilishlarini yashirish. | Telemetriya quvuriga kirishni talab qiladi. |

## Ishonch chegaralari

- **Kirish chegarasi:** Torii DA kengaytmasi mijozi. Soʻrov darajasida autentifikatsiyani talab qiladi,
  tezlikni cheklash va yukni tekshirish.
- **Replikatsiya chegarasi:** Bo'laklar va dalillarni almashadigan saqlash tugunlari. Tugunlar
  o'zaro autentifikatsiya qilingan, lekin o'zini Vizantiya tutishi mumkin.
- **Buxgalteriya hisobi chegarasi:** Qabul qilingan blok ma'lumotlari va zanjirdan tashqari saqlash. Konsensus himoyachilari
  yaxlitlik, ammo mavjudlik zanjirdan tashqari amal qilishni talab qiladi.
- **Boshqaruv chegarasi:** Operatorlarni tasdiqlovchi Kengash/Parlament qarorlari,
  byudjetlar va qisqartirish. Bu yerdagi tanaffuslar DA o'rnatilishiga bevosita ta'sir qiladi.
- **Kuzatilish chegarasi:** Ko‘rsatkichlar/jurnallar to‘plami asboblar paneli/ogohlantirishga eksport qilindi
  asboblar. Buzg'unchilik uzilishlar yoki hujumlarni yashiradi.

## Tahdid stsenariylari va boshqaruvlari

### Yutish yo'li hujumlari

**Ssenariy:** Zararli mijoz noto‘g‘ri tuzilgan Norito foydali yuklarni yoki katta hajmdagi yuklarni yuboradi
resurslarni sarflash yoki noto'g'ri metama'lumotlarni noqonuniy olib o'tish uchun bloblar.

**Boshqaruvlar**
- qat'iy versiya muzokaralari bilan Norito sxemasini tekshirish; noma'lum bayroqlarni rad qilish.
- Torii qabul qilish so'nggi nuqtasida tarifni cheklash va autentifikatsiya qilish.
- SoraFS chunker tomonidan qo'llaniladigan bo'lak o'lchami chegaralari va deterministik kodlash.
- Qabul quvur liniyasi yaxlitlikni tekshirish summasi mos kelgandan keyingina namoyon bo'ladi.
- Deterministik takrorlash keshi (`ReplayCache`) `(lane, epoch, sequence)` oynalarini kuzatib boradi, diskdagi yuqori suv belgilarini saqlaydi va dublikatlarni/eskirgan takrorlarni rad etadi; mulk va noaniq jabduqlar turli barmoq izlari va tartibsiz taqdimotlarni qamrab oladi.【crates/iroha_core/src/da/replay_cache.rs:1】【fuzz/da_replay_cache.rs:1】【crates/iroha_torii/src/da/ingest.

**Qoldiq bo'shliqlar**
- Torii ingest takrorlash keshini qabul qilish va qayta ishga tushirishda kursorlar ketma-ketligini saqlab turishi kerak.
- Norito DA sxemalarida endi o'zgarmaslarni kodlash/dekodlash uchun maxsus noaniq jabduqlar (`fuzz/da_ingest_schema.rs`) mavjud; qamrov panellari, agar maqsad orqaga qaytsa, ogohlantirishi kerak.

### Replikatsiyani ushlab qolish

**Ssenariy:** Vizantiya saqlash operatorlari pin topshiriqlarini qabul qiladilar, lekin qismlarni tashlab yuborishadi,
soxta javoblar yoki til biriktirish orqali PDP/PoTR muammolaridan o'tish.

**Boshqaruvlar**
- PDP/PoTR sinov jadvali har bir davr qamroviga ega bo'lgan DA foydali yuklarini qamrab oladi.
- kvorum chegaralari bilan ko'p manbali replikatsiya; olib kelish orkestrini aniqlaydi
  etishmayotgan parchalar va triggerlarni tuzatish.
- Muvaffaqiyatsiz isbotlar va etishmayotgan nusxalar bilan bog'liq boshqaruvni qisqartirish.
- Avtomatlashtirilgan yarashuv ishi (`cargo xtask da-commitment-reconcile`) solishtiradi
  DA majburiyatlari bilan kvitansiyalarni qabul qilish (SignedBlockWire, `.norito` yoki JSON),
  boshqaruv uchun JSON dalillar to'plamini chiqaradi va etishmayotgan/mos kelmaydigan bo'lsa muvaffaqiyatsiz
  chiptalar, shuning uchun Alertmanager o'tkazib yuborish/buzg'unchilik haqida sahifaga kirishi mumkin.

**Qoldiq bo'shliqlar**
- `integration_tests/src/da/pdp_potr.rs` simulyatsiya jabduqlari (qoplangan
  `integration_tests/tests/da/pdp_potr_simulation.rs`) endi til biriktirishni amalga oshiradi
  va bo'lim stsenariylari, PDP/PoTR jadvali aniqlaganligini tasdiqlaydi
  Vizantiya xulq-atvori deterministik. Uni DA-5 bilan birga kengaytirishda davom eting
  yangi isbotlangan yuzalarni yoping.
- Sovuq darajadagi evakuatsiya siyosati yashirin tushishning oldini olish uchun imzolangan audit izini talab qiladi.

### Majburiyatni buzish

**Ssenariy:** Buzilgan sekvenser DA ni o'tkazib yuborgan yoki o'zgartirgan bloklarni nashr etadi
qabul qilishda muvaffaqiyatsizliklar yoki engil mijoz nomuvofiqliklariga olib keladigan majburiyatlar.

**Boshqaruvlar**
- Konsensus o'zaro tekshiruvlar DA topshirish navbatlari bilan takliflarni bloklaydi; tengdoshlar rad etadi
  talab qilinadigan majburiyatlardan mahrum bo'lgan takliflar.
- Yengil mijozlar qabul qilish tutqichlarini yopishdan oldin majburiyatni kiritish dalillarini tekshiradi.
- Taqdim etilgan kvitansiyalarni blokli majburiyatlar bilan taqqoslaydigan audit tekshiruvi.
- Avtomatlashtirilgan yarashuv ishi (`cargo xtask da-commitment-reconcile`) solishtiradi
  DA majburiyatlari bilan kvitansiyalarni qabul qilish (SignedBlockWire, `.norito` yoki JSON),
  boshqaruv uchun JSON dalillar to'plamini chiqaradi va etishmayotgan yoki muvaffaqiyatsiz
  noto'g'ri chiptalar, shuning uchun Alertmanager o'tkazib yuborilgan/buzg'unchi bo'lishi mumkin.

**Qoldiq bo'shliqlar**
- yarashtirish ishi + Alertmanager kancasi bilan qoplangan; boshqaruv paketlari hozir
  sukut bo'yicha JSON dalillar to'plamini qabul qiling.

### Tarmoq bo'limi va tsenzura**Ssenariy:** Dushman bo'limlari replikatsiya tarmog'i, tugunlarning oldini oladi
tayinlangan qismlarni olish yoki PDP/PoTR muammolariga javob berish.

**Boshqaruvlar**
- Ko'p mintaqaviy provayder talablari turli tarmoq yo'llarini ta'minlaydi.
- Challenge oynalari jitter va tarmoqdan tashqari ta'mirlash kanallariga qaytishni o'z ichiga oladi.
- Kuzatish panellari replikatsiya chuqurligini nazorat qiladi, muvaffaqiyatga erishadi va
  ogohlantirish chegaralari bilan kechikishni olish.

**Qoldiq bo'shliqlar**
- Taikai jonli voqealari uchun bo'linish simulyatsiyalari hali ham mavjud emas; namlash testlari kerak.
- Ta'mirlash o'tkazish qobiliyatini band qilish siyosati hali kodlanmagan.

### Insayder suiiste'moli

**Ssenariy:** Ro‘yxatga olish kitobiga kirish huquqiga ega operator saqlash siyosatini boshqaradi,
zararli provayderlarni oq ro'yxatga kiritadi yoki ogohlantirishlarni bostiradi.

**Boshqaruvlar**
- Boshqaruv harakatlari ko'p partiyali imzolarni va Norito-notarial tasdiqlangan yozuvlarni talab qiladi.
- Siyosat o'zgarishlari monitoring va arxiv jurnallariga voqealarni chiqaradi.
- Kuzatiladigan quvur liniyasi xesh zanjiri bilan faqat qo'shiladigan Norito jurnallarini majbur qiladi.
- Har chorakda kirishni ko'rib chiqishni avtomatlashtirish (`cargo xtask da-privilege-audit`) yurishlari
  DA manifest/takrorlash kataloglari (plyus operator tomonidan taqdim etilgan yo'llar), bayroqlar
  etishmayotgan/katalog bo'lmagan/dunyoda yoziladigan yozuvlar va imzolangan JSON to'plamini chiqaradi
  boshqaruv paneli uchun.

**Qoldiq bo'shliqlar**
- Boshqaruv panelini buzish-dalil imzolangan suratlarni talab qiladi.

## Qoldiq xavf registri

| Xavf | Ehtimollik | Ta'sir | Egasi | Yumshatish rejasi |
| --- | --- | --- | --- | --- |
| DA ning takrorlanishi DA-2 ketma-ketligi keshiga tushishidan oldin namoyon bo'ladi | Mumkin | O'rtacha | Asosiy protokol WG | DA-2da ketma-ketlik keshini + bir martalik tekshiruvni amalga oshirish; regressiya testlarini qo'shing. |
| >f tugunlari buzilganda PDP/PoTR kelishuvi | Darhaqiqat | Yuqori | Saqlash jamoasi | Provayderlar o'rtasida namuna olish bilan yangi sinov jadvalini oling; simulyatsiya jabduqlari orqali tasdiqlang. |
| Sovuq darajadagi ko'chirish audit bo'shlig'i | Mumkin | Yuqori | SRE / Saqlash jamoasi | Imzolangan audit jurnallari va ko'chirish uchun zanjirli kvitansiyalarni ilova qiling; asboblar paneli orqali nazorat qilish. |
| Sekvenser qoldirilishini aniqlash kechikishi | Mumkin | Yuqori | Asosiy protokol WG | Kechagi `cargo xtask da-commitment-reconcile` tushumlar va majburiyatlar (SignedBlockWire/`.norito`/JSON) va etishmayotgan yoki mos kelmaydigan chiptalar boʻyicha sahifalar boshqaruvini taqqoslaydi. |
| Taikai jonli translatsiyalari uchun qismlarga chidamlilik | Mumkin | Tanqidiy | Networking TL | Ajratish mashqlarini bajaring; zaxira ta'mirlash o'tkazish qobiliyati; Hujjatni almashtirish SOP. |
| Boshqaruv imtiyozlarining o'zgarishi | Darhol | Yuqori | Boshqaruv Kengashi | Imzolangan JSON + asboblar paneli eshigi bilan har chorakda `cargo xtask da-privilege-audit` ishga tushirish (manifest/takroriy dirssiyalar + qo'shimcha yo'llar); zanjirdagi langar audit artefaktlari. |

## Majburiy kuzatuvlar

1. DA ingest Norito sxemalarini va misol vektorlarini nashr qiling (DA-2ga o'tkaziladi).
2. Qayta o'ynash keshini Torii DA ingest orqali o'tkazing va tugunni qayta ishga tushirishda ketma-ket kursorlarni davom ettiring.
3. **Tugallandi (2026-02-05):** PDP/PoTR simulyatsiya jabduqlari endi QoS orqada qolmagan modellashtirish bilan til biriktirish + qismlarga ajratish stsenariylarini amalga oshiradi; Quyida olingan amalga oshirish va deterministik xulosalar uchun `integration_tests/src/da/pdp_potr.rs` (`integration_tests/tests/da/pdp_potr_simulation.rs` ostidagi testlar bilan) ga qarang.
4. **Tugallandi (29-05-2026):** `cargo xtask da-commitment-reconcile` qabul qilish kvitansiyalarini DA majburiyatlari (SignedBlockWire/`.norito`/JSON) bilan solishtiradi, `artifacts/da/commitment_reconciliation.json` ni chiqaradi va ATVga o‘raladi. o'tkazib yuborish/buzg'unchilik haqida ogohlantirishlar (`xtask/src/da.rs`).
5. **Tugallandi (2026-05-29):** `cargo xtask da-privilege-audit` manifest/takrorlash spulini (shuningdek, operator tomonidan taqdim etilgan yo‘llar) bo‘ylab yuradi, etishmayotgan/katalogda bo‘lmagan/dunyoga yozilmaydigan yozuvlarni belgilab beradi va boshqaruv panellari uchun imzolangan JSON to‘plamini ishlab chiqaradi. (`artifacts/da/privilege_audit.json`), kirish va ko'rib chiqishni avtomatlashtirish bo'shlig'ini yopish.

**Keyingi qayerga qarash kerak:**

- DA replay keshi va kursorning barqarorligi DA-2 ga tushdi. ga qarang
  `crates/iroha_core/src/da/replay_cache.rs` da amalga oshirish (kesh mantig'i) va
  `crates/iroha_torii/src/da/ingest.rs` da Torii integratsiyasi,
  barmoq izlari `/v1/da/ingest` orqali tekshiriladi.
- PDP/PoTR oqim simulyatsiyalari proof-stream jabduqlari orqali amalga oshiriladi
  `crates/sorafs_car/tests/sorafs_cli.rs`, PoR/PDP/PoTR soʻrov oqimlarini qamrab oladi
  va tahdid modelida jonlantirilgan muvaffaqiyatsizlik stsenariylari.
- Imkoniyatlar va ta'mirlash ho'llash natijalari ostida yashaydi
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, kengroq esa
  Sumeragi namlash matritsasi `docs/source/sumeragi_soak_matrix.md` da kuzatiladi
  (mahalliylashtirilgan variantlar kiritilgan). Bu artefaktlar uzoq davom etgan mashqlarni ushlaydi
  qoldiq xavf reestrida havola qilingan.
- Kelishuv + imtiyoz-audit avtomatizatsiyasi yashaydi
  `docs/automation/da/README.md` va yangi `cargo xtask da-commitment-reconcile`
  / `cargo xtask da-privilege-audit` buyruqlari; ostidagi standart chiqishlardan foydalaning
  Boshqaruv paketlariga dalillarni biriktirishda `artifacts/da/`.

## Simulyatsiya dalillari va QoS modellashtirish (2026-02)

DA-1 №3 kuzatuvini yopish uchun biz deterministik PDP/PoTR simulyatsiyasini kodlashtirdik
`integration_tests/src/da/pdp_potr.rs` ostida jabduqlar (qoplangan
`integration_tests/tests/da/pdp_potr_simulation.rs`). Jabduqlar
uchta hudud bo'ylab tugunlarni ajratadi, bo'limlarni/to'tiqushlarni mos ravishda kiritadi
yo'l xaritasi ehtimoli, PoTR kechikishini kuzatib boradi va ta'mirlash-ortda qolishni ta'minlaydi
issiq darajadagi ta'mirlash byudjetini aks ettiruvchi model. Standart stsenariyni ishga tushirish
(12 ta davr, 18 ta PDP muammosi + har bir davr uchun 2 ta PoTR oynasi)
quyidagi ko'rsatkichlar:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metrik | Qiymat | Eslatmalar |
| --- | --- | --- |
| PDP nosozliklari aniqlandi | 48/49 (98,0%) | Bo'limlar hali ham aniqlashni boshlaydi; bitta aniqlanmagan muvaffaqiyatsizlik halol jitterdan kelib chiqadi. |
| PDP o'rtacha aniqlash kechikishi | 0,0 davrlar | Muvaffaqiyatsizliklar dastlabki davrda yuzaga keladi. |
| PoTR nosozliklari aniqlandi | 28/77 (36,4%) | Aniqlash tugun ≥2 PoTR oynasini o‘tkazib yuborgandan so‘ng ishga tushadi va aksariyat hodisalar qoldiq xavf registrida qoladi. |
| PoTR o'rtacha aniqlash kechikishi | 2.0 davrlar | Arxiv eskalatsiyasiga aylantirilgan ikki davr kechikish chegarasiga mos keladi. |
| Ta'mirlash navbatining eng yuqori nuqtasi | 38 manifest | Bo'limlar har bir davr uchun mavjud bo'lgan to'rtta ta'mirlashdan tezroq yig'ilsa, orqada qoladigan yuk ko'tariladi. |
| Javob kechikishi p95 | 30,068 ms | QoS namunasi uchun qo'llaniladigan ±75 ms jitter bilan 30 s sinov oynasini aks ettiradi. |
<!-- END_DA_SIM_TABLE -->

Ushbu chiqishlar endi DA asboblar paneli prototiplarini boshqaradi va "simulyatsiya" ni qondiradi
jabduqlar + QoS modellashtirish" qabul qilish mezonlari yo'l xaritasida keltirilgan.

Avtomatlashtirish endi `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]` orqasida yashaydi, bu umumiy jabduqlar va
sukut bo'yicha Norito JSONni `artifacts/da/threat_model_report.json` ga chiqaradi. Kechasi
Ishlar ushbu hujjatdagi matritsalarni yangilash va ogohlantirish uchun ushbu faylni ishlatadi
aniqlash tezligi, ta'mirlash navbatlari yoki QoS namunalarida drift.

Hujjatlar uchun yuqoridagi jadvalni yangilash uchun `make docs-da-threat-model` ni ishga tushiring.
`cargo xtask da-threat-model-report` ni chaqiradi, qayta hosil qiladi
`docs/source/da/_generated/threat_model_report.json` va ushbu bo'limni qayta yozadi
`scripts/docs/render_da_threat_model_tables.py` orqali. `docs/portal` oynasi
(`docs/portal/docs/da/threat-model.md`) ikkalasi ham bir xil passda yangilanadi
nusxalar sinxronlashtiriladi.