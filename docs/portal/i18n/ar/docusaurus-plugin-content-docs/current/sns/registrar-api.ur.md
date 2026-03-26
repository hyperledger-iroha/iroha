---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة مستند ماخذ
هذه الصفحة `docs/source/sns/registrar_api.md` عنوان المقالة والبريد الإلكتروني
هذا هو العنوان الإلكتروني. لقد ترجمت سورس فاميلي العلاقات العامة إلى قرار جديد.
:::

# واجهة برمجة تطبيقات مسجل SNS وخطافات الاتصال (SN-2b)

**الحالة:** 2026-03-24 مسودہ - Nexus Core ريویو تحت  
**روم لنك:** SN-2b "واجهة برمجة تطبيقات المسجل وخطافات الإدارة"  
**الشريط الشريطي:** سعر معقول [`registry-schema.md`](./registry-schema.md).

هذه نقطة النهاية Torii، وخدمات gRPC، وطلب/استجابة DTOs والحوكمة
المصنوعات اليدوية وضاحية كرتا هو مسجل خدمة أسماء Sora (SNS) چلانے کے لئے
دركار . تم إنشاء حزم SDK والمحافظ والأتمتة من خلال تطبيق SNS الأصلي
سجل، قم بتجديد أو إدارة كل ما تحتاجه.

## 1. ٹرانسبورت ووثيقة| شرط | تفاصيل |
|-----|-------|
| پروٹوكولز | REST `/v1/sns/*` تحت خدمة gRPC `sns.v1.Registrar`. Norito-JSON (`application/json`) و Norito-RPC باير (`application/x-norito`) يوافق على ذلك. |
| مصادقة | رموز `Authorization: Bearer` أو شهادات mTLS هي لاحقة مضيفة. نقاط النهاية الحساسة للحوكمة (التجميد/إلغاء التجميد، التعيينات المحجوزة) `scope=sns.admin` مطلوب. |
| ریٹ حدود | المسجلون `torii.preauth_scheme_limits` مجموعات المتصلين JSON التي يتم تسجيلها بشكل متكرر ولاحقة لأحرف استهلالية متقطعة: `sns.register`، `sns.renew`، `sns.controller`، `sns.freeze`۔ |
| ٹيليميٹری | Torii معالجات المسجل `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` ظاہر کرتا ہے (مرشح `scheme="norito_rpc"`)؛ API موجود أيضًا `sns_registrar_status_total{result, suffix_id}`. |

## 2.خلاصة DTO

فيلڈز [`registry-schema.md`](./registry-schema.md) استمتع بالبنيات الأساسية التي تشير إليها. يشمل كل الحمولات `NameSelectorV1` + `SuffixId` بطاقة توجيه شاملة.

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

## 3. نقاط النهاية REST| نقطة النهاية | طريقی | الحمولة | تفاصيل |
|----------|-------|---------|-------|
| `/v1/sns/names` | مشاركة | `RegisterNameRequestV1` | اسم السجل أو تكرار. حل طبقة التسعير هو إثبات الدفع/الحوكمة، وتصدر أحداث التسجيل هذه البطاقة. |
| `/v1/sns/names/{namespace}/{literal}/renew` | مشاركة | `RenewNameRequestV1` | مدت بڑهاتا ہے۔ نوافذ النعمة/الفداء في باليس هي نافعة للكرتا. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | مشاركة | `TransferNameRequestV1` | الموافقات الحكومرانية يتم الحصول عليها بعد انتقال الملكية. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ضع | `UpdateControllersRequestV1` | وحدات تحكم سيلتا بلتا؛ عناوين الحساب الموقعة کی توثیق کرتا ہے۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | مشاركة | `FreezeNameRequestV1` | تجميد الوصي/المجلس ۔ تذكرة الوصي وجدول الحوكمة حوالة درکار۔ |
| `/v1/sns/names/{namespace}/{literal}/freeze` | حذف | `GovernanceHookV1` | علاج بعد التجميد؛ لقد أدى تجاوز المجلس إلى زيادة كبيرة في عدد التسجيلات. |
| `/v1/sns/reserved/{selector}` | مشاركة | `ReservedAssignmentRequestV1` | الأسماء المحجوزة هي المضيف/المجلس كمهمة مهمة. |
| `/v1/sns/policies/{suffix_id}` | احصل على | -- | `SuffixPolicyV1` موجود ومتاح (قابل للتخزين المؤقت). |
| `/v1/sns/names/{namespace}/{literal}` | احصل على | -- | موجود `NameRecordV1` + أكثر حالة (نشط، Grace وغيره) واپس کرتا. |

**ترميز المحدد:** `{selector}` مقطع المسار i105، مضغوط (`sora`) أو سداسي عشري أساسي ADDR-5 کے مطابق مقبول کرتا ہے؛ Torii `NameSelectorV1` تطبيع الكرتا.**نموذج الخطأ:** نقاط النهاية الكاملة Norito JSON `code`, `message`, `details`. تتضمن الرموز `sns_err_reserved` و`sns_err_payment_mismatch` و`sns_err_policy_violation` و`sns_err_governance_missing`.

### 3.1 مساعدي CLI (N0 دستی المسجل ضروري)

يستخدم مضيفو الإصدار التجريبي المغلق CLI استخدام المسجل لـ JSON:

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

- `--owner` بحساب تكوين ڈيفالٹ CLI؛ يتم تعيين حسابات وحدة التحكم الإضافية إلى `--controller` (الافتراضي `[owner]`).
- علامات الدفع المضمنة لخريطة `PaymentProofV1`؛ يجب أن يكون الإيصال المنظم `--payment-json PATH`. يتم أيضًا استخدام البيانات الوصفية (`--metadata-json`) وخطافات الإدارة (`--governance-json`).

تدريبات المساعدين للقراءة فقط هي مكملة:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

تم تنفيذه بواسطة `crates/iroha_cli/src/commands/sns.rs`؛ يتم استخدام الأوامر بشكل متكرر من خلال Norito DTOs بالإضافة إلى إخراج CLI لاستجابات Torii التي تزيد من عدد البايتات مقابل البايتات.

تجديدات المساعدين الإضافيين، والتحويلات، وإجراءات الوصي:

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
  --new-owner <katakana-i105-account-id> \
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

`--governance-json` صحيح `GovernanceHookV1` تسجيل الدخول (معرف الاقتراح، تجزئات التصويت، توقيعات المضيف/الوصي). هناك علاقة بين نقطة نهاية `/v1/sns/names/{namespace}/{literal}/...` ونقطة النهاية التي يتدرب فيها مشغلو بيتا وأسطح Torii على أدوات تطوير البرامج (SDK) الجديدة.

## 4. خدمة gRPC

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
```تنسيق السلك: تجزئة المخطط Norito في وقت الترجمة
`fixtures/norito_rpc/schema_hashes.json` درج درج (الصفوف `RegisterNameRequestV1`،
`RegisterNameResponseV1`، `NameRecordV1`، إلخ).

## 5. خطافات الحوكمة والأدلة

قم بتغيير المكالمة وإعادة التشغيل للحصول على دليل منسلك كرانا:

| العمل | بيانات الحوكمة المطلوبة |
|--------|-------------------------|
| التسجيل القياسي/التجديد | تعليمات التسوية کو الرجوع إلى إثبات الدفع؛ تصويت المجلس ليس ضروريا طالما أن موافقة المضيف ليست ضرورية. |
| تسجيل الطبقة المميزة / المهمة المحجوزة | `GovernanceHookV1` معرف مقترح جو + إقرار المضيف راجع کرتا ہے۔ |
| نقل | تجزئة تصويت المجلس + تجزئة إشارة DAO؛ تصريح الوصي جب نقل حل النزاعات سے الزناد ہو۔ |
| تجميد/إلغاء التجميد | توقيع تذكرة الوصي کے ساتھ تجاوز المجلس (إلغاء التجميد)۔ |

Torii البراهين التي تم إنشاؤها من قبل:

1. دفتر الأستاذ لإدارة معرف الاقتراح (`/v1/governance/proposals/{id}`) موجود وحالته `Approved` .
2. يتم تسجيل التجزئات من خلال تطابق عناصر التصويت مع البطاقة.
3. توقيعات المضيف/الوصي `SuffixPolicyV1` ستؤدي إلى المفاتيح العامة للإشارة إلى البطاقة.

الشيكات الفاشلة `sns_err_governance_missing` بطاقة الائتمان.

## 6. أمثلة على سير العمل

### 6.1 التسجيل القياسي1. العميل `/v1/sns/policies/{suffix_id}` الذي يمكنه الاستعلام عن السعر والتسعير والنعمة ومستويات الأجهزة متاح.
2. العميل `RegisterNameRequestV1` بناتا:
   - `selector` ترجیحی i105 أو ثاني أفضل تسمية مضغوطة (`sora`) سے مشتقة ہے۔
   - `term_years` باليسى حدود میں۔
   - `payment` نقل الخزينة/المضيف المقسم للإشارة إلى کرتا ہے۔
3. Torii التحقق من صحة البطاقة:
   - تطبيع التسمية + القائمة المحجوزة۔
   - المصطلح/السعر الإجمالي مقابل `PriceTierV1`.
   - مبلغ إثبات الدفع >= السعر المحسوب + الرسوم۔
4. الكاميرا Torii:
   - `NameRecordV1` محفوظ كرتا ہے۔
   - `RegistryEventV1::NameRegistered` تنبعث منها كرتا ہے۔
   - `RevenueAccrualEventV1` تنبعث منها كرتا ہے۔
   - سجل جديد + أحداث واپس كرتا ہے۔

### 6.2 التجديد أثناء فترة السماح

تتضمن تجديدات النعمة طلبًا قياسيًا يتم من خلاله اكتشاف العقوبة، وتتضمن ما يلي:

- Torii `now` vs `grace_expires_at` جداول الرسوم الإضافية و`SuffixPolicyV1` تتضمن جداول الرسوم الإضافية.
- إثبات الدفع وتغطية الرسوم الإضافية. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` جديد `expires_at` تسجيل الدخول .

### 6.3 تجميد الوصي وتجاوز المجلس1. Guardian `FreezeNameRequestV1` أرسل بطاقة وتذكرة معرّف الحادث کا حوالة دینے وتذكرة ہہے.
2. سجل Torii الذي ينتقل إلى `NameStatus::Frozen`، `NameFrozen` ينبعث من الكرتا.
3. الإصلاح بعد تجاوز المجلس للقرار؛ عامل التشغيل DELETE `/v1/sns/names/{namespace}/{literal}/freeze` أو `GovernanceHookV1` وهو ما حدث بالفعل.
4. Torii تجاوز التحقق من صحة البطاقة، `NameUnfrozen` ينبعث منها بطاقة.

## 7. رموز التحقق والخطأ

| الكود | الوصف | HTTP |
|------|------------|------|
| `sns_err_reserved` | التسمية محجوزة أو محظورة ہے۔ | 409 |
| `sns_err_policy_violation` | المصطلح أو الطبقة أو وحدات التحكم في سيت باليس هي تناقضات كرتا. | 422 |
| `sns_err_payment_mismatch` | قيمة إثبات الدفع أو عدم تطابق الأصول. | 402 |
| `sns_err_governance_missing` | عناصر الحكم المطلوبة غائب/غير صالحة ہیں۔ | 403 |
| `sns_err_state_conflict` | حالة دورة الحياة المتاحة غير مسموح بها. | 409 |

رموز تمام `X-Iroha-Error-Code` ومغلفات Norito JSON/NRPC المهيكلة ذات سطح ظاهر.

## 8. ملاحظات التنفيذ

- Torii المزادات المعلقة التي `NameRecordV1.auction` تم رفضها و `PendingAuction` خلال محاولات التسجيل المباشر ورفضت كرتا .
- إثباتات الدفع Norito إيصالات دفتر الأستاذ المستخدمة مرة أخرى؛ واجهات برمجة التطبيقات (API) المساعدة لخدمات الخزانة (`/v1/finance/sns/payments`)
- SDKs هي نقاط نهاية ومساعدات مكتوبة بقوة، التفاف المحفظة توضح أسباب الخطأ (`ERR_SNS_RESERVED`، وغیرہ) دکھا سکیں.## 9. الخطوات التالية

- مزادات SN-3 التي تتبع معالجات Torii هي عقد التسجيل الأصلي عبر الأسلاك.
- الأدلة الخاصة بـ SDK (Rust/JS/Swift) شائعة الاستخدام في API للرجوع إليها.
- [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) وحقول الأدلة المرتبطة بالحوكمة والتي تمتد عبر الروابط المستمرة.