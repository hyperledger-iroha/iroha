---
slug: /nexus/confidential-assets
lang: uz
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Assets & ZK Transfers
description: Phase C blueprint for shielded circulation, registries, and operator controls.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Maxfiy aktivlar va ZK transfer dizayni

## Motivatsiya
- Domenlar shaffof muomalani o'zgartirmasdan tranzaksiya maxfiyligini saqlab qolishi uchun himoyalangan aktiv oqimlarini taqdim eting.
- Auditorlar va operatorlarni sxemalar va kriptografik parametrlar uchun hayot aylanishini boshqarish vositalari (faollashtirish, aylantirish, bekor qilish) bilan ta'minlash.

## Tahdid modeli
- Tasdiqlovchilar halol, ammo qiziquvchan: ular konsensusni sodiqlik bilan bajaradilar, lekin daftarni/davlatni tekshirishga harakat qilishadi.
- Tarmoq kuzatuvchilari blok ma'lumotlarini va g'iybat qilingan operatsiyalarni ko'rishadi; shaxsiy g'iybat kanallari haqida hech qanday taxmin yo'q.
- Qo'llash doirasi tashqarida: daftardan tashqari trafik tahlili, kvant raqiblari (PQ yo'l xaritasi bo'yicha alohida kuzatiladi), daftar mavjudligiga hujumlar.

## Dizayn umumiy ko'rinishi
- aktivlar mavjud shaffof balanslarga qo'shimcha ravishda *himoyalangan hovuz* e'lon qilishi mumkin; himoyalangan aylanish kriptografik majburiyatlar orqali ifodalanadi.
- Eslatmalar `(asset_id, amount, recipient_view_key, blinding, rho)` bilan qoplangan:
  - Majburiyat: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, nota tartibidan mustaqil.
  - Shifrlangan foydali yuk: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Norito kodli `ConfidentialTransfer` foydali yuklarni o'z ichiga olgan tranzaktsiyalar:
  - Umumiy ma'lumotlar: Merkle langari, bekor qiluvchilar, yangi majburiyatlar, aktiv identifikatori, sxema versiyasi.
  - Qabul qiluvchilar va ixtiyoriy auditorlar uchun shifrlangan yuklamalar.
  - Qiymatni saqlash, egalik qilish va avtorizatsiyani tasdiqlovchi nol bilim isboti.
- tekshirish kalitlari va parametrlar to'plami faollashtirish oynalari bo'lgan buxgalteriya registrlari orqali boshqariladi; tugunlar noma'lum yoki bekor qilingan yozuvlarga ishora qiluvchi dalillarni tasdiqlashdan bosh tortadi.
- Konsensus sarlavhalari faol konfidensial xususiyat dayjestiga amal qiladi, shuning uchun bloklar faqat registr va parametr holati mos kelganda qabul qilinadi.
- Proof konstruktsiyasi ishonchli sozlashsiz Halo2 (Plonkish) stekidan foydalanadi; Groth16 yoki boshqa SNARK variantlari v1 da ataylab qo‘llab-quvvatlanmaydi.

### Deterministik moslamalar

Maxfiy eslatma konvertlari endi `fixtures/confidential/encrypted_payload_v1.json` da kanonik moslama bilan jo'natiladi. Ma'lumotlar to'plami musbat v1 konvertini va salbiy noto'g'ri shakllangan namunalarni oladi, shuning uchun SDKlar tahlil paritetini tasdiqlay oladi. Rust ma'lumotlar modeli testlari (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) va Swift to'plami (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) ikkalasi ham moslamani to'g'ridan-to'g'ri yuklaydi, bu Norito kodlash, xato yuzalar va regressiya qamrovi kodek rivojlanishi bilan bir xilda qolishini kafolatlaydi.

Swift SDK-lar endi maxsus JSON elimsiz qalqon ko'rsatmalarini chiqarishi mumkin:
`ShieldRequest` 32 baytli nota majburiyati, shifrlangan foydali yuk va debet metamaʼlumotlari bilan,
keyin imzo qo'yish va uzatish uchun `IrohaSDK.submit(shield:keypair:)` (yoki `submitAndWait`) ga qo'ng'iroq qiling.
`/v1/pipeline/transactions` dan ortiq tranzaksiya. Yordamchi majburiyat muddatini tasdiqlaydi,
`ConfidentialEncryptedPayload` ni Norito kodlovchisiga ulaydi va `zk::Shield` ni aks ettiradi
tartibi quyida tasvirlangan, shuning uchun hamyonlar Rust bilan qulflangan qadamda qoladi.

## Konsensus majburiyatlari va imkoniyatlarni aniqlash
- Blok sarlavhalari `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` ni ko'rsatadi; dayjest konsensus xeshida ishtirok etadi va blokni qabul qilish uchun mahalliy registr ko'rinishiga teng bo'lishi kerak.
- Boshqaruv kelajakdagi `activation_height` bilan `next_conf_features` dasturlash orqali yangilanishlarni bosqichma-bosqich amalga oshirishi mumkin; bu balandlikka qadar blok ishlab chiqaruvchilari oldingi dayjestni chiqarishni davom ettirishlari kerak.
- Validator tugunlari `confidential.enabled = true` va `assume_valid = false` bilan ishlashi kerak. Agar ikkala shart bajarilmasa yoki mahalliy `conf_features` farq qilsa, ishga tushirish tekshiruvlari validator to'plamiga qo'shilishni rad etadi.
- P2P qoʻl siqish metamaʼlumotlariga endi `{ enabled, assume_valid, conf_features }` kiradi. Qo'llab-quvvatlanmaydigan xususiyatlarni reklama qiluvchi tengdoshlar `HandshakeConfidentialMismatch` bilan rad etiladi va hech qachon konsensus aylanishiga kirmaydi.
- Validator bo'lmagan kuzatuvchilar `assume_valid = true` ni belgilashlari mumkin; ular ko'r-ko'rona maxfiy deltalarni qo'llaydilar, lekin konsensus xavfsizligiga ta'sir qilmaydi.

## Obyekt siyosati
- Har bir aktiv taʼrifi yaratuvchi tomonidan yoki boshqaruv orqali oʻrnatilgan `AssetConfidentialPolicy` ga ega:
  - `TransparentOnly`: standart rejim; faqat shaffof ko'rsatmalarga (`MintAsset`, `TransferAsset` va boshqalar) ruxsat beriladi va himoyalangan operatsiyalar rad etiladi.
  - `ShieldedOnly`: barcha emissiya va o'tkazmalar maxfiy ko'rsatmalardan foydalanishi kerak; `RevealConfidential` taqiqlangan, shuning uchun balanslar hech qachon ommaga ko'rinmaydi.
  - `Convertible`: egalari quyidagi yoqish/o'chirish yo'riqnomalaridan foydalangan holda shaffof va ekranlangan tasvirlar o'rtasida qiymatni o'tkazishi mumkin.
- Siyosat pul mablag'larining qolib ketishining oldini olish uchun cheklangan FSMga amal qiladi:
  - `TransparentOnly → Convertible` (qalqonlangan hovuzni darhol yoqish).
  - `TransparentOnly → ShieldedOnly` (kutish kutilayotgan o'tish va konvertatsiya oynasini talab qiladi).
  - `Convertible → ShieldedOnly` (majburiy minimal kechikish).
  - `ShieldedOnly → Convertible` (migratsiya rejasi kerak, shuning uchun himoyalangan qaydlar sarflanishi mumkin).
  - `ShieldedOnly → TransparentOnly`, agar himoyalangan hovuz bo'sh bo'lmasa yoki boshqaruv muhim eslatmalarni himoya qiluvchi migratsiyani kodlamasa, ruxsat etilmaydi.
- Boshqaruv koʻrsatmalari `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` `ScheduleConfidentialPolicyTransition` ISI orqali oʻrnatiladi va `CancelConfidentialPolicyTransition` bilan rejalashtirilgan oʻzgarishlarni bekor qilishi mumkin. Mempool tekshiruvi hech qanday tranzaksiya o'tish balandligini bosib o'tmasligini ta'minlaydi va agar siyosat tekshiruvi o'rta blokni o'zgartirsa, qo'shilish aniq bajarilmaydi.
- Kutilayotgan o'tishlar yangi blok ochilganda avtomatik ravishda qo'llaniladi: blok balandligi konversiya oynasiga kirganda (`ShieldedOnly` yangilanishlari uchun) yoki dasturlashtirilgan `effective_height` ga yetganda, ish vaqti `AssetConfidentialPolicy` yangilanadi, `AssetConfidentialPolicy` yangilanadi, `AssetConfidentialPolicy` yangilanadi, I18NI00000 va metatalar tozalanadi. Agar `ShieldedOnly` o'tish muddati tugashi bilan shaffof ta'minot saqlanib qolsa, ish vaqti o'zgarishlarni bekor qiladi va oldingi rejimni o'zgarmagan holda ogohlantirishni qayd qiladi.
- `policy_transition_delay_blocks` va `policy_transition_window_blocks` konfiguratsiya tugmalari hamyonlarga kalit atrofida eslatmalarni aylantirish imkonini berish uchun minimal ogohlantirish va imtiyozli muddatlarni qo'llaydi.
- `pending_transition.transition_id` audit dastagi sifatida ishlaydi; Boshqaruv o'tishlarni yakunlash yoki bekor qilishda operatorlar hisobotlarni o'zaro bog'lashlari uchun iqtibos keltirishi kerak.
- `policy_transition_window_blocks` standarti 720 ga (≈12 soat, 60 soniya blokirovka vaqtida). Tugunlar qisqaroq xabar berishga harakat qiladigan boshqaruv so'rovlarini siqib chiqaradi.
- Ibtido namoyon bo'ladi va CLI joriy va kutilayotgan siyosatlarni yuzaga chiqaradi. Qabul qilish mantig'i har bir maxfiy ko'rsatmaga ruxsat berilganligini tasdiqlash uchun bajarilish vaqtida siyosatni o'qiydi.
- Migratsiya nazorat roʻyxati — Milestone M0 kuzatib boradigan bosqichma-bosqich yangilash rejasi uchun quyidagi “Migratsiya ketma-ketligi”ga qarang.

#### Torii orqali o'tishlarni kuzatish

Hamyonlar va auditorlar tekshirish uchun `GET /v1/confidential/assets/{definition_id}/transitions` so'rovi
faol `AssetConfidentialPolicy`. JSON foydali yuki har doim kanonikni o'z ichiga oladi
aktiv identifikatori, oxirgi kuzatilgan blok balandligi, siyosatning `current_mode`, rejim
bu balandlikda amal qiladi (konversiya oynalari vaqtincha `Convertible` haqida xabar beradi) va
kutilgan `vk_set_hash`/Poseidon/Pedersen parametr identifikatorlari. Boshqaruv bo'lganda
o'tish kutilmoqda, shuningdek, javob kiritiladi:

- `transition_id` - `ScheduleConfidentialPolicyTransition` tomonidan qaytarilgan audit dastagi.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` va olingan `window_open_height` (hamyonlar kerak bo'lgan blok
  ShieldedOnly cut-overs uchun konvertatsiya qilishni boshlang).

Javobga misol:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

`404` javobi mos keladigan aktiv taʼrifi yoʻqligini bildiradi. Hech qanday o'tish bo'lmaganda
rejalashtirilgan `pending_transition` maydoni `null`.

### Siyosat holati mashinasi| Joriy rejim | Keyingi rejim | Old shartlar | Samarali balandlikda ishlov berish | Eslatmalar |
|--------------------|------------------|----------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
| TransparentOnly | Konvertatsiya qilinadigan | Boshqaruv tasdiqlovchi/parametrlar registridagi yozuvlarni faollashtirdi. `ScheduleConfidentialPolicyTransition`ni `effective_height ≥ current_height + policy_transition_delay_blocks` bilan yuboring. | O'tish `effective_height` da aniq amalga oshiriladi; himoyalangan hovuz darhol mavjud bo'ladi.                   | Shaffof oqimlarni saqlashda maxfiylikni yoqish uchun standart yo'l.               |
| TransparentOnly | ShieldedOnly | Yuqoridagi kabi, shuningdek, `policy_transition_window_blocks ≥ 1`.                                                         | Ish vaqti avtomatik ravishda `Convertible`ga `effective_height - policy_transition_window_blocks` da kiradi; `effective_height` da `ShieldedOnly` ga aylanadi. | Shaffof ko'rsatmalar o'chirilishidan oldin deterministik konvertatsiya oynasini taqdim etadi.   |
| Konvertatsiya qilinadigan | ShieldedOnly | `effective_height ≥ current_height + policy_transition_delay_blocks` bilan rejalashtirilgan o'tish. Boshqaruv (`transparent_supply == 0`) audit metama'lumotlari orqali sertifikatlashi KERAK; ish vaqti buni kesishda amalga oshiradi. | Yuqoridagi kabi bir xil oyna semantikasi. Agar `effective_height` da shaffof ta'minot nolga teng bo'lmasa, `PolicyTransitionPrerequisiteFailed` bilan o'tish to'xtatiladi. | Aktivni to'liq maxfiy muomalaga kiritadi.                                     |
| ShieldedOnly | Konvertatsiya qilinadigan | Rejalashtirilgan o'tish; faol favqulodda chiqish yo'q (`withdraw_height` o'rnatilmagan).                                    | Davlat burilishlari `effective_height`; himoyalangan eslatmalar o'z kuchini saqlab qolganda, rampalar qayta ochiladi.                           | Xizmat oynalari yoki auditorlik tekshiruvlari uchun ishlatiladi.                                          |
| ShieldedOnly | TransparentOnly | Boshqaruv `shielded_supply == 0` ni isbotlashi yoki imzolangan `EmergencyUnshield` rejasini tuzishi kerak (auditor imzolari talab qilinadi). | Ish vaqti `effective_height` dan oldin `Convertible` oynasini ochadi; balandlikda maxfiy ko'rsatmalar bajarilmaydi va aktiv faqat shaffof rejimga qaytadi. | Yakuniy chiqish. Oyna davomida biron bir maxfiy eslatma sarflansa, o'tish avtomatik ravishda bekor qilinadi. |
| Har qanday | Hozirgi | bilan bir xil `CancelConfidentialPolicyTransition` kutilayotgan oʻzgarishlarni tozalaydi.                                                        | `pending_transition` darhol olib tashlandi.                                                                          | Status-kvoni saqlaydi; to'liqligi uchun ko'rsatilgan.                                             |

Yuqorida sanab o'tilmagan o'tishlar boshqaruvni taqdim etish paytida rad etiladi. Ish vaqti rejalashtirilgan o'tishni qo'llashdan oldin dastlabki shartlarni tekshiradi; Old shartlar bajarilmasa, aktivni avvalgi holatiga qaytaradi va telemetriya va blokirovka hodisalari orqali `PolicyTransitionPrerequisiteFailed` chiqaradi.

### Migratsiya ketma-ketligi

2. **O‘tish bosqichi:** `ScheduleConfidentialPolicyTransition` ni `effective_height` bilan `policy_transition_delay_blocks` ga mos ravishda yuboring. `ShieldedOnly` tomon harakatlanayotganda konvertatsiya oynasini belgilang (`window ≥ policy_transition_window_blocks`).
3. **Operator yo‘riqnomasini nashr qilish:** Qaytarilgan `transition_id` ni yozib oling va rampni yoqish/o‘chirish kitobini tarqating. Hamyonlar va auditorlar oynaning ochiq balandligini o'rganish uchun `/v1/confidential/assets/{id}/transitions` ga obuna bo'lishadi.
4. **Oynani qo'llash:** Oyna ochilganda, ish vaqti siyosatni `Convertible` ga o'zgartiradi, `PolicyTransitionWindowOpened { transition_id }` chiqaradi va ziddiyatli boshqaruv so'rovlarini rad etishni boshlaydi.
5. **Yakunlash yoki to‘xtatish:** `effective_height` da ish vaqti o‘tish shartlarini tekshiradi (nol shaffof ta’minot, favqulodda vaziyatlarda olib qo‘yish yo‘q va h.k.). Muvaffaqiyat siyosatni talab qilingan rejimga o'zgartiradi; muvaffaqiyatsizlik `PolicyTransitionPrerequisiteFailed` chiqaradi, kutilayotgan o'tishni o'chiradi va siyosatni o'zgarishsiz qoldiradi.
6. **Sxema yangilanishi:** Muvaffaqiyatli oʻtishdan soʻng boshqaruv aktivlar sxemasi versiyasini (masalan, `asset_definition.v2`) oʻzgartiradi va manifestlarni ketma-ketlashtirishda CLI vositalari `confidential_policy` ni talab qiladi. Genesis yangilash hujjatlari operatorlarga validatorlarni qayta ishga tushirishdan oldin siyosat sozlamalari va registr barmoq izlarini qo‘shishni buyuradi.

Maxfiylik bilan boshlangan yangi tarmoqlar to'g'ridan-to'g'ri genezisda kerakli siyosatni kodlaydi. Ular konversiya oynalari deterministik bo'lib qolishi va hamyonlarni sozlash uchun vaqtlari bo'lishi uchun ishga tushirilgandan keyin rejimlarni o'zgartirganda hamon yuqoridagi nazorat ro'yxatiga amal qilishadi.

### Norito manifest versiyasini yaratish va faollashtirish

- Ibtido manifestlari `confidential_registry_root` maxsus kaliti uchun `SetParameter` ni o'z ichiga olishi KERAK. Foydali yuk - Norito JSON mos keladigan `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: hech qanday tasdiqlovchi yozuvlari faol bo'lmaganda maydonni (`null`) o'tkazib yuboring, aks holda 32 baytlik olti burchakli qatorni (`0x…`) kiriting (`0x…`) `0x…`. manifestda yuborilgan tekshirgich ko'rsatmalari. Agar parametr etishmayotgan bo'lsa yoki xesh kodlangan ro'yxatga olish kitobiga rozi bo'lmasa, tugunlar boshlashni rad etadi.
- Simli `ConfidentialFeatureDigest::conf_rules_version` manifest layout versiyasini o'z ichiga oladi. V1 tarmoqlari uchun u `Some(1)` bo'lib qolishi KERAK va `iroha_config::parameters::defaults::confidential::RULES_VERSION` ga teng. Qoidalar to'plami o'zgarganda, konstantani yo'q qiling, manifestlarni qayta yarating va bloklash bosqichida ikkilik fayllarni chiqaring; versiyalarni aralashtirish validatorlarning `ConfidentialFeatureDigestMismatch` bilan bloklarni rad etishiga olib keladi.
- Faollashtirish manifestida ro'yxatga olish kitobi yangilanishlari, parametrlarning hayotiy siklidagi o'zgarishlar va siyosatga o'tishlar to'plami KERAK bo'lib, dayjest barqaror bo'lib qoladi:
  1. Rejalashtirilgan registr mutatsiyalarini (`Publish*`, `Set*Lifecycle`) oflayn holat ko'rinishida qo'llang va `compute_confidential_feature_digest` bilan faollashtirilgandan keyingi dayjestni hisoblang.
  2. Hisoblangan xesh yordamida `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` chiqaring, shunda ortda qolgan tengdoshlar oraliq registr ko'rsatmalarini o'tkazib yuborsalar ham to'g'ri dayjestni tiklashlari mumkin.
  3. `ScheduleConfidentialPolicyTransition` ko'rsatmalarini qo'shing. Har bir ko'rsatma boshqaruv tomonidan chiqarilgan `transition_id` dan iqtibos keltirishi kerak; unutgan manifestlar ish vaqti tomonidan rad etiladi.
  4. Manifest baytlarini, SHA-256 barmoq izini va faollashtirish rejasida ishlatiladigan dayjestni saqlang. Operatorlar qismlarga bo'linmaslik uchun manifestga ovoz berishdan oldin barcha uchta artefaktni tekshiradilar.
- Chiqarishlar kechiktirilgan kesishni talab qilganda, maqsadli balandlikni moslashtirilgan parametrga yozib oling (masalan, `custom.confidential_upgrade_activation_height`). Bu auditorlarga Norito bilan kodlangan dalil beradi, validatorlar dayjest o'zgarishi kuchga kirgunga qadar ogohlantirish oynasini hurmat qilgan.

## Tekshiruvchi va Parametrning hayot aylanishi
### ZK reestri
- Ledger `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` ni saqlaydi, bu erda `proving_system` hozirda `Halo2` ga o'rnatiladi.
- `(circuit_id, version)` juftlari global miqyosda noyobdir; reestr elektron metama'lumotlar bo'yicha qidirish uchun ikkinchi darajali indeksni saqlaydi. Qabul paytida ikki nusxadagi juftlikni ro'yxatdan o'tkazishga urinishlar rad etiladi.
- `circuit_id` bo'sh bo'lmasligi va `public_inputs_schema_hash` ko'rsatilishi kerak (odatda tekshirgichning kanonik ommaviy kirish kodlashining Blake2b-32 xeshi). Qabul qilish ushbu maydonlarni o'tkazib yuborgan yozuvlarni rad etadi.
- Boshqaruv ko'rsatmalariga quyidagilar kiradi:
  - `PUBLISH` faqat metadata bilan `Proposed` yozuvini qo'shish uchun.
  - `ACTIVATE { vk_id, activation_height }` davr chegarasida kirishni faollashtirishni rejalashtirish uchun.
  - Yakuniy balandlikni belgilash uchun `DEPRECATE { vk_id, deprecation_height }`, bunda dalillar yozuvga murojaat qilishi mumkin.
  - favqulodda o'chirish uchun `WITHDRAW { vk_id, withdraw_height }`; ta'sirlangan aktivlar yangi yozuvlar faollashgunga qadar olib qo'yish balandligidan keyin maxfiy xarajatlarni muzlatib qo'yadi.
- `vk_set_hash` faol yozuvlarga mos keladigan `confidential_registry_root` maxsus parametrini Ibtido manifestlari avtomatik ravishda chiqaradi; validation tugun konsensusga kirishidan oldin ushbu dayjestni mahalliy ro'yxatga olish holati bilan o'zaro tekshiradi.
- Verifierni ro'yxatdan o'tkazish yoki yangilash uchun `gas_schedule_id` talab qilinadi; tekshirish roʻyxatga olish kitobi `Active` ekanligini, `(circuit_id, version)` indeksida mavjudligini va Halo2 dalillari `OpenVerifyEnvelope` ni taʼminlaganligini, `circuit_id`, `circuit_id`, I18NI000000, I15NI00X00010101010101 bilan mos kelishini taʼminlaydi. ro'yxatga olish kitobi.

### Tasdiqlash kalitlari
- Tasdiqlash kalitlari kitobdan tashqarida qoladi, lekin tekshirgich metamaʼlumotlari bilan birga chop etilgan kontent manzilli identifikatorlar (`pk_cid`, `pk_hash`, `pk_len`) tomonidan havola qilinadi.
- Wallet SDK'lari PK ma'lumotlarini oladi, xeshlarni tekshiradi va mahalliy ravishda keshlaydi.

### Pedersen va Poseidon parametrlari
- Alohida registrlar (`PedersenParams`, `PoseidonParams`), har biri `params_id`, generatorlar/doimiylar xeshlari, faollashtirish, eskirish va olib tashlash balandliklariga ega bo'lgan ko'zgu tekshiruvi hayot aylanishini boshqarish.## Deterministik tartiblash va bekor qiluvchilar
- Har bir aktiv `next_leaf_index` bilan `CommitmentTree` ni saqlaydi; bloklar majburiyatlarni deterministik tartibda qo'shadi: tranzaktsiyalarni blok tartibida takrorlash; Har bir tranzaksiya doirasida `output_idx` seriyali ko'tarilish orqali himoyalangan chiqishlarni takrorlang.
- `note_position` daraxt ofsetlaridan olingan, lekin nullifierning bir qismi emas**; u faqat dalil guvohi ichida a'zolik yo'llarini oziqlantiradi.
- reorgs ostida nullifier barqarorligi PRF dizayni bilan kafolatlanadi; PRF kiritish `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` bilan bog'lanadi va `max_anchor_age_blocks` bilan cheklangan tarixiy Merkle ildizlariga langar qo'yadi.

## Hisob kitobi oqimi
1. **MintConfidential { asset_id, summa, recipient_shint }**
   - `Convertible` yoki `ShieldedOnly` aktiv siyosatini talab qiladi; qabul qilish aktivlar vakolatini tekshiradi, joriy `params_id`ni oladi, `rho` namunalarini oladi, majburiyatlarni chiqaradi, Merkle daraxtini yangilaydi.
   - Yangi majburiyat, Merkle ildiz deltasi va audit izlari uchun tranzaksiya chaqiruvi xesh bilan `ConfidentialEvent::Shielded` chiqaradi.
2. **TransferConfidential { asset_id, proof, circuit_id, version, noulifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - VM syscall ro'yxatga olish kitobi orqali dalilni tekshiradi; xost bekor qiluvchilarning foydalanilmaganligini ta'minlaydi, majburiyatlar deterministik tarzda qo'shiladi, langar yaqinda.
   - Ledger `NullifierSet` yozuvlarini qayd qiladi, qabul qiluvchilar/auditorlar uchun shifrlangan foydali yuklarni saqlaydi va `ConfidentialEvent::Transferred` xulosalarini bekor qiluvchilar, tartiblangan natijalar, isbot xesh va Merkle ildizlarini chiqaradi.
3. **RevealConfidential { asset_id, proof, circuit_id, versiya, nullifier, summa, recipient_count, anchor_root }**
   - Faqat `Convertible` aktivlari uchun mavjud; proof banknot qiymatini aniqlangan summaga tengligini tasdiqlaydi, buxgalteriya hisobi shaffof balansni kreditlaydi va sarflangan bekor qiluvchini belgilash orqali ekranlangan banknotni yoqib yuboradi.
   - `ConfidentialEvent::Unshielded` ni umumiy miqdor, iste'mol qilingan nullifiers, isbot identifikatorlari va tranzaksiya chaqiruvi xesh bilan chiqaradi.

## Ma'lumotlar modeli qo'shimchalari
- Yoqish bayrog'i bilan `ConfidentialConfig` (yangi konfiguratsiya bo'limi), `assume_valid`, gaz/cheklash tugmalari, langar oynasi, tekshirgich orqa tomoni.
- Ochiq versiya baytli (`CONFIDENTIAL_ASSET_V1 = 0x01`) `ConfidentialNote`, `ConfidentialTransfer` va `ConfidentialMint` Norito sxemalari.
- `ConfidentialEncryptedPayload` AEAD memo baytlarini `{ version, ephemeral_pubkey, nonce, ciphertext }` bilan o'rab oladi, XChaCha20-Poly1305 tartibi uchun sukut bo'yicha `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1`.
- kanonik kalit hosila vektorlari `docs/source/confidential_key_vectors.json` da yashaydi; CLI va Torii so'nggi nuqtalari ushbu qurilmalarga nisbatan regressga uchraydi.
- `asset::AssetDefinition` `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }` ga erishadi.
- `ZkAssetState` uzatish/ekrandan chiqarish tekshiruvchilari uchun `(backend, name, commitment)` majburiyligini saqlab qoladi; bajarilish havola qilingan yoki inline tekshirish kaliti ro'yxatdan o'tgan majburiyatga mos kelmagan dalillarni rad etadi.
- `CommitmentTree` (chegara oʻtkazish punktlari boʻlgan obyekt uchun), `NullifierSet` kalitlari `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` dunyoda saqlanadi.
- Mempool dublikatlarni erta aniqlash va langar yoshini tekshirish uchun vaqtinchalik `NullifierIndex` va `AnchorIndex` tuzilmalarini saqlaydi.
- Norito sxema yangilanishlari ommaviy kirishlar uchun kanonik tartiblashni o'z ichiga oladi; aylanish testlari kodlash determinizmini ta'minlaydi.
- Shifrlangan foydali yuk aylanma sayohatlar birlik sinovlari (`crates/iroha_data_model/src/confidential.rs`) orqali bloklanadi. Kuzatuv hamyon vektorlari auditorlar uchun kanonik AEAD transkriptlarini biriktiradi. `norito.md` konvert uchun sim sarlavhasini hujjatlashtiradi.

## IVM Integratsiya va Syscall
- Qabul qiluvchi `VERIFY_CONFIDENTIAL_PROOF` tizimi qo'ng'iroqlarini joriy qiling:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` va natijada `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall registrdan tekshirgich meta-ma'lumotlarini yuklaydi, o'lcham/vaqt chegaralarini qo'llaydi, deterministik gazni to'laydi va faqat isbot muvaffaqiyatli bo'lsa, deltani qo'llaydi.
- Xost Merkle ildiz snapshotlarini va nullifier holatini olish uchun faqat o'qish uchun `ConfidentialLedger` xususiyatini ochib beradi; Kotodama kutubxonasi guvohlarni yig'ish yordamchilari va sxemani tekshirishni ta'minlaydi.
- Pointer-ABI hujjatlari tasdiqlovchi bufer tartibini va ro'yxatga olish kitobi tutqichlarini aniqlashtirish uchun yangilandi.

## Tugun qobiliyati bo'yicha muzokaralar
- Handshake `feature_bits.confidential` ni `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }` bilan birga reklama qiladi. Validator ishtiroki uchun `confidential.enabled=true`, `assume_valid=false`, bir xil verifier backend identifikatorlari va mos keladigan dayjestlar talab qilinadi; mos kelmasligi `HandshakeConfidentialMismatch` bilan qoʻl siqishda muvaffaqiyatsizlikka uchraydi.
- Konfiguratsiya faqat kuzatuvchi tugunlari uchun `assume_valid` ni qo'llab-quvvatlaydi: o'chirilganda, maxfiy ko'rsatmalarga duch kelsangiz, vahimasiz `UnsupportedInstruction` deterministik beradi; yoqilganda, kuzatuvchilar dalillarni tasdiqlamasdan e'lon qilingan holat deltalarini qo'llaydilar.
- Agar mahalliy imkoniyatlar o'chirilgan bo'lsa, Mempool maxfiy tranzaksiyalarni rad etadi. G'iybat filtrlari o'lcham chegaralari ichida noma'lum tasdiqlovchi identifikatorlarni ko'r-ko'rona yo'naltirish bilan birga, mos keladigan qobiliyatsiz himoyalangan tranzaktsiyalarni tengdoshlarga yuborishdan qochadi.

### Azizillo va bekor qiluvchini saqlash siyosatini ko'rsating

Maxfiy daftarlar eslatmaning yangiligini isbotlash uchun etarli tarixni saqlashi kerak
boshqaruvga asoslangan auditlarni takrorlang. Birlamchi siyosat, tomonidan amalga oshiriladi
`ConfidentialLedger`, bu:

- **Nullifikatorni ushlab turish:** sarflangan bekor qiluvchilarni *minimal* `730` kun davomida saqlang (24
  oy) xarajat balandligidan keyin yoki agar uzoqroq bo'lsa, regulyator tomonidan belgilangan oyna.
  Operatorlar oynani `confidential.retention.nullifier_days` orqali kengaytirishlari mumkin.
  Saqlash oynasidan kichik bo'lmagan nullifierlar Torii orqali so'ralishi KERAK.
  auditorlar ikki marta xarajat yo'qligini isbotlash mumkin.
- **Oshkora Azizillo:** shaffof ochiladi (`RevealConfidential`)
  blok yakunlangandan so'ng darhol tegishli eslatma majburiyatlari, lekin
  iste'mol qilingan nullifier yuqoridagi saqlash qoidasiga bo'ysunadi. Oshkora bilan bog'liq
  hodisalar (`ConfidentialEvent::Unshielded`) davlat miqdorini, oluvchini,
  va isboti hash shunday rekonstruksiya tarixiy vahiylarni kesish kerak emas
  shifrlangan matn.
- **Chegara nazorat punktlari: ** majburiyat chegaralari aylanma nazorat punktlarini saqlaydi
  kattaroq `max_anchor_age_blocks` va saqlash oynasini qamrab oladi. Tugunlar
  ixcham eski nazorat punktlari faqat intervaldagi barcha bekor qiluvchilarning muddati tugaganidan keyin.
- **Eskirgan hazm qilish:** agar `HandshakeConfidentialMismatch` ko'tarilgan bo'lsa
  Driftni hazm qilish uchun operatorlar (1) nullifierni ushlab turish oynalarini tekshirishlari kerak
  klaster bo'ylab tekislang, (2) `iroha_cli app confidential verify-ledger` ni ishga tushiring
  saqlangan nullifier to'plamiga qarshi digestni qayta tiklash va (3) qayta joylashtirish
  yangilangan manifest. Vaqtdan oldin kesilgan har qanday nullifiers dan tiklanishi kerak
  tarmoqqa qayta ulanishdan oldin sovuq saqlash.

Operatsiyalar kitobida mahalliy bekor qilishni hujjatlash; boshqaruv siyosati kengaymoqda
saqlash oynasi tugun konfiguratsiyasi va arxivni saqlash rejalarini yangilashi kerak
qulflangan qadam.

### Ko'chirish va tiklash oqimi

1. Terish paytida `IrohaNetwork` e'lon qilingan imkoniyatlarni taqqoslaydi. Har qanday nomuvofiqlik `HandshakeConfidentialMismatch` ni oshiradi; ulanish yopiladi va peer hech qachon `Ready` ga ko'tarilmasdan kashfiyot navbatida qoladi.
2. Muvaffaqiyatsizlik tarmoq xizmati jurnali (jumladan, masofaviy dayjest va backend) orqali yuzaga keladi va Sumeragi hech qachon taklif yoki ovoz berish uchun tengdoshni rejalashtirmaydi.
3. Operatorlar tekshirgich registrlari va parametrlar to'plamini (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) yoki `next_conf_features`ni kelishilgan `next_conf_features` bilan moslashtirish orqali tuzatadi. Dijest mos kelgandan so'ng, keyingi qo'l siqish avtomatik ravishda muvaffaqiyatli bo'ladi.
4. Agar eski tengdosh blokni translyatsiya qilishga muvaffaq bo'lsa (masalan, arxivni takrorlash orqali), validatorlar uni `BlockRejectionReason::ConfidentialFeatureDigestMismatch` bilan aniq rad etadilar, bu esa tarmoq bo'ylab daftar holatini izchil saqlaydi.

### Qayta o'ynash uchun xavfsiz qo'l siqish oqimi

1. Har bir chiqish urinishi yangi Shovqin/X25519 asosiy materialini ajratadi. Imzolangan qoʻl siqish yuki (`handshake_signature_payload`) mahalliy va masofaviy vaqtinchalik ochiq kalitlarni, Norito kodli reklama qilingan rozetka manzilini va `handshake_chain_id` bilan tuzilganda zanjir identifikatorini birlashtiradi. Xabar tugunni tark etishidan oldin AEAD shifrlangan.
2. Javob beruvchi foydali yukni teng/mahalliy kalit tartibi teskari hisoblab chiqadi va `HandshakeHelloV1` ichiga o'rnatilgan Ed25519 imzosini tekshiradi. Efemer kalitlar ham, e'lon qilingan manzil ham imzo domenining bir qismi bo'lganligi sababli, olingan xabarni boshqa tengdoshga qarshi takrorlash yoki eskirgan ulanishni tiklash aniq tekshirilmaydi.
3. Maxfiy qobiliyat bayroqlari va `ConfidentialFeatureDigest` `HandshakeConfidentialMeta` ichida harakatlanadi. Qabul qiluvchi `{ enabled, assume_valid, verifier_backend, digest }` kortejini mahalliy konfiguratsiya qilingan `ConfidentialHandshakeCaps` bilan taqqoslaydi; har qanday nomuvofiqlik `Ready` ga transport oʻtishdan oldin `HandshakeConfidentialMismatch` bilan erta chiqib ketadi.
4. Qayta ulanishdan oldin operatorlar dayjestni qayta hisoblashi (`compute_confidential_feature_digest` orqali) va tugunlarni yangilangan registrlar/siyosatlar bilan qayta ishga tushirishi KERAK. Eski dayjestlarni reklama qilayotgan tengdoshlar qo'l siqishda davom etmay, eski holatni validator to'plamiga qayta kirishiga yo'l qo'ymaydi.
5. Qo‘l siqishdagi muvaffaqiyat va muvaffaqiyatsizliklar standart `iroha_p2p::peer` hisoblagichlarini (`handshake_failure_count`, xato taksonomiyasi yordamchilari) yangilaydi va masofaviy tengdosh identifikatori va dayjest barmoq izi bilan belgilangan tizimli jurnal yozuvlarini chiqaradi. Qayta o'ynashga urinishlar yoki tarqatish paytida noto'g'ri konfiguratsiyalarni aniqlash uchun ushbu ko'rsatkichlarni kuzatib boring.## Kalitlarni boshqarish va foydali yuklar
- Hisob uchun kalitlarni hosil qilish ierarxiyasi:
  - `sk_spend` → `nk` (nullifier kaliti), `ivk` (kirish ko'rish kaliti), `ovk` (chiqish ko'rish kaliti), `fvk`.
- Shifrlangan eslatma yuklamalari ECDH-dan olingan umumiy kalitlarga ega AEAD-dan foydalanadi; ixtiyoriy auditor ko'rinishi kalitlari har bir aktiv siyosati natijalariga biriktirilishi mumkin.
- CLI qo'shimchalari: `confidential create-keys`, `confidential send`, `confidential export-view-key`, eslatmalar shifrini ochish uchun auditor asboblari va Kotodama oflayn konvertlarini ishlab chiqarish/tekshirish uchun `iroha app zk envelope` yordamchisi. Torii `POST /v1/confidential/derive-keyset` orqali bir xil hosila oqimini ochib beradi, ham hex va base64 shakllarini qaytaradi, shuning uchun hamyonlar asosiy ierarxiyalarni dasturiy tarzda olishlari mumkin.

## Gaz, limitlar va DoS boshqaruvlari
- deterministik gaz jadvali:
  - Halo2 (Plonkish): tayanch `250_000` gaz + `2_000` gaz har bir umumiy kirish.
  - Har bir isbot bayti uchun `5` gaz, ortiqcha har bir nullifier (`300`) va har bir majburiyat (`500`) toʻlovlari.
  - Operatorlar ushbu konstantalarni tugun konfiguratsiyasi (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`) orqali bekor qilishi mumkin; o'zgarishlar ishga tushirilganda yoki konfiguratsiya qatlami qayta yuklanganda tarqaladi va klaster bo'ylab aniq qo'llaniladi.
- Qattiq chegaralar (sozlanishi mumkin bo'lgan standart):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. `verify_timeout_ms` dan oshib ketgan dalillar ko'rsatmani aniq bekor qiladi (boshqaruv byulletenlari `proof verification exceeded timeout` chiqaradi, `VerifyProof` xatoni qaytaradi).
- Qo'shimcha kvotalar jonlilikni ta'minlaydi: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` va `max_public_inputs` bog'langan blok quruvchilar; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) chegara nazorat punktini saqlashni boshqaradi.
- Ish vaqtining bajarilishi endi ushbu tranzaksiya yoki blok limitlaridan oshib ketadigan tranzaktsiyalarni rad etadi, `InvalidParameter` deterministik xatolarni chiqaradi va daftar holatini o'zgarishsiz qoldiradi.
- Mempool resursdan foydalanishni chegaralangan holda saqlash uchun tekshirgichni chaqirishdan oldin maxfiy tranzaktsiyalarni `vk_id`, isbot uzunligi va langar yoshi bo'yicha oldindan filtrlaydi.
- Vaqt tugashi yoki majburiyatlarning buzilishi bilan tekshirish qat'iy ravishda to'xtatiladi; tranzaktsiyalar aniq xatolar bilan muvaffaqiyatsizlikka uchraydi. SIMD orqa uchlari ixtiyoriy, lekin gaz hisobini o'zgartirmaydi.

### Kalibrlash asoslari va qabul qilish eshiklari
- **Malumot platformalari.** Kalibrlash ishlari quyida keltirilgan uchta apparat profilini qamrab olishi SHART. Barcha profillarni yozib ololmaydigan yugurishlar ko'rib chiqish vaqtida rad etiladi.

  | Profil | Arxitektura | CPU / Instance | Kompilyator bayroqlari | Maqsad |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) yoki Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Vektor intrinsiklarisiz zamin qiymatlarini o'rnatish; zaxira xarajatlar jadvallarini sozlash uchun ishlatiladi. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | standart versiya | AVX2 yo'lini tasdiqlaydi; SIMD tezlashuvining neytral gazga tolerantlik darajasida qolishini tekshiradi. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | standart versiya | NEON backend deterministik va x86 jadvallariga mos kelishini ta'minlaydi. |

- **Benchmark jabduqlar.** Gazni kalibrlash bo'yicha barcha hisobotlar quyidagilar bilan tayyorlanishi kerak:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - Deterministik moslamani tasdiqlash uchun `cargo test -p iroha_core bench_repro -- --ignored`.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` VM opcode xarajatlari har doim o'zgarganda.

- ** Ruxsat etilgan tasodifiylik.** `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` skameykalarni ishga tushirishdan oldin eksport qiling, shunda `iroha_test_samples::gen_account_in` deterministik `KeyPair::from_seed` yo'liga o'tadi. Jabduqlar `IROHA_CONF_GAS_SEED_ACTIVE=…` bir marta chop etadi; agar o'zgaruvchi yo'q bo'lsa, ko'rib chiqish muvaffaqiyatsizlikka uchradi. Har qanday yangi kalibrlash yordam dasturlari yordamchi tasodifiylikni kiritishda ushbu env varni hurmat qilishni davom ettirishi kerak.

- **Natijani olish.**
  - Har bir profil uchun mezon xulosalarini (`target/criterion/**/raw.csv`) chiqarish artefaktiga yuklang.
  - Olingan ko'rsatkichlarni (`ns/op`, `gas/op`, `ns/gas`) [Maxfiy gaz kalibrlash kitobi](./confidential-gas-calibration) da, foydalanilgan git commit va kompilyator versiyasi bilan birga saqlang.
  - Har bir profil uchun oxirgi ikkita asosiy chiziqni saqlang; eng yangi hisobot tasdiqlangandan so'ng eski suratlarni o'chirib tashlang.

- **Qabul qilish tolerantliklari.**
  - `baseline-simd-neutral` va `baseline-avx2` oralig'idagi gaz deltalari ≤ ±1,5% BO'LISHI KERAK.
  - `baseline-simd-neutral` va `baseline-neon` oralig'idagi gaz deltalari ≤ ±2,0% BO'LISHI KERAK.
  - Ushbu chegaralardan oshib ketgan kalibrlash takliflari jadvalga tuzatishlar kiritishni yoki nomuvofiqlik va yumshatishni tushuntiruvchi RFCni talab qiladi.

- **Tekshirish roʻyxatini koʻrib chiqing.** Taqdimotchilar quyidagilar uchun javobgardirlar:
  - Kalibrlash jurnaliga `uname -a`, `/proc/cpuinfo` parchalari (model, qadam) va `rustc -Vv` kiradi.
  - `IROHA_CONF_GAS_SEED` dastgoh chiqishida aks-sado berilganligini tekshirish (skameykalar faol urug'ni chop etadi).
  - Elektron yurak stimulyatori va konfidensial tekshirgich funksiyalarini ko'zgu ishlab chiqarishni ta'minlash (`--features confidential,telemetry` Telemetriya bilan skameykalarda ishlayotganda).

## Konfiguratsiya va operatsiyalar
- `iroha_config` `[confidential]` qismini oladi:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetriya jami ko'rsatkichlarni chiqaradi: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, I18NI000003110, I18NI00000311030, ochiq matnli ma'lumotlar.
- RPC sirtlari:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Sinov strategiyasi
- Determinizm: bloklar ichida tasodifiy tranzaksiyalarni aralashtirish bir xil Merkle ildizlari va nullifier to'plamlarini beradi.
- Qayta tuzilishga chidamlilik: langar bilan ko'p blokli reorglarni simulyatsiya qilish; nullifiers barqaror qoladi va eskirgan langarlar rad etiladi.
- Gaz invariantlari: SIMD tezlashuvi bo'lgan va bo'lmagan tugunlarda bir xil gazdan foydalanishni tekshiring.
- Chegaraviy test: o'lchamdagi / gaz shiftidagi dalillar, maksimal kirish / chiqish hisoblari, vaqt tugashi.
- Hayotiy tsikl: tekshirgich va parametrlarni faollashtirish/eskirish uchun boshqaruv operatsiyalari, aylanish xarajatlari testlari.
- FSM siyosati: ruxsat etilgan/ruxsat etilmagan o'tishlar, kutilayotgan o'tish kechikishlari va samarali balandliklar atrofida mempulni rad etish.
- Favqulodda ro'yxatga olish kitobi: favqulodda olib qo'yish ta'sirlangan aktivlarni `withdraw_height` da muzlatib qo'yadi va keyin dalillarni rad etadi.
- Imkoniyatlar chegarasi: mos kelmaydigan `conf_features` rad etish bloklari bo'lgan validatorlar; `assume_valid=true` bilan kuzatuvchilar konsensusga ta'sir qilmasdan turib.
- Davlat ekvivalentligi: validator/to'liq/kuzatuvchi tugunlari kanonik zanjirda bir xil holat ildizlarini hosil qiladi.
- Salbiy noaniqlik: noto'g'ri shakllangan dalillar, katta hajmdagi foydali yuklar va bekor qiluvchi to'qnashuvlar qat'iy ravishda rad etadi.

## Ajoyib ish
- Benchmark Halo2 parametr to'plamlarini (sxema o'lchami, qidirish strategiyasi) va natijalarni kalibrlash kitobiga yozib oling, shunda gaz/vaqt tugashining sukut bo'yicha parametrlari keyingi `confidential_assets_calibration.md` yangilanishi bilan birga yangilanishi mumkin.
- Boshqaruv loyihasi imzolangandan so'ng tasdiqlangan ish oqimini Torii ga ulab, auditorlik ma'lumotlarini oshkor qilish siyosati va tegishli tanlab ko'rish API'larini yakunlang.
- SDKni amalga oshiruvchilar uchun konvert formatini hujjatlashtirib, ko'p qabul qiluvchi chiqishi va paketli eslatmalarni qamrab olish uchun guvohlarni shifrlash sxemasini kengaytiring.
- Sxemalar, registrlar va parametrlarni aylantirish tartib-qoidalarining tashqi xavfsizlik tekshiruvini o'tkazish va ichki audit hisobotlari yonida topilmalarni arxivlash.
- Hamyon sotuvchilari bir xil attestatsiya semantikasini amalga oshirishi uchun auditor xarajatlarini solishtirish API-larini belgilang va ko'rish kaliti ko'rsatmalarini nashr eting.## Amalga oshirish bosqichlari
1. **M0 fazasi — Kemani to‘xtatish**
   - ✅ Nullifier hosilasi endi Poseidon PRF dizayniga (`nk`, `rho`, `asset_id`, `chain_id`) amal qiladi va buxgalteriya daftarini yangilashda deterministik majburiyatlarni buyurtma qilish amalga oshiriladi.
   - ✅ Amalga oshirish deterministik xatolar bilan byudjetdan ortiq tranzaktsiyalarni rad etib, isbot o'lchami chegaralarini va har bir tranzaksiya/blok uchun maxfiy kvotalarni qo'llaydi.
   - ✅ P2P qoʻl siqish `ConfidentialFeatureDigest` (backend dayjesti + registr barmoq izlari) ni reklama qiladi va `HandshakeConfidentialMismatch` orqali aniq mos kelmaslikni bartaraf etadi.
   - ✅ Maxfiy ijro yo'llaridagi vahimalarni olib tashlang va mos keladigan qobiliyatsiz tugunlar uchun rolli eshiklarni qo'shing.
   - ⚪ Chegara oʻtkazish punktlari uchun tekshirish muddati tugashi byudjetlarini va chuqurlik chegaralarini oʻzgartirishni taʼminlash.
     - ✅ Tasdiqlash muddati tugaydigan byudjetlar amalga oshirildi; `verify_timeout_ms` dan oshgan dalillar endi deterministik ravishda muvaffaqiyatsizlikka uchraydi.
     - ✅ Chegara nazorat punktlari endi `reorg_depth_bound` ni hurmat qiladi, deterministik suratlarni saqlagan holda sozlangan oynadan eskiroq nazorat punktlarini kesib tashlaydi.
   - `AssetConfidentialPolicy`, FSM siyosati va yalpiz/o'tkazish/oshkor qilish ko'rsatmalari uchun ijro eshiklarini joriy qiling.
   - Blok sarlavhalarida `conf_features` so'rovini bajaring va registr/parametr dayjestlari farqlanganda validator ishtirokini rad eting.
2. **M1 bosqich — registrlar va parametrlar**
   - Land `ZkVerifierEntry`, `PedersenParams` va `PoseidonParams` registrlari boshqaruv operatsiyalari, genezis ankrajlari va keshlarni boshqarish.
   - Ro'yxatga olish kitobini qidirish, gaz jadvali identifikatorlari, sxemalarni xeshlash va o'lchamlarni tekshirishni talab qilish uchun simli tizim.
   - Shifrlangan foydali yuk formati v1, hamyon kalitini olish vektorlarini jo'natish va maxfiy kalitlarni boshqarish uchun CLI yordami.
3. **M2 fazasi — gaz va ishlash**
   - Deterministik gaz jadvalini, blokli hisoblagichlarni va telemetriya bilan taqqoslanadigan jabduqlarni amalga oshiring (kechikish vaqtini, isbot o'lchamlarini, mempulni rad etishni tekshiring).
   - Harden CommitmentTree nazorat punktlari, LRU yuklash va ko'p aktivli ish yuklari uchun bekor qiluvchi indekslar.
4. **M3 fazasi — aylanish va hamyon asboblari**
   - Ko'p parametrli va ko'p versiyali isbotni qabul qilishni yoqish; boshqaruvga asoslangan faollashtirish/eskirishni o'tish davri kitoblari bilan qo'llab-quvvatlash.
   - Hamyon SDK/CLI migratsiya oqimlarini, auditor skanerlash ish oqimlarini va sarflangan mablag'larni solishtirish vositalarini taqdim eting.
5. **M4 bosqich — Audit va operatsiyalar**
   - Auditorning asosiy ish oqimlarini, tanlab ochish API'larini va operatsion ish kitoblarini taqdim eting.
   - Tashqi kriptografiya/xavfsizlik tekshiruvini rejalashtiring va natijalarni `status.md` da chop eting.

Har bir bosqich blokcheyn tarmog'i uchun deterministik bajarilish kafolatlarini saqlab qolish uchun yo'l xaritasi bosqichlarini va tegishli testlarni yangilaydi.