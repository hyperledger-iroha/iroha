---
lang: es
direction: ltr
source: docs/portal/docs/sns/registrar-api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está reemplazada por `docs/source/sns/registrar_api.md` y un cable de conexión
канонической копией портала. Исходный файл остается для PR переводов.
:::

# Registro API de SNS y actualización de aplicaciones (SN-2b)

**Estado:** Podgotovlen 2026-03-24 — en рассмотрении Nexus Core  
**Hoja de ruta de Ссылка:** SN-2b "API de registrador y ganchos de gobernanza"  
**Предпосылки:** Определения схемы в [`registry-schema.md`](./registry-schema.md)

Esta es la descripción de los componentes Torii, servicios gRPC, DTO запросов/ответов y
артефакты управления, необходимые для работы регистратора Sora Name Service
(Redes Sociales). Este contrato de autor para SDK, codificadores y automatizadores, códigos
No es necesario registrarse, actualizar o actualizar los mensajes SNS.

## 1. Transporte y autenticación| Требование | Detalles |
|------------|--------|
| Protocolos | REST под `/v1/sns/*` y gRPC сервис `sns.v1.Registrar`. Tiene el código Norito-JSON (`application/json`) y el propio Norito-RPC (`application/x-norito`). |
| Autenticación | `Authorization: Bearer` токены или mTLS сертификаты, выданные по sufijo administrador. Чувствительные к управлению эндпоинты (congelar/descongelar, asignaciones reservadas) требуют `scope=sns.admin`. |
| Organización | registrador делят cubos `torii.preauth_scheme_limits` с JSON вызывающими плюс лимиты всплеска по sufijo: `sns.register`, `sns.renew`, `sns.controller`, `sns.freeze`. |
| Telemetría | Torii publica `torii_request_duration_seconds{scheme}` / `torii_request_failures_total{scheme,code}` para el registrador de trabajadores (filtro de `scheme="norito_rpc"`); La API utiliza `sns_registrar_status_total{result, suffix_id}`. |

## 2. Objeto DTO

Поля ссылаются на канонические структуры, определенные в [`registry-schema.md`](./registry-schema.md). Все payloads встраивают `NameSelectorV1` + `SuffixId`, чтобы избежать неоднозначной маршрутизации.

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

## 3. DESCANSO эндпоинты| Punto | Método | Carga útil | Descripción |
|----------|-------|---------|----------|
| `/v1/sns/registrations` | PUBLICAR | `RegisterNameRequestV1` | Regístrese o regístrese previamente. Разрешает ценовой tier, проверяет доказательства платежа/управления, испускает события реестра. |
| `/v1/sns/registrations/{selector}/renew` | PUBLICAR | `RenewNameRequestV1` | Продлевает срок. Применяет окна gracia/redención из политики. |
| `/v1/sns/registrations/{selector}/transfer` | PUBLICAR | `TransferNameRequestV1` | Evite la descarga después de una actualización de la configuración. |
| `/v1/sns/registrations/{selector}/controllers` | PONER | `UpdateControllersRequestV1` | Заменяет набор controladores; проверяет подписанные адреса аккаунтов. |
| `/v1/sns/registrations/{selector}/freeze` | PUBLICAR | `FreezeNameRequestV1` | Congelar tutor/consejo. Требует guardián billete y ссылку на expediente de gobernanza. |
| `/v1/sns/registrations/{selector}/freeze` | BORRAR | `GovernanceHookV1` | Descongelar después del mantenimiento; убеждается, что consejo anulación зафиксирован. |
| `/v1/sns/reserved/{selector}` | PUBLICAR | `ReservedAssignmentRequestV1` | Назначение reservado имен mayordomo/consejo. |
| `/v1/sns/policies/{suffix_id}` | OBTENER | -- | Pulse el botón `SuffixPolicyV1` (кэшируемо). |
| `/v1/sns/registrations/{selector}` | OBTENER | -- | Возвращает текущий `NameRecordV1` + эффективное состояние (Active, Grace, y т. д.). |

**Selector de código:** segmento `{selector}` del formato IH58, comprimido (`sora`) o código hexadecimal para ADDR-5; Torii normalmente se encuentra en `NameSelectorV1`.**Modelь ошибок:** estos componentes combinan Norito JSON con `code`, `message`, `details`. Коды включают `sns_err_reserved`, `sns_err_payment_mismatch`, `sns_err_policy_violation`, `sns_err_governance_missing`.

### 3.1 CLI помощники (требование ручного регистратора N0)

Steward puede utilizar el registrador en la CLI mediante archivos JSON:

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

- `--owner` para completar la configuración de la cuenta CLI; Utilice `--controller` para agregar las cuentas del controlador correspondientes (por ejemplo `[owner]`).
- Placa de bandera en línea compatible con `PaymentProofV1`; Передайте `--payment-json PATH`, если уже есть структурированная квитанция. Los metadatos (`--metadata-json`) y los ganchos de gobernanza (`--governance-json`) se encuentran aquí.

помощники дополняют репетиции de sólo lectura:

```bash
iroha sns registration --selector makoto.sora
iroha sns policy --suffix-id 1
```

См. `crates/iroha_cli/src/commands/sns.rs` para la realización; Los comandos disponibles actualmente son Norito DTO y este documento, que se encuentra en la CLI compatible con Torii байт-в-байт.

Дополнительные помощники покрывают продления, передачи и действия guardian:

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
  --new-owner ih58... \
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

`--governance-json` должен содержать корректную запись `GovernanceHookV1` (id de propuesta, hashes de voto, подписи administrador/tutor). Каждая команда просто отражает соответствующий эндпоинт `/v1/sns/registrations/{selector}/...` чтобы операторы беты могли репетировать точные поверхности Torii, которые будут вызывать SDK.

## 4. Servicios gRPC```text
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

Formato de cable: хеш схемы Norito на этапе компиляции записан в
`fixtures/norito_rpc/schema_hashes.json` (строки `RegisterNameRequestV1`,
`RegisterNameResponseV1`, `NameRecordV1`, y т. д.).

## 5. Хуки управления и доказательства

Каждый изменяющий вызов должен приложить доказательства, пригодные для воспроизведения:

| Действие | Требуемые данные управления |
|----------|-----------------------|
| Registro/prescripción estándar | Доказательство платежа со ссылкой на инструкцию; голосование consejo не нужно, если tier не требует одобрения mayordomo. |
| Registro nivel premium / asignación reservada | `GovernanceHookV1` es una identificación de propuesta + reconocimiento del administrador. |
| Передача | Хеш голосования consejo + хеш сигнала DAO; autorización del tutor, когда передача инициирована разрешением спора. |
| Congelar/Descongelar | Подпись ticket de guardián más anulación del consejo (descongelación). |

Torii проверяет доказательства, проверяя:

1. ID de propuesta registrada en la actualización del libro mayor (`/v1/governance/proposals/{id}`) y estado `Approved`.
2. Хеши совпадают с записанными артефактами голосования.
3. El mayordomo/tutor puede consultar las claves públicas de `SuffixPolicyV1`.

Las pruebas necesarias son `sns_err_governance_missing`.

## 6. Примеры рабочих процессов

### 6.1 Registro estándar1. El cliente descarga `/v1/sns/policies/{suffix_id}` para obtener niveles de gracia y de descarga.
2. Estilo del cliente `RegisterNameRequestV1`:
   - `selector` está conectado a una etiqueta previamente comprimida (`sora`).
   - `term_years` en políticas anteriores.
   - `payment` ссылается на перевод splitter Treasury/steward.
3. Torii proporciona:
   - Нормализацию метки + reservados список.
   - Plazo/precio bruto vs `PriceTierV1`.
   - Сумма доказательства платежа >= рассчитанной цены + комиссии.
4. En la versión Torii:
   - Сохраняет `NameRecordV1`.
   - Mensaje `RegistryEventV1::NameRegistered`.
   - Mensaje `RevenueAccrualEventV1`.
   - Возвращает новую запись + события.

### 6.2 Продление в период gracia

Продления в Grace включают стандартный запрос плюс детект штрафа:

- Torii combina `now` con `grace_expires_at` y aplica un recargo a las tablas con `SuffixPolicyV1`.
- Доказательство платежа должно покрывать recargo. Ошибка => `sns_err_payment_mismatch`.
- `RegistryEventV1::NameRenewed` записывает новый `expires_at`.

### 6.3 Congelación del tutor y anulación del consejo

1. Guardian отправляет `FreezeNameRequestV1` с ticket, ссылающимся на id инцидента.
2. Torii escribe primero en `NameStatus::Frozen`, escribe `NameFrozen`.
3. Anulación del consejo de administración posterior; El operador controla BORRAR `/v1/sns/registrations/{selector}/freeze` con `GovernanceHookV1`.
4. Torii anula el controlador, envía `NameUnfrozen`.## 7. Validación y códigos de uso

| Código | Descripción | HTTP |
|-----|----------|------|
| `sns_err_reserved` | Метка зарезервирована или заблокирована. | 409 |
| `sns_err_policy_violation` | Срок, nivel или набор controladores нарушает политику. | 422 |
| `sns_err_payment_mismatch` | No se activará o no se activará en la placa base. | 402 |
| `sns_err_governance_missing` | Отсутствуют/некорректны требуемые артефакты управления. | 403 |
| `sns_err_state_conflict` | Операция недопустима в текущем состоянии жизненного цикла. | 409 |

Estos códigos están probados entre `X-Iroha-Error-Code` y la estructura Norito Convertidores JSON/NRPC.

## 8. Примечания по реализации

- Torii está pendiente de consultas en `NameRecordV1.auction` y se han eliminado algunos registros populares, en `PendingAuction`.
- Доказательства платежа переиспользуют Norito recibos del libro mayor; API auxiliar de servicios de tesorería (`/v1/finance/sns/payments`).
- SDK para abrir estos endpoints con varias fuentes de información, que pueden ser utilizadas por otros usuarios. ошибок (`ERR_SNS_RESERVED`, и т. д.).

## 9. Следующие шаги

- Inserte el interruptor Torii en un contrato de alquiler real después de conectar los auriculares SN-3.
- Descarga el SDK (Rust/JS/Swift), integrado en esta API.
- Расширить [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md) перекрестными ссылками на поля доказательств gancho de gobernanza.