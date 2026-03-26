---
lang: he
direction: rtl
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Esta pagina refleja `docs/source/sns/registrar_api.md` y ahora sirve como la
copia canonica del portal. El archivo fuente se mantiene para flujos de
traducion.
:::

# API של registrador SNS y hooks de gobernanza (SN-2b)

**Estado:** Borrador 2026-03-24 -- bajo revision de Nexus Core  
**Enlace del Roadmap:** SN-2b "Registrar API & Governance Hooks"  
**דרישות מוקדמות:** Definiciones de esquema en [`registry-schema.md`](./registry-schema.md)

יש לציין את נקודות הקצה Torii, שירותי gRPC, DTOs de solicitud
תשובה y artefactos de gobernanza necesarios para operar el registrador del
שירות שמות סורה (SNS). Es el contrato autoritativo para SDKs, ארנקים y
אוטומטיזציה que necesitan רשם, renovar או gestionar nombres SNS.

## 1. Transporte y autenticacion

| Requisito | Detalle |
|----------------|--------|
| פרוטוקולים | REST bajo `/v1/sns/*` y servicio gRPC `sns.v1.Registrar`. Ambos aceptan Norito-JSON (`application/json`) y Norito-RPC בינארי (`application/x-norito`). |
| Auth | אסימונים `Authorization: Bearer` או אישורי mTLS נשלחים לסיומת דייל. נקודות קצה sensibles a gobernanza (הקפאה/ביטול הקפאה, asignaciones reservadas) מחייבים `scope=sns.admin`. |
| Limites de tasa | הרשמים משווים את דליים `torii.preauth_scheme_limits` עם המתקשרים JSON mas limites de rafaga por סיומת: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| טלמטריה | Torii expone `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` עבור מטפלים לרשום (מסנן `scheme="norito_rpc"`); la API tambien incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. קורות חיים של DTO

Los campos referencian los structs canonicos definidos en [`registry-schema.md`](./registry-schema.md). Todas las cargas incluyen `NameSelectorV1` + `SuffixId` para evitar ruteo ambiguo.

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

## 3. נקודות קצה REST

| נקודת קצה | Metodo | מטען | תיאור |
|--------|--------|--------|----------------|
| `/v1/sns/names` | פוסט | `RegisterNameRequestV1` | רשם או reabrir un nombre. Resuelve la tier de precios, valida pruebas de pago/gobernanza, emite eventos de registro. |
| `/v1/sns/names/{namespace}/{literal}/renew` | פוסט | `RenewNameRequestV1` | טרמינו אקסטינדה. Aplica ventanas de gracia/redencion segun la politica. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | פוסט | `TransferNameRequestV1` | Transfiere propiedad una vez adjuntas las aprobaciones de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | Reemplaza el set de controls; valida direcciones de cuenta firmadas. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | פוסט | `FreezeNameRequestV1` | הקפאת האפוטרופוס/המועצה. דרוש כרטיס לאפוטרופוס ו-referencia al docket de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | מחק | `GovernanceHookV1` | בטל את הקפאת תיקון הטראס; asegura override del Council registrado. |
| `/v1/sns/reserved/{selector}` | פוסט | `ReservedAssignmentRequestV1` | Asignacion de nombres reservados por דייל/מועצה. |
| `/v1/sns/policies/{suffix_id}` | קבל | -- | השג `SuffixPolicyV1` בפועל (ניתן לאחסון במטמון). |
| `/v1/sns/names/{namespace}/{literal}` | קבל | -- | Devuelve `NameRecordV1` בפועל + אפקטיבי (Active, Grace וכו'). |**קוד הבורר:** מקטע `{selector}` מאפיין i105, קומפרימי או משושה קנוני מדגם ADDR-5; Torii נורמל דרך `NameSelectorV1`.

**מודל שגיאות:** todos los point endpoints devuelven Norito JSON con `code`, `message`, `details`. Los codigos כוללים `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (דרישת מדריך לרשם N0)

Los stewards de beta cerrada pueden אופר אל הרשום דרך la CLI sin armar JSON a mano:

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

- `--owner` toma por defecto la cuenta de configuracion de la CLI; repite `--controller` para adjuntar cuentas controller addictionales (ברירת מחדל `[owner]`).
- Los flags inline de pago mapean directo a `PaymentProofV1`; ארה"ב `--payment-json PATH` יש לך ניסיון. מטא-נתונים (`--metadata-json`) y governance hooks (`--governance-json`) סיואן el mismo patron.

מסייעים להרצאות סולו משלימות:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ver `crates/iroha_cli/src/commands/sns.rs` para la implementacion; los comandos reutilizan los DTOs Norito תיאורים en este documento para que la salida de la CLI coincida byte por byte con las respuestas de Torii.

עוזרי שיפוצים, העברות ועזרי אפוטרופוס:

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

`--governance-json` debe contener un registro `GovernanceHookV1` valid (מזהה הצעה, גיבוב של הצבעה, פירמס דייל/אפוטרופוס). Cada comando simplemente refleja el point end `/v1/sns/names/{namespace}/{literal}/...` correspondiente para que los operadores de beta ensayen exactamente las superficies Torii que llamaran los SDKs.

## 4. Servicio gRPC

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

פורמט חוט: hash del esquema Norito בכתובת הקומפילציה registrado en
`fixtures/norito_rpc/schema_hashes.json` (filas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1` וכו').

## 5. Hooks de gobernanza y evidencia

Cada llamada que muta estado debe adjuntar evidencia apta para replay:

| אקציון | Datos de gobernanza requeridos |
|--------|-------------------------------------|
| Registro/renovacion estandar | Prueba de pago que referencia una instruccion de settlement; no se requiere voto de Council salvo que la tier requiera aprobacion de steward. |
| Registro premium / signacion reservada | `GovernanceHookV1` מזהה הצעה להתייחסות + אישור דייל. |
| העברה | Hash de voto del Council + hash de senal DAO; אישור אפוטרופוס cuando la transferencia se active por resolucion de disputa. |
| הקפאה/בטל הקפאה | Firma de ticket guardian mas override del Council (ביטול הקפאה). |

Torii אימות לאס פרובאס קומפרובנדו:

1. זיהוי ההצעה existe en el Ledger de gobernanza (`/v1/governance/proposals/{id}`) y el status es `Approved`.
2. Hashes coinciden con los artefactos de voto registrados.
3. Firmas de steward/guardian referencian las claves publicas esperadas de `SuffixPolicyV1`.

Fallos devuelven `sns_err_governance_missing`.

## 6. דגמי הפלוג'ו

### 6.1 Registro estandar1. ייעוץ לקוחות `/v1/sns/policies/{suffix_id}` לפרטי רכישה, gracia y tiers disponibles.
2. El cliente arma `RegisterNameRequestV1`:
   - `selector` תווית i105 (מועדפת) או קומפרימיידו (אופציה חשובה).
   - `term_years` dentro de los limites de la politica.
   - `payment` que referencia la transferencia del splitter tesoreria/steward.
3. תוקף Torii:
   - Normalizacion de label + List reservada.
   - טווח/מחיר ברוטו לעומת `PriceTierV1`.
   - סכום הוכחה דה פאגו >= מחיר חישוב + עמלות.
4. יציאה Torii:
   - Persiste `NameRecordV1`.
   - Emite `RegistryEventV1::NameRegistered`.
   - Emite `RevenueAccrualEventV1`.
   - Devuelve el nuevo registro + אירועים.

### 6.2 Renovacion durante gracia

Las renovaciones en gracia incluyen la solicitud estandar mas deteccion de penalidad:

- Torii השוואת `now` לעומת `grace_expires_at` y agrega tablas de recargo desde `SuffixPolicyV1`.
- La prueba de pago debe cubrir el recargo. Fallo => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra el nuevo `expires_at`.

### 6.3 Congelamiento de guardian y override del Council

1. Guardian envia `FreezeNameRequestV1` עם כרטיס que referencia id de incidente.
2. Torii מועב אל רישום ל-`NameStatus::Frozen`, emite `NameFrozen`.
3. תיקון Tras, ביטול המועצה הפליטה; el operator envia DELETE `/v1/sns/names/{namespace}/{literal}/freeze` con `GovernanceHookV1`.
4. Torii valida el override, emite `NameUnfrozen`.

## 7. Validacion y codigos de error

| קודיגו | תיאור | HTTP |
|--------|-------------|------|
| `sns_err_reserved` | תווית reservado o bloqueado. | 409 |
| `sns_err_policy_violation` | מונח, דרג או סט פקחי ויולה לה פוליטיקה. | 422 |
| `sns_err_payment_mismatch` | חוסר התאמה דה חיל או נכס en la prueba de pago. | 402 |
| `sns_err_governance_missing` | Artefactos de gobernanza requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | Operacion no permitida en el estado actual del ciclo de vida. | 409 |

Todos los codigos salen via `X-Iroha-Error-Code` y envelopes Norito JSON/NRPC estructurados.

## 8. Notas de implementacion

- Torii guarda subastas pendientes en `NameRecordV1.auction` y rechaza intentos de registro directo mientras este en `PendingAuction`.
- Las pruebas de pago reutilizan recibos del ספר חשבונות Norito; Servicios de tesoreria עוזר ממשקי API (`/v1/finance/sns/payments`).
- Los SDKs deben envolver estos endpoints con helpers tipados para que las wallets muestren razones claras de error (`ERR_SNS_RESERVED`, וכו').

## 9. Proximos pasos

- Conectar los handlers de Torii al contrato de registro real una vez que lleguen las subastas SN-3.
- הסבר מפורט על SDK (Rust/JS/Swift) שמתייחס ל-API.
- Extender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) מצמיד קרוזאדוס ל-Los Campos de Evidencia de Governance Hook.