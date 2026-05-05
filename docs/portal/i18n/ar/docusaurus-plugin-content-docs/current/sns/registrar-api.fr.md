---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sns/registrar_api.md` وستؤدي إلى تغيير الوضع
انسخ Canonique du portail. يتم حفظ مصدر الملف من أجل التدفق
ترجمة.
:::

# API لمسجل SNS وخطافات الإدارة (SN-2b)

**الحالة:** Redige 24-03-2026 -- مراجعة وفقًا لـ Nexus Core  
**خريطة طريق الامتياز:** SN-2b "واجهة برمجة تطبيقات المسجل وخطافات الإدارة"  
**المتطلبات المسبقة:** تعريفات المخطط في [`registry-schema.md`](./registry-schema.md)

هذه الملاحظة تحدد نقاط النهاية Torii وخدمات gRPC وDTOs للطلب/الاستجابة
وأدوات الحكم اللازمة لتشغيل مسجل اسم Sora
الخدمة (SNS). هذا هو العقد المرجعي لحزم SDK والمحافظ وما إلى ذلك
الأتمتة التي تقوم بتسجيل أو تجديد أو تشغيل أسماء SNS.

## 1. النقل والمصادقة| الضرورة | تفصيل |
|----------|--------|
| البروتوكولات | REST sous `/v1/sns/*` وخدمة gRPC `sns.v1.Registrar`. يقبل الثنائي Norito-JSON (`application/json`) وNorito-RPC الثنائي (`application/x-norito`). |
| مصادقة | Jetons `Authorization: Bearer` أو شهادات mTLS emis par suffixsteward. نقاط النهاية الحساسة للحوكمة (التجميد/إلغاء التجميد، التأثيرات الاحتياطية) تتطلب `scope=sns.admin`. |
| حدود الخصم | يشارك المسجلون المستودعات `torii.preauth_scheme_limits` مع المستأنفين JSON بالإضافة إلى حدود rafale باللاحقة: `sns.register`، `sns.renew`، `sns.controller`، `sns.freeze`. |
| القياس عن بعد | يعرض Torii `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المسجل (مرشح `scheme="norito_rpc"`)؛ يتم زيادة واجهة برمجة التطبيقات أيضًا `sns_registrar_status_total{result, suffix_id}`. |

## 2. إغلاق DTO

تشير الأبطال إلى البنيات الأساسية المحددة في [`registry-schema.md`](./registry-schema.md). جميع الرسوم تستخدم `NameSelectorV1` + `SuffixId` لتجنب أي غموض في التوجيه.

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

## 3. راحة نقاط النهاية| نقطة النهاية | ميثود | الحمولة | الوصف |
|----------|--------|--------|-------------|
| `/v1/sns/names` | مشاركة | `RegisterNameRequestV1` | قم بالتسجيل أو البحث عن اسم. إعادة تحديد مستوى السعر، وتفعيل إجراءات الدفع/الحوكمة، وإجراء أحداث التسجيل. |
| `/v1/sns/names/{namespace}/{literal}/renew` | مشاركة | `RenewNameRequestV1` | إطالة المدة. قم بتزيين نوافذ النعمة/الخلاص من السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | مشاركة | `TransferNameRequestV1` | نقل الملكية مرة واحدة إلى موافقات الحوكمة المشتركة. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | ضع | `UpdateControllersRequestV1` | استبدل مجموعة وحدات التحكم؛ صالح عناوين الحساب الموقعة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | مشاركة | `FreezeNameRequestV1` | تجميد الوصي/المجلس. اطلب حارس تذكرة ومرجعًا إلى ملف الإدارة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | حذف | `GovernanceHookV1` | قم بإلغاء تجميد المعالجة اللاحقة؛ أؤكد أن تجاوز المجلس مسجل. |
| `/v1/sns/reserved/{selector}` | مشاركة | `ReservedAssignmentRequestV1` | تأثير الأسماء الاحتياطية على اسم المضيف/المجلس. |
| `/v1/sns/policies/{suffix_id}` | احصل على | -- | استرجع `SuffixPolicyV1` courant (قابل للتخزين المؤقت). |
| `/v1/sns/names/{namespace}/{literal}` | احصل على | -- | Retourne le `NameRecordV1` courant + Etat Effectif (Active, Grace, etc.). |

**تشفير المحدد:** الجزء `{selector}` يقبل i105، ضغط، أو سداسي عشري canonique selon ADDR-5؛ Torii يتم التطبيع عبر `NameSelectorV1`.**نماذج الأخطاء:** جميع نقاط النهاية العائدة Norito JSON مع `code`، `message`، `details`. تتضمن الرموز `sns_err_reserved`، و`sns_err_payment_mismatch`، و`sns_err_policy_violation`، و`sns_err_governance_missing`.

### 3.1 مساعدي CLI (شرط التسجيل اليدوي N0)

قد يتمكن مشرفو الإصدار التجريبي من استخدام المسجل عبر CLI بدون تصنيع JSON بشكل رئيسي:

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

- `--owner` يضغط بشكل افتراضي على حساب تكوين CLI؛ كرر `--controller` لإضافة وحدة تحكم الحسابات الإضافية (افتراضيًا `[owner]`).
- الأعلام المضمنة في مخطط الدفع المباشر مقابل `PaymentProofV1`؛ قم بتمرير `--payment-json PATH` عندما يكون لديك بنية تسجيل. تتبع البيانات الوصفية (`--metadata-json`) وخطافات الإدارة (`--governance-json`) مخطط الصورة.

المساعدات في المحاضرة تكمل التكرارات فقط:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Voir `crates/iroha_cli/src/commands/sns.rs` للتنفيذ؛ يتم إعادة استخدام الأوامر لـ DTOs Norito في هذا المستند بحيث تتوافق عملية CLI مع البايت للردود Torii.

المساعدات الإضافية تغطي التجديدات والتحويلات والإجراءات الوصية:

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
````--governance-json` يحتوي على تسجيل `GovernanceHookV1` صالح (معرف الاقتراح، تجزئات التصويت، مضيف التوقيعات/الوصي). كل ما عليك فعله هو إعادة تعيين نقطة النهاية `/v1/sns/names/{namespace}/{literal}/...` المتوافقة حتى يتمكن مشغلو الإصدار التجريبي من تكرار الأسطح Torii تمامًا التي تستجيب لها SDK.

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

تنسيق السلك: تجزئة المخطط Norito في وقت التجميع للتسجيل في
`fixtures/norito_rpc/schema_hashes.json` (خطوط `RegisterNameRequestV1`,
`RegisterNameResponseV1`، `NameRecordV1`، وما إلى ذلك).

## 5. خطافات الحكم والحماية

Chaque appel qui modifie l'état doit joinre des preuves reutilisables pour la relecture:

| العمل | تتطلب بيانات الحوكمة |
|--------|-----------------------------|
| معيار التسجيل/التجديد | تشير عملية الدفع إلى تعليمات التسوية؛ يتطلب مجلس التصويت الحصول على ما إذا كانت الطبقة تتطلب الموافقة على المضيف. |
| تسجيل الطبقة المميزة / التأثير الاحتياطي | `GovernanceHookV1` معرف الاقتراح المرجعي + إقرار المضيف. |
| نقل | هاش دو مجلس التصويت + هاش دو إشارة DAO؛ الوصي على التخليص عندما يتم رفض النقل بموجب قرار التقاضي. |
| تجميد/إلغاء التجميد | توقيع تذكرة الوصي بالإضافة إلى تجاوز مجلس دو (إلغاء التجميد). |

Torii التحقق من الإجراءات والتحقق:1. معرف الاقتراح الموجود في دفتر حسابات الإدارة (`/v1/governance/proposals/{id}`) والنظام هو `Approved`.
2. تتوافق التجزئة مع عناصر التصويت المسجلة.
3. تشير توقيعات الوكيل/الوصي إلى العناصر العامة الحاضرة في `SuffixPolicyV1`.

تعمل عناصر التحكم الإلكترونية على `sns_err_governance_missing`.

## 6. أمثلة على سير العمل

### 6.1 معيار التسجيل

1. يقوم العميل باستجواب `/v1/sns/policies/{suffix_id}` لاستعادة الجائزة والنعمة والمستويات المتاحة.
2. بناء العميل `RegisterNameRequestV1`:
   - `selector` اشتقاق التسمية i105 (تفضيل) أو ضغط (الاختيار الثاني).
   - `term_years` في حدود السياسة.
   - `payment` يشير إلى نقل خزانة/مضيف الفاصل.
3. Torii صالح:
   - التطبيع دو التسمية + liste Reservee.
   - المصطلح/السعر الإجمالي مقابل `PriceTierV1`.
   - مبلغ الدفع >= حساب السعر + الرسوم.
4. النجاح Torii:
   - الثبات `NameRecordV1`.
   - إيميت `RegistryEventV1::NameRegistered`.
   - إيميت `RevenueAccrualEventV1`.
   - العودة إلى السجل الجديد + الأحداث.

### 6.2 قلادة التجديد على شكل نعمة

تشمل التجديدات التي تتم على مدى النعمة الطلب القياسي بالإضافة إلى الكشف عن العقوبة:- Torii قارن `now` مقابل `grace_expires_at` وقم بإضافة جداول الرسوم الإضافية لـ `SuffixPolicyV1`.
- La preuve de paiement doit couvrir la extracharge. إيشيك => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` قم بتسجيل `expires_at` الجديد.

### 6.3 تجميد ولي الأمر وتجاوز مجلس الدو

1. الوصي لديه `FreezeNameRequestV1` مع تذكرة تشير إلى معرف الحادث.
2. Torii قم بإزالة السجل في `NameStatus::Frozen`، ثم `NameFrozen`.
3. بعد العلاج، سيتم تجاوز المجلس؛ يرسل المشغل DELETE `/v1/sns/names/{namespace}/{literal}/freeze` مع `GovernanceHookV1`.
4. Torii صالح للتجاوز، Emet `NameUnfrozen`.

## 7. التحقق من الصحة ورموز الأخطاء

| الكود | الوصف | HTTP |
|------|------------|------|
| `sns_err_reserved` | ضع علامة على الاحتياطي أو الكتلة. | 409 |
| `sns_err_policy_violation` | مصطلح، طبقة أو مجموعة من وحدات التحكم تنتهك السياسة. | 422 |
| `sns_err_payment_mismatch` | عدم تطابق القيمة أو الأصول في سداد الدفع. | 402 |
| `sns_err_governance_missing` | تتطلب مصنوعات الحكم الغياب/العجز. | 403 |
| `sns_err_state_conflict` | لا تسمح العملية بحالة دورة الحياة الفعلية. | 409 |

تظهر جميع الرموز عبر هياكل `X-Iroha-Error-Code` والمغلفات Norito JSON/NRPC.

## 8. ملاحظات التنفيذ- Torii يخزن المزادات ويراقب `NameRecordV1.auction` ويعيد تسجيل محاولات التسجيل مباشرة مثل `PendingAuction`.
- يتم إعادة استخدام إجراءات الدفع من خلال دفتر الأستاذ Norito؛ توفر خدمات الخزانة مساعد واجهات برمجة التطبيقات (`/v1/finance/sns/payments`).
- تقوم أدوات تطوير البرامج (SDK) بتغليف نقاط النهاية هذه مع أنواع مختلفة من المساعدين حتى تتمكن المحافظ من تقديم أسباب الخطأ الواضحة (`ERR_SNS_RESERVED`، وما إلى ذلك).

## 9. الأشرطة Prochaines

- استخدم المعالجات Torii في عقد التسجيل مرة واحدة في مزادات SN-3 المتوفرة.
- نشر أدلة SDK المحددة (Rust/JS/Swift) المرجعية لهذه API.
- قم بتمديد [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) مع الامتيازات إلى رؤساء خطافات الإدارة.