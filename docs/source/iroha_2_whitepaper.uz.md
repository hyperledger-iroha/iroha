---
lang: uz
direction: ltr
source: docs/source/iroha_2_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a4e8824c128b9f2a34262a5c9bc09f6b2cd790a0561aa083fa18a987accd7004
source_last_modified: "2026-01-22T16:26:46.570053+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha v2.0

Hyperledger Iroha v2 deterministik, Vizantiya xatosiga bardoshli taqsimlangan daftar boʻlib, u
modulli arxitektura, kuchli standart sozlamalar va qulay API. Platforma Rust qutilari to'plami sifatida jo'natiladi
buyurtma asosida joylashtirilishi yoki ishlab chiqarish blokcheyn tarmog'ini boshqarish uchun birgalikda ishlatilishi mumkin.

---

## 1. Umumiy ko'rinish

Iroha 2 Iroha 1 bilan taqdim etilgan dizayn falsafasini davom ettiradi: tanlangan to'plamni taqdim eting
imkoniyatlar qutidan tashqarida, shuning uchun operatorlar katta hajmdagi buyurtma yozmasdan tarmoqqa turishlari mumkin
kod. v2 versiyasi ijro muhiti, konsensus quvur liniyasi va ma'lumotlar modelini birlashtiradi
yagona birlashtirilgan ish maydoni.

v2 liniyasi o'z ruxsati yoki konsorsiumini boshqarishni xohlaydigan tashkilotlar uchun mo'ljallangan
blokcheynlar. Har bir joylashtirish o'z konsensus tarmog'ini boshqaradi, mustaqil boshqaruvni qo'llab-quvvatlaydi va moslashtira oladi
konfiguratsiya, genezis ma'lumotlari va uchinchi tomonlarga bog'liq holda kadansni yangilash. Umumiy ish maydoni
xususiyatlarni tanlashda bir nechta mustaqil tarmoqlarni aynan bir xil kod bazasiga qarshi qurish imkonini beradi
ulardan foydalanish holatlariga mos keladigan siyosatlar.

Iroha 2 va SORA Nexus (Iroha 3) bir xil Iroha virtual mashinasini (IVM) ishlaydi. Ishlab chiquvchilar Kotodama mualliflik qilishlari mumkin
shartnomalarni bir marta tuzing va ularni qayta kompilyatsiya qilmasdan yoki o'z-o'zidan joylashtirilgan tarmoqlarda yoki global Nexus kitobida joylashtiring
ijro muhitini forking.
### 1.1 Hyperledger ekotizimiga aloqasi

Iroha komponentlari boshqa Hyperledger loyihalari bilan hamkorlik qilish uchun moʻljallangan. Konsensus, ma'lumotlar modeli va
Serializatsiya qutilarini kompozit steklarda yoki Fabric, Sawtooth va Besu joylashtirishlari bilan bir qatorda qayta ishlatish mumkin.
Norito kodeklari va boshqaruv manifestlari kabi umumiy vositalar interfeyslarni bir xilda saqlashga yordam beradi.
ekotizim Iroha ga fikrga asoslangan standart dasturni taqdim etishga ruxsat beradi.

### 1.2 Mijoz kutubxonalari va SDKlar

Birinchi darajali mobil va veb-tajribalarni ta'minlash uchun loyiha SDK-larni nashr etadi:

- iOS va macOS mijozlari uchun `IrohaSwift`, Metall/NEON tezlashuvini deterministik zaxiralar ortida birlashtiradi.
- JavaScript va TypeScript ilovalari, jumladan Kaigi quruvchilari va Norito yordamchilari uchun `iroha_js`.
- HTTP, WebSocket va telemetriyani qo'llab-quvvatlaydigan Python integratsiyasi uchun `iroha_python`.
- Terminal boshqaruvi va skript uchun `iroha_cli`.

tillar va platformalar.

### 1.3 Dizayn tamoyillari- **Avval determinizm:** Har bir tugun bir xil kod yo‘llarini bajaradi va bir xil berilganda bir xil natijalarni beradi.
  kirishlar. SIMD/CUDA/NEON yo'llari xususiyatga ega va deterministik skalyar ilovalarga qaytadi.
- **Tuzilishi mumkin bo'lgan modullar: ** Tarmoq, konsensus, ijro, telemetriya va saqlash har biri alohida ajratilgan.
  qutilar, shuning uchun o'rnatuvchilar to'liq stekni ko'tarmasdan kichik to'plamlarni qabul qilishlari mumkin.
- **Aniq konfiguratsiya:** Xulq-atvor tugmalari `iroha_config` orqali chiqariladi; atrof-muhitni o'zgartirish tugmalari
  ishlab chiquvchilarning qulayliklari bilan cheklangan.
- **Xavfsiz standart sozlamalar: ** Kanonik kodeklar, ABI ko'rsatkichlarining qat'iy bajarilishi va versiyali manifestlar
  tarmoqlararo yangilanishlarni bashorat qilish mumkin.

## 2. Platforma arxitekturasi

### 2.1 Tugun tarkibi

Iroha tugunida bir nechta hamkorlik xizmatlari mavjud:

- **Torii (`iroha_torii`)** tranzaktsiyalar, so'rovlar, oqim hodisalari va boshqalar uchun HTTP/WebSocket API'larini ochib beradi.
  telemetriya (`/v1/...` oxirgi nuqtalari).
- **Core (`iroha_core`)** tasdiqlash, konsensus, ijro, boshqaruv va davlat boshqaruvini muvofiqlashtiradi.
- **Sumeragi (`iroha_core::sumeragi`)** NPoS-ga tayyor konsensus quvur liniyasini ko'rinishdagi o'zgarishlar bilan amalga oshiradi,
  ishonchli translyatsiya ma'lumotlari mavjudligi va sertifikatlar. ga qarang
  Tafsilotlar uchun [Sumeragi konsensus qoʻllanma](./sumeragi.md).
- **Kura (`iroha_core::kura`)** diskdagi kanonik bloklar, qayta tiklash yordamchilari va guvoh metama'lumotlarini saqlab qoladi.
- **World State View (`iroha_core::state`)** tekshirish uchun ishlatiladigan nufuzli xotiradagi suratni saqlaydi
  va so'rovlar.
- **Iroha Virtual Mashina (`ivm`)** Kotodama bayt kodini (`.to`) bajaradi va ko'rsatkich ABI siyosatini amalga oshiradi.
- **Norito (`crates/norito`)** har bir sim turi uchun deterministik ikkilik va JSON serializatsiyasini ta'minlaydi.
- **Telemetriya (`iroha_telemetry`)** Prometheus ko'rsatkichlari, tizimli jurnallar va oqim hodisalarini eksport qiladi.
- **P2P (`iroha_p2p`)** g'iybat, topologiya va tengdoshlar o'rtasidagi xavfsiz ulanishlarni boshqaradi.

### 2.2 Tarmoq va topologiya

Iroha tengdoshlari belgilangan holatdan olingan tartiblangan topologiyaga ega. Har bir konsensus raundida yetakchi tanlanadi,
tasdiqlash to'plami, proksi-server va B to'plami validatorlari. Tranzaktsiyalar Norito kodli xabarlar yordamida g'iybat qilinadi
Rahbar ularni taklif qilishdan oldin. Ishonchli eshittirish blokirovka qilish va qo'llab-quvvatlashni kafolatlaydi
dalillar barcha halol tengdoshlariga etib boradi, hatto tarmoq uzilishida ham ma'lumotlar mavjudligini ta'minlaydi. O'zgarishlarni ko'rish aylantiriladi
muddatlar o'tkazib yuborilganda etakchilik qiladi va majburiyat sertifikatlari har bir qabul qilingan blokning o'z zimmasiga olishini ta'minlaydi
barcha tengdoshlar tomonidan ishlatiladigan kanonik imzo to'plami.

### 2.3 Kriptografiya

`iroha_crypto` kassasi kalitlarni boshqarish, xeshlash va imzoni tekshirishni amalga oshiradi:- Ed25519 standart validator kalit sxemasi.
- Majburiy emas backends Secp256k1, TC26 GOST, BLS (jamlangan attestatsiyalar uchun) va ML-DSA yordamchilarini o'z ichiga oladi.
- Oqimli kanallar Ed25519 identifikatorlarini Kyber-ga asoslangan HPKE bilan Norito oqim seanslarini himoyalash uchun birlashtiradi.
- Barcha xeshlash tartiblari ish maydoni bilan deterministik ilovalardan (SHA-2, SHA-3, Bleyk2, Poseidon2) foydalanadi.
  auditlar `docs/source/crypto/dependency_audits.md` da hujjatlashtirilgan.

### 2.4 Streaming va amaliy ko'priklar

- **Norito oqim (`iroha_core::streaming`, `norito::streaming`)** deterministik, shifrlangan mediani taqdim etadi
  va seans suratlari, HPKE tugmachalarini aylantirish va telemetriya ilgaklari bilan ma'lumotlar kanallari. Kaigi konferentsiyasi va
  maxfiy dalillarni o'tkazish ushbu chiziqdan foydalanadi.
- **Ulanish ko'prigi (`connect_norito_bridge`)** platforma SDK-larini quvvatlaydigan C ABI sirtini ochib beradi
  (Swift, Kotlin/Android) kaput ostidagi Rust mijozlaridan qayta foydalanishda.
- **ISO 20022 ko'prigi (`iroha_torii::iso20022_bridge`)** tartibga solinadigan to'lov xabarlarini Norito ga o'zgartiradi
  bitimlar, konsensus yoki tasdiqlashni chetlab o'tmasdan moliyaviy ish oqimlari bilan o'zaro ishlash imkonini beradi.
- Barcha ko'priklar deterministik Norito foydali yuklarni saqlaydi, shuning uchun quyi oqim tizimlari holat o'tishlarini tekshirishi mumkin.

## 3. Ma'lumotlar modeli

`iroha_data_model` kassasi barcha daftar ob'ektlari, ko'rsatmalar, so'rovlar va hodisalarni belgilaydi. Diqqatga sazovor joylar:

- **Domenlar, hisoblar va aktivlar** kanonik I105 hisob identifikatorlaridan foydalanadi (afzal); `name@dataspace` / `name@domain.dataspace` marshrut sifatida qolmoqda
  aniq berilganda taxallus. Metadata deterministik (`Metadata` xaritasi). Raqamli aktivlar belgilangan nuqtani qo'llab-quvvatlaydi
  operatsiyalar; NFTlar o'zboshimchalik bilan tuzilgan metama'lumotlarni olib yuradi.
- **Rollar va ruxsatlar** Norito sanab o'tilgan tokenlardan foydalanadi, ular bevosita ijrochi tekshiruvlariga mos keladi.
- **Triggerlar** (vaqtga asoslangan, blokga asoslangan yoki predikatga asoslangan) zanjirdagi deterministik tranzaktsiyalarni chiqaradi
  ijrochi.
- **Voqealar** oqimi Torii orqali va bajarilgan holat oʻtishlarini, jumladan, maxfiy oqimlar va
  boshqaruv harakatlari.
- **Tranzaksiyalar, bloklar va manifestlar** Norito kodlangan (`SignedTransaction`, `SignedBlockWire`) bilan
  aniq versiya sarlavhalari, oldinga kengaytiriladigan dekodlashni ta'minlaydi.
- **Moslashtirish** ijrochi ma'lumotlar modeli orqali amalga oshiriladi: operatorlar maxsus ko'rsatmalarni ro'yxatdan o'tkazishi mumkin,
  determinizmni saqlagan holda ruxsatlar va parametrlar.
- **Repozitoriylar (`RepoInstruction`)** deterministik yangilash rejalarini (ijrochilar, manifestlar va) birlashtirishga imkon beradi.
  aktivlar) shuning uchun ko'p bosqichli ishlab chiqarishni boshqaruv roziligi bilan zanjirda boshqarish mumkin.
- **Konsensus artefaktlari** (masalan, majburiyat guvohnomalari va guvohlar roʻyxati) maʼlumotlar modelida va
  `iroha_core`, Torii va SDKlar o'rtasidagi muvofiqlikni kafolatlash uchun oltin testlar orqali ikki tomonlama sayohat.
- **Maxfiy registrlar va hodisalar** himoyalangan aktiv tavsiflovchilarini, tasdiqlovchi kalitlarni, majburiyatlarni,
  nullifiers va voqea yuklamalari (`ConfidentialEvent::{Shielded,Transferred,Unshielded}`) shuning uchun maxfiy oqimlar
  ochiq matn ma'lumotlarini sizdirmasdan tekshirilishi mumkin.

## 4. Tranzaksiyaning hayot aylanishi1. **Qabul qilish:** Torii Norito foydali yukini dekodlaydi, imzolar, TTL va o‘lcham chegaralarini tekshiradi, so‘ngra navbatni qo‘yadi.
   mahalliy tranzaktsiya.
2. **G'iybat:** Bitim topologiya bo'ylab tarqaladi; tengdoshlar xesh bo'yicha takrorlanadi va qabul qilishni takrorlaydi
   tekshiruvlar.
3. **Tanlash:** Joriy yetakchi kutilayotgan to‘plamdan tranzaktsiyalarni olib tashlaydi va fuqaroligi bo‘lmagan tekshiruvni amalga oshiradi.
4. **Statistik simulyatsiya:** Nomzod tranzaksiyalari vaqtinchalik `StateBlock` ichida, IVM yoki chaqiruv orqali amalga oshiriladi.
   o'rnatilgan ko'rsatmalar. To'qnashuvlar yoki qoidalarning buzilishi qat'iy ravishda bekor qilinadi.
5. **Triggerni amalga oshirish:** Raundda to'lanishi kerak bo'lgan rejalashtirilgan triggerlar ichki tranzaktsiyalarga aylantiriladi.
   va bir xil quvur liniyasi yordamida tasdiqlangan.
6. **Taklifni muhrlash:** Blok chegaralariga erishilganda yoki vaqt tugashi bilan yetakchi Norito kodlangan signal chiqaradi
   `BlockCreated` xabari.
7. **Tasdiqlash:** Tekshiruv to'plamidagi tengdoshlar fuqaroligi yo'qligi/statistik tekshiruvlarni qayta o'tkazadi. Muvaffaqiyatli tengdoshlar belgisi
   `BlockSigned` xabarlarini yuboring va ularni deterministik kollektor to'plamiga yuboring.
8. **Tasdiqlash:** Kollektor kanonik imzolar to'plamini yig'gandan so'ng majburiyat sertifikatini yig'adi,
   `BlockCommitted` translyatsiyasini amalga oshiradi va blokni mahalliy darajada yakunlaydi.
9. **Ilova:** Barcha tengdoshlar blokni Kurada yozib oladi, holat yangilanishlarini qo'llaydi, telemetriya/hodisalar chiqaradi, tozalash
   mempuldan tranzaktsiyalarni amalga oshiring va topologiya rollarini aylantiring.

Qayta tiklash yo'llari etishmayotgan bloklarni qayta uzatish uchun deterministik translyatsiyadan foydalanadi va o'zgarishlarni ko'rish etakchilikni aylantiradi
muddatlar tugashi bilan. Sidecars va telemetriya konsensus natijalarini o'zgartirmasdan diagnostika tushunchalarini beradi.

## 5. Aqlli shartnomalar va ijro

Smart kontraktlar Iroha virtual mashinasida (IVM) ishlaydi:

- **Kotodama** yuqori darajali `.ko` manbalarini deterministik `.to` bayt kodiga kompilyatsiya qiladi.
- **Pointer ABI enforement** shartnomalarning tasdiqlangan ko'rsatkich turlari orqali xost xotirasi bilan o'zaro ta'sirini ta'minlaydi.
  Syscall sirtlari `ivm/docs/syscalls.md` da tasvirlangan; ABI ro'yxati xeshlangan va versiyalangan.
- **Syscalls va hosts** daftar holatiga kirishni, ishga tushirishni rejalashtirishni, maxfiy primitivlarni, Kaigi mediasini qamrab oladi.
  oqimlar va deterministik tasodifiylik.
- **O'rnatilgan ijrochi** aktivlar, hisoblar, ruxsatlar,
  va boshqaruv operatsiyalari. Maxsus ijrochilar Norito sxemalariga rioya qilgan holda ko'rsatmalar to'plamini kengaytirishi mumkin.
- **Maxfiy funksiyalar**, shu jumladan himoyalangan o‘tkazmalar va tasdiqlovchi registrlar ijrochi orqali ochiladi.
  ko'rsatmalar va Poseidon majburiyatlari bilan xostlar tomonidan tasdiqlangan.

## 6. Saqlash va doimiylik- **Kura bloklari do'koni** har bir yakunlangan blokni Norito sarlavhasi bilan `SignedBlockWire` foydali yuk sifatida yozadi.
  kanonik sarlavhalar, tranzaktsiyalar, sertifikatlar va guvohlar ma'lumotlari birgalikda.
- **World State View** tezkor so'rovlar uchun vakolatli holatni xotirada saqlaydi. Deterministik suratlar va
  quvur liniyasi yonbag'irlari (`pipeline/sidecars.norito` + `pipeline/sidecars.index`) tiklash va auditlarni qo'llab-quvvatlaydi.
- **Shtat darajasi** deterministikni saqlab, katta joylashtirishlar uchun issiq/sovuq qismlarga ajratish imkonini beradi.
  tasdiqlash.
- **Sinxronlash va qayta o'ynatish** Xuddi shu tekshirish qoidalaridan foydalangan holda bajarilgan bloklarni holatiga qaytaring. Deterministik
  translyatsiya tengdoshlarning ishonchli xotiraga tayanmasdan qo'shnilaridan etishmayotgan ma'lumotlarni qayta tiklashini ta'minlaydi.

## 7. Boshqaruv va iqtisodiyot

- Zanjirdagi parametrlar (`SetParameter`) nazorat konsensus taymerlari, mempul limitlari, telemetriya tugmalari, to'lov diapazonlari,
  va xususiyatli bayroqlar. `kagami` tomonidan yaratilgan Genesis manifestlari dastlabki konfiguratsiyani o'rnatadi.
- **Kaigi** ko'rsatmalari hamkorlikdagi seanslarni boshqaradi (yaratish/qo'shilish/tashlash/tugatish) va Norito oqimini ta'minlaydi
  konferentsiyadan foydalanish holatlari uchun telemetriya.
- **Hijiri** deterministik tengdosh va hisob obro'sini ta'minlaydi, konsensus, qabul qilish bilan birlashadi
  siyosatlar va to'lov multiplikatorlari (Q16-qismli matematika). Dalillar, nazorat nuqtalari va obro'si
  registrlar zanjirda tuzilgan va kuzatuvchi profillari kvitansiya kelib chiqishini boshqaradi.
- **NPoS rejimi** (yoqilganda) VRF tomonidan qo‘llab-quvvatlanadigan saylov oynalari va stavkalar bo‘yicha qo‘mitalardan foydalanadi.
  deterministik konfiguratsiya standartlari.
- **Maxfiy registrlar** nol ma'lumotga ega bo'lmagan tasdiqlovchi kalitlarni, isbotning ishlash davrlarini va majburiyatlarni boshqaradi.
  himoyalangan oqimlar.

## 8. Mijoz tajribasi va asboblar

- **Torii API** tranzaktsiyalar, so'rovlar, voqealar oqimi, telemetriya va boshqalar uchun REST va WebSocket interfeyslarini taklif qiladi.
  boshqaruvning yakuniy nuqtalari. JSON proyeksiyalari Norito sxemalaridan olingan.
- **CLI asboblari** (`iroha_cli`, `iroha_monitor`) boshqaruv, jonli tengdosh asboblar paneli va quvur liniyasini qamrab oladi
  tekshirish.
- **Genesis asboblari** (`kagami`) Norito kodli manifestlarni, validator kalit materialini va konfiguratsiyani yaratadi
  andozalar.
- **SDKs** (Swift, JS/TS, Python) ko'rsatmalar, so'rovlar, triggerlar va telemetriyaga idiomatik kirishni ta'minlaydi.
- `scripts/` ichidagi **skriptlar va CI ilgaklari** asboblar panelini tekshirish, kodekni qayta tiklash va tutunni avtomatlashtiradi
  testlar.

## 9. Ishlash, chidamlilik va yo'l xaritasi- Joriy quvur liniyasi qulay tarmoq ostida **2–3 soniya** blokirovka qilish vaqtlari bilan **20 000 ts ** ni mo'ljallamoqda.
  To'plam imzosini tekshirish va deterministik rejalashtirish bilan ta'minlangan shartlar.
- **Telemetriya** konsensus taymerlari, mempuldagi bandlik, blok tarqalishi salomatligi,
  Kaigi foydalanish va Hijiri obro'si yangilanadi.
- **Mustaqillik xususiyatlari** deterministik ma'lumotlarning mavjudligi, tiklash yonboshlari, topologiyani aylantirish va
  konfiguratsiya qilinadigan ko'rish/o'zgartirish chegaralari.
- Kelajakdagi yo'l xaritasi bosqichlari (qarang: `roadmap.md`) Nexus ma'lumotlar bo'shliqlarida ishlashni davom ettiradi, kengaytirilgan maxfiylik
  asboblar va deterministik natijalarni saqlab qolgan holda kengroq apparat tezlashuvi.

## 10. Operatsiyalar va joylashtirish

- **Artifaktlar:** Dockerfiles, Nix flake va `cargo` ish oqimlari qayta tiklanadigan tuzilmalarni qo'llab-quvvatlaydi. `kagami` chiqaradi
  genezis manifestlari, validator kalitlari va ruxsat etilgan va NPoS o'rnatish uchun misol konfiguratsiyalari.
- **O'z-o'zidan joylashtirilgan tarmoqlar:** Operatorlar o'zlarining tengdoshlari to'plamlarini, qabul qilish qoidalarini va kadansni yangilashni boshqaradilar. The
  ish maydoni koordinatsiyasiz birgalikda mavjud bo'lgan, faqat bir-biriga ulashgan ko'plab mustaqil Iroha 2 tarmoqlarini qo'llab-quvvatlaydi.
  yuqori oqim kodi.
- **Konfiguratsiyaning ishlash davri:** `iroha_config` foydalanuvchi → haqiqiy → standart qatlamlarni hal qiladi, bu esa har bir tugmaning ishlashini ta'minlaydi.
  aniq va versiya tomonidan boshqariladigan. Ish vaqti o'zgarishlari `SetParameter` ko'rsatmalari orqali amalga oshiriladi.
- **Kuzatilishi:** `iroha_telemetry` Prometheus koʻrsatkichlari, tuzilgan jurnallar va nazorat paneli maʼlumotlarini eksport qiladi
  CI skriptlari boʻyicha (`ci/check_swift_dashboards.sh`, `scripts/render_swift_dashboards.sh`,
  `scripts/check_swift_dashboard_data.py`). Streaming, konsensus va hijiriy voqealar ustidan mavjud
  WebSocket va `scripts/sumeragi_backpressure_log_scraper.py` yurak stimulyatori orqa bosimini o'zaro bog'laydi.
  muammolarni bartaraf etish uchun telemetriya.
- **Test:** `cargo test --workspace`, integratsiya testlari (`integration_tests/`), til SDK to'plamlari va
  Norito oltin armatura determinizmni himoya qiladi. Pointer ABI, syscall ro'yxatlari va boshqaruv manifestlarida mavjud
  maxsus oltin testlar.
- **Qayta tiklash:** Kura yonboshlagichlari, deterministik takrorlash va translyatsiyani sinxronlash tugunlarning diskdagi holatini tiklashga imkon beradi.
  yoki tengdoshlar. Hijiriy nazorat punktlari va boshqaruv manifestlari muvofiqlik uchun tekshiriladigan oniy suratlarni taqdim etadi.

# Lug'at

Ushbu hujjatda ko'rsatilgan terminologiya uchun quyidagi manzildagi loyiha bo'yicha lug'atga murojaat qiling
.