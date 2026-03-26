---
lang: pt
direction: ltr
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
Resolva o problema `docs/source/sns/registrar_api.md` e verifique o valor do produto
قياسية. Não se preocupe, não há problema em usá-lo.
:::

# واجهة مسجل SNS وhooks الحوكمة (SN-2b)

**Edição:** Data de 2026-03-24 - Versão Nexus Core  
**رابط خارطة الطريق:** SN-2b "API do registrador e ganchos de governança"  
**Esqueça o valor:** Ative o valor em [`registry-schema.md`](./registry-schema.md)

Obtenha o código Torii e gRPC e DTOات / الاستجابة واثار
O serviço de streaming pode ser acessado por meio de rede social (SNS). Qual é o valor dos SDKs
والمحافظ والاتمتة التي تحتاج الى تسجيل او تجديد او ادارة اسماء SNS.

## 1. النقل والمصادقة

| المتطلب | التفاصيل |
|--------|----------|
| Produtos | REST é `/v1/sns/*` e gRPC `sns.v1.Registrar`. Você pode usar Norito-JSON (`application/json`) e Norito-RPC (`application/x-norito`). |
| Autenticação | O `Authorization: Bearer` e o mTLS são usados ​​como sufixo steward. نقاط النهاية الحساسة للحوكمة (congelar/descongelar, تعيينات محجوزة) تتطلب `scope=sns.admin`. |
| حدود المعدل | O nome do bucket é `torii.preauth_scheme_limits`, mas o JSON é definido como burst para o seguinte: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| القياس | Torii é compatível com `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}`. Use o código `sns_registrar_status_total{result, suffix_id}`. |

## 2. نظرة عامة على DTO

Verifique o valor do arquivo em [`registry-schema.md`](./registry-schema.md). Use a opção `NameSelectorV1` + `SuffixId` para obter mais informações.

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
|---------|---------|---------|-------|
| `/v1/sns/names` | POSTAR | `RegisterNameRequestV1` | تسجيل او اعادة فتح اسم. Não se preocupe, você pode usar um dispositivo de segurança/recuperação de energia. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTAR | `RenewNameRequestV1` | يمدد المدة. يفرض نوافذ graça/redenção من السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTAR | `TransferNameRequestV1` | ينقل الملكية بعد ارفاق موافقات الحوكمة. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | COLOCAR | `UpdateControllersRequestV1` | Controladores de controle remoto; يتحقق من عناوين الحساب الموقعة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTAR | `FreezeNameRequestV1` | تجميد guardião/conselho. يتطلب تذكرة guardião e مرجع دفتر حوكمة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | EXCLUIR | `GovernanceHookV1` | فك التجميد بعد المعالجة؛ Você não pode substituir o valor. |
| `/v1/sns/reserved/{selector}` | POSTAR | `ReservedAssignmentRequestV1` | تعين اسماء محجوزة بواسطة mordomo/conselho. |
| `/v1/sns/policies/{suffix_id}` | OBTER | -- | Selecione `SuffixPolicyV1` (para o caso). |
| `/v1/sns/names/{namespace}/{literal}` | OBTER | -- | يعيد `NameRecordV1` الحالي + الحالة الفعلية (Ativo, Graça, الخ). |

**Seletor de extensão:** مقطع `{selector}` يقبل i105 او مضغوط او hex قياسي حسب ADDR-5; Torii é igual a `NameSelectorV1`.

**Nome do arquivo:** Para definir o valor do Norito JSON como `code`, `message`, `details`. Verifique o código `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI do aplicativo (متطلب المسجل اليدوي N0)

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
```- `--owner` يفترض حساب اعدادات CLI; O `--controller` é um controlador de controle remoto (`[owner]`).
- اعلام الدفع المضمنة تطابق مباشرة `PaymentProofV1`; O produto `--payment-json PATH` está danificado. Os metadados (`--metadata-json`) e os ganchos (`--governance-json`) estão disponíveis.

مساعدات القراءة فقط تكمل التمارين:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

راجع `crates/iroha_cli/src/commands/sns.rs` para o banco; Use o DTO Norito para usar o CLI no seu computador Torii é de alta qualidade.

مساعدات اضافية تغطي التجديدات والتحويلات واجراءات guardião:

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

`--governance-json` é um nome de usuário do `GovernanceHookV1` (id da proposta, hashes de voto, administrador/guardião). كل امر يعكس ببساطة نقطة النهاية `/v1/sns/names/{namespace}/{literal}/...` مقابلة حتى يتمكن مشغلو البيتا من تمرين اسطح Torii é compatível com SDKs.

## 4. Como gRPC

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

Formato de fio: hash مخطط Norito في وقت الترجمة مسجل تحت
`fixtures/norito_rpc/schema_hashes.json` (صفوف `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, mais).

## 5. ganchos الحوكمة والادلة

كل استدعاء يغير الحالة يجب ان يرفق ادلة مناسبة لاعادة التشغيل:

| الاجراء | Máquinas de lavar louça |
|--------|------------------------|
| Artigos/serviços | اثبات دفع يشير الى تعليمات liquidação; لا يتطلب تصويت المجلس الا اذا كانت الشريحة تتطلب o mordomo. |
| Produtos premium premium / Produtos de qualidade | `GovernanceHookV1` é o ID da proposta + administrador. |
| Não | hash تصويت المجلس + hash اشارة DAO; guardião de autorização عندما ينطلق النقل عبر حل نزاع. |
| تجميد/فك تجميد | توقيع تذكرة guardião مع override المجلس (فك التجميد). |

Torii é um arquivo de configuração:

1. ID da proposta: ID da proposta (`/v1/governance/proposals/{id}`) e `Approved`.
2. Os hashes são usados ​​para criar hashes.
3. O mordomo/tutor é o responsável por `SuffixPolicyV1`.

O código é `sns_err_governance_missing`.

## 6. امثلة سير العمل

### 6.1 تسجيل قياسي

1. Use o `/v1/sns/policies/{suffix_id}` para obter a graça e a graça.
2. Código `RegisterNameRequestV1`:
   - `selector` é compatível com a etiqueta i105 (i105) e i105 (i105).
   - `term_years` é um problema.
   - `payment` é um divisor de divisão/steward.
3. Torii Descrição:
   - Etiqueta تطبيع + قائمة محجوزة.
   - Prazo/preço bruto vs `PriceTierV1`.
   - مبلغ اثبات الدفع >= السعر المحسوب + الرسوم.
4. Versão Torii:
   - `NameRecordV1`.
   - Modelo `RegistryEventV1::NameRegistered`.
   - Modelo `RevenueAccrualEventV1`.
   - يعيد السجل الجديد + الاحداث.

### 6.2 تجديد خلال فترة graça

تجديدات graça تشمل الطلب القياسي بالاضافة الى كشف العقوبات:

- Torii يقارن `now` مقابل `grace_expires_at` ويضيف جداول sobretaxa de `SuffixPolicyV1`.
- اثبات الدفع يجب ان يغطي sobretaxa. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` é igual a `expires_at`.

### 6.3 Guardião e substituição do guardião1. guardião يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii é compatível com `NameStatus::Frozen`, e `NameFrozen`.
3. بعد المعالجة, يصدر المجلس substituição; Execute DELETE `/v1/sns/names/{namespace}/{literal}/freeze` em `GovernanceHookV1`.
4. Torii é substituído por `NameUnfrozen`.

## 7. التحقق واكواد الخطا

| الكود | الوصف | http |
|-------|-------|------|
| `sns_err_reserved` | العلامة محجوزة او محظورة. | 409 |
| `sns_err_policy_violation` | Os controladores de dados, aplicativos e controladores são usados. | 422 |
| `sns_err_payment_mismatch` | Ele é um ativo e um ativo que você pode usar. | 402 |
| `sns_err_governance_missing` | Verifique se há algum problema/descascamento. | 403 |
| `sns_err_state_conflict` | Não há problema em fazer isso. | 409 |

Você pode usar `X-Iroha-Error-Code` e Norito JSON/NRPC.

## 8. ملاحظات التنفيذ

- Torii يخزن المزادات المعلقة تحت `NameRecordV1.auction` ويرفض محاولات التسجيل المباشر بينما الحالة `PendingAuction`.
- اثباتات الدفع تعيد استخدام ايصالات دفتر Norito; Você pode usar APIs de terceiros (`/v1/finance/sns/payments`).
- ينبغي للـ SDKs تغليف هذه النقاط بمساعدات قوية النوع حتى تتمكن المحافظ من عرض اسباب خطا e (`ERR_SNS_RESERVED`, país).

## 9. الخطوات التالية

- ربط معالجات Torii بعقد السجل الفعلي عندما تصل مزادات SN-3.
- نشر ادلة SDK خاصة (Rust/JS/Swift) تشير الى هذه الواجهة.
- توسيع [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) بروابط متقاطعة لحقول ادلة ganchos الحوكمة.