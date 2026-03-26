---
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/registrar_api.md` y تعمل الان كنسخة بوابة
قياسية. يبقى ملف المصدر من اجل تدفقات الترجمة.
:::

# واجهة مسجل SNS وhooks الحوكمة (SN-2b)

**الحالة:** صيغ 2026-03-24 - قيد مراجعة Nexus Core  
**رابط خارطة الطريق:** SN-2b "API de registrador y ganchos de gobernanza"  
**المتطلبات المسبقة:** تعريفات المخطط في [`registry-schema.md`](./registry-schema.md)

تحدد هذه المذكرة نقاط نهاية Torii y gRPC y DTOات الطلب/الاستجابة y
الحوكمة اللازمة لتشغيل مسجل خدمة اسماء سورا (SNS). Otros SDK
والمحافظ والاتمتة التي تحتاج الى تسجيل او تجديد او ادارة اسماء SNS.

## 1. النقل والمصادقة

| المتطلب | التفاصيل |
|---------|----------|
| Artículos | REST utiliza `/v1/sns/*` y gRPC `sns.v1.Registrar`. Utilice Norito-JSON (`application/json`) y Norito-RPC (`application/x-norito`). |
| Autenticación | Utilice `Authorization: Bearer` y mTLS como administrador del sufijo. نقاط النهاية الحساسة للحوكمة (congelar/descongelar, تعيينات محجوزة) تتطلب `scope=sns.admin`. |
| حدود المعدل | Establece un cubo `torii.preauth_scheme_limits` con un archivo JSON que contiene una ráfaga de texto: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| القياس | Torii يعرض `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` لمعالجات المسجل (رشح `scheme="norito_rpc"`); Aquí está el mensaje `sns_registrar_status_total{result, suffix_id}`. |

## 2. نظرة عامة على DTOالحقول تشير الى البنى القياسية المعرفة في [`registry-schema.md`](./registry-schema.md). Para ello, utilice `NameSelectorV1` + `SuffixId`.

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

## 3. نقاط نهاية DESCANSO

| نقطة النهاية | الطريقة | الحمولة | الوصف |
|-------------|---------|---------|-------|
| `/v1/sns/names` | PUBLICAR | `RegisterNameRequestV1` | تسجيل او اعادة فتح اسم. يحل شريحة التسعير، يتحقق من اثباتات الدفع/الحوكمة, ويصدر احداث السجل. |
| `/v1/sns/names/{namespace}/{literal}/renew` | PUBLICAR | `RenewNameRequestV1` | يمدد المدة. يفرض نوافذ gracia/redención من السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | PUBLICAR | `TransferNameRequestV1` | ينقل الملكية بعد ارفاق موافقات الحوكمة. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PONER | `UpdateControllersRequestV1` | يستبدل مجموعة controladores؛ يتحقق من عناوين الحساب الموقعة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | PUBLICAR | `FreezeNameRequestV1` | تجميد tutor/consejo. يتطلب تذكرة guardian ومرجع دفتر حوكمة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | BORRAR | `GovernanceHookV1` | فك التجميد بعد المعالجة؛ Esta es una anulación de la función. |
| `/v1/sns/reserved/{selector}` | PUBLICAR | `ReservedAssignmentRequestV1` | تعيين اسماء محجوزة بواسطة administrador/consejo. |
| `/v1/sns/policies/{suffix_id}` | OBTENER | -- | يجلب `SuffixPolicyV1` الحالي (قابل للكاش). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENER | -- | يعيد `NameRecordV1` الحالي + الحالة الفعلية (Activo, Gracia, الخ). |

**Selector ترميز:** مقطع `{selector}` يقبل i105 او مضغوط او hex قياسي حسب ADDR-5; Torii يطبعها عبر `NameSelectorV1`.**Actualización:** Establece el JSON Norito entre `code`, `message`, `details`. Utilice los valores `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI de configuración (متطلب المسجل اليدوي N0)

Los azafatos están disponibles en la CLI mediante JSON:

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

- `--owner` يفترض حساب اعدادات CLI؛ كرر `--controller` لاضافة حسابات controlador اضافية (الافتراضي `[owner]`).
- اعلام الدفع المضمنة تطابق مباشرة `PaymentProofV1`; مرر `--payment-json PATH` عندما تكون لديك ايصال منظم. Los metadatos (`--metadata-json`) y los ganchos (`--governance-json`) están disponibles.

مساعدات القراءة فقط تكمل التمارين:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

راجع `crates/iroha_cli/src/commands/sns.rs` للتنفيذ; Utilice el controlador DTO Norito y conecte la CLI a Torii. ببايت.

مساعدات اضافية تغطي التجديدات والتحويلات واجراءات tutor:

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

`--governance-json` es el nombre de `GovernanceHookV1` (identificación de propuesta, hashes de voto, administrador/tutor). كل امر يعكس ببساطة نقطة النهاية `/v1/sns/names/{namespace}/{literal}/...` المقابلة حتى يتمكن مشغلو البيتا من تمرين اسطح Torii Estos son los SDK.

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

Formato de cable: hash مخطط Norito في وقت الترجمة مسجل تحت
`fixtures/norito_rpc/schema_hashes.json` (صفوف `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, الخ).

## 5. ganchos الحوكمة والادلةكل استدعاء يغير الحالة يجب ان يرفق ادلة مناسبة لاعادة التشغيل:

| الاجراء | بيانات الحوكمة المطلوبة |
|---------|-------------------------|
| التسجيل/التجديد القياسي | اثبات دفع يشير الى تعليمات asentamiento؛ لا يتطلب تصويت المجلس الا اذا كانت الشريحة تتطلب موافقة mayordomo. |
| تسجيل شريحة premium / تعيين محجوز | `GovernanceHookV1` يشير الى ID de propuesta + اقرار administrador. |
| نقل | hash تصويت المجلس + hash اشارة DAO؛ guardián de autorización عندما ينطلق النقل عبر حل نزاع. |
| تجميد/فك تجميد | توقيع تذكرة guardian مع anular المجلس (فك التجميد). |

Torii يتحقق من الاثباتات عبر فحص:

1. ID de propuesta موجود في دفتر الحوكمة (`/v1/governance/proposals/{id}`) وحالته `Approved`.
2. الـ hashes تطابق اثار التصويت المسجلة.
3. تواقيع mayordomo/tutor تشير الى المفاتيح العامة المتوقعة من `SuffixPolicyV1`.

التحقق الفاشل يعيد `sns_err_governance_missing`.

## 6. امثلة سير العمل

### 6.1 Actualizar1. يستعلم العميل `/v1/sns/policies/{suffix_id}` للحصول على الاسعار وفترة Grace والشرائح المتاحة.
2. يبني العميل `RegisterNameRequestV1`:
   - `selector` Coloque la etiqueta i105 (المفضل) او المضغوط (الخيار الثاني).
   - `term_years` ضمن حدود السياسة.
   - `payment` يشير الى تحويل splitter الخزينة/steward.
3. Torii Texto:
   - تطبيع etiqueta + قائمة محجوزة.
   - Plazo/precio bruto vs `PriceTierV1`.
   - مبلغ اثبات الدفع >= السعر المحسوب + الرسوم.
4. عند النجاح Torii:
   - يحفظ `NameRecordV1`.
   - Número `RegistryEventV1::NameRegistered`.
   - Número `RevenueAccrualEventV1`.
   - يعيد السجل الجديد + الاحداث.

### 6.2 تجديد خلال فترة gracia

تجديدات gracia تشمل الطلب القياسي بالاضافة الى كشف العقوبات:

- Torii يقارن `now` مقابل `grace_expires_at` y جداول recargo من `SuffixPolicyV1`.
- اثبات الدفع يجب ان يغطي recargo. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` يسجل `expires_at` الجديد.

### 6.3 تجميد guardián و anular المجلس

1. guardián يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii y `NameStatus::Frozen`, y `NameFrozen`.
3. Anulación de بعد المعالجة، يصدر المجلس; Haga clic en DELETE `/v1/sns/names/{namespace}/{literal}/freeze` o `GovernanceHookV1`.
4. Torii está anulado y `NameUnfrozen`.

## 7. التحقق واكواد الخطا| الكود | الوصف | HTTP |
|-------|-------|------|
| `sns_err_reserved` | العلامة محجوزة او محظورة. | 409 |
| `sns_err_policy_violation` | المدة او الشريحة او مجموعة controladores تخالف السياسة. | 422 |
| `sns_err_payment_mismatch` | عدم تطابق قيمة او activo في اثبات الدفع. | 402 |
| `sns_err_governance_missing` | اثار الحوكمة المطلوبة غائبة/غير صالحة. | 403 |
| `sns_err_state_conflict` | العملية غير مسموحة في حالة دورة الحياة الحالية. | 409 |

Utilice el formato `X-Iroha-Error-Code` y Norito JSON/NRPC.

## 8. ملاحظات التنفيذ

- Torii `NameRecordV1.auction` y `PendingAuction`.
- Otros productos relacionados con el producto Norito؛ خدمات الخزينة توفر API مساعدة (`/v1/finance/sns/payments`).
- ينبغي للـ SDKs تغليف هذه النقاط بمساعدات قوية النوع حتى تتمكن المحافظ من عرض اسباب خطا واضحة (`ERR_SNS_RESERVED`, الخ).

## 9. الخطوات التالية

- El cable de alimentación Torii está conectado al cable SN-3.
- Nuevo SDK (Rust/JS/Swift) disponible.
- توسيع [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) بروابط متقاطعة لحقول ادلة ganchos الحوكمة.