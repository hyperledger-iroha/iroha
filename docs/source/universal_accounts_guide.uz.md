---
lang: uz
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f972f8f82b7f4e89c1d48b0dbbc6eb5b73303e2fab0f580ab21e63990ba03af8
source_last_modified: "2026-03-27T19:05:17.617064+00:00"
translation_last_reviewed: 2026-03-28
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Universal hisob qo'llanmasi

Ushbu qo'llanma UAID (Universal Account ID) tarqatish talablarini distilatsiya qiladi
Nexus yo'l xaritasi va ularni operator + SDK yo'nalishiga yo'naltirilgan ko'rsatmalarga to'playdi.
U UAID hosilasi, portfel/manifest tekshiruvi, regulyator shablonlarini,
va har bir `iroha ilovasi makon katalogi manifestiga hamroh bo'lishi kerak bo'lgan dalillar
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID tezkor ma'lumotnomasi- UAIDlar `uaid:<hex>` literallari, `<hex>` esa Blake2b-256 dayjestidir.
  LSB `1` ga o'rnatiladi. Kanonik tip yashaydi
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Hisob qaydnomalarida (`Account` va `AccountDetails`) endi ixtiyoriy `uaid` mavjud
  maydon, shuning uchun ilovalar identifikatorni maxsus xeshlashsiz o'rganishi mumkin.
- Yashirin funksiya identifikatori siyosatlari o'zboshimchalik bilan normallashtirilgan kirishlarni bog'lashi mumkin
  (telefon raqamlari, elektron pochta manzillari, hisob raqamlari, hamkor satrlari) `opaque:` identifikatorlariga
  UAID nom maydoni ostida. Zanjirdagi qismlar `IdentifierPolicy`,
  `IdentifierClaimRecord` va `opaque_id -> uaid` indeksi.
- Space Directory har bir UAIDni bog'laydigan `World::uaid_dataspaces` xaritasini saqlaydi.
  faol manifestlar tomonidan havola qilingan ma'lumotlar maydoni hisoblariga. Torii buni qayta ishlatadi
  `/portfolio` va `/uaids/*` API uchun xarita.
- `POST /v1/accounts/onboard` standart kosmik katalog manifestini nashr etadi
  global ma'lumotlar maydoni mavjud bo'lmaganda, shuning uchun UAID darhol bog'lanadi.
  Bort idoralari `CanPublishSpaceDirectoryManifest{dataspace=0}` ni ushlab turishi kerak.
- Barcha SDKlar UAID literallarini kanoniklashtirish uchun yordamchilarni ko'rsatadi (masalan,
  Android SDK da `UaidLiteral`). Yordamchilar xom 64-hex hazm qilishni qabul qiladilar
  (LSB=1) yoki `uaid:<hex>` literallari va bir xil Norito kodeklarini qayta ishlating.
  digest tillar bo'ylab o'tib keta olmaydi.

## 1.1 Yashirin identifikator siyosatlari

UAIDlar endi ikkinchi identifikatsiya qatlami uchun langar hisoblanadi:

- Global `IdentifierPolicyId` (`<kind>#<business_rule>`) quyidagilarni belgilaydi
  nom maydoni, umumiy majburiyat metamaʼlumotlari, hal qiluvchi tasdiqlash kaliti va
  kanonik kirishni normallashtirish rejimi (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress` yoki `AccountNumber`).
- Da'vo bitta olingan `opaque:` identifikatorini aynan bitta UAID va bittaga bog'laydi
  ushbu siyosat bo'yicha kanonik `AccountId`, lekin zanjir faqat qabul qiladi
  u imzolangan `IdentifierResolutionReceipt` bilan birga bo'lganda da'vo.
- Ruxsat `resolve -> transfer` oqimi bo'lib qoladi. Torii noaniqlikni hal qiladi
  kanonik `AccountId` ni qayta ishlaydi va qaytaradi; transferlar hali ham maqsadli
  kanonik hisob, `uaid:` yoki `opaque:` to'g'ridan-to'g'ri emas.
- Siyosatlar endi BFV kirish-shifrlash parametrlarini nashr etishi mumkin
  `PolicyCommitment.public_parameters`. Agar mavjud bo'lsa, Torii ularni reklama qiladi
  `GET /v1/identifier-policies` va mijozlar BFV bilan o'ralgan ma'lumotlarni yuborishlari mumkin
  ochiq matn o'rniga. Dasturlashtirilgan siyosatlar BFV parametrlarini a ichiga oladi
  kanonik `BfvProgrammedPublicParameters` to'plami ham nashr etadi
  ommaviy `ram_fhe_profile`; Eski xom BFV foydali yuklari bunga yangilanadi
  majburiyat qayta tiklanganda kanonik to'plam.
- Identifikator marshrutlari bir xil Torii kirish tokeni va tarif limiti orqali o'tadi
  ilovalarga qaragan boshqa so'nggi nuqtalar kabi tekshiradi. Ular odatdagidan chetlab o'tadigan yo'l emas
  API siyosati.

## 1.2 Terminologiya

Nom ajratish qasddan:

- `ram_lfe` tashqi yashirin funksiya abstraktsiyasidir. U siyosatni qamrab oladi
  ro'yxatga olish, majburiyatlar, ommaviy metama'lumotlar, ijro kvitantsiyalari va
  tekshirish rejimi.
- `BFV` - Brakerski/Fan-Vercauteren gomomorf shifrlash sxemasi
  shifrlangan kiritishni baholash uchun ba'zi `ram_lfe` backends.
- `ram_fhe_profile` - bu BFV-ga xos metama'lumotlar, umuman ikkinchi nom emas
  xususiyat. Bu hamyonlar va dasturlashtirilgan BFV ijro mashinasini ta'riflaydi
  Siyosat dasturlashtirilgan backenddan foydalanganda tekshiruvchilar maqsadli boʻlishi kerak.

Aniq ma'noda:

- `RamLfeProgramPolicy` va `RamLfeExecutionReceipt` LFE-qatlam turlari.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters` va
  `BfvRamProgramProfile` - FHE-qatlam turlari.
- `HiddenRamFheProgram` va `HiddenRamFheInstruction` ichki nomlardir.
  dasturlashtirilgan backend tomonidan bajariladigan yashirin BFV dasturi. Ular ustida qoladilar
  FHE tomoni, chunki ular o'rniga shifrlangan ijro mexanizmini tasvirlaydi
  tashqi siyosat yoki kvitansiya abstraksiyasi.

## 1.3 Hisob identifikatori taxalluslarga nisbatan

Universal hisobni ishga tushirish kanonik hisob identifikatsiya modelini o'zgartirmaydi:

- `AccountId` kanonik, domensiz hisob mavzusi bo'lib qoladi.
- `ScopedAccountId { account, domain }` ko'rishlar uchun aniq domen konteksti yoki
  domen havolasini amalga oshiradigan ro'yxatga olishlar. Bu ikkinchi kanonik emas
  shaxs.
- SNS/hisob qaydnomasi taxalluslari bu mavzu ustidagi alohida bog'lanishlardir. A
  `merchant@hbl.sbp` kabi domenga tegishli taxallus va maʼlumotlar maydoni ildizi taxalluslari
  `merchant@sbp` kabi ikkalasi ham bir xil kanonik `AccountId` ni hal qilishi mumkin.
- Saqlangan hisob qaydnomalarida `linked_domains` holati dan olingan
  hisob-domen indekslari. Buning uchun hozirda amalga oshirilgan havolalarni tavsiflaydi
  mavzu; u kanonik identifikatorning bir qismi emas.

Operatorlar, SDKlar va testlar uchun amal qilish qoidasi: kanonikdan boshlang
`AccountId`, keyin taxallus ijarasi, maʼlumotlar maydoni/domen ruxsatlari va aniq qoʻshing
domen havolalari alohida. Soxta domen miqyosidagi kanonikni sintez qilmang
taxallus yoki marshrut domen segmentini olib yurganligi sababli hisob qaydnomasi.

Joriy Torii marshrutlari:

| Marshrut | Maqsad |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Faol va nofaol RAM-LFE dastur siyosatlari hamda ularning ochiq ijro metamaʼlumotlari, jumladan, ixtiyoriy BFV `input_encryption` parametrlari va dasturlashtirilgan orqa qism `ram_fhe_profile` roʻyxati. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | `{ input_hex }` yoki `{ encrypted_input }` dan aynan birini qabul qiladi va tanlangan dastur uchun fuqaroligi bo'lmagan `RamLfeExecutionReceipt` va `{ output_hex, output_hash, receipt_hash }`ni qaytaradi. Joriy Torii ish vaqti dasturlashtirilgan BFV backend uchun kvitansiyalarni chiqaradi. |
| `POST /v1/ram-lfe/receipts/verify` | `RamLfeExecutionReceipt` ni eʼlon qilingan zanjirli dastur siyosatiga nisbatan fuqaroligisiz tasdiqlaydi va ixtiyoriy ravishda qoʻngʻiroq qiluvchi tomonidan taqdim etilgan `output_hex` `output_hash` kvitansiyasiga mos kelishini tekshiradi. |
| `GET /v1/identifier-policies` | Faol va nofaol yashirin funksiya siyosati nom maydonlarini hamda ularning umumiy metamaʼlumotlarini, jumladan, ixtiyoriy BFV `input_encryption` parametrlarini, mijoz tomonidan shifrlangan kiritish uchun zarur `normalization` rejimini va dasturlashtirilgan BFV siyosatlari uchun `ram_fhe_profile` roʻyxati. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | `{ input }` yoki `{ encrypted_input }` dan birini qabul qiladi. Oddiy matn `input` server tomonida normallashtirilgan; BFV `encrypted_input` allaqachon nashr etilgan siyosat rejimiga muvofiq normallashtirilgan bo'lishi kerak. Keyin oxirgi nuqta `opaque:` dastagini oladi va `ClaimIdentifier` zanjirda yuborishi mumkin bo'lgan imzolangan chekni qaytaradi, shu jumladan xom `signature_payload_hex` va tahlil qilingan `signature_payload`. || `POST /v1/identifiers/resolve` | `{ input }` yoki `{ encrypted_input }` dan aynan birini qabul qiladi. Oddiy matn `input` server tomonida normallashtirilgan; BFV `encrypted_input` allaqachon nashr etilgan siyosat rejimiga muvofiq normallashtirilgan bo'lishi kerak. Faol da'vo mavjud bo'lganda oxirgi nuqta identifikatorni `{ opaque_id, receipt_hash, uaid, account_id, signature }` ga o'chiradi va kanonik imzolangan foydali yukni `{ signature_payload_hex, signature_payload }` sifatida qaytaradi. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Deterministik kvitansiya xeshiga bog‘langan doimiy `IdentifierClaimRecord` ni qidiradi, shuning uchun operatorlar va SDKlar egalik huquqini tekshirishlari yoki to‘liq identifikator indeksini skanerlashsiz takrorlash/mos kelmaslik xatoliklarini tashxislashlari mumkin. |

Torii ning protsessdagi bajarish vaqti quyidagi ostida sozlangan.
`torii.ram_lfe.programs[*]`, kaliti `program_id`. Identifikator hozir yo'nalish
alohida `identifier_resolver` o'rniga o'sha RAM-LFE ish vaqtini qayta ishlating
konfiguratsiya yuzasi.

Joriy SDK qo'llab-quvvatlash:

- `normalizeIdentifierInput(value, normalization)` Rustga mos keladi
  `exact`, `lowercase_trimmed`, `phone_e164` uchun kanonikalizatorlar,
  `email_address` va `account_number`.
- `ToriiClient.listIdentifierPolicies()` siyosat metamaʼlumotlarini, jumladan, BFV roʻyxatini koʻrsatadi
  Siyosat uni nashr qilganda kiritish-shifrlash metamaʼlumotlari, shuningdek, dekodlangan
  BFV parametr ob'ekti `input_encryption_public_parameters_decoded` orqali.
  Dasturlashtirilgan siyosatlar dekodlangan `ram_fhe_profile` ni ham ochib beradi. Bu maydon
  ataylab BFV-ko'lamli: bu hamyonlarga kutilgan registrni tekshirish imkonini beradi
  soni, qatorlar soni, kanoniklashtirish rejimi va minimal shifrlangan matn moduli
  mijoz tomonidan kirishni shifrlashdan oldin dasturlashtirilgan FHE backend.
- `getIdentifierBfvPublicParameters(policy)` va
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` yordam
  JS qo'ng'iroq qiluvchilar chop etilgan BFV metama'lumotlarini iste'mol qiladi va siyosatdan xabardor so'rovni yaratadi
  siyosat-id va normalizatsiya qoidalarini qayta amalga oshirmasdan organlar.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` va
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` endi ruxsat bering
  JS hamyonlari to'liq BFV Norito shifrlangan matn konvertini mahalliy sifatida quradi.
  oldindan tuzilgan shifrlangan hex o‘rniga chop etilgan siyosat parametrlari.
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  yashirin identifikatorni hal qiladi va imzolangan kvitansiya yukini qaytaradi,
  shu jumladan `receipt_hash`, `signature_payload_hex` va
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId, kiritish |
  encryptedInput })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` qaytarilganni tasdiqlaydi
  mijoz tomonidagi siyosatni hal qiluvchi kalitga qarshi kvitansiya va`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` ni oladi
  keyingi audit/disklarni tuzatish oqimlari uchun doimiy da'vo rekordi.
- `IrohaSwift.ToriiClient` endi `listIdentifierPolicies()` ni ochib beradi,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  va `getIdentifierClaimByReceiptHash(_)`, ortiqcha
  Xuddi shu telefon/elektron pochta/hisob raqami uchun `ToriiIdentifierNormalization`
  kanoniklashtirish usullari.
- `ToriiIdentifierLookupRequest` va
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` yordamchilari terilgan Swift so'rov yuzasini taqdim etadi
  qo'ng'iroqlarni hal qilish va da'vo qilish va Swift siyosatlari endi BFVni olishi mumkin
  `encryptInput(...)` / `encryptedRequest(input:...)` orqali mahalliy shifrlangan matn.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` buni tasdiqlaydi
  yuqori darajadagi kvitansiya maydonlari imzolangan foydali yukga mos keladi va buni tasdiqlaydi
  yuborishdan oldin hal qiluvchi imzo mijoz tomoni.
- Android SDK-dagi `HttpClientTransport` endi ochiladi
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, kiritish,
  encryptedInputHex)`, `issueIdentifierClaimReceipt(hisob identifikatori, siyosat identifikatori,
  kirish, shifrlanganInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  plyus bir xil kanoniklashtirish qoidalari uchun `IdentifierNormalization`.
- `IdentifierResolveRequest` va
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` yordamchilari terilgan Android so'rov yuzasini ta'minlaydi,
  esa `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` BFV shifrlangan matn konvertini oladi
  nashr etilgan siyosat parametrlaridan mahalliy.
  `IdentifierResolutionReceipt.verifySignature(policy)` qaytarilganni tasdiqlaydi
  hal qiluvchi imzo mijoz tomoni.

Joriy ko'rsatmalar to'plami:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (kvitansiya bilan bog'langan; xom `opaque_id` da'volari rad etilgan)
- `RevokeIdentifier`

Endi `iroha_crypto::ram_lfe` da uchta backend mavjud:

- tarixiy majburiyat bilan bog'liq `HKDF-SHA3-512` PRF, va
- BFV bilan shifrlangan identifikatorni iste'mol qiladigan BFV tomonidan qo'llab-quvvatlanadigan maxfiy affin baholovchi
  to'g'ridan-to'g'ri uyalar. `iroha_crypto` sukut bo'yicha qurilganda
  `bfv-accel` xususiyati, BFV halqasini ko'paytirish aniq deterministikdan foydalanadi
  Ichki CRT-NTT backend; bu xususiyatni o'chirib qo'yish ga qaytadi
  bir xil natijalarga ega skalyar maktab kitobi yo'li va
- BFV tomonidan qo'llab-quvvatlanadigan maxfiy dasturlashtirilgan baholovchi, ko'rsatmalarga asoslangan
  Shifrlangan registrlar va shifrlangan matn xotirasi orqali operativ xotira uslubidagi bajarilish izi
  noaniq identifikator va kvitansiya xeshini olishdan oldin chiziqlar. Dasturlashtirilgan
  backend endi affin yo'liga qaraganda kuchliroq BFV modulli qavatni talab qiladi va
  uning umumiy parametrlari o'z ichiga olgan kanonik to'plamda nashr etiladi
  Hamyonlar va tekshiruvchilar tomonidan iste'mol qilinadigan RAM-FHE ijro profili.

Bu erda BFV joriy etilgan Brakerski/Fan-Vercauteren FHE sxemasini bildiradi
`crates/iroha_crypto/src/fhe_bfv.rs`. Bu shifrlangan ijro mexanizmi
tashqi yashirin nomi emas, balki affin va dasturlashtirilgan backendlar tomonidan ishlatiladi
funksiya abstraktsiyasi.Torii siyosat majburiyati tomonidan chop etilgan backenddan foydalanadi. BFV backend qachon
faol bo'lsa, ochiq matn so'rovlari normallashtiriladi, keyin server tomoni shifrlanadi
baholash. Affin backend uchun BFV `encrypted_input` so'rovlari baholanadi
to'g'ridan-to'g'ri va allaqachon mijoz tomonidan normallashtirilgan bo'lishi kerak; dasturlashtirilgan backend
shifrlangan kiritishni qayta rezolyutsiyaning deterministik BFV ga kanoniklashtiradi
maxfiy RAM dasturini amalga oshirishdan oldin konvertni oling, shuning uchun kvitansiya xeshlari qoladi
semantik ekvivalent shifrlangan matnlarda barqaror.

## 2. UAIDlarni olish va tekshirish

UAID olishning uchta qo'llab-quvvatlanadigan usuli mavjud:

1. **Uni jahon davlati yoki SDK modellaridan oʻqing.** Har qanday `Account`/`AccountDetails`
   Torii orqali so'ralgan foydali yuk endi `uaid` maydoniga ega
   ishtirokchi universal hisoblarni tanladi.
2. **UAID registrlarini so'rang.** Torii fosh qiladi
   `GET /v1/space-directory/uaids/{uaid}`, bu ma'lumotlar maydoni ulanishlarini qaytaradi
   va Space Directory xostidagi manifest metama'lumotlari saqlanib qoladi (qarang
   Foydali yuk namunalari uchun `docs/space-directory.md` §3).
3. **Uni aniq belgilang.** Yangi UAID-larni oflayn yuklashda xesh
   kanonik ishtirokchi urug'ini Blake2b-256 bilan belgilang va natijani prefiks bilan belgilang
   `uaid:`. Quyidagi parcha hujjatlashtirilgan yordamchini aks ettiradi
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```Har doim harfni kichik harflarda saqlang va xeshlashdan oldin bo'sh joyni normalizatsiya qiling.
`iroha app space-directory manifest scaffold` va Android kabi CLI yordamchilari
`UaidLiteral` tahlilchisi boshqaruvni ko'rib chiqish uchun bir xil kesish qoidalarini qo'llaydi.
maxsus skriptlarsiz qiymatlarni o'zaro tekshirish.

## 3. UAID xoldinglari va manifestlarini tekshirish

`iroha_core::nexus::portfolio` da deterministik portfel agregatori
UAID ga havola qiluvchi har bir aktiv/maʼlumotlar maydoni juftligini koʻrsatadi. Operatorlar va SDKlar
ma'lumotlarni quyidagi sirtlar orqali iste'mol qilishi mumkin:

| Yuzaki | Foydalanish |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Maʼlumotlar maydoni → aktiv → balans xulosalarini qaytaradi; `docs/source/torii/portfolio_api.md` da tasvirlangan. |
| `GET /v1/space-directory/uaids/{uaid}` | UAID bilan bog'langan ma'lumotlar maydoni identifikatorlari + hisob harflari ro'yxati. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Auditlar uchun to'liq `AssetPermissionManifest` tarixini taqdim etadi. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Bog'lanishning so'nggi nuqtasini o'rab oladigan va ixtiyoriy ravishda JSONni diskka yozadigan CLI yorlig'i (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Dalillar to'plami uchun manifest JSON to'plamini oladi. |

Misol CLI sessiyasi (Torii URL `iroha.json` da `torii_api_url` orqali sozlangan):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

JSON snapshotlarini ko'rib chiqishda foydalanilgan manifest xesh bilan birga saqlang; the
Space Directory kuzatuvchisi har doim paydo bo'lganda `uaid_dataspaces` xaritasini tiklaydi.
faollashtirish, muddatini tugatish yoki bekor qilish, shuning uchun bu suratlar isbotlashning eng tezkor usulidir.
ma'lum bir davrda qanday bog'lanishlar faol bo'lgan.

## 4. Nashr qilish qobiliyati dalillar bilan namoyon bo'ladi

Har safar yangi nafaqa chiqarilganda quyidagi CLI oqimidan foydalaning. Har bir qadam kerak
boshqaruvni imzolash uchun qayd etilgan dalillar to'plamidagi yer.

1. **Manifest JSON** kodini kiriting, shunda ko‘rib chiquvchilar deterministik xeshni avval ko‘radi
   topshirish:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Norito foydali yukidan (`--manifest`) yoki
   JSON tavsifi (`--manifest-json`). Torii/CLI kvitansiyasini plus yozib oling
   `PublishSpaceDirectoryManifest` ko'rsatma xeshi:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent dalillarini oling.** Obuna bo'ling
   `SpaceDirectoryEvent::ManifestActivated` va voqea yukini o'z ichiga oladi
   to'plam, shunda auditorlar o'zgarish qachon kelganini tasdiqlashlari mumkin.

4. **Audit toʻplamini yarating** manifestni uning maʼlumotlar maydoni profiliga bogʻlash va
   telemetriya kancalari:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` va `manifests fetch`) orqali ulanishlarni tekshiring va
   ushbu JSON fayllarini yuqoridagi xesh + to'plami bilan arxivlang.

Dalillarni tekshirish ro'yxati:

- [ ] Manifest xeshi (`*.manifest.hash`) oʻzgarishlarni tasdiqlovchi tomonidan imzolangan.
- [ ] Chop etish chaqiruvi uchun CLI/Torii kvitansiyasi (stdout yoki `--json-out` artefakt).
- [ ] `SpaceDirectoryEvent` foydali yukni faollashtirishni tasdiqlovchi.
- [ ] Maʼlumotlar maydoni profili, ilgaklar va manifest nusxasi bilan audit toʻplami katalogini.
- [ ] Torii faollashtirilgandan keyin bog'lanishlar + manifest snapshotlari.Bu SDK berishda `docs/space-directory.md` §3.2 talablarini aks ettiradi
relizlar ko'rib chiqish paytida ishora qilish uchun bitta sahifa egalari.

## 5. Regulyator/mintaqaviy manifest shablonlari

Ishlash qobiliyati namoyon bo'lganda boshlang'ich nuqta sifatida in-repo moslamalaridan foydalaning
regulyatorlar yoki mintaqaviy nazoratchilar uchun. Ular qanday qilib ruxsat berish/rad etishni ko'rsatishadi
qoidalar va sharhlovchilar kutayotgan siyosat eslatmalarini tushuntiring.

| Armatura | Maqsad | Eng muhim voqealar |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB audit tasmasi. | `compliance.audit::{stream_reports, request_snapshot}` uchun faqat oʻqish uchun ruxsatnomalar, regulyator UAIDlarni passiv saqlash uchun chakana pul oʻtkazmalarida yutuqni rad etish. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA nazorat chizig'i. | Ikkilamchi boshqaruvni amalga oshirish uchun cheklangan `cbdc.supervision.issue_stop_order` ruxsati (Kunlik oyna + `max_amount`) va `force_liquidation` da aniq rad etishni qo'shadi. |

Ushbu armaturalarni klonlashda yangilang:

1. `uaid` va `dataspace` identifikatorlari siz yoqayotgan ishtirokchi va qatorga mos keladi.
2. Boshqaruv jadvali asosida `activation_epoch`/`expiry_epoch` oynalari.
3. Regulyator siyosati havolalari bilan `notes` maydonlari (MiCA maqolasi, JFSA
   dumaloq va boshqalar).
4. Imtiyozli oynalar (`PerSlot`, `PerMinute`, `PerDay`) va ixtiyoriy
   `max_amount` cheklaydi, shuning uchun SDKlar xost bilan bir xil cheklovlarni amalga oshiradi.

## 6. SDK iste'molchilari uchun migratsiya qaydlariHar bir domen hisobi identifikatoriga havola qilingan mavjud SDK integratsiyalari oʻtishi kerak
yuqorida tavsiflangan UAID-markazli sirtlar. Yangilash vaqtida ushbu nazorat roʻyxatidan foydalaning:

  hisob identifikatorlari. Rust/JS/Swift/Android uchun bu oxirgi versiyaga yangilashni anglatadi
  ish maydoni qutilari yoki Norito ulanishlarini qayta tiklash.
- **API qo‘ng‘iroqlari:** Domenga tegishli portfel so‘rovlarini quyidagi bilan almashtiring
  `GET /v1/accounts/{uaid}/portfolio` va manifest/bog'lash so'nggi nuqtalari.
  `GET /v1/accounts/{uaid}/portfolio` ixtiyoriy `asset_id` so'rovini qabul qiladi
  hamyonga faqat bitta aktiv nusxasi kerak bo'lganda parametr. Bunday mijozlar yordamchilari
  `ToriiClient.getUaidPortfolio` (JS) va Android kabi
  `SpaceDirectoryClient` allaqachon ushbu marshrutlarni o'rab oladi; ularni buyurtma qilishdan afzal ko'ring
  HTTP kodi.
- **Keshlash va telemetriya:** Kesh yozuvlari xomashyo o‘rniga UAID + ma’lumotlar maydoni orqali
  hisob identifikatorlari va UAID literalini ko'rsatadigan telemetriyani chiqaradi, shuning uchun operatsiyalar mumkin
  jurnallarni Space Directory dalillari bilan to'plang.
- **Xatolarni qayta ishlash:** Yangi so'nggi nuqtalar qattiq UAID tahlil xatolarini qaytaradi
  `docs/source/torii/portfolio_api.md` da hujjatlashtirilgan; bu kodlarni ko'rsating
  so'zma-so'z, shuning uchun qo'llab-quvvatlash guruhlari muammolarni takroriy qadamlarsiz hal qilishlari mumkin.
- **Sinov:** Yuqorida aytib o'tilgan moslamalarni ulang (shuningdek, shaxsiy UAID manifestlari)
  Norito bo'ylab sayohatlar va manifest baholarini isbotlash uchun SDK test to'plamlariga
  xostni amalga oshirishga mos keladi.

## 7. Adabiyotlar- `docs/space-directory.md` - hayot tsikli tafsilotlari bilan operator o'yin kitobi.
- `docs/source/torii/portfolio_api.md` - UAID portfeli uchun REST sxemasi va
  aniq yakuniy nuqtalar.
- `crates/iroha_cli/src/space_directory.rs` - CLI ilovasiga havola qilingan
  ushbu qo'llanma.
- `fixtures/space_directory/capability/*.manifest.json` — regulyator, chakana savdo va
  CBDC manifest shablonlari klonlash uchun tayyor.