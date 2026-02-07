---
lang: es
direction: ltr
source: docs/portal/docs/sns/registry-schema.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página está equipada con `docs/source/sns/registry_schema.md` y se utiliza el portal de copias canónicas. Исходный файл сохраняется для обновлений переводов.
:::

# Схема реестра Servicio de nombres de Sora (SN-2a)

**Estado:** Черновик 2026-03-24 -- отправлено на ревью программы SNS  
**Ссылка на roadmap:** SN-2a "Esquema de registro y diseño de almacenamiento"  
**Observar:** Estructuras canónicas actualizadas Norito, ciclo de sincronización y conexión con Sora Name Service (SNS) Realización de registros y registradores instalados en contratos, SDK y puertas de enlace.

Este documento contiene mensajes de texto para SN-2a, previamente:

1. Identificadores y правила хеширования (`SuffixId`, `NameHash`, selectores de derivación).
2. Norito estructuras/enumeraciones para archivos, aplicaciones políticas, niveles avanzados, archivos compartidos y registros de datos.
3. Configuración de diseño y parámetros precisos para la reproducción determinada.
4. Машину состояний, охватывающую регистрацию, продление, gracia/redención, congelación y lápida.
5. Канонические события, потребляемые DNS/gateway automatizado.

## 1. Identificadores y verificadores| Identificador | Descripción | Производная |
|------------|-------------|------------|
| `SuffixId` (`u16`) | Идентификатор рееестра для суффиксов верхнего уровня (`.sora`, `.nexus`, `.dao`). Согласован с каталогом суффиксов в [`sns_suffix_governance_charter.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns_suffix_governance_charter.md). | Назначается голосованием по gobernancia; хранится в `SuffixPolicyV1`. |
| `SuffixSelector` | Каноническая строковая форма суффикса (ASCII, minúsculas). | Ejemplo: `.sora` -> `sora`. |
| `NameSelectorV1` | El selector binario permite seleccionar la ley. | `struct NameSelectorV1 { version:u8 (=1); suffix_id:u16; label_len:u16; label_bytes:Vec<u8> }`. Лейбл в NFC + minúsculas по Norm v1. |
| `NameHash` (`[u8;32]`) | Основной ключ поиска, используемый контрактами, событиями и кешами. | `blake3(NameSelectorV1_bytes)`. |

Tres determinaciones:

- Лейблы нормализуются через Norm v1 (UTS-46 estricto, STD3 ASCII, NFC). Пользовательские строки ДОЛЖНЫ быть нормализованы перед хешированием.
- Зарезервированные лейблы (из `SuffixPolicyV1.reserved_labels`) никогда не входят в реестр; anulaciones de solo gobernanza выпускают события `ReservedNameAssigned`.

## 2. Estructura Norito

### 2.1 NombreRegistroV1| Polo | Consejo | Примечания |
|-------|------|-----------|
| `suffix_id` | `u16` | Ссылка на `SuffixPolicyV1`. |
| `selector` | `NameSelectorV1` | Seleccione un selector para auditoría/depuración. |
| `name_hash` | `[u8; 32]` | Ключ для карт/событий. |
| `normalized_label` | `AsciiString` | Читаемый лейбл (после Norma v1). |
| `display_label` | `AsciiString` | Carcasa de mayordomo; cosmética. |
| `owner` | `AccountId` | Управляет продлениями/трансферами. |
| `controllers` | `Vec<NameControllerV1>` | Ссылки на адреса аккаунтов, solucionadores y aplicaciones de metadatos. |
| `status` | `NameStatus` | Флаг жизненного цикла (см. Раздел 4). |
| `pricing_class` | `u8` | Индекс в ценовых niveles суффикса (estándar, premium, reservado). |
| `registered_at` | `Timestamp` | Время блока первичной активации. |
| `expires_at` | `Timestamp` | Конец оплаченного срока. |
| `grace_expires_at` | `Timestamp` | Конец gracia de renovación automática (predeterminado +30 días). |
| `redemption_expires_at` | `Timestamp` | Конец окна canje (predeterminado +60 дней). |
| `auction` | `Option<NameAuctionStateV1>` | Присутствует при reapertura holandesa o аукционах premium. |
| `last_tx_hash` | `Hash` | Детерминированный указатель на транзакцию версии. |
| `metadata` | `Metadata` | Произвольная registrador de metadatos (registros de texto, pruebas). |

Поддерживающие estructuras:

```text
Enum NameStatus {
    Available,          // derived, not stored on-ledger
    PendingAuction,
    Active,
    GracePeriod,
    Redemption,
    Frozen(NameFrozenStateV1),
    Tombstoned(NameTombstoneStateV1)
}

Struct NameFrozenStateV1 {
    reason: String,
    until_ms: u64,
}

Struct NameTombstoneStateV1 {
    reason: String,
}

Struct NameControllerV1 {
    controller_type: ControllerType,   // Account, ResolverTemplate, ExternalLink
    account_address: Option<AccountAddress>,   // Serialized as canonical `0x...` hex in JSON
    resolver_template_id: Option<String>,
    payload: Metadata,                 // Extra selector/value pairs for wallets/gateways
}

Struct TokenValue {
    asset_id: AsciiString,
    amount: u128,
}

Enum ControllerType {
    Account,
    Multisig,
    ResolverTemplate,
    ExternalLink
}

Struct NameAuctionStateV1 {
    kind: AuctionKind,             // Vickrey, DutchReopen
    opened_at_ms: u64,
    closes_at_ms: u64,
    floor_price: TokenValue,
    highest_commitment: Option<Hash>,  // reference to sealed bid
    settlement_tx: Option<Json>,
}

Enum AuctionKind {
    VickreyCommitReveal,
    DutchReopen
}
```

### 2.2 Política de sufijo V1| Polo | Consejo | Примечания |
|-------|------|-----------|
| `suffix_id` | `u16` | Первичный ключ; стабилен между версиями политики. |
| `suffix` | `AsciiString` | por ejemplo, `sora`. |
| `steward` | `AccountId` | Steward, определенный в carta de gobernanza. |
| `status` | `SuffixStatus` | `Active`, `Paused`, `Revoked`. |
| `payment_asset_id` | `AsciiString` | Актив liquidación по умолчанию (nombre `xor#sora`). |
| `pricing` | `Vec<PriceTierV1>` | Los cafés tienen niveles y niveles más bajos. |
| `min_term_years` | `u8` | Números mínimos de anulaciones de niveles. |
| `grace_period_days` | `u16` | Predeterminado 30. |
| `redemption_period_days` | `u16` | Predeterminado 60. |
| `max_term_years` | `u8` | Максимальная длительность предоплаты. |
| `referral_cap_bps` | `u16` | <=1000 (10%) por alquiler. |
| `reserved_labels` | `Vec<ReservedNameV1>` | Список от gobernancia с инструкциями назначения. |
| `fee_split` | `SuffixFeeSplitV1` | Доли tesorería / administrador / remisión (puntos básicos). |
| `fund_splitter_account` | `AccountId` | Аккаунт escrow + распределение средств. |
| `policy_version` | `u16` | Увеличивается при каждом изменении. |
| `metadata` | `Metadata` | Расширенные заметки (pacto de KPI, hashes docs по cumplimiento). |

```text
Struct PriceTierV1 {
    tier_id: u8,
    label_regex: String,       // RE2-syntax pattern describing eligible labels
    base_price: TokenValue,    // Price per one-year term before suffix coefficient
    auction_kind: AuctionKind, // Default auction when the tier triggers
    dutch_floor: Option<TokenValue>,
    min_duration_years: u8,
    max_duration_years: u8,
}

Struct ReservedNameV1 {
    normalized_label: AsciiString,
    assigned_to: Option<AccountId>,
    release_at_ms: Option<u64>,
    note: String,
}

Struct SuffixFeeSplitV1 {
    treasury_bps: u16,     // default 7000 (70%)
    steward_bps: u16,      // default 3000 (30%)
    referral_max_bps: u16, // optional referral carve-out (<= 1000)
    escrow_bps: u16,       // % routed to claw-back escrow
}
```

### 2.3 Записи доходов и liquidación| Estructura | Polonia | Назначение |
|--------|------|------------|
| `RevenueShareRecordV1` | `suffix_id`, `epoch_id`, `treasury_amount`, `steward_amount`, `referral_amount`, `escrow_amount`, `settled_at`, `tx_hash`. | Детерминированная запись распределенных выPLат по эпохам liquidación (неделя). |
| `RevenueAccrualEventV1` | `name_hash`, `suffix_id`, `event`, `gross_amount`, `net_amount`, `referral_account`. | Эмитируется при каждом платеже (registro, renovación, subasta). |

El polo `TokenValue` utiliza una codificación física canónica Norito con un código de valores de conexión `SuffixPolicyV1`.

### 2.4 События реестра

Канонические события дают replay log для DNS/gateway automatización y análisis.

```text
Struct RegistryEventV1 {
    name_hash: [u8; 32],
    suffix_id: u16,
    selector: NameSelectorV1,
    version: u64,               // increments per NameRecord update
    timestamp: Timestamp,
    tx_hash: Hash,
    actor: AccountId,
    event: RegistryEventKind,
}

Enum RegistryEventKind {
    NameRegistered { expires_at: Timestamp, pricing_class: u8 },
    NameRenewed { expires_at: Timestamp, term_years: u8 },
    NameTransferred { previous_owner: AccountId, new_owner: AccountId },
    NameControllersUpdated { controller_count: u16 },
    NameFrozen(NameFrozenStateV1),
    NameUnfrozen,
    NameTombstoned(NameTombstoneStateV1),
    AuctionOpened { kind: AuctionKind },
    AuctionSettled { winning_account: AccountId, clearing_price: TokenValue },
    RevenueSharePosted { epoch_id: u64, treasury_amount: TokenValue, steward_amount: TokenValue },
    SuffixPolicyUpdated { policy_version: u16 },
}
```

La mayoría de las veces se agregan registros reproducibles (por ejemplo, `RegistryEvents`) y se almacenan en fuentes de puerta de enlace, y las cachés de DNS no se pueden usar en пределах SLA.

## 3. Diseño хранения и индексы

| Ключ | Descripción |
|-----|----------|
| `Names::<name_hash>` | Основная карта `name_hash` -> `NameRecordV1`. |
| `NamesByOwner::<AccountId, suffix_id>` | Вторичный индекс для wallet UI (amigable con la paginación). |
| `NamesByLabel::<suffix_id, normalized_label>` | Обнаружение конфликтов, детерминированный поиск. |
| `SuffixPolicies::<suffix_id>` | Actual `SuffixPolicyV1`. |
| `RevenueShare::<suffix_id, epoch_id>` | Historia `RevenueShareRecordV1`. |
| `RegistryEvents::<u64>` | Registro de solo anexar с монотонной последовательностью. |Hay varias tuplas serializadas Norito para determinar la configuración de las configuraciones. Обновления индексов выполняются атомарно вместе с основным рекордом.

## 4. Машина состояний жизненного цикла| Состояние | Условия входа | Períodos dobles | Примечания |
|-------|----------------|---------------------|------------|
| Disponible | Производно, когда `NameRecord` отсутствует. | `PendingAuction` (premium), `Active` (registro estándar). | Поиск доступности читает только индексы. |
| Subasta Pendiente | Создается, когда `PriceTierV1.auction_kind` != ninguno. | `Active` (se liquida la subasta), `Tombstoned` (sin ofertas). | Los auriculares emiten `AuctionOpened` y `AuctionSettled`. |
| Activo | Registro o registro incorrecto. | `GracePeriod`, `Frozen`, `Tombstoned`. | `expires_at` двигает переход. |
| Período de Gracia | Automáticos según `now > expires_at`. | `Active` (renovación puntual), `Redemption`, `Tombstoned`. | Por defecto +30 días; резолвится, но помечено. |
| Redención | `now > grace_expires_at` y `< redemption_expires_at`. | `Active` (renovación tardía), `Tombstoned`. | Команды требуют штрафного платежа. |
| Congelado | Congelar la gobernanza y el guardián. | `Active` (posteriormente soluciones), `Tombstoned`. | No actualice ni modifique los controladores. |
| Lápida | Добровольная сдача, итог спора или истекшая redención. | `PendingAuction` (reapertura holandesa) или остается tombstoned. | Событие `NameTombstoned` должно включать причину. |Después de eliminar el software I18NI000000159X, estos cachés descendentes se instalan de forma predeterminada. Tombstoned имена, входящие в Dutch reabre las aplicaciones, прикрепляют payload `AuctionKind::DutchReopen`.

## 5. Puerta de enlace de sincronización y sincronización

Las puertas de enlace se pueden conectar a `RegistryEventV1` y sincronizar DNS/SoraFS, según:

1. Загрузка последнего `NameRecordV1`, на который указывает последовательность событий.
2. Plantillas de resolución de archivos (IH58 preдпочтительно + comprimido (`sora`) как второй выбор, registros de texto).
3. Pin обновленных данных зоны через SoraDNS flow из [`soradns_registry_rfc.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/soradns/soradns_registry_rfc.md).

Garantías de entrega:

- Каждая транзакция, влияющая на `NameRecordV1`, *должна* добавить ровно одно событие со строго возрастающей `version`.
- `RevenueSharePosted` события ссылаются на asentamientos из `RevenueShareRecordV1`.
- События congelar/descongelar/tombstone включают хеши артефактов в `metadata` para la reproducción de auditoría.

## 6. Cargas útiles primarias Norito

### 6.1 Primer registro de nombre

```text
NameRecordV1 {
    suffix_id: 0x0001,                       // .sora
    selector: NameSelectorV1 { version:1, suffix_id:1, label_len:5, label_bytes:"makoto" },
    name_hash: 0x5f57...9c2a,
    normalized_label: "makoto",
    display_label: "Makoto",
    owner: "ih58...",
    controllers: [
        NameControllerV1 {
            controller_type: Account,
            account_address: Some(AccountAddress("0x02000001...")),
            resolver_template_id: None,
            payload: {}
        }
    ],
    status: Active,
    pricing_class: 0,
    registered_at: 1_776_000_000,
    expires_at: 1_807_296_000,
    grace_expires_at: 1_809_888_000,
    redemption_expires_at: 1_815_072_000,
    auction: None,
    last_tx_hash: 0xa3d4...c001,
    metadata: { "resolver": "wallet.default", "notes": "SNS beta cohort" },
}
```

### 6.2 Primera política de sufijos

```text
SuffixPolicyV1 {
    suffix_id: 0x0001,
    suffix: "sora",
    steward: "ih58...",
    status: Active,
    payment_asset_id: "xor#sora",
    pricing: [
        PriceTierV1 { tier_id:0, label_regex:"^[a-z0-9]{3,}$", base_price:"120 XOR", auction_kind:VickreyCommitReveal, dutch_floor:None, min_duration_years:1, max_duration_years:5 },
        PriceTierV1 { tier_id:1, label_regex:"^[a-z]{1,2}$", base_price:"10_000 XOR", auction_kind:DutchReopen, dutch_floor:Some("1_000 XOR"), min_duration_years:1, max_duration_years:3 }
    ],
    min_term_years: 1,
    grace_period_days: 30,
    redemption_period_days: 60,
    max_term_years: 5,
    referral_cap_bps: 500,
    reserved_labels: [
        ReservedNameV1 { normalized_label:"treasury", assigned_to:Some("ih58..."), release_at:None, note:"Protocol reserved" }
    ],
    fee_split: SuffixFeeSplitV1 { treasury_bps:7000, steward_bps:3000, referral_max_bps:1000, escrow_bps:500 },
    fund_splitter_account: "ih58...",
    policy_version: 3,
    metadata: { "kpi_covenant":"bafybeigd..." },
}
```

## 7. Следующие шаги- **SN-2b (API de registro y enlaces de gobernanza):** elimina estas estructuras de Torii (Norito y enlaces JSON) y proporciona comprobaciones de admisión a artefactos de gobernanza.
- **SN-3 (motor de subasta y registro):** переиспользовать `NameAuctionStateV1` para la confirmación/revelación de contraseñas y la reapertura holandesa.
- **SN-5 (Pago y liquidación):** использовать `RevenueShareRecordV1` для финансовой сверки и автоматизации отчетов.

Las aplicaciones y programas de configuración se pueden configurar en la hoja de ruta SNS actualizada en `roadmap.md` y en `status.md`. слиянии.