---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ معیاری ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب گیٹ وے کاپی کے طور پر کام کرتا ہے
معیار ماخذ فائل ترجمہ کے بہاؤ کے لئے باقی ہے۔
:::

# SNS لاگر انٹرفیس اور گورننس ہکس (SN-2B)

** حیثیت: ** ورژن 2026-03-24-جائزہ کے تحت Nexus کور  
** روڈ میپ لنک: ** SN-2B "رجسٹرار API اور گورننس ہکس"  
** شرائط: ** اسکیما تعریفیں [`registry-schema.md`] (./registry-schema.md) میں)

اس نوٹ میں Torii اختتامی نکات ، GRPC خدمات ، DTOS ، درخواست/جواب ، اور نشانات کی وضاحت کی گئی ہے۔
سورہ نام سروس (ایس این ایس) رجسٹرار کو چلانے کے لئے ضروری گورننس۔ یہ ایس ڈی کے کے لئے حوالہ معاہدہ ہے
بٹوے اور آٹومیشن جس میں ایس این ایس کے ناموں کو اندراج ، تجدید یا انتظام کرنے کی ضرورت ہوتی ہے۔

## 1۔ منتقلی اور توثیق

| ضرورت | تفصیلات |
| --------- | ------------ |
| پروٹوکول | `/v1/sns/*` اور GRPC سروس `sns.v1.Registrar` کے تحت آرام کریں۔ وہ دونوں Norito-JSON (`application/json`) اور Norito-RPC بائنری (`application/x-norito`) قبول کرتے ہیں۔ |
| Auth | `Authorization: Bearer` ٹوکن یا MTLS سرٹیفکیٹ ہر لاحقہ اسٹیورڈ کو جاری کرتے ہیں۔ گورننس حساس اختتامی نکات (منجمد/غیر منقولہ ، محفوظ اسائنمنٹس) کے لئے `scope=sns.admin` کی ضرورت ہوتی ہے۔ |
| شرح کی حد | رجسٹرارس شیئر بالٹی `torii.preauth_scheme_limits` JSON کالر کے ساتھ ساتھ ہر لاحقہ کے لئے پھٹ جانے والی حدود: `sns.register` ، `sns.renew` ، `sns.controller` ، `sns.freeze`۔ |
| پیمائش | Torii `torii_request_duration_seconds{scheme}` / Norito رجسٹر پروسیسرز کے لئے (نامزد `scheme="norito_rpc"`) ؛ انٹرفیس `sns_registrar_status_total{result, suffix_id}` میں بھی اضافہ کرتا ہے۔ |

## 2۔ ڈی ٹی او جائزہ

فیلڈز [`registry-schema.md`] (./registry-schema.md) میں بیان کردہ معیاری ڈھانچے کا حوالہ دیتے ہیں۔ تمام پے لوڈ میں مبہم روٹنگ سے بچنے کے لئے `NameSelectorV1` + `SuffixId` شامل ہیں۔

```text
Struct RegisterNameRequestV1 {
    selector: NameSelectorV1,
    owner: AccountId,
    controllers: Vec<NameControllerV1>,
    term_years: u8,                     // 1..=max_term_years
    pricing_class_hint: Option<u8>,     // steward-advertised tier id
    payment: PaymentProofV1,
    governance: GovernanceHookV1,
    metadata: Metadata,
}

Struct RegisterNameResponseV1 {
    name_record: NameRecordV1,
    registry_event: RegistryEventV1,
    revenue_accrual: RevenueAccrualEventV1,
}

Struct PaymentProofV1 {
    asset_id: AssetId,
    gross_amount: TokenValue,
    net_amount: TokenValue,
    settlement_tx: Hash,
    payer: AccountId,
    signature: Signature,               // steward/treasury cosign
}

Struct GovernanceHookV1 {
    proposal_id: String,
    council_vote_hash: Hash,
    dao_vote_hash: Hash,
    steward_ack: Signature,
    guardian_clearance: Option<Signature>,
}

Struct RenewNameRequestV1 {
    selector: NameSelectorV1,
    term_years: u8,
    payment: PaymentProofV1,
}

Struct TransferNameRequestV1 {
    selector: NameSelectorV1,
    new_owner: AccountId,
    governance: GovernanceHookV1,
}

Struct UpdateControllersRequestV1 {
    selector: NameSelectorV1,
    controllers: Vec<NameControllerV1>,
}

Struct FreezeNameRequestV1 {
    selector: NameSelectorV1,
    reason: String,
    until: Timestamp,
    guardian_ticket: Signature,
}

Struct ReservedAssignmentRequestV1 {
    selector: NameSelectorV1,
    reserved_label: ReservedNameV1,
    governance: GovernanceHookV1,
}
```

## 3. آرام کے اختتامی مقامات

| اختتامی نقطہ | طریقہ | پے لوڈ | تفصیل |
| ------------- | --------- | --------- | ------- |
| `/v1/sns/names` | پوسٹ | `RegisterNameRequestV1` | کسی نام کو رجسٹر کریں یا دوبارہ کھولیں۔ قیمتوں کا تعین طبقہ حل کرتا ہے ، ادائیگی/گورننس کے ثبوتوں کی تصدیق کرتا ہے ، اور لاگ ان واقعات کو جاری کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/renew` | پوسٹ | `RenewNameRequestV1` | مدت میں توسیع کرتا ہے۔ پالیسی سے ونڈوز گریس/چھٹکارے کو نافذ کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/transfer` | پوسٹ | `TransferNameRequestV1` | گورننس کی منظوری سے منسلک ہونے کے بعد ملکیت منتقل کردی جاتی ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/controllers` | put | `UpdateControllersRequestV1` | گروپ کنٹرولرز کی جگہ ؛ دستخط شدہ اکاؤنٹ کے پتے کی توثیق کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | پوسٹ | `FreezeNameRequestV1` | گارڈین/کونسل کو منجمد کریں۔ گارڈین ٹکٹ اور گورننس کتاب کے حوالہ کی ضرورت ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | حذف کریں | `GovernanceHookV1` | پروسیسنگ کے بعد ڈیفروسٹنگ ؛ بورڈ کے لئے اوور رائڈ رجسٹریشن کو یقینی بناتا ہے۔ |
| `/v1/sns/reserved/{selector}` | پوسٹ | `ReservedAssignmentRequestV1` | اسٹیورڈ/کونسل کے ذریعہ محفوظ نام تفویض کرنا۔ |
| `/v1/sns/policies/{suffix_id}` | حاصل کریں | - | موجودہ `SuffixPolicyV1` (کیچ ایبل) ملتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}` | حاصل کریں | - | `NameRecordV1` موجودہ + اصل حالت (فعال ، فضل ، وغیرہ) کو لوٹاتا ہے۔ |

** سلیکٹر انکوڈنگ: ** `{selector}` طبقہ i105 ، ADDR-5 کے مطابق کمپریسڈ یا معیاری ہیکس قبول کرتا ہے۔ Torii اسے `NameSelectorV1` کے ذریعے پرنٹ کرتا ہے۔

** غلطی کا نمونہ: ** تمام اختتامی نکات Norito ، `message` ، `details` کے ساتھ Norito JSON واپس کریں گے۔ کوڈز میں `sns_err_reserved` ، `sns_err_payment_mismatch` ، `sns_err_policy_violation` ، `sns_err_governance_missing` شامل ہیں۔

### 3.1 سی ایل آئی مددگار (N0 دستی لاگر کی ضرورت)

بند بیٹا اسٹیورڈز اب جے ایس این کو دستی طور پر کارروائی کیے بغیر سی ایل آئی کے ذریعے لاگر چلا سکتے ہیں:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```- `--owner` نے CLI کی ترتیبات کا اکاؤنٹ فرض کیا ؛ اضافی کنٹرولر اکاؤنٹس (پہلے سے طے شدہ `[owner]`) شامل کرنے کے لئے `--controller` کو دہرائیں۔
- بلٹ ان پش اطلاعات براہ راست `PaymentProofV1` سے ملتے ہیں۔ جب آپ کے پاس منظم رسید ہوتی ہے تو `--payment-json PATH` پاس کریں۔ میٹا ڈیٹا (`--metadata-json`) اور گورننس ہکس (`--governance-json`) اسی طرز پر عمل کرتے ہیں۔

ایڈز پڑھنا صرف مشقوں کی تکمیل کرتا ہے:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

عمل درآمد کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں ؛ کمانڈز اس دستاویز میں بیان کردہ Norito DTOs کو دوبارہ استعمال کرتے ہیں تاکہ CLI آؤٹ پٹس Torii جوابات بائٹ کے ذریعہ بائٹ کے ساتھ ملتے ہیں۔

تجدیدات ، منتقلی ، اور سرپرست کے طریقہ کار کو پورا کرنے والی اضافی مدد:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner <i105-account-id> \
  --governance-json /path/to/hook.json

# Freeze/unfreeze flows
iroha sns freeze \
  --selector makoto.sora \
  --reason "guardian investigation" \
  --until-ms 1750000000000 \
  --guardian-ticket '{"sig":"guardian"}'

iroha sns unfreeze \
  --selector makoto.sora \
  --governance-json /path/to/unfreeze_hook.json
```

`--governance-json` میں ایک درست `GovernanceHookV1` ریکارڈ (تجویز ID ، ووٹ ہیشس ، اسٹیورڈ/سرپرست دستخط) پر مشتمل ہونا ضروری ہے۔ ہر کمانڈ صرف اسی طرح کے `/v1/sns/names/{namespace}/{literal}/...` اختتامی نقطہ کی آئینہ دار ہے تاکہ بیٹا پلیئر ورزش کرسکیں جس کو Torii SDKs کو فون کرے گا۔

## 4. جی آر پی سی سروس

```text
service Registrar {
    rpc Register(RegisterNameRequestV1) returns (RegisterNameResponseV1);
    rpc Renew(RenewNameRequestV1) returns (NameRecordV1);
    rpc Transfer(TransferNameRequestV1) returns (NameRecordV1);
    rpc UpdateControllers(UpdateControllersRequestV1) returns (NameRecordV1);
    rpc Freeze(FreezeNameRequestV1) returns (NameRecordV1);
    rpc Unfreeze(GovernanceHookV1) returns (NameRecordV1);
    rpc AssignReserved(ReservedAssignmentRequestV1) returns (NameRecordV1);
    rpc GetRegistration(NameSelectorV1) returns (NameRecordV1);
    rpc GetPolicy(SuffixId) returns (SuffixPolicyV1);
}
```

تار فارمیٹ: ہیش اسکیم Norito مرتب وقت پر رجسٹرڈ ہے
`fixtures/norito_rpc/schema_hashes.json` (قطار `RegisterNameRequestV1` ،
`RegisterNameResponseV1` ، `NameRecordV1` ، وغیرہ)۔

## 5. گورننس ہکس اور ثبوت

ہر ریاستی تبدیلی کال کے ساتھ مناسب ریبوٹ شواہد کے ساتھ ہونا ضروری ہے۔

| عمل | گورننس کا مطلوبہ ڈیٹا
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| معیاری رجسٹریشن/تجدید | ادائیگی کا ثبوت تصفیے کی ہدایات کی نشاندہی کرتا ہے۔ کونسل کے ووٹ کی ضرورت نہیں ہے جب تک کہ ٹرینچ کو اسٹیورڈ کی منظوری کی ضرورت نہ ہو۔ |
| پریمیم سم رجسٹریشن/محفوظ عہدہ | `GovernanceHookV1` تجویز ID + اسٹیورڈ کے اعتراف کی نشاندہی کرتا ہے۔ |
| منتقلی | بورڈ ووٹ ہیش + داؤ سگنل ہیش ؛ کلیئرنس گارڈین جب تنازعات کے حل کے ذریعہ گاڑی کو متحرک کیا جاتا ہے۔ |
| منجمد/غیر منقولہ | اوور رائڈ بورڈ (غیر منقولہ) کے ساتھ گارڈین ٹکٹ پر دستخط کریں۔ |

Torii جانچ پڑتال کرکے شواہد کی تصدیق کرتا ہے:

1. پروپوزل ID گورننس بک (`/v1/governance/proposals/{id}`) میں ہے اور اس کی حیثیت `Approved` ہے۔
2. ہیش ریکارڈ شدہ ووٹنگ کے نشانات سے ملتی ہے۔
3. اسٹیورڈ/گارڈین کے دستخط `SuffixPolicyV1` سے متوقع عوامی چابیاں کی نشاندہی کرتے ہیں۔

ناکام توثیق `sns_err_governance_missing` کی واپسی کرتا ہے۔

## 6. ورک فلو کی مثالیں

### 6.1 معیاری ریکارڈنگ

1. قیمتیں ، فضل کی مدت ، اور دستیاب طبقات حاصل کرنے کے لئے کسٹمر `/v1/sns/policies/{suffix_id}` سے پوچھ گچھ کرتا ہے۔
2. کلائنٹ بلڈ `RegisterNameRequestV1`:
   - `selector` لیبل i105 (ترجیحی) یا سی ڈی (دوسرا آپشن) سے اخذ کیا گیا ہے۔
   - پالیسی کی حدود میں `term_years`۔
   - `payment` ٹریژری اسپلٹر/اسٹیورڈ سوئچ سے مراد ہے۔
3. Torii چیک:
   - لیبل + محفوظ فہرست کو معمول پر لائیں۔
   - اصطلاح/مجموعی قیمت بمقابلہ `PriceTierV1`۔
   - ادائیگی کی رقم کا ثبوت> = حساب شدہ قیمت + فیس۔
4. کامیابی کے بعد Torii:
   - `NameRecordV1` کو بچاتا ہے۔
   - `RegistryEventV1::NameRegistered` کو جاری کرتا ہے۔
   - `RevenueAccrualEventV1` کو جاری کرتا ہے۔
   - نیا ریکارڈ + واقعات واپس کرتا ہے۔

### 6.2 فضل کی مدت کے دوران تجدید

فضل کی تجدید میں جرمانے کے بیان کے علاوہ معیاری درخواست بھی شامل ہے:

- Torii `now` کا موازنہ `grace_expires_at` کے خلاف کرتا ہے اور `SuffixPolicyV1` کے سرچارج ٹیبلز کا اضافہ کرتا ہے۔
- ادائیگی کے ثبوت کو سرچارج کا احاطہ کرنا ہوگا۔ ناکامی => `sns_err_payment_mismatch`۔
- `RegistryEventV1::NameRenewed` نئے `expires_at` کو رجسٹر کرتا ہے۔

### 6.3 گارڈین منجمد اور اوور رائڈ بورڈ1. گارڈین `FreezeNameRequestV1` کو ٹکٹ کے ساتھ بھیجتا ہے جس میں حادثے کی شناخت کی نشاندہی ہوتی ہے۔
2. Torii رجسٹر کو `NameStatus::Frozen` میں منتقل کرتا ہے ، اور `NameFrozen` کو جاری کرتا ہے۔
3. پروسیسنگ کے بعد ، بورڈ کو زیربحث ؛ آپریٹر `/v1/sns/names/{namespace}/{literal}/freeze` کو `GovernanceHookV1` کے ساتھ حذف کرتا ہے۔
4. Torii اوور رائڈ کے لئے چیک کرتا ہے ، اور `NameUnfrozen` کو جاری کرتا ہے۔

## 7. توثیق اور غلطی کے کوڈز

| کوڈ | تفصیل | HTTP |
| ------- | ------- | ------ |
| `sns_err_reserved` | نشان محفوظ یا ممنوع ہے۔ | 409 |
| `sns_err_policy_violation` | مدت ، طبقہ ، یا کنٹرولرز کا گروپ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | قیمت یا اثاثہ ادائیگی کے ثبوت میں مماثل نہیں ہے۔ | 402 |
| `sns_err_governance_missing` | گورننس کے مطلوبہ اثرات غیر حاضر/غلط ہیں۔ | 403 |
| `sns_err_state_conflict` | موجودہ لائف سائیکل اسٹیٹ میں آپریشن کی اجازت نہیں ہے۔ | 409 |

تمام کوڈ `X-Iroha-Error-Code` اور Norito JSON/NRPC فارمیٹڈ ریپرز کے ذریعے ظاہر ہوتے ہیں۔

## 8. نفاذ کے نوٹ

- Torii اسٹورز `NameRecordV1.auction` کے تحت زیر التواء نیلامی اور براہ راست رجسٹریشن کی کوششوں کو مسترد کرتا ہے جبکہ حیثیت `PendingAuction` ہے۔
- ادائیگی کے ثبوت Norito نوٹ بک کی رسیدیں ؛ ٹریژری سروسز ہیلپ APIs (`/v1/finance/sns/payments`) فراہم کرتی ہے۔
- SDKs کو ان نکات کو مضبوط مددگاروں سے لپیٹنا چاہئے تاکہ بٹوے واضح غلطی کی وجوہات (`ERR_SNS_RESERVED` ، وغیرہ) ظاہر کرسکیں۔

## 9. اگلے اقدامات

- جب SN-3 نیلامی آتی ہے تو جسمانی لاگ نوڈس پر Torii پروسیسرز کو پابند کریں۔
- خصوصی SDK گائیڈز (مورچا/JS/Swift) شائع کریں جو اس انٹرفیس کا حوالہ دیتے ہیں۔
- گورننس ہکس میں کراس لنکس کے ساتھ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) میں توسیع کریں۔