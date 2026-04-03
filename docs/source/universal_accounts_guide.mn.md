<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Бүх нийтийн дансны гарын авлага

Энэхүү гарын авлага нь UAID (Universal Account ID)-ийн танилцуулгад тавигдах шаардлагыг
Nexus замын зураглалыг гаргаж, тэдгээрийг оператор + SDK-д чиглэсэн заавар болгон багцлана.
Энэ нь UAID гаралт, багц/манифест шалгалт, зохицуулагчийн загварууд,
мөн `iroha програмын сансрын лавлах манифест бүрийг дагалдах ёстой нотлох баримтууд
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. UAID хурдан лавлагаа- UAID нь `uaid:<hex>` литерал бөгөөд `<hex>` нь Blake2b-256 дижест бөгөөд
  LSB-г `1` гэж тохируулсан. Каноник төрөл нь амьдардаг
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- Дансны бүртгэлд (`Account` ба `AccountDetails`) одоо нэмэлт `uaid` бичигдсэн байна.
  талбарт оруулснаар программууд тусгай хэшгүйгээр танигчийг сурах боломжтой.
- Далд функц тодорхойлогч бодлого нь дурын хэвийн оролтыг холбож болно
  (утасны дугаар, имэйл, дансны дугаар, түншийн тэмдэгт) `opaque:` ID-ууд
  UAID нэрийн орон зайн дор. Гинжний хэсгүүд нь `IdentifierPolicy`,
  `IdentifierClaimRecord`, `opaque_id -> uaid` индекс.
- Сансрын лавлах нь UAID бүрийг холбосон `World::uaid_dataspaces` газрын зургийг хадгалдаг.
  идэвхтэй манифестуудаар иш татсан өгөгдлийн орон зайн данс руу. Torii үүнийг дахин ашигладаг
  `/portfolio` болон `/uaids/*` API-д зориулсан газрын зураг.
- `POST /v1/accounts/onboard` нь өгөгдмөл сансрын лавлах манифестийг нийтэлдэг.
  дэлхийн өгөгдлийн орон зай байхгүй үед UAID шууд холбогддог.
  Онгоцны эрх баригчид `CanPublishSpaceDirectoryManifest{dataspace=0}` байх ёстой.
- Бүх SDK-ууд UAID литералуудыг каноник болгоход туслах хэрэгслүүдийг гаргадаг (жишээ нь,
  Android SDK дээрх `UaidLiteral`). Туслах ажилтнууд 64-гекс түүхий эдийг хүлээн авдаг
  (LSB=1) эсвэл `uaid:<hex>` литерал ба ижил Norito кодлогчийг дахин ашигла.
  digest нь хэлээр дамжиж чадахгүй.

## 1.1 Нуугдсан танигч бодлого

UAID нь одоо хоёр дахь таних давхаргын зангуу болж байна:- Глобал `IdentifierPolicyId` (`<kind>#<business_rule>`) нь дараахь зүйлийг тодорхойлдог.
  нэрийн орон зай, нийтийн амлалтын мета өгөгдөл, шийдвэрлэгчийн баталгаажуулах түлхүүр болон
  каноник оролтыг хэвийн болгох горим (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress`, эсвэл `AccountNumber`).
- Нэхэмжлэл нь нэг үүсмэл `opaque:` танигчийг яг нэг UAID болон нэгтэй холбодог.
  Энэ бодлогын дагуу `AccountId` каноник боловч гинж нь зөвхөн
  гарын үсэг зурсан `IdentifierResolutionReceipt` бичгийг хавсаргасан тохиолдолд нэхэмжлэх.
- Нарийвчлал нь `resolve -> transfer` урсгалтай хэвээр байна. Torii тунгалаг бус байдлыг шийддэг
  каноник `AccountId`-ийг зохицуулж, буцаана; шилжүүлэг зорилтот хэвээр байна
  `uaid:` эсвэл `opaque:` шууд утгаар биш, каноник данс.
- Бодлого нь одоо BFV оролт-шифрлэлтийн параметрүүдийг дамжуулан нийтлэх боломжтой
  `PolicyCommitment.public_parameters`. Байгаа үед Torii тэднийг сурталчилдаг
  `GET /v1/identifier-policies` ба үйлчлүүлэгчид BFV ороосон оролтыг оруулж болно
  энгийн текстийн оронд. Програмчлагдсан бодлого нь BFV параметрүүдийг a
  каноник `BfvProgrammedPublicParameters` багц нь мөн хэвлэгддэг
  нийтийн `ram_fhe_profile`; Хуучин түүхий BFV ачааллыг үүн дээр шинэчилсэн
  амлалт дахин бий болсон үед каноник багц.
- Тодорхойлогч маршрутууд нь ижил Torii хандалтын токен болон хурдны хязгаараар дамждаг.
  бусад апп-д тулгарч буй төгсгөлийн цэгүүд шиг шалгадаг. Тэд ердийнхөөс тойрч гарах зам биш юм
  API бодлого.

## 1.2 Нэр томьёо

Нэрний хуваагдал нь зориудаар:- `ram_lfe` нь далд функцийн гаднах хийсвэрлэл юм. Энэ нь бодлогыг хамардаг
  бүртгэл, амлалт, нийтийн мета өгөгдөл, гүйцэтгэлийн баримт, болон
  баталгаажуулах горим.
- `BFV` нь Brakerski/Fan-Vercauteren гомоморф шифрлэлтийн схем юм.
  шифрлэгдсэн оролтыг үнэлэх зарим `ram_lfe` backends.
- `ram_fhe_profile` нь BFV-д хамаарах мета өгөгдөл бөгөөд бүхэлд нь хоёр дахь нэр биш
  онцлог. Энэ нь түрийвч болон программчлагдсан BFV гүйцэтгэх машин тайлбарлах
  Бодлого нь програмчлагдсан арын хэсгийг ашиглах үед баталгаажуулагч зорилтот байх ёстой.

Тодорхой хэллэгээр:

- `RamLfeProgramPolicy` ба `RamLfeExecutionReceipt` нь LFE давхаргын төрөл юм.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`, болон
  `BfvRamProgramProfile` нь FHE давхаргын төрлүүд юм.
- `HiddenRamFheProgram` ба `HiddenRamFheInstruction` нь дотоод нэр юм.
  програмчлагдсан backend-ээр гүйцэтгэгдсэн далд BFV программ. Тэд дээр үлддэг
  FHE тал, учир нь тэд илүү шифрлэгдсэн гүйцэтгэх механизмыг тайлбарлах
  гадаад бодлого эсвэл баримтын хийсвэрлэл.

## 1.3 Бүртгэлийн таниулбар болон бусад нэр

Бүх нийтийн дансны танилцуулга нь каноник дансны таних загварыг өөрчлөхгүй:- `AccountId` нь каноник, домэйнгүй дансны сэдэв хэвээр байна.
- `AccountAlias` утгууд нь тухайн сэдвийн дээд талд байгаа тусдаа SNS холболтууд юм. А
  `merchant@banka.sbp` болон өгөгдлийн орон зайн эх нэр гэх мэт домэйны шаардлага хангасан нэр
  `merchant@sbp` зэрэг нь хоёулаа ижил каноник `AccountId`-г шийдэж чадна.
- Каноник дансны бүртгэл үргэлж `Account::new(AccountId)` байна /
  `NewAccount::new(AccountId)`; домайны шаардлага хангасан эсвэл домэйн материалжуулсан зүйл байхгүй
  бүртгэлийн зам.
- Домэйн эзэмшил, бусад нэрийн зөвшөөрөл болон бусад домэйны хамрах хүрээний зан үйлүүд амьдардаг
  Бүртгэлийн таниулбараас илүүтэйгээр өөрсдийн төлөв болон API-д.
- Нийтийн акаунтын хайлтыг дараах байдлаар хуваана: нэрийн асуулга нь нийтэд үлддэг
  Каноник дансны таних тэмдэг нь цэвэр `AccountId` хэвээр байна.

Оператор, SDK болон тестийг хэрэгжүүлэх дүрэм: каноникаас эхэлнэ
`AccountId`, дараа нь нэрийн түрээс, дата зай/домэйн зөвшөөрөл болон дурын зүйлийг нэмнэ үү.
тус тусад нь домэйн эзэмшдэг муж. Хуурамч нэрээр үүсгэгдсэн бүртгэлийг нэгтгэж болохгүй
эсвэл дансны бүртгэл дээрх ямар нэгэн холбоотой домэйн талбарыг зөвхөн өөр нэр эсвэл
маршрут нь домэйн сегментийг дамжуулдаг.

Одоогийн Torii маршрутууд:| Маршрут | Зорилго |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Идэвхтэй болон идэвхгүй RAM-LFE програмын бодлогууд болон тэдгээрийн нийтийн гүйцэтгэлийн мета өгөгдлүүд, үүнд нэмэлт BFV `input_encryption` параметрүүд болон программчлагдсан `ram_fhe_profile` багтана. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | `{ input_hex }` эсвэл `{ encrypted_input }`-ийн яг аль нэгийг нь хүлээн авч, сонгогдсон програмын хувьд харьяалалгүй `RamLfeExecutionReceipt` дээр нэмэх нь `{ output_hex, output_hash, receipt_hash }`-г буцаана. Одоогийн Torii ажиллах хугацаа нь программчлагдсан BFV backend-ийн төлбөрийн баримтыг гаргадаг. |
| `POST /v1/ram-lfe/receipts/verify` | Иргэншилгүй `RamLfeExecutionReceipt`-г нийтэлсэн гинжин хэлхээний програмын бодлогын эсрэг баталгаажуулж, дуудлага хийгчийн нийлүүлсэн `output_hex` нь `output_hash` баримттай таарч байгаа эсэхийг шалгадаг. |
| `GET /v1/identifier-policies` | Идэвхтэй болон идэвхгүй далд функцийн бодлогын нэрсийн орон зай, тэдгээрийн олон нийтийн мета өгөгдлүүд, үүнд нэмэлт BFV `input_encryption` параметрүүд, шифрлэгдсэн үйлчлүүлэгчийн оролтод шаардлагатай `normalization` горим, програмчлагдсан BFV бодлогод зориулсан `ram_fhe_profile` зэрэг багтана. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | `{ input }` эсвэл `{ encrypted_input }`-ийн яг аль нэгийг нь хүлээн авна. Энгийн текст `input` сервер талдаа хэвийн; BFV `encrypted_input` нь хэвлэгдсэн бодлогын горимын дагуу аль хэдийн хэвийн болсон байх ёстой. Дараа нь төгсгөлийн цэг нь `opaque:` бариулыг гаргаж, `ClaimIdentifier` нь гинжин хэлхээнд илгээх боломжтой гарын үсэгтэй баримтыг буцаана, үүнд түүхий `signature_payload_hex` болон задлан шинжлэгдсэн `signature_payload` орно. || `POST /v1/identifiers/resolve` | `{ input }` эсвэл `{ encrypted_input }`-ийн яг аль нэгийг нь хүлээн авна. Энгийн текст `input` сервер талын хэвийн; BFV `encrypted_input` нь хэвлэгдсэн бодлогын горимын дагуу аль хэдийн хэвийн болсон байх ёстой. Төгсгөлийн цэг нь идэвхтэй нэхэмжлэл байгаа үед танигчийг `{ opaque_id, receipt_hash, uaid, account_id, signature }` болгон шийдэж, мөн `{ signature_payload_hex, signature_payload }` гэж каноник гарын үсэгтэй ачааллыг буцаана. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Тодорхойлолттой төлбөрийн хэштэй холбогдсон `IdentifierClaimRecord`-ийг хайж олох бөгөөд ингэснээр операторууд болон SDK-ууд бүрэн танигчийн индексийг скан хийлгүйгээр эзэмшигчийн эрхийг шалгах эсвэл дахин тоглуулах / таарахгүй алдааг оношлох боломжтой. |

Torii-ийн процессын гүйцэтгэлийн ажиллах хугацааг доор тохируулсан.
`torii.ram_lfe.programs[*]`, `program_id` түлхүүртэй. Тодорхойлогч одоо чиглүүлж байна
Тусдаа `identifier_resolver`-ийн оронд ижил RAM-LFE ажиллах цагийг дахин ашиглах
тохиргооны гадаргуу.

Одоогийн SDK дэмжлэг:- `normalizeIdentifierInput(value, normalization)` Rust-тэй таарч байна
  `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, `account_number`.
- `ToriiClient.listIdentifierPolicies()` бодлогын мета өгөгдлийг, түүний дотор BFV-г жагсаасан
  Бодлого нийтлэх үед оролтын шифрлэлтийн мета өгөгдөл, дээр нь тайлагдсан
  `input_encryption_public_parameters_decoded`-ээр дамжуулан BFV параметрийн объект.
  Програмчлагдсан бодлого нь мөн код тайлагдсан `ram_fhe_profile`-г илчилдэг. Тэр талбар
  зориудаар BFV-ийн хамрах хүрээг хамарсан: энэ нь хэтэвчүүдэд хүлээгдэж буй бүртгэлийг шалгах боломжийг олгодог
  тоо, эгнээний тоо, канончлолын горим, шифр текстийн хамгийн бага модуль
  клиент талын оролтыг шифрлэхээс өмнө програмчлагдсан FHE backend.
- `getIdentifierBfvPublicParameters(policy)` ба
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` тусламж
  JS дуудагсад нийтлэгдсэн BFV мета өгөгдлийг ашиглаж, бодлоготой холбоотой хүсэлтийг бий болгодог
  бодлого-id болон хэвийн болгох дүрмийг дахин хэрэгжүүлэхгүйгээр байгууллагууд.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` ба
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` одоо зөвшөөрнө үү
  JS түрийвч нь бүрэн BFV Norito шифр текстийн дугтуйг дараахаас бүтээдэг.
  Урьдчилан бүтээгдсэн зургаан текстийг илгээхийн оронд бодлогын параметрүүдийг нийтэлсэн.
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  далд танигчийг шийдэж, гарын үсэг зурсан баримтын ачааллыг буцаана,
  үүнд `receipt_hash`, `signature_payload_hex`, болон
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId, оролт |
  encryptedInput })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` буцаасаныг баталгаажуулна
  Үйлчлүүлэгч тал дахь бодлого шийдвэрлэгч түлхүүрийн эсрэг баримт, болон`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` нь татаж авдаг
  хожим аудит/дибаг хийх урсгалд зориулсан байнгын нэхэмжлэлийн бүртгэл.
- `IrohaSwift.ToriiClient` одоо `listIdentifierPolicies()`-г ил гаргаж байна,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  болон `getIdentifierClaimByReceiptHash(_)`, нэмэх
  `ToriiIdentifierNormalization` ижил утас/и-мэйл/дансны дугаар
  канончлолын горимууд.
- `ToriiIdentifierLookupRequest` болон
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` туслахууд нь шивсэн Swift хүсэлтийн гадаргууг өгдөг
  дуудлагыг шийдвэрлэх, нэхэмжлэх-хүлээн авах, мөн Swift бодлого нь одоо BFV-г гаргаж авах боломжтой
  `encryptInput(...)` / `encryptedRequest(input:...)`-ээр дамжуулан орон нутгийн шифр текст.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` үүнийг баталж байна
  дээд түвшний төлбөрийн талбарууд нь гарын үсэг зурсан ачаатай таарч, баталгаажуулдаг
  илгээхээс өмнө шийдвэрлэх гарын үсэг үйлчлүүлэгч тал.
- Android SDK дахь `HttpClientTransport` одоо ил гарч байна
  `listIdentifierPolicies()`, `resolveIdentifier(бодлогын ID, оролт,
  encryptedInputHex)`, `issueIdentifierClaimReceipt(дансны дугаар, бодлогын дугаар,
  оролт, шифрлэгдсэнInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  дээр нь ижил канончлолын дүрмийн хувьд `IdentifierNormalization`.
- `IdentifierResolveRequest` болон
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` туслахууд нь бичсэн Android хүсэлтийн гадаргууг хангаж,
  байхад `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` нь BFV шифр текстийн дугтуйг гаргаж авдаг
  нийтлэгдсэн бодлогын параметрүүдээс орон нутагт .
  `IdentifierResolutionReceipt.verifySignature(policy)` буцаасаныг баталгаажуулна
  шийдвэрлэх гарын үсэг үйлчлүүлэгч тал.

Одоогийн зааврын багц:- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (баримт бичигдсэн; түүхий `opaque_id` нэхэмжлэлийг татгалзсан)
- `RevokeIdentifier`

Одоо `iroha_crypto::ram_lfe` дээр гурван арын хэсэг байна:

- түүхэн үүрэг хүлээсэн `HKDF-SHA3-512` PRF, мөн
- BFV-ээр шифрлэгдсэн танигчийг ашигладаг BFV-д суурилсан нууц аффин үнэлгээч
  шууд оролт. `iroha_crypto` анхдагчаар бүтээгдсэн үед
  `bfv-accel` онцлог, BFV цагираг үржүүлэх нь яг тодорхой детерминистикийг ашигладаг
  Дотооддоо CRT-NTT backend; Энэ функцийг идэвхгүй болгох нь буцаж ирдэг
  ижил үр дүн бүхий скаляр сургуулийн номын зам, ба
- Зааварт тулгуурласан BFV-д суурилсан нууц программчлагдсан үнэлгээч
  Шифрлэгдсэн регистрүүд болон шифрлэгдсэн текстийн санах ой дээр RAM хэлбэрийн гүйцэтгэлийн ул мөр
  тунгалаг бус танигч болон хүлээн авсан хэшийг гаргахын өмнөх эгнээ. Програмчлагдсан
  backend нь одоо аффин замаас илүү хүчтэй BFV модулийн шалыг шаарддаг ба
  түүний нийтийн параметрүүдийг багтаасан каноник багцад нийтлэв
  Түрийвч болон баталгаажуулагчийн хэрэглэдэг RAM-FHE гүйцэтгэлийн профайл.

Энд BFV нь хэрэгжсэн Brakerski/Fan-Vercauteren FHE схемийг хэлнэ
`crates/iroha_crypto/src/fhe_bfv.rs`. Энэ нь шифрлэгдсэн гүйцэтгэх механизм юм
affine болон программчлагдсан backends ашигладаг, гадна далд нэр биш
функцийн хийсвэрлэл.Torii нь бодлогын амлалтаар нийтлэгдсэн арын хэсгийг ашигладаг. BFV backend үед
идэвхтэй байгаа тул энгийн текстийн хүсэлтийг хэвийн болгож, өмнө нь сервер талдаа шифрлэнэ
үнэлгээ. Affin backend-д зориулсан BFV `encrypted_input` хүсэлтийг үнэлдэг
шууд бөгөөд аль хэдийн үйлчлүүлэгчийн талаас хэвийн байх ёстой; програмчлагдсан арын хэсэг
Шифрлэгдсэн оролтыг шийдвэрлэгчийн детерминист BFV руу буцааж каноникчилна
нууц RAM програмыг ажиллуулахын өмнө дугтуйг хийснээр хүлээн авсан хэшүүд хэвээр үлдэнэ
семантикийн хувьд ижил төстэй шифр текстүүдэд тогтвортой.

## 2. UAID-г гаргаж авах, баталгаажуулах

UAID авах гурван дэмжигдсэн арга байдаг:

1. **Үүнийг дэлхийн төлөв эсвэл SDK загвараас уншина уу.** Дурын `Account`/`AccountDetails`
   Torii-ээр асуусан ачааллыг одоо `uaid` талбарт бөглөсөн байна.
   Оролцогч бүх нийтийн дансыг сонгосон.
2. **UAID бүртгэлээс лавлана уу.** Torii илрүүлнэ
   `GET /v1/space-directory/uaids/{uaid}` нь өгөгдлийн орон зайн холболтыг буцаадаг
   болон Сансрын лавлах хостын манифест метадата (харна уу
   Ачааллын дээжийн хувьд `docs/space-directory.md` §3).
3. **Үүнийг тодорхой гаргаарай.** Шинэ UAID-г офлайнаар ачаалах үед хэш
   каноник оролцогчийн үрийг Blake2b-256-тай холбож, үр дүнгийн угтварыг бичнэ
   `uaid:`. Доорх хэсэг нь баримтжуулсан туслагчийг толилуулж байна
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```Хэшгэхийн өмнө үсгийг жижиг үсгээр бичиж, хоосон зайг хэвийн болго.
`iroha app space-directory manifest scaffold` болон Android зэрэг CLI туслахууд
`UaidLiteral` задлагч ижил шүргэх дүрмийг ашигладаг тул засаглалын тойм
тусгай скриптгүйгээр утгуудыг шалгах.

## 3. UAID-ийн эзэмшил болон манифестуудыг шалгах

`iroha_core::nexus::portfolio` дахь детерминист багцын агрегатор
UAID-д хамаарах хөрөнгө/өгөгдлийн орон зай бүрийг харуулдаг. Операторууд ба SDK
дараах гадаргуугаар дамжуулан өгөгдлийг ашиглаж болно:

| Гадаргуу | Хэрэглээ |
|---------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | Өгөгдлийн орон зай → хөрөнгө → үлдэгдлийн хураангуйг буцаана; `docs/source/torii/portfolio_api.md`-д тайлбарласан. |
| `GET /v1/space-directory/uaids/{uaid}` | UAID-тэй холбогдсон өгөгдлийн орон зайн ID + бүртгэлийн литералуудыг жагсаана. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | Аудитын бүрэн `AssetPermissionManifest` түүхийг өгдөг. |
| `iroha app space-directory bindings fetch --uaid <literal>` | Холболтын төгсгөлийн цэгийг ороож, JSON-г диск рүү (`--json-out`) бичдэг CLI товчлол. |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | Нотлох баримтын багцад зориулсан манифест JSON багцыг татаж авдаг. |

Жишээ CLI сесс (`iroha.json` дээр `torii_api_url`-ээр тохируулсан Torii URL):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

JSON агшин агшингуудыг шалгах явцад ашигласан манифест хэшийн хажууд хадгалах; нь
Сансрын лавлах ажиглагч нь илрэх бүрт `uaid_dataspaces` газрын зургийг дахин бүтээдэг.
идэвхжүүлэх, хүчингүй болгох, хүчингүй болгох, тиймээс эдгээр хормын хувилбарууд нь нотлох хамгийн хурдан арга юм.
Тухайн эрин үед ямар холболтууд идэвхтэй байсан.## 4. Нийтлэх чадвар нь нотлох баримтаар илэрдэг

Шинэ тэтгэмж гарах бүрт доорх CLI урсгалыг ашиглаарай. Алхам бүр заавал байх ёстой
засаглалын гарын үсэг зурахаар бүртгэгдсэн нотлох баримтын багцад газар.

1. **Манифест JSON-г кодчилно уу**, ингэснээр хянагчид өмнө нь тодорхойлогч хэшийг харна
   илгээх:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **Тэтгэмжийг нийтлэх** эсвэл Norito (`--manifest`) эсвэл
   JSON тайлбар (`--manifest-json`). Torii/CLI баримтыг нэмж бичнэ үү
   `PublishSpaceDirectoryManifest` зааврын хэш:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent-ийн нотлох баримтыг аваарай.** Бүртгүүлэх
   `SpaceDirectoryEvent::ManifestActivated` ба үйл явдлын ачааллыг оруулах
   аудиторууд өөрчлөлт хэзээ орж ирснийг батлах боломжтой багц.

4. **Аудитын багц үүсгэх** манифестыг өгөгдлийн орон зайн профайл болон
   телеметрийн дэгээ:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` болон `manifests fetch`)-ээр дамжуулан холболтыг баталгаажуулна уу.
   эдгээр JSON файлуудыг дээрх хэш + багцаар архивлана.

Нотлох баримт шалгах хуудас:

- [ ] Өөрчлөлтийг баталгаажуулагч гарын үсэг зурсан манифест хэш (`*.manifest.hash`).
- [ ] Нийтлэх дуудлагын CLI/Torii баримт (stdout эсвэл `--json-out` олдвор).
- [ ] `SpaceDirectoryEvent` ачааллыг нотлох идэвхжүүлэлт.
- [ ] Өгөгдлийн орон зайн профайл, дэгээ, манифест хуулбар бүхий багцын лавлахад аудит хийх.
- [ ] Идэвхжүүлсний дараах Torii-аас авчирсан холболтууд + манифест агшин зуурын зургууд.Энэ нь SDK өгөх үед `docs/space-directory.md` §3.2-ын шаардлагыг тусгадаг.
Эзэмшигчид нь хувилбарын үнэлгээний үеэр зааж өгөх нэг хуудас.

## 5. Зохицуулагч/бүс нутгийн манифест загварууд

Урлах чадвар илрэх үед репо дахь бэхэлгээг эхлэх цэг болгон ашиглаарай
зохицуулагчид эсвэл бүс нутгийн хянагчдад зориулсан. Тэд зөвшөөрөх/татгалзах хамрах хүрээг хэрхэн харуулахыг харуулдаг
журмууд болон тоймчдын хүлээж буй бодлогын тэмдэглэлүүдийг тайлбарла.

| Бэхэлгээ | Зорилго | Онцлох үйл явдал |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB аудитын хангамж. | `compliance.audit::{stream_reports, request_snapshot}`-д зориулсан зөвхөн унших боломжтой, зохицуулагчийн UAID-г идэвхгүй байлгахын тулд жижиглэнгийн шилжүүлгийг үгүйсгэх боломжтой. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA хяналтын эгнээ. | Давхар хяналтыг хэрэгжүүлэхийн тулд `cbdc.supervision.issue_stop_order` хязгаарлагдмал тэтгэмж (Өдөр тутмын цонх + `max_amount`) болон `force_liquidation` дээр тодорхой үгүйсгэлийг нэмдэг. |

Эдгээр бэхэлгээг клон хийхдээ дараахыг шинэчилнэ үү:

1. `uaid` болон `dataspace` id-ууд нь таны идэвхжүүлж буй оролцогч болон эгнээнд таарна.
2. Засаглалын хуваарь дээр үндэслэн `activation_epoch`/`expiry_epoch` цонх.
3. Зохицуулагчийн бодлогын лавлагаа бүхий `notes` талбарууд (MiCA нийтлэл, JFSA)
   дугуй гэх мэт).
4. Тэтгэмжийн цонх (`PerSlot`, `PerMinute`, `PerDay`) болон нэмэлт
   `max_amount` хязгаартай тул SDK нь хосттой ижил хязгаарлалтыг мөрддөг.

## 6. SDK хэрэглэгчдэд зориулсан шилжүүлгийн тэмдэглэлДомэйн дансны ID-д хандсан одоо байгаа SDK интеграцууд руу шилжих ёстой
дээр дурдсан UAID төвтэй гадаргуу. Шинэчлэлтийн үед энэ хяналтын хуудсыг ашиглана уу:

  дансны дугаар. Rust/JS/Swift/Android-ын хувьд энэ нь хамгийн сүүлийн үеийн хувилбар руу шинэчлэх гэсэн үг юм
  ажлын талбайн хайрцаг эсвэл Norito холболтыг сэргээж байна.
- **API дуудлагууд:** Домэйн хамрах хүрээний багцын асуулгыг дараахаар солино
  `GET /v1/accounts/{uaid}/portfolio` ба манифест/холбох төгсгөлийн цэгүүд.
  `GET /v1/accounts/{uaid}/portfolio` нь нэмэлт `asset_id` хүсэлтийг хүлээн авдаг.
  түрийвчэнд зөвхөн нэг хөрөнгийн жишээ хэрэгтэй үед параметр. Үйлчлүүлэгчийн туслахууд ийм
  `ToriiClient.getUaidPortfolio` (JS) болон Android
  `SpaceDirectoryClient` аль хэдийн эдгээр маршрутуудыг боож; захиалгаар хийхээс илүүд үздэг
  HTTP код.
- **Кэш ба телеметр:** Түүхий биш харин UAID + өгөгдлийн орон зайгаар кэш оруулах
  дансны ids, мөн UAID утгыг харуулсан телеметрийг ялгаруулж, үйлдлүүдийг хийх боломжтой
  Сансрын лавлах нотлох баримт бүхий бүртгэлүүдийг жагсаах.
- **Алдаа боловсруулах:** Шинэ төгсгөлийн цэгүүд нь хатуу UAID задлан шинжлэх алдааг буцаана
  `docs/source/torii/portfolio_api.md`-д баримтжуулсан; эдгээр кодуудыг гарга
  Тиймээс тусламжийн багууд дахин давтах алхамгүйгээр асуудлыг шийдвэрлэх боломжтой.
- **Туршилт:** Дээр дурдсан хэрэгслүүдийг холбоно уу (өөрийн UAID манифест)
  Norito хоёр талын аялал болон манифест үнэлгээг батлахын тулд SDK тестийн багц руу оруулна
  хостын хэрэгжилттэй таарч байна.

## 7. Ашигласан материал- `docs/space-directory.md` — амьдралын мөчлөгийн дэлгэрэнгүй мэдээлэл бүхий операторын тоглоомын ном.
- `docs/source/torii/portfolio_api.md` — UAID багцын REST схем болон
  илэрхий төгсгөлийн цэгүүд.
- `crates/iroha_cli/src/space_directory.rs` — CLI хэрэгжилтийг иш татсан
  энэ гарын авлага.
- `fixtures/space_directory/capability/*.manifest.json` — зохицуулагч, жижиглэн худалдаа, болон
  CBDC манифест загварууд клончлоход бэлэн байна.