---
lang: fr
direction: ltr
source: docs/portal/docs/sns/registrar-api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/registrar_api.md` وتعمل الان كنسخة بوابة
قياسية. يبقى ملف المصدر من اجل تدفقات الترجمة.
:::

# Crochets SNS (SN-2b)

**الحالة:** 2026-03-24 - pour Nexus Core  
**رابط خارطة الطريق :** SN-2b "API du registraire et hooks de gouvernance"  
**المتطلبات المسبقة :** تعريفات المخطط في [`registry-schema.md`](./registry-schema.md)

تحدد هذه المذكرة نقاط نهاية Torii et gRPC et DTOات الطلب/الاستجابة واثار
Le réseau social est doté d'un réseau social (SNS). Et les SDK
Il existe également des réseaux sociaux et des réseaux sociaux.

## 1. النقل والمصادقة

| المتطلب | التفاصيل |
|---------|----------|
| البروتوكولات | REST est `/v1/sns/*` et gRPC `sns.v1.Registrar`. Il s'agit de Norito-JSON (`application/json`) et Norito-RPC (`application/x-norito`). |
| Authentification | Le `Authorization: Bearer` et mTLS sont utilisés comme gestionnaire de suffixes. نقاط النهاية الحساسة للحوكمة (geler/dégeler, تعيينات محجوزة) تتطلب `scope=sns.admin`. |
| حدود المعدل | Les buckets `torii.preauth_scheme_limits` sont compatibles avec JSON pour le burst comme suit : `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| القياس | Torii et `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` pour les appareils photo (رشح `scheme="norito_rpc"`); Il s'agit de la référence `sns_registrar_status_total{result, suffix_id}`. |

## 2. نظرة عامة على DTOIl s'agit de [`registry-schema.md`](./registry-schema.md). كل الحمولة تضمن `NameSelectorV1` + `SuffixId` لتجنب التوجيه الغامض.

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

## 3. نقاط نهاية REPOS

| نقطة النهاية | الطريقة | الحمولة | الوصف |
|-------------|---------|---------|-------|
| `/v1/sns/names` | POSTER | `RegisterNameRequestV1` | تسجيل او اعادة فتح اسم. يحل شريحة التسعير، يتحقق من اثباتات الدفع/الحوكمة، ويصدر احداث السجل. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POSTER | `RenewNameRequestV1` | يمدد المدة. يفرض نوافذ grâce/rédemption من السياسة. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POSTER | `TransferNameRequestV1` | ينقل الملكية بعد ارفاق موافقات الحوكمة. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | METTRE | `UpdateControllersRequestV1` | يستبدل مجموعة contrôleurs؛ يتحقق من عناوين الحساب الموقعة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POSTER | `FreezeNameRequestV1` | تجميد tuteur/conseil. يتطلب تذكرة gardien ومرجع دفتر حوكمة. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | SUPPRIMER | `GovernanceHookV1` | فك التجميد بعد المعالجة؛ يضمن تسجيل override للمجلس. |
| `/v1/sns/reserved/{selector}` | POSTER | `ReservedAssignmentRequestV1` | تعيين اسماء محجوزة بواسطة intendant/conseil. |
| `/v1/sns/policies/{suffix_id}` | OBTENIR | -- | يجلب `SuffixPolicyV1` الحالي (قابل للكاش). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENIR | -- | يعيد `NameRecordV1` الحالي + الحالة الفعلية (Active, Grace, الخ). |

** Sélecteur de type : ** مقطع `{selector}` يقبل i105 و مضغوط او hex قياسي حسب ADDR-5; Torii est remplacé par `NameSelectorV1`.**Modalités :** Le format de fichier est Norito JSON avec `code`, `message`, `details`. Utilisez `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 مساعدات CLI (متطلب المسجل اليدوي N0)

Les stewards sont également compatibles avec JSON :

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

- `--owner` يفترض حساب اعدادات CLI؛ Utilisez le contrôleur `--controller` pour le contrôleur (`[owner]`).
- اعلام الدفع المضمنة تطابق مباشرة `PaymentProofV1` ; مرر `--payment-json PATH` عندما تكون لديك ايصال منظم. Les métadonnées (`--metadata-json`) et les crochets (`--governance-json`) sont disponibles.

مساعدات القراءة فقط تكمل التمارين:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

راجع `crates/iroha_cli/src/commands/sns.rs` للتنفيذ; Utilisez le DTOات Norito pour créer un lien vers la CLI Torii. بايتا ببايت.

مساعدات اضافية تغطي التجديدات والتحويلات واجراءات tuteur :

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

`--governance-json` est considéré comme `GovernanceHookV1` (identifiant de proposition, hachages de vote, administrateur/tuteur). كل امر يعكس ببساطة نقطة النهاية `/v1/sns/names/{namespace}/{literal}/...` المقابلة حتى يتمكن مشغلو البيتا من تمرين اسطح Torii contient des SDK.

## 4. Utiliser gRPC

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

Wire-format: hash مخطط Norito pour plus de détails
`fixtures/norito_rpc/schema_hashes.json` (voir `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, خ).

## 5. crochets الحوكمة والادلةكل استدعاء يغير الحالة يجب ان يرفق ادلة مناسبة لاعادة التشغيل:

| الاجراء | بيانات الحوكمة المطلوبة |
|---------|-------------------------|
| التسجيل/التجديد القياسي | اثبات دفع يشير الى تعليمات règlement؛ لا يتطلب تصويت المجلس الا اذا كانت الشريحة تتطلب موافقة steward. |
| تسجيل شريحة premium / تعيين محجوز | `GovernanceHookV1` يشير الى identifiant de proposition + اقرار steward. |
| نقل | hash تصويت المجلس + hash اشارة DAO؛ gardien de liquidation عندما ينطلق النقل عبر حل نزاع. |
| تجميد/فك تجميد | Le gardien peut remplacer la fonction (فك التجميد). |

Torii يتحقق من الاثباتات عبر فحص:

1. ID de proposition موجود في دفتر الحوكمة (`/v1/governance/proposals/{id}`) et `Approved`.
2. Les hachages تطابق اثار التصويت المسجلة.
3. Le rôle d'intendant/tuteur est celui de `SuffixPolicyV1`.

Il s'agit de `sns_err_governance_missing`.

## 6. امثلة سير العمل

### 6.1 تسجيل قياسي1. يستعلم العميل `/v1/sns/policies/{suffix_id}` للحصول على الاسعار وفترة grace والشرائح المتاحة.
2. يبني العميل `RegisterNameRequestV1` :
   - `selector` مشتق من label i105 (المفضل) او المضغوط (الخيار الثاني).
   - `term_years` est disponible.
   - `payment` pour diviseur/steward.
3. Torii inclus :
   - Étiquette تطبيع + قائمة محجوزة.
   - Durée/prix brut vs `PriceTierV1`.
   - مبلغ اثبات الدفع >= السعر المحسوب + الرسوم.
4. عند النجاح Torii :
   - يحفظ `NameRecordV1`.
   - par `RegistryEventV1::NameRegistered`.
   - par `RevenueAccrualEventV1`.
   - يعيد السجل الجديد + الاحداث.

### 6.2 تجديد خلال فترة grâce

تجديدات grace تشمل الطلب القياسي بالاضافة الى كشف العقوبات:

- Torii et `now` et `grace_expires_at` et supplément pour `SuffixPolicyV1`.
- اثبات الدفع يجب ان يغطي supplément. فشل => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` ou `expires_at`.

### 6.3 Utiliser le gardien et remplacer la fonction

1. Guardian يرسل `FreezeNameRequestV1` مع تذكرة تشير الى id حادث.
2. Torii correspond à `NameStatus::Frozen`, et `NameFrozen`.
3. بعد المعالجة، يصدر المجلس remplacement ; يرسل المشغل DELETE `/v1/sns/names/{namespace}/{literal}/freeze` ou `GovernanceHookV1`.
4. Torii est un remplacement pour `NameUnfrozen`.

## 7. التحقق واكواد الخطا| الكود | الوصف | HTTP |
|-------|-------|------|
| `sns_err_reserved` | العلامة محجوزة او محظورة. | 409 |
| `sns_err_policy_violation` | Les contrôleurs sont également compatibles avec les contrôleurs. | 422 |
| `sns_err_payment_mismatch` | Il s'agit d'un atout pour les actifs. | 402 |
| `sns_err_governance_missing` | اثار الحوكمة المطلوبة غائبة/غير صالحة. | 403 |
| `sns_err_state_conflict` | العملية غير مسموحة في حالة دورة الحياة الحالية. | 409 |

Utilisez la fonction `X-Iroha-Error-Code` et Norito JSON/NRPC.

## 8. ملاحظات التنفيذ

- Torii est compatible avec `NameRecordV1.auction` et `PendingAuction`.
- اثباتات الدفع تعيد استخدام ايصالات دفتر Norito؛ Les API sont utilisées pour les API (`/v1/finance/sns/payments`).
- ينبغي للـ SDK تغليف هذه النقاط بمساعدات قوية النوع حتى تتمكن المحافظ من عرض اسباب خطا واضحة (`ERR_SNS_RESERVED`, الخ).

## 9. الخطوات التالية

- ربط الجات Torii byعقد السجل الفعلي عندما تصل مزادات SN-3.
- Utilisez le SDK (Rust/JS/Swift) comme indiqué.
- توسيع [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) بروابط متقاطعة لحقول ادلة hooks الحوكمة.