---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
هذه الصفحة تعكس `docs/source/sns/registrar_api.md` والآن سيرف كما لا
نسخة من بوابة كانونيكا. يتم الحفاظ على الملف بشكل فعال من أجل التدفق
ترجمة.
:::

# API لمسجل SNS وخطافات التحكم (SN-2b)

**الحالة:** Borrador 2026-03-24 -- آخر مراجعة لـ Nexus Core  
**إدراج خريطة الطريق:** SN-2b "واجهة برمجة تطبيقات المسجل وخطافات الإدارة"  
**المتطلبات الأساسية:** تعريفات التحدي في [`registry-schema.md`](./registry-schema.md)

هذه الملاحظة المحددة لنقاط النهاية Torii، خدمات gRPC، DTOs للطلب
الاستجابة والمصنوعات اللازمة لإدارة المسجل
خدمة اسم سورا (SNS). إنه العقد التلقائي لحزم SDK والمحافظ وغيرها
الأتمتة التي تحتاجها المسجل أو تجديد أو إدارة أرقام SNS.

## 1. النقل والمصادقة| المتطلبات | التفاصيل |
|-----------|--------|
| البروتوكولات | REST bajo `/v1/sns/*` وخدمة gRPC `sns.v1.Registrar`. Ambos aceptan Norito-JSON (`application/json`) وNorito-RPC ثنائي (`application/x-norito`). |
| مصادقة | الرموز المميزة `Authorization: Bearer` أو شهادات mTLS الصادرة بواسطة مضيف لاحقة. تتطلب نقاط النهاية الحساسة للإدارة (التجميد/إلغاء التجميد والتخصيصات المحجوزة) `scope=sns.admin`. |
| حدود المهمة | يتشارك المسجلون في مجموعات `torii.preauth_scheme_limits` مع المتصلين JSON بحدود تعريف من خلال اللاحقة: `sns.register`، `sns.renew`، `sns.controller`، `sns.freeze`. |
| القياس عن بعد | Torii يعرض `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المسجل (مرشح `scheme="norito_rpc"`)؛ يتم زيادة واجهة برمجة التطبيقات أيضًا `sns_registrar_status_total{result, suffix_id}`. |

## 2. استئناف DTO

تشير الحقول إلى البنيات القانونية المحددة في [`registry-schema.md`](./registry-schema.md). تشتمل جميع الشحنات على `NameSelectorV1` + `SuffixId` لتجنب الغموض.

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

## 3. راحة نقاط النهاية| نقطة النهاية | الطريقة | الحمولة | الوصف |
|----------|-------|--------|-------------|
| `/v1/sns/names` | مشاركة | `RegisterNameRequestV1` | قم بالتسجيل أو إعادة إنشاء اسم. قم بإعادة تحديد مستوى السعر، وصلاحية الدفع/الإشراف، وإصدار أحداث التسجيل. |
| `/v1/sns/names/{namespace}/{literal}/renew` | مشاركة | `RenewNameRequestV1` | تمديد النهاية. Aplica ventanas de gracia/redencion segun la politica. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | مشاركة | `TransferNameRequestV1` | قم بنقل بعض الملحقات الإضافية لمتطلبات الإدارة. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ضع | `UpdateControllersRequestV1` | استبدال مجموعة وحدات التحكم؛ اتجاهات صالحة للحسابات التجارية. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | مشاركة | `FreezeNameRequestV1` | تجميد الوصي/المجلس. يتطلب تذكرة الوصي والرجوع إلى جدول أعمال الحكومة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | حذف | `GovernanceHookV1` | قم بإلغاء تجميد العلاج؛ asegura تجاوز مجلس التسجيل. |
| `/v1/sns/reserved/{selector}` | مشاركة | `ReservedAssignmentRequestV1` | تعيين الأسماء المحجوزة من قبل المضيف/المجلس. |
| `/v1/sns/policies/{suffix_id}` | احصل على | -- | احصل على `SuffixPolicyV1` الفعلي (قابل للتخزين المؤقت). |
| `/v1/sns/names/{namespace}/{literal}` | احصل على | -- | Devuelve `NameRecordV1` الفعلي + الحالة الفعالة (نشط، نعمة، وما إلى ذلك). |

** كود المحدد: ** الجزء `{selector}` يقبل i105، مدمج أو سداسي كانون ثاني ADDR-5؛ Torii يتم تطبيعه عبر `NameSelectorV1`.**نموذج الأخطاء:** جميع نقاط النهاية ترجع إلى Norito JSON مع `code`، `message`، `details`. تشتمل الرموز على `sns_err_reserved`، و`sns_err_payment_mismatch`، و`sns_err_policy_violation`، و`sns_err_governance_missing`.

### 3.1 واجهة سطر الأوامر للمساعدين (متطلبات دليل المسجل N0)

يمكن لمشرفي النسخة التجريبية المفتوحة تشغيل المسجل عبر CLI بدون تثبيت JSON يدويًا:

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

- `--owner` توما بسبب خلل في حساب تكوين CLI؛ كرر `--controller` لوحدات التحكم الإضافية الإضافية (`[owner]` الافتراضي).
- الأعلام المضمنة في خريطة الدفع مباشرة `PaymentProofV1`؛ usa `--payment-json PATH` عندما يكون لديك هيكل استلام. البيانات الوصفية (`--metadata-json`) وخطافات الإدارة (`--governance-json`) تتبع نفس المستفيد.

مساعدات للقراءة المنفردة لإكمال الدراسات:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

الإصدار `crates/iroha_cli/src/commands/sns.rs` للتنفيذ؛ أعاد القادة استخدام DTOs Norito الموصوفة في هذا المستند حتى يتزامن إخراج CLI بايت لكل بايت مع إجابة Torii.

مساعدات إضافية في عمليات التجديد والتحويلات وإجراءات الوصي:

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
````--governance-json` يجب أن يحتوي على سجل `GovernanceHookV1` صالح (معرف الاقتراح، تجزئات التصويت، شركة المضيف/الوصي). كل أمر يشير ببساطة إلى نقطة النهاية `/v1/sns/names/{namespace}/{literal}/...` التي تتوافق مع مشغلي الإصدار التجريبي من الأسطح Torii التي تتعرف على SDKs تمامًا.

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
```

تنسيق السلك: hash del esquema Norito في وقت التجميع المسجل في
`fixtures/norito_rpc/schema_hashes.json` (الفيلات `RegisterNameRequestV1`،
`RegisterNameResponseV1`، `NameRecordV1`، وما إلى ذلك).

## 5. خطافات الحكم والأدلة

يجب أن تكون كل مكالمة في هذه الحالة بمثابة دليل إضافي مناسب لإعادة التشغيل:

| أكسيون | بيانات الإدارة المطلوبة |
|--------|-----------------------------|
| التسجيل/التجديد القياسي | جرب الدفع الذي يشير إلى تعليمات التسوية; لا يتطلب الأمر تصويت المجلس على أن المستوى يتطلب موافقة المضيف. |
| التسجيل المميز / الحجز المسبق | `GovernanceHookV1` الذي يشير إلى معرف الاقتراح + إقرار المضيف. |
| نقل | تجزئة مجلس التصويت + تجزئة DAO؛ تصريح الوصي عندما يتم تفعيل النقل من أجل حل النزاع. |
| تجميد/إلغاء التجميد | شركة تذكرة الوصي ماس تتجاوز المجلس (إلغاء التجميد). |

Torii التحقق من اختبار المنتج:1. معرف الاقتراح موجود في دفتر الأستاذ الحكومي (`/v1/governance/proposals/{id}`) والحالة هي `Approved`.
2. تتزامن التجزئات مع عناصر التصويت المسجلة.
3. تشير شركة المضيف/الوصي إلى العناصر العامة المتوقعة في `SuffixPolicyV1`.

فالوس devuelven `sns_err_governance_missing`.

## 6. نماذج التدفق

### 6.1 معيار التسجيل

1. يستشير العميل `/v1/sns/policies/{suffix_id}` للحصول على أسعار معقولة ومستويات متاحة.
2. العميل آرما `RegisterNameRequestV1`:
   - `selector` مشتق من الملصق i105 (المفضل) أو المجمع (الخيار الأفضل الثاني).
   - `term_years` داخل حدود السياسة.
   - `payment` يشير إلى نقل وحدة التخزين/المضيف.
3. Torii صالحة:
   - تسوية التسمية + القائمة المحجوزة.
   - المصطلح/السعر الإجمالي مقابل `PriceTierV1`.
   - إثبات مبلغ الدفع >= السعر المحسوب + الرسوم.
4. عند الخروج Torii:
   - الثبات `NameRecordV1`.
   -إميت `RegistryEventV1::NameRegistered`.
   -إميت `RevenueAccrualEventV1`.
   - تطوير السجل الجديد + الأحداث.

### 6.2 تجديد دورانتي جراسيا

تتضمن التجديدات التي تتم بفضل اللطف معيارًا إضافيًا للكشف عن العقوبات:

- Torii يقارن بين `now` و `grace_expires_at` ويجمع جداول الشحن من `SuffixPolicyV1`.
- يجب أن يقوم اختبار الدفع بتغطية الشحنة. فالو => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` سجل `expires_at` الجديد.### 6.3 تجميد الوصي وتجاوز المجلس

1. يرسل ولي الأمر `FreezeNameRequestV1` مع التذكرة التي تشير إلى معرف الحادث.
2. Torii يغير السجل إلى `NameStatus::Frozen`، ينبعث `NameFrozen`.
3. من خلال العلاج، تجاوز انبعاث المجلس؛ يقوم المشغل بإرسال DELETE `/v1/sns/names/{namespace}/{literal}/freeze` مع `GovernanceHookV1`.
4. Torii التحقق من صحة التجاوز، وإصدار `NameUnfrozen`.

## 7. التحقق من صحة رموز الخطأ

| كوديجو ​​| الوصف | HTTP |
|--------|-----------|------|
| `sns_err_reserved` | قم بالتسمية المحجوزة أو المحظورة. | 409 |
| `sns_err_policy_violation` | مصطلح، طبقة أو مجموعة من المتحكمين في السياسة. | 422 |
| `sns_err_payment_mismatch` | عدم تطابق القيمة أو الأصول في تجربة الدفع. | 402 |
| `sns_err_governance_missing` | تتطلب المصنوعات اليدوية/غير الصالحة. | 403 |
| `sns_err_state_conflict` | العملية غير مسموح بها في الوضع الفعلي لدائرة الحياة. | 409 |

يتم بيع جميع الرموز عبر `X-Iroha-Error-Code` والمغلفات Norito JSON/NRPC estructurados.

## 8. ملاحظات التنفيذ

- Torii يحمي العناصر الفرعية المعلقة في `NameRecordV1.auction` ويعيد أهداف التسجيل المباشر أثناء ذلك في `PendingAuction`.
- تقوم عمليات الدفع بإعادة استخدام إيصالات دفتر الأستاذ Norito؛ خدمات البحث المثبتة لمساعد واجهات برمجة التطبيقات (`/v1/finance/sns/payments`).
- يجب أن تشتمل مجموعات SDK على نقاط النهاية هذه مع أنواع مساعدة تجعل المحافظ توفر أسبابًا واضحة للخطأ (`ERR_SNS_RESERVED`، وما إلى ذلك).## 9. بروكسيموس باسوس

- قم بتوصيل معالجات Torii بعقد التسجيل الحقيقي بمجرد تسجيل العناصر الفرعية SN-3.
- نشر أدوات SDK المحددة (Rust/JS/Swift) التي تشير إلى واجهة برمجة التطبيقات هذه.
- الموسع [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) يدمج سلاسل الأدلة في مجال الإدارة.