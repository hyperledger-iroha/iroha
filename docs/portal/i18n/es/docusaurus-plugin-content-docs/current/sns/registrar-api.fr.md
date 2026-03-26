---
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página refleja `docs/source/sns/registrar_api.md` y sirve desormais de
copia canónica del portal. El archivo fuente se conserva para el flujo de
traducción.
:::

# API del registrador SNS y ganchos de gobierno (SN-2b)

**Estado:** Redige 2026-03-24 -- en revista por Nexus Core  
**Hoja de ruta de gravámenes:** SN-2b "API de registrador y ganchos de gobernanza"  
**Requisitos previos:** Definiciones de esquema en [`registry-schema.md`](./registry-schema.md)

Esta nota especifica los puntos finales Torii, servicios gRPC, DTO de solicitud/respuesta
et artefactos de gobierno necesarios para operar el registrador de Sora
Servicio (SNS). Este es el contrato de referencia para los SDK, billeteras y
Automatización que permite registrar, renovar o administrar nombres SNS.

## 1. Transporte y autenticación| Exigencia | Detalle |
|----------|--------|
| Protocolos | REST en `/v1/sns/*` y servicio gRPC `sns.v1.Registrar`. Los dos aceptan el binario Norito-JSON (`application/json`) y Norito-RPC (`application/x-norito`). |
| Autenticación | Jetons `Authorization: Bearer` o certificados mTLS emis por sufijo administrador. Los puntos finales sensibles a la gobernanza (congelar/descongelar, afectaciones de reserva) exigen `scope=sns.admin`. |
| Límites de débito | Los registradores participan en los depósitos `torii.preauth_scheme_limits` con los apelantes JSON más los límites de rafale por sufijo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expone `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para los controladores del registrador (filtro `scheme="norito_rpc"`); La API aumenta además de `sns_registrar_status_total{result, suffix_id}`. |

## 2. Apertura del DTO

Los campos que hacen referencia a las estructuras canónicas se definen en [`registry-schema.md`](./registry-schema.md). Todos los cargos útiles integran `NameSelectorV1` + `SuffixId` para evitar una ruta ambigua.

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
|----------|---------|---------|-------------|
| `/v1/sns/names` | PUBLICAR | `RegisterNameRequestV1` | Registrar o buscar un nombre. Resout le tier de prix, valide les preuves de paiement/gouvernance, emet des eventements de registre. |
| `/v1/sns/names/{namespace}/{literal}/renew` | PUBLICAR | `RenewNameRequestV1` | Prolongar el plazo. Apliques les fenetres de gracia/redención después de la política. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | PUBLICAR | `TransferNameRequestV1` | Transfere la propriete une fois les approbations de gouvernance joints. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PONER | `UpdateControllersRequestV1` | Reemplace el conjunto de controladores; valide les adresses de compte signees. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | PUBLICAR | `FreezeNameRequestV1` | Congelar tutor/consejo. Exige un ticket guardian y una referencia en el expediente de gobierno. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | BORRAR | `GovernanceHookV1` | Descongelar después de la remediación; Asegúrese de que la anulación del consejo esté registrada. |
| `/v1/sns/reserved/{selector}` | PUBLICAR | `ReservedAssignmentRequestV1` | Afectación de nombres reservas par administrador/consejo. |
| `/v1/sns/policies/{suffix_id}` | OBTENER | -- | Recupere el flujo `SuffixPolicyV1` (almacenable en caché). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENER | -- | Regrese el curso `NameRecordV1` + estado efectivo (Active, Grace, etc.). |

**Codificación del selector:** el segmento `{selector}` acepta i105, comprimido o hexadecimal según ADDR-5; Torii archivo normalizado a través de `NameSelectorV1`.**Modelos de errores:** todos los puntos finales devuelven Norito JSON con `code`, `message`, `details`. Los códigos incluyen `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 Ayudas CLI (exigencia del registrador manual N0)

Los administradores de la beta cerrada pueden utilizar el registrador a través de la CLI sin fabricante de JSON en el main:

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

- `--owner` incluye de forma predeterminada la cuenta de configuración de la CLI; Repita `--controller` para agregar cuentas adicionales del controlador (por defecto `[owner]`).
- Les flags inline de paiement mappent directement vers `PaymentProofV1`; pase `--payment-json PATH` cuando tenga una estructura de recuperación. Les metadonnees (`--metadata-json`) et les hooks de gouvernance (`--governance-json`) siguientes al esquema de memes.

Las ayudas de lectura solo completan las repeticiones:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Ver `crates/iroha_cli/src/commands/sns.rs` para la implementación; Los comandos reutilizan los DTO Norito en este documento porque la salida CLI corresponde byte por byte a las respuestas Torii.

Des aides suplementaires couvrent les renouvellements, transferts et action guardian:

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
````--governance-json` debe contener un registro `GovernanceHookV1` válido (identificación de propuesta, hashes de voto, administrador/tutor de firmas). Cada comando refleja simplemente el punto final `/v1/sns/names/{namespace}/{literal}/...` correspondiente a los operadores de beta que pueden repetir exactamente las superficies Torii que aplican los SDK.

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

Formato de cable: hash del esquema Norito y en tiempo de compilación registrado en
`fixtures/norito_rpc/schema_hashes.json` (líneas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de gobierno y preuves

Chaque appel qui modifie l'etat doit joindre des preuves reutilisables pour la relecture:

| Acción | Demandas de gobierno |
|--------|-------------------------------|
| Norma de registro/renovación | Preuve de pago referente a una instrucción de liquidación; El consejo de votación de Aucun requis sauf si le tier exige la aprobación del administrador. |
| Registro de nivel premium / afectación reserva | `GovernanceHookV1` ID de propuesta de referencia + reconocimiento del administrador. |
| Transferencia | Hash du vote consejo + hash du señal DAO; Clearance Guardian quand le transfert est declinado par resolución de litigio. |
| Congelar/Descongelar | Firma del guardián del billete más anulación del consejo (descongelación). |

Torii verifique las preuves en verificativo:1. El identificador de propuesta existe en el libro mayor de gobierno (`/v1/governance/proposals/{id}`) y el estatuto es `Approved`.
2. Los hash correspondientes a los artefactos de registro de votos.
3. Les firmas administrador/guardián referenciant les cles publiques asistentes de `SuffixPolicyV1`.

Les controles en echec renvoient `sns_err_governance_missing`.

## 6. Ejemplos de flujo de trabajo

### 6.1 Estándar de registro

1. El cliente interroga `/v1/sns/policies/{suffix_id}` para recuperar los precios, la gracia y los niveles disponibles.
2. El cliente construye `RegisterNameRequestV1`:
   - `selector` deriva la etiqueta i105 (preferir) o comprimir (segunda opción).
   - `term_years` en los límites de la política.
   - `payment` referente a la transferencia del divisor tresorerie/steward.
3. Torii válido:
   - Normalización del sello + lista reservada.
   - Plazo/precio bruto vs `PriceTierV1`.
   - Montant de preuve de paiement >= precio calculado + frais.
4. En éxito Torii:
   - Persiste `NameRecordV1`.
   -Emet `RegistryEventV1::NameRegistered`.
   -Emet `RevenueAccrualEventV1`.
   - Retourne le nouveau record + eventos.

### 6.2 Renovación colgante la gracia

Las renovaciones pendientes de gracia incluyen la petición estándar más la detección de pena:- Torii compara `now` con `grace_expires_at` y agrega las tablas de recargos de `SuffixPolicyV1`.
- El pago previo debe cubrir el recargo. Echec => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registre el nuevo `expires_at`.

### 6.3 Congelar guardián y anular el consejo

1. Guardian soumet `FreezeNameRequestV1` con un ticket referente al identificador del incidente.
2. Torii reemplaza el registro en `NameStatus::Frozen`, emet `NameFrozen`.
3. Después de la remediación, el consejo emitió una anulación; El operador envió DELETE `/v1/sns/names/{namespace}/{literal}/freeze` con `GovernanceHookV1`.
4. Torii valida la anulación, emet `NameUnfrozen`.

## 7. Validación y códigos de error

| Código | Descripción | HTTP |
|------|-------------|------|
| `sns_err_reserved` | Etiqueta reserva o bloque. | 409 |
| `sns_err_policy_violation` | Término, nivel o conjunto de controladores que violan la política. | 422 |
| `sns_err_payment_mismatch` | Desajuste de valor o activo en el precio de pago. | 402 |
| `sns_err_governance_missing` | Artefactos de gobierno requeridos ausentes/inválidos. | 403 |
| `sns_err_state_conflict` | Operation non permise dans l'etat de Cycle de vie actual. | 409 |

Todos los códigos aparecen a través de `X-Iroha-Error-Code` y las estructuras Norito JSON/NRPC.

## 8. Notas de implementación- Torii almacena las subastas en atención bajo `NameRecordV1.auction` y rechaza las tentativas de registro directo como `PendingAuction`.
- Les preuves de paiement reutilisent les recus du ledger Norito; Les Services de Tresorerie Fournissent des API Helper (`/v1/finance/sns/payments`).
- Los SDK envuelven estos puntos finales con ayudantes de tipos fuertes para que las billeteras puedan presentar las razones del error claro (`ERR_SNS_RESERVED`, etc.).

## 9. Etapas de prochaines

- Confíe en los manipuladores Torii en el contrato de registro del carrete una de las subastas SN-3 disponibles.
- Publicación de guías SDK específicas (Rust/JS/Swift) referentes a esta API.
- Etendre [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) con los gravámenes croises vers les champs de preuve des hooks de gouvernance.