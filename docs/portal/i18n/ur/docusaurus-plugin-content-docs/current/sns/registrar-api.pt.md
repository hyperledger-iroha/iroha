---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کا آئینہ دار ہے اور اب اس کے طور پر کام کرتا ہے
پورٹل کی کیننیکل کاپی۔ ماخذ فائل ترجمہ کے بہاؤ کے لئے باقی ہے۔
:::

# SNS رجسٹرار API اور گورننس ہکس (SN-2B)

** حیثیت: ** لکھا ہوا 2026-03-24-Nexus کور کے جائزہ کے تحت  
** روڈ میپ لنک: ** SN-2B "API اور گورننس ہکس رجسٹر کریں"  
** شرائط: ** اسکیما تعریفیں [`registry-schema.md`] (./registry-schema.md) میں)

یہ نوٹ Torii اختتامی نکات ، GRPC خدمات ، درخواست/رسپانس DTOS اور کی وضاحت کرتا ہے
گورننس نمونے SORA نام سروس (SNS) رجسٹرار کو چلانے کے لئے درکار ہیں۔
یہ ایس ڈی کے ، بٹوے اور آٹومیشن کا مستند معاہدہ ہے جس کو اندراج کرنے کی ضرورت ہے ،
ایس این ایس کے ناموں کی تجدید کریں یا ان کا نظم کریں۔

## 1. نقل و حمل اور توثیق

| ضرورت | تفصیل |
| ----------- | -------------- |
| پروٹوکول | `/v1/sns/*` اور GRPC سروس `sns.v1.Registrar` کے تحت آرام کریں۔ دونوں Norito-JSON (`application/json`) اور بائنری Norito-RPC (`application/x-norito`) کو قبول کرتے ہیں۔ |
| Auth | `Authorization: Bearer` ٹوکن یا MTLS سرٹیفکیٹ لاحقہ اسٹیورڈ کے ذریعہ جاری کردہ۔ گورننس سے حساس اختتامی نکات (منجمد/غیر منقولہ ، محفوظ کردار) کی ضرورت ہوتی ہے `scope=sns.admin`۔ |
| شرح کی حد | رجسٹرار `torii.preauth_scheme_limits` بالٹیوں کو JSON کالرز کے ساتھ ساتھ برسٹ حدود کے ساتھ شیئر کریں: `sns.register` ، `sns.renew` ، `sns.controller` ، `sns.freeze`۔ |
| ٹیلی میٹری | Torii `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` کو رجسٹرار ہینڈلرز (فلٹر `scheme="norito_rpc"`) سے بے نقاب کرتا ہے۔ API `sns_registrar_status_total{result, suffix_id}` میں بھی اضافہ کرتا ہے۔ |

## 2۔ ڈی ٹی او جائزہ

فیلڈز [`registry-schema.md`] (./registry-schema.md) میں بیان کردہ کیننیکل ڈھانچے کا حوالہ دیتے ہیں۔ تمام پے لوڈ میں مبہم روٹنگ سے بچنے کے لئے `NameSelectorV1` + `SuffixId` شامل ہیں۔

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
| ---------- | ------------ | --------- | ----------- |
| `/v1/sns/names` | پوسٹ | `RegisterNameRequestV1` | کسی نام کو رجسٹر کریں یا دوبارہ کھولیں۔ قیمتوں کے درجے کو حل کرتا ہے ، ادائیگی/گورننس کے ثبوتوں ، رجسٹریشن کے واقعات کو جاری کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/renew` | پوسٹ | `RenewNameRequestV1` | اصطلاح میں توسیع کریں۔ ونڈوز گریس/چھٹکارے کی پالیسی کو نافذ کریں۔ |
| `/v1/sns/names/{namespace}/{literal}/transfer` | پوسٹ | `TransferNameRequestV1` | جب گورننس کی منظوریوں سے منسلک ہوتا ہے تو ملکیت کی منتقلی ہوتی ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/controllers` | put | `UpdateControllersRequestV1` | کنٹرولرز کے سیٹ کی جگہ لے لیتا ہے۔ دستخط شدہ اکاؤنٹ کے پتے کی توثیق کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | پوسٹ | `FreezeNameRequestV1` | گارڈین/کونسل منجمد۔ ٹکٹ سرپرست اور گورننس ڈاکٹ کے حوالے کی ضرورت ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | حذف کریں | `GovernanceHookV1` | تدارک کے بعد غیر منقولہ ؛ رجسٹرڈ کونسل کے اوور رائڈ کی ضمانت دیتا ہے۔ |
| `/v1/sns/reserved/{selector}` | پوسٹ | `ReservedAssignmentRequestV1` | اسٹیورڈ/کونسل کے ذریعہ محفوظ ناموں کی تفویض۔ |
| `/v1/sns/policies/{suffix_id}` | حاصل کریں | - | موجودہ `SuffixPolicyV1` (کیچ ایبل) تلاش کریں۔ |
| `/v1/sns/names/{namespace}/{literal}` | حاصل کریں | - | موجودہ `NameRecordV1` + موثر حالت (فعال ، فضل ، وغیرہ) لوٹاتا ہے۔ |** سلیکٹر کوڈنگ: ** طبقہ `{selector}` i105 ، ADDR-5 کے مطابق کمپریسڈ یا کیننیکل ہیکس قبول کرتا ہے۔ Torii `NameSelectorV1` کے ذریعے معمول بناتا ہے۔

** غلطی کا ماڈل: ** تمام اختتامی نکات `code` ، `message` ، `details` کے ساتھ Norito JSON واپس کریں گے۔ کوڈز میں `sns_err_reserved` ، `sns_err_payment_mismatch` ، `sns_err_policy_violation` ، `sns_err_governance_missing` شامل ہیں۔

### 3.1 سی ایل آئی مددگار (N0 دستی رجسٹریشن کی ضرورت)

بند بیٹا اسٹیورز اب جے ایس او این کو دستی طور پر جمع کیے بغیر سی ایل آئی کے ذریعے رجسٹرار کو چلاسکتے ہیں:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id 61CtjvNd9T3THAR65GsMVHr82Bjc \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` ڈیفالٹ سی ایل آئی کنفیگریشن اکاؤنٹ ہے۔ اضافی کنٹرولر اکاؤنٹس (ڈیفالٹ `[owner]`) منسلک کرنے کے لئے `--controller` کو دہرائیں۔
- ان لائن ادائیگی کے جھنڈے براہ راست `PaymentProofV1` پر نقشہ ؛ جب آپ کے پاس پہلے سے ہی ساختی رسید ہو تو `--payment-json PATH` پاس کریں۔ میٹا ڈیٹا (`--metadata-json`) اور گورننس ہکس (`--governance-json`) اسی طرز پر عمل کریں۔

پڑھنے کے لئے صرف مددگار مکمل مضامین:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

عمل درآمد کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں ؛ کمانڈز اس دستاویز میں بیان کردہ Norito DTOs کو دوبارہ استعمال کرتے ہیں تاکہ CLI آؤٹ پٹ Torii جوابات بائٹ کے لئے بائٹ سے میل کھاتا ہے۔

اضافی مددگار تجدیدات ، منتقلی اور سرپرست اقدامات کا احاطہ کرتے ہیں:

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
  --new-owner soraカタカナ... \
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

`--governance-json` میں ایک درست `GovernanceHookV1` ریکارڈ (تجویز ID ، ووٹ ہیشس ، اسٹیورڈ/سرپرست دستخط) پر مشتمل ہونا ضروری ہے۔ ہر کمانڈ آسانی سے متعلقہ `/v1/sns/names/{namespace}/{literal}/...` اختتامی نقطہ کی آئینہ دار ہے تاکہ بیٹا آپریٹرز بالکل Torii سطحوں کی جانچ کرسکیں جس پر SDKs کال کریں گے۔

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

تار فارمیٹ: Norito اسکیم کا ہیش مرتب وقت میں ریکارڈ کیا گیا
`fixtures/norito_rpc/schema_hashes.json` (لائنز `RegisterNameRequestV1` ،
`RegisterNameResponseV1` ، `NameRecordV1` ، وغیرہ)۔

## 5. گورننس ہکس اور ثبوت

ہر کال جو ریاست کو تبدیل کرتی ہے اسے ری پلے کے لئے موزوں شواہد کو جوڑنا چاہئے:

| ایکشن | گورننس ڈیٹا کی ضرورت ہے |
| ------ | ---------------------------------- |
| معیاری رجسٹریشن/تجدید | تصفیہ کی ہدایت کا حوالہ دیتے ہوئے ادائیگی کا ثبوت۔ کونسل کے ووٹ کی ضرورت نہیں ہے جب تک کہ ٹائر کو اسٹیورڈ کی منظوری کی ضرورت نہ ہو۔ |
| پریمیم درجے کی رجسٹریشن / محفوظ اسائنمنٹ | `GovernanceHookV1` حوالہ دینے کی تجویز ID + اسٹیورڈ اعتراف۔ |
| منتقلی | کونسل ووٹ ہیش + داؤ سگنل ہیش ؛ گارڈین کلیئرنس جب تنازعات کے حل کے ذریعہ منتقلی کو متحرک کیا جاتا ہے۔ |
| منجمد/غیر منقولہ | ٹکٹ گارڈین کے علاوہ کونسل کے زیر اثر (غیر منقولہ) |

Torii جانچ پڑتال کرکے شواہد کی جانچ پڑتال کرتا ہے:

1. پروپوزل ID گورننس لیجر (`/v1/governance/proposals/{id}`) میں موجود ہے اور حیثیت `Approved` ہے۔
2. ہیش رجسٹرڈ ووٹنگ نمونے کے مطابق ہے۔
3. اسٹیورڈ/گارڈین کے دستخط `SuffixPolicyV1` کی متوقع عوامی چابیاں کا حوالہ دیتے ہیں۔

ناکامی `sns_err_governance_missing` کی واپسی کرتی ہے۔

## 6. ورک فلو کی مثالیں

### 6.1 معیاری رجسٹریشن1. کسٹمر کے سوالات `/v1/sns/policies/{suffix_id}` دستیاب قیمتوں ، فضل اور درجے کے لئے۔
2. گاہک `RegisterNameRequestV1` جمع کرتا ہے:
   - `selector` لیبل i105 (ترجیحی) یا ٹیبلٹ (دوسرا بہترین آپشن) سے ماخوذ ہے۔
   - پالیسی کی حدود میں `term_years`۔
   - `payment` خزانہ/اسٹیورڈ اسپلٹر کی منتقلی کا حوالہ دیتے ہوئے۔
3. Torii توثیق کرتا ہے:
   - لیبل نارملائزیشن + محفوظ فہرست۔
   - اصطلاح/مجموعی قیمت بمقابلہ `PriceTierV1`۔
   - ادائیگی کی رقم کا ثبوت> = حساب شدہ قیمت + فیس۔
4. کامیابی پر Torii:
   - `NameRecordV1` برقرار ہے۔
   - `RegistryEventV1::NameRegistered` کو جاری کرتا ہے۔
   - `RevenueAccrualEventV1` کو جاری کرتا ہے۔
   - نیا ریکارڈ + واقعات واپس کرتا ہے۔

### 6.2 فضل کے دوران تجدید

فضل کے دوران تجدیدات میں معیاری درخواست کے علاوہ جرمانے کا پتہ لگانا بھی شامل ہے:

- Torii `now` بمقابلہ `grace_expires_at` کا موازنہ کرتا ہے اور `SuffixPolicyV1` سرچارج ٹیبلز شامل کرتا ہے۔
- ادائیگی کے ثبوت کو سرچارج کا احاطہ کرنا چاہئے۔ ناکامی => `sns_err_payment_mismatch`۔
- `RegistryEventV1::NameRenewed` نئے `expires_at` کو رجسٹر کرتا ہے۔

### 6.3 گارڈین منجمد اور کونسل اوور رائڈ

1. گارڈین `FreezeNameRequestV1` کو ٹکٹ حوالہ دینے والے واقعے کی شناخت کے ساتھ بھیجتا ہے۔
2. Torii ریکارڈ کو `NameStatus::Frozen` میں منتقل کرتا ہے ، آؤٹ پٹ `NameFrozen`۔
3. علاج کے بعد ، کونسل ایک اوور رائڈ جاری کرتی ہے۔ آپریٹر `/v1/sns/names/{namespace}/{literal}/freeze` کو `GovernanceHookV1` کے ساتھ حذف کرتا ہے۔
4. Torii اوور رائڈ ، ایشوز `NameUnfrozen` کی توثیق کرتا ہے۔

## 7. توثیق اور غلطی کے کوڈ

| کوڈ | تفصیل | HTTP |
| -------- | ----------- | ------ |
| `sns_err_reserved` | لیبل محفوظ یا مسدود ہے۔ | 409 |
| `sns_err_policy_violation` | اصطلاح ، درجے یا کنٹرولرز کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | ادائیگی کے ثبوت میں قدر یا اثاثہ مماثلت۔ | 402 |
| `sns_err_governance_missing` | لاپتہ/غلط مطلوبہ گورننس نمونے۔ | 403 |
| `sns_err_state_conflict` | زندگی کے چکر کی موجودہ حالت میں آپریشن کی اجازت نہیں ہے۔ | 409 |

تمام کوڈ `X-Iroha-Error-Code` اور Norito ساختی JSON/NRPC لفافوں کے ذریعے ظاہر ہوتے ہیں۔

## 8. نفاذ کے نوٹ

- Torii اسٹورز `NameRecordV1.auction` میں زیر التواء نیلامی اور براہ راست رجسٹریشن کی کوششوں کو مسترد کرتا ہے جبکہ `PendingAuction`۔
- ادائیگی کے ثبوت Norito لیجر سے رسیدوں کو دوبارہ استعمال کرتے ہیں۔ ٹریژری سروسز مددگار APIs (`/v1/finance/sns/payments`) مہیا کرتی ہے۔
- SDKs کو ان اختتامی نکات کو سخت ٹائپ شدہ مددگاروں سے لپیٹنا چاہئے تاکہ بٹوے واضح غلطی کی واضح وجوہات (`ERR_SNS_RESERVED` ، وغیرہ) پیش کریں۔

## 9. اگلے اقدامات

- جب SN-3 نیلامی آتی ہے تو Torii ہینڈلرز کو اصل رجسٹریشن معاہدے سے مربوط کریں۔
- اس API کا حوالہ دیتے ہوئے مخصوص SDK گائیڈز (مورچا/JS/Swift) شائع کریں۔
- گورننس ہک شواہد کے شعبوں میں کراس لنکس کے ساتھ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کو بڑھائیں۔