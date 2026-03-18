---
lang: ru
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2077d985b10b26b29b821646b435cc8850cbc6c842d372de6c9c4523ee95a5b7
source_last_modified: "2026-03-12T11:24:34.970622+00:00"
translation_last_reviewed: 2026-03-12
translator: machine-google-reviewed
---

# Iroha Модель данных v2 и ISI — спецификация, производная от реализации

Эта спецификация является результатом обратного проектирования текущей реализации `iroha_data_model` и `iroha_core`, чтобы облегчить анализ конструкции. Пути в обратных кавычках указывают на авторитетный код.

## Область применения
- Определяет канонические объекты (домены, учетные записи, активы, NFT, роли, разрешения, одноранговые узлы, триггеры) и их идентификаторы.
- Описывает инструкции изменения состояния (ISI): типы, параметры, предварительные условия, переходы между состояниями, создаваемые события и состояния ошибок.
- Обобщает управление параметрами, транзакциями и сериализацией инструкций.

Детерминизм: вся семантика инструкций представляет собой чистые переходы между состояниями без аппаратно-зависимого поведения. Сериализация использует Norito; Байт-код виртуальной машины использует IVM и проверяется на стороне хоста перед выполнением в цепочке.

---

## Сущности и идентификаторы
Идентификаторы имеют стабильную строковую форму с двусторонним обменом `Display`/`FromStr`. Правила имен запрещают пробелы и зарезервированные символы `@ # $`.- `Name` — проверенный текстовый идентификатор. Правила: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Домен: `{ id, logo, metadata, owned_by }`. Строители: `NewDomain`. Код: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — канонические адреса создаются через `AccountAddress` (I105 / hex), а Torii нормализует входные данные через `AccountAddress::parse_encoded`. I105 — предпочтительный формат учетной записи; форма I105 предназначена для UX только для Sora. Знакомая строка `alias` (отклоненная устаревшая форма) сохраняется только как псевдоним маршрутизации. Аккаунт: `{ id, metadata }`. Код: `crates/iroha_data_model/src/account.rs`.- Политика допуска учетных записей — домены контролируют неявное создание учетных записей, сохраняя Norito-JSON `AccountAdmissionPolicy` под ключом метаданных `iroha:account_admission_policy`. Если ключ отсутствует, пользовательский параметр уровня цепочки `iroha:default_account_admission_policy` предоставляет значение по умолчанию; если он также отсутствует, жесткое значение по умолчанию — `ImplicitReceive` (первая версия). Теги политики `mode` (`ExplicitOnly` или `ImplicitReceive`), а также дополнительные ограничения на каждую транзакцию (по умолчанию `16`) и ограничения на создание каждого блока, необязательный `implicit_creation_fee` (сжигающий или поглощающий аккаунт), `min_initial_amounts` для каждого определения актива и необязательный `default_role_on_create` (предоставляется после `AccountCreated`, отклоняется с `DefaultRoleError`, если отсутствует). Genesis не может принять участие; отключенные/недействительные политики отклоняют инструкции в виде квитанций для неизвестных учетных записей с `InstructionExecutionError::AccountAdmission`. Неявные учетные записи отмечают метаданные `iroha:created_via="implicit"` до `AccountCreated`; роли по умолчанию выдают последующий `AccountRoleGranted`, а базовые правила владельца-исполнителя позволяют новой учетной записи расходовать свои собственные активы/NFT без дополнительных ролей. Код: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — канонический `aid:<32-lower-hex-no-dash>` (UUID-v4 байта). Определение: `{ id, name, description?, alias?, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Литералы `alias` должны быть `<name>#<domain>@<dataspace>` или `<name>#<dataspace>`, где `<name>` соответствует имени определения актива. Код: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: канонический литерал `norito:<hex>` (устаревшие текстовые формы не поддерживаются в первом выпуске).- `NftId` — `nft$domain`. НФТ: `{ id, content: Metadata, owned_by }`. Код: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Роль: `{ id, permissions: BTreeSet<Permission> }` со строителем `NewRole { inner: Role, grant_to }`. Код: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Код: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — идентификатор узла (открытый ключ) и адрес. Код: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Триггер: `{ id, action }`. Действие: `{ executable, repeats, authority, filter, metadata }`. Код: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` с проверенным вставкой/извлечением. Код: `crates/iroha_data_model/src/metadata.rs`.
- Шаблон подписки (уровень приложения): планы представляют собой записи `AssetDefinition` с метаданными `subscription_plan`; подписки — это записи `Nft` с метаданными `subscription`; выставление счетов выполняется с помощью триггеров времени, ссылающихся на NFT подписки. См. `docs/source/subscriptions_api.md` и `crates/iroha_data_model/src/subscription.rs`.
- **Криптографические примитивы** (функция `sm`):
  - `Sm2PublicKey` / `Sm2Signature` отражают каноническую точку SEC1 + кодировку `r∥s` фиксированной ширины для SM2. Конструкторы обеспечивают членство в кривой и различение семантики идентификаторов (`DEFAULT_DISTID`), в то время как проверка отклоняет некорректные скаляры или скаляры с высоким диапазоном. Код: `crates/iroha_crypto/src/sm.rs` и `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` предоставляет дайджест GM/T 0004 как сериализуемый Norito новый тип `[u8; 32]`, используемый везде, где хеши появляются в манифестах или телеметрии. Код: `crates/iroha_data_model/src/crypto/hash.rs`.- `Sm4Key` представляет 128-битные ключи SM4 и используется совместно системными вызовами хоста и устройствами модели данных. Код: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Эти типы располагаются рядом с существующими примитивами Ed25519/BLS/ML-DSA и доступны потребителям модели данных (Torii, SDK, инструменты Genesis) после включения функции `sm`.
- Хранилища отношений, производные от пространства данных (`space_directory_manifests`, `uaid_dataspaces`, `axt_policies`, `axt_replay_ledger`, реестр аварийного переопределения реле полосы) и разрешения целевого пространства данных (`CanPublishSpaceDirectoryManifest{dataspace: ...}` в хранилищах разрешений учетной записи/роли) удалены. `State::set_nexus(...)`, когда пространства данных исчезают из активного `dataspace_catalog`, предотвращая устаревшие ссылки на пространство данных после обновлений каталога среды выполнения. Кэши DA/реле с областью действия (`lane_relays`, `da_commitments`, `da_confidential_compute`, `da_pin_intents`) также удаляются, когда полоса выводится из эксплуатации или переназначается в другое пространство данных, поэтому локальное состояние полосы не может просачиваться при миграции пространства данных. ISI Space Directory (`PublishSpaceDirectoryManifest`, `RevokeSpaceDirectoryManifest`, `ExpireSpaceDirectoryManifest`) также проверяют `dataspace` на соответствие активному каталогу и отклоняют неизвестные идентификаторы с помощью `InvalidParameter`.

Важные черты: `Identifiable`, `Registered`/`Registrable` (шаблон строителя), `HasMetadata`, `IntoKeyValue`. Код: `crates/iroha_data_model/src/lib.rs`.

События: у каждой сущности есть события, генерируемые при мутациях (создание/удаление/изменение владельца/изменение метаданных и т. д.). Код: `crates/iroha_data_model/src/events/`.

---## Параметры (конфигурация цепочки)
- Семейства: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, плюс `custom: BTreeMap`.
— Отдельные перечисления для различий: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Агрегатор: `Parameters`. Код: `crates/iroha_data_model/src/parameter/system.rs`.

Настройка параметров (ISI): `SetParameter(Parameter)` обновляет соответствующее поле и выдает `ConfigurationEvent::Changed`. Код: `crates/iroha_data_model/src/isi/transparent.rs`, исполнитель в `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Сериализация и регистрация инструкций
- Основная черта: `Instruction: Send + Sync + 'static` с `dyn_encode()`, `as_any()`, стабильный `id()` (по умолчанию указано имя конкретного типа).
- `InstructionBox`: оболочка `Box<dyn Instruction>`. Clone/Eq/Ord работают с `(type_id, encoded_bytes)`, поэтому равенство осуществляется по значению.
- Norito serde для `InstructionBox` сериализуется как `(String wire_id, Vec<u8> payload)` (возврат к `type_name`, если нет идентификатора провода). Десериализация использует глобальные идентификаторы сопоставления `InstructionRegistry` с конструкторами. Реестр по умолчанию включает все встроенные ISI. Код: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: типы, семантика, ошибки
Выполнение реализовано через `Execute for <Instruction>` в `iroha_core::smartcontracts::isi`. Ниже перечислены общедоступные эффекты, предварительные условия, возникающие события и ошибки.

### Зарегистрироваться/Отменить регистрацию
Типы: `Register<T: Registered>` и `Unregister<T: Identifiable>`, с типами сумм `RegisterBox`/`UnregisterBox`, охватывающими конкретные цели.- Зарегистрировать пир: вставляется в набор мировых пиров.
  - Предварительные условия: не должно существовать.
  - События: `PeerEvent::Added`.
  - Ошибки: `Repetition(Register, PeerId)`, если дублируются; `FindError` при поиске. Код: `core/.../isi/world.rs`.

- Регистрация домена: создается на основе `NewDomain` с `owned_by = authority`. Запрещено: домен `genesis`.
  - Предварительные условия: отсутствие домена; не `genesis`.
  - События: `DomainEvent::Created`.
  - Ошибки: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Код: `core/.../isi/world.rs`.

- Регистрация учетной записи: сборка на основе `NewAccount`, запрещена в домене `genesis`; Аккаунт `genesis` не может быть зарегистрирован.
  - Предварительные условия: домен должен существовать; отсутствие аккаунта; не в домене генезиса.
  - События: `DomainEvent::Account(AccountEvent::Created)`.
  - Ошибки: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Код: `core/.../isi/domain.rs`.

- Регистрация AssetDefinition: строится из конструктора; устанавливает `owned_by = authority`.
  - Предпосылки: отсутствие определения; домен существует; `name` является обязательным, должен быть непустым после обрезки и не должен содержать `#`/`@`.
  - События: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Ошибки: `Repetition(Register, AssetDefinitionId)`. Код: `core/.../isi/domain.rs`.

- Регистрация NFT: сборки из конструктора; устанавливает `owned_by = authority`.
  - Предпосылки: отсутствие NFT; домен существует.
  - События: `DomainEvent::Nft(NftEvent::Created)`.
  - Ошибки: `Repetition(Register, NftId)`. Код: `core/.../isi/nft.rs`.- Регистрация роли: создается на основе `NewRole { inner, grant_to }` (первый владелец записан посредством сопоставления ролей учетной записи), сохраняется `inner: Role`.
  - Предпосылки: отсутствие роли.
  - События: `RoleEvent::Created`.
  - Ошибки: `Repetition(Register, RoleId)`. Код: `core/.../isi/world.rs`.

- Зарегистрировать триггер: сохраняет триггер в соответствующем наборе триггеров по типу фильтра.
  - Предварительные условия: если фильтр не подлежит установке, `action.repeats` должен быть `Exactly(1)` (в противном случае `MathError::Overflow`). Дублирование удостоверений личности запрещено.
  - События: `TriggerEvent::Created(TriggerId)`.
  - Ошибки: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` при ошибках преобразования/проверки. Код: `core/.../isi/triggers/mod.rs`.- Отменить регистрацию узла/домена/аккаунта/AssetDefinition/NFT/роли/триггера: удаляет цель; выдает события удаления. Дополнительные каскадные удаления:- Отменить регистрацию домена: удаляет объект домена, а также его состояние выбора/поддержки политики; удаляет определения активов в домене (и конфиденциальное побочное состояние `zk_assets`, связанное с этими определениями), активы этих определений (и метаданные для каждого актива), NFT в домене и проекции меток учетной записи/псевдонимов на уровне домена. Он также отключает оставшиеся учетные записи от удаленного домена и удаляет записи разрешений на уровне учетной записи/роли, которые ссылаются на удаленный домен или ресурсы, удаленные вместе с ним (разрешения домена, разрешения на определение актива/актива для удаленных определений и разрешения NFT для удаленных идентификаторов NFT). Удаление домена не удаляет глобальный `AccountId`, его состояние tx-sequence/UAID, владение иностранным активом или NFT, полномочия триггера или другие внешние ссылки аудита/конфигурации, которые указывают на сохранившуюся учетную запись. Ограждения: отклоняется, когда на какое-либо определение актива в домене все еще ссылаются соглашение репо, реестр расчетов, общедоступное вознаграждение/претензия, автономное разрешение/перевод, настройки расчетного репо по умолчанию (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), ссылки на определения активов, настроенные правительством, голосование/гражданство/парламентское право/заражение за вирусное вознаграждение, оракул-экономика настроенные ссылки на определение актива вознаграждения/косой черты/спорной облигации или ссылки на определение актива Nexus комиссии/стейкинга (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). События: `DomainEvent::Deleted`, плюс удаление каждого элемента.о событиях для удаленных ресурсов домена. Ошибки: `FindError::Domain`, если отсутствует; `InvariantViolation` о сохраненных конфликтах ссылок на определения активов. Код: `core/.../isi/world.rs`.- Отменить регистрацию учетной записи: удаляет разрешения, роли, счетчик tx-последовательности, сопоставление меток учетной записи и привязки UAID; удаляет активы, принадлежащие учетной записи (и метаданные каждого актива); удаляет NFT, принадлежащие учетной записи; удаляет триггеры, полномочиями которых является эта учетная запись; удаляет записи разрешений на уровне учетной записи/роли, которые ссылаются на удаленную учетную запись, разрешения NFT-цели на уровне учетной записи/роли для удаленных принадлежащих идентификаторов NFT и разрешения на целевые триггеры на уровне учетной записи/роли для удаленных триггеров. Ограждения: отклоняется, если учетная запись все еще владеет доменом, определением актива, привязкой поставщика SoraFS, активной записью гражданства, общедоступным состоянием ставки/вознаграждения (включая ключи запроса на вознаграждение, где учетная запись отображается как претендент или владелец актива вознаграждения), активное состояние оракула (включая записи поставщика истории каналов Oracle, записи поставщика привязки к Твиттеру или настроенные в Oracle-экономике ссылки на учетные записи с вознаграждением/косой чертой), активный Nexus ссылки на счета комиссий/стейкинга (`nexus.fees.fee_sink_account_id`, `nexus.staking.stake_escrow_account_id`, `nexus.staking.slash_sink_account_id`; анализируются как канонические идентификаторы учетных записей без домена и отклоняются при сбое закрытия при недопустимых литералах), активное состояние соглашения РЕПО, активное состояние расчетной книги, активное автономное разрешение/перевод или автономный режим состояние отзыва вердикта, активные ссылки на конфигурацию автономного условного депонирования для определений активных активов (`settlement.offline.escrow_accounts`), активное состояние управления (предложение/этап утверждениясписки als/locks/slashes/council/parlament, снимки парламента предложений, записи предлагающих обновление во время выполнения, ссылки на учетные записи условного депонирования/slash-receiver/viral-pool, настроенные для управления, ссылки на отправителя телеметрии управления SoraFS через `gov.sorafs_telemetry.submitters` / `gov.sorafs_telemetry.per_provider_submitters` или настроенные для управления SoraFS ссылки поставщика-владельца через `gov.sorafs_provider_owners`), настроенные ссылки на учетные записи разрешенного списка публикации контента (`content.publish_allow_accounts`), активное состояние отправителя социального депонирования, активное состояние создателя пакета контента, активное состояние владельца PIN-намерения DA, активное состояние переопределения экстренного валидатора реле полосы или активный реестр контактов SoraFS записи эмитента/связывающего устройства (манифесты контактов, псевдонимы манифестов, заказы репликации). События: `AccountEvent::Deleted` плюс `NftEvent::Deleted` для каждого удаленного NFT. Ошибки: `FindError::Account`, если отсутствует; `InvariantViolation` о собственности сироты. Код: `core/.../isi/domain.rs`.- Отменить регистрацию AssetDefinition: удаляет все активы этого определения и их метаданные для каждого актива, а также удаляет конфиденциальное побочное состояние `zk_assets`, связанное с этим определением; также удаляются соответствующие записи `settlement.offline.escrow_accounts` и записи разрешений на уровне учетной записи/роли, которые ссылаются на удаленное определение актива или его экземпляры актива. Ограждения: отклоняется, если на определение все еще ссылаются соглашение репо, реестр расчетов, публичное вознаграждение/претензия, автономное разрешение/состояние передачи, настройки расчетного репо по умолчанию (`settlement.repo.eligible_collateral`, `settlement.repo.collateral_substitution_matrix`), настроенное правительством голосование/гражданство/парламентское право на участие/вирусное вознаграждение, ссылки на определение активов, настроенная экономика оракула Ссылки на определение активов вознаграждения/косой черты/спорной облигации или ссылки на определение активов Nexus комиссии/стейкинга (`nexus.fees.fee_asset_id`, `nexus.staking.stake_asset_id`). События: `AssetDefinitionEvent::Deleted` и `AssetEvent::Deleted` для каждого актива. Ошибки: `FindError::AssetDefinition`, `InvariantViolation` при конфликтах ссылок. Код: `core/.../isi/domain.rs`.
  - Отменить регистрацию NFT: удаляет NFT и сокращает записи разрешений на уровне учетной записи/роли, которые ссылаются на удаленный NFT. События: `NftEvent::Deleted`. Ошибки: `FindError::Nft`. Код: `core/.../isi/nft.rs`.
  - Отменить регистрацию роли: сначала отменяет роль у всех учетных записей; затем удаляет роль. События: `RoleEvent::Deleted`. Ошибки: `FindError::Role`. Код: `core/.../isi/world.rs`.- Отменить регистрацию триггера: удаляет триггер, если он присутствует, и удаляет записи разрешений на уровне учетной записи/роли, которые ссылаются на удаленный триггер; дубликат отмены регистрации дает `Repetition(Unregister, TriggerId)`. События: `TriggerEvent::Deleted`. Код: `core/.../isi/triggers/mod.rs`.

### Мята / Ожог
Типы: `Mint<O, D: Identifiable>` и `Burn<O, D: Identifiable>`, в упаковке как `MintBox`/`BurnBox`.

- Актив (числовой) mint/burn: корректирует балансы и определения `total_quantity`.
  - Предварительные условия: значение `Numeric` должно соответствовать `AssetDefinition.spec()`; мята разрешена `mintable`:
    - `Infinitely`: разрешено всегда.
    - `Once`: разрешено ровно один раз; первый монетный двор меняет `mintable` на `Not` и выдает `AssetDefinitionEvent::MintabilityChanged`, а также подробный `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` для проверки.
    - `Limited(n)`: разрешает дополнительные операции монетного двора `n`. Каждый успешный монетный двор уменьшает счетчик; когда оно достигает нуля, определение меняется на `Not` и генерирует те же события `MintabilityChanged`, что и выше.
    - `Not`: ошибка `MintabilityError::MintUnmintable`.
  - Изменения состояния: создает актив, если он отсутствует на монетном дворе; удаляет запись актива, если баланс становится нулевым при сжигании.
  - События: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (когда `Once` или `Limited(n)` исчерпывает свой лимит).
  - Ошибки: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Код: `core/.../isi/asset.rs`.- Повторения триггера mint/burn: изменяет количество `action.repeats` для триггера.
  - Предварительные условия: на чеканке, фильтр должен быть чеканным; арифметика не должна переполняться/опускаться.
  - События: `TriggerEvent::Extended`/`TriggerEvent::Shortened`.
  - Ошибки: `MathError::Overflow` на недействительном монетном дворе; `FindError::Trigger`, если отсутствует. Код: `core/.../isi/triggers/mod.rs`.

### Передача
Типы: `Transfer<S: Identifiable, O, D: Identifiable>`, в упаковке как `TransferBox`.

- Актив (числовой): вычесть из источника `AssetId`, добавить к месту назначения `AssetId` (то же определение, другая учетная запись). Удалить обнуленный исходный актив.
  - Предварительные условия: существует исходный актив; значение удовлетворяет `spec`.
  - События: `AssetEvent::Removed` (источник), `AssetEvent::Added` (назначение).
  - Ошибки: `FindError::Asset`, `TypeError::AssetNumericSpec`, `MathError::NotEnoughQuantity/Overflow`. Код: `core/.../isi/asset.rs`.

- Владение доменом: меняет `Domain.owned_by` на целевую учетную запись.
  - Предварительные условия: обе учетные записи существуют; домен существует.
  - События: `DomainEvent::OwnerChanged`.
  - Ошибки: `FindError::Account/Domain`. Код: `core/.../isi/domain.rs`.

- Владение AssetDefinition: меняет `AssetDefinition.owned_by` на целевую учетную запись.
  - Предварительные условия: обе учетные записи существуют; определение существует; источник должен в настоящее время владеть им; уполномоченным лицом должен быть исходный аккаунт, владелец исходного домена или владелец домена определения актива.
  - События: `AssetDefinitionEvent::OwnerChanged`.
  - Ошибки: `FindError::Account/AssetDefinition`. Код: `core/.../isi/account.rs`.- Владение NFT: меняет `Nft.owned_by` на целевой аккаунт.
  - Предварительные условия: обе учетные записи существуют; NFT существует; источник должен в настоящее время владеть им; полномочия должны быть исходной учетной записью, владельцем исходного домена, владельцем NFT-домена или владельцем `CanTransferNft` для этого NFT.
  - События: `NftEvent::OwnerChanged`.
  - Ошибки: `FindError::Account/Nft`, `InvariantViolation`, если источник не владеет NFT. Код: `core/.../isi/nft.rs`.

### Метаданные: установка/удаление значения ключа
Типы: `SetKeyValue<T>` и `RemoveKeyValue<T>` с `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Предоставлены коробочные перечисления.

- Комплект: вставляет или заменяет `Metadata[key] = Json(value)`.
- Удалить: удаляет ключ; ошибка, если отсутствует.
- События: `<Target>Event::MetadataInserted`/`MetadataRemoved` со старыми/новыми значениями.
- Ошибки: `FindError::<Target>`, если цель не существует; `FindError::MetadataKey` об отсутствии ключа для удаления. Код: `crates/iroha_data_model/src/isi/transparent.rs` и исполнитель реализуется для каждой цели.

### Разрешения и роли: предоставление/отзыв
Типы: `Grant<O, D>` и `Revoke<O, D>`, с упакованными перечислениями для `Permission`/`Role` в/из `Account` и `Permission` в/из `Role`.- Предоставить разрешение учетной записи: добавляет `Permission`, если он еще не присущ. События: `AccountEvent::PermissionAdded`. Ошибки: `Repetition(Grant, Permission)`, если дублируются. Код: `core/.../isi/account.rs`.
- Отозвать разрешение от учетной записи: удаляется, если оно имеется. События: `AccountEvent::PermissionRemoved`. Ошибки: `FindError::Permission`, если отсутствуют. Код: `core/.../isi/account.rs`.
- Предоставить роль учетной записи: вставляет сопоставление `(account, role)`, если оно отсутствует. События: `AccountEvent::RoleGranted`. Ошибки: `Repetition(Grant, RoleId)`. Код: `core/.../isi/account.rs`.
- Отозвать роль из учетной записи: удаляет сопоставление, если оно имеется. События: `AccountEvent::RoleRevoked`. Ошибки: `FindError::Role`, если отсутствуют. Код: `core/.../isi/account.rs`.
- Предоставить разрешение роли: перестраивает роль с добавленным разрешением. События: `RoleEvent::PermissionAdded`. Ошибки: `Repetition(Grant, Permission)`. Код: `core/.../isi/world.rs`.
- Отозвать разрешение от роли: перестраивает роль без этого разрешения. События: `RoleEvent::PermissionRemoved`. Ошибки: `FindError::Permission`, если отсутствуют. Код: `core/.../isi/world.rs`.### Триггеры: выполнить
Тип: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Поведение: ставит в очередь `ExecuteTriggerEvent { trigger_id, authority, args }` для подсистемы триггера. Ручное выполнение разрешено только для триггеров по вызову (фильтр `ExecuteTrigger`); фильтр должен совпадать, а вызывающий абонент должен быть уполномоченным на триггерное действие или иметь `CanExecuteTrigger` для этого полномочия. Когда исполнитель, предоставленный пользователем, активен, выполнение триггера проверяется исполнителем во время выполнения и потребляет топливный бюджет исполнителя транзакции (базовый `executor.fuel` плюс дополнительные метаданные `additional_fuel`).
- Ошибки: `FindError::Trigger`, если не зарегистрирован; `InvariantViolation`, если вызван неавторизованный пользователь. Код: `core/.../isi/triggers/mod.rs` (и тесты в `core/.../smartcontracts/isi/mod.rs`).

### Обновление и журнал
- `Upgrade { executor }`: переносит исполнителя, используя предоставленный байт-код `Executor`, обновляет исполнителя и его модель данных, выдает `ExecutorEvent::Upgraded`. Ошибки: при неудачной миграции заворачиваются в `InvalidParameterError::SmartContract`. Код: `core/.../isi/world.rs`.
- `Log { level, msg }`: выдает журнал узла с заданным уровнем; никаких изменений состояния. Код: `core/.../isi/world.rs`.

### Модель ошибки
Общий конверт: `InstructionExecutionError` с вариантами ошибок оценки, сбоев запроса, преобразований, объекта не найден, повторения, возможности чеканки, математических вычислений, недопустимого параметра и нарушения инварианта. Перечисления и помощники находятся в `crates/iroha_data_model/src/isi/mod.rs` под `pub mod error`.

---## Транзакции и исполняемые файлы
- `Executable`: либо `Instructions(ConstVec<InstructionBox>)`, либо `Ivm(IvmBytecode)`; байт-код сериализуется как base64. Код: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: создает, подписывает и упаковывает исполняемый файл с метаданными, `chain_id`, `authority`, `creation_time_ms`, необязательно `ttl_ms` и `nonce`. Код: `crates/iroha_data_model/src/transaction/`.
- Во время выполнения `iroha_core` выполняет пакеты `InstructionBox` через `Execute for InstructionBox`, приводя к соответствующему `*Box` или конкретной инструкции. Код: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Бюджет проверки исполнителя во время выполнения (исполнитель, предоставленный пользователем): базовый `executor.fuel` из параметров плюс дополнительные метаданные транзакции `additional_fuel` (`u64`), общие для проверок инструкций/триггеров в транзакции.

---## Инварианты и примечания (из тестов и охранников)
- Защита Genesis: невозможно зарегистрировать домен `genesis` или учетные записи в домене `genesis`; Аккаунт `genesis` не может быть зарегистрирован. Код/тесты: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Числовые активы должны соответствовать `NumericSpec` при выпуске/передаче/сжигании; Несоответствие спецификаций дает `TypeError::AssetNumericSpec`.
- Возможность чеканки: `Once` позволяет выпускать одну монету, а затем переключается на `Not`; `Limited(n)` позволяет использовать ровно `n` перед переключением на `Not`. Попытки запретить чеканку на `Infinitely` вызывают `MintabilityError::ForbidMintOnMintable`, а настройка `Limited(0)` приводит к `MintabilityError::InvalidMintabilityTokens`.
- Операции с метаданными точны по ключу; удаление несуществующего ключа является ошибкой.
- Триггерные фильтры могут быть неминтируемыми; тогда `Register<Trigger>` разрешает только повторы `Exactly(1)`.
- Ключ метаданных триггера `__enabled` (bool) запускает выполнение вентилей; отсутствуют значения по умолчанию для включенных, а отключенные триггеры пропускаются по путям данных/времени/по вызову.
- Детерминизм: вся арифметика использует проверяемые операции; Under/overflow возвращает типизированные математические ошибки; нулевой баланс удаляет записи об активах (нет скрытого состояния).

---## Практические примеры
- Чеканка и передача:
  - `Mint::asset_numeric(10, asset_id)` → добавляет 10, если это разрешено спецификацией/готовностью; события: `AssetEvent::Added`.
  - `Transfer::asset_numeric(asset_id, 5, to_account)` → ход 5; события для удаления/добавления.
- Обновления метаданных:
  - `SetKeyValue::account(account_id, "avatar".parse()?, json)` → обновить; удаление через `RemoveKeyValue::account(...)`.
- Управление ролями/разрешениями:
  - `Grant::account_role(role_id, account)`, `Grant::role_permission(perm, role)` и их аналоги `Revoke`.
- Жизненный цикл триггера:
  - `Register::trigger(Trigger::new(id, Action::new(exec, repeats, authority, filter)))` с проверкой чеканки, подразумеваемой фильтром; `ExecuteTrigger::new(id).with_args(&args)` должен соответствовать настроенным полномочиям.
  - Триггеры можно отключить, установив для ключа метаданных `__enabled` значение `false` (отсутствующие значения по умолчанию включены); переключиться через системный вызов `SetKeyValue::trigger` или системный вызов IVM `set_trigger_enabled`.
  - Хранилище триггеров восстанавливается при загрузке: повторяющиеся идентификаторы, несовпадающие идентификаторы и триггеры, ссылающиеся на отсутствующий байт-код, удаляются; счетчики ссылок на байт-код пересчитываются.
  — Если байт-код IVM триггера отсутствует во время выполнения, триггер удаляется, и выполнение рассматривается как неактивное с результатом сбоя.
  - Исчерпанные триггеры удаляются сразу; если во время выполнения встречается исчерпанная запись, она удаляется и рассматривается как отсутствующая.
- Обновление параметров:
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` обновляет и выдает `ConfigurationEvent::Changed`.CLI/Torii `aid` + примеры псевдонимов:
- Зарегистрируйтесь с помощью канонической помощи + явное имя + длинный псевдоним:
  - `iroha ledger asset definition register --id aid:2f17c72466f84a4bb8a8e24884fdcd2f --name pkr --alias pkr#ubl@sbp`
- Зарегистрируйтесь с помощью канонической помощи + явное имя + короткий псевдоним:
  - `iroha ledger asset definition register --id aid:550e8400e29b41d4a7164466554400dd --name pkr --alias pkr#sbp`
- Mint по псевдониму + компоненты аккаунта:
  - `iroha ledger asset mint --definition-alias pkr#ubl@sbp --account <i105> --quantity 500`
- Разрешить псевдоним канонической помощи:
  - `POST /v1/assets/aliases/resolve` с JSON `{ "alias": "pkr#ubl@sbp" }`

Примечание по миграции:
— Текстовые идентификаторы определения активов `name#domain` намеренно не поддерживаются в первом выпуске.
— Идентификаторы активов на границах выпуска/сжигания/передачи остаются каноническими `norito:<hex>`; используйте `iroha tools encode asset-id` с `--definition aid:...` или `--alias ...` плюс `--account`.

---

## Прослеживаемость (отдельные источники)
 - Ядро модели данных: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - Определения и реестр ISI: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - Исполнение ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - События: `crates/iroha_data_model/src/events/**`.
 - Транзакции: `crates/iroha_data_model/src/transaction/**`.

Если вы хотите, чтобы эта спецификация была расширена до визуализированной таблицы API/поведения или привязана к каждому конкретному событию/ошибке, скажите слово, и я расширю ее.