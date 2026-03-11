---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
هذا الجزء يرسل `docs/source/sns/registrar_api.md` ويصل إلى الخدمة
بوابة النسخ القانوني. الملف الأصلي مخصص لتحويلات العلاقات العامة.
:::

# مسجل API SNS وإدارة المنافذ (SN-2b)

**الحالة:** بتاريخ 24-03-2026 — على السعة Nexus Core  
**خريطة الطريق:** SN-2b "واجهة برمجة تطبيقات المسجل وخطافات الإدارة"  
**الاقتراحات:** أنظمة الاقتراحات في [`registry-schema.md`](./registry-schema.md)

يشير هذا الرابط إلى نقاط الاتصال Torii وخدمات gRPC ومكالمات/ردات DTO وما إلى ذلك
إدارة المصنوعات اليدوية، متطلبات العمل لمسجل Sora Name Service
(SNS). هذا عقد تفويض لـ SDK والملحقات والأتمتة التي
تحتاج إلى التسجيل أو النشر أو التحكم في رموز SNS.

## 1. النقل والتوثيق| تريبوفانيا | التفاصيل |
|------------|--------|
| البروتوكولات | الراحة تحت `/v1/sns/*` وخدمة gRPC `sns.v1.Registrar`. ستبدأ الآن Norito-JSON (`application/json`) وNorito-RPC (`application/x-norito`). |
| مصادقة | رموز `Authorization: Bearer` أو شهادات mTLS، يتم اختيارها من خلال مضيف لاحقة. نقاط التحكم المعززة (التجميد/إلغاء التجميد، التعيينات المحجوزة) تحتاج إلى `scope=sns.admin`. |
| التغذية | يقوم المسجل بتخصيص مستودعات `torii.preauth_scheme_limits` مع بيانات JSON بالإضافة إلى الحدود الإضافية حسب اللاحقة: `sns.register`، `sns.renew`، `sns.controller`، `sns.freeze`. |
| القياس عن بعد | Torii نشر `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمسجل المعالج (مرشح لـ `scheme="norito_rpc"`); تم أيضًا تحسين واجهة برمجة التطبيقات (API) `sns_registrar_status_total{result, suffix_id}`. |

## 2. فحص DTO

قم بالرجوع إلى الهياكل الأساسية، المقترحة في [`registry-schema.md`](./registry-schema.md). يتم تثبيت جميع الحمولات على `NameSelectorV1` + `SuffixId` لإخراج التخطيط غير المشروط.

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

## 3. نقاط الراحة| نقطة النهاية | الطريقة | الحمولة | الوصف |
|----------|-------|--------|----------|
| `/v1/sns/registrations` | مشاركة | `RegisterNameRequestV1` | قم بالتسجيل أو فتح الاسم مرة أخرى. قم بتقسيم الطبقة السعرية، وتحقق من صحة اللوحة/الإدارة، واحصل على مسجل الاشتراك. |
| `/v1/sns/registrations/{selector}/renew` | مشاركة | `RenewNameRequestV1` | توسيع نطاق. بادئ ذي بدء، النعمة/الفداء من السياسة. |
| `/v1/sns/registrations/{selector}/transfer` | مشاركة | `TransferNameRequestV1` | قبل الترحيب بعد التحكم في الانحناء. |
| `/v1/sns/registrations/{selector}/controllers` | ضع | `UpdateControllersRequestV1` | تخصيص وحدات التحكم; التحقق من حسابات عنوان البريد الإلكتروني. |
| `/v1/sns/registrations/{selector}/freeze` | مشاركة | `FreezeNameRequestV1` | تجميد الوصي/المجلس. تحتاج إلى تذكرة الوصي وقائمة الحوكمة. |
| `/v1/sns/registrations/{selector}/freeze` | حذف | `GovernanceHookV1` | قم بإلغاء التجميد بعد التصريف؛ من المفرح أن المجلس يتجاوز الإجازة. |
| `/v1/sns/reserved/{selector}` | مشاركة | `ReservedAssignmentRequestV1` | الاسم محفوظة لمضيف/مجلس. |
| `/v1/sns/policies/{suffix_id}` | احصل على | -- | احصل على تيكويو `SuffixPolicyV1` (كشيريمو). |
| `/v1/sns/registrations/{selector}` | احصل على | -- | تم تجديده `NameRecordV1` + مواد فعالة (Active، Grace، إلخ). |

**محدد الإضافة:** الجزء `{selector}` يستخدم I105، مضغوط (`sora`) أو سداسي عشري قانوني بواسطة ADDR-5؛ تمت تسوية Torii من خلال `NameSelectorV1`.**نموذج أوشيبوك:** جميع نقاط الاتصال تتصل بـ Norito JSON مع `code`، `message`، `details`. تتضمن الرموز `sns_err_reserved`، و`sns_err_payment_mismatch`، و`sns_err_policy_violation`، و`sns_err_governance_missing`.

### 3.1 تعزيزات CLI (المسجل الأفضل N0)

يمكن للمضيف استخدام الرهان المخفي باستخدام المسجل من خلال CLI دون استخدام أدوات بسيطة JSON:

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

- `--owner` للترقية باستخدام تكوين الحساب CLI؛ قم بإعادة `--controller` لإضافة حسابات وحدة التحكم الإضافية (من خلال `[owner]`).
- لوحة العلم المضمنة متوافقة مع `PaymentProofV1`؛ قم بالانتقال إلى `--payment-json PATH`، إذا كنت تمتلك طاقة هيكلية. البيانات التعريفية (`--metadata-json`) وخطافات الإدارة (`--governance-json`) تتبعها أيضًا.

تُكمل رسائل القراءة فقط التكرار:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

سم. `crates/iroha_cli/src/commands/sns.rs` للتنفيذ؛ تستخدم الأوامر مرة أخرى Norito DTO من هذه الوثيقة، وهو ما يتوافق مع CLI مع Torii بايت في بايت.

تعمل التعزيزات الإضافية على زيادة المبيعات والأخطاء والتصرف الوصي:

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

`--governance-json` يجب أن تكتب بشكل صحيح `GovernanceHookV1` (معرف الاقتراح، تجزئات التصويت، إضافة المضيف/الوصي). يقوم الأمر بمجرد توجيه نقطة الاتصال اللاسلكية `/v1/sns/registrations/{selector}/...` إلى المشغلين الذين يمكنهم تكرارها مرة أخرى Torii، الذي سيتم إنشاء SDK.

## 4. خدمة gRPC```text
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

تنسيق السلك: هذه المخططات Norito على المجموعة المكتوبة في
`fixtures/norito_rpc/schema_hashes.json` (رقم `RegisterNameRequestV1`,
`RegisterNameResponseV1`، `NameRecordV1`، إلخ. د.).

## 5. الإدارة والتوصيل الجيدان

كل ما تحتاجه للحصول على تطبيق رائع للتسليم، تسهيلات في المشاركة:

| الحقيقة | إدارة البيانات المطلوبة |
|----------|---------------------------|
| التسجيل/البيع القياسي | إيداع اللوحة مع تعليمات التسوية; لا حاجة إلى مجلس الإدارة، إذا لم تكن الطبقة بحاجة إلى مضيف الموافقة. |
| تسجيل الطبقة المميزة / المهمة المحجوزة | `GovernanceHookV1` مزود بمعرف الاقتراح + إقرار المضيف. |
| مقدمة | مجلس الكلمة الخاص به + إشارة DAO; تصريح ولي الأمر، عندما يتم البدء في إنشاء جراثيم مختلفة. |
| تجميد/إلغاء التجميد | Подпись تذكرة الوصي بالإضافة إلى تجاوز المجلس (إلغاء التجميد). |

Torii يختبر التوثيق، يختبر:

1. معرف الاقتراح موجود في إدارة دفتر الأستاذ (`/v1/governance/proposals/{id}`) والحالة `Approved`.
2. يتم تضمين هذه القطع الأثرية المسجلة.
3. يتم إرسال المضيف/الوصي إلى مجموعة المفاتيح العامة من `SuffixPolicyV1`.

يتم إجراء اختبار غير مريح `sns_err_governance_missing`.

## 6. الامثلة العملية العملية

### 6.1 التسجيل القياسي1. يطلب العميل `/v1/sns/policies/{suffix_id}` للحصول على الأسعار والمزايا والمستويات القابلة للاستكمال.
2. جهاز العميل `RegisterNameRequestV1`:
   - تم الحصول على `selector` من التصنيف I105 أو من الاقتراح المضغوط (`sora`).
   - `term_years` في السياسة السابقة.
   - `payment` يتصل بالخزانة/المضيف المقسم.
3. Torii تحقق:
   - تطبيع المعايير + قائمة محفوظة.
   - المصطلح/السعر الإجمالي مقابل `PriceTierV1`.
   - مجموع الاشتراكات >= أسعار معقولة + عمولات.
4. في حالة النجاح Torii:
   - الاشتراك `NameRecordV1`.
   - سجل `RegistryEventV1::NameRegistered`.
   - سجل `RevenueAccrualEventV1`.
   - قم بكتابة رسالة جديدة + الاشتراك.

### 6.2 التوسع في فترة النعمة

يشتمل التوسع في Grace على تأمين قياسي بالإضافة إلى شريط اكتشاف:

- Torii يتوافق مع `now` مع `grace_expires_at` ويضفي رسومًا إضافية على الأجهزة اللوحية من `SuffixPolicyV1`.
- يجب فرض رسوم إضافية على اللوحة. أوشيبكا => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` تم كتابة `expires_at` الجديد.

### 6.3 تجميد ولي الأمر وتجاوز المجلس

1. يقوم الجارديان بمعالجة `FreezeNameRequestV1` بالتذكرة، مع تحديد هوية الهوية.
2. Torii يعيد الكتابة إلى `NameStatus::Frozen`، ويكتب `NameFrozen`.
3. قام مجلس التفويض بتجاوز التجاوز؛ يقوم المشغل بحذف `/v1/sns/registrations/{selector}/freeze` مع `GovernanceHookV1`.
4. Torii تحقق من التجاوز، واكتب `NameUnfrozen`.## 7. التحقق من صحة ورموز أوشيبوك

| كود | الوصف | HTTP |
|-----|----------|-----|
| `sns_err_reserved` | تم حفظ البيانات أو حظرها. | 409 |
| `sns_err_policy_violation` | الطبقة أو الطبقة أو عدد من المتحكمين يزيد من السياسة. | 422 |
| `sns_err_payment_mismatch` | لا يوجد أي نشاط أو نشاط في لوحة التقديم. | 402 |
| `sns_err_governance_missing` | إجابة/إدارة القطع الأثرية غير الصحيحة. | 403 |
| `sns_err_state_conflict` | عملية غير مناسبة في دورة الحياة. | 409 |

يتم اختبار جميع الأكواد عبر `X-Iroha-Error-Code` وتحويلات Norito JSON/NRPC.

## 8.التمويل بعد التنفيذ

- Torii المزادات المعلقة في `NameRecordV1.auction` وإلغاء تسجيلات الدفعات الأولية، مثل `PendingAuction`.
- إيصالات تسجيل دفتر الأستاذ Norito; تقدم خدمات الخزانة واجهة برمجة تطبيقات المساعد (`/v1/finance/sns/payments`).
- يجب على SDK ملاحظة هذه النقاط من خلال الميزات المميزة التي يمكن من خلالها إظهار الأسباب الجيدة أوشيبوك (`ERR_SNS_RESERVED`، وما إلى ذلك).

## 9. الخطوات التالية

- قم بإضافة المنتج Torii إلى العقد الحقيقي بعد عرض المزاد SN-3.
- نشر SDK-rуководства (Rust/JS/Swift)، متصل بواجهة برمجة التطبيقات هذه.
- إعادة ربط [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) لربط الإدارة المسبقة.