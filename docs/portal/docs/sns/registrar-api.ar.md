---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 14373f09e9a691a910fbde08548c5fdfe03581049c85af425c27c94fc4fcafd8
source_last_modified: "2025-11-10T20:06:34.010395+00:00"
translation_last_reviewed: 2026-01-01
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/registrar_api.md` وتعمل الان كنسخة بوابة
قياسية. يبقى ملف المصدر من اجل تدفقات الترجمة.
:::

# واجهة مسجل SNS وhooks الحوكمة (SN-2b)

**الحالة:** صيغ 2026-03-24 - قيد مراجعة Nexus Core  
**رابط خارطة الطريق:** SN-2b "Registrar API & governance hooks"  
**المتطلبات المسبقة:** تعريفات المخطط في [`registry-schema.md`](./registry-schema.md)

تحدد هذه المذكرة نقاط نهاية Torii وخدمات gRPC وDTOات الطلب/الاستجابة واثار
الحوكمة اللازمة لتشغيل مسجل خدمة اسماء سورا (SNS). وهي العقد المرجعي للـ SDKs
والمحافظ والاتمتة التي تحتاج الى تسجيل او تجديد او ادارة اسماء SNS.

## 1. النقل والمصادقة

| المتطلب | التفاصيل |
|---------|----------|
| البروتوكولات | REST تحت `/v1/sns/*` وخدمة gRPC `sns.v1.Registrar`. كلاهما يقبل Norito-JSON (`application/json`) و Norito-RPC الثنائي (`application/x-norito`). |
| Auth | توكنات `Authorization: Bearer` او شهادات mTLS صادرة لكل suffix steward. نقاط النهاية الحساسة للحوكمة (freeze/unfreeze, تعيينات محجوزة) تتطلب `scope=sns.admin`. |
| حدود المعدل | المسجلون يشتركون في buckets `torii.preauth_scheme_limits` مع مستدعي JSON بالاضافة الى حدود burst لكل لاحقة: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| القياس | Torii يعرض `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المسجل (رشح `scheme="norito_rpc"`); كما تزيد الواجهة `sns_registrar_status_total{result, suffix_id}`. |

## 2. نظرة عامة على DTO

الحقول تشير الى البنى القياسية المعرفة في [`registry-schema.md`](./registry-schema.md). كل الحمولة تتضمن `NameSelectorV1` + `SuffixId` لتجنب التوجيه الغامض.

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

## 3. نقاط نهاية REST

| نقطة النهاية | الطريقة | الحمولة | الوصف |
|-------------|---------|---------|-------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | تسجيل او اعادة فتح اسم. يحل شريحة التسعير، يتحقق من اثباتات الدفع/الحوكمة، ويصدر احداث السجل. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | يمدد المدة. يفرض نوافذ grace/redemption من السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | ينقل الملكية بعد ارفاق موافقات الحوكمة. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | يستبدل مجموعة controllers؛ يتحقق من عناوين الحساب الموقعة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | تجميد guardian/council. يتطلب تذكرة guardian ومرجع دفتر حوكمة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | DELETE | `GovernanceHookV1` | فك التجميد بعد المعالجة؛ يضمن تسجيل override للمجلس. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | تعيين اسماء محجوزة بواسطة steward/council. |
| `/v1/sns/policies/{suffix_id}` | GET | -- | يجلب `SuffixPolicyV1` الحالي (قابل للكاش). |
| `/v1/sns/names/{namespace}/{literal}` | GET | -- | يعيد `NameRecordV1` الحالي + الحالة الفعلية (Active, Grace, الخ). |

**ترميز selector:** مقطع `{selector}` يقبل I105 او مضغوط او hex قياسي حسب ADDR-5; Torii يطبعها عبر `NameSelectorV1`.

**نموذج الاخطاء:** كل نقاط النهاية تعيد Norito JSON مع `code`, `message`, `details`. تشمل الاكواد `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 مساعدات CLI (متطلب المسجل اليدوي N0)

يمكن لـ stewards البيتا المغلقة الان تشغيل المسجل عبر CLI بدون تجهيز JSON يدويا:

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

- `--owner` يفترض حساب اعدادات CLI؛ كرر `--controller` لاضافة حسابات controller اضافية (الافتراضي `[owner]`).
- اعلام الدفع المضمنة تطابق مباشرة `PaymentProofV1`; مرر `--payment-json PATH` عندما تكون لديك ايصال منظم. الـ metadata (`--metadata-json`) و hooks الحوكمة (`--governance-json`) تتبع نفس النمط.

مساعدات القراءة فقط تكمل التمارين:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

راجع `crates/iroha_cli/src/commands/sns.rs` للتنفيذ; تعيد الاوامر استخدام DTOات Norito الموصوفة في هذا المستند بحيث تتطابق مخرجات CLI مع ردود Torii بايتا ببايت.

مساعدات اضافية تغطي التجديدات والتحويلات واجراءات guardian:

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

`--governance-json` يجب ان يحتوي على سجل `GovernanceHookV1` صالح (proposal id، vote hashes، تواقيع steward/guardian). كل امر يعكس ببساطة نقطة النهاية `/v1/sns/names/{namespace}/{literal}/...` المقابلة حتى يتمكن مشغلو البيتا من تمرين اسطح Torii التي ستستدعيها SDKs.

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

Wire-format: hash مخطط Norito في وقت الترجمة مسجل تحت
`fixtures/norito_rpc/schema_hashes.json` (صفوف `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, الخ).

## 5. hooks الحوكمة والادلة

كل استدعاء يغير الحالة يجب ان يرفق ادلة مناسبة لاعادة التشغيل:

| الاجراء | بيانات الحوكمة المطلوبة |
|---------|-------------------------|
| التسجيل/التجديد القياسي | اثبات دفع يشير الى تعليمات settlement؛ لا يتطلب تصويت المجلس الا اذا كانت الشريحة تتطلب موافقة steward. |
| تسجيل شريحة premium / تعيين محجوز | `GovernanceHookV1` يشير الى proposal id + اقرار steward. |
| نقل | hash تصويت المجلس + hash اشارة DAO؛ clearance guardian عندما ينطلق النقل عبر حل نزاع. |
| تجميد/فك تجميد | توقيع تذكرة guardian مع override المجلس (فك التجميد). |

Torii يتحقق من الاثباتات عبر فحص:

1. proposal id موجود في دفتر الحوكمة (`/v1/governance/proposals/{id}`) وحالته `Approved`.
2. الـ hashes تطابق اثار التصويت المسجلة.
3. تواقيع steward/guardian تشير الى المفاتيح العامة المتوقعة من `SuffixPolicyV1`.

التحقق الفاشل يعيد `sns_err_governance_missing`.

## 6. امثلة سير العمل

### 6.1 تسجيل قياسي

1. يستعلم العميل `/v1/sns/policies/{suffix_id}` للحصول على الاسعار وفترة grace والشرائح المتاحة.
2. يبني العميل `RegisterNameRequestV1`:
   - `selector` مشتق من label I105 (المفضل) او المضغوط (الخيار الثاني).
   - `term_years` ضمن حدود السياسة.
   - `payment` يشير الى تحويل splitter الخزينة/steward.
3. Torii يتحقق:
   - تطبيع label + قائمة محجوزة.
   - Term/gross price vs `PriceTierV1`.
   - مبلغ اثبات الدفع >= السعر المحسوب + الرسوم.
4. عند النجاح Torii:
   - يحفظ `NameRecordV1`.
   - يصدر `RegistryEventV1::NameRegistered`.
   - يصدر `RevenueAccrualEventV1`.
   - يعيد السجل الجديد + الاحداث.

### 6.2 تجديد خلال فترة grace

تجديدات grace تشمل الطلب القياسي بالاضافة الى كشف العقوبات:

- Torii يقارن `now` مقابل `grace_expires_at` ويضيف جداول surcharge من `SuffixPolicyV1`.
- اثبات الدفع يجب ان يغطي surcharge. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` يسجل `expires_at` الجديد.

### 6.3 تجميد guardian وoverride المجلس

1. guardian يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii ينقل السجل الى `NameStatus::Frozen`, ويصدر `NameFrozen`.
3. بعد المعالجة، يصدر المجلس override; يرسل المشغل DELETE `/v1/sns/names/{namespace}/{literal}/freeze` مع `GovernanceHookV1`.
4. Torii يتحقق من override، ويصدر `NameUnfrozen`.

## 7. التحقق واكواد الخطا

| الكود | الوصف | HTTP |
|-------|-------|------|
| `sns_err_reserved` | العلامة محجوزة او محظورة. | 409 |
| `sns_err_policy_violation` | المدة او الشريحة او مجموعة controllers تخالف السياسة. | 422 |
| `sns_err_payment_mismatch` | عدم تطابق قيمة او asset في اثبات الدفع. | 402 |
| `sns_err_governance_missing` | اثار الحوكمة المطلوبة غائبة/غير صالحة. | 403 |
| `sns_err_state_conflict` | العملية غير مسموحة في حالة دورة الحياة الحالية. | 409 |

تظهر كل الاكواد عبر `X-Iroha-Error-Code` واغلفة Norito JSON/NRPC منظمة.

## 8. ملاحظات التنفيذ

- Torii يخزن المزادات المعلقة تحت `NameRecordV1.auction` ويرفض محاولات التسجيل المباشر بينما الحالة `PendingAuction`.
- اثباتات الدفع تعيد استخدام ايصالات دفتر Norito؛ خدمات الخزينة توفر APIs مساعدة (`/v1/finance/sns/payments`).
- ينبغي للـ SDKs تغليف هذه النقاط بمساعدات قوية النوع حتى تتمكن المحافظ من عرض اسباب خطا واضحة (`ERR_SNS_RESERVED`, الخ).

## 9. الخطوات التالية

- ربط معالجات Torii بعقد السجل الفعلي عندما تصل مزادات SN-3.
- نشر ادلة SDK خاصة (Rust/JS/Swift) تشير الى هذه الواجهة.
- توسيع [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) بروابط متقاطعة لحقول ادلة hooks الحوكمة.
