---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب اس کی خدمت کرتا ہے
پورٹل کی کیننیکل کاپی۔ ماخذ فائل کی ندیوں کے لئے محفوظ ہے
ترجمہ
:::

# SNS رجسٹرار API اور گورننس ہکس (SN-2B)

** حیثیت: ** ریڈیج 2026-03-24-Nexus کور کے ذریعہ جائزہ لیا گیا  
** روڈ میپ لنک: ** SN-2B "رجسٹرار API اور گورننس ہکس"  
** شرائط: ** اسکیما تعریفیں [`registry-schema.md`] (./registry-schema.md) میں)

یہ نوٹ Torii اختتامی نکات ، GRPC خدمات ، درخواست/رسپانس DTOS کی وضاحت کرتا ہے
اور گورننس نمونے SORA نام رجسٹرار کو چلانے کے لئے ضروری ہیں
سروس (ایس این ایس)۔ یہ SDKs ، بٹوے اور کے لئے حوالہ معاہدہ ہے
آٹومیشن جس کو ایس این ایس کے ناموں کو رجسٹر ، تجدید یا انتظام کرنے کی ضرورت ہے۔

## 1. نقل و حمل اور توثیق

| ضرورت | تفصیلات |
| --------- | -------- |
| پروٹوکول | `/v1/sns/*` اور GRPC سروس `sns.v1.Registrar` کے تحت آرام کریں۔ دونوں Norito-JSON (`application/json`) اور Norito-RPC بائنری (`application/x-norito`) قبول کرتے ہیں۔ |
| Auth | `Authorization: Bearer` ٹوکن یا MTLS سرٹیفکیٹ لاحقہ اسٹیورڈ کے ذریعہ جاری کردہ۔ گورننس سے حساس اختتامی مقامات (منجمد/غیر منقولہ ، محفوظ اسائنمنٹس) کے لئے `scope=sns.admin` کی ضرورت ہوتی ہے۔ |
| شرح کی حد | رجسٹرار شیئر بالٹی `torii.preauth_scheme_limits` JSON کالرز کے ساتھ پلس برسٹ حدود کے ساتھ لاحقہ کے ذریعہ: `sns.register` ، `sns.renew` ، `sns.controller` ، `sns.freeze`۔ |
| ٹیلی میٹری | Torii `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` کو رجسٹرار ہینڈلرز (فلٹر `scheme="norito_rpc"`) کے لئے بے نقاب کرتا ہے۔ API `sns_registrar_status_total{result, suffix_id}` میں بھی اضافہ کرتا ہے۔ |

## 2۔ ڈی ٹی او ایس کا جائزہ

فیلڈز [`registry-schema.md`] (./registry-schema.md) میں بیان کردہ کیننیکل ڈھانچے کا حوالہ دیتے ہیں۔ مبہم روٹنگ سے بچنے کے لئے تمام پے لوڈ `NameSelectorV1` + `SuffixId` کو مربوط کرتے ہیں۔

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
| --------- | --------- | --------- | ---------- |
| `/v1/sns/registrations` | پوسٹ | `RegisterNameRequestV1` | ایک نام محفوظ کریں یا دوبارہ کھولیں۔ قیمت کے درجے کو حل کرتا ہے ، ادائیگی/گورننس کے ثبوتوں کی توثیق کرتا ہے ، رجسٹری کے واقعات کا اخراج کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/renew` | پوسٹ | `RenewNameRequestV1` | اصطلاح میں توسیع کریں۔ پالیسی سے فضل/چھٹکارا ونڈوز کا اطلاق کریں۔ |
| `/v1/sns/registrations/{selector}/transfer` | پوسٹ | `TransferNameRequestV1` | گورننس کی منظوریوں سے منسلک ہونے کے بعد ملکیت کی منتقلی۔ |
| `/v1/sns/registrations/{selector}/controllers` | put | `UpdateControllersRequestV1` | تمام کنٹرولرز کی جگہ لیتا ہے۔ دستخط شدہ اکاؤنٹ کے پتے کی توثیق کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | پوسٹ | `FreezeNameRequestV1` | گارڈین/کونسل کو منجمد کریں۔ ایک ٹکٹ گارڈین اور گورننس فائل کا حوالہ درکار ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | حذف کریں | `GovernanceHookV1` | تدارک کے بعد غیر منقولہ ؛ اس بات کو یقینی بناتا ہے کہ کونسل کا اوور رائڈ ریکارڈ کیا گیا ہے۔ |
| `/v1/sns/reserved/{selector}` | پوسٹ | `ReservedAssignmentRequestV1` | اسٹیورڈ/کونسل کے ذریعہ محفوظ ناموں کی تفویض۔ |
| `/v1/sns/policies/{suffix_id}` | حاصل کریں | - | موجودہ `SuffixPolicyV1` (کیچ ایبل) کو بازیافت کرتا ہے۔ |
| `/v1/sns/registrations/{selector}` | حاصل کریں | - | موجودہ `NameRecordV1` + موثر حالت (فعال ، فضل ، وغیرہ) لوٹاتا ہے۔ |** سلیکٹر انکوڈنگ: ** طبقہ `{selector}` IDR-5 کے مطابق I105 ، کمپریس ، یا کیننیکل ہیکس قبول کرتا ہے۔ Torii اسے `NameSelectorV1` کے ذریعے معمول بناتا ہے۔

** غلطی کا نمونہ: ** تمام اختتامی نکات Norito ، `message` ، `details` کے ساتھ Norito JSON واپس کریں گے۔ کوڈز میں `sns_err_reserved` ، `sns_err_payment_mismatch` ، `sns_err_policy_violation` ، `sns_err_governance_missing` شامل ہیں۔

### 3.1 سی ایل آئی مددگار (N0 دستی رجسٹرار کی ضرورت)

بند بیٹا اسٹیورڈز اب JSON کو ہاتھ سے تیار کیے بغیر CLI کے ذریعے رجسٹرار کا استعمال کرسکتے ہیں:

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

- `--owner` CLI کنفیگریشن اکاؤنٹ میں ڈیفالٹس ؛ اضافی کنٹرولر اکاؤنٹس (پہلے سے طے شدہ `[owner]`) شامل کرنے کے لئے `--controller` کو دہرائیں۔
- ان لائن ادائیگی کے جھنڈے براہ راست `PaymentProofV1` پر نقشہ ؛ جب آپ کے پاس پہلے سے ہی ساختی رسید ہو تو `--payment-json PATH` پاس کریں۔ میٹا ڈیٹا (`--metadata-json`) اور گورننس ہکس (`--governance-json`) اسی طرز پر عمل کریں۔

صرف پڑھنے والے مددگاروں کو مکمل تکرار:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

عمل درآمد کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں ؛ کمانڈز اس دستاویز میں بیان کردہ Norito DTOs کو دوبارہ استعمال کرتے ہیں تاکہ CLI آؤٹ پٹ Torii جوابات بائٹ کے لئے بائٹ سے میل کھاتا ہے۔

اضافی مدد میں تجدیدات ، منتقلی اور سرپرست اقدامات کا احاطہ کیا گیا ہے:

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
  --new-owner i105... \
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

`--governance-json` میں ایک درست `GovernanceHookV1` ریکارڈ (تجویز ID ، ووٹ ہیشس ، اسٹیورڈ/سرپرست دستخط) پر مشتمل ہونا ضروری ہے۔ ہر کمانڈ آسانی سے متعلقہ `/v1/sns/registrations/{selector}/...` اختتامی نقطہ کی عکاسی کرتا ہے تاکہ بیٹا آپریٹرز Torii سطحوں کو بالکل دہرا سکیں جس پر SDKs کال کریں گے۔

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

تار فارمیٹ: مرتب وقت میں اسکیما Norito کی ہیش میں بچت ہوتی ہے
`fixtures/norito_rpc/schema_hashes.json` (لائنز `RegisterNameRequestV1` ،
`RegisterNameResponseV1` ، `NameRecordV1` ، وغیرہ)۔

## 5. گورننس ہکس اور ثبوت

ہر کال جو ریاست میں ترمیم کرتی ہے اسے ری پلے کے لئے دوبارہ قابل استعمال ثبوتوں سے جوڑنا ہوگا:

| ایکشن | گورننس ڈیٹا کی ضرورت ہے |
| -------- | ------------------------------------ |
| معیاری رجسٹریشن/تجدید | تصفیہ کی ہدایت کا حوالہ دیتے ہوئے ادائیگی کا ثبوت۔ کونسل کے کسی ووٹ کی ضرورت نہیں جب تک کہ درجے کو اسٹیورڈ کی منظوری کی ضرورت نہ ہو۔ |
| پریمیم درجے کی رجسٹریشن / محفوظ اسائنمنٹ | `GovernanceHookV1` حوالہ تجویز ID + اسٹیورڈ اعتراف۔ |
| منتقلی | ووٹ کونسل ہیش + داؤ سگنل ہیش ؛ کلیئرنس گارڈین جب تنازعہ کے حل کے ذریعہ منتقلی کو متحرک کیا جاتا ہے۔ |
| منجمد/غیر منقولہ | کونسل کے گارڈین کے علاوہ اوور رائڈ ٹکٹ (غیر منقولہ) پر دستخط کرنا۔ |

Torii جانچ پڑتال کرکے شواہد کی تصدیق کرتا ہے:

1. تجویز شناخت کنندہ گورننس لیجر (`/v1/governance/proposals/{id}`) میں موجود ہے اور حیثیت `Approved` ہے۔
2. ہیش ریکارڈ شدہ ووٹنگ نمونے کے مطابق ہے۔
3. اسٹیورڈ/گارڈین کے دستخط `SuffixPolicyV1` کی متوقع عوامی چابیاں کا حوالہ دیتے ہیں۔

ناکام چیک واپس `sns_err_governance_missing`۔

## 6. ورک فلو کی مثالیں### 6.1 معیاری رجسٹریشن

1. کلائنٹ نے قیمتوں ، فضل اور دستیاب تیسری پارٹیوں کو بازیافت کرنے کے لئے `/v1/sns/policies/{suffix_id}` سے استفسار کیا ہے۔
2. کسٹمر `RegisterNameRequestV1` تعمیر کرتا ہے:
   - `selector` I105 لیبل (ترجیحی) یا کمپریس (دوسری پسند) سے اخذ کرتا ہے۔
   - پالیسی کی حدود میں `term_years`۔
   - `payment` خزانہ/اسٹیورڈ اسپلٹر کی منتقلی کا حوالہ دیتے ہوئے۔
3. درست Torii:
   - لیبل + محفوظ فہرست کو معیاری بنانا۔
   - اصطلاح/مجموعی قیمت بمقابلہ `PriceTierV1`۔
   - ادائیگی کے ثبوت کی رقم> = حساب شدہ قیمت + فیس۔
4. کامیابی میں Torii:
   - `NameRecordV1` پر برقرار ہے۔
   - `RegistryEventV1::NameRegistered` کو جاری کرتا ہے۔
   - `RevenueAccrualEventV1` کو جاری کرتا ہے۔
   - نیا ریکارڈ + واقعات واپس کرتا ہے۔

### 6.2 فضل کے دوران تجدید

فضل کے دوران تجدیدات میں معیاری درخواست کے علاوہ جرمانے کا پتہ لگانا بھی شامل ہے:

- Torii `now` بمقابلہ `grace_expires_at` کا موازنہ کرتا ہے اور `SuffixPolicyV1` کے اوورلوڈ ٹیبلز کو شامل کرتا ہے۔
- ادائیگی کے ثبوت کو سرچارج کا احاطہ کرنا چاہئے۔ ناکام => `sns_err_payment_mismatch`۔
- `RegistryEventV1::NameRenewed` نئے `expires_at` کو رجسٹر کرتا ہے۔

### 6.3 فریج گارڈین اور کونسل اوور رائڈ

1. گارڈین نے `FreezeNameRequestV1` کو ایک ٹکٹ کے ساتھ پیش کیا جس میں واقعہ کی شناخت کا حوالہ دیا گیا ہے۔
2. Torii ریکارڈ `NameStatus::Frozen` میں منتقل کرتا ہے ، آؤٹ پٹ `NameFrozen`۔
3. علاج کے بعد ، کونسل ایک اوور رائڈ جاری کرتی ہے۔ آپریٹر `/v1/sns/registrations/{selector}/freeze` کو `GovernanceHookV1` کے ساتھ حذف کرتا ہے۔
4. Torii اوور رائڈ کی توثیق کرتا ہے ، `NameUnfrozen` کو خارج کرتا ہے۔

## 7. توثیق اور غلطی کے کوڈ

| کوڈ | تفصیل | HTTP |
| ------ | ------------- | ------ |
| `sns_err_reserved` | لیبل محفوظ یا مسدود ہے۔ | 409 |
| `sns_err_policy_violation` | اصطلاح ، درجے یا کنٹرولرز کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | ادائیگی کے ثبوت میں قدر یا اثاثہ کی مماثلت۔ | 402 |
| `sns_err_governance_missing` | مطلوبہ گورننس نمونے لاپتہ/غلط۔ | 403 |
| `sns_err_state_conflict` | موجودہ لائف سائیکل اسٹیٹ میں آپریشن کی اجازت نہیں ہے۔ | 409 |

تمام کوڈ `X-Iroha-Error-Code` کے ذریعے ظاہر ہوتے ہیں اور Norito JSON/NRPC لفافوں کے ذریعے۔

## 8. نفاذ کے نوٹ

- Torii نے `NameRecordV1.auction` کے طور پر نیلامی کا شکار کیا ہے اور براہ راست محفوظ کرنے کی کوششوں کو `PendingAuction` کے طور پر مسترد کردیا ہے۔
- ادائیگی کے ثبوت لیجر Norito سے رسیدوں کو دوبارہ استعمال کرتے ہیں۔ ٹریژری سروسز مددگار APIs (`/v1/finance/sns/payments`) مہیا کرتی ہے۔
- SDKs کو ان اختتامی نکات کو مضبوطی سے ٹائپ شدہ مددگاروں سے لپیٹنا چاہئے تاکہ بٹوے واضح غلطی کی وجوہات (`ERR_SNS_RESERVED` ، وغیرہ) پیش کرسکیں۔

## 9. اگلے اقدامات

- ایک بار SN-3 نیلامی دستیاب ہونے کے بعد Torii ہینڈلرز کو اصل رجسٹری معاہدے سے لنک کریں۔
- اس API کا حوالہ دیتے ہوئے مخصوص SDK گائیڈز (مورچا/JS/Swift) شائع کریں۔
- گورننس ہکس پروف فیلڈز میں کراس لنکس کے ساتھ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) کو بڑھائیں۔