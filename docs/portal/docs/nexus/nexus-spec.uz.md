---
lang: uz
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c01eb9c61f61a550dcfa542d29beedb5aa6554e5e0fa8f776f72949d8044843c
source_last_modified: "2026-01-05T09:28:11.846525+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-spec
title: Sora Nexus technical specification
description: Full mirror of `docs/source/nexus.md`, covering the architecture and design constraints for the Iroha 3 (Sora Nexus) ledger.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
Bu sahifa `docs/source/nexus.md`ni aks ettiradi. Tarjima to'plami portalga tushmaguncha ikkala nusxani ham bir xilda saqlang.
:::

#! Iroha 3 – Sora Nexus Ledger: Texnik dizayn spetsifikatsiyasi

Ushbu hujjat Iroha 3 uchun Sora Nexus Ledger arxitekturasini taklif qiladi, Iroha 2 Data Spaces (DS) atrofida tashkil etilgan yagona global, mantiqiy birlashtirilgan daftargacha rivojlanadi. Data Spaces kuchli maxfiylik domenlarini ("xususiy ma'lumotlar bo'shliqlari") va ochiq ishtirokni ("ommaviy ma'lumotlar bo'shliqlari") ta'minlaydi. Dizayn shaxsiy DS ma'lumotlari uchun qat'iy izolyatsiya va maxfiylikni ta'minlagan holda global buxgalteriya kitobi bo'ylab birlashtirilishini saqlaydi va Kura (blokli saqlash) va WSV (Jahon davlat ko'rinishi) bo'ylab o'chirish kodlari orqali ma'lumotlar mavjudligini masshtablashni joriy qiladi.

Xuddi shu ombor Iroha 2 (oʻz-oʻzidan joylashtirilgan tarmoqlar) va Iroha 3 (SORA Nexus) ni ham quradi. Bajarish tomonidan quvvatlanadi
umumiy Iroha virtual mashinasi (IVM) va Kotodama asboblar zanjiri, shuning uchun shartnomalar va bayt-kod artefaktlari qoladi
o'z-o'zidan joylashtirilgan joylashtirishlar va Nexus global kitobi bo'ylab ko'chma.

Maqsadlar
- Ko'p hamkorlik qiluvchi validatorlar va ma'lumotlar bo'shliqlaridan tuzilgan bitta global mantiqiy kitob.
- Ruxsat etilgan ishlash uchun shaxsiy ma'lumotlar bo'shliqlari (masalan, CBDC), ma'lumotlar hech qachon shaxsiy DSdan chiqmaydi.
- Ochiq ishtirokli ommaviy ma'lumotlar maydonlari, Ethereum-ga o'xshash ruxsatsiz kirish.
- Shaxsiy-DS aktivlariga kirish uchun aniq ruxsatlarga ega bo'lgan holda, Data Spaces bo'ylab tuziladigan aqlli shartnomalar.
- Ishlashning izolyatsiyasi, shuning uchun jamoat faoliyati shaxsiy-DS ichki tranzaktsiyalarini yomonlashtira olmaydi.
- Ma'lumotlarning keng miqyosda mavjudligi: shaxsiy DS ma'lumotlarini maxfiy saqlagan holda samarali cheklanmagan ma'lumotlarni qo'llab-quvvatlash uchun o'chirish-kodlangan Kura va WSV.

Maqsadsiz (boshlang'ich bosqich)
- token iqtisodini yoki validator rag'batlarini aniqlash; rejalashtirish va staking siyosatlari ulanishi mumkin.
- ABIning yangi versiyasini joriy etish; IVM siyosati boʻyicha aniq tizim qoʻngʻirogʻi va pointer-ABI kengaytmalari bilan maqsadli ABI v1 ni oʻzgartiradi.

Terminologiya
- Nexus Ledger: Ma'lumotlar maydoni (DS) bloklarini yagona, tartiblangan tarix va davlat majburiyatiga tuzish orqali shakllanadigan global mantiqiy kitob.
- Ma'lumotlar maydoni (DS): O'zining tekshiruvchilari, boshqaruvi, maxfiylik klassi, DA siyosati, kvotalar va to'lov siyosati bilan cheklangan ijro va saqlash domeni. Ikkita sinf mavjud: ommaviy DS va xususiy DS.
- Shaxsiy ma'lumotlar maydoni: Ruxsat etilgan validatorlar va kirishni boshqarish; tranzaksiya ma'lumotlari va holati hech qachon DSni tark etmaydi. Faqat majburiyatlar/meta-maʼlumotlar global miqyosda mustahkamlangan.
- Umumiy ma'lumotlar maydoni: ruxsatsiz ishtirok etish; to'liq ma'lumotlar va davlat hamma uchun ochiqdir.
- Ma'lumotlar maydoni manifesti (DS manifesti): DS parametrlarini (validatorlar/QC kalitlari, maxfiylik klassi, ISI siyosati, DA parametrlari, saqlash, kvotalar, ZK siyosati, to'lovlar) e'lon qiluvchi Norito kodli manifest. Manifest xesh nexus zanjiriga biriktirilgan. Agar bekor qilinmasa, DS kvorum sertifikatlari birlamchi kvantdan keyingi imzo sxemasi sifatida ML‑DSA‑87 (Dilithium5‑sinf) dan foydalanadi.
- Space Directory: Yechim va audit uchun DS manifestlari, versiyalari va boshqaruv/aylanish hodisalarini kuzatuvchi global zanjirli katalog shartnomasi.
- DSID: Ma'lumotlar maydoni uchun global noyob identifikator. Barcha ob'ektlar va havolalarni nomlash uchun ishlatiladi.
- Anchor: DS tarixini global kitobga ulash uchun nexus zanjiriga kiritilgan DS bloki/sarlavhasidan olingan kriptografik majburiyat.
- Kura: Iroha blokli saqlash. Bu yerda oʻchirish kodli blob xotirasi va majburiyatlari bilan kengaytirilgan.
- WSV: Iroha World State View. Bu yerda versiyali, suratga olish qobiliyatiga ega, oʻchirish kodli holat segmentlari bilan kengaytirilgan.
- IVM: Iroha aqlli shartnomani bajarish uchun virtual mashina (Kotodama bayt kodi `.to`).
 - AIR: algebraik oraliq vakillik. STARK uslubidagi dalillarni hisoblashning algebraik ko'rinishi, bajarilishini o'tish va chegara cheklovlari bilan maydonga asoslangan izlar sifatida tavsiflaydi.

Ma'lumotlar maydoni modeli
- Identifikatsiya: `DataSpaceId (DSID)` DS ni aniqlaydi va hamma narsani nomlar maydoniga joylashtiradi. DS ni ikkita granularlikda yaratish mumkin:
  - Domain‑DS: `ds::domain::<domain_name>` — bajarilish va domenga tegishli holat.
  - Asset‑DS: `ds::asset::<domain_name>::<asset_name>` — ijro va holat yagona aktiv taʼrifiga kiritilgan.
  Ikkala shakl ham birga mavjud; tranzaktsiyalar bir nechta DSID ga atomik tarzda tegishi mumkin.
- Manifest hayotiy tsikli: DS yaratish, yangilanishlar (kalitlarni aylantirish, siyosat o'zgarishlari) va foydalanishdan chiqish Fazo katalogida qayd etiladi. Har bir slotga DS artefakti so'nggi manifest xeshga havola qiladi.
- Darslar: ommaviy DS (ochiq ishtirok, ommaviy DA) va xususiy DS (ruxsat berilgan, maxfiy DA). Gibrid siyosatlar manifest bayroqlari orqali mumkin.
- DS bo'yicha siyosatlar: ISI ruxsatnomalari, DA parametrlari `(k,m)`, shifrlash, saqlash, kvotalar (har bir blok uchun min/maks tx ulushi), ZK/optimistik isbot siyosati, to'lovlar.
- Boshqaruv: manifestning boshqaruv bo'limida aniqlangan DS a'zoligi va validator rotatsiyasi (zamandagi takliflar, multisig yoki nexus tranzaksiyalari va attestatsiyalar bilan bog'langan tashqi boshqaruv).

Qobiliyat manifestlari va UAID
- Universal hisoblar: Har bir ishtirokchi barcha maʼlumotlar maydonlarini qamrab oluvchi deterministik UAID (`UniversalAccountId`, `crates/iroha_data_model/src/nexus/manifest.rs`) oladi. Imkoniyatlar manifestlari (`AssetPermissionManifest`) UAIDni maʼlum maʼlumotlar maydoniga, faollashtirish/muddati tugash davrlariga va `dataspace`, `dataspace`, `dataspace`, I18NI0000000, I18NI00X00, I18NI00X00, I18NI0000069X ruxsat berish/rad etishning tartiblangan roʻyxatiga bogʻlaydi. `asset` va ixtiyoriy AMX rollari. Qoidalarni rad etish har doim g'alaba qozonadi; baholovchi audit sababi bilan `ManifestVerdict::Denied` yoki mos keladigan toʻlov metamaʼlumotlari bilan `Allowed` grantini chiqaradi.
- Imtiyozlar: Har bir ruxsat berilgan yozuvda deterministik `AllowanceWindow` chelaklari (`PerSlot`, `PerMinute`, `PerDay`) va ixtiyoriy `max_amount` mavjud. Xostlar va SDKlar bir xil Norito foydali yukini iste'mol qiladi, shuning uchun qo'llash apparat va SDK ilovalarida bir xil bo'lib qoladi.
- Telemetriyani tekshirish: Manifest holatini o'zgartirganda, kosmik katalog `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) translyatsiyasini amalga oshiradi. Yangi `SpaceDirectoryEventFilter` yuzasi Torii/data-event obunachilariga maxsus sanitariya-tesisatsiz UAID manifest yangilanishlari, bekor qilish va rad etish-yutuq qarorlarini kuzatish imkonini beradi.

Operator dalillari, SDK migratsiya eslatmalari va manifest nashri roʻyxatlari uchun Universal Account Guide (`docs/source/universal_accounts_guide.md`) bilan ushbu boʻlimni aks ettiring. UAID siyosati yoki asboblari har doim o'zgarganda ikkala hujjatni ham tekislang.

Yuqori darajadagi arxitektura
1) Global kompozitsion qatlam (Nexus zanjiri)
- Bir yoki bir nechta ma'lumotlar bo'shliqlarini (DS) qamrab olgan atom tranzaktsiyalarini yakunlovchi 1 soniyali Nexus bloklarining yagona, kanonik tartibini saqlaydi. Har bir amalga oshirilgan tranzaksiya yagona global dunyo holatini yangilaydi (per‑DS ildizlari vektori).
- Tarkibida minimal meta-maʼlumotlar hamda birlashtirilgan, yakuniylik va firibgarlikni aniqlashni taʼminlash uchun jamlangan dalillar/QClar (tegilgan DSIDlar, har bir DS holatidan oldingi/keyin ildizlar, DA majburiyatlari, per-DS uchun haqiqiylik dalillari va ML‑DSA‑87 yordamida DS kvorum sertifikati). Hech qanday shaxsiy ma'lumotlar kiritilmagan.
- Konsensus: 22 o'lchamdagi (3f+1 bilan f=7) yagona global, quvurli BFT qo'mitasi, davr VRF/stake mexanizmi orqali ~200k gacha bo'lgan potentsial tasdiqlovchilar to'plamidan tanlangan. Nexus qo'mitasi tranzaktsiyalarni tartiblaydi va blokni 1 soniya ichida yakunlaydi.

2) Ma'lumotlar maydoni qatlami (Ommaviy/Xususiy)
- Global tranzaksiyalarning har bir DS fragmentini bajaradi, DS-mahalliy WSV-ni yangilaydi va 1-sekundlik Nexus blokiga o'tuvchi har bir blok yaroqlilik artefaktlarini (har bir DS isboti va DA majburiyatlari) ishlab chiqaradi.
- Maxfiy DS vakolatli validatorlar oʻrtasida dam turgan va parvozdagi maʼlumotlarni shifrlaydi; DSni faqat majburiyatlar va PQ amal qilish dalillari tark etadi.
- Public DS to'liq ma'lumotlar jismlarini eksport qiladi (DA orqali) va PQ haqiqiyligini isbotlaydi.

3) Atom oʻzaro maʼlumotlari-kosmik operatsiyalari (AMX)
- Model: Har bir foydalanuvchi tranzaksiyasi bir nechta DS ga tegishi mumkin (masalan, DS domeni va bir yoki bir nechta aktiv DS). U bitta Nexus blokida atomik tarzda amalga oshiriladi yoki bekor qilinadi; qisman ta'siri yo'q.
- 1 soniya ichida tayyorlang: Har bir nomzod tranzaktsiyasi uchun tegilgan DS bir xil suratga (slot DS ildizlari) qarshi parallel ravishda bajariladi va har bir DS PQ haqiqiyligini isbotlash (FASTPQ‑ISI) va DA majburiyatlarini ishlab chiqing. Nexus qo'mitasi tranzaksiyani faqat barcha kerakli DS dalillari tekshirilganda va DA sertifikatlari kelganda (≤300 ms maqsadli) amalga oshiradi; aks holda tranzaksiya keyingi slotga qayta rejalashtirilgan.
- Muvofiqlik: o'qish-yozish to'plamlari e'lon qilinadi; nizolarni aniqlash slotning boshlang'ich ildizlariga qarshi sodir bo'lganda sodir bo'ladi. DS uchun blokirovkasiz optimistik ijro global stendlardan qochadi; atomiklik nexus commit qoidasi (DS bo'ylab hammasi yoki hech narsa) tomonidan amalga oshiriladi.
- Maxfiylik: Xususiy DS eksporti faqat DSdan oldingi/posengi ildizlarga bog'langan dalillar/majburiyatlar. Hech qanday shaxsiy ma'lumotlar DSni tark etmaydi.4) Oʻchirish kodlash bilan maʼlumotlar mavjudligi (DA).
- Kura blok jismlarini va WSV snapshotlarini oʻchirish kodli bloblar sifatida saqlaydi. Ommaviy bloblar keng tarqalgan; shaxsiy bloblar faqat shaxsiy DS validatorlarida, shifrlangan qismlar bilan saqlanadi.
- DA majburiyatlari DS artefaktlarida ham, Nexus bloklarida ham qayd etilgan bo'lib, shaxsiy tarkibni oshkor qilmasdan namuna olish va tiklash kafolatlarini ta'minlaydi.

Bloklash va topshirish tuzilmasi
- Ma'lumotlar maydonini isbotlovchi artefakt (har bir DS uchun 1 soniya uchun)
  - Maydonlar: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML‑DSA‑87), ds_validity_proof (FASTIQ‑).
  - Ma'lumotlar organisiz xususiy-DS eksport artefaktlari; ommaviy DS DA orqali tanani olish imkonini beradi.

- Nexus bloki (1s kadans)
  - Maydonlar: blok_raqam, ota-ona_xesh, slot_vaqti, tx_list (DSIDlar tegilgan atom oʻzaro DS tranzaksiyalari), ds_artifacts[], nexus_qc.
  - Funktsiya: kerakli DS artefaktlari tekshiriladigan barcha atom operatsiyalarini yakunlaydi; DS ildizlarining global dunyo holati vektorini bir qadamda yangilaydi.

Konsensus va rejalashtirish
- Nexus zanjirli konsensus: 1s bloklari va 1s yakuniyligini maqsad qilgan 22 tugunli qo'mitaga (f=7 bilan 3f+1) ega bo'lgan yagona global, quvurli BFT (Sumeragi-sinf). Qo'mita a'zolari 200 ming nomzod orasidan VRF / ulush orqali tanlanadi; aylanish markazsizlashtirish va tsenzura qarshiligini saqlaydi.
- Ma'lumotlar maydoni bo'yicha konsensus: Har bir DS har bir slotga artefaktlar (dalillar, DA majburiyatlari, DS QC) ishlab chiqarish uchun o'zining validatorlari orasida o'z BFT-ni ishga tushiradi. Lane-rele qo'mitalari `3f+1` ma'lumotlar maydoni `fault_tolerance` sozlamasidan foydalangan holda o'lchamlari va `(dataspace_id, lane_id)` bilan bog'langan VRF davri urug'idan foydalangan holda ma'lumotlar maydoni validator hovuzidan har bir davr uchun deterministik namunalar olinadi. Shaxsiy DSga ruxsat berilgan; Ommaviy DS Sybilga qarshi siyosatlarga muvofiq ochiq hayotga ruxsat beradi. Global Nexus qo'mitasi o'zgarishsiz qolmoqda.
- Tranzaktsiyalarni rejalashtirish: foydalanuvchilar tegilgan DSID va o'qish-yozish to'plamlarini e'lon qilgan atom tranzaktsiyalarini taqdim etadilar. DS slot ichida parallel ravishda ishlaydi; Agar barcha DS artefaktlari tekshirilsa va DA sertifikatlari o'z vaqtida (≤300 ms) bo'lsa, nexus qo'mitasi tranzaksiyani 1s blokiga o'z ichiga oladi.
- Ishlash izolyatsiyasi: Har bir DS mustaqil mempoollarga va ijroga ega. Har bir DS kvotalari DS ning shaxsiy kechikishini oldini olish va blokirovka qilishning oldini olish uchun har bir blokda maʼlum DS ga tegadigan qancha tranzaksiyalarni amalga oshirish mumkinligini belgilaydi.

Ma'lumotlar modeli va nomlar oralig'i
- DS-Qualified identifikatorlari: Barcha ob'ektlar (domenlar, hisoblar, aktivlar, rollar) `dsid` tomonidan kvalifikatsiya qilingan. Misol: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Global ma'lumotnomalar: Global ma'lumotnoma `(dsid, object_id, version_hint)` korteji bo'lib, uni nexus qatlamida zanjirga yoki o'zaro DSdan foydalanish uchun AMX deskriptorlariga joylashtirish mumkin.
- Norito Seriyalashtirish: Barcha oʻzaro DS xabarlari (AMX deskriptorlari, isbotlari) Norito kodeklaridan foydalanadi. Ishlab chiqarish yo'llarida serde ishlatilmaydi.

Aqlli kontraktlar va IVM kengaytmalari
- Bajarish konteksti: `dsid` ni IVM ijro kontekstiga qo'shing. Kotodama shartnomalari har doim ma'lum bir ma'lumot maydonida amalga oshiriladi.
- Atomic Cross-DS primitivlari:
  - `amx_begin()` / `amx_commit()` IVM xostidagi atomli multi-DS tranzaksiyasini belgilang.
  - `amx_touch(dsid, key)` slot snapshot ildizlariga qarshi ziddiyatlarni aniqlash uchun o'qish/yozish niyatini e'lon qiladi.
  - `verify_space_proof(dsid, proof, statement)` → bool
  - `use_asset_handle(handle, op, amount)` → natija (faqat siyosat ruxsat bergan va dastak haqiqiy bo'lsa, operatsiyaga ruxsat beriladi)
- Aktivlarni boshqarish va to'lovlar:
  - Aktiv operatsiyalari DS ning ISI/rol siyosati bilan ruxsat etilgan; to'lovlar DS ning gaz tokenida to'lanadi. Ixtiyoriy qobiliyat tokenlari va yanada boyroq siyosat (koʻp tasdiqlovchi, tarif chegaralari, geofencing) atom modelini oʻzgartirmasdan keyinroq qoʻshilishi mumkin.
- Determinizm: Barcha yangi tizimlar sof va deterministik berilgan kirishlar va e'lon qilingan AMX o'qish/yozish to'plamlaridir. Yashirin vaqt yoki atrof-muhit ta'siri yo'q.

Post-kvant haqiqiyligini isbotlash (umumiylashtirilgan ISI)
- FASTPQ‑ISI (PQ, ishonchli oʻrnatish yoʻq): yadrolashtirilgan, xeshga asoslangan argument boʻlib, u barcha ISI oilalari uchun uzatish dizaynini umumlashtiradi, shu bilan birga GPU-sinf apparatida 20k masshtabli partiyalar uchun soniyadan-sonli isbotlashni maqsad qiladi.
  - Operatsion profil:
    - Ishlab chiqarish tugunlari proverni `fastpq_prover::Prover::canonical` orqali quradi, endi u har doim ishlab chiqarish orqa qismini ishga tushiradi; deterministik masxara olib tashlandi.【crates/fastpq_prover/src/proof.rs:126】
    - `zk.fastpq.execution_mode` (konfiguratsiya) va `irohad --fastpq-execution-mode` operatorlarga protsessor/GPU bajarilishini aniq belgilash imkonini beradi, kuzatuvchi kancasi esa flot uchun so'ralgan/hal qilingan/backend uchliklarini qayd qiladi. audits.【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:2192】【crates/iroha_config/src/parameters/8】crates/iroha_878
- Arifmetizatsiya:
  - KV‑Update AIR: WSV ni Poseidon2‑SMT orqali kiritilgan kalit-qiymat xaritasi sifatida ko'rib chiqing. Har bir ISI kalitlar (hisoblar, aktivlar, rollar, domenlar, metama'lumotlar, ta'minot) ustidagi o'qish-tekshirish-yozish satrlarining kichik to'plamiga kengaytiriladi.
  - Opkod bilan bog'langan cheklovlar: Selektor ustunlari bo'lgan yagona AIR jadvali har bir ISI qoidalarini (saqlash, monotonik hisoblagichlar, ruxsatnomalar, diapazonni tekshirish, cheklangan metama'lumotlarni yangilash) amalga oshiradi.
  - Argumentlarni qidirish: Ruxsatlar/rollar, aktivlar aniqligi va siyosat parametrlari uchun shaffof, xesh bilan tuzilgan jadvallar katta bitli cheklovlardan qochadi.
- Davlat majburiyatlari va yangilanishlari:
  - Agregatlangan SMT isboti: Barcha tegilgan tugmalar (oldindan/post) `old_root`/`new_root` ga qarshi siqilgan chegaradan foydalanib, aka-uka birodarlar bilan tasdiqlangan.
  - Invariantlar: Global invariantlar (masalan, har bir aktiv uchun umumiy ta'minot) effekt qatorlari va kuzatilgan hisoblagichlar o'rtasidagi ko'p to'plam tengligi orqali amalga oshiriladi.
- Tasdiqlash tizimi:
  - FRI uslubidagi polinom majburiyatlari (DEEP‑FRI) yuqori aritmli (8/16) va 8–16 portlash; Poseidon2 xeshlari; SHA‑2/3 bilan Fiat-Shamir transkripti.
  - Majburiy emas rekursiya: agar kerak bo'lsa, mikro-to'plamlarni har bir slot uchun bitta isbotga siqish uchun DS-mahalliy rekursiv yig'ish.
- Qo'llanilgan doira va misollar:
  - Aktivlar: o'tkazish, zarb qilish, yoqish, aktiv ta'riflarini ro'yxatdan o'tkazish/ro'yxatdan o'chirish, aniqlikni belgilash (chegaralangan), metama'lumotlarni o'rnatish.
  - Hisoblar/domenlar: yaratish/o‘chirish, kalit/eshikni belgilash, imzolovchilarni qo‘shish/o‘chirish (faqat shtat; imzo tekshiruvlari DS validatorlari tomonidan tasdiqlangan, AIR ichida tasdiqlanmagan).
  - Rollar/ruxsatlar (ISI): rollar va ruxsatlarni berish/bekor qilish; Qidiruv jadvallari va monoton siyosat tekshiruvlari orqali amalga oshiriladi.
  - Shartnomalar/AMX: AMX start/commit markerlari, agar yoqilgan bo'lsa, zarb qilish/bekor qilish imkoniyati; davlat o'tishlari va siyosat hisoblagichlari sifatida isbotlangan.
- Kechikishni saqlab qolish uchun havodan tashqari tekshiruvlar:
  - Imzolar va ogʻir kriptografiya (masalan, ML-DSA foydalanuvchi imzolari) DS validatorlari tomonidan tekshiriladi va DS QCda tasdiqlanadi; yaroqlilik isboti faqat davlat izchilligi va siyosatga muvofiqligini qamrab oladi. Bu PQ va tezkor dalillarni saqlaydi.
- Ishlash maqsadlari (tasviriy, 32 yadroli CPU + yagona zamonaviy GPU):
  - Kichkina tugma teginishli 20 ming aralash ISI (≤8 tugma/ISI): ~0,4–0,9 s isbotlash, ~150–450 KB isbotlash, ~5–15 ms tekshirish.
  - Og'irroq ISI (ko'proq kalitlar/boy cheklovlar): mikro-to'plam (masalan, 10×2k) + har bir slot uchun <1 s saqlash uchun rekursiya.
- DS Manifest konfiguratsiyasi:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (imzolar DS QC tomonidan tasdiqlangan)
  - `attestation.qc_signature = "ml_dsa_87"` (standart; muqobillar aniq e'lon qilinishi kerak)
- Qaytarilishlar:
  - Murakkab/moslashtirilgan ISIlar umumiy STARKdan (`zk.policy = "stark_fri_general"`) foydalanishi mumkin, bu esa kechiktirilgan isbot va 1 soniya yakuniy QC attestatsiyasi + noto'g'ri dalillarni kesish orqali amalga oshirilishi mumkin.
  - PQ bo'lmagan variantlar (masalan, KZG bilan Plonk) ishonchli sozlashni talab qiladi va endi standart tuzilmada qo'llab-quvvatlanmaydi.

AIR Primer (Nexus uchun)
- Bajarilish izi: Kengligi (registr ustunlari) va uzunligi (qadamlar) bo'lgan matritsa. Har bir satr ISIni qayta ishlashning mantiqiy bosqichidir; ustunlar oldingi/post qiymatlari, selektorlar va bayroqlarni saqlaydi.
- Cheklovlar:
  - O'tish cheklovlari: qatorlar orasidagi munosabatlarni qo'llash (masalan, post_balance = pre_balance − `sel_transfer = 1` bo'lganda debet qatori summasi).
  - Chegara cheklovlari: ommaviy kiritish-chiqarish (old_root/new_root, hisoblagichlar)ni birinchi/oxirgi qatorlarga bog'lash.
  - Qidiruvlar/o'zgartirishlar: a'zolik va ko'p to'plam tengligini bit-og'ir sxemalarsiz tasdiqlangan jadvallar (ruxsatlar, aktivlar parametrlari) bilan ta'minlash.
- Majburiyat va tekshirish:
  - Prover xeshga asoslangan kodlash orqali izlarni o'z zimmasiga oladi va cheklovlar mavjud bo'lsa, amal qiladigan past darajali polinomlarni tuzadi.
  - Verifier bir nechta Merkle teshiklari bilan FRI (xesh-asosli, post-kvant) orqali past darajani tekshiradi; xarajat bosqichlarda logarifmik hisoblanadi.
- Misol (Transfer): registrlar balansdan oldingi, summa, balansdan keyingi, nonce va selektorlarni o'z ichiga oladi. Cheklovlar salbiy bo'lmaganlik/diapazon, saqlash va monotonlikni ta'minlaydi, yig'ilgan SMT esa barglarni eski/yangi ildizlar bilan bog'laydi.ABI va Syscall Evolution (ABI v1)
- Qo'shiladigan tizimlar (tasviriy nomlar):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Qo'shiladigan Pointer-ABI turlari:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Kerakli yangilanishlar:
  - `ivm::syscalls::abi_syscall_list()` ga qo'shing (buyurtma berishda davom eting), siyosat bo'yicha eshik.
  - Noma'lum raqamlarni xostlarda `VMError::UnknownSyscall` bilan xaritalash.
  - Yangilash testlari: tizim ro'yxati oltin, ABI xesh, ko'rsatkich turi identifikatori oltin rang va siyosat testlari.
  - Hujjatlar: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Maxfiylik modeli
- Shaxsiy maʼlumotlarni saqlash: Tranzaksiya organlari, shtat farqlari va shaxsiy DS uchun WSV snapshotlari hech qachon xususiy validator kichik toʻplamini tark etmaydi.
- Ommaviy ta'sir qilish: faqat sarlavhalar, DA majburiyatlari va PQ haqiqiyligini tasdiqlovchi hujjatlar eksport qilinadi.
- Ixtiyoriy ZK isbotlari: Xususiy DS ichki holatni ko‘rsatmasdan o‘zaro DS harakatlarini amalga oshirish imkonini beruvchi ZK dalillarini (masalan, balans yetarli, siyosat bajarilgan) ishlab chiqishi mumkin.
- Kirish nazorati: Avtorizatsiya DS ichidagi ISI/rol siyosati bilan amalga oshiriladi. Imkoniyat tokenlari ixtiyoriy va agar kerak bo'lsa, keyinroq kiritilishi mumkin.

Ishlash izolyatsiyasi va QoS
- DS uchun alohida konsensus, mempullar va saqlash.
- Nexus langarni kiritish vaqtini chegaralash va chiziq boshini blokirovka qilishni oldini olish uchun DS uchun rejalashtirish kvotalari.
- IVM xosti tomonidan amalga oshiriladigan DS (hisoblash/xotira/IO) bo'yicha shartnoma resurslari byudjetlari. Ommaviy-DS bahsi shaxsiy-DS budjetlarini ishlata olmaydi.
- Asinxron o'zaro DS qo'ng'iroqlari shaxsiy-DS ijrosi doirasida uzoq sinxron kutishlardan saqlaydi.

Ma'lumotlarning mavjudligi va saqlash dizayni
1) Kodlashni o'chirish
- Kura bloklari va WSV snapshotlarini blob darajasida oʻchirish kodlash uchun tizimli Reed-Solomon (masalan, GF(2^16)) dan foydalaning: `(k, m)` parametrlari `n = k + m` parchalari bilan.
- Standart parametrlar (taklif etilgan, ommaviy DS): `k=32, m=16` (n=48), ~1,5× kengayishi bilan 16 tagacha parchalanish yo‘qotishlarini tiklash imkonini beradi. Shaxsiy DS uchun: `k=16, m=8` (n=24) ruxsat etilgan toʻplam doirasida. Ikkalasi ham DS Manifestiga muvofiq sozlanishi mumkin.
- Ommaviy Bloblar: Ko'p DA tugunlari/validatorlari bo'ylab taqsimlangan parchalar namuna olish asosida mavjudligi tekshiruvi bilan. Sarlavhalardagi DA majburiyatlari engil mijozlarga tekshirishga imkon beradi.
- Xususiy Bloblar: Parchalar shifrlangan va faqat shaxsiy-DS validatorlari (yoki belgilangan saqlovchilar) ichida tarqatiladi. Global zanjir faqat DA majburiyatlarini o'z zimmasiga oladi (shad joylashuvi yoki kalitlari yo'q).

2) Majburiyatlar va namunalar
- Har bir blob uchun: parchalar ustida Merkle ildizini hisoblang va uni `*_da_commitment` ichiga kiriting. Elliptik egri chiziq majburiyatlaridan qochib, PQ darajasida qoling.
- DA Attesters: VRF-namuna olgan mintaqaviy attestatorlar (masalan, har bir mintaqada 64 ta) parcha namunasini muvaffaqiyatli olganligini tasdiqlovchi ML-DSA‑87 sertifikatini beradi. Maqsadli DA attestatsiyasining kechikishi ≤300 ms. Nexus qo'mitasi parchalarni tortib olish o'rniga sertifikatlarni tasdiqlaydi.

3) Kura integratsiyasi
- Bloklar Merkle majburiyatlari bilan o'chirish kodli bloklar sifatida tranzaksiya organlarini saqlaydi.
- Sarlavhalar blob majburiyatlarini o'z ichiga oladi; jismlarni umumiy DS uchun DA tarmog'i va xususiy DS uchun xususiy kanallar orqali olish mumkin.

4) WSV integratsiyasi
- WSV suratga olish: Vaqti-vaqti bilan nazorat nuqtasi DS holatini sarlavhalarda yozilgan majburiyatlar bilan bo'laklarga bo'lingan, o'chirish kodli oniy tasvirlarga aylantiring. Suratlar o'rtasida o'zgarishlar jurnallarini saqlang. Ommaviy suratlar keng tarqalgan; shaxsiy suratlar shaxsiy validatorlar ichida qoladi.
- Proof-Tashish ruxsati: Shartnomalar oniy surat majburiyatlari bilan bog'langan davlat dalillarini (Merkle/Verkle) taqdim etishi (yoki so'rashi) mumkin. Xususiy DS xom dalillar o'rniga nol bilim sertifikatlarini taqdim etishi mumkin.

5) Saqlash va kesish
- Ommaviy DS uchun Azizillo yo'q: DA (gorizontal o'lchov) orqali barcha Kura jismlari va WSV snapshotlarini saqlang. Xususiy DS ichki saqlashni belgilashi mumkin, ammo eksport qilingan majburiyatlar o'zgarmas bo'lib qoladi. Nexus qatlami barcha Nexus bloklari va DS artefakt majburiyatlarini saqlab qoladi.

Tarmoq va tugun rollari
- Global Validatorlar: Nexus konsensusda ishtirok eting, Nexus bloklari va DS artefaktlarini tasdiqlang, ommaviy DS uchun DA tekshiruvlarini o'tkazing.
- Ma'lumotlar maydonini tekshiruvchilar: DS konsensusini ishga tushiring, shartnomalarni bajaring, mahalliy Kura/WSVni boshqaring, DS uchun DAni boshqaring.
- DA tugunlari (ixtiyoriy): Ommaviy bloklarni saqlash/eʼlon qilish, namuna olishni osonlashtirish. Xususiy DS uchun DA tugunlari validatorlar yoki ishonchli vasiylar bilan birga joylashgan.

Tizim darajasidagi takomillashtirish va mulohazalar
- Sekvensiya/mempulni ajratish: mantiqiy modelni o'zgartirmasdan kechikishni kamaytirish va o'tkazish qobiliyatini yaxshilash uchun nexus qatlamida quvurli BFT ni oziqlantiruvchi DAG mempulini (masalan, Narval uslubida) qabul qiling.
- DS kvotalari va adolatliligi: blokirovkaning oldini olish va shaxsiy DS uchun taxmin qilinadigan kechikishni ta'minlash uchun har bir blok uchun kvota va vazn chegaralari.
- DS attestatsiyasi (PQ): Standart DS kvorum sertifikatlarida ML‑DSA‑87 (Dilithium5‑sinf) ishlatiladi. Bu post-kvant va EC imzolaridan kattaroq, lekin har bir slot uchun bitta QCda qabul qilinadi. Agar DS manifestida e'lon qilingan bo'lsa, DS ML‑DSA‑65/44 (kichikroq) yoki EC imzolarini tanlashi mumkin; ommaviy DS ML‑DSA‑87 ni saqlashga qat'iy tavsiya etiladi.
- DA attestatorlari: Ommaviy DS uchun DA sertifikatlarini chiqaradigan VRF namunali mintaqaviy attestatorlardan foydalaning. Nexus qo'mitasi xom parcha namunalarini olish o'rniga sertifikatlarni tasdiqlaydi; xususiy DS DA attestatsiyalarini ichki saqlaydi.
- Rekursiya va davr isbotlari: ixtiyoriy ravishda isbot oʻlchamlarini saqlash va yuqori yuk ostida vaqt barqarorligini tekshirish uchun DS ichidagi bir nechta mikro-toʻplamlarni har bir slot/davr uchun bitta rekursiv isbotga jamlang.
- Bo'lakni masshtablash (agar kerak bo'lsa): Agar bitta global qo'mita muammoga aylansa, deterministik birlashma bilan K parallel ketma-ketlik qatorlarini kiriting. Bu gorizontal miqyosda yagona global tartibni saqlaydi.
- Deterministik tezlashtirish: o'zaro apparat determinizmini saqlab qolish uchun bir oz aniq protsessor zaxirasi bilan xeshlash/FFT uchun SIMD/CUDA xususiyatiga ega yadrolarni taqdim eting.
- Yo‘lakni faollashtirish chegaralari (taklif): Agar (a) p95 yakuniyligi ketma-ket >3 daqiqa davomida 1,2 s dan oshsa yoki (b) har bir blokda bandlik >5 daqiqa davomida 85% dan oshsa yoki (c) keladigan tx tezligi >1,2× blokirovka sig‘imini talab qilsa, 2–4 qatorni yoqing. Yo'llar DSID xesh orqali tranzaktsiyalarni aniqlab beradi va nexus blokida birlashadi.

Toʻlovlar va iqtisod (dastlabki defoltlar)
- Gaz birligi: hisoblangan hisoblash/IO bilan per‑DS gaz tokeni; to'lovlar DSning tabiiy gaz aktivida to'lanadi. DS bo'ylab konvertatsiya qilish - bu ilova muammosi.
- Inklyuzivlik ustuvorligi: adolat va 1s SLOsni saqlash uchun har bir DS kvotalari bilan DS bo'ylab aylanma rejim; DS doirasida, to'lov taklifi aloqalarni buzishi mumkin.
- Kelajak: ixtiyoriy global to'lov bozori yoki MEV-minimallashtirish siyosatlarini atomiklik yoki PQ isbotlangan dizaynni o'zgartirmasdan o'rganish mumkin.

Oʻzaro maʼlumotlar-kosmik ish jarayoni (misol)
1) Foydalanuvchi ochiq DS P va shaxsiy DS S ga tegishli AMX tranzaksiyasini yuboradi: X aktivini S dan hisobi Pda joylashgan B benefisiariga o‘tkazing.
2) Slot ichida P va S har biri o'z fragmentini slot snapshotiga qarshi bajaradi. S avtorizatsiya va mavjudligini tekshiradi, ichki holatini yangilaydi va PQ haqiqiyligini isbotlash va DA majburiyatini ishlab chiqaradi (hech qanday shaxsiy ma'lumotlar sizib chiqmagan). P tegishli holat yangilanishini tayyorlaydi (masalan, siyosatga muvofiq Pda yalpiz/yonish/qulflash) va uning isboti.
3) Nexus qo'mitasi DS dalillarini ham, DA sertifikatlarini ham tekshiradi; agar ikkalasi ham slot ichida tekshirilsa, tranzaksiya 1s Nexus blokida atomik tarzda amalga oshiriladi va global dunyo davlat vektoridagi ikkala DS ildizini yangilaydi.
4) Agar biron-bir dalil yoki DA sertifikati etishmayotgan/yaroqsiz bo'lsa, tranzaksiya to'xtatiladi (hech qanday effekt yo'q) va mijoz keyingi o'ringa qayta yuborishi mumkin. Hech qanday shaxsiy ma'lumotlar hech qanday bosqichda S ni tark etmaydi.

- Xavfsizlik masalalari
- Deterministik ijro: IVM tizim chaqiruvlari deterministik bo'lib qoladi; o'zaro DS natijalari devor soati yoki tarmoq vaqti emas, balki AMX majburiyati va yakuniyligi bilan belgilanadi.
- Kirish nazorati: xususiy DSda ISI ruxsatnomalari tranzaktsiyalarni kim topshirishi va qanday operatsiyalarga ruxsat berilishini cheklaydi. Imkoniyat tokenlari oʻzaro DSdan foydalanish uchun nozik huquqlarni kodlaydi.
- Maxfiylik: shaxsiy DS ma'lumotlari uchun uchdan-end shifrlash, faqat vakolatli a'zolar orasida saqlanadigan o'chirish kodli parchalar, tashqi attestatsiyalar uchun ixtiyoriy ZK dalillari.
- DoS qarshiligi: mempool/consensus/saqlash qatlamlaridagi izolyatsiya jamoat tirbandligining shaxsiy-DS taraqqiyotiga ta'sir qilishini oldini oladi.Iroha komponentlariga o'zgartirishlar
- iroha_data_model: `DataSpaceId`, DS-malakali identifikatorlar, AMX identifikatorlari (o'qish/yozish to'plamlari), isbot/DA majburiyat turlarini joriy qiling. Norito-faqat seriallashtirish.
- ivm: AMX (`amx_begin`, `amx_commit`, `amx_touch`) va DA isbotlari uchun tizim chaqiruvlari va koʻrsatkich-ABI turlarini qoʻshing; v1 siyosati bo'yicha ABI testlarini/hujjatlarni yangilang.
- iroha_core: Nexus rejalashtiruvchisini, Space Directory, AMX marshrutini/validatsiyasini, DS artefaktini tekshirishni va DA tanlab olish va kvotalar uchun siyosatni amalga oshirishni amalga oshiring.
- Kosmik katalog va manifest yuklagichlari: FMS so'nggi nuqta metama'lumotlarini (va boshqa umumiy yaxshi xizmat identifikatorlarini) DS manifestini tahlil qilish orqali o'tkazing, shunda tugunlar Ma'lumotlar maydoniga qo'shilganda mahalliy xizmatning so'nggi nuqtalarini avtomatik ravishda topadi.
- kura: Blob do'koni o'chirish kodlari, majburiyatlar, shaxsiy/davlat siyosatlariga mos keladigan qidiruv API'lari.
- WSV: Snapshot olish, qismlarga ajratish, majburiyatlar; isbotlangan API; AMX ziddiyatlarini aniqlash va tekshirish bilan integratsiya.
- irohod: tugun rollari, DA uchun tarmoq, shaxsiy-DS a'zoligi/autentifikatsiya, `iroha_config` orqali konfiguratsiya (ishlab chiqarish yo'llarida env o'tishlari yo'q).

Konfiguratsiya va determinizm
- Ish vaqtining barcha xatti-harakatlari `iroha_config` orqali sozlangan va konstruktorlar/xostlar orqali uzatilgan. Hech qanday ishlab chiqarish env o'zgartirilmaydi.
- Uskuna tezlashuvi (SIMD/NEON/METAL/CUDA) ixtiyoriy va xususiyatga ega; deterministik zaxiralar apparat bo'ylab bir xil natijalarni berishi kerak.
 - Kvantdan keyingi sukut: Barcha DS sukut bo'yicha DS QC uchun PQ haqiqiyligini isbotlash (STARK/FRI) va ML‑DSA‑87 dan foydalanishi kerak. Alternativlar aniq DS manifest deklaratsiyasi va siyosatni tasdiqlashni talab qiladi.

Migratsiya yoʻli (Iroha 2 → Iroha 3)
1) Ma'lumotlar modelida ma'lumotlar maydoni uchun malakali identifikatorlar va nexus bloki/global holat tarkibini joriy qilish; o'tish paytida Iroha 2 eski rejimlarini saqlab qolish uchun xususiyat bayroqlarini qo'shing.
2) Funksiya bayroqlari orqasida Kura/WSV oʻchirish-kodlash kodlarini joriy eting, dastlabki bosqichlarda joriy backendlarni sukut boʻyicha saqlang.
3) AMX (atomic multi‑DS) operatsiyalari uchun IVM tizimli qoʻngʻiroqlari va koʻrsatkich turlarini qoʻshing; testlar va hujjatlarni kengaytirish; ABI v1 ni saqlang.
4) Yagona umumiy DS va 1s bloklari bilan minimal nexus zanjirini yetkazib berish; keyin faqat birinchi xususiy-DS pilotini eksport qiluvchi dalillar/majburiyatlarni qo'shing.
5) DS-mahalliy FASTPQ-ISI dalillari va DA attesters bilan to'liq atomik o'zaro DS tranzaktsiyalarini (AMX) kengaytiring; DS boʻylab ML‑DSA‑87 QCni yoqish.

Sinov strategiyasi
- Ma'lumotlar modeli turlari, Norito aylanma sayohatlar, AMX tizim chaqiruvi xatti-harakatlari va isbot kodlash/dekodlash uchun birlik testlari.
- Yangi tizimlar va ABI oltinlari uchun IVM sinovlari.
- Atom o'zaro DS tranzaksiyalari (ijobiy/salbiy), DA attesterining kechikish maqsadlari (≤300 ms) va yuk ostida ishlash izolyatsiyasi uchun integratsiya testlari.
- DS QC tekshiruvi (ML‑DSA‑87), nizolarni aniqlash/bekor qilish semantikasi va maxfiy parchalanishning oldini olish uchun xavfsizlik testlari.

### NX-18 Telemetriya va Runbook aktivlari

- **Grafana doska:** `dashboards/grafana/nexus_lanes.json` endi NX‑18 talab qilgan “Nexus Lane Finality & Oracles” boshqaruv panelini eksport qiladi. Panellar `histogram_quantile()` ni `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, DA mavjudligi haqida ogohlantirishlar (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), oracle narxlari/eskilik/TWAP/soch o'lchagichlari va jonli operator I0104 buffer panellarini o'z ichiga oladi. 1s slot, DA va g'aznachilik SLOlari buyurtma so'rovlarisiz.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` asboblar paneliga hamroh boʻlgan chaqiruv boʻyicha ish jarayonini (boʻsagʻalar, hodisa bosqichlari, dalillarni toʻplash, tartibsizlik mashqlari) hujjatlashtirib, NX‑18 dan “operator asboblar paneli/runbooklarini nashr etish” bandini bajaradi.
- **Telemetriya yordamchilari:** mavjud `scripts/telemetry/compare_dashboards.py` dan eksport qilinadigan asboblar panelini farqlash uchun qayta foydalaning (staging/mahsulot siljishining oldini olish), DA/quorum/oracle/buffer/qo'ng'iroqlarini ochish uchun `ci/check_nexus_lane_smoke.sh` ichida `ci/check_nexus_lane_smoke.sh`-ni ishga tushiring, `scripts/telemetry/check_nexus_audit_outcome.py` marshrutlangan kuzatuv yoki tartibsizlik mashqlari paytida, shuning uchun har bir NX‑18 matkap mos keladigan `nexus.audit.outcome` foydali yukini arxivlaydi.

Ochiq savollar (tushuntirish kerak)
1) Tranzaksiya imzolari: Qaror — oxirgi foydalanuvchilar maqsadli DS reklama qiladigan istalgan imzolash algoritmini tanlashi mumkin (Ed25519, secp256k1, ML‑DSA va boshqalar). Xostlar manifestlarda multisig/egri chiziqli qobiliyat bayroqlarini qo'llashi, deterministik qaytarilishlarni ta'minlashi va algoritmlarni aralashtirishda kechikish oqibatlarini hujjatlashtirishi kerak. Ajoyib: Torii/SDK’lar bo‘ylab imkoniyatlar haqida muzokaralar oqimini yakunlang va qabul testlarini yangilang.
2) Gaz iqtisodiyoti: Har bir DS mahalliy tokenda gazni ko'rsatishi mumkin, global hisob-kitob to'lovlari esa SORA XOR da to'lanadi. Muvaffaqiyatli: standart konversiya yo‘lini (ommaviy tarmoq DEX va boshqa likvidlik manbalariga nisbatan), buxgalteriya hisobi ilgaklarini va subsidiyalash yoki nol narxdagi operatsiyalarni DS uchun himoya vositalarini aniqlang.
3) DA attesters: Har bir hudud va chegara boʻyicha maqsadli raqam (masalan, 64 ta namuna olingan, 64 ML‑DSA‑87 ta imzodan 43‑toʻgʻi) chidamlilikni saqlab, ≤300 ms ga javob beradi. Birinchi kundan boshlab qaysi hududlarni kiritishimiz kerak?
4) Standart DA parametrlari: Biz ommaviy DS `k=32, m=16` va xususiy DS `k=16, m=8` ni taklif qildik. Muayyan DS sinflari uchun yuqoriroq zaxira profilini (masalan, `k=30, m=20`) xohlaysizmi?
5) DS granularity: Domenlar va aktivlar ikkalasi ham DS bo'lishi mumkin. Siyosatlarni ixtiyoriy meros qilib olish bilan ierarxik DSni (DS aktivining ota-onasi sifatidagi DS domeni) qo‘llab-quvvatlashimiz kerakmi yoki ularni v1 uchun tekis saqlashimiz kerakmi?
6) Og'ir ISIlar: soniyadan past dalillar keltira olmaydigan murakkab ISIlar uchun biz (a) ularni rad etishimiz, (b) bloklar bo'ylab kichikroq atom bosqichlariga bo'linishimiz yoki (c) aniq bayroqlar bilan kechiktirilgan kiritishga ruxsat berishimiz kerakmi?
7) O'zaro DS ziddiyatlari: mijoz tomonidan e'lon qilingan o'qish/yozish sozlamalari yetarlimi yoki xost xavfsizlik uchun (ko'proq nizolar evaziga) avtomatik ravishda xulosa qilishi va kengaytirishi kerakmi?

Ilova: Repozitariy siyosatiga muvofiqligi
- Norito barcha sim formatlari va Norito yordamchilari orqali JSON serializatsiyasi uchun ishlatiladi.
- faqat ABI v1; ABI siyosatlari uchun ish vaqti almashtirilmaydi. Syscall va marker tipidagi qo'shimchalar oltin testlar bilan hujjatlashtirilgan evolyutsiya jarayonini kuzatib boradi.
- apparat bo'ylab saqlanib qolgan determinizm; tezlashtirish ixtiyoriy va eshikli.
- ishlab chiqarish yo'llarida serda yo'q; ishlab chiqarishda atrof-muhitga asoslangan konfiguratsiya yo'q.