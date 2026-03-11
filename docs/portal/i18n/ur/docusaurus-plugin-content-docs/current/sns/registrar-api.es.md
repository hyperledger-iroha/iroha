---
lang: ur
direction: rtl
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: نوٹ کینونیکل ماخذ
یہ صفحہ `docs/source/sns/registrar_api.md` کی عکاسی کرتا ہے اور اب اس کے طور پر کام کرتا ہے
پورٹل کی کیننیکل کاپی۔ ماخذ فائل کو ندیوں کے لئے برقرار رکھا گیا ہے۔
ترجمہ
:::

# SNS لاگر API اور گورننس ہکس (SN-2B)

** حیثیت: ** مسودہ 2026-03-24-Nexus کور کے جائزہ کے تحت  
** روڈ میپ لنک: ** SN-2B "API اور گورننس ہکس رجسٹر کریں"  
** شرائط: ** اسکیما تعریفیں [`registry-schema.md`] (./registry-schema.md) میں)

اس نوٹ میں Torii اختتامی مقامات ، GRPC خدمات ، درخواست DTOS کی وضاحت کی گئی ہے
ردعمل اور گورننس نمونے کے لئے ضروری ہے
سورہ نام کی خدمت (ایس این ایس)۔ یہ ایس ڈی کے ، بٹوے اور کے لئے مستند معاہدہ ہے
آٹومیشن جس کو ایس این ایس کے ناموں کو رجسٹر ، تجدید یا انتظام کرنے کی ضرورت ہے۔

## 1. نقل و حمل اور توثیق

| ضرورت | تفصیل |
| ----------- | --------- |
| پروٹوکول | `/v1/sns/*` اور GRPC سروس `sns.v1.Registrar` کے تحت آرام کریں۔ دونوں Norito-JSON (`application/json`) اور بائنری Norito-RPC (`application/x-norito`) کو قبول کرتے ہیں۔ |
| Auth | `Authorization: Bearer` ٹوکن یا MTLS سرٹیفکیٹ لاحقہ اسٹیورڈ کے ذریعہ جاری کردہ۔ گورننس سے حساس اختتامی مقامات (منجمد/غیر منقولہ ، محفوظ اسائنمنٹس) کے لئے `scope=sns.admin` کی ضرورت ہوتی ہے۔ |
| شرح کی حد | رجسٹرار شیئر بالٹی `torii.preauth_scheme_limits` JSON کالرز کے ساتھ ساتھ پھٹ جانے والی حدود کے ساتھ ہر لاحقہ: `sns.register` ، `sns.renew` ، `sns.controller` ، `sns.freeze`۔ |
| ٹیلی میٹری | Torii `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` کو لاگر ہینڈلرز (فلٹر `scheme="norito_rpc"`) سے بے نقاب کرتا ہے۔ API `sns_registrar_status_total{result, suffix_id}` میں بھی اضافہ کرتا ہے۔ |

## 2۔ ڈی ٹی او کا خلاصہ

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
| ---------- | -------- | --------- | ------------- |
| `/v1/sns/registrations` | پوسٹ | `RegisterNameRequestV1` | کسی نام کو رجسٹر کریں یا دوبارہ کھولیں۔ قیمتوں کا تعین کرنے والے درجے کو حل کرتا ہے ، ادائیگی/گورننس کے ثبوتوں کی توثیق کرتا ہے ، لاگنگ کے واقعات کا اخراج کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/renew` | پوسٹ | `RenewNameRequestV1` | توسیع کی مدت. پالیسی کے مطابق فضل/چھٹکارا ونڈوز کا اطلاق کریں۔ |
| `/v1/sns/registrations/{selector}/transfer` | پوسٹ | `TransferNameRequestV1` | گورننس کی منظوریوں سے منسلک ہونے کے بعد ملکیت کی منتقلی۔ |
| `/v1/sns/registrations/{selector}/controllers` | put | `UpdateControllersRequestV1` | کنٹرولرز کے سیٹ کی جگہ لے لیتا ہے۔ دستخط شدہ اکاؤنٹ کے پتے کی توثیق کرتا ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | پوسٹ | `FreezeNameRequestV1` | گارڈین/کونسل منجمد۔ گارڈین کے ٹکٹ اور گورننس ڈاکٹ کے حوالے کی ضرورت ہے۔ |
| `/v1/sns/registrations/{selector}/freeze` | حذف کریں | `GovernanceHookV1` | تدارک کے بعد غیر منقولہ ؛ کونسل کے رجسٹرڈ کو اوور رائڈ کو یقینی بناتا ہے۔ |
| `/v1/sns/reserved/{selector}` | پوسٹ | `ReservedAssignmentRequestV1` | اسٹیورڈ/کونسل کے ذریعہ محفوظ ناموں کی تفویض۔ |
| `/v1/sns/policies/{suffix_id}` | حاصل کریں | - | موجودہ `SuffixPolicyV1` (کیچ ایبل) ملتا ہے۔ |
| `/v1/sns/registrations/{selector}` | حاصل کریں | - | موجودہ `NameRecordV1` + موثر حالت (فعال ، فضل ، وغیرہ) لوٹاتا ہے۔ |** سلیکٹر انکوڈنگ: ** طبقہ `{selector}` I105 ، ADDR-5 کے مطابق کمپریسڈ یا کیننیکل ہیکس قبول کرتا ہے۔ Torii اسے `NameSelectorV1` کے ذریعے معمول بناتا ہے۔

** غلطی کا نمونہ: ** تمام اختتامی نکات Norito ، `message` ، `details` کے ساتھ Norito JSON واپس کریں گے۔ کوڈز میں `sns_err_reserved` ، `sns_err_payment_mismatch` ، `sns_err_policy_violation` ، `sns_err_governance_missing` شامل ہیں۔

### 3.1 مددگار CLI (N0 دستی لاگر کی ضرورت)

بند بیٹا اسٹیورڈز JSON کو ہاتھ سے جمع کیے بغیر سی ایل آئی کے ذریعے لاگر چل سکتے ہیں:

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

- `--owner` CLI کنفیگریشن اکاؤنٹ میں ڈیفالٹس ؛ اضافی کنٹرولر اکاؤنٹس (ڈیفالٹ `[owner]`) منسلک کرنے کے لئے `--controller` کو دہرائیں۔
- ادائیگی ان لائن پرچموں کا نقشہ براہ راست `PaymentProofV1` پر ہے۔ جب آپ کے پاس پہلے سے ہی ساختی رسید ہوتی ہے تو `--payment-json PATH` استعمال کریں۔ میٹا ڈیٹا (`--metadata-json`) اور گورننس ہکس (`--governance-json`) اسی طرز پر عمل کریں۔

صرف پڑھنے والے مددگار مضامین کو مکمل کرتے ہیں:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

عمل درآمد کے لئے `crates/iroha_cli/src/commands/sns.rs` دیکھیں ؛ کمانڈز اس دستاویز میں بیان کردہ Norito DTOs کو دوبارہ استعمال کرتے ہیں تاکہ CLI آؤٹ پٹ بائٹ کے لئے بائٹ کے لئے Torii کے ردعمل سے میل کھاتا ہے۔

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

`--governance-json` میں ایک درست `GovernanceHookV1` ریکارڈ (تجویز ID ، ووٹ ہیشس ، اسٹیورڈ/سرپرست دستخط) پر مشتمل ہونا ضروری ہے۔ ہر کمانڈ آسانی سے اسی طرح کے `/v1/sns/registrations/{selector}/...` اختتامی نقطہ کی آئینہ دار ہے تاکہ بیٹا آپریٹرز بالکل جانچ سکیں جس میں Torii SDKs کو کال کرے گا۔

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

تار فارمیٹ: ہیش آف اسکیما Norito میں مرتب شدہ وقت میں ریکارڈ کیا گیا
`fixtures/norito_rpc/schema_hashes.json` (قطار `RegisterNameRequestV1` ،
`RegisterNameResponseV1` ، `NameRecordV1` ، وغیرہ)۔

## 5. گورننس اور شواہد ہکس

ہر کال جو ریاست کو تبدیل کرتی ہے اسے ری پلے کے لئے موزوں شواہد سے منسلک کرنا ہوگا:

| ایکشن | گورننس ڈیٹا کی ضرورت ہے |
| -------- | ---------------------------- |
| معیاری رجسٹریشن/تجدید | تصفیہ کی ہدایت کا حوالہ دیتے ہوئے ادائیگی کا ثبوت۔ کونسل کے کسی ووٹ کی ضرورت نہیں ہے جب تک کہ درجے کو اسٹیورڈ کی منظوری کی ضرورت نہ ہو۔ |
| پریمیم رجسٹریشن / محفوظ اسائنمنٹ | `GovernanceHookV1` کون سا حوالہ تجویز ID + اسٹیورڈ اعتراف۔ |
| منتقلی | کونسل کے ووٹ ہیش + داؤ ٹوکن ہیش ؛ گارڈین کلیئرنس جب تنازعہ کے حل کے ذریعہ منتقلی کو چالو کیا جاتا ہے۔ |
| منجمد/غیر منقولہ | گارڈین ٹکٹ کے علاوہ کونسل کے اوور رائڈ (غیر منقولہ) پر دستخط کرنا۔ |

Torii جانچ پڑتال کرکے ٹیسٹوں کی تصدیق کرتا ہے:

1. پروپوزل ID گورننس لیجر (`/v1/governance/proposals/{id}`) میں موجود ہے اور حیثیت `Approved` ہے۔
2. ہیش رجسٹرڈ ووٹنگ نمونے سے ملتے ہیں۔
3. اسٹیورڈ/گارڈین کے دستخط `SuffixPolicyV1` کی متوقع عوامی چابیاں کا حوالہ دیتے ہیں۔

ناکامی `sns_err_governance_missing` کی واپسی کرتی ہے۔

## 6. بہاؤ کی مثالیں

### 6.1 معیاری رجسٹریشن1. قیمتیں ، فضل اور دستیاب درجے حاصل کرنے کے لئے کسٹمر `/v1/sns/policies/{suffix_id}` سے مشورہ کرتا ہے۔
2. کلائنٹ اسلحہ `RegisterNameRequestV1`:
   - `selector` لیبل I105 (ترجیحی) یا کمپریسڈ (دوسرا بہترین آپشن) سے ماخوذ ہے۔
   - `term_years` پالیسی کی حدود میں۔
   - `payment` جو خزانے/اسٹیورڈ اسپلٹر کی منتقلی کا حوالہ دیتا ہے۔
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

فضل کی تجدید میں معیاری درخواست کے علاوہ جرمانے کا پتہ لگانا بھی شامل ہے:

- Torii `now` بمقابلہ `grace_expires_at` کا موازنہ کرتا ہے اور `SuffixPolicyV1` سے سرچارج ٹیبل شامل کرتا ہے۔
- ادائیگی کے ثبوت کو سرچارج کا احاطہ کرنا چاہئے۔ ناکامی => `sns_err_payment_mismatch`۔
- `RegistryEventV1::NameRenewed` نئے `expires_at` کو رجسٹر کرتا ہے۔

### 6.3 گارڈین منجمد اور کونسل اوور رائڈ

1. گارڈین `FreezeNameRequestV1` کو ٹکٹ کے ساتھ بھیجتا ہے جو واقعہ کی شناخت کا حوالہ دیتا ہے۔
2. Torii رجسٹر کو `NameStatus::Frozen` میں منتقل کرتا ہے ، آؤٹ پٹس `NameFrozen`۔
3. تدارک کے بعد ، کونسل کو زیربحث لایا جاتا ہے۔ آپریٹر `/v1/sns/registrations/{selector}/freeze` کو `GovernanceHookV1` کے ساتھ حذف کرتا ہے۔
4. Torii اوور رائڈ ، ایشوز `NameUnfrozen` کی توثیق کرتا ہے۔

## 7. توثیق اور غلطی کے کوڈ

| کوڈ | تفصیل | HTTP |
| -------- | ------------- | ------ |
| `sns_err_reserved` | لیبل محفوظ یا مسدود ہے۔ | 409 |
| `sns_err_policy_violation` | اصطلاح ، درجے یا کنٹرولرز کا سیٹ پالیسی کی خلاف ورزی کرتا ہے۔ | 422 |
| `sns_err_payment_mismatch` | ادائیگی کے ثبوت میں قدر یا اثاثہ کی مماثلت۔ | 402 |
| `sns_err_governance_missing` | غیر حاضر/غلط مطلوبہ گورننس نمونے۔ | 403 |
| `sns_err_state_conflict` | زندگی کے چکر کی موجودہ حالت میں آپریشن کی اجازت نہیں ہے۔ | 409 |

تمام کوڈز `X-Iroha-Error-Code` اور Norito ساختی JSON/NRPC لفافوں کے ذریعے آؤٹ پٹ ہیں۔

## 8. نفاذ کے نوٹ

- Torii `NameRecordV1.auction` میں زیر التوا نیلامی کی بچت کرتا ہے اور براہ راست رجسٹریشن کی کوششوں کو مسترد کرتا ہے جبکہ یہ `PendingAuction` میں ہے۔
- ادائیگی کے ثبوت لیجر Norito سے رسیدوں کو دوبارہ استعمال کرتے ہیں۔ ٹریژری سروسز مددگار APIs (`/v1/finance/sns/payments`) مہیا کرتی ہے۔
- SDKs کو ان اختتامی نکات کو ٹائپڈ مددگاروں سے لپیٹنا چاہئے تاکہ بٹوے واضح غلطی کی وجوہات (`ERR_SNS_RESERVED` ، وغیرہ) دکھائیں۔

## 9. اگلے اقدامات

- ایک بار SN-3 نیلامی کے پہنچنے کے بعد Torii ہینڈلرز کو اصل رجسٹریشن معاہدے سے مربوط کریں۔
- مخصوص ایس ڈی کے (زنگ/جے ایس/سوئفٹ) ہدایت نامہ شائع کریں جو اس API کا حوالہ دیتے ہیں۔
- گورننس ہک ثبوت کے شعبوں میں کراس لنکس کے ساتھ [`sns_suffix_governance_charter.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) میں توسیع کریں۔