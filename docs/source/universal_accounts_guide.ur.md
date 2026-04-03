<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
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

# یونیورسل اکاؤنٹ گائیڈ

یہ گائیڈ UAID (یونیورسل اکاؤنٹ ID) سے رول آؤٹ کی ضروریات کو دور کرتا ہے۔
Nexus روڈ میپ اور انہیں آپریٹر + SDK فوکسڈ واک تھرو میں پیک کرتا ہے۔
اس میں UAID اخذ، پورٹ فولیو/مینی فیسٹ معائنہ، ریگولیٹر ٹیمپلیٹس،
اور ثبوت جو ہر `iroha ایپ اسپیس ڈائرکٹری مینی فیسٹ کے ساتھ ہونا چاہیے۔
publish` run (roadmap reference: `roadmap.md:2209`)۔

## 1. UAID فوری حوالہ- UAIDs `uaid:<hex>` لٹریلز ہیں جہاں `<hex>` ایک Blake2b-256 ڈائجسٹ ہے جس کا
  LSB `1` پر سیٹ ہے۔ کینونیکل قسم میں رہتا ہے۔
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`۔
- اکاؤنٹ کے ریکارڈ (`Account` اور `AccountDetails`) اب اختیاری `uaid` رکھتے ہیں
  فیلڈ تاکہ ایپلی کیشنز بغیر ہیشنگ کے شناخت کنندہ کو سیکھ سکیں۔
- پوشیدہ فنکشن شناخت کنندہ پالیسیاں صوابدیدی نارملائزڈ ان پٹس کو پابند کرسکتی ہیں۔
  (فون نمبرز، ای میلز، اکاؤنٹ نمبرز، پارٹنر سٹرنگز) سے `opaque:` IDs
  UAID نام کی جگہ کے تحت۔ آن چین کے ٹکڑے `IdentifierPolicy` ہیں،
  `IdentifierClaimRecord`، اور `opaque_id -> uaid` انڈیکس۔
- اسپیس ڈائرکٹری ایک `World::uaid_dataspaces` نقشہ برقرار رکھتی ہے جو ہر UAID کو جوڑتی ہے۔
  فعال مینی فیسٹس کے ذریعے حوالہ کردہ ڈیٹا اسپیس اکاؤنٹس میں۔ Torii اسے دوبارہ استعمال کرتا ہے۔
  `/portfolio` اور `/uaids/*` APIs کے لیے نقشہ۔
- `POST /v1/accounts/onboard` اس کے لیے پہلے سے طے شدہ اسپیس ڈائرکٹری مینی فیسٹ شائع کرتا ہے
  عالمی ڈیٹا اسپیس جب کوئی موجود نہ ہو، تو UAID فوری طور پر پابند ہو جاتا ہے۔
  آن بورڈنگ حکام کو `CanPublishSpaceDirectoryManifest{dataspace=0}` رکھنا ضروری ہے۔
- تمام SDKs UAID لٹریلز (جیسے،
  Android SDK میں `UaidLiteral`)۔ مددگار خام 64-ہیکس ہضم قبول کرتے ہیں۔
  (LSB=1) یا `uaid:<hex>` لٹریلز اور وہی Norito کوڈیکس دوبارہ استعمال کریں تاکہ
  ڈائجسٹ زبانوں میں نہیں بڑھ سکتا۔

## 1.1 پوشیدہ شناخت کنندہ پالیسیاں

UAIDs اب دوسری شناختی پرت کے لیے اینکر ہیں:- ایک عالمی `IdentifierPolicyId` (`<kind>#<business_rule>`) کی وضاحت کرتا ہے
  نام کی جگہ، عوامی عزم کا میٹا ڈیٹا، حل کنندہ کی تصدیق کی کلید، اور
  کیننیکل ان پٹ نارملائزیشن موڈ (`Exact`, `LowercaseTrimmed`،
  `PhoneE164`، `EmailAddress`، یا `AccountNumber`)۔
- ایک دعوی ایک اخذ کردہ `opaque:` شناخت کنندہ کو بالکل ایک UAID اور ایک سے منسلک کرتا ہے۔
  اس پالیسی کے تحت کیننیکل `AccountId`، لیکن سلسلہ صرف قبول کرتا ہے
  دعوی کریں جب اس کے ساتھ دستخط شدہ `IdentifierResolutionReceipt` ہو۔
- ریزولوشن `resolve -> transfer` بہاؤ رہتا ہے۔ Torii مبہم کو حل کرتا ہے۔
  کینونیکل `AccountId` کو ہینڈل کرتا ہے اور واپس کرتا ہے۔ منتقلی اب بھی ہدف ہے
  کیننیکل اکاؤنٹ، براہ راست `uaid:` یا `opaque:` لٹریلز نہیں۔
- پالیسیاں اب BFV ان پٹ انکرپشن پیرامیٹرز کے ذریعے شائع کر سکتی ہیں۔
  `PolicyCommitment.public_parameters`۔ موجود ہونے پر، Torii ان کی تشہیر کرتا ہے۔
  `GET /v1/identifier-policies`، اور کلائنٹ BFV لپیٹے ہوئے ان پٹ جمع کر سکتے ہیں
  سادہ متن کے بجائے. پروگرام شدہ پالیسیاں BFV پیرامیٹرز کو a میں لپیٹتی ہیں۔
  canonical `BfvProgrammedPublicParameters` بنڈل جو بھی شائع کرتا ہے۔
  عوامی `ram_fhe_profile`؛ لیگیسی خام BFV پے لوڈز کو اس پر اپ گریڈ کیا گیا ہے۔
  کیننیکل بنڈل جب عزم کو دوبارہ بنایا جاتا ہے۔
- شناخت کنندہ کے راستے اسی Torii رسائی ٹوکن اور شرح کی حد سے گزرتے ہیں
  دوسرے ایپ کا سامنا کرنے والے اختتامی پوائنٹس کی طرح چیک کرتا ہے۔ وہ عام کے ارد گرد ایک بائی پاس نہیں ہیں
  API پالیسی۔

## 1.2 اصطلاحات

نام کی تقسیم جان بوجھ کر کی گئی ہے:- `ram_lfe` بیرونی پوشیدہ فنکشن خلاصہ ہے۔ یہ پالیسی کا احاطہ کرتا ہے۔
  رجسٹریشن، وعدے، عوامی میٹا ڈیٹا، عملدرآمد کی رسیدیں، اور
  تصدیقی موڈ
- `BFV` Brakerski/Fan-Vercauteren homomorphic encryption سکیم ہے
  خفیہ کردہ ان پٹ کا اندازہ کرنے کے لیے کچھ `ram_lfe` بیک اینڈ۔
- `ram_fhe_profile` BFV مخصوص میٹا ڈیٹا ہے، پورے کا دوسرا نام نہیں
  خصوصیت یہ پروگرام شدہ BFV عملدرآمد مشین کی وضاحت کرتا ہے جو بٹوے اور
  جب پالیسی پروگرام شدہ بیک اینڈ کا استعمال کرتی ہے تو تصدیق کنندگان کو ہدف بنانا چاہیے۔

ٹھوس الفاظ میں:

- `RamLfeProgramPolicy` اور `RamLfeExecutionReceipt` LFE-پرت کی اقسام ہیں۔
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`، اور
  `BfvRamProgramProfile` FHE-پرت کی اقسام ہیں۔
- `HiddenRamFheProgram` اور `HiddenRamFheInstruction` کے اندرونی نام ہیں
  پوشیدہ BFV پروگرام جو پروگرام شدہ بیک اینڈ کے ذریعہ انجام دیا گیا ہے۔ وہ اس پر رہتے ہیں۔
  ایف ایچ ای کی طرف کیونکہ وہ انکرپٹڈ ایگزیکیوشن میکانزم کی بجائے بیان کرتے ہیں۔
  بیرونی پالیسی یا رسید کا خلاصہ۔

## 1.3 اکاؤنٹ کی شناخت بمقابلہ عرفی نام

یونیورسل اکاؤنٹ رول آؤٹ کیننیکل اکاؤنٹ شناختی ماڈل کو تبدیل نہیں کرتا ہے:- `AccountId` کیننیکل، ڈومین لیس اکاؤنٹ کا موضوع ہے۔
- `AccountAlias` اقدار اس موضوع کے اوپر علیحدہ SNS پابندیاں ہیں۔ اے
  ڈومین کوالیفائیڈ عرف جیسے `merchant@banka.sbp` اور ڈیٹا اسپیس روٹ عرف
  جیسے `merchant@sbp` دونوں ایک ہی کیننیکل `AccountId` کو حل کر سکتے ہیں۔
- کیننیکل اکاؤنٹ کی رجسٹریشن ہمیشہ `Account::new(AccountId)` / ہوتی ہے۔
  `NewAccount::new(AccountId)`; کوئی ڈومین کے لیے اہل یا ڈومین کے لیے مواد نہیں ہے۔
  رجسٹریشن کا راستہ
- ڈومین کی ملکیت، عرفی اجازتیں، اور دیگر ڈومین کے دائرہ کار کے طرز عمل زندہ ہیں۔
  اکاؤنٹ کی شناخت کے بجائے اپنی ریاست اور APIs میں۔
- عوامی اکاؤنٹ کی تلاش اس تقسیم کی پیروی کرتی ہے: عرف کے سوالات عوامی رہتے ہیں، جبکہ
  کیننیکل اکاؤنٹ کی شناخت ایک خالص `AccountId` رہتی ہے۔

آپریٹرز، SDKs اور ٹیسٹوں کے لیے نفاذ کا اصول: کینونیکل سے شروع کریں۔
`AccountId`، پھر عرفی لیز، ڈیٹا اسپیس/ڈومین کی اجازتیں، اور کوئی بھی شامل کریں
الگ سے ڈومین کی ملکیت والی ریاست۔ جعلی عرف سے حاصل کردہ اکاؤنٹ کی ترکیب نہ کریں۔
یا اکاؤنٹ کے ریکارڈ پر کسی بھی منسلک ڈومین فیلڈ کی توقع کریں صرف اس وجہ سے کہ عرف یا
روٹ میں ایک ڈومین سیگمنٹ ہوتا ہے۔

موجودہ Torii راستے:| راستہ | مقصد |
|---------|---------|
| `GET /v1/ram-lfe/program-policies` | فعال اور غیر فعال RAM-LFE پروگرام کی پالیسیوں کے علاوہ ان کے پبلک ایگزیکیوشن میٹا ڈیٹا کی فہرست بناتا ہے، بشمول اختیاری BFV `input_encryption` پیرامیٹرز اور پروگرامڈ بیک اینڈ `ram_fhe_profile`۔ |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | `{ input_hex }` یا `{ encrypted_input }` میں سے بالکل ایک کو قبول کرتا ہے اور منتخب پروگرام کے لیے بے وطن `RamLfeExecutionReceipt` پلس `{ output_hex, output_hash, receipt_hash }` واپس کرتا ہے۔ موجودہ Torii رن ٹائم پروگرام شدہ BFV بیک اینڈ کے لیے رسیدیں جاری کرتا ہے۔ |
| `POST /v1/ram-lfe/receipts/verify` | شائع شدہ آن چین پروگرام پالیسی کے خلاف بے وطنی سے `RamLfeExecutionReceipt` کی توثیق کرتا ہے اور اختیاری طور پر چیک کرتا ہے کہ کالر کی طرف سے فراہم کردہ `output_hex` رسید `output_hash` سے میل کھاتا ہے۔ |
| `GET /v1/identifier-policies` | فعال اور غیر فعال پوشیدہ فنکشن پالیسی کے نام کی جگہوں کے علاوہ ان کے عوامی میٹا ڈیٹا کی فہرست بناتا ہے، بشمول اختیاری BFV `input_encryption` پیرامیٹرز، انکرپٹڈ کلائنٹ سائیڈ ان پٹ کے لیے مطلوبہ `normalization` موڈ، اور پروگرام شدہ BF پالیسیوں کے لیے `ram_fhe_profile`۔ |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | `{ input }` یا `{ encrypted_input }` میں سے بالکل ایک کو قبول کرتا ہے۔ سادہ متن `input` سرور سائیڈ کو عام کیا گیا ہے۔ BFV `encrypted_input` کو شائع شدہ پالیسی وضع کے مطابق پہلے سے ہی معمول پر لانا ضروری ہے۔ اختتامی نقطہ پھر `opaque:` ہینڈل حاصل کرتا ہے اور ایک دستخط شدہ رسید واپس کرتا ہے جسے `ClaimIdentifier` آن چین جمع کرا سکتا ہے، بشمول خام `signature_payload_hex` اور پارس شدہ `signature_payload`۔ || `POST /v1/identifiers/resolve` | `{ input }` یا `{ encrypted_input }` میں سے بالکل ایک کو قبول کرتا ہے۔ سادہ متن `input` سرور سائیڈ کو عام کیا گیا ہے۔ BFV `encrypted_input` کو شائع شدہ پالیسی وضع کے مطابق پہلے سے ہی معمول پر لانا ضروری ہے۔ اختتامی نقطہ شناخت کنندہ کو `{ opaque_id, receipt_hash, uaid, account_id, signature }` میں حل کرتا ہے جب ایک فعال دعوی موجود ہوتا ہے، اور `{ signature_payload_hex, signature_payload }` کے طور پر کیننیکل دستخط شدہ پے لوڈ بھی واپس کرتا ہے۔ |
| `GET /v1/identifiers/receipts/{receipt_hash}` | مستقل `IdentifierClaimRecord` کو تلاش کرتا ہے جو ایک تعییناتی رسید ہیش سے منسلک ہوتا ہے تاکہ آپریٹرز اور SDKs ملکیت کے دعوے کا آڈٹ کر سکیں یا مکمل شناخت کنندہ انڈیکس کو اسکین کیے بغیر دوبارہ پلے / مماثل ناکامیوں کی تشخیص کر سکیں۔ |

Torii کا ان پروسیس ایگزیکیوشن رن ٹائم کے تحت ترتیب دیا گیا ہے
`torii.ram_lfe.programs[*]`، جس کی کلید `program_id` ہے۔ اب شناخت کنندہ کے راستے
ایک الگ `identifier_resolver` کی بجائے اسی RAM-LFE رن ٹائم کو دوبارہ استعمال کریں
تشکیل کی سطح.

موجودہ SDK سپورٹ:- `normalizeIdentifierInput(value, normalization)` زنگ سے میل کھاتا ہے۔
  `exact`, `lowercase_trimmed`, `phone_e164` کے لیے کینونیکلائزرز،
  `email_address`، اور `account_number`۔
- `ToriiClient.listIdentifierPolicies()` پالیسی میٹا ڈیٹا کی فہرست دیتا ہے، بشمول BFV
  ان پٹ-انکرپشن میٹا ڈیٹا جب پالیسی اسے شائع کرتی ہے، نیز ڈی کوڈ شدہ
  BFV پیرامیٹر آبجیکٹ بذریعہ `input_encryption_public_parameters_decoded`۔
  پروگرام شدہ پالیسیاں ڈی کوڈ شدہ `ram_fhe_profile` کو بھی بے نقاب کرتی ہیں۔ وہ میدان ہے۔
  جان بوجھ کر BFV-scoped: یہ بٹوے کو متوقع رجسٹر کی تصدیق کرنے دیتا ہے۔
  شمار، لین کی گنتی، کینونیکلائزیشن موڈ، اور کم از کم سائفر ٹیکسٹ ماڈیولس کے لیے
  کلائنٹ سائڈ ان پٹ کو خفیہ کرنے سے پہلے پروگرام شدہ FHE بیک اینڈ۔
- `getIdentifierBfvPublicParameters(policy)` اور
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` مدد
  JS کالر شائع شدہ BFV میٹا ڈیٹا استعمال کرتے ہیں اور پالیسی سے آگاہی کی درخواست بناتے ہیں۔
  پالیسی آئی ڈی اور نارملائزیشن کے قواعد کو دوبارہ نافذ کیے بغیر باڈیز۔
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` اور
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` اب دو
  JS بٹوے مقامی طور پر مکمل BFV Norito سائفر ٹیکسٹ لفافہ بناتے ہیں
  پہلے سے تعمیر شدہ سائفر ٹیکسٹ ہیکس شپنگ کے بجائے پالیسی کے پیرامیٹرز شائع کیے۔
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  ایک پوشیدہ شناخت کنندہ کو حل کرتا ہے اور دستخط شدہ رسید پے لوڈ واپس کرتا ہے،
  بشمول `receipt_hash`، `signature_payload_hex`، اور
  `signature_payload`۔
- `ToriiClient.issueIdentifierClaimReceipt(accountId, {policyId, input |
  encryptedInput })` issues the signed receipt needed by `ClaimIdentifier`۔
- `verifyIdentifierResolutionReceipt(receipt, policy)` واپسی کی تصدیق کرتا ہے۔
  کلائنٹ کی طرف سے پالیسی ریزولور کلید کے خلاف رسید، اور`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` لاتا ہے۔
  بعد کے آڈٹ/ڈیبگ فلو کے لیے دعویٰ کا مستقل ریکارڈ۔
- `IrohaSwift.ToriiClient` اب `listIdentifierPolicies()` کو بے نقاب کرتا ہے،
  `resolveIdentifier(policyId:input:encryptedInputHex:)`،
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`،
  اور `getIdentifierClaimByReceiptHash(_)`، پلس
  اسی فون/ای میل/اکاؤنٹ نمبر کے لیے `ToriiIdentifierNormalization`
  کینونیکلائزیشن کے طریقوں
- `ToriiIdentifierLookupRequest` اور
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` مددگار ٹائپ شدہ سوئفٹ درخواست کی سطح فراہم کرتے ہیں۔
  کالز کو حل کریں اور دعویٰ وصول کریں، اور سوئفٹ پالیسیاں اب BFV حاصل کر سکتی ہیں۔
  سائفر ٹیکسٹ مقامی طور پر `encryptInput(...)` / `encryptedRequest(input:...)` کے ذریعے۔
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` اس کی توثیق کرتا ہے۔
  اعلی درجے کی رسید کے خانے دستخط شدہ پے لوڈ سے میل کھاتے ہیں اور تصدیق کرتے ہیں۔
  جمع کرنے سے پہلے حل کرنے والے کے دستخط کلائنٹ کی طرف۔
- Android SDK میں `HttpClientTransport` اب سامنے آ رہا ہے۔
  `listIdentifierPolicies()`, `ResolveIdentifier(policyId، ان پٹ،
  encryptedInputHex)`, `issueIdentifierClaimReceipt(اکاؤنٹ آئی ڈی، پالیسی آئی ڈی،
  input, encryptedInputHex)`, and `getIdentifierClaimByReceiptHash(...)`،
  plus `IdentifierNormalization` اسی کینونیکلائزیشن کے اصولوں کے لیے۔
- `IdentifierResolveRequest` اور
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` مددگار ٹائپ کردہ Android درخواست کی سطح فراہم کرتے ہیں،
  جبکہ `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` BFV سائفر ٹیکسٹ لفافہ اخذ کرتا ہے
  مقامی طور پر شائع شدہ پالیسی پیرامیٹرز سے۔
  `IdentifierResolutionReceipt.verifySignature(policy)` واپسی کی تصدیق کرتا ہے۔
  حل کنندہ دستخط کلائنٹ کی طرف.

موجودہ ہدایات سیٹ:- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (رسید کے پابند؛ خام `opaque_id` کے دعوے مسترد کر دیے گئے ہیں)
- `RevokeIdentifier`

`iroha_crypto::ram_lfe` میں اب تین بیک اینڈز موجود ہیں:

- تاریخی عزم کا پابند `HKDF-SHA3-512` PRF، اور
- ایک BFV کی حمایت یافتہ خفیہ affine evaluator جو BFV- انکرپٹڈ شناخت کنندہ استعمال کرتا ہے
  براہ راست سلاٹ. جب `iroha_crypto` ڈیفالٹ کے ساتھ بنایا جاتا ہے۔
  `bfv-accel` خصوصیت، BFV انگوٹی ضرب ایک قطعی تعییناتی استعمال کرتا ہے
  اندرونی طور پر CRT-NTT پسدید؛ اس خصوصیت کو غیر فعال کرنا واپس آتا ہے۔
  اسکیلر سکول بک پاتھ ایک جیسی آؤٹ پٹس کے ساتھ، اور
- ایک BFV کی حمایت یافتہ خفیہ پروگرام شدہ ایویلیویٹر جو کہ ایک ہدایات پر مبنی ہے۔
  انکرپٹڈ رجسٹرز اور سائفر ٹیکسٹ میموری پر RAM طرز پر عمل درآمد
  مبہم شناخت کنندہ اور رسید ہیش حاصل کرنے سے پہلے لین۔ پروگرام شدہ
  بیک اینڈ کو اب ایفائن پاتھ سے زیادہ مضبوط BFV ماڈیولس فلور کی ضرورت ہے۔
  اس کے عوامی پیرامیٹرز ایک کینونیکل بنڈل میں شائع کیے گئے ہیں جس میں شامل ہیں۔
  RAM-FHE ایگزیکیوشن پروفائل کو بٹوے اور تصدیق کنندگان کے ذریعے استعمال کیا جاتا ہے۔

یہاں BFV کا مطلب ہے Brakerski/Fan-Vercauteren FHE اسکیم جس میں لاگو کیا گیا ہے۔
`crates/iroha_crypto/src/fhe_bfv.rs`۔ یہ انکرپٹڈ ایگزیکیوشن میکانزم ہے۔
affine اور پروگرام شدہ بیک اینڈ کے ذریعے استعمال کیا جاتا ہے، نہ کہ بیرونی پوشیدہ کا نام
فنکشن خلاصہTorii پالیسی کمٹمنٹ کے ذریعہ شائع کردہ بیک اینڈ کا استعمال کرتا ہے۔ جب BFV پسدید
فعال ہے، سادہ متن کی درخواستوں کو معمول پر لایا جاتا ہے پھر اس سے پہلے سرور سائڈ کو انکرپٹ کیا جاتا ہے۔
تشخیص affine بیک اینڈ کے لیے BFV `encrypted_input` درخواستوں کا جائزہ لیا جاتا ہے
براہ راست اور پہلے سے ہی کلائنٹ سائیڈ کو معمول پر لانا ضروری ہے۔ پروگرام شدہ پسدید
انکرپٹڈ ان پٹ کو دوبارہ ریزولور کے ڈیٹرمنسٹک BFV پر canonicalize کرتا ہے۔
خفیہ RAM پروگرام کو انجام دینے سے پہلے لفافے کو استعمال کریں تاکہ رسید ہیش باقی رہیں
معنوی طور پر مساوی سائفر ٹیکسٹس میں مستحکم۔

## 2. UAIDs اخذ کرنا اور اس کی تصدیق کرنا

UAID حاصل کرنے کے تین معاون طریقے ہیں:

1. **اسے عالمی ریاست یا SDK ماڈلز سے پڑھیں۔** کوئی بھی `Account`/`AccountDetails`
   Torii کے توسط سے پوچھے گئے پے لوڈ میں اب `uaid` فیلڈ ہے جب
   شریک نے یونیورسل اکاؤنٹس کا انتخاب کیا۔
2. **UAID رجسٹریوں سے استفسار کریں۔** Torii بے نقاب
   `GET /v1/space-directory/uaids/{uaid}` جو ڈیٹا اسپیس بائنڈنگز کو لوٹاتا ہے۔
   اور مینی فیسٹ میٹا ڈیٹا اسپیس ڈائرکٹری کا میزبان برقرار رہتا ہے (دیکھیں۔
   پے لوڈ کے نمونوں کے لیے `docs/space-directory.md` §3)۔
3. **اسے قطعی طور پر اخذ کریں۔** نئے UAIDs کو آف لائن بوٹسٹریپ کرتے وقت، ہیش
   Blake2b-256 کے ساتھ کیننیکل شریک بیج اور اس کے ساتھ نتیجہ کا سابقہ لگائیں۔
   `uaid:`۔ ذیل کا ٹکڑا دستاویز میں مددگار کی عکاسی کرتا ہے۔
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
   ```لٹریل کو ہمیشہ لوئر کیس میں اسٹور کریں اور ہیش کرنے سے پہلے وائٹ اسپیس کو معمول پر رکھیں۔
CLI مددگار جیسے `iroha app space-directory manifest scaffold` اور Android
`UaidLiteral` پارسر تراشنے کے ایک ہی اصول کا اطلاق کرتا ہے تاکہ گورننس کے جائزے
ایڈہاک اسکرپٹ کے بغیر اقدار کو کراس چیک کریں۔

## 3. UAID ہولڈنگز اور مینی فیسٹس کا معائنہ کرنا

`iroha_core::nexus::portfolio` میں ڈیٹرمنسٹک پورٹ فولیو ایگریگیٹر
UAID کا حوالہ دینے والے ہر اثاثہ/ڈیٹا اسپیس جوڑے کو ظاہر کرتا ہے۔ آپریٹرز اور SDKs
درج ذیل سطحوں کے ذریعے ڈیٹا استعمال کر سکتے ہیں:

| سطح | استعمال |
|---------|---------|
| `GET /v1/accounts/{uaid}/portfolio` | ڈیٹا اسپیس → اثاثہ → بیلنس کے خلاصے لوٹاتا ہے۔ `docs/source/torii/portfolio_api.md` میں بیان کیا گیا ہے۔ |
| `GET /v1/space-directory/uaids/{uaid}` | UAID سے منسلک ڈیٹا اسپیس IDs + اکاؤنٹ لٹریلز کی فہرست۔ |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | آڈٹ کے لیے مکمل `AssetPermissionManifest` ہسٹری فراہم کرتا ہے۔ |
| `iroha app space-directory bindings fetch --uaid <literal>` | CLI شارٹ کٹ جو بائنڈنگ اینڈ پوائنٹ کو لپیٹتا ہے اور اختیاری طور پر JSON کو ڈسک (`--json-out`) پر لکھتا ہے۔ |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | ثبوت کے پیک کے لیے مینی فیسٹ JSON بنڈل لاتا ہے۔ |

مثال کے طور پر CLI سیشن (`iroha.json` میں `torii_api_url` کے ذریعے Torii URL ترتیب دیا گیا):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

JSON سنیپ شاٹس کو جائزوں کے دوران استعمال ہونے والے مینی فیسٹ ہیش کے ساتھ اسٹور کریں۔ دی
اسپیس ڈائرکٹری واچر جب بھی ظاہر ہوتا ہے `uaid_dataspaces` نقشہ دوبارہ بناتا ہے۔
چالو، میعاد ختم، یا منسوخ، لہذا یہ سنیپ شاٹس ثابت کرنے کا تیز ترین طریقہ ہیں۔
کسی مخصوص دور میں کون سی پابندیاں فعال تھیں۔## 4. اشاعت کی صلاحیت ثبوت کے ساتھ ظاہر ہوتی ہے۔

جب بھی نیا الاؤنس جاری کیا جائے تو نیچے دیے گئے CLI کا استعمال کریں۔ ہر قدم ضروری ہے۔
گورننس سائن آف کے لئے ریکارڈ شدہ ثبوت بنڈل میں زمین۔

1. **مینی فیسٹ JSON** کو انکوڈ کریں تاکہ جائزہ لینے والوں کو اس سے پہلے ڈیٹرمنسٹک ہیش نظر آئے
   جمع کرانے:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **الاؤنس شائع کریں** یا تو Norito پے لوڈ (`--manifest`) یا
   JSON تفصیل (`--manifest-json`)۔ Torii/CLI رسید پلس ریکارڈ کریں۔
   `PublishSpaceDirectoryManifest` انسٹرکشن ہیش:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture SpaceDirectoryEvent ثبوت۔** سبسکرائب کریں۔
   `SpaceDirectoryEvent::ManifestActivated` اور ایونٹ پے لوڈ کو شامل کریں۔
   بنڈل تاکہ آڈیٹرز تصدیق کر سکیں کہ تبدیلی کب آئی۔

4. مینی فیسٹ کو اس کے ڈیٹا اسپیس پروفائل کے ساتھ باندھتے ہوئے **ایک آڈٹ بنڈل تیار کریں اور
   ٹیلی میٹری ہکس:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` اور `manifests fetch`) کے ذریعے پابندیوں کی تصدیق کریں اور
   ان JSON فائلوں کو اوپر ہیش + بنڈل کے ساتھ آرکائیو کریں۔

ثبوت چیک لسٹ:

- [ ] مینی فیسٹ ہیش (`*.manifest.hash`) تبدیلی کے منظور کنندہ کے دستخط شدہ۔
- [ ] CLI/Torii اشاعت کال کی رسید (stdout یا `--json-out` آرٹ فیکٹ)۔
- [ ] `SpaceDirectoryEvent` پے لوڈ ایکٹیویشن ثابت کر رہا ہے۔
- ڈیٹا اسپیس پروفائل، ہکس، اور مینی فیسٹ کاپی کے ساتھ آڈٹ بنڈل ڈائرکٹری۔
- [ ] بائنڈنگز + مینی فیسٹ اسنیپ شاٹس جو Torii پوسٹ ایکٹیویشن سے حاصل کیے گئے ہیں۔یہ SDK دیتے وقت `docs/space-directory.md` §3.2 میں ضروریات کی عکاسی کرتا ہے۔
ریلیز کے جائزوں کے دوران اشارہ کرنے کے لیے ایک صفحے کے مالکان۔

## 5. ریگولیٹر/علاقائی مینی فیسٹ ٹیمپلیٹس

کرافٹنگ کی صلاحیت ظاہر ہونے پر ان ریپو فکسچر کو ابتدائی پوائنٹس کے طور پر استعمال کریں۔
ریگولیٹرز یا علاقائی نگرانوں کے لیے۔ وہ اسکوپ کی اجازت/انکار کا مظاہرہ کرتے ہیں۔
اصولوں اور پالیسی نوٹوں کی وضاحت کریں جو جائزہ لینے والوں کی توقع ہے۔

| فکسچر | مقصد | جھلکیاں |
|---------|---------|------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB آڈٹ فیڈ۔ | ریگولیٹر UAIDs کو غیر فعال رکھنے کے لیے ریٹیل ٹرانسفرز پر انکار جیت کے ساتھ `compliance.audit::{stream_reports, request_snapshot}` کے لیے صرف پڑھنے کے الاؤنسز۔ |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | جے ایف ایس اے کی نگرانی کی لین۔ | ڈوئل کنٹرولز کو نافذ کرنے کے لیے ایک محدود `cbdc.supervision.issue_stop_order` الاؤنس (PerDay window + `max_amount`) اور `force_liquidation` پر واضح انکار شامل کرتا ہے۔ |

ان فکسچر کی کلوننگ کرتے وقت، اپ ڈیٹ کریں:

1. `uaid` اور `dataspace` آئی ڈیز اس شریک اور لین سے مماثل ہیں جنہیں آپ فعال کر رہے ہیں۔
2. گورننس شیڈول پر مبنی `activation_epoch`/`expiry_epoch` ونڈوز۔
3. `notes` ریگولیٹر کے پالیسی حوالوں کے ساتھ فیلڈز (MiCA آرٹیکل، JFSA
   سرکلر، وغیرہ)۔
4. الاؤنس ونڈوز (`PerSlot`, `PerMinute`, `PerDay`) اور اختیاری
   `max_amount` کیپس کرتا ہے لہذا SDKs میزبان کے طور پر وہی حدود نافذ کرتے ہیں۔

## 6. SDK صارفین کے لیے مائیگریشن نوٹسموجودہ SDK انضمام جن کا حوالہ فی ڈومین اکاؤنٹ IDs پر منتقل ہونا ضروری ہے۔
اوپر بیان کردہ UAID مرکوز سطحیں۔ اپ گریڈ کے دوران اس چیک لسٹ کا استعمال کریں:

  اکاؤنٹ آئی ڈیز Rust/JS/Swift/Android کے لیے اس کا مطلب تازہ ترین میں اپ گریڈ کرنا ہے۔
  ورک اسپیس کریٹس یا Norito بائنڈنگز کو دوبارہ تخلیق کرنا۔
- **API کالز:** ڈومین کے دائرہ کار والے پورٹ فولیو سوالات کو اس سے بدل دیں۔
  `GET /v1/accounts/{uaid}/portfolio` اور مینی فیسٹ/بائنڈنگ اینڈ پوائنٹس۔
  `GET /v1/accounts/{uaid}/portfolio` اختیاری `asset_id` استفسار کو قبول کرتا ہے
  پیرامیٹر جب بٹوے کو صرف ایک اثاثہ مثال کی ضرورت ہوتی ہے۔ کلائنٹ مددگار اس طرح
  جیسا کہ `ToriiClient.getUaidPortfolio` (JS) اور Android
  `SpaceDirectoryClient` پہلے ہی ان راستوں کو لپیٹ چکا ہے۔ ان کو اپنی مرضی سے ترجیح دیں۔
  HTTP کوڈ۔
- **کیچنگ اور ٹیلی میٹری:** خام کی بجائے UAID + ڈیٹا اسپیس کے ذریعہ کیش اندراجات
  اکاؤنٹ آئی ڈیز، اور ایمیٹ ٹیلی میٹری کو UAID لٹریل دکھاتا ہے تاکہ آپریشنز ہو سکیں
  اسپیس ڈائرکٹری شواہد کے ساتھ لاگز کو لائن اپ کریں۔
- **خرابی سے نمٹنا:** نئے اختتامی نقطے UAID کی تصریف کی سخت غلطیاں واپس کرتے ہیں۔
  `docs/source/torii/portfolio_api.md` میں دستاویزی؛ ان کوڈز کی سطح
  لفظی طور پر تاکہ سپورٹ ٹیمیں ریپرو اقدامات کے بغیر مسائل کو ٹریج کر سکیں۔
- **ٹیسٹنگ:** اوپر بیان کردہ فکسچر کو وائر کریں (علاوہ آپ کا اپنا UAID مینی فیسٹ)
  SDK ٹیسٹ سویٹس میں Norito راؤنڈ ٹرپس اور واضح تشخیص کو ثابت کرنے کے لیے
  میزبان کے نفاذ سے ملائیں۔

## 7. حوالہ جات- `docs/space-directory.md` — آپریٹر پلے بک جس میں لائف سائیکل کی گہری تفصیل ہے۔
- `docs/source/torii/portfolio_api.md` - UAID پورٹ فولیو کے لیے REST اسکیما اور
  ظاہری نقطہ.
- `crates/iroha_cli/src/space_directory.rs` — CLI نفاذ کا حوالہ دیا گیا ہے۔
  یہ گائیڈ.
- `fixtures/space_directory/capability/*.manifest.json` — ریگولیٹر، ریٹیل، اور
  CBDC مینی فیسٹ ٹیمپلیٹس کلوننگ کے لیے تیار ہیں۔