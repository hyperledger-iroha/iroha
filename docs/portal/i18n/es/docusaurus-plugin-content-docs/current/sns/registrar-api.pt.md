---
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Esta página espelha `docs/source/sns/registrar_api.md` y ahora sirve como a
copia canónica del portal. El archivo fuente permanece para flujos de traducción.
:::

# API del registrador SNS y ganchos de gobierno (SN-2b)

**Estado:** Redigido 2026-03-24 -- sob revisao do Nexus Core  
**Enlace a hoja de ruta:** SN-2b "API de registrador y ganchos de gobernanza"  
**Requisitos previos:** Definicos de esquema en [`registry-schema.md`](./registry-schema.md)

Esta nota especifica los puntos finales Torii, servicios gRPC, DTO de solicitud/respuesta y
Artefatos de gobierno necesarios para operar o registrar do Sora Name Service (SNS).
E o contrato autoritativo para SDK, billeteras y registrador automático que necesita,
renovar o gerenciar nombres SNS.

## 1. Transporte y autenticacao| Requisito | Detalles |
|-----------|------------------|
| Protocolos | REST sollozo `/v1/sns/*` y servicio gRPC `sns.v1.Registrar`. Ambos incluyen Norito-JSON (`application/json`) e Norito-RPC binario (`application/x-norito`). |
| Autenticación | Tokens `Authorization: Bearer` o certificados mTLS emitidos por sufijo Steward. Los puntos finales sensibles son una gobernanza (congelar/descongelar, atribuicos reservados) exigem `scope=sns.admin`. |
| Límites de taxones | Los registradores comparten los cubos `torii.preauth_scheme_limits` con los chamadores JSON más límites de ráfaga por sufijo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetria | Torii expoe `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para manejadores del registrador (filtrar `scheme="norito_rpc"`); Una API también incrementa `sns_registrar_status_total{result, suffix_id}`. |

## 2. Visao general de DTO

Los campos hacen referencia a las estructuras canónicas definidas en [`registry-schema.md`](./registry-schema.md). Todas las cargas incluyen `NameSelectorV1` + `SuffixId` para evitar roteamento ambiguo.

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
|----------|--------|---------|-----------|
| `/v1/sns/names` | PUBLICAR | `RegisterNameRequestV1` | Registrar o reabrir un nombre. Resuelva el nivel de precos, valide las pruebas de pago/gobernanza, emita eventos de registro. |
| `/v1/sns/names/{namespace}/{literal}/renew` | PUBLICAR | `RenewNameRequestV1` | Estende o termo. Aplica janelas de gracia/redención da política. |
| `/v1/sns/names/{namespace}/{literal}/transfer` | PUBLICAR | `TransferNameRequestV1` | Transfere propriedade quando aprovacoes degobernanza forem anexadas. |
| `/v1/sns/names/{namespace}/{literal}/controllers` | PONER | `UpdateControllersRequestV1` | Sustitutos o conjuntos de controladores; valida enderecos de conta assinados. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | PUBLICAR | `FreezeNameRequestV1` | Congelar al tutor/consejo. Solicite ticket guardian e referencia al expediente de gobierno. |
| `/v1/sns/names/{namespace}/{literal}/freeze` | BORRAR | `GovernanceHookV1` | Descongelar apos remediacao; garantía de anulación del consejo registrado. |
| `/v1/sns/reserved/{selector}` | PUBLICAR | `ReservedAssignmentRequestV1` | Atribuicao de nomes reservados por administrador/consejo. |
| `/v1/sns/policies/{suffix_id}` | OBTENER | -- | Busca `SuffixPolicyV1` atual (cacheavel). |
| `/v1/sns/names/{namespace}/{literal}` | OBTENER | -- | Retorna `NameRecordV1` atual + estado efectivo (Active, Grace, etc.). |

**Codificación del selector:** o segmento `{selector}` aceita i105, comprimido o hex canonico conforme ADDR-5; Torii se normaliza vía `NameSelectorV1`.**Modelo de errores:** todos los puntos finales regresan Norito JSON con `code`, `message`, `details`. Los códigos incluyen `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI de ayuda (requisito del manual del registrador N0)

Administradores de fechada beta ahora pueden operar o registrar a través de CLI sin montar JSON manualmente:

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

- `--owner` padrao e a conta de configuracao da CLI; Repita `--controller` para conectar el controlador adicional (padrao `[owner]`).
- Las banderas en línea de pago mapeiam directo para `PaymentProofV1`; passe `--payment-json PATH` quando voce ja tiver um recibo estructurado. Metadados (`--metadata-json`) y ganchos de gobierno (`--governance-json`) siguen o mesmo padrao.

Helpers de leitura somente completam os ensaios:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

Veja `crates/iroha_cli/src/commands/sns.rs` para implementar; Los comandos reutilizan los DTO Norito descritos en este documento para que dicho CLI coincida byte por byte con las respuestas de Torii.

Helpers adicionais cobrem renovacoes, transferencias e acoes de guardian:

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
```

`--governance-json` debe conter um registro `GovernanceHookV1` válido (identificación de propuesta, hashes de voto, administrador/tutor de funciones). Cada comando simplemente configura el punto final `/v1/sns/names/{namespace}/{literal}/...` correspondiente para que los operadores de beta ensaiem exactamente como las superficies Torii que os SDK chamarao.## 4. Servicio gRPC

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

Formato de cable: hash del esquema Norito en el tiempo de compilación registrado en
`fixtures/norito_rpc/schema_hashes.json` (líneas `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, etc.).

## 5. Ganchos de gobernanza y evidencias

Toda chamada que altera estado deve anexar evidencias adecuadas para reproducir:

| Acao | Dados de gobierno requeridos |
|------|-------------------------------|
| Registro/renovacao padrao | Prova de pago referenciando uma instrucao de asentamiento; nao exige voto do consejo a menos que o tier exija aprovacao do mayordomo. |
| Registro de nivel premium / atribuicao reservada | `GovernanceHookV1` referenciando id de propuesta + reconocimiento del administrador. |
| Transferencia | Hash de voto do consejo + hash de sinal DAO; autorización del tutor cuando a transferencia e accionada por resolución de disputa. |
| Congelar/Descongelar | Assinatura do ticket guardian mais anula el consejo (descongelar). |

Torii verifica como prueba de conferencia:

1. La identificación de la propuesta no existe en el libro mayor de gobierno (`/v1/governance/proposals/{id}`) y en el estado e `Approved`.
2. Los hashes corresponden a los artefatos de voto registrados.
3. Assinaturas steward/tutor referenciam as chaves publicas esperadas de `SuffixPolicyV1`.

Falhas retornam `sns_err_governance_missing`.

## 6. Ejemplos de flujo de trabajo

### 6.1 Registro padrón1. O cliente consulta `/v1/sns/policies/{suffix_id}` para obtener precios, gracia y niveles disponibles.
2. O cliente monta `RegisterNameRequestV1`:
   - `selector` derivado de la etiqueta i105 (preferido) ou comprimido (segunda melhor opcao).
   - `term_years` dentro de los límites de la política.
   - `payment` referenciando a transferencia do splitter tesouraria/steward.
3. Torii validado:
   - Normalizacao de etiqueta + lista reservada.
   - Plazo/precio bruto vs `PriceTierV1`.
   - Importe de la prueba de pago >= preco calculado + honorarios.
4. En el éxito Torii:
   - Persiste `NameRecordV1`.
   - Emite `RegistryEventV1::NameRegistered`.
   - Emite `RevenueAccrualEventV1`.
   - Retorna o novo registro + eventos.

### 6.2 Renovación durante la gracia

Las renovaciones durante la gracia incluyen a requisicao padrao mais deteccao de penalidade:

- Torii compara `now` vs `grace_expires_at` y agrega tablas de sobretaxa de `SuffixPolicyV1`.
- A prueba de pago deve cobrar a sobretaxa. Falha => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` registra el nuevo `expires_at`.

### 6.3 Congelar al tutor y anular el consejo

1. Guardian envió `FreezeNameRequestV1` con ticket referenciando id de incidente.
2. Torii mueve el registro para `NameStatus::Frozen`, emite `NameFrozen`.
3. Apos remediacao, el consejo emite anulación; El operador envía DELETE `/v1/sns/names/{namespace}/{literal}/freeze` con `GovernanceHookV1`.
4. Torii valida o anula, emite `NameUnfrozen`.## 7. Validacao e codigos de error

| Código | Descripción | HTTP |
|--------|-----------|------|
| `sns_err_reserved` | Etiqueta reservada o bloqueada. | 409 |
| `sns_err_policy_violation` | Termo, tier ou conjunto de controladores violan la política. | 422 |
| `sns_err_payment_mismatch` | Desajuste de valor o activo en la prueba de pago. | 402 |
| `sns_err_governance_missing` | Artefatos de gobierno requeridos ausentes/inválidos. | 403 |
| `sns_err_state_conflict` | Operacao nao permitida no estado actual del ciclo de vida. | 409 |

Todos los códigos aparecen vía `X-Iroha-Error-Code` y sobres Norito JSON/NRPC estructurados.

## 8. Notas de implementación

- Torii armazena leiloes pendentes em `NameRecordV1.auction` e rejeita tentativas de registro directo enquanto estiver `PendingAuction`.
- Provas de pago reutilizam recibos do ledger Norito; Ayudante de API de servicios de tesouraria fornecem (`/v1/finance/sns/payments`).
- Los SDK deben involucrar estos puntos finales con ayudantes fuertemente tipados para que las billeteras presenten motivos claros de error (`ERR_SNS_RESERVED`, etc.).

## 9. Próximos pasos

- Conectar os handlers do Torii al contrato de registro real cuando os leiloes SN-3 chegarem.
- Publicar guías específicas de SDK (Rust/JS/Swift) referenciando esta API.
- Estender [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) con enlaces cruzados para os campos de evidencia de gobernanza gancho.