---
lang: uz
direction: ltr
source: docs/source/crypto/sm_compliance_brief.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 73f5ca7a7484a26e901102dd6950b7110a18e7fa215a46540c7189c919e0958f
source_last_modified: "2025-12-29T18:16:35.942266+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM2/SM3/SM4 muvofiqligi va eksport haqida qisqacha ma'lumot

Bu qisqacha `docs/source/crypto/sm_program.md` arxitektura eslatmalarini to'ldiradi
va muhandislik, Ops va yuridik guruhlar uchun amaliy ko'rsatmalar beradi
GM/T algoritmlar oilasi faqat tekshirish uchun oldindan koʻrishdan kengroq faollashtirishga oʻtadi.

## Xulosa
- **Me'yoriy asos:** Xitoyning *Kriptografiya qonuni* (2019), *Kiberxavfsizlik qonuni* va
  *Maʼlumotlar xavfsizligi toʻgʻrisidagi qonun* SM2/SM3/SM4-ni ishga tushirilganda “tijorat kriptografiyasi” sifatida tasniflaydi
  quruqlikda. Operatorlar foydalanish hisobotlarini topshirishlari kerak va ba'zi sektorlar akkreditatsiyani talab qiladi
  ishlab chiqarishdan oldin sinovdan o'tkazish.
- **Xalqaro nazorat:** Xitoydan tashqarida algoritmlar AQSh EAR toifasiga kiradi.
  5 2-qism, EI 2021/821 1-ilova (5D002) va shunga o‘xshash milliy rejimlar. Ochiq manba
  nashr odatda litsenziya istisnolari (ENC/TSU), lekin ikkilik uchun mos keladi
  embargo qilingan hududlarga yuborilgan eksport nazorat ostida qoladi.
- **Loyiha siyosati:** SM funksiyalari sukut bo‘yicha o‘chirilgan bo‘lib qoladi. Imzolash funksiyasi
  faqat tashqi audit yopilgandan so'ng yoqiladi, deterministik perf/temetriya
  gating, va operator hujjatlari (bu qisqacha) er.

## Funktsiya bo'yicha talab qilinadigan harakatlar
| Jamoa | Mas'uliyat | Artefaktlar | Egalari |
|------|------------------|-----------|--------|
| Crypto WG | GM/T spetsifikatsiyalari yangilanishlarini kuzatib boring, uchinchi tomon auditlarini muvofiqlashtiring, deterministik siyosatni saqlang (bir martalik hosil bo'lmagan, kanonik r∥s). | `sm_program.md`, audit hisobotlari, armatura to'plamlari. | Crypto WG etakchi |
| Release Engineering | Aniq konfiguratsiya ortidagi Gate SM xususiyatlari, faqat tasdiqlash uchun standartni saqlab qo'ying, xususiyatlarni ishga tushirish nazorat ro'yxatini boshqaring. | `release_dual_track_runbook.md`, reliz manifestlari, chiqish chiptasi. | TLni chiqaring |
| Ops / SRE | SMni yoqish nazorat ro'yxatini, telemetriya asboblar panelini (foydalanish, xatolik stavkalari), hodisalarga javob berish rejasini taqdim eting. | Runbooks, Grafana asboblar paneli, bortga chiqish chiptalari. | Ops/SRE |
| Yuridik aloqa | Tugunlar materik Xitoyda ishlaganda XXR ishlab chiqish/foydalanish hisobotlarini fayl; har bir to'plam uchun eksport holatini ko'rib chiqing. | Shablonlar, eksport bayonotlari. | Yuridik aloqa |
| SDK dasturi | Surface SM algoritmi doimiy ravishda qo'llab-quvvatlanadi, deterministik xatti-harakatlarni amalga oshiradi, SDK hujjatlariga muvofiqlik eslatmalarini tarqatadi. | SDK reliz yozuvlari, hujjatlar, CI gating. | SDK yetakchilari |## Hujjatlar va topshirish talablari (Xitoy)
1. **Mahsulotni topshirish (xàngàngìní):** Quruqlikda ishlab chiqish uchun mahsulot tavsifini yuboring,
   manba mavjudligi haqidagi bayonot, qaramlik ro'yxati va deterministik qurish bosqichlari
   ozod qilishdan oldin viloyat kriptografiya ma'muriyati.
2. **Sotish/Foydalanish uchun ariza berish (yànghàng/yàngìní):** SM-ni yoqadigan tugunlarni boshqaradigan operatorlar
   foydalanish doirasi, kalitlarni boshqarish va telemetriya to'plamini xuddi shu bilan ro'yxatdan o'tkazing
   hokimiyat. Aloqa ma'lumotlarini va hodisaga javob berish SLAlarini taqdim eting.
3. **Sertifikatlash (hìngí/yíní):** Muhim infratuzilma operatorlari talab qilishi mumkin
   akkreditatsiyalangan sinov. Qayta tiklanadigan skriptlarni, SBOM va test hisobotlarini taqdim eting
   shuning uchun quyi oqim integratorlari kodni o'zgartirmasdan sertifikatlashni yakunlashlari mumkin.
4. **Hisob yuritish:** Arxiv arizalari va tasdiqlarni muvofiqlik kuzatuvchisida saqlash.
   Yangi hududlar yoki operatorlar jarayonni tugatgandan so'ng `status.md` ni yangilang.

## Muvofiqlikni tekshirish ro'yxati

### SM funksiyalarini yoqishdan oldin
- [ ] Yuridik maslahatchi maqsadli joylashtirish hududlarini ko'rib chiqqanini tasdiqlang.
- [ ] Deterministik qurish ko'rsatmalari, qaramlik manifestlari va SBOMni yozib oling
      hujjatlar bilan birga kiritish uchun eksport.
- [ ] `crypto.allowed_signing`, `crypto.default_hash` va kirishni tekislang
      siyosat tarqatish chiptasi bilan namoyon bo'ladi.
- [ ] SM funksiyalarini tavsiflovchi operator aloqalarini yaratish,
      yoqish uchun zarur shartlar va o'chirish uchun zaxira rejalari.
- [ ] SM tasdiqlash/imzo hisoblagichlarini qamrab oluvchi telemetriya asboblar panelini eksport qilish,
      xato stavkalari va mukammal ko'rsatkichlar (`sm3`, `sm4`, tizim chaqiruvi vaqti).
- [ ] Hodisalarga javob berish aloqalarini va quruqlik uchun kuchayish yo'llarini tayyorlang
      operatorlari va Crypto WG.

### Hujjat topshirish va auditga tayyorlik
- [ ] Tegishli fayl shablonini tanlang (mahsulot va sotish/foydalanish) va toʻldiring
      yuborishdan oldin reliz metama'lumotlarida.
- [ ] SBOM arxivlarini, deterministik test transkriptlarini va manifest xeshlarini biriktiring.
- [ ] Eksport-nazorat bayonnomasi mavjud artefaktlarni aniq aks ettirishiga ishonch hosil qiling
      yetkazib berilgan va asoslangan litsenziya istisnolarini keltirgan (ENC/TSU).
- [ ] Audit hisobotlari, tuzatishni kuzatish va operatorning ish kitoblari ekanligini tekshiring
      fayl paketidan bog'langan.
- [ ] Imzolangan arizalar, tasdiqlashlar va yozishmalarni muvofiqlikda saqlang
      versiyali havolalar bilan kuzatuvchi.

### Tasdiqlashdan keyingi operatsiyalar
- [ ] Ariza qabul qilingandan keyin `status.md` va chiqish chiptasini yangilang.
- [ ] Kuzatish imkoniyati qamrovi mos kelishini tasdiqlash uchun telemetriya tekshiruvini qayta ishga tushiring
      fayl kiritishlari.
- [ ] Arizalarni, audit hisobotlarini davriy (kamida yiliga) ko'rib chiqishni rejalashtirish;
      va spetsifikatsiya/tartibga solish yangilanishlarini olish uchun eksport bayonotlari.
- [ ] Konfiguratsiya, xususiyat doirasi yoki xostingda fayl qoʻshimchalarini ishga tushiring
      izi sezilarli darajada o'zgaradi.## Eksport va tarqatish bo'yicha qo'llanma
- Ishonchlilik haqida qisqacha eksport bayonotini chiqarish eslatmalariga/manifestlariga qo'shing
  ENC/TSU da. Misol:
  > "Ushbu nashr SM2/SM3/SM4 ilovalarini o'z ichiga oladi. Tarqatish ENCdan keyin
  > (15 CFR-qism 742) / EI 2021/821 1-ilova 5D002. Operatorlar muvofiqlikni ta'minlashi kerak
  > mahalliy eksport/import qonunlari bilan.”
- Xitoy ichida joylashgan qurilishlar uchun, artefaktlarni nashr qilish uchun Ops bilan muvofiqlashtiring
  quruqlikdagi infratuzilma; SM-ni yoqadigan ikkilik fayllarni transchegaraviy uzatishdan saqlaning
  tegishli litsenziyalar mavjud.
- Paket omborlarini aks ettirishda qaysi artefaktlar SM funksiyalarini o'z ichiga olganligini yozib oling
  muvofiqlik hisobotini soddalashtirish.

## Operator nazorat ro'yxati
- [ ] Chiqarish profilini tasdiqlang (`scripts/select_release_profile.py`) + SM xususiyati belgisi.
- [ ] `sm_program.md` va ushbu qisqacha sharhni ko'rib chiqing; huquqiy hujjatlar ro'yxatga olinishini ta'minlash.
- [ ] `sm` bilan kompilyatsiya qilish, `crypto.allowed_signing`-ni `sm2`-ni o'z ichiga olish uchun yangilash va `crypto.default_hash`-ni `sm3-256` ga almashtirish orqali SM xususiyatlarini yoqing.
- [ ] SM hisoblagichlarini kiritish uchun telemetriya asboblar paneli/ogohlantirishlarini yangilang (tasdiqlash xatosi,
      imzolash so'rovlari, mukammal ko'rsatkichlar).
- [ ] Manifestlar, xesh/imzo dalillari va hujjat topshirish tasdiqlarini biriktirilgan holda saqlang
      chiqish chiptasi.

## Namuna topshirish shablonlari

Shablonlar osongina qo'shilishi uchun `docs/source/crypto/attachments/` ostida ishlaydi
paketlarni topshirish. Tegishli Markdown shablonini operator o'zgarishiga nusxalash
mahalliy hokimiyat talabiga binoan jurnalga kiring yoki uni PDF-ga eksport qiling.

- [`sm_product_filing_template.md`](attachments/sm_product_filing_template.md) -
  provinsiyaviy mahsulot fayli (xàngàngìnì) nashr metama'lumotlarini, algoritmlarni,
  SBOM ma'lumotnomalari va qo'llab-quvvatlash kontaktlari.
- [`sm_sales_usage_filing_template.md`](attachments/sm_sales_usage_filing_template.md) —
  joylashtirish bo'yicha izni ko'rsatuvchi operator sotuvi/foydalanish to'g'risidagi arizasi (yàngàng/yàngàngàng),
  asosiy boshqaruv, telemetriya va hodisalarga javob berish tartib-qoidalari.
- [`sm_export_statement_template.md`](attachments/sm_export_statement_template.md) -
  e'lon qilish eslatmalari, manifest yoki qonuniy uchun mos eksport-nazorat deklaratsiyasi
  ENC/TSU litsenziyasi istisnolariga asoslangan yozishmalar.## Standartlar va iqtiboslar
- **GM/T 0002-2012 / GB/T 32907-2016** — SM4 blok shifr va AEAD parametrlari (ECB/GCM/CCM). `docs/source/crypto/sm_vectors.md` da olingan vektorlarga mos keladi.
- **GM/T 0003-2012 / GB/T 32918.x-2016** — SM2 ochiq kalitli kriptografiya, egri parametrlar, imzo/tasdiqlash jarayoni va D-ilovadagi maʼlum javob testlari.
- **GM/T 0004-2012 / GB/T 32905-2016** — SM3 xesh funksiyasi spetsifikatsiyasi va muvofiqlik vektorlari.
- **RFC 8998** — TLSda SM2 kalit almashinuvi va imzodan foydalanish; OpenSSL/Tongsuo bilan o'zaro ishlashni hujjatlashda keltiring.
- **Xitoy Xalq Respublikasining kriptografiya qonuni (2019)**, **Kiberxavfsizlik to‘g‘risidagi qonun (2017)**, **Ma’lumotlar xavfsizligi to‘g‘risidagi qonun (2021)** — Yuqorida qayd etilgan hujjatlarni topshirish jarayoni uchun huquqiy asos.
- **US EAR toifasi 5           ** va ** Yevropa Ittifoqi qoidalari 2021/821 ilova 1 (5D002)** — SM-ni qo'llab-quvvatlaydigan ikkilik fayllarni boshqaruvchi eksport-nazorat rejimlari.
- **Iroha artefaktlari:** `scripts/sm_interop_matrix.sh` va `scripts/sm_openssl_smoke.sh` auditorlar muvofiqlik hisobotlarini imzolashdan oldin takrorlashi mumkin bo'lgan deterministik interop transkriptlarini taqdim etadi.

## Ma'lumotnomalar
- `docs/source/crypto/sm_program.md` — texnik arxitektura va siyosat.
- `docs/source/release_dual_track_runbook.md` - chiqarish shlyuzi va ishlab chiqarish jarayoni.
- `docs/source/sora_nexus_operator_onboarding.md` - namunaviy operatorni ishga tushirish oqimi.
- GM/T 0002-2012, GM/T 0003-2012, GM/T 0004-2012, GB/T 32918 seriyali, RFC 8998.

Savollar? SM tarqatish trekeri orqali Crypto WG yoki yuridik aloqa bilan bog'laning.