---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب خدمت کرتا ہے
پورٹل کی ایک کیننیکل کاپی۔ ماخذ فائل PR ترجموں کے لئے باقی ہے۔
:::

# SNS رجسٹرار API اور مینجمنٹ ہکس (SN-2B)

** حیثیت: ** تیار کردہ 2026-03-24-زیر التواء Nexus کور  
** روڈ میپ لنک: ** SN-2B "رجسٹرار API اور گورننس ہکس"  
** شرائط: ** اسکیما تعریفیں [`registry-schema.md`] (./registry-schema.md) میں)

اس نوٹ میں Torii اختتامی نکات ، GRPC خدمات ، درخواست/رسپانس DTOS ، اور کی وضاحت کی گئی ہے
SORA نام سروس رجسٹرار کے آپریشن کے لئے درکار انتظامی نمونے
(SNS) یہ ایس ڈی کے ، بٹوے اور آٹومیشن کا مستند معاہدہ ہے
ایس این ایس کے ناموں کو اندراج ، تجدید یا ان کا نظم کرنے کی ضرورت ہے۔

## 1. نقل و حمل اور توثیق

| ضرورت | تفصیلات |
| ------------ | -------- |
| پروٹوکول | `/v1/sns/*` اور GRPC سروس `sns.v1.Registrar` کے تحت آرام کریں۔ دونوں Norito-JSON (`application/json`) اور بائنری Norito-RPC (`application/x-norito`) کو قبول کرتے ہیں۔ |
| Auth | `Authorization: Bearer` ٹوکن یا MTLS سرٹیفکیٹ لاحقہ اسٹیورڈ کے ذریعہ جاری کردہ۔ انتظامیہ سے متعلق حساس اختتامی مقامات (منجمد/غیر منقولہ ، محفوظ اسائنمنٹس) کے لئے `scope=sns.admin` کی ضرورت ہوتی ہے۔ |
| پابندیاں | رجسٹرار شیئر بالٹی `torii.preauth_scheme_limits` JSON کالرز کے ساتھ ساتھ پھٹ جانے والی حدود کے ساتھ لاحقہ: `sns.register` ، `sns.renew` ، `sns.controller` ، `sns.freeze`۔ |
| ٹیلی میٹری | Torii `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` کو رجسٹرار ہینڈلرز کے لئے شائع کرتا ہے (`scheme="norito_rpc"` کے ذریعہ فلٹر) ؛ API `sns_registrar_status_total{result, suffix_id}` میں بھی اضافہ کرتا ہے۔ |

## 2۔ ڈی ٹی او جائزہ

فیلڈز ریفرنس کینونیکل ڈھانچے [`registry-schema.md`] (./registry-schema.md) میں بیان کردہ۔ مبہم روٹنگ سے بچنے کے ل All تمام پے لوڈ ان لائن `NameSelectorV1` + `SuffixId`۔

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
| ---------- | ------- | --------- | ------------ |
| `/v1/sns/registrations` | پوسٹ | `RegisterNameRequestV1` | کسی نام کو رجسٹر یا دوبارہ کھولتا ہے۔ قیمت کے درجے کو حل کرتا ہے ، ادائیگی/کنٹرول کے ثبوت کی تصدیق کرتا ہے ، رجسٹری کے واقعات کو خارج کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/renew` | پوسٹ | `RenewNameRequestV1` | ڈیڈ لائن میں توسیع کرتا ہے۔ پالیسی سے ونڈوز گریس/چھٹکارے کا اطلاق کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/transfer` | پوسٹ | `TransferNameRequestV1` | انتظامیہ کی منظوری سے منسلک ہونے کے بعد ملکیت کی منتقلی۔ |
| `/v1/sns/registrations/{selector}/controllers` | put | `UpdateControllersRequestV1` | کنٹرولرز کے ایک سیٹ کی جگہ لے لیتا ہے۔ دستخط شدہ اکاؤنٹ کے پتے کی توثیق کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | پوسٹ | `FreezeNameRequestV1` | گارڈین/کونسل کو منجمد کریں۔ گارڈین کے ٹکٹ اور گورننس دستاویز سے لنک کی ضرورت ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | حذف کریں | `GovernanceHookV1` | خاتمے کے بعد غیر منقولہ ؛ اس بات کو یقینی بناتا ہے کہ کونسل کا اوور رائڈ طے شدہ ہے۔ |
| `/v1/sns/reserved/{selector}` | پوسٹ | `ReservedAssignmentRequestV1` | محفوظ اسٹیورڈ/کونسل کے نام تفویض کرنا۔ |
| `/v1/sns/policies/{suffix_id}` | حاصل کریں | - | موجودہ `SuffixPolicyV1` (کیچ ایبل) ملتا ہے۔ |
| `/v1/sns/registrations/{selector}` | حاصل کریں | - | موجودہ `NameRecordV1` + موثر حالت (فعال ، فضل ، وغیرہ) لوٹاتا ہے۔ |** سلیکٹر انکوڈنگ: ** طبقہ `{selector}` IH58 ، کمپریسڈ (`sora`) یا ADDR-5 کے ذریعہ کیننیکل ہیکس قبول کرتا ہے۔ Torii `NameSelectorV1` کے ذریعے معمول بناتا ہے۔

** غلطی کا ماڈل: ** تمام اختتامی نکات Norito ، `message` ، `details` کے ساتھ Norito JSON واپس کریں گے۔ کوڈز میں `sns_err_reserved` ، `sns_err_payment_mismatch` ، `sns_err_policy_violation` ، `sns_err_governance_missing` شامل ہیں۔

### 3.1 سی ایل آئی اسسٹنٹس (دستی ریکارڈر N0 کی ضرورت)

بند بیٹا کے اسٹیورڈز اب جے ایس او این کو دستی طور پر جمع کیے بغیر سی ایل آئی کے ذریعے رجسٹرار کا استعمال کرسکتے ہیں:

```bash
iroha sns register \
  --label makoto \
  --suffix-id 1 \
  --term-years 2 \
  --payment-asset-id xor#sora \
  --payment-gross 240 \
  --payment-settlement '"settlement-tx-hash"' \
  --payment-signature '"steward-signature"'
```

- `--owner` ڈیفالٹ کے ذریعہ سی ایل آئی کنفیگریشن اکاؤنٹ کا استعمال کرتا ہے۔ اضافی کنٹرولر اکاؤنٹس (ڈیفالٹ `[owner]`) شامل کرنے کے لئے `--controller` کو دہرائیں۔
- ان لائن ادائیگی کے جھنڈوں کو براہ راست `PaymentProofV1` پر نقش کیا جاتا ہے۔ اگر پہلے ہی کوئی ساختی رسید موجود ہے تو `--payment-json PATH` پاس کریں۔ میٹا ڈیٹا (`--metadata-json`) اور گورننس ہکس (`--governance-json`) اسی طرز پر عمل کریں۔

صرف پڑھنے والے معاونین کی تکمیل کی مشقیں:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

عمل درآمد کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں ؛ کمانڈز اس دستاویز سے Norito DTO کو دوبارہ استعمال کرتے ہیں ، لہذا CLI آؤٹ پٹ ایک ہی بائٹ کے لئے بائٹ Torii ردعمل ہے۔

اضافی مددگار تجدیدات ، منتقلی اور سرپرست اقدامات کا احاطہ کرتے ہیں:

```bash
# Renew an expiring name
iroha sns renew \
  --selector makoto.sora \
  --term-years 1 \
  --payment-asset-id xor#sora \
  --payment-gross 120 \
  --payment-settlement '"renewal-settlement"' \
  --payment-signature '"steward-signature"'

# Transfer ownership once governance approves
iroha sns transfer \
  --selector makoto.sora \
  --new-owner ih58... \
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

`--governance-json` میں صحیح اندراج `GovernanceHookV1` (تجویز ID ، ووٹ ہیشس ، اسٹیورڈ/سرپرست دستخطوں) پر مشتمل ہونا ضروری ہے۔ ہر کمانڈ آسانی سے متعلقہ `/v1/sns/registrations/{selector}/...` اختتامی نقطہ کی عکاسی کرتا ہے تاکہ بیٹا آپریٹرز عین مطابق Torii سطحوں کی تکرار کرسکیں جو SDK کو کال کریں گے۔

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

تار فارمیٹ: Norito سرکٹ کا ہیش لکھا گیا ہے
`fixtures/norito_rpc/schema_hashes.json` (لائنز `RegisterNameRequestV1` ،
`RegisterNameResponseV1` ، `NameRecordV1` ، وغیرہ)۔

## 5. کنٹرول ہکس اور ثبوت

ہر ترمیم کرنے والے چیلنج میں تولیدی صلاحیت کے ل suitable موزوں شواہد شامل ہونا ضروری ہیں:

| ایکشن | مطلوبہ مینجمنٹ ڈیٹا |
| ---------- | ---------------------------------- |
| معیاری رجسٹریشن/تجدید | تصفیہ کی ہدایات کے حوالے سے ادائیگی کا ثبوت ؛ کونسل کے ووٹ کی ضرورت نہیں ہے جب تک کہ اس درجے کو اسٹیورڈ کی منظوری کی ضرورت نہ ہو۔ |
| رجسٹریشن پریمیم ٹائر / محفوظ اسائنمنٹ | `GovernanceHookV1` تجویز ID + اسٹیورڈ کے اعتراف کے حوالے سے۔ |
| منتقلی | کونسل ووٹ ہیش + داؤ سگنل ہیش ؛ گارڈین کلیئرنس ، جب کسی تنازعہ کے حل کے ذریعہ منتقلی شروع کی جاتی ہے۔ |
| منجمد/غیر منقولہ | دستخط گارڈین ٹکٹ پلس کونسل اوور رائڈ (غیر منقولہ)۔ |

Torii جانچ پڑتال کرکے شواہد کی جانچ پڑتال کرتا ہے:

1. پروپوزل ID مینجمنٹ لیجر (`/v1/governance/proposals/{id}`) اور حیثیت `Approved` میں موجود ہے۔
2. ہیش ریکارڈ شدہ ووٹنگ نمونے سے ملتی ہے۔
3. اسٹیورڈ/گارڈین کے دستخط `SuffixPolicyV1` سے متوقع عوامی چابیاں کا حوالہ دیتے ہیں۔

ناکام چیک واپس `sns_err_governance_missing`۔

## 6. ورک فلو کی مثالیں

### 6.1 معیاری رجسٹریشن1. کلائنٹ قیمتیں ، فضل اور دستیاب درجات حاصل کرنے کے لئے `/v1/sns/policies/{suffix_id}` سے درخواست کرتا ہے۔
2. کلائنٹ `RegisterNameRequestV1` تیار کرتا ہے:
   - `selector` ترجیحی IH58 یا دوسرا ترجیحی کمپریسڈ (`sora`) لیبل سے اخذ کیا گیا ہے۔
   - پالیسی کے اندر `term_years`۔
   - `payment` سے مراد ترجمہ اسپلٹر ٹریژری/اسٹیورڈ ہے۔
3. Torii چیک:
   - لیبل + محفوظ فہرست کو معمول پر لانا۔
   - اصطلاح/مجموعی قیمت بمقابلہ `PriceTierV1`۔
   - ادائیگی کی ثبوت کی رقم> = حساب شدہ قیمت + کمیشن۔
4. اگر Torii کامیاب ہے:
   - `NameRecordV1` کو بچاتا ہے۔
   - `RegistryEventV1::NameRegistered` کو خارج کرتا ہے۔
   - `RevenueAccrualEventV1` کو خارج کرتا ہے۔
   - ایک نیا اندراج + واقعات واپس کرتا ہے۔

### 6.2 فضل کی مدت کے دوران توسیع

فضل میں توسیع میں ایک معیاری درخواست کے علاوہ جرمانے کا پتہ لگانا بھی شامل ہے:

- Torii `now` کا موازنہ `grace_expires_at` کے ساتھ کرتا ہے اور `SuffixPolicyV1` سے سرچارج ٹیبلز کا اضافہ کرتا ہے۔
- ادائیگی کے ثبوت کو سرچارج کا احاطہ کرنا چاہئے۔ غلطی => `sns_err_payment_mismatch`۔
- `RegistryEventV1::NameRenewed` ایک نیا `expires_at` لکھتا ہے۔

### 6.3 گارڈین منجمد اور کونسل اوور رائڈ

1. گارڈین `FreezeNameRequestV1` کو ٹکٹ کے ساتھ واقعہ کی شناخت کا حوالہ دیتے ہوئے بھیجتا ہے۔
2. Torii ریکارڈ کو `NameStatus::Frozen` میں ترجمہ کرتا ہے ، `NameFrozen` کو خارج کرتا ہے۔
3. خاتمے کے بعد ، کونسل ایک اوور رائڈ جاری کرتی ہے۔ بیان `GovernanceHookV1` کے ساتھ `/v1/sns/registrations/{selector}/freeze` کو حذف کرتا ہے۔
4. Torii چیک اوور رائڈ ، `NameUnfrozen` کو خارج کرتا ہے۔

## 7. توثیق اور غلطی کے کوڈ

| کوڈ | تفصیل | HTTP |
| ----- | ---------- | ------ |
| `sns_err_reserved` | لیبل محفوظ یا لاک ہے۔ | 409 |
| `sns_err_policy_violation` | اصطلاح ، درجے ، یا کنٹرولرز کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | ادائیگی کے ثبوت میں قدر یا اثاثہ کی عدم مطابقت۔ | 402 |
| `sns_err_governance_missing` | مطلوبہ انتظامی نمونے غائب ہیں/غلط ہیں۔ | 403 |
| `sns_err_state_conflict` | موجودہ لائف سائیکل اسٹیٹ میں آپریشن درست نہیں ہے۔ | 409 |

تمام کوڈز `X-Iroha-Error-Code` کے ذریعے بھیجے جاتے ہیں اور Norito JSON/NRPC لفافوں کے ذریعے۔

## 8. نفاذ کے نوٹ

- Torii اسٹورز `NameRecordV1.auction` میں نیلامی زیر التواء اور `PendingAuction` تک براہ راست رجسٹریشن کی کوششوں کو مسترد کرتا ہے۔
- ادائیگی کے ثبوت Norito لیجر رسیدوں کو دوبارہ استعمال کریں۔ ٹریژری سروسز ایک مددگار API (`/v1/finance/sns/payments`) مہیا کرتی ہے۔
- SDKs کو ان اختتامی نکات کو مضبوطی سے ٹائپ شدہ مددگاروں سے لپیٹنا چاہئے تاکہ بٹوے غلطیوں (`ERR_SNS_RESERVED` ، وغیرہ) کی واضح وجوہات دکھا سکیں۔

## 9. اگلے اقدامات

- SN-3 نیلامی کی ظاہری شکل کے بعد Torii ہینڈلرز کو حقیقی رجسٹری معاہدے سے مربوط کریں۔
- ایس ڈی کے گائیڈز (زنگ/جے ایس/سوئفٹ) شائع کریں جو اس API کا حوالہ دیتے ہیں۔
- گورننس ہک شواہد کے شعبوں کو کراس ریفرنس کے ساتھ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کو بڑھا دیں۔