---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sns/registrar-api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 466e827e34d24d9f79f43fd990797a2af66fcc0c5eae4ef591fee1abcb4e229b
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: registrar-api
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Esta pagina refleja `docs/source/sns/registrar_api.md` y ahora sirve como la
copia canonica del portal. El archivo fuente se mantiene para flujos de
traduccion.
:::

# API del registrador SNS y hooks de gobernanza (SN-2b)

**Estado:** Borrador 2026-03-24 -- bajo revision de Nexus Core  
**Enlace del roadmap:** SN-2b "Registrar API & governance hooks"  
**Prerequisitos:** Definiciones de esquema en [`registry-schema.md`](./registry-schema.md)

Esta nota especifica los endpoints Torii, servicios gRPC, DTOs de solicitud
respuesta y artefactos de gobernanza necesarios para operar el registrador del
Sora Name Service (SNS). Es el contrato autoritativo para SDKs, wallets y
automatizacion que necesitan registrar, renovar o gestionar nombres SNS.

## 1. Transporte y autenticacion

| Requisito | Detalle |
|-----------|---------|
| Protocolos | REST bajo `/v1/sns/*` y servicio gRPC `sns.v1.Registrar`. Ambos aceptan Norito-JSON (`application/json`) y Norito-RPC binario (`application/x-norito`). |
| Auth | Tokens `Authorization: Bearer` o certificados mTLS emitidos por suffix steward. Endpoints sensibles a gobernanza (freeze/unfreeze, asignaciones reservadas) requieren `scope=sns.admin`. |
| Limites de tasa | Registrars comparten los buckets `torii.preauth_scheme_limits` con callers JSON mas limites de rafaga por suffix: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expone `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para handlers del registrador (filtrar `scheme="norito_rpc"`); la API tambien incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Resumen de DTO

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

## 3. Endpoints REST

| Endpoint | Metodo | Payload | Descripcion |
|----------|--------|---------|-------------|
| `/v1/sns/names` | POST | `RegisterNameRequestV1` | Registrar o reabrir un nombre. Resuelve la tier de precios, valida pruebas de pago/gobernanza, emite eventos de registro. |
| `/v1/sns/names/{namespace}/{literal}/renew` | POST | `RenewNameRequestV1` | Extiende termino. Aplica ventanas de gracia/redencion segun la politica. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | POST | `TransferNameRequestV1` | Transfiere propiedad una vez adjuntas las aprobaciones de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PUT | `UpdateControllersRequestV1` | Reemplaza el set de controllers; valida direcciones de cuenta firmadas. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | POST | `FreezeNameRequestV1` | Freeze de guardian/council. Requiere ticket de guardian y referencia al docket de gobernanza. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | DELETE | `GovernanceHookV1` | Unfreeze tras remediacion; asegura override del council registrado. |
| `/v1/sns/reserved/{selector}` | POST | `ReservedAssignmentRequestV1` | Asignacion de nombres reservados por steward/council. |
| `/v1/sns/policies/{suffix_id}` | GET | -- | Obtiene `SuffixPolicyV1` actual (cacheable). |
| `/v1/sns/names/{namespace}/{literal}` | GET | -- | Devuelve `NameRecordV1` actual + estado efectivo (Active, Grace, etc.). |

**Codificacion de selector:** el segmento `{selector}` acepta I105, comprimido o hex canonico segun ADDR-5; Torii lo normaliza via `NameSelectorV1`.

**Modelo de errores:** todos los endpoints devuelven Norito JSON con `code`, `message`, `details`. Los codigos incluyen `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Helpers CLI (requisito de registrador manual N0)

Los stewards de beta cerrada pueden operar el registrador via la CLI sin armar JSON a mano:

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

- `--owner` toma por defecto la cuenta de configuracion de la CLI; repite `--controller` para adjuntar cuentas controller adicionales (default `[owner]`).
- Los flags inline de pago mapean directo a `PaymentProofV1`; usa `--payment-json PATH` cuando ya tengas un recibo estructurado. Metadata (`--metadata-json`) y governance hooks (`--governance-json`) siguen el mismo patron.

Helpers de solo lectura completan los ensayos:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ver `crates/iroha_cli/src/commands/sns.rs` para la implementacion; los comandos reutilizan los DTOs Norito descritos en este documento para que la salida de la CLI coincida byte por byte con las respuestas de Torii.

Helpers adicionales cubren renovaciones, transfers y acciones de guardian:

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

`--governance-json` debe contener un registro `GovernanceHookV1` valido (proposal id, vote hashes, firmas de steward/guardian). Cada comando simplemente refleja el endpoint `/v1/sns/names/{namespace}/{literal}/...` correspondiente para que los operadores de beta ensayen exactamente las superficies Torii que llamaran los SDKs.

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

Wire-format: hash del esquema Norito en tiempo de compilacion registrado en
`fixtures/norito_rpc/schema_hashes.json` (filas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Hooks de gobernanza y evidencia

Cada llamada que muta estado debe adjuntar evidencia apta para replay:

| Accion | Datos de gobernanza requeridos |
|--------|-------------------------------|
| Registro/renovacion estandar | Prueba de pago que referencia una instruccion de settlement; no se requiere voto de council salvo que la tier requiera aprobacion de steward. |
| Registro premium / asignacion reservada | `GovernanceHookV1` que referencia proposal id + steward acknowledgement. |
| Transfer | Hash de voto del council + hash de senal DAO; guardian clearance cuando la transferencia se activa por resolucion de disputa. |
| Freeze/Unfreeze | Firma de ticket guardian mas override del council (unfreeze). |

Torii verifica las pruebas comprobando:

1. Proposal id existe en el ledger de gobernanza (`/v1/governance/proposals/{id}`) y el status es `Approved`.
2. Hashes coinciden con los artefactos de voto registrados.
3. Firmas de steward/guardian referencian las claves publicas esperadas de `SuffixPolicyV1`.

Fallos devuelven `sns_err_governance_missing`.

## 6. Ejemplos de flujo

### 6.1 Registro estandar

1. El cliente consulta `/v1/sns/policies/{suffix_id}` para obtener precios, gracia y tiers disponibles.
2. El cliente arma `RegisterNameRequestV1`:
   - `selector` derivado de label I105 (preferido) o comprimido (segunda mejor opcion).
   - `term_years` dentro de los limites de la politica.
   - `payment` que referencia la transferencia del splitter tesoreria/steward.
3. Torii valida:
   - Normalizacion de label + lista reservada.
   - Term/gross price vs `PriceTierV1`.
   - Proof de pago amount >= precio calculado + fees.
4. En exito Torii:
   - Persiste `NameRecordV1`.
   - Emite `RegistryEventV1::NameRegistered`.
   - Emite `RevenueAccrualEventV1`.
   - Devuelve el nuevo registro + eventos.

### 6.2 Renovacion durante gracia

Las renovaciones en gracia incluyen la solicitud estandar mas deteccion de penalidad:

- Torii compara `now` vs `grace_expires_at` y agrega tablas de recargo desde `SuffixPolicyV1`.
- La prueba de pago debe cubrir el recargo. Fallo => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra el nuevo `expires_at`.

### 6.3 Congelamiento de guardian y override del council

1. Guardian envia `FreezeNameRequestV1` con ticket que referencia id de incidente.
2. Torii mueve el registro a `NameStatus::Frozen`, emite `NameFrozen`.
3. Tras remediacion, el council emite override; el operador envia DELETE `/v1/sns/names/{namespace}/{literal}/freeze` con `GovernanceHookV1`.
4. Torii valida el override, emite `NameUnfrozen`.

## 7. Validacion y codigos de error

| Codigo | Descripcion | HTTP |
|--------|-------------|------|
| `sns_err_reserved` | Label reservado o bloqueado. | 409 |
| `sns_err_policy_violation` | Term, tier o set de controllers viola la politica. | 422 |
| `sns_err_payment_mismatch` | Mismatch de valor o asset en la prueba de pago. | 402 |
| `sns_err_governance_missing` | Artefactos de gobernanza requeridos ausentes/invalidos. | 403 |
| `sns_err_state_conflict` | Operacion no permitida en el estado actual del ciclo de vida. | 409 |

Todos los codigos salen via `X-Iroha-Error-Code` y envelopes Norito JSON/NRPC estructurados.

## 8. Notas de implementacion

- Torii guarda subastas pendientes en `NameRecordV1.auction` y rechaza intentos de registro directo mientras este en `PendingAuction`.
- Las pruebas de pago reutilizan recibos del ledger Norito; servicios de tesoreria proveen APIs helper (`/v1/finance/sns/payments`).
- Los SDKs deben envolver estos endpoints con helpers tipados para que las wallets muestren razones claras de error (`ERR_SNS_RESERVED`, etc.).

## 9. Proximos pasos

- Conectar los handlers de Torii al contrato de registro real una vez que lleguen las subastas SN-3.
- Publicar guias especificas de SDK (Rust/JS/Swift) que referencien esta API.
- Extender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) con enlaces cruzados a los campos de evidencia de governance hook.
