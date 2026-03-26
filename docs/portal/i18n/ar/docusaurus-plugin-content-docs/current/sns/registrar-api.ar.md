---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر القياسي
احترام هذه الصفحة `docs/source/sns/registrar_api.md` المعتمد الان كنسخة البوابة
حتى. يبقى ملف المصدر من تدفقات الترجمة.
:::

# واجهة تسجيل SNS وhooks الـ تور (SN-2b)

**الحالة:** صيغ 2026-03-24 - نسخة تجريبية Nexus Core  
**رابط خريطة الطريق:** SN-2b "واجهة برمجة تطبيقات المسجل وخطافات الإدارة"  
**المتطلبات المسبقة:** التعريفات المسبقة في [`registry-schema.md`](./registry-schema.md)

تسجيل هذه المذكرة نهاية نقاط Torii خدمات gRPC وDTOات الطلب/الاستجابة واثار
اللازمة لتشغيل خدمة تسجيل اسماء سورا (SNS). العقد المرجعي للـ SDKs
والمحافظ والامتعة التي تحتاج الى تسجيل او تجديد او ادارة اسماء SNS.

## 1. النقل والمصادقة

| المتطلب | التفاصيل |
|---------|---------|
| تفويضات | راحة تحت `/v1/sns/*` gRPC `sns.v1.Registrar`. يقبل فقط Norito-JSON (`application/json`) و Norito-RPC الثنائي (`application/x-norito`). |
| مصادقة | توكنات `Authorization: Bearer` او شهادات mTLS صادرة لكل suffixsteward. نقاط النهاية للحسم (تجميد/إلغاء التجميد، تعيينات محجوزة) تتطلب `scope=sns.admin`. |
| حدود العدل | المسجلون يشتركون في مجموعات `torii.preauth_scheme_limits` مع مستدعي JSON بالاضافة الى حدود انفجار لكل لاحقة: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| القياس | Torii المقدمة `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المستخدم (رشح `scheme="norito_rpc"`); كما أنت الواجهة `sns_registrar_status_total{result, suffix_id}`. |

## 2. نظرة عامة على DTOتوم تشير الى البنى القياسي للمعرفة في [`registry-schema.md`](./registry-schema.md). كل الحمولة تتضمن `NameSelectorV1` + `SuffixId` العلم الغامض.

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

## 3. نهاية نقاط الراحة

| نقطة |النهاية الطريقة | الحمولة | الوصف |
|-------------|---------|---------|-------|
| `/v1/sns/names` | مشاركة | `RegisterNameRequestV1` | تسجيل او إعادة فتح الاسم. يحل شريحة التسعير، ويحقق من اثبات الدفع/التكامل، ويصدر سجل الأحداث. |
| `/v1/sns/names/{namespace}/{literal}/renew` | مشاركة | `RenewNameRequestV1` | يمدد المدة. يفرض نافذة النعمة/الفداء من السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | مشاركة | `TransferNameRequestV1` | تنتقل الملكية بعد موافقة الرفاق. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ضع | `UpdateControllersRequestV1` | يستبدل مجموعة وحدات التحكم؛ ومن عناوين الحساب الموقعة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | مشاركة | `FreezeNameRequestV1` | تجميد الوصي/المجلس. تتطلب تذكرة ولي الأمر ومرجع ملزمة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | حذف | `GovernanceHookV1` | فك التجميد بعد المعالجة المركزية؛ ضمان تسجيل تجاوز للمجلس. |
| `/v1/sns/reserved/{selector}` | مشاركة | `ReservedAssignmentRequestV1` | تعيين اسماء محجوزة بواسطة ستيوارد/مجلس. |
| `/v1/sns/policies/{suffix_id}` | احصل على | -- | يأتي `SuffixPolicyV1` الحالي (قابل للكاش). |
| `/v1/sns/names/{namespace}/{literal}` | احصل على | -- | إعادة `NameRecordV1` الحالي + الحالة الحالية (Active, Grace, الخ). |

**ترميز المحدد:** مقطع `{selector}` يقبل i105 او سعيد او سداسي عشري حسب ADDR-5; Torii يطبعها عبر `NameSelectorV1`.**نموذج الاخطاء:** كل النقاط النهائية Norito JSON مع `code`, `message`, `details`. تشمل الاكواد `sns_err_reserved`، `sns_err_payment_mismatch`، `sns_err_policy_violation`، `sns_err_governance_missing`.

### 3.1 مساعدات CLI (متطلب المسجل يدوي N0)

يمكن لـ Stewards البيتا الان المشغل عبر CLI بدون خبرة JSON يدويا:

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

- `--owner` لإعدادات إعدادات CLI؛ كرر `--controller` اضافة حسابات المراقب المالي (الافتراضي `[owner]`).
- اعلام الدفع المضمنة تطابق مباشرة `PaymentProofV1`; مرر `--payment-json PATH` عندما يكون لديك أيصال منظم. الـ البيانات الوصفية (`--metadata-json`) والخطافات الـ تور (`--governance-json`) تتبع نفس النمط.

مساعدات القراءة فقط تكمل التمارين:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

مراجعة `crates/iroha_cli/src/commands/sns.rs` للتنفيذ; متضمنة الاوامر استخدام DTOات Norito الموصوفة في هذا المستند بحيث تتطابق فرق CLI مع ردود Torii بايتا ببايت.

مساعدات التغطية الشاملة للوثائق الأصلية والتحويلات وإجراءات ولي الأمر:

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

`--governance-json` يجب ان يحتوي على سجل `GovernanceHookV1` صالح (معرف الاقتراح، تجزئات التصويت، توقيع ستيوارد/وصي). كل امريكان يعكس بوضوح النقطة النهائية `/v1/sns/names/{namespace}/{literal}/...`.

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

تنسيق السلك: مخطط التجزئة Norito في وقت التسجيل تحت
`fixtures/norito_rpc/schema_hashes.json` (الصفوف `RegisterNameRequestV1`,
`RegisterNameResponseV1`، `NameRecordV1`، الخ).

## 5. خطافات الدمج والتكاملكل الاتصال بالحالة يجب أن يرفق معادلة مناسبة لعادة التشغيل:

| الاجراء | البيانات الأساسية الأساسية |
|---------|-----------------------|
| التسجيل/التجديد القياسي | اثبات يشير الى تعليمات التسوية؛ لا يلزم تصويت المجلس الا اذا طلبت موافقة ستيوارد. |
| تسجيل شريحة premium / تعيين محجوز | `GovernanceHookV1` يشير الى معرف الاقتراح + قرار ستيوارد. |
| نقل | hash تصويت المجلس + hash اشارة DAO؛ ولي التخليص عندما ينطلق النقل عبر حل النزاع. |
| تجميد/فك تجميد | توقيع تذكرة ولي الأمر مع تجاوز المجلس (فك التجميد). |

Torii يتحقق من الاثبات عبر الفحص:

1. معرف الاقتراح موجود في الكمبيوتر (`/v1/governance/proposals/{id}`) وحالته `Approved`.
2. الـ hashes تطابق اثار التصويت المختارة.
3. توقيع الوكيل/الوصي يشير الى المفاتيح العامة محدد من `SuffixPolicyV1`.

التحقق من الفاشل يعيد `sns_err_governance_missing`.

## 6. امثلة سير العمل

### 6.1 تسجيل قياسي1. يستعلم العميل `/v1/sns/policies/{suffix_id}` للحصول على الأسعار وفترة النعمة والشرائح المتاحة.
2. يبني العميل `RegisterNameRequestV1`:
   - `selector` مشغل من التسمية i105 (المفضل) او المصدر (الخيار الثاني).
   - `term_years` ضمن حدود السياسة.
   - `payment` يشير الى تحويل Splitter الخزينة/steward.
3.Torii ويتحقق:
   - تطبيع التسمية + قائمة محجوزة.
   - المصطلح/السعر الإجمالي مقابل `PriceTierV1`.
   - مبلغ اثبات الدفع >= السعر المحسوب + اللمسات.
4. عند النجاح Torii:
   - يحفظ `NameRecordV1`.
   - يصدر `RegistryEventV1::NameRegistered`.
   - يصدر `RevenueAccrualEventV1`.
   - إعادة التسجيل الجديد + الاحداث.

### 6.2 التجديد خلال فترة النعمة

تشمل تجديدات النعمة الطلب الكلاسيكي بالاضافة الى حماية الحماية:

- Torii يقارن `now` مقابل `grace_expires_at` بالإضافة إلى الرسم الإضافي من `SuffixPolicyV1`.
- اثبات الدفع يجب ان يغطي رسوم إضافية. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` تم التسجيل `expires_at` الجديد.

### 6.3 تجميد مجلس الوصي وتجاوزه

1. ولي الأمر يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii ينقل السجل إلى `NameStatus::Frozen`, ويصدر `NameFrozen`.
3. بعد المعالجة المركزية، يصدر تجاوز المجلس؛ أرسل لتشغيل DELETE `/v1/sns/names/{namespace}/{literal}/freeze` مع `GovernanceHookV1`.
4. Torii يتحقق من التجاوز، ويصدر `NameUnfrozen`.

## 7. التحقق من واكواد الخطا| الكود | الوصف | HTTP |
|-------|-------|------|
| `sns_err_reserved` | العلامة محجوزة او محظورة. | 409 |
| `sns_err_policy_violation` | مدة او جوان او مجموعة وحدات تحكم تخالف السياسة. | 422 |
| `sns_err_payment_mismatch` | لا تطابق قيمة او الأصول في اثبات الدفع. | 402 |
| `sns_err_governance_missing` | اثار الضرورات غائبة/غير صالحة. | 403 |
| `sns_err_state_conflict` | التجربة غير ناجحة في حالة دورة الحياة الحالية. | 409 |

قياس كل الاكواد عبر `X-Iroha-Error-Code` والمغلفة Norito JSON/NRPC منظمة.

## 8. ملاحظات التنفيذ

- Torii يخزن المزادات المعلقة تحت `NameRecordV1.auction` ويرفض البحث المباشر عن الحالة `PendingAuction`.
- اثبات الدفعات الرسمية استخدام ايصالات دفتر Norito؛ خدمات الخزينة توفر مساعدة APIs (`/v1/finance/sns/payments`).
-يجب للـ SDKs تتضمن هذه النقاط بمساعدات قوية النوع حتى غير مقبول من عرض اسباب خطا موافقة (`ERR_SNS_RESERVED`, الخ).

##9.الخطوات التالية

- معالج الارتباطات Torii بعقد السجل الفعلي عندما يصل إلى مزادات SN-3.
- نشر يعادل SDK خاص (Rust/JS/Swift) شاهد إلى هذه الصفحة.
- ينتهي [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) بروابط متقاطعة لقول تعادل السنانير ال تور.