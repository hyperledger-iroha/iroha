---
lang: ru
direction: ltr
source: docs/source/data_model_and_isi_spec.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 55ac770cf80229c23d6067ef1ab312422c76fb928a08e8cad8c040bdab396016
source_last_modified: "2026-01-28T18:33:51.650362+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha Модель данных v2 и ISI — спецификация, производная от реализации

Эта спецификация является результатом обратного проектирования текущей реализации `iroha_data_model` и `iroha_core`, чтобы облегчить анализ конструкции. Пути в обратных кавычках указывают на авторитетный код.

## Область применения
- Определяет канонические объекты (домены, учетные записи, активы, NFT, роли, разрешения, одноранговые узлы, триггеры) и их идентификаторы.
- Описывает инструкции изменения состояния (ISI): типы, параметры, предварительные условия, переходы между состояниями, создаваемые события и состояния ошибок.
- Обобщает управление параметрами, транзакциями и сериализацией инструкций.

Детерминизм: вся семантика инструкций представляет собой чистые переходы состояний без аппаратно-зависимого поведения. Сериализация использует Norito; Байт-код виртуальной машины использует IVM и проверяется на стороне хоста перед выполнением в цепочке.

---

## Сущности и идентификаторы
Идентификаторы имеют стабильную строковую форму с двусторонним обменом `Display`/`FromStr`. Правила имен запрещают пробелы и зарезервированные символы `@ # $`.- `Name` — проверенный текстовый идентификатор. Правила: `crates/iroha_data_model/src/name.rs`.
- `DomainId` — `name`. Домен: `{ id, logo, metadata, owned_by }`. Строители: `NewDomain`. Код: `crates/iroha_data_model/src/domain.rs`.
- `AccountId` — канонические адреса создаются через `AccountAddress` (IH58 / `sora…` сжатый/шестнадцатеричный), а Torii нормализует входные данные через `AccountAddress::parse_encoded`. IH58 — предпочтительный формат учетной записи; Форма `sora…` является второй лучшей для UX только для Sora. Знакомая строка `alias@domain` сохраняется только как псевдоним маршрутизации. Аккаунт: `{ id, metadata }`. Код: `crates/iroha_data_model/src/account.rs`.
- Политика допуска учетных записей — домены контролируют неявное создание учетных записей, сохраняя Norito-JSON `AccountAdmissionPolicy` под ключом метаданных `iroha:account_admission_policy`. Если ключ отсутствует, пользовательский параметр уровня цепочки `iroha:default_account_admission_policy` предоставляет значение по умолчанию; если он также отсутствует, жесткое значение по умолчанию — `ImplicitReceive` (первая версия). Теги политики `mode` (`ExplicitOnly` или `ImplicitReceive`), а также дополнительные ограничения на каждую транзакцию (по умолчанию `16`) и ограничения на создание каждого блока, необязательный `implicit_creation_fee` (сжигать или поглощать учетную запись), `min_initial_amounts` для каждого определения актива и необязательный `default_role_on_create` (предоставляется после `AccountCreated`, отклоняется с `DefaultRoleError`, если отсутствует). Genesis не может принять участие; отключенные/недействительные политики отклоняют инструкции в виде квитанций для неизвестных учетных записей с `InstructionExecutionError::AccountAdmission`. Неявные учетные записи отмечают метаданные `iroha:created_via="implicit"` до `AccountCreated`; роли по умолчанию выдают последующий `AccountRoleGranted`, а базовые правила владельца-исполнителя позволяют новой учетной записи расходовать свои собственные активы/NFT без дополнительных ролей. Код: `crates/iroha_data_model/src/account/admission.rs`, `crates/iroha_core/src/smartcontracts/isi/account_admission.rs`.
- `AssetDefinitionId` — `asset#domain`. Определение: `{ id, spec: NumericSpec, mintable: Mintable, logo, metadata, owned_by, total_quantity }`. Код: `crates/iroha_data_model/src/asset/definition.rs`.
- `AssetId`: canonical encoded literal `norito:<hex>` (legacy textual forms are not supported in first release).
- `NftId` — `nft$domain`. НФТ: `{ id, content: Metadata, owned_by }`. Код: `crates/iroha_data_model/src/nft.rs`.
- `RoleId` — `name`. Роль: `{ id, permissions: BTreeSet<Permission> }` со строителем `NewRole { inner: Role, grant_to }`. Код: `crates/iroha_data_model/src/role.rs`.
- `Permission` — `{ name: Ident, payload: Json }`. Код: `crates/iroha_data_model/src/permission.rs`.
- `PeerId`/`Peer` — идентификатор и адрес узла (открытый ключ). Код: `crates/iroha_data_model/src/peer.rs`.
- `TriggerId` — `name`. Триггер: `{ id, action }`. Действие: `{ executable, repeats, authority, filter, metadata }`. Код: `crates/iroha_data_model/src/trigger/`.
- `Metadata` — `BTreeMap<Name, Json>` с проверенной вставкой/извлечением. Код: `crates/iroha_data_model/src/metadata.rs`.
- Шаблон подписки (уровень приложения): планы представляют собой записи `AssetDefinition` с метаданными `subscription_plan`; подписки — это записи `Nft` с метаданными `subscription`; выставление счетов выполняется с помощью триггеров времени, ссылающихся на NFT подписки. См. `docs/source/subscriptions_api.md` и `crates/iroha_data_model/src/subscription.rs`.
- **Криптографические примитивы** (функция `sm`):- `Sm2PublicKey` / `Sm2Signature` отражают каноническую точку SEC1 + кодировку `r∥s` фиксированной ширины для SM2. Конструкторы обеспечивают членство в кривой и различение семантики идентификаторов (`DEFAULT_DISTID`), в то время как проверка отклоняет некорректные скаляры или скаляры с высоким диапазоном. Код: `crates/iroha_crypto/src/sm.rs` и `crates/iroha_data_model/src/crypto/mod.rs`.
  - `Sm3Hash` представляет дайджест GM/T 0004 как сериализуемый Norito новый тип `[u8; 32]`, используемый везде, где хеши появляются в манифестах или телеметрии. Код: `crates/iroha_data_model/src/crypto/hash.rs`.
  - `Sm4Key` представляет 128-битные ключи SM4 и используется совместно системными вызовами хоста и устройствами модели данных. Код: `crates/iroha_data_model/src/crypto/symmetric.rs`.
  Эти типы находятся рядом с существующими примитивами Ed25519/BLS/ML-DSA и доступны потребителям модели данных (Torii, SDK, инструменты Genesis) после включения функции `sm`.

Важные черты: `Identifiable`, `Registered`/`Registrable` (шаблон строителя), `HasMetadata`, `IntoKeyValue`. Код: `crates/iroha_data_model/src/lib.rs`.

События: у каждой сущности есть события, генерируемые при мутациях (создание/удаление/изменение владельца/изменение метаданных и т. д.). Код: `crates/iroha_data_model/src/events/`.

---

## Параметры (конфигурация цепочки)
- Семейства: `SumeragiParameters { block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, collectors_redundant_send_r }`, `BlockParameters { max_transactions }`, `TransactionParameters { max_signatures, max_instructions, ivm_bytecode_size, max_tx_bytes, max_decompressed_bytes }`, `SmartContractParameters { fuel, memory, execution_depth }`, плюс `custom: BTreeMap`.
— Отдельные перечисления для различий: `SumeragiParameter`, `BlockParameter`, `TransactionParameter`, `SmartContractParameter`. Агрегатор: `Parameters`. Код: `crates/iroha_data_model/src/parameter/system.rs`.

Настройка параметров (ISI): `SetParameter(Parameter)` обновляет соответствующее поле и выдает `ConfigurationEvent::Changed`. Код: `crates/iroha_data_model/src/isi/transparent.rs`, исполнитель в `crates/iroha_core/src/smartcontracts/isi/world.rs`.

---

## Сериализация и регистрация инструкций
- Основная черта: `Instruction: Send + Sync + 'static` с `dyn_encode()`, `as_any()`, стабильный `id()` (по умолчанию используется имя конкретного типа).
- `InstructionBox`: оболочка `Box<dyn Instruction>`. Clone/Eq/Ord работают с `(type_id, encoded_bytes)`, поэтому равенство осуществляется по значению.
- Серде Norito для `InstructionBox` сериализуется как `(String wire_id, Vec<u8> payload)` (возврат к `type_name`, если нет идентификатора провода). Десериализация использует глобальные идентификаторы сопоставления `InstructionRegistry` с конструкторами. Реестр по умолчанию включает все встроенные ISI. Код: `crates/iroha_data_model/src/isi/{mod.rs,registry.rs}`.

---

## ISI: типы, семантика, ошибки
Выполнение реализовано через `Execute for <Instruction>` в `iroha_core::smartcontracts::isi`. Ниже перечислены общедоступные эффекты, предварительные условия, возникающие события и ошибки.

### Зарегистрироваться/Отменить регистрацию
Типы: `Register<T: Registered>` и `Unregister<T: Identifiable>`, с типами сумм `RegisterBox`/`UnregisterBox`, охватывающими конкретные цели.

- Зарегистрировать пир: вставляется в набор мировых пиров.
  - Предварительные условия: не должно существовать.
  - События: `PeerEvent::Added`.
  - Ошибки: `Repetition(Register, PeerId)`, если дублируются; `FindError` при поиске. Код: `core/.../isi/world.rs`.

- Регистрация домена: создается на основе `NewDomain` с `owned_by = authority`. Запрещено: домен `genesis`.
  - Предварительные условия: отсутствие домена; не `genesis`.
  - События: `DomainEvent::Created`.
  - Ошибки: `Repetition(Register, DomainId)`, `InvariantViolation("Not allowed to register genesis domain")`. Код: `core/.../isi/world.rs`.- Регистрация учетной записи: сборка на основе `NewAccount`, запрещена в домене `genesis`; Аккаунт `genesis` не может быть зарегистрирован.
  - Предварительные условия: домен должен существовать; отсутствие аккаунта; не в домене генезиса.
  - События: `DomainEvent::Account(AccountEvent::Created)`.
  - Ошибки: `Repetition(Register, AccountId)`, `InvariantViolation("Not allowed to register account in genesis domain")`. Код: `core/.../isi/domain.rs`.

- Регистрация AssetDefinition: строится из конструктора; устанавливает `owned_by = authority`.
  - Предпосылки: отсутствие определения; домен существует.
  - События: `DomainEvent::AssetDefinition(AssetDefinitionEvent::Created)`.
  - Ошибки: `Repetition(Register, AssetDefinitionId)`. Код: `core/.../isi/domain.rs`.

- Регистрация NFT: сборки из конструктора; устанавливает `owned_by = authority`.
  - Предпосылки: отсутствие NFT; домен существует.
  - События: `DomainEvent::Nft(NftEvent::Created)`.
  - Ошибки: `Repetition(Register, NftId)`. Код: `core/.../isi/nft.rs`.

- Регистрация роли: создается на основе `NewRole { inner, grant_to }` (первый владелец записан посредством сопоставления ролей учетной записи), сохраняется `inner: Role`.
  - Предпосылки: отсутствие роли.
  - События: `RoleEvent::Created`.
  - Ошибки: `Repetition(Register, RoleId)`. Код: `core/.../isi/world.rs`.

- Зарегистрировать триггер: сохраняет триггер в соответствующем наборе триггеров по типу фильтра.
  - Предварительные условия: если фильтр не подлежит сборке, `action.repeats` должен быть `Exactly(1)` (в противном случае `MathError::Overflow`). Дублирование удостоверений личности запрещено.
  - События: `TriggerEvent::Created(TriggerId)`.
  - Ошибки: `Repetition(Register, TriggerId)`, `InvalidParameterError::SmartContract(..)` при ошибках преобразования/проверки. Код: `core/.../isi/triggers/mod.rs`.

- Отменить регистрацию узла/домена/аккаунта/AssetDefinition/NFT/роли/триггера: удаляет цель; выдает события удаления. Дополнительные каскадные удаления:
  - Отменить регистрацию домена: удаляет все учетные записи в домене, их роли, разрешения, счетчики tx-последовательностей, метки учетных записей и привязки UAID; удаляет свои активы (и метаданные каждого актива); удаляет все определения активов в домене; удаляет NFT в домене и любые NFT, принадлежащие удаленным учетным записям; удаляет триггеры, чей домен полномочий соответствует. События: `DomainEvent::Deleted`, а также события удаления каждого элемента. Ошибки: `FindError::Domain`, если отсутствует. Код: `core/.../isi/world.rs`.
  - Отменить регистрацию учетной записи: удаляет разрешения, роли, счетчик tx-последовательности, сопоставление меток учетной записи и привязки UAID; удаляет активы, принадлежащие учетной записи (и метаданные каждого актива); удаляет NFT, принадлежащие учетной записи; удаляет триггеры, полномочиями которых является эта учетная запись. События: `AccountEvent::Deleted` плюс `NftEvent::Deleted` за каждый удаленный NFT. Ошибки: `FindError::Account`, если отсутствует. Код: `core/.../isi/domain.rs`.
  - Отменить регистрацию AssetDefinition: удаляет все активы этого определения и их метаданные для каждого актива. События: `AssetDefinitionEvent::Deleted` и `AssetEvent::Deleted` для каждого актива. Ошибки: `FindError::AssetDefinition`. Код: `core/.../isi/domain.rs`.
  - Отменить регистрацию NFT: удаляет NFT. События: `NftEvent::Deleted`. Ошибки: `FindError::Nft`. Код: `core/.../isi/nft.rs`.
  - Отменить регистрацию роли: сначала отменяет роль у всех учетных записей; затем удаляет роль. События: `RoleEvent::Deleted`. Ошибки: `FindError::Role`. Код: `core/.../isi/world.rs`.
  - Отменить регистрацию триггера: удаляет триггер, если он присутствует; дубликат отмены регистрации дает `Repetition(Unregister, TriggerId)`. События: `TriggerEvent::Deleted`. Код: `core/.../isi/triggers/mod.rs`.

### Мята / Ожог
Типы: `Mint<O, D: Identifiable>` и `Burn<O, D: Identifiable>`, в упаковке как `MintBox`/`BurnBox`.- Актив (числовой) mint/burn: корректирует балансы и определения `total_quantity`.
  - Предварительные условия: значение `Numeric` должно соответствовать `AssetDefinition.spec()`; мята разрешена `mintable`:
    - `Infinitely`: разрешено всегда.
    - `Once`: разрешено ровно один раз; первый монетный двор меняет `mintable` на `Not` и выдает `AssetDefinitionEvent::MintabilityChanged`, а также подробный `AssetDefinitionEvent::MintabilityChangedDetailed { asset_definition, minted_amount, authority }` для проверки.
    - `Limited(n)`: разрешает дополнительные операции монетного двора `n`. Каждый успешный монетный двор уменьшает счетчик; когда оно достигает нуля, определение меняется на `Not` и генерирует те же события `MintabilityChanged`, что и выше.
    - `Not`: ошибка `MintabilityError::MintUnmintable`.
  - Изменения состояния: создает актив, если он отсутствует на монетном дворе; удаляет запись актива, если баланс становится нулевым при сжигании.
  - События: `AssetEvent::Added`/`AssetEvent::Removed`, `AssetDefinitionEvent::MintabilityChanged` (когда `Once` или `Limited(n)` исчерпывает свой лимит).
  - Ошибки: `TypeError::AssetNumericSpec(Mismatch)`, `MathError::Overflow`/`NotEnoughQuantity`. Код: `core/.../isi/asset.rs`.

- Повторения триггера mint/burn: изменяет количество `action.repeats` для триггера.
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
  - Предварительные условия: обе учетные записи существуют; определение существует; источник должен в настоящее время владеть им.
  - События: `AssetDefinitionEvent::OwnerChanged`.
  - Ошибки: `FindError::Account/AssetDefinition`. Код: `core/.../isi/account.rs`.

- Владение NFT: меняет `Nft.owned_by` на целевой аккаунт.
  - Предварительные условия: обе учетные записи существуют; NFT существует; источник должен в настоящее время владеть им.
  - События: `NftEvent::OwnerChanged`.
  - Ошибки: `FindError::Account/Nft`, `InvariantViolation`, если источник не владеет NFT. Код: `core/.../isi/nft.rs`.

### Метаданные: установка/удаление значения ключа
Типы: `SetKeyValue<T>` и `RemoveKeyValue<T>` с `T ∈ { Domain, Account, AssetDefinition, Nft, Trigger }`. Предоставлены коробочные перечисления.

- Комплект: вставляет или заменяет `Metadata[key] = Json(value)`.
- Удалить: удаляет ключ; ошибка, если отсутствует.
- События: `<Target>Event::MetadataInserted`/`MetadataRemoved` со старыми/новыми значениями.
- Ошибки: `FindError::<Target>`, если цель не существует; `FindError::MetadataKey` об отсутствии ключа для удаления. Код: `crates/iroha_data_model/src/isi/transparent.rs` и исполнитель реализуется для каждой цели.### Разрешения и роли: предоставление/отзыв
Типы: `Grant<O, D>` и `Revoke<O, D>`, с коробочными перечислениями для `Permission`/`Role` в/из `Account` и `Permission` в/из `Role`.

- Предоставить разрешение учетной записи: добавляет `Permission`, если он еще не присущ. События: `AccountEvent::PermissionAdded`. Ошибки: `Repetition(Grant, Permission)`, если дублируются. Код: `core/.../isi/account.rs`.
- Отозвать разрешение от учетной записи: удаляется, если оно имеется. События: `AccountEvent::PermissionRemoved`. Ошибки: `FindError::Permission`, если отсутствуют. Код: `core/.../isi/account.rs`.
- Предоставить роль учетной записи: вставляет сопоставление `(account, role)`, если оно отсутствует. События: `AccountEvent::RoleGranted`. Ошибки: `Repetition(Grant, RoleId)`. Код: `core/.../isi/account.rs`.
- Отозвать роль из учетной записи: удаляет сопоставление, если оно имеется. События: `AccountEvent::RoleRevoked`. Ошибки: `FindError::Role`, если отсутствуют. Код: `core/.../isi/account.rs`.
- Предоставить разрешение роли: перестраивает роль с добавленным разрешением. События: `RoleEvent::PermissionAdded`. Ошибки: `Repetition(Grant, Permission)`. Код: `core/.../isi/world.rs`.
- Отозвать разрешение от роли: перестраивает роль без этого разрешения. События: `RoleEvent::PermissionRemoved`. Ошибки: `FindError::Permission`, если отсутствуют. Код: `core/.../isi/world.rs`.

### Триггеры: выполнить
Тип: `ExecuteTrigger { trigger: TriggerId, args: Json }`.
- Поведение: ставит в очередь `ExecuteTriggerEvent { trigger_id, authority, args }` для подсистемы триггера. Ручное выполнение разрешено только для триггеров по вызову (фильтр `ExecuteTrigger`); фильтр должен совпадать, а вызывающий абонент должен быть уполномоченным на триггерное действие или иметь `CanExecuteTrigger` для этого полномочия. Когда пользовательский исполнитель активен, выполнение триггера проверяется исполнителем во время выполнения и расходует топливный бюджет исполнителя транзакции (базовый `executor.fuel` плюс дополнительные метаданные `additional_fuel`).
- Ошибки: `FindError::Trigger`, если не зарегистрирован; `InvariantViolation`, если вызван неавторизованный пользователь. Код: `core/.../isi/triggers/mod.rs` (и тесты в `core/.../smartcontracts/isi/mod.rs`).

### Обновление и журнал
- `Upgrade { executor }`: переносит исполнителя, используя предоставленный байт-код `Executor`, обновляет исполнителя и его модель данных, выдает `ExecutorEvent::Upgraded`. Ошибки: заворачиваются в `InvalidParameterError::SmartContract` при сбое миграции. Код: `core/.../isi/world.rs`.
- `Log { level, msg }`: выдает журнал узла с заданным уровнем; никаких изменений состояния. Код: `core/.../isi/world.rs`.

### Модель ошибки
Общий конверт: `InstructionExecutionError` с вариантами для ошибок оценки, сбоев запроса, преобразований, объекта не найден, повторения, возможности чеканки, математики, недопустимого параметра и нарушения инварианта. Перечисления и помощники находятся в `crates/iroha_data_model/src/isi/mod.rs` под `pub mod error`.

---## Транзакции и исполняемые файлы
- `Executable`: либо `Instructions(ConstVec<InstructionBox>)`, либо `Ivm(IvmBytecode)`; байт-код сериализуется как base64. Код: `crates/iroha_data_model/src/transaction/executable.rs`.
- `TransactionBuilder`/`SignedTransaction`: создает, подписывает и упаковывает исполняемый файл с метаданными, `chain_id`, `authority`, `creation_time_ms`, необязательно `ttl_ms` и `nonce`. Код: `crates/iroha_data_model/src/transaction/`.
- Во время выполнения `iroha_core` выполняет пакеты `InstructionBox` через `Execute for InstructionBox`, приводя к соответствующему `*Box` или конкретной инструкции. Код: `crates/iroha_core/src/smartcontracts/isi/mod.rs`.
- Бюджет проверки исполнителя во время выполнения (исполнитель, предоставленный пользователем): базовый `executor.fuel` из параметров плюс дополнительные метаданные транзакции `additional_fuel` (`u64`), общие для всех проверок инструкций/триггеров в транзакции.

---

## Инварианты и примечания (из тестов и охранников)
- Защита Genesis: невозможно зарегистрировать домен `genesis` или учетные записи в домене `genesis`; Аккаунт `genesis` не может быть зарегистрирован. Код/тесты: `core/.../isi/world.rs`, `core/.../smartcontracts/isi/mod.rs`.
- Числовые активы должны соответствовать `NumericSpec` при выпуске/передаче/сжигании; Несоответствие спецификаций дает `TypeError::AssetNumericSpec`.
- Возможность чеканки: `Once` позволяет выпускать одну монету, а затем переключается на `Not`; `Limited(n)` позволяет использовать ровно `n` перед переключением на `Not`. Попытки запретить чеканку на `Infinitely` вызывают `MintabilityError::ForbidMintOnMintable`, а настройка `Limited(0)` приводит к `MintabilityError::InvalidMintabilityTokens`.
- Операции с метаданными точны по ключу; удаление несуществующего ключа является ошибкой.
- Триггерные фильтры могут быть неминтируемыми; тогда `Register<Trigger>` разрешает только повторения `Exactly(1)`.
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
  - `SetParameter(SumeragiParameter::BlockTimeMs(2500).into())` обновляет и выдает `ConfigurationEvent::Changed`.

---

## Прослеживаемость (отдельные источники)
 - Ядро модели данных: `crates/iroha_data_model/src/{account.rs,domain.rs,asset/**,nft.rs,role.rs,permission.rs,metadata.rs,trigger/**,parameter/**}`.
 - Определения и реестр ISI: `crates/iroha_data_model/src/isi/{mod.rs,register.rs,transfer.rs,mint_burn.rs,transparent.rs,registry.rs}`.
 - Исполнение ISI: `crates/iroha_core/src/smartcontracts/isi/{mod.rs,world.rs,domain.rs,account.rs,asset.rs,nft.rs,triggers/**}`.
 - События: `crates/iroha_data_model/src/events/**`.
 - Транзакции: `crates/iroha_data_model/src/transaction/**`.

Если вы хотите, чтобы эта спецификация была расширена до визуализированной таблицы API/поведения или привязана к каждому конкретному событию/ошибке, скажите слово, и я расширю ее.