---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 963c22085107a828fb801f1cdbcef3745e975431f7168531688f4c8d487cf2b3
source_last_modified: "2026-01-05T09:28:11.843792+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-lane-model
title: Nexus lane model
description: Logical lane taxonomy, configuration geometry, and world-state merge rules for Sora Nexus.
translator: machine-google-reviewed
---

# Nexus Lane modeli va WSV bo'limlari

> **Holat:** NX-1 yetkazib berilishi mumkin — qator taksonomiyasi, konfiguratsiya geometriyasi va saqlash sxemasi amalga oshirishga tayyor.  
> **Egalari:** Nexus Asosiy WG, Governance WG  
> **Yo‘l xaritasi ma’lumotnomasi:** `roadmap.md` da NX-1

Ushbu portal sahifasi kanonik `docs/source/nexus_lanes.md` qisqacha ma'lumotlarini aks ettiradi, shuning uchun Sora
Nexus operatorlari, SDK egalari va sharhlovchilar yoʻnalish boʻyicha koʻrsatmalarni oʻqishi mumkin.
mono-repo daraxtiga sho'ng'ish. Maqsadli arxitektura dunyo holatini saqlaydi
alohida ma'lumotlar bo'shliqlari (yo'laklar) umumiy foydalanishga ruxsat berish paytida deterministik yoki
izolyatsiyalangan ish yuklari bilan shaxsiy validator to'plamlari.

## tushunchalar

- **Line:** Nexus daftarining mantiqiy bo'lagi, o'zining validator to'plami va
  bajarilish orqasida. Barqaror `LaneId` tomonidan aniqlangan.
- **Maʼlumotlar maydoni:** Birgalikda boʻlgan bir yoki bir nechta yoʻlaklarni guruhlaydigan boshqaruv paqiri
  muvofiqlik, marshrutlash va hisob-kitob siyosatlari.
- **Lane Manifest:** Validatorlarni tavsiflovchi boshqaruv tomonidan boshqariladigan metamaʼlumotlar, DA
  siyosat, gaz tokeni, hisob-kitob qoidalari va marshrut ruxsatnomalari.
- **Global majburiyat:** Yangi shtat ildizlarini umumlashtiruvchi chiziq tomonidan chiqarilgan dalil,
  hisob-kitob ma'lumotlari va ixtiyoriy ko'ndalang bo'lakli o'tkazmalar. Global NPoS halqasi
  buyurtma majburiyatlari.

## Lane taksonomiyasi

Yo'lak turlari kanonik tarzda ularning ko'rinishini, boshqaruv yuzasini va
turar-joy ilgaklari. Konfiguratsiya geometriyasi (`LaneConfig`) bularni qamrab oladi
atributlar, shuning uchun tugunlar, SDK'lar va asboblar joylashuvsiz tartib haqida fikr yuritishi mumkin
moslashtirilgan mantiq.

| Yo'lak turi | Ko'rinish | Validator a'zoligi | WSV ta'siri | Standart boshqaruv | Hisob-kitob siyosati | Oddiy foydalanish |
|----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | ommaviy | Ruxsatsiz (global ulush) | To'liq holat nusxasi | SORA Parlament | `xor_global` | Asosiy ommaviy kitob |
| `public_custom` | ommaviy | Ruxsatsiz yoki qoziqli | To'liq holat nusxasi | Qoziq o'lchovli modul | `xor_lane_weighted` | Yuqori samarali ommaviy ilovalar |
| `private_permissioned` | cheklangan | Ruxsat etilgan validator to'plami (boshqaruv tasdiqlangan) | Majburiyatlar va dalillar | Federatsiya Kengashi | `xor_hosted_custody` | CBDC, konsortsium ish yuklari |
| `hybrid_confidential` | cheklangan | Aralash a'zolik; ZK dalillarini o'rab oladi | Majburiyatlar + tanlab oshkor qilish | Dasturlashtiriladigan pul moduli | `xor_dual_fund` | Maxfiylikni saqlaydigan dasturlashtiriladigan pul |

Barcha yo'lak turlari e'lon qilinishi kerak:

- Ma'lumotlar maydoni taxalluslari - muvofiqlik siyosatlarini bog'laydigan odamlar tomonidan o'qiladigan guruhlash.
- Boshqaruv dastagi — `Nexus.governance.modules` orqali hal qilingan identifikator.
- Hisob-kitob dastagi — XOR debetiga hisob-kitob marshrutizatori tomonidan iste'mol qilinadigan identifikator
  buferlar.
- Qo'shimcha telemetriya metama'lumotlari (tavsif, aloqa, biznes domeni) paydo bo'ldi
  `/status` va asboblar paneli orqali.

## Yo'lak konfiguratsiyasi geometriyasi (`LaneConfig`)

`LaneConfig` - tasdiqlangan chiziqli katalogdan olingan ish vaqti geometriyasi. Bu
boshqaruv manifestlarini **o'zgartirmaydi**; Buning o'rniga u deterministik beradi
har bir sozlangan qator uchun saqlash identifikatorlari va telemetriya maslahatlari.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` har doim konfiguratsiya bo'lganda geometriyani qayta hisoblaydi
  yuklangan (`State::set_nexus`).
- taxalluslar kichik shlaklarga sanitarlanadi; ketma-ket alfanumerik bo'lmagan
  belgilar `_` ga tushadi. Agar taxallus bo'sh slug'ni keltirsa, biz orqaga tushamiz
  `lane{id}` gacha.
- `shard_id` katalog metadata kalitidan olingan `da_shard_id` (standart
  `lane_id` ga) va DA takrorini davom ettirish uchun doimiy parcha kursor jurnalini boshqaradi
  qayta ishga tushirish/qayta taqsimlash bo'yicha deterministik.
- Kalit prefikslari WSV ning har bir qatordagi kalit diapazonlarini bir-biridan ajratib turishini ta'minlaydi
  bir xil backend baham ko'riladi.
- Kura segment nomlari xostlar bo'yicha deterministikdir; auditorlar o'zaro tekshirishlari mumkin
  segment kataloglari va manifestlari maxsus asboblarsiz.
- Birlashtirish segmentlari (`lane_{id:03}_merge`) so'nggi birlashma-ko'rsatma ildizlariga ega va
  ushbu yo'lak bo'yicha global davlat majburiyatlari.

## Dunyo davlatlarining bo'linishi

- mantiqiy Nexus dunyo holati - bu chiziqli holat bo'shliqlarining birlashmasi. Ommaviy
  yo'llar to'liq holatda qoladi; xususiy/maxfiy yo'llar eksport Merkle / majburiyat
  birlashma kitobining ildizlari.
- MV saqlash prefikslari har bir kalitdan 4 baytlik chiziqli prefiksga ega
  `LaneConfigEntry::key_prefix`, `[00 00 00 01] ++ kabi kalitlarni beradi
  PackedKey`.
- Umumiy jadvallar (hisoblar, aktivlar, triggerlar, boshqaruv yozuvlari) shuning uchun saqlanadi
  qator prefiksi bo'yicha guruhlangan yozuvlar diapazonni skanerlashda deterministik bo'ladi.
- Birlashtirish daftarining metama'lumotlari bir xil tartibni aks ettiradi: har bir qatorda birlashma maslahati yoziladi
  ildizlar va `lane_{id:03}_merge` ga qisqartirilgan global holat ildizlari, imkon beradi
  maqsadli ushlab turish yoki yo'lak iste'foga chiqqanda ko'chirish.
- O'zaro faoliyat indekslari (hisob taxalluslari, aktivlar registrlari, boshqaruv manifestlari)
  aniq chiziqli prefikslarni saqlang, shunda operatorlar yozuvlarni tezda moslashtirishi mumkin.
- **Saqlash siyosati** — umumiy yo'laklar to'liq blok jismlarini saqlab qoladi; faqat majburiyat
  yo'llar nazorat punktlaridan keyin eski jismlarni ixchamlashtirishi mumkin, chunki majburiyatlar mavjud
  vakolatli. Maxfiy yo'llar shifrlangan matnli jurnallarni ajratilgan holda saqlaydi
  boshqa ish yuklarini blokirovka qilmaslik uchun segmentlar.
- **Tooling** — texnik xizmat ko'rsatish dasturlari (`kagami`, CLI administrator buyruqlari)
  ko'rsatkichlar, Prometheus teglari yoki
  Kura segmentlarini arxivlash.

## Marshrutlash va API

- Torii REST/gRPC so'nggi nuqtalari ixtiyoriy `lane_id` ni qabul qiladi; yo'qligi nazarda tutadi
  `lane_default`.
- SDK-larning sirt yo'laklarini tanlash moslamalari va foydalanuvchilarga qulay taxalluslarni `LaneId`-ga xaritalash.
  yo'lak katalogi.
- Marshrutlash qoidalari tasdiqlangan katalogda ishlaydi va ikkala qatorni ham tanlashi mumkin
  ma'lumotlar maydoni. `LaneConfig` asboblar paneli va telemetriya uchun qulay taxalluslarni taqdim etadi.
  jurnallar.

## Hisob-kitob va to'lovlar

- Har bir chiziq global validator to'plamiga XOR to'lovlarini to'laydi. Yo'laklar mahalliyni to'plashi mumkin
  gaz tokenlari, lekin majburiyatlar bilan bir qatorda XOR ekvivalentlarini saqlashi kerak.
- Hisob-kitoblarni tasdiqlovchi hujjatlar miqdori, konvertatsiya metama'lumotlari va depozitni tasdiqlovchi hujjatlarni o'z ichiga oladi
  (masalan, global to'lov kassasiga o'tkazish).
- Yagona hisob-kitob marshrutizatori (NX-3) bir xil chiziqdan foydalangan holda buferlarni debet qiladi
  prefikslar, shuning uchun hisob-kitob telemetriyasi saqlash geometriyasiga mos keladi.

## Boshqaruv

- Yo'llar o'zlarining boshqaruv modullarini katalog orqali e'lon qiladilar. `LaneConfigEntry`
  telemetriya va audit izlarini saqlash uchun asl taxallus va slugni olib yuradi
  o'qilishi mumkin.
- Nexus reestri imzolangan chiziqli manifestlarni tarqatadi.
  `LaneId`, maʼlumotlar maydonini bogʻlash, boshqaruv dastagi, hisob-kitob dastagi va
  metadata.
- Ish vaqtini yangilash ilgaklari boshqaruv siyosatini qo'llashda davom etmoqda
  (Sukut bo'yicha `gov_upgrade_id`) va jurnallar telemetriya ko'prigi orqali farqlanadi
  (`nexus.config.diff` hodisalari).

## Telemetriya va holat

- `/status` qator taxalluslari, maʼlumotlar maydoni bogʻlashlari, boshqaruv tutqichlari va
  Katalog va `LaneConfig` dan olingan hisob-kitob profillari.
- Rejalashtiruvchi ko'rsatkichlari (`nexus_scheduler_lane_teu_*`) shunday qilib chiziq taxalluslari/slug'larini ko'rsatadi
  operatorlar orqada qolish va TEU bosimini tezda xaritalashlari mumkin.
- `nexus_lane_configured_total` olingan chiziqli yozuvlar sonini hisoblaydi va
  konfiguratsiya o'zgarganda qayta hisoblab chiqiladi. Telemetriya har doim imzolangan farqlarni chiqaradi
  chiziq geometriyasi o'zgaradi.
- Ma'lumotlar bo'shlig'i zaxiralari ko'rsatkichlari yordam berish uchun taxallus/ta'rif metama'lumotlarini o'z ichiga oladi
  operatorlar navbat bosimini biznes domenlari bilan bog'laydilar.

## Konfiguratsiya va Norito turlari

- `LaneCatalog`, `LaneConfig` va `DataSpaceCatalog` yashaydi
  `iroha_data_model::nexus` va Norito formatidagi tuzilmalarni taqdim eting.
  manifestlar va SDKlar.
- `LaneConfig` `iroha_config::parameters::actual::Nexus` da yashaydi va olingan
  avtomatik ravishda katalogdan; u Norito kodlashni talab qilmaydi, chunki u
  ichki ish vaqti yordamchisidir.
- Foydalanuvchi uchun konfiguratsiya (`iroha_config::parameters::user::Nexus`)
  deklarativ yo'lak va ma'lumotlar maydoni deskriptorlarini qabul qilishni davom ettiradi; hozir tahlil qilish
  geometriyani chiqaradi va noto'g'ri taxalluslar yoki takroriy qator identifikatorlarini rad etadi.

## Ajoyib ish

- XOR buferi uchun hisob-kitob marshrutizatori yangilanishlarini (NX-3) yangi geometriya bilan birlashtiring
  debet va tushumlar qatorli slug bilan belgilanadi.
- Ustunlar oilalari ro'yxati, ixcham nafaqaga chiqqan qatorlar va ro'yxat uchun administrator vositalarini kengaytiring
  slugged nom maydonidan foydalanib, har bir qator bloklari jurnallarini tekshiring.
- Birlashtirish algoritmini yakunlash (tartib berish, kesish, ziddiyatni aniqlash) va
  o'zaro chiziqli takrorlash uchun regressiya moslamalarini biriktiring.
- Oq ro'yxatlar/qora ro'yxatlar va dasturlashtiriladigan pullar uchun muvofiqlik ilgaklarini qo'shing
  siyosatlar (NX-12 ostida kuzatilgan).

---

*Ushbu sahifa NX-1 kuzatuvlarini NX-2 dan NX-18 ga qadar kuzatishda davom etadi.
Iltimos, ochiq savollarni `roadmap.md` yoki boshqaruv kuzatuvchisiga yozing.
portal kanonik hujjatlarga mos keladi.*