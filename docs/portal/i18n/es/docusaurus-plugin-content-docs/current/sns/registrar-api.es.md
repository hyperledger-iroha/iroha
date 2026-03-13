---
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta pagina refleja `docs/source/sns/registrar_api.md` y ahora sirve como la
copia canónica del portal. El archivo fuente se mantiene para flujos de
traducción.
:::

# API del registrador SNS y ganchos de gobernanza (SN-2b)

**Estado:** Borrador 2026-03-24 -- bajo revisión de Nexus Core  
**Enlace del roadmap:** SN-2b "API de registrador y ganchos de gobernanza"  
**Prerrequisitos:** Definiciones de esquema en [`registry-schema.md`](./registry-schema.md)

Esta nota especifica los endpoints Torii, servicios gRPC, DTOs de solicitud
respuesta y artefactos de gobernanza necesarios para operar el registrador del
Servicio de nombres de Sora (SNS). Es el contrato autoritativo para SDKs, wallets y
automatizacion que necesitan registrar, renovar o gestionar nombres SNS.

## 1. Transporte y autenticación| Requisito | Detalle |
|-----------|------------------|
| Protocolos | REST bajo `/v2/sns/*` y servicio gRPC `sns.v1.Registrar`. Ambos aceptan Norito-JSON (`application/json`) y Norito-RPC binario (`application/x-norito`). |
| Autenticación | Tokens `Authorization: Bearer` o certificados mTLS emitidos por sufijo steward. Los puntos finales sensibles a gobernanza (congelar/descongelar, asignaciones reservadas) requieren `scope=sns.admin`. |
| Límites de tasa | Los registradores comparten los buckets `torii.preauth_scheme_limits` con llamantes JSON mas limites de rafaga por sufijo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii exponen `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para manejadores del registrador (filtrar `scheme="norito_rpc"`); La API también incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Resumen de DTO

Los campos hacen referencia a las estructuras canónicas definidas en [`registry-schema.md`](./registry-schema.md). Todas las cargas incluyen `NameSelectorV1` + `SuffixId` para evitar ruteo ambiguo.

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

## 3. Puntos finales DESCANSO| Punto final | Método | Carga útil | Descripción |
|----------|--------|---------|-------------|
| `/v2/sns/registrations` | PUBLICAR | `RegisterNameRequestV1` | Registrar o reabrir un nombre. Resuelve el nivel de precios, valida pruebas de pago/gobernanza, emite eventos de registro. |
| `/v2/sns/registrations/{selector}/renew` | PUBLICAR | `RenewNameRequestV1` | Extender término. Aplica ventanas de gracia/redencion segun la politica. |
| `/v2/sns/registrations/{selector}/transfer` | PUBLICAR | `TransferNameRequestV1` | Transfiere propiedad una vez adjuntas las aprobaciones de gobernanza. |
| `/v2/sns/registrations/{selector}/controllers` | PONER | `UpdateControllersRequestV1` | Reemplaza el conjunto de controladores; valida direcciones de cuenta firmadas. |
| `/v2/sns/registrations/{selector}/freeze` | PUBLICAR | `FreezeNameRequestV1` | Congelar al tutor/consejo. Requiere ticket de guardian y referencia al expediente de gobernanza. |
| `/v2/sns/registrations/{selector}/freeze` | BORRAR | `GovernanceHookV1` | Descongelar tras remediacion; asegura override del consejo registrado. |
| `/v2/sns/reserved/{selector}` | PUBLICAR | `ReservedAssignmentRequestV1` | Asignación de nombres reservados por administrador/consejo. |
| `/v2/sns/policies/{suffix_id}` | OBTENER | -- | Obtiene `SuffixPolicyV1` real (almacenable en caché). |
| `/v2/sns/registrations/{selector}` | OBTENER | -- | Devuelve `NameRecordV1` actual + estado efectivo (Active, Grace, etc.). |

**Codificación de selector:** el segmento `{selector}` acepta I105, comprimido o hex canonico según ADDR-5; Torii lo normaliza vía `NameSelectorV1`.**Modelo de errores:** todos los endpoints devuelven Norito JSON con `code`, `message`, `details`. Los códigos incluyen `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI de ayuda (requisito de registrador manual N0)

Los stewards de beta cerrada pueden operar el registrador vía la CLI sin armar JSON a mano:

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

- `--owner` toma por defecto la cuenta de configuración de la CLI; repite `--controller` para adjuntar cuentas controlador adicionales (predeterminado `[owner]`).
- Los flags inline de pago mapean directo a `PaymentProofV1`; usa `--payment-json PATH` cuando ya tengas una estructura recibidodo. Metadatos (`--metadata-json`) y ganchos de gobernanza (`--governance-json`) siguen el mismo patrón.

Helpers de solo lectura completan los ensayos:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ver `crates/iroha_cli/src/commands/sns.rs` para la implementación; los comandos reutilizan los DTO Norito descritos en este documento para que la salida de la CLI coincida byte por byte con las respuestas de Torii.

Ayudantes adicionales cubren renovaciones, traslados y acciones de tutoría:

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
````--governance-json` debe contener un registro `GovernanceHookV1` válido (id de propuesta, hashes de voto, firmas de administrador/tutor). Cada comando simplemente refleja el endpoint `/v2/sns/registrations/{selector}/...` correspondiente para que los operadores de beta ensayen exactamente las superficies Torii que llamarán los SDK.

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

Formato de cable: hash del esquema Norito en tiempo de compilación registrado en
`fixtures/norito_rpc/schema_hashes.json` (filas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de gobernanza y evidencia

Cada llamada que muta estado debe adjuntar evidencia apta para repetición:

| Acción | Datos de gobernanza requeridos |
|--------|-------------------------------|
| Registro/renovación estándar | Prueba de pago que hace referencia a una instrucción de liquidación; no se requiere voto de consejo salvo que la tier requiera aprobación de mayordomo. |
| Registro premium / asignación reservada | `GovernanceHookV1` que hace referencia a la identificación de la propuesta + reconocimiento del administrador. |
| Traslado | Hash de voto del consejo + hash de senal DAO; autorización del tutor cuando la transferencia se activa por resolución de disputa. |
| Congelar/Descongelar | Firma de ticket guardian mas override del consejo (descongelar). |

Torii verifica las pruebas comprobando:1. La identificación de la propuesta existe en el libro mayor de gobernanza (`/v2/governance/proposals/{id}`) y el estado es `Approved`.
2. Los hashes coinciden con los artefactos de voto registrados.
3. Firmas de steward/guardian referencian las claves públicas esperadas de `SuffixPolicyV1`.

Fallos devuelven `sns_err_governance_missing`.

## 6. Ejemplos de flujo

### 6.1 Registro estándar

1. El cliente consulta `/v2/sns/policies/{suffix_id}` para obtener precios, gracia y niveles disponibles.
2. El cliente arma `RegisterNameRequestV1`:
   - `selector` derivado de etiqueta I105 (preferido) o comprimido (segunda mejor opción).
   - `term_years` dentro de los límites de la política.
   - `payment` que referencia la transferencia del splitter tesoreria/steward.
3. Torii validado:
   - Normalización de etiqueta + lista reservada.
   - Plazo/precio bruto vs `PriceTierV1`.
   - Monto del comprobante de pago >= precio calculado + honorarios.
4. En salida Torii:
   - Persiste `NameRecordV1`.
   - Emite `RegistryEventV1::NameRegistered`.
   - Emite `RevenueAccrualEventV1`.
   - Devuelve el nuevo registro + eventos.

### 6.2 Renovación durante gracia

Las renovaciones en gracia incluyen la solicitud estándar mas detección de penalidad:

- Torii compara `now` vs `grace_expires_at` y agrega tablas de recarga desde `SuffixPolicyV1`.
- La prueba de pago debe cubrir el recargo. Fallo => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra el nuevo `expires_at`.### 6.3 Congelamiento de guardián y anulación del consejo

1. Guardian envió `FreezeNameRequestV1` con ticket que referencia id de incidente.
2. Torii mueve el registro a `NameStatus::Frozen`, emite `NameFrozen`.
3. Tras la remediación, el consejo emite anulación; el operador envia DELETE `/v2/sns/registrations/{selector}/freeze` con `GovernanceHookV1`.
4. Torii valida el override, emite `NameUnfrozen`.

## 7. Validación y códigos de error

| Código | Descripción | HTTP |
|--------|-------------|------|
| `sns_err_reserved` | Etiqueta reservada o bloqueada. | 409 |
| `sns_err_policy_violation` | Término, nivel o conjunto de controladores viola la política. | 422 |
| `sns_err_payment_mismatch` | Desajuste de valor o activo en la prueba de pago. | 402 |
| `sns_err_governance_missing` | Artefactos de gobernanza requeridos ausentes/inválidos. | 403 |
| `sns_err_state_conflict` | Operación no permitida en el estado actual del ciclo de vida. | 409 |

Todos los códigos salen vía `X-Iroha-Error-Code` y sobres Norito JSON/NRPC estructurados.

## 8. Notas de implementación

- Torii guarda subastas pendientes en `NameRecordV1.auction` y rechaza intentos de registro directo mientras este en `PendingAuction`.
- Las pruebas de pago reutilizan recibos del libro mayor Norito; servicios de tesoreria proveen APIs helper (`/v2/finance/sns/payments`).
- Los SDK deben incluir estos puntos finales con ayudantes tipados para que las billeteras muestren razones claras de error (`ERR_SNS_RESERVED`, etc.).## 9. Proximos pasos

- Conectar los handlers de Torii al contrato de registro real una vez que lleguen las subastas SN-3.
- Publicar guías específicas de SDK (Rust/JS/Swift) que hacen referencia a esta API.
- Extender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) con enlaces cruzados a los campos de evidencia de Governance Hook.