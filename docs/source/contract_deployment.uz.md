---
lang: uz
direction: ltr
source: docs/source/contract_deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0f2b1d7d027d715eac5a3ca8be29dea8f0e76013e948947a4de66108ac561f34
source_last_modified: "2026-01-22T14:58:53.689594+00:00"
translation_last_reviewed: 2026-02-07
title: Contract Deployment (.to) — API & Workflow
translator: machine-google-reviewed
---

Holati: Torii, CLI va asosiy kirish testlari (2025-yil noyabr) tomonidan amalga oshirilgan va bajarilgan.

## Umumiy ko'rinish

- Kompilyatsiya qilingan IVM bayt kodini (`.to`) Torii ga yuborish yoki chiqarish orqali joylashtiring.
  `RegisterSmartContractCode`/`RegisterSmartContractBytes` ko'rsatmalari
  bevosita.
- Tugunlar `code_hash` va kanonik ABI xeshini lokal ravishda qayta hisoblaydi; mos kelmasligi
  deterministik ravishda rad etish.
- Saqlangan artefaktlar `contract_manifests` zanjiri ostida yashaydi va
  `contract_code` registrlari. Faqat mos yozuvlar xeshlarini ko'rsatadi va kichik bo'lib qoladi;
  kod baytlari `code_hash` tomonidan kalitlanadi.
- Himoyalangan nom maydonlari a dan oldin qabul qilingan boshqaruv taklifini talab qilishi mumkin
  joylashtirishga ruxsat beriladi. Qabul qilish yo'li taklifning foydali yukini ko'radi va
  qachon `(namespace, contract_id, code_hash, abi_hash)` tengligini ta'minlaydi
  nom maydoni himoyalangan.

## Saqlangan artefaktlar va saqlash

- `RegisterSmartContractCode` ma'lum bir manifestni qo'shadi/ustiga yozadi
  `code_hash`. Xuddi shu xesh allaqachon mavjud bo'lsa, u yangi bilan almashtiriladi
  namoyon.
- `RegisterSmartContractBytes` kompilyatsiya qilingan dasturni ostida saqlaydi
  `contract_code[code_hash]`. Agar xesh uchun baytlar allaqachon mavjud bo'lsa, ular mos kelishi kerak
  aniq; turli baytlar invariant buzilishni keltirib chiqaradi.
- Kod hajmi `max_contract_code_bytes` maxsus parametri bilan chegaralangan
  (standart 16 MiB). Oldin `SetParameter(Custom)` tranzaksiyasi bilan uni bekor qiling
  kattaroq artefaktlarni ro'yxatga olish.
- Saqlash cheklanmagan: manifestlar va kodlar aniq bo'lmaguncha mavjud bo'lib qoladi
  kelajakdagi boshqaruv ish jarayonida olib tashlanadi. TTL yoki avtomatik GC mavjud emas.

## Qabul qilish quvuri

- Validator IVM sarlavhasini tahlil qiladi, `version_major == 1`ni qo'llaydi va tekshiradi
  `abi_version == 1`. Noma'lum versiyalar darhol rad etadi; ish vaqti yo'q
  almashtirish.
- `code_hash` uchun manifest allaqachon mavjud bo'lsa, tekshirish
  saqlangan `code_hash`/`abi_hash` taqdim etilganlardan hisoblangan qiymatlarga teng
  dastur. Mos kelmaslik `Manifest{Code,Abi}HashMismatch` xatolarini keltirib chiqaradi.
- Himoyalangan nom maydonlariga qaratilgan tranzaktsiyalar metadata kalitlarini o'z ichiga olishi kerak
  `gov_namespace` va `gov_contract_id`. Qabul qilish yo'li ularni taqqoslaydi
  qabul qilingan `DeployContract` takliflariga qarshi; mos keladigan taklif mavjud bo'lmasa
  tranzaksiya `NotPermitted` bilan rad etilgan.

## Torii oxirgi nuqtalari (`app_api` xususiyati)- `POST /v1/contracts/deploy`
  - So'rovning asosiy qismi: `DeployContractDto` (maydon tafsilotlari uchun `docs/source/torii_contracts_api.md` ga qarang).
  - Torii base64 foydali yukini dekodlaydi, ikkala xeshni hisoblaydi, manifest tuzadi,
    va `RegisterSmartContractCode` plus taqdim etadi
    nomidan imzolangan bitimda `RegisterSmartContractBytes`
    qo'ng'iroq qiluvchi.
  - Javob: `{ ok, code_hash_hex, abi_hash_hex }`.
  - Xatolar: noto'g'ri base64, qo'llab-quvvatlanmaydigan ABI versiyasi, ruxsat yo'q
    (`CanRegisterSmartContractCode`), o'lcham chegarasi oshib ketdi, boshqaruv chegarasi.
- `POST /v1/contracts/code`
  - `RegisterContractCodeDto` (vakolat, shaxsiy kalit, manifest) qabul qiladi va faqat taqdim etadi
    `RegisterSmartContractCode`. Manifestlar alohida sahnalashtirilganda foydalaning
    bayt-kod.
- `POST /v1/contracts/instance`
  - `DeployAndActivateInstanceDto` (vakolat, shaxsiy kalit, nom maydoni/contract_id, `code_b64`, ixtiyoriy manifest bekor qilish) ni qabul qiladi va tarqatadi + atomik faollashtiradi.
- `POST /v1/contracts/instance/activate`
  - `ActivateInstanceDto` (vakolat, shaxsiy kalit, nomlar maydoni, contract_id, `code_hash`) qabul qiladi va faqat faollashtirish yo'riqnomasini taqdim etadi.
- `GET /v1/contracts/code/{code_hash}`
  - `{ manifest: { code_hash, abi_hash } }` ni qaytaradi.
    Qo'shimcha manifest maydonlari ichkarida saqlanadi, lekin bu erda a uchun kiritilmagan
    barqaror API.
- `GET /v1/contracts/code-bytes/{code_hash}`
  - `{ code_b64 }` ni baza64 sifatida kodlangan saqlangan `.to` tasviri bilan qaytaradi.

Shartnomaning barcha muddati yakuniy nuqtalari orqali sozlangan maxsus joylashtirish cheklovchisi mavjud
`torii.deploy_rate_per_origin_per_sec` (sekundiga tokenlar) va
`torii.deploy_burst_per_origin` (burst tokens). Birlamchi parametrlar portlash bilan 4 req/s
`X-API-Token`, masofaviy IP yoki oxirgi nuqta maslahatidan olingan har bir token/kalit uchun 8.
Ishonchli operatorlar uchun cheklovchini o'chirish uchun istalgan maydonni `null` ga o'rnating. Qachon
cheklovchi yong'inlar, Torii oshiradi
`torii_contract_throttled_total{endpoint="code|deploy|instance|activate"}` telemetriya hisoblagichi va
HTTP 429 ni qaytaradi; har qanday ishlov beruvchi xatosi o'sishi
Ogohlantirish uchun `torii_contract_errors_total{endpoint=…}`.

## Boshqaruv integratsiyasi va himoyalangan nomlar maydoni- `gov_protected_namespaces` (JSON nomlar maydoni massivi) maxsus parametrini o'rnating
  strings) kirish eshigini yoqish uchun. Torii ostida yordamchilarni ochib beradi
  `/v1/gov/protected-namespaces` va CLI ularni aks ettiradi
  `iroha_cli app gov protected set` / `iroha_cli app gov protected get`.
- `ProposeDeployContract` (yoki Torii) bilan yaratilgan takliflar
  `/v1/gov/proposals/deploy-contract` oxirgi nuqtasi) qo'lga olish
  `(namespace, contract_id, code_hash, abi_hash, abi_version)`.
- Referendum o'tgandan so'ng, `EnactReferendum` taklif kuchga kirdi va
  qabul qilish mos metadata va kodni o'z ichiga olgan joylashtirishlarni qabul qiladi.
- Tranzaksiyalar `gov_namespace=a namespace` va metama'lumotlar juftligini o'z ichiga olishi kerak
  `gov_contract_id=an identifier` (va `contract_namespace` / o'rnatilishi kerak)
  Qo'ng'iroq vaqtini ulash uchun `contract_id`). CLI yordamchilari ularni to'ldiradi
  avtomatik ravishda siz `--namespace`/`--contract-id` o'tganingizda.
- Himoyalangan nom maydonlari yoqilganda, navbatga kirish urinishlarini rad etadi
  mavjud `contract_id` ni boshqa nom maydoniga qayta bog'lash; qabul qilinganidan foydalaning
  taklif qiling yoki boshqa joyga joylashtirishdan oldin oldingi majburiylikni bekor qiling.
- Agar chiziqli manifest bittadan yuqori tasdiqlovchi kvorumni belgilasa, kiriting
  `gov_manifest_approvers` (validator hisobi identifikatorlarining JSON massivi), shuning uchun navbatni hisoblash mumkin
  tranzaktsiya organi bilan birga qo'shimcha tasdiqlar. Lanes ham rad etadi
  manifestda mavjud bo'lmagan nom bo'shliqlariga havola qiluvchi metama'lumotlar
  `protected_namespaces` to'plami.

## CLI yordamchilari

- `iroha_cli app contracts deploy --authority <id> --private-key <hex> --code-file <path>`
  Torii joylashtirish so'rovini yuboradi (heshlarni tezda hisoblash).
- `iroha_cli app contracts deploy-activate --authority <id> --private-key <hex> --namespace <ns> --contract-id <id> --code-file <path>`
  manifestni tuzadi (berilgan kalit bilan imzolanadi), baytlarni + manifestni qayd qiladi,
  va bitta tranzaksiyada `(namespace, contract_id)` ulanishini faollashtiradi. Foydalanish
  Hisoblangan xeshlarni va ko'rsatmalar sonini chop etish uchun `--dry-run`
  yuborish va imzolangan JSON manifestini saqlash uchun `--manifest-out`.
- `iroha_cli app contracts manifest build --code-file <path> [--sign-with <hex>]` hisoblaydi
  `code_hash`/`abi_hash` kompilyatsiya qilingan `.to` uchun va ixtiyoriy ravishda manifestni imzolaydi,
  JSON chop etish yoki `--out` ga yozish.
- `iroha_cli app contracts simulate --authority <id> --private-key <hex> --code-file <path> --gas-limit <u64>`
  oflayn VM passni ishga tushiradi va ABI/xesh metamaʼlumotlari hamda navbatdagi ISIlar haqida hisobot beradi
  (hisoblar va ko'rsatmalar identifikatorlari) tarmoqqa tegmasdan. Biriktiring
  `--namespace/--contract-id` qo'ng'iroq vaqti metama'lumotlarini aks ettirish uchun.
- `iroha_cli app contracts manifest get --code-hash <hex>` manifestni Torii orqali oladi
  va ixtiyoriy ravishda uni diskka yozadi.
- `iroha_cli app contracts code get --code-hash <hex> --out <path>` yuklamalar
  saqlangan `.to` tasviri.
- `iroha_cli app contracts instances --namespace <ns> [--table]` ro'yxatlari faollashtirilgan
  shartnoma misollari (manifest + metama'lumotlarga asoslangan).
- Boshqaruv yordamchilari (`iroha_cli app gov deploy propose`, `iroha_cli app gov enact`,
  `iroha_cli app gov protected set/get`) himoyalangan nomlar maydoni ish oqimini tartibga soladi va
  JSON artefaktlarini tekshirish uchun ochish.

## Sinov va qamrov

- `crates/iroha_core/tests/contract_code_bytes.rs` qopqoq kodi ostida birlik sinovlari
  saqlash, idempotency va o'lcham qopqog'i.
- `crates/iroha_core/tests/gov_enact_deploy.rs` orqali manifest kiritishni tasdiqlaydi
  qabul qilish va `crates/iroha_core/tests/gov_protected_gate.rs` mashqlari
  himoyalangan nomlar maydonini oxirigacha qabul qilish.
- Torii marshrutlari so'rov/javob birligi testlarini o'z ichiga oladi va CLI buyruqlari mavjud
  JSON bo'ylab sayohatlari barqaror bo'lishini ta'minlaydigan integratsiya testlari.

Batafsil referendum yuklamalari uchun `docs/source/governance_api.md` ga qarang va
saylov byulletenlarining ish jarayonlari.