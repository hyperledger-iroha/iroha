---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Конфиденциальные активы и ZK-переводы
description: Блюпринт Phase C для shielded циркуляции, реестров и операторских контролей.
slug: /nexus/confidential-assets
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Дизайн конфиденциальных активов и ZK-переводов

## Мотивация
- Дать opt-in shielded потоки активов, чтобы домены могли сохранять транзакционную приватность, не изменяя прозрачную циркуляцию.
- Сохранять детерминированное исполнение на разнородном железе валидаторов и совместимость с Norito/Kotodama ABI v1.
- Предоставить аудиторам и операторам контроль жизненного цикла (активация, ротация, отзыв) для circuits и криптографических параметров.

## Модель угроз
- Валидаторы честные-но-любопытные (honest-but-curious): исполняют консенсус корректно, но пытаются изучать ledger/state.
- Наблюдатели сети видят данные блоков и gossiped транзакции; приватные gossip-каналы не предполагаются.
- Вне области: анализ off-ledger трафика, квантовые противники (отдельно в PQ roadmap), атаки доступности ledger.

## Обзор дизайна
- Активы могут объявлять *shielded pool* помимо существующих прозрачных балансов; shielded циркуляция представляется криптографическими commitments.
- Notes инкапсулируют `(asset_id, amount, recipient_view_key, blinding, rho)` с:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, независим от порядка notes.
  - Encrypted payload: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Транзакции несут Norito-кодированные payloads `ConfidentialTransfer`, содержащие:
  - Public inputs: Merkle anchor, nullifiers, новые commitments, asset id, версия circuit.
  - Зашифрованные payloads для получателей и опциональных аудиторов.
  - Zero-knowledge proof, подтверждающую сохранение стоимости, ownership и авторизацию.
- Verifying keys и наборы параметров контролируются on-ledger registry с окнами активации; узлы отказываются валидировать proofs, которые ссылаются на неизвестные или отозванные записи.
- Заголовки консенсуса коммитятся к активному digest конфиденциальных возможностей, поэтому блоки принимаются только при совпадении состояния registry и параметров.
- Построение proofs использует стек Halo2 (Plonkish) без trusted setup; Groth16 и другие варианты SNARK намеренно не поддерживаются в v1.

### Детерминированные фикстуры

Конфиденциальные memo-конверты теперь поставляются с каноническим фикстуром в `fixtures/confidential/encrypted_payload_v1.json`. Набор данных включает положительный v1-конверт и негативные поврежденные образцы, чтобы SDK могли проверять паритет парсинга. Rust data-model тесты (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) и Swift suite (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) загружают фикстуру напрямую, гарантируя, что Norito-кодирование, поверхности ошибок и регрессионное покрытие остаются согласованными по мере эволюции кодека.

Swift SDKs теперь могут выдавать shield-инструкции без bespoke JSON glue: соберите `ShieldRequest` с 32-байтным commitment ноты, зашифрованным payload и debit metadata, затем вызовите `IrohaSDK.submit(shield:keypair:)` (или `submitAndWait`), чтобы подписать и отправить транзакцию через `/v1/pipeline/transactions`. Хелпер валидирует длины commitments, пропускает `ConfidentialEncryptedPayload` через Norito encoder и зеркалит layout `zk::Shield`, описанный ниже, чтобы кошельки оставались синхронизированы с Rust.

## Коммитменты консенсуса и gating возможностей
- Заголовки блоков раскрывают `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; digest участвует в хэше консенсуса и должен совпадать с локальным представлением registry для принятия блока.
- Governance может подготавливать апгрейды, программируя `next_conf_features` с будущим `activation_height`; до этой высоты продюсеры блоков обязаны продолжать публиковать прежний digest.
- Валидаторские узлы ДОЛЖНЫ работать с `confidential.enabled = true` и `assume_valid = false`. Стартовые проверки не допускают узел в набор валидаторов, если любое условие нарушено или локальные `conf_features` расходятся.
- Метаданные P2P handshake теперь включают `{ enabled, assume_valid, conf_features }`. Peers, рекламирующие несовместимые возможности, отклоняются с `HandshakeConfidentialMismatch` и никогда не входят в ротацию консенсуса.
- Результаты совместимости между валидаторами, observers и устаревшими peers фиксируются в матрице handshake в разделе [Node Capability Negotiation](#node-capability-negotiation). Ошибки handshake проявляются как `HandshakeConfidentialMismatch` и держат peer вне ротации консенсуса, пока digest не совпадет.
- Observers, не являющиеся валидаторами, могут выставить `assume_valid = true`; они слепо применяют конфиденциальные дельты, но не влияют на безопасность консенсуса.

## Политики активов
- Каждое определение актива несет `AssetConfidentialPolicy`, установленную создателем или через governance:
  - `TransparentOnly`: режим по умолчанию; разрешены только прозрачные инструкции (`MintAsset`, `TransferAsset` и т.д.), а shielded операции отклоняются.
  - `ShieldedOnly`: вся эмиссия и переводы должны использовать конфиденциальные инструкции; `RevealConfidential` запрещен, поэтому балансы никогда не становятся публичными.
  - `Convertible`: держатели могут переносить стоимость между прозрачными и shielded представлениями, используя инструкции on/off-ramp ниже.
- Политики следуют ограниченному FSM, чтобы не блокировать средства:
  - `TransparentOnly → Convertible` (немедленное включение shielded pool).
  - `TransparentOnly → ShieldedOnly` (требует запланированного перехода и окна конверсии).
  - `Convertible → ShieldedOnly` (обязательная минимальная задержка).
  - `ShieldedOnly → Convertible` (нужен план миграции, чтобы shielded notes оставались тратимыми).
  - `ShieldedOnly → TransparentOnly` запрещен, если shielded pool не пуст или governance не кодирует миграцию, которая раскрывает оставшиеся notes.
- Инструкции governance задают `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` через ISI `ScheduleConfidentialPolicyTransition` и могут отменять запланированные изменения через `CancelConfidentialPolicyTransition`. Валидация mempool гарантирует, что ни одна транзакция не пересекает высоту перехода, а включение падает детерминированно, если проверка политики изменилась бы посреди блока.
- Запланированные переходы применяются автоматически при открытии нового блока: когда высота входит в окно конверсии (для апгрейдов `ShieldedOnly`) или достигает `effective_height`, runtime обновляет `AssetConfidentialPolicy`, обновляет metadata `zk.policy` и очищает pending entry. Если при созревании перехода `ShieldedOnly` остается прозрачное предложение, runtime отменяет изменение и логирует предупреждение, оставляя прежний режим.
- Конфигурационные knobs `policy_transition_delay_blocks` и `policy_transition_window_blocks` задают минимальное уведомление и grace periods, чтобы позволить кошелькам конвертировать notes вокруг переключения.
- `pending_transition.transition_id` также служит audit handle; governance обязана цитировать его при финализации или отмене переходов, чтобы операторы могли коррелировать отчеты on/off-ramp.
- `policy_transition_window_blocks` по умолчанию 720 (около 12 часов при времени блока 60 с). Узлы ограничивают запросы governance, пытающиеся задать более короткое уведомление.
- Genesis manifests и CLI потоки показывают текущие и ожидающие политики. Логика admission читает политику во время исполнения, подтверждая, что каждая конфиденциальная инструкция разрешена.
- Checklist миграции — см. “Migration sequencing” ниже для пошагового плана апгрейда, который отслеживает Milestone M0.

#### Мониторинг переходов через Torii

Кошельки и аудиторы опрашивают `GET /v1/confidential/assets/{definition_id}/transitions`, чтобы проверить активную `AssetConfidentialPolicy`. JSON payload всегда включает канонический asset id, последнюю наблюдаемую высоту блока, `current_mode` политики, режим, эффективный на этой высоте (окна конверсии временно отдают `Convertible`), и ожидаемые идентификаторы параметров `vk_set_hash`/Poseidon/Pedersen. Когда переход governance ожидается, ответ также включает:

- `transition_id` — audit handle, возвращенный `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` и вычисленный `window_open_height` (блок, где кошельки должны начать конверсию для cut-over ShieldedOnly).

Пример ответа:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Ответ `404` означает, что соответствующее определение актива не найдено. Когда переход не запланирован, поле `pending_transition` равно `null`.

### Машина состояний политики

| Текущий режим       | Следующий режим    | Предпосылки                                                                 | Обработка effective_height                                                                                         | Примечания                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance активировал записи registry verifier/parameter. Отправить `ScheduleConfidentialPolicyTransition` с `effective_height ≥ current_height + policy_transition_delay_blocks`. | Переход выполняется ровно на `effective_height`; shielded pool становится доступен сразу.                   | Путь по умолчанию для включения конфиденциальности при сохранении прозрачных потоков.               |
| TransparentOnly    | ShieldedOnly     | То же, что выше, плюс `policy_transition_window_blocks ≥ 1`.                                                         | Runtime автоматически входит в `Convertible` на `effective_height - policy_transition_window_blocks`; переключается на `ShieldedOnly` на `effective_height`. | Дает детерминированное окно конверсии до отключения прозрачных инструкций.   |
| Convertible        | ShieldedOnly     | Запланированный переход с `effective_height ≥ current_height + policy_transition_delay_blocks`. Governance SHOULD подтвердить (`transparent_supply == 0`) через audit metadata; runtime проверяет это на cut-over. | Та же семантика окна, что выше. Если прозрачное предложение не нулевое на `effective_height`, переход прерывается с `PolicyTransitionPrerequisiteFailed`. | Закрепляет актив в полностью конфиденциальной циркуляции.                                     |
| ShieldedOnly       | Convertible      | Запланированный переход; нет активного emergency withdrawal (`withdraw_height` не задан).                                    | Состояние переключается на `effective_height`; reveal-ramps снова открываются, пока shielded notes остаются валидными.                           | Используется для окон обслуживания или аудиторских проверок.                                          |
| ShieldedOnly       | TransparentOnly  | Governance должна доказать `shielded_supply == 0` или подготовить подписанный план `EmergencyUnshield` (нужны подписи аудиторов). | Runtime открывает окно `Convertible` перед `effective_height`; на этой высоте конфиденциальные инструкции жестко проваливаются, и актив возвращается в режим только прозрачных операций. | Выход последней инстанции. Переход автоматически отменяется, если какая-либо конфиденциальная note расходуется в окне. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` очищает ожидающее изменение.                                                        | `pending_transition` удаляется немедленно.                                                                          | Сохраняет статус-кво; показано для полноты.                                             |

Переходы, не указанные выше, отклоняются при подаче в governance. Runtime проверяет предпосылки прямо перед применением запланированного перехода; несоответствие возвращает актив в предыдущий режим и отправляет `PolicyTransitionPrerequisiteFailed` в телеметрию и события блока.

### Последовательность миграции

1. **Подготовить реестры:** Активировать все записи verifier и параметров, которые требуются целевой политике. Узлы объявляют получившиеся `conf_features`, чтобы peers могли проверить совместимость.
2. **Спланировать переход:** Отправить `ScheduleConfidentialPolicyTransition` с `effective_height`, учитывающей `policy_transition_delay_blocks`. При движении к `ShieldedOnly` указать окно конверсии (`window ≥ policy_transition_window_blocks`).
3. **Опубликовать инструкции для операторов:** Зафиксировать полученный `transition_id` и распространить runbook on/off-ramp. Кошельки и аудиторы подписываются на `/v1/confidential/assets/{id}/transitions`, чтобы узнать высоту открытия окна.
4. **Применение окна:** Когда окно открывается, runtime переключает политику в `Convertible`, эмитирует `PolicyTransitionWindowOpened { transition_id }` и начинает отклонять конфликтующие governance-запросы.
5. **Финализировать или отменить:** На `effective_height` runtime проверяет предпосылки перехода (нулевое прозрачное предложение, отсутствие emergency withdrawals и т.п.). Успех переключает политику в запрошенный режим; ошибка эмитирует `PolicyTransitionPrerequisiteFailed`, очищает pending transition и оставляет политику без изменений.
6. **Обновления схемы:** После успешного перехода governance повышает версию схемы актива (например, `asset_definition.v2`), а CLI tooling требует `confidential_policy` при сериализации manifests. Документы по апгрейду genesis инструктируют операторов добавить настройки политик и отпечатки registry перед перезапуском валидаторов.

Новые сети, начинающие с включенной конфиденциальностью, кодируют желаемую политику непосредственно в genesis. Они все равно следуют чек-листу выше при изменении режимов после запуска, чтобы окна конверсии оставались детерминированными, а кошельки успевали адаптироваться.

### Версионирование и активация Norito manifests

- Genesis manifests ДОЛЖНЫ включать `SetParameter` для кастомного ключа `confidential_registry_root`. Payload — Norito JSON, соответствующий `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: опускайте поле (`null`), когда активных verifier entries нет, иначе передайте 32-байтную hex-строку (`0x…`), равную хэшу, вычисленному `compute_vk_set_hash` по verifier инструкциям в manifest. Узлы отказываются стартовать, если параметр отсутствует или хэш не совпадает с записанными изменениями registry.
- On-wire `ConfidentialFeatureDigest::conf_rules_version` встраивает версию layout manifest. Для сетей v1 он ДОЛЖЕН оставаться `Some(1)` и равен `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Когда ruleset эволюционирует, увеличьте константу, пересоздайте manifests и разверните бинарники синхронно; смешение версий заставляет валидаторов отклонять блоки с `ConfidentialFeatureDigestMismatch`.
- Activation manifests ДОЛЖНЫ связывать обновления registry, изменения жизненного цикла параметров и переходы политик, чтобы digest оставался консистентным:
  1. Примените планируемые изменения registry (`Publish*`, `Set*Lifecycle`) в офлайн-снимке состояния и вычислите digest после активации через `compute_confidential_feature_digest`.
  2. Эмитируйте `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`, используя вычисленный хэш, чтобы отстающие peers могли восстановить корректный digest, даже если они пропустили промежуточные registry инструкции.
  3. Добавьте инструкции `ScheduleConfidentialPolicyTransition`. Каждая инструкция должна цитировать выданный governance `transition_id`; manifests, которые его забывают, будут отклонены runtime.
  4. Сохраните байты manifest, SHA-256 отпечаток и digest, использованный в activation plan. Операторы проверяют все три артефакта перед голосованием, чтобы избежать разделения сети.
- Когда rollout требует отложенного cut-over, запишите целевую высоту в сопутствующий кастомный параметр (например, `custom.confidential_upgrade_activation_height`). Это дает аудиторам Norito-кодированное доказательство того, что валидаторы соблюли окно уведомления до вступления изменения digest.

## Жизненный цикл verifier и параметров
### ZK Registry
- Ledger хранит `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`, где `proving_system` сейчас фиксирован на `Halo2`.
- Пары `(circuit_id, version)` глобально уникальны; registry поддерживает вторичный индекс для поиска по метаданным circuit. Попытки зарегистрировать дубликат отклоняются при admission.
- `circuit_id` не может быть пустым, а `public_inputs_schema_hash` обязателен (обычно Blake2b-32 хэш канонического публичного ввода verifier). Admission отклоняет записи без этих полей.
- Инструкции governance включают:
  - `PUBLISH` для добавления `Proposed` entry только с metadata.
  - `ACTIVATE { vk_id, activation_height }` для планирования активации entry на границе эпохи.
  - `DEPRECATE { vk_id, deprecation_height }` для отметки последней высоты, где proofs могут ссылаться на entry.
  - `WITHDRAW { vk_id, withdraw_height }` для экстренного отключения; затронутые активы замораживают конфиденциальные траты после withdraw height, пока не активируются новые entries.
- Genesis manifests автоматически эмитируют кастомный параметр `confidential_registry_root` с `vk_set_hash`, совпадающим с активными entries; валидация кросс-проверяет этот digest с локальным состоянием registry до вступления узла в консенсус.
- Регистрация или обновление verifier требует `gas_schedule_id`; проверка требует, чтобы запись registry была `Active`, присутствовала в индексе `(circuit_id, version)`, и чтобы Halo2 proofs содержали `OpenVerifyEnvelope`, у которого `circuit_id`, `vk_hash` и `public_inputs_schema_hash` совпадают с записью registry.

### Proving Keys
- Proving keys остаются off-ledger, но на них ссылаются content-addressed идентификаторы (`pk_cid`, `pk_hash`, `pk_len`), опубликованные вместе с metadata verifier.
- Wallet SDKs загружают PK данные, проверяют хэши и кэшируют локально.

### Pedersen & Poseidon Parameters
- Отдельные registry (`PedersenParams`, `PoseidonParams`) зеркалируют контроль жизненного цикла verifier, каждая с `params_id`, хэшами генераторов/констант, высотами активации, deprecation и withdraw.
- Commitments и хэши доменно разделяют `params_id`, поэтому ротация параметров никогда не переиспользует битовые паттерны из устаревших наборов; ID встраивается в commitments notes и доменные теги nullifier.

## Детерминированный порядок и nullifiers
- Каждый актив поддерживает `CommitmentTree` с `next_leaf_index`; блоки добавляют commitments в детерминированном порядке: итерация транзакций по порядку блока; внутри каждой транзакции — shielded outputs по возрастанию сериализованного `output_idx`.
- `note_position` выводится из оффсетов дерева, но **не** входит в nullifier; он используется только для путей membership в proof witness.
- Стабильность nullifier при reorg обеспечивается дизайном PRF; вход PRF связывает `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, а anchors ссылаются на исторические Merkle roots, ограниченные `max_anchor_age_blocks`.

## Поток ledger
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Требует политику актива `Convertible` или `ShieldedOnly`; admission проверяет authority актива, извлекает текущий `params_id`, сэмплирует `rho`, эмитирует commitment, обновляет Merkle tree.
   - Эмитирует `ConfidentialEvent::Shielded` с новым commitment, Merkle root delta и hash вызова транзакции для audit trail.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Syscall VM проверяет proof по записи registry; host убеждается, что nullifiers не использованы, commitments добавлены детерминированно, а anchor свежий.
   - Ledger записывает `NullifierSet` entries, хранит зашифрованные payloads для получателей/аудиторов и эмитирует `ConfidentialEvent::Transferred`, суммируя nullifiers, упорядоченные outputs, proof hash и Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - Доступно только для активов `Convertible`; proof подтверждает, что значение note равно раскрытой сумме, ledger начисляет прозрачный баланс и сжигает shielded note, помечая nullifier как потраченный.
   - Эмитирует `ConfidentialEvent::Unshielded` с публичной суммой, использованными nullifiers, идентификаторами proof и hash вызова транзакции.

## Дополнения data model
- `ConfidentialConfig` (новый раздел конфигурации) с флагом включения, `assume_valid`, knobs газа/лимитов, окном anchor и backend verifier.
- `ConfidentialNote`, `ConfidentialTransfer` и `ConfidentialMint` схемы Norito с явным байтом версии (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` оборачивает AEAD memo bytes в `{ version, ephemeral_pubkey, nonce, ciphertext }`, по умолчанию `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` для раскладки XChaCha20-Poly1305.
- Канонические векторы key-derivation лежат в `docs/source/confidential_key_vectors.json`; и CLI, и Torii endpoint регрессируют по этим фикстурам.
- `asset::AssetDefinition` получает `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` сохраняет привязку `(backend, name, commitment)` для transfer/unshield verifiers; исполнение отклоняет proofs, у которых referenced или inline verifying key не совпадает с зарегистрированным commitment.
- `CommitmentTree` (по активу с frontier checkpoints), `NullifierSet` с ключом `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` хранятся в world state.
- Mempool поддерживает временные структуры `NullifierIndex` и `AnchorIndex` для раннего обнаружения дубликатов и проверки возраста anchor.
- Обновления схемы Norito включают канонический порядок public inputs; round-trip tests гарантируют детерминированное кодирование.
- Round-trip зашифрованных payloads зафиксирован unit tests (`crates/iroha_data_model/src/confidential.rs`). Векторные файлы кошельков далее добавят канонические AEAD transcripts для аудиторов. `norito.md` документирует on-wire header для envelope.

## Интеграция с IVM и syscall
- Ввести syscall `VERIFY_CONFIDENTIAL_PROOF`, принимающий:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` и результирующий `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall загружает metadata verifier из registry, применяет лимиты размеров/времени, списывает детерминированный gas и применяет delta только при успехе proof.
- Host предоставляет read-only trait `ConfidentialLedger` для получения снимков Merkle root и статуса nullifier; библиотека Kotodama дает helpers для сборки witness и валидации схем.
- Документация pointer-ABI обновлена, чтобы прояснить layout proof buffer и registry handles.

## Согласование возможностей узлов
- Handshake объявляет `feature_bits.confidential` вместе с `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Участие валидатора требует `confidential.enabled=true`, `assume_valid=false`, идентичных идентификаторов backend verifier и совпадающих digest; несоответствия завершают handshake с `HandshakeConfidentialMismatch`.
- Конфигурация поддерживает `assume_valid` только для observer узлов: когда выключено, встреча конфиденциальных инструкций дает детерминированный `UnsupportedInstruction` без panic; когда включено, observers применяют объявленные state deltas без проверки proofs.
- Mempool отклоняет конфиденциальные транзакции, если локальная capability отключена. Gossip filters избегают отправки shielded транзакций несовместимым peers, но слепо пересылают неизвестные verifier IDs в пределах лимитов размера.

### Матрица совместимости handshake

| Объявление удаленной стороны | Итог для валидаторских узлов | Примечания оператора |
|-----------------------------|------------------------------|----------------------|
| `enabled=true`, `assume_valid=false`, backend совпадает, digest совпадает | Принят | Peer достигает состояния `Ready` и участвует в proposal, vote и RBC fan-out. Ручных действий не требуется. |
| `enabled=true`, `assume_valid=false`, backend совпадает, digest устарел или отсутствует | Отклонен (`HandshakeConfidentialMismatch`) | Remote должна применить ожидаемые активации registry/параметров или дождаться запланированной `activation_height`. Пока не исправлено, узел остается обнаруживаемым, но не входит в ротацию консенсуса. |
| `enabled=true`, `assume_valid=true` | Отклонен (`HandshakeConfidentialMismatch`) | Валидаторы требуют проверку proofs; настройте remote как observer с ingress только через Torii или переключите `assume_valid=false` после включения полной проверки. |
| `enabled=false`, поля handshake отсутствуют (старый билд) или backend verifier отличается | Отклонен (`HandshakeConfidentialMismatch`) | Устаревшие или частично обновленные peers не могут войти в сеть консенсуса. Обновите их до текущего релиза и убедитесь, что backend + digest совпадают до переподключения. |

Observer узлы, намеренно пропускающие проверку proofs, не должны открывать консенсусные соединения с валидаторами, работающими с capability gates. Они могут принимать блоки через Torii или архивные API, но сеть консенсуса отвергает их, пока они не объявят совместимые возможности.

### Политика удаления reveal и retention nullifier

Конфиденциальные ledgers должны сохранять достаточно истории для доказательства свежести notes и воспроизведения governance-аудитов. Политика по умолчанию, применяемая `ConfidentialLedger`, такова:

- **Хранение nullifier:** хранить потраченные nullifiers минимум `730` дней (24 месяца) после высоты траты или дольше, если требуется регулятором. Операторы могут увеличить окно через `confidential.retention.nullifier_days`. Nullifiers, которые моложе окна, ДОЛЖНЫ оставаться доступными через Torii, чтобы аудиторы могли доказать отсутствие double-spend.
- **Удаление reveal:** прозрачные reveal (`RevealConfidential`) удаляют соответствующие commitments немедленно после финализации блока, но использованный nullifier остается под правилом retention выше. События reveal (`ConfidentialEvent::Unshielded`) фиксируют публичную сумму, получателя и hash proof, чтобы реконструкция исторических reveal не требовала удаленного ciphertext.
- **Frontier checkpoints:** фронтиры commitments поддерживают rolling checkpoints, покрывающие большее из `max_anchor_age_blocks` и окна retention. Узлы компактизируют более старые checkpoints только после истечения всех nullifiers в интервале.
- **Ремедиация устаревшего digest:** если `HandshakeConfidentialMismatch` поднят из-за дрейфа digest, операторам следует (1) проверить, что окна retention nullifier совпадают по кластеру, (2) запустить `iroha_cli app confidential verify-ledger` для пересчета digest по сохраненному набору nullifier, и (3) переразвернуть обновленный manifest. Любые nullifiers, удаленные раньше срока, должны быть восстановлены из холодного хранения перед повторным входом в сеть.

Документируйте локальные переопределения в operations runbook; governance-политики, расширяющие окно retention, должны синхронно обновлять конфигурацию узлов и планы архивного хранения.

### Процедура эвикции и восстановления

1. При dial `IrohaNetwork` сравнивает объявленные capabilities. Любое несоответствие поднимает `HandshakeConfidentialMismatch`; соединение закрывается, а peer остается в discovery queue без перевода в `Ready`.
2. Ошибка фиксируется в логе сетевого сервиса (включая удаленный digest и backend), и Sumeragi никогда не планирует peer для proposal или voting.
3. Операторы устраняют проблему, выравнивая registries и параметрические наборы (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) или подготавливая `next_conf_features` с согласованной `activation_height`. Как только digest совпадает, следующий handshake проходит автоматически.
4. Если устаревший peer сумеет разослать блок (например, через архивный replay), валидаторы детерминированно отклоняют его с `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, сохраняя консистентность состояния ledger по сети.

### Replay-safe handshake flow

1. Каждая исходящая попытка выделяет свежий материал Noise/X25519. Подписываемый handshake payload (`handshake_signature_payload`) конкатенирует локальные и удаленные ephemeral public keys, Norito-кодированный advertised socket address и — если скомпилировано с `handshake_chain_id` — идентификатор chain. Сообщение AEAD-шифруется перед отправкой.
2. Ответчик пересчитывает payload с обратным порядком ключей peer/local и проверяет подпись Ed25519, встроенную в `HandshakeHelloV1`. Поскольку обе ephemeral keys и advertised address входят в домен подписи, replay захваченного сообщения против другого peer или восстановление устаревшего соединения детерминированно проваливает валидацию.
3. Confidential capability flags и `ConfidentialFeatureDigest` передаются внутри `HandshakeConfidentialMeta`. Получатель сравнивает tuple `{ enabled, assume_valid, verifier_backend, digest }` со своим локально настроенным `ConfidentialHandshakeCaps`; любое несоответствие завершает handshake с `HandshakeConfidentialMismatch` до перехода транспорта в `Ready`.
4. Операторы ДОЛЖНЫ пересчитать digest (через `compute_confidential_feature_digest`) и перезапустить узлы с обновленными registries/policies перед повторным подключением. Peers, рекламирующие старые digests, продолжают проваливать handshake, предотвращая возвращение устаревшего состояния в набор валидаторов.
5. Успехи и ошибки handshake обновляют стандартные счетчики `iroha_p2p::peer` (`handshake_failure_count`, error taxonomy helpers) и эмитят структурированные логи с тегами remote peer ID и fingerprint digest. Отслеживайте эти индикаторы, чтобы выявлять попытки replay или неверные конфигурации во время rollout.

## Управление ключами и payloads
- Иерархия derivation ключей на аккаунт:
  - `sk_spend` → `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- Зашифрованные payloads notes используют AEAD с ECDH-derived shared keys; опциональные auditor view keys могут быть прикреплены к outputs в соответствии с политикой актива.
- Дополнения CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, аудит-инструменты для расшифровки memo и helper `iroha app zk envelope` для создания/инспекции Norito memo envelopes офлайн.

## Газ, лимиты и DoS-контроли
- Детерминированный gas schedule:
  - Halo2 (Plonkish): базовый `250_000` gas + `2_000` gas на каждый public input.
  - `5` gas за байт proof, плюс per-nullifier (`300`) и per-commitment (`500`) начисления.
  - Операторы могут переопределять эти константы через конфигурацию узла (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); изменения применяются при старте или hot-reload слоя конфигурации и детерминированно распространяются по кластеру.
- Жесткие лимиты (настраиваемые значения по умолчанию):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Proofs, превышающие `verify_timeout_ms`, детерминированно прерывают инструкцию (governance-голосования эмитят `proof verification exceeded timeout`, `VerifyProof` возвращает ошибку).
- Дополнительные квоты обеспечивают живучесть: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, и `max_public_inputs` ограничивают block builders; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) управляет retention frontier checkpoints.
- Runtime теперь отклоняет транзакции, превышающие эти per-transaction или per-block лимиты, эмитируя детерминированные ошибки `InvalidParameter` и не изменяя состояние ledger.
- Mempool предварительно фильтрует конфиденциальные транзакции по `vk_id`, длине proof и возрасту anchor до вызова verifier, чтобы ограничить потребление ресурсов.
- Проверка детерминированно останавливается при timeout или нарушении лимитов; транзакции падают с явными ошибками. SIMD backends опциональны, но не меняют accounting газа.

### Базовые калибровки и критерии приемки
- **Reference platforms.** Калибровочные прогоны ДОЛЖНЫ покрывать три профиля оборудования ниже. Прогоны без всех профилей отклоняются при ревью.

  | Профиль | Архитектура | CPU / Instance | Флаги компилятора | Назначение |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) или Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Устанавливает базовые значения без векторных инструкций; используется для настройки fallback cost tables. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | default release | Валидирует путь AVX2; проверяет, что SIMD ускорения укладываются в допуск нейтрального gas. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | default release | Гарантирует, что backend NEON остается детерминированным и согласован с x86 расписаниями. |

- **Benchmark harness.** Все отчеты по калибровке газа ДОЛЖНЫ быть получены с:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` для подтверждения детерминированной фикстуры.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` всякий раз, когда меняются стоимости VM opcode.

- **Fixed randomness.** Экспортируйте `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` перед запуском бенчей, чтобы `iroha_test_samples::gen_account_in` переключился на детерминированный путь `KeyPair::from_seed`. Harness печатает `IROHA_CONF_GAS_SEED_ACTIVE=…` один раз; если переменная отсутствует, ревью ДОЛЖНО провалиться. Любые новые утилиты калибровки должны продолжать учитывать эту env var при введении вспомогательной случайности.

- **Result capture.**
  - Загружайте Criterion summaries (`target/criterion/**/raw.csv`) для каждого профиля в release artefact.
  - Храните производные метрики (`ns/op`, `gas/op`, `ns/gas`) в [Confidential Gas Calibration ledger](./confidential-gas-calibration) вместе с git commit и версией компилятора.
  - Держите последние две baselines на профиль; удаляйте более старые снимки после проверки нового отчета.

- **Acceptance tolerances.**
  - Дельты газа между `baseline-simd-neutral` и `baseline-avx2` ДОЛЖНЫ оставаться ≤ ±1.5%.
  - Дельты газа между `baseline-simd-neutral` и `baseline-neon` ДОЛЖНЫ оставаться ≤ ±2.0%.
  - Калибровочные предложения, выходящие за эти пороги, требуют либо корректировки расписания, либо RFC с объяснением расхождения и мерами.

- **Review checklist.** Отправители отвечают за:
  - Включение `uname -a`, выдержек `/proc/cpuinfo` (model, stepping) и `rustc -Vv` в калибровочный лог.
  - Проверку, что `IROHA_CONF_GAS_SEED` отображается в выводе бенчей (бенчи печатают активный seed).
  - Убедиться, что pacemaker и confidential verifier feature flags совпадают с production (`--features confidential,telemetry` при запуске бенчей с Telemetry).

## Конфигурация и операции
- `iroha_config` получает секцию `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Телеметрия эмитит агрегированные метрики: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, и `confidential_policy_transitions_total`, не раскрывая plaintext данные.
- RPC surfaces:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Стратегия тестирования
- Детерминизм: рандомизированное перемешивание транзакций в блоках дает одинаковые Merkle roots и nullifier sets.
- Устойчивость к reorg: симуляция многоблочных reorg с anchors; nullifiers остаются стабильными, а устаревшие anchors отклоняются.
- Инварианты газа: одинаковый расход газа на узлах с и без SIMD ускорения.
- Граничное тестирование: proofs на потолках размера/gas, максимальные входы/выходы, enforcement timeout.
- Жизненный цикл: операции governance для активации/деактивации verifier и параметров, тесты трат при ротации.
- Policy FSM: разрешенные/запрещенные переходы, задержки pending transition и отклонения mempool вокруг effective heights.
- Чрезвычайные ситуации registry: emergency withdrawal замораживает затронутые активы на `withdraw_height` и отклоняет proofs после нее.
- Capability gating: валидаторы с несовпадающими `conf_features` отклоняют блоки; observers с `assume_valid=true` продолжают следовать без влияния на консенсус.
- Эквивалентность состояния: validator/full/observer узлы создают идентичные state roots на канонической цепи.
- Негативный fuzzing: поврежденные proofs, oversized payloads и collisions nullifier детерминированно отклоняются.

## Миграция и совместимость
- Feature-gated rollout: до завершения Phase C3 `enabled` по умолчанию `false`; узлы объявляют capabilities перед входом в набор валидаторов.
- Прозрачные активы не затронуты; конфиденциальные инструкции требуют записей registry и переговоров capability.
- Узлы, собранные без поддержки confidential, детерминированно отклоняют соответствующие блоки; они не могут входить в набор валидаторов, но могут работать как observers с `assume_valid=true`.
- Genesis manifests включают начальные registry entries, наборы параметров, конфиденциальные политики для активов и опциональные auditor keys.
- Операторы следуют опубликованным runbooks для ротации registry, переходов политик и emergency withdrawal, чтобы поддерживать детерминированные апгрейды.

## Незавершенная работа
- Провести benchmark наборов параметров Halo2 (размер circuit, стратегия lookup) и записать результаты в calibration playbook, чтобы обновить дефолты gas/timeout вместе со следующим обновлением `confidential_assets_calibration.md`.
- Завершить политики раскрытия аудиторам и связанные selective-viewing API, подключив утвержденный workflow в Torii после одобрения governance.
- Расширить witness encryption схему для multi-recipient outputs и batched memos, задокументировать формат envelope для SDK implementers.
- Заказать внешнюю security review circuits, registry и процедур ротации параметров и архивировать результаты рядом с внутренними audit reports.
- Специфицировать API сверки spentness для аудиторов и опубликовать guidance по view-key scope, чтобы вендоры кошельков реализовали те же семантики аттестации.

## Этапы реализации
1. **Phase M0 — Stop-Ship Hardening**
   - ✅ Derivation nullifier теперь следует дизайну Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) с детерминированным порядком commitments в обновлениях ledger.
   - ✅ Исполнение применяет лимиты размера proof и квоты конфиденциальных операций per-transaction/per-block, отклоняя транзакции сверх лимитов детерминированными ошибками.
   - ✅ P2P handshake объявляет `ConfidentialFeatureDigest` (backend digest + отпечатки registry) и детерминированно отклоняет несоответствия через `HandshakeConfidentialMismatch`.
   - ✅ Удалены panics в конфиденциальных execution paths и добавлен role gating для несовместимых узлов.
   - ⚪ Применить бюджеты timeout для verifier и bounds глубины reorg для frontier checkpoints.
     - ✅ Бюджеты timeout для проверки применены; proofs, превышающие `verify_timeout_ms`, теперь детерминированно падают.
     - ✅ Frontier checkpoints теперь учитывают `reorg_depth_bound`, удаляя checkpoints старше заданного окна при сохранении детерминированных снимков.
   - Ввести `AssetConfidentialPolicy`, policy FSM и enforcement gates для инструкций mint/transfer/reveal.
   - Коммитить `conf_features` в заголовках блоков и отказывать в участии валидаторов при расхождении registry/parameter digests.
2. **Phase M1 — Registries & Parameters**
   - Внедрить registry `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` с governance ops, genesis anchoring и управлением кэшем.
   - Подключить syscall для обязательных registry lookups, gas schedule IDs, schema hashing и size checks.
   - Поставить формат зашифрованного payload v1, векторы derivation ключей кошелька и поддержку CLI для управления confidential keys.
3. **Phase M2 — Gas & Performance**
   - Реализовать детерминированный gas schedule, счетчики per-block и benchmark harnesses с телеметрией (verify latency, proof sizes, mempool rejections).
   - Укрепить checkpoints CommitmentTree, LRU loading и nullifier indices для мульти-активных нагрузок.
4. **Phase M3 — Rotation & Wallet Tooling**
   - Включить поддержку multi-parameter и multi-version proofs; поддержать governance-driven activation/deprecation с transition runbooks.
   - Поставить миграционные потоки для wallet SDK/CLI, workflow сканирования аудиторов и tooling сверки spentness.
5. **Phase M4 — Audit & Ops**
   - Предоставить workflows для auditor keys, selective disclosure API и operational runbooks.
   - Запланировать внешнюю криптографическую/безопасностную проверку и опубликовать выводы в `status.md`.

Каждая фаза обновляет roadmap milestones и связанные тесты, чтобы сохранить детерминированные гарантии исполнения для blockchain сети.
