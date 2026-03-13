---
lang: ar
direction: rtl
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sns/registrar_api.md` وagora تخدم كما أ
نسخة كانونيكا تفعل البوابة. الملف الدائم لتدفقات التجارة.
:::

# واجهة برمجة التطبيقات الخاصة بتسجيل SNS وخطافات الإدارة (SN-2b)

**الحالة:** Redigido 2026-03-24 -- تنهدات مراجعة Nexus Core  
**رابط خريطة الطريق:** SN-2b "واجهة برمجة تطبيقات المسجل وخطافات الإدارة"  
**المتطلبات المسبقة:** تعريفات التحدي في [`registry-schema.md`](./registry-schema.md)

هذه الملاحظة المحددة لنقاط النهاية Torii وخدمات gRPC وDTOs المطلوبة/الاستجابة
أدوات الإدارة اللازمة لتشغيل أو تسجيل خدمة اسم Sora (SNS).
وعقد التفويض لحزم SDK والمحافظ والأتمتة التي تطلب المسجل،
تجديد أو إدارة أسماء SNS.

## 1. النقل والأصالة| المتطلبات | ديتال |
|-----------|--------|
| البروتوكولات | REST sob `/v2/sns/*` وخدمة gRPC `sns.v1.Registrar`. نحن نستخدم Norito-JSON (`application/json`) وNorito-RPC ثنائي (`application/x-norito`). |
| مصادقة | الرموز المميزة `Authorization: Bearer` أو شهادات mTLS الصادرة بواسطة مضيف لاحق. نقاط النهاية الحساسة هي الحاكمة (التجميد/إلغاء التجميد، الخصائص المحجوزة) على سبيل المثال `scope=sns.admin`. |
| حدود التصنيف | يقوم المسجلون بتقسيم الدلاء `torii.preauth_scheme_limits` مع شارات JSON ذات حدود الاندفاع باللاحقة: `sns.register`، `sns.renew`، `sns.controller`، `sns.freeze`. |
| القياس عن بعد | Torii يعرض `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المسجل (filtrar `scheme="norito_rpc"`)؛ يتم زيادة واجهة برمجة التطبيقات (API) إلى `sns_registrar_status_total{result, suffix_id}`. |

## 2. تأشيرة DTO العامة

تشير المجالات المرجعية إلى الهياكل القانونية المحددة في [`registry-schema.md`](./registry-schema.md). تتضمن جميع الشحنات `NameSelectorV1` + `SuffixId` لتجنب الغموض.

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

## 3. راحة نقاط النهاية| نقطة النهاية | الطريقة | الحمولة | وصف |
|----------|-------|--------|-----------|
| `/v2/sns/registrations` | مشاركة | `RegisterNameRequestV1` | قم بالتسجيل أو إعادة إنشاء الاسم. حل مستوى الأسعار، وإثبات صحة الدفع/الإدارة، وإصدار أحداث التسجيل. |
| `/v2/sns/registrations/{selector}/renew` | مشاركة | `RenewNameRequestV1` | estende o termo. Aplica janelas de Grace/Redemption da Politica. |
| `/v2/sns/registrations/{selector}/transfer` | مشاركة | `TransferNameRequestV1` | نقل الملكية عند الحصول على موافقة الإدارة على الإضافات. |
| `/v2/sns/registrations/{selector}/controllers` | ضع | `UpdateControllersRequestV1` | استبدال مجموعة وحدات التحكم؛ صالح enderecos de conta assinados. |
| `/v2/sns/registrations/{selector}/freeze` | مشاركة | `FreezeNameRequestV1` | تجميد الوصي/المجلس. اطلب من ولي الأمر الرجوع إلى جدول الحوكمة. |
| `/v2/sns/registrations/{selector}/freeze` | حذف | `GovernanceHookV1` | قم بإلغاء تجميد apos remediacao؛ تجاوز الضمان من خلال تسجيل المجلس. |
| `/v2/sns/reserved/{selector}` | مشاركة | `ReservedAssignmentRequestV1` | تمت إضافة الأسماء المحجوزة إلى المضيف/المجلس. |
| `/v2/sns/policies/{suffix_id}` | احصل على | -- | Busca `SuffixPolicyV1` atual (cacheavel). |
| `/v2/sns/registrations/{selector}` | احصل على | -- | Retorna `NameRecordV1` atual + estado efetivo (نشط، نعمة، وما إلى ذلك). |

**مفتاح التحديد:** الجزء `{selector}` يحتوي على I105، مدمج أو سداسي عشري متوافق مع ADDR-5؛ Torii تطبيع عبر `NameSelectorV1`.**نموذج الأخطاء:** جميع نقاط نهاية نظام التشغيل ترجع إلى Norito JSON com `code`, `message`, `details`. تتضمن الرموز `sns_err_reserved`، و`sns_err_payment_mismatch`، و`sns_err_policy_violation`، و`sns_err_governance_missing`.

### 3.1 واجهة سطر الأوامر للمساعدين (متطلبات دليل المسجل N0)

يمكن لمشرفي النسخة التجريبية الآن تشغيل أو تسجيل عبر CLI دون إنشاء JSON يدويًا:

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

- `--owner` بطاقة وحساب تكوين CLI؛ أعد `--controller` لإضافة وحدة التحكم الإضافية (padrao `[owner]`).
- علامات الدفع المضمنة مضمنة مباشرة لـ `PaymentProofV1`؛ قم بالتمرير `--payment-json PATH` عندما تقوم بالتحدث وتلقي الدعم. Metadados (`--metadata-json`) وخطافات التحكم (`--governance-json`) تتبع أو في نفس الوقت.

مساعدو القراءة يكتملون أحيانًا بالأفكار:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` للتنفيذ؛ تقوم الأوامر بإعادة استخدام DTOs Norito الموضحة في هذا المستند بحيث يتطابق قول CLI مع بايت لكل بايت مع إجابات Torii.

المساعدون الإضافيون في عمليات التجديد والتحويلات وأصول الوصي:

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

`--governance-json` يجب أن يكون لديك سجل `GovernanceHookV1` صالح (معرف الاقتراح، تجزئات التصويت، المضيف/الوصي). كل ما عليك فعله هو تحديد نقطة النهاية `/v2/sns/registrations/{selector}/...` التي تتوافق مع مشغلي الإصدار التجريبي تمامًا مثل السطوح Torii التي ترسمها SDKs.## 4.سيرفيكو جي آر بي سي

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

تنسيق السلك: علامة التجزئة Norito في وقت التجميع المسجل
`fixtures/norito_rpc/schema_hashes.json` (لينهاس `RegisterNameRequestV1`،
`RegisterNameResponseV1`، `NameRecordV1`، وما إلى ذلك).

## 5. خطافات الحكم والأدلة

يجب على كل من يغير الحالة أن يقدم أدلة كافية لإعادة التشغيل:

| أكاو | بيانات الحكم المطلوبة |
|------|----------------------------|
| ريجيسترو/رينوفاكاو بادراو | يشير إثبات الدفع إلى تعليمات التسوية؛ لا ينبغي لنا أن نصوت على المجلس إلا إذا كان المستوى المطلوب من المضيفة. |
| تسجيل الطبقة المميزة / الملكية المحجوزة | `GovernanceHookV1` معرف الاقتراح المرجعي + إقرار المضيف. |
| ترانسفيرنسيا | تجزئة مجلس التصويت + تجزئة DAO؛ تصريح الوصي عند النقل والإجراء من أجل حل النزاع. |
| تجميد/إلغاء التجميد | يقوم Assinatura بتجاوز ولي الأمر لتذكرة المجلس (إلغاء التجميد). |

Torii تم التحقق منه كما يتم تقديمه:

1. معرف الاقتراح غير موجود في دفتر الأستاذ الحاكم (`/v2/governance/proposals/{id}`) والحالة e `Approved`.
2. تتوافق التجزئة مع مصنوعات الصوت المسجلة.
3. يشير Assinaturas إلى الوكيل / الوصي كأشخاص من المتوقعين من `SuffixPolicyV1`.

إعادة فالهاس `sns_err_governance_missing`.

## 6. أمثلة على تدفق العمل

### 6.1 سجل التسجيل1. قم باستشارة العميل `/v2/sns/policies/{suffix_id}` للحصول على الأسعار والمزايا والمستويات المتاحة.
2. أيها العميل مونتا `RegisterNameRequestV1`:
   - `selector` مشتق من الملصق I105 (المفضل) أو المدمج (الثاني أفضل حجب).
   - `term_years` داخل حدود السياسة.
   - `payment` يشير إلى نقل وحدة التخزين/المضيف.
3. Torii صالحة:
   - تطبيع التسمية + القائمة المحجوزة.
   - المصطلح/السعر الإجمالي مقابل `PriceTierV1`.
   - إثبات مبلغ الدفع >= الحساب المسبق + الرسوم.
4. النجاح Torii:
   - الثبات `NameRecordV1`.
   -إميت `RegistryEventV1::NameRegistered`.
   -إميت `RevenueAccrualEventV1`.
   - العودة إلى السجل الجديد + الأحداث.

### 6.2 Renovacao durante Grace

تشتمل عمليات التجديد خلال فترة السماح على متطلبات الكشف عن العقوبة الإضافية:

- Torii يقارن بين `now` و`grace_expires_at`، بالإضافة إلى إضافة علامات التبويب الخاصة بـ `SuffixPolicyV1`.
- يجب أن يتم إثبات الدفع من خلال سوبريتاكسا. فالها => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` سجل أو `expires_at` جديد.

### 6.3 تجميد الوصي وتجاوز المجلس

1. يرسل الجارديان `FreezeNameRequestV1` بمرجع تذكرة معرف الحادث.
2. Torii قم بنقل التسجيل لـ `NameStatus::Frozen`، قم بإصدار `NameFrozen`.
3. Apos remediacao، o تجاوز المجلس؛ o يقوم المشغل بحذف `/v2/sns/registrations/{selector}/freeze` com `GovernanceHookV1`.
4. Torii التحقق من الصحة أو التجاوز، إصدار `NameUnfrozen`.## 7. التحقق من صحة رموز الخطأ

| كوديجو ​​| وصف | HTTP |
|--------|----------|------|
| `sns_err_reserved` | قم بتسمية الحجز أو الحظر. | 409 |
| `sns_err_policy_violation` | في المصطلحات، طبقة أو مجموعة من وحدات التحكم تنتهك السياسة. | 422 |
| `sns_err_payment_mismatch` | عدم تطابق القيمة أو الأصول مع إثبات الدفع. | 402 |
| `sns_err_governance_missing` | Artefatos de Goveranca requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | التشغيل لا يسمح بالوضع الحالي لدائرة الحياة. | 409 |

تظهر جميع الرموز عبر `X-Iroha-Error-Code` والمغلفات Norito JSON/NRPC estruturados.

## 8. ملاحظات التنفيذ

- Torii قم بحفظ المعلقات الصغيرة في `NameRecordV1.auction` وقم بإعادة تسجيل التجارب مباشرة حتى يكون `PendingAuction`.
- تجربة إعادة استخدام مدفوعات دفتر الأستاذ Norito؛ servicos de tesouraria fornecem APIs helper (`/v2/finance/sns/payments`).
- تتضمن أدوات تطوير البرمجيات (SDKs) نقاط النهاية هذه كأدوات مساعدة مفيدة جدًا حتى تقدم المحافظ دوافع واضحة للخطأ (`ERR_SNS_RESERVED`، وما إلى ذلك).

## 9. بروكسيموس باسوس

- قم بتوصيل معالجات Torii بعقد التسجيل الحقيقي عند تسجيل أرقام SN-3.
- نشر أدوات SDK المحددة (Rust/JS/Swift) المرجعية إلى واجهة برمجة التطبيقات هذه.
- المُرسل [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) com الروابط المتوجهة لمجالات أدلة الحكم.