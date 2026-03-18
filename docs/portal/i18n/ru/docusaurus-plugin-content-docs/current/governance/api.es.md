---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Стадо: боррадор/бочето, чтобы сопровождать расходы по осуществлению правительства. Форма может быть изменена во время реализации. Детерминизм и политика RBAC соответствуют нормативным ограничениям; Torii может совершать/отправлять транзакции, если они соответствуют `authority` и `private_key`, а клиенты наоборот строят и отправляют `/transaction`.

Резюме
- Все конечные точки в формате JSON. Для выполнения транзакций, ответы включают `tx_instructions` - список следующих инструкций:
  - `wire_id`: идентификатор регистрации для указания типа инструкции.
  - `payload_hex`: байты полезной нагрузки Norito (шестнадцатеричный)
- Если вы пропорционируете `authority` и `private_key` (или `private_key` в DTO бюллетеней), Torii фирма и передача транзакции и развитие `tx_instructions`.
- Напротив, клиенты получают SignedTransaction с использованием полномочий и идентификатора цепочки, затем фирмы и получения POST a `/transaction`.
- Кобертура SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` devuelve `GovernanceProposalResult` (нормализация статуса/вида кампоса), `ToriiClient.get_governance_referendum_typed` devuelve `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` devuelve `GovernanceTally`, `ToriiClient.get_governance_locks_typed`, версия `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed`, версия `GovernanceUnlockStats`, y `ToriiClient.list_governance_instances_typed`, версия `GovernanceInstancesPage`, доступ к ним невозможен. на всей поверхности управления с примерами использования в README.
- Клиентский клиент Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` и `ToriiClient.enact_proposal`, дополнительные пакеты типов `GovernanceInstructionDraft` (включен в компьютер `tx_instructions` от Torii), удален Руководство по parseo JSON и компонентам скриптов flujos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` отображает вспомогательные советы для предложений, референдумов, подсчетов, блокировок, статистики разблокировки и т. д. `listGovernanceInstances(namespace, options)` для конечных точек совета (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) для клиентов Node.js, которые можно открыть на странице `/v1/gov/instances/{ns}` и обеспечить возможность ответа на запросы VRF в списке экземпляров существующих контрактов.

Конечные точки

- ПОСТ `/v1/gov/proposals/deploy-contract`
  - Запрос (JSON):
    {
      "пространство имен": "приложения",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64шестнадцатеричный",
      "abi_hash": "blake2b32:..." | "...64шестнадцатеричный",
      "abi_version": "1",
      "окно": { "нижний": 12345, "верхний": 12400 },
      "authority": "i105…?",
      "private_key": "...?"
    }
  - Ответ (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Проверка: канонизированные узлы `abi_hash` для `abi_version` будут проверены и отменены. Для `abi_version = "v1"`, честь была достигнута `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API контрактов (развертывание)
- ПОСТ `/v1/contracts/deploy`
  - Solicitud: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Совместимость: вычисление `code_hash` строки программы IVM и `abi_hash` заголовка `abi_version`, затем передача `RegisterSmartContractCode` (манифест) и `RegisterSmartContractBytes` (байты). `.to` завершено) под номером `authority`.
  - Ответ: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Сообщение:
    - GET `/v1/contracts/code/{code_hash}` -> раскрыть манифесто альмасенадо
    - GET `/v1/contracts/code-bytes/{code_hash}` -> развернуть `{ code_b64 }`
- ПОСТ `/v1/contracts/instance`
  - Solicitud: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Совместимость: выберите байт-код и немедленно активируйте карту `(namespace, contract_id)` через `ActivateContractInstance`.
  - Ответ: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Служба псевдонимов
- ПОСТ `/v1/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - Ответ: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценки. Фактическая доблесть: `blake2b512-mock`.
  - Примечания: оцените макетную детерминированность, которую применяет Blake2b512 с разделением домена `iroha.alias.voprf.mock.v1`. Предназначен для подготовки к работе производственного конвейера VOPRF с кабелем Iroha.
  - Ошибки: HTTP `400` в неправильном шестнадцатеричном формате. Torii разработайте конверт Norito `ValidationFail::QueryFailed::Conversion` с сообщением об ошибке декодера.
- ПОСТ `/v1/aliases/resolve`
  - Запрос: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Ответ: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Примечания: требуется создание моста ISO во время выполнения (`[iso_bridge.account_aliases]` и `iroha_config`). Torii нормализовать псевдоним, удалив пробелы и отступив перед поиском. Devuelve 404, когда псевдоним не существует, и 503, когда мост времени выполнения ISO устарел.
- ПОСТ `/v1/aliases/resolve_index`
  - Запрос: { "индекс": 0 }
  - Ответ: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Примечания: индексы псевдонимов назначаются в форме, определенной в следующем порядке конфигурации (отсчет от 0). Клиенты могут кэшировать ответы в автономном режиме, чтобы создавать проходы аудиторий для событий подтверждения псевдонимов.

Топе де тамано де кодиго
- Пользовательский параметр: `max_contract_code_bytes` (JSON u64)
  - Контроль максимального разрешения (в байтах) для записи кода контракта в цепочке.
  - По умолчанию: 16 МБ. Лос-ноды заменены `RegisterSmartContractBytes`, когда изображение `.to` выходит за пределы топа с ошибкой нарушения инварианта.
  - Оперативные операторы могут настроить отправку `SetParameter(Custom)` с `id = "max_contract_code_bytes"` и числовое значение полезной нагрузки.- ПОСТ `/v1/gov/ballots/zk`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Примечания:
    - Когда публичные входы в схему включают `owner`, `amount` и `duration_blocks`, и проверка подлинности против конфигурации VK, мы создаем или расширяем блокировку правительства для `election_id` с `owner`. Постоянное скрытое направление (`unknown`); соло актуализируется сумма/срок действия. Las revotaciones son monotonic: сумма и срок действия соло увеличиваются (el nodo aplica max(amount, prev.amount) y max(expiry, prev.expiry)).
    - Отмены ZK, которые намереваются уменьшить сумму или срок действия, должны быть заменены сервисом с диагностикой `BallotRejected`.
    - Выброс контратаки для звонка `ZK_VOTE_VERIFY_BALLOT` перед запечатыванием `SubmitBallot`; лос-хосты не имеют защелки на одном месте.

- ПОСТ `/v1/gov/ballots/plain`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Да|Нет|Воздержаться" }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Примечания: las revotaciones son de Solo Extension - новое голосование не может уменьшить сумму или истечение срока действия существующего блока. `owner` должен быть указан в качестве органа транзакции. Минимальная продолжительность — `conviction_step_blocks`.

- ПОСТ `/v1/gov/finalize`
  - Solicitud: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Эффект в цепочке (фактическое): обнародовать запрос на развертывание, вставленный в `ContractManifest` minimo с клавой `code_hash` с `abi_hash`, который был успешно выполнен и отмечен как введенный в действие. Если вы существуете манифест для `code_hash` с `abi_hash` отдельно, обнародование будет повторено.
  - Примечания:
    - Для выборов ZK, руты контрато должны быть отключены `ZK_VOTE_VERIFY_TALLY` до выброса `FinalizeElection`; лос-хосты не имеют защелки на одном месте. `FinalizeReferendum` повторяет референдумы ZK, когда подсчет выборов завершен.
    - El cierre auto en `h_end` выдает одобрено/отклонено отдельно для референдумов Plain; лос-референдо ZK навсегда закрыто, потому что вы завидуете окончательному подсчету и выбросу `FinalizeReferendum`.
    - Лас-компробации явки в США в одиночку одобряют+отклоняют; воздерживаться от участия в выборах без учета явки.- ПОСТ `/v1/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Примечание: Torii отправляет фирму по транзакции, когда она соответствует `authority`/`private_key`; Напротив, усовершенствуйте esqueleto, чтобы клиенты укрепились и завидовали. Прообраз является необязательным и фактически информативным.

- ПОЛУЧИТЬ `/v1/gov/proposals/{id}`
  - Путь `{id}`: шестнадцатеричный идентификатор объекта (64 символа).
  - Ответ: { "найдено": bool, "предложение": { ... }? }

- ПОЛУЧИТЬ `/v1/gov/locks/{rid}`
  - Путь `{rid}`: идентификатор строки референдума.
  - Ответ: { "найдено": bool, "referendum_id": "rid", "locks": { ... }? }

- ПОЛУЧИТЬ `/v1/gov/council/current`
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." }, ...] }
  - Примечания: ослабить совет, сохраняющийся, когда он существует; В противоположность этому происходит ответ, детерминированный с использованием актива, сконфигурированного и скрытого (отображение спецификации VRF, которая выполняется в естественных условиях в сети).

- POST `/v1/gov/council/derive-vrf` (функция: gov_vrf)
  - Запрос: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Совместимость: проверка проверки VRF каждого кандидата против канонического ввода `chain_id`, `epoch` и маяка последнего хэша блока; порядок байтов для разрешения споров с тай-брейками; devuelve los top `committee_size` miembros. Никакого упорства.
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk en G1, доказательство en G2 (96 байт). Small = pk en G2, доказательство en G1 (48 байт). Входы были разделены для домена и включены `chain_id`.

### Значения по умолчанию (iroha_config `gov.*`)

Совет по ответу, используемый для Torii, пока не существует списка, который сохраняется при параметризации через `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Эквивалентные переопределения:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` ограничивает количество ответных действий, пока не сохраняется совет, `parliament_term_blocks` определяет долготу эпохи, используемой для получения семени (`epoch = floor(height / term_blocks)`), `parliament_min_stake` применение минимальной ставки (в едином стиле) Минимальные значения) для выбора элегантного актива и `parliament_eligibility_asset_id` выбирают, какой баланс актива можно найти, чтобы построить конъюнктуру кандидатов.

Проверка губернатора VK не обходится без обхода: проверка бюллетеней всегда требует наличия ключа проверки `Active` с встроенными байтами, и от этого не требуется зависимости от переключателей проверки для пропуска проверки.

РБАК
- Для выброса по цепочке требуются разрешения:
  - Предложения: `CanProposeContractDeployment{ contract_id }`
  - Бюллетени: `CanSubmitGovernanceBallot{ referendum_id }`
  - Принятие: `CanEnactGovernance`
  - Управление советом (будущее): `CanManageParliament`Защита пространств имен
- Пользовательский параметр `gov_protected_namespaces` (массив строк JSON), обеспечивающий пропускную способность для развертывания в списках пространств имен.
- Клиентам необходимо включать клавиши метаданных транзакций для развертывания управляемых и защищенных пространств имен:
  - `gov_namespace`: пространство имен objetivo (например, «приложения»)
  - `gov_contract_id`: логический идентификатор контракта в пространстве имен.
- `gov_manifest_approvers`: необязательный массив JSON для идентификаторов учетных записей валидаторов. Когда манифест де-лейн объявляет кворума мэра, требуется допуск к полномочиям по транзакциям, если списки списков для удовлетворения кворума манифестов.
- Телеметрия показывает контакты входа через `governance_manifest_admission_total{result}` для того, чтобы отдельные операторы допускали выход из рутов `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, y `runtime_hook_rejected`.
- Телеметрия демонстрирует руту принудительного исполнения через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`) для того, чтобы операторы проверяли неверные проверки.
- Доступен список разрешенных пространств имен, опубликованных в наших манифестах. Любая транзакция, которая соответствует `gov_namespace`, соответствует `gov_contract_id`, а пространство имен должно быть доступно в наборе `protected_namespaces` манифеста. Los envios `RegisterSmartContractCode` без метаданных, которые необходимо изменить, когда защита является привычной.
- La допуск невозможен, что существует собственность губернатора, принятая для кортежа `(namespace, contract_id, code_hash, abi_hash)`; Напротив, проверка произошла из-за ошибки NotPermited.

Перехватчики обновления среды выполнения
- В манифестах полосы можно объявить `hooks.runtime_upgrade` для управления инструкциями по обновлению среды выполнения (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Кампос дель крючок:
  - `allow` (bool, по умолчанию `true`): если используется `false`, повторите все инструкции по обновлению во время выполнения.
  - `require_metadata` (bool, по умолчанию `false`): выберите ввод метаданных, специально предназначенный для `metadata_key`.
  - `metadata_key` (строка): имя метаданных, применяемое для перехватчика. По умолчанию `gov_upgrade_id`, если требуются метаданные или белый список разрешений.
  - `allowed_ids` (массив строк): дополнительный список разрешенных значений метаданных (без обрезки). Rechaza cuando el valor provisto no esta listado.
- Когда крючок присутствует, допустите приложение колы к политике метаданных до того, как транзакция попадет в колу. Неверные метаданные, количество удалений или количество разрешенных списков, созданных из-за определенной ошибки NotPermitted.
- Результат телеметрии через `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, которые включают в себя метаданные `gov_upgrade_id=<value>` (определенный ключ для манифеста) вместе с необходимой апробацией валидаторов, необходимой для кворума манифестов.Конечная точка удобства
- POST `/v1/gov/protected-namespaces` - применение `gov_protected_namespaces` прямо в этом месте.
  - Solicitud: { "пространства имен": ["приложения", "система"] }
  - Ответ: { "ок": правда, "применено": 1 }
  - Примечания: мысли для администрирования/тестирования; Требуется API токена, если он настроен. Для производства рекомендуется отправить фирменную транзакцию с `SetParameter(Custom)`.

Помощники CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Получите экземпляры контракта для пространства имен и проверки:
    - Байт-код Torii almacena для каждого `code_hash`, и его дайджест Blake2b-32 совпадают с `code_hash`.
    - El manifico almacenado bajo `/v1/contracts/code/{code_hash}` сообщает о значениях `code_hash` и `abi_hash` совпадений.
    - Существует правительственная собственность, введенная в действие для пункта `(namespace, contract_id, code_hash, abi_hash)`, полученная из-за хеширования идентификатора предложения, которое использует этот узел.
  - Отправьте отчет JSON с `results[]` по контракту (проблемы, резюме манифеста/кода/предложения), как резюме линейного залпа, который был выше (`--no-summary`).
  - Используйте для проверки защищенности пространств имен или проверки правильности развертывания контролируемых территорий.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Создайте JSON-файл метаданных, используемый при развертывании защищенных пространств имен, включая дополнительный `gov_manifest_approvers` для удовлетворения требований кворума манифестов.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — подсказки по блокировке, обязательные для `min_bond_amount > 0`, и некоторые подсказки, пропорциональные необходимости включать `owner`, `amount` и `duration_blocks`.
  — Проверяет канонические идентификаторы учетных записей, канонизирует 32-байтовые подсказки обнулителя и объединяет подсказки в `public_inputs_json` (с `--public <path>` для дополнительных переопределений).
  - Обнулитель получается из обязательства доказательства (общедоступный вклад) плюс `domain_tag`, `chain_id` и `election_id`; `--nullifier` проверяется на соответствие при поставке.
  - Возобновление линии сейчас демонстрирует `fingerprint=<hex>`, определенное производное от `CastZkBallot`, кодифицированное соединение с декодированными подсказками (`owner`, `amount`, `duration_blocks`, `direction`, когда это пропорционально).
  - Ответы CLI на `tx_instructions[]` с `payload_fingerprint_hex` декодируются в нескольких командах для последующей проверки кода, чтобы повторно реализовать декодификацию Norito.
  - Докажите, что подсказки о блокировке позволяют, чтобы этот узел излучал события `LockCreated`/`LockExtended` для голосования ZK, и это означает, что схема выдвинула свои ценности.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Псевдоним `--lock-amount`/`--lock-duration-blocks` отражает номера флагов ZK для парных в сценариях.
  - Возобновленное отражение `vote --mode zk` с указанием отпечатков пальцев кодифицированной инструкции и разборчивых полей для голосования (`owner`, `amount`, `duration_blocks`, `direction`), официальное подтверждение Rapida Antes de Firmar el esqueleto.Список экземпляров
- GET `/v1/gov/instances/{ns}` - список экземпляров контрактов, активируемых для пространства имен.
  - Параметры запроса:
    - `contains`: фильтрация подстроки `contract_id` (с учетом регистра)
    - `hash_prefix`: фильтр по шестнадцатеричному префиксу `code_hash_hex` (строчные буквы)
    - `offset` (по умолчанию 0), `limit` (по умолчанию 100, максимум 10_000)
    - `order`: номер `cid_asc` (по умолчанию), `cid_desc`, `hash_asc`, `hash_desc`
  - Ответ: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Вспомогательный SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) или `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Барридо де разблокировка (Операдор/Аудитория)
- ПОЛУЧИТЬ `/v1/gov/unlocks/stats`
  - Ответ: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Примечание: `last_sweep_height` отражает высоту блока, когда замки истекают и сохраняются. `expired_locks_now` вычисляет сканированные регистры блокировки с `expiry_height <= height_current`.
- ПОСТ `/v1/gov/ballots/zk-v1`
  - Solicitud (стиль DTO v1):
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "бэкэнд": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "владелец": "i105…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- ПОСТ `/v1/gov/ballots/zk-v1/ballot-proof` (характеристика: `zk-ballot`)
  - Примите JSON `BallotProof` и разработайте экран `CastZkBallot`.
  - Требование:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "голосование": {
        "бэкэнд": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 соперника ZK1 или H2*
        "root_hint": null, // необязательная 32-байтовая шестнадцатеричная строка (корень соответствия)
        "owner": null, // AccountId необязательно, если владелец схемы компрометирован
        "nullifier": null // необязательная 32-байтовая шестнадцатеричная строка (подсказка обнулителя)
      }
    }
  - Ответ:
    {
      «ок»: правда,
      «принято»: правда,
      "reason": "построить скелет транзакции",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Примечания:
    - Сервер карты `root_hint`/`owner`/`nullifier` является дополнительным после голосования `public_inputs_json` для `CastZkBallot`.
    - Байты конверта перекодируются в формате Base64 для полезной нагрузки инструкции.
    - Ответ `reason` поступил на `submitted transaction`, когда Torii был отправлен в бюллетень.
    - Эта конечная точка доступна только тогда, когда функция `zk-ballot` уже доступна.Процедура проверки CastZkBallot
- `CastZkBallot` декодирует отправку base64, обеспечивая и восстанавливая полезные данные в других или неправильных форматах (`BallotRejected` с `invalid or empty proof`).
- Хост должен выполнить проверку клавы для голосования после референдума (`vk_ballot`) или по умолчанию для губернатора и потребовать, чтобы существующий реестр был `Active`, и ввести в строку несколько байтов.
- Лос-байты верифицированного клавы будут перехешированы с `hash_vk`; необходимо отменить компромиссное решение перед проверкой для защиты от внесения в реестр фальсификаций (`BallotRejected` с `verifying key commitment mismatch`).
- Байты отправки отправляются на серверную часть, зарегистрированную через `zk::verify_backend`; транскрипции недействительны, как `BallotRejected` с `invalid proof`, и инструкция не определена.
- Доказательства должны раскрывать обязательства по голосованию и основу права на участие в качестве общественного мнения; корень должен соответствовать `eligible_root` выборов, а производный обнулитель должен соответствовать любой предоставленной подсказке.
- Прюбас экзитозас эмитен `BallotAccepted`; нуллификаторы дубликатов, корни элегибилидад вьехос или регрессии блокировки, которые производятся лас-разоны де-реказо существуют, описанные до этого в этом документе.

## Малое проведение валидаторов и согласованное согласие

### Flujo de порезы и тюремное заключение

Согласие `Evidence` кодифицировано в Norito, когда валидатор нарушает протокол. Если полезная нагрузка будет добавлена ​​в `EvidenceStore` в память, если раньше не было никаких нарушений, она материализуется на карте `consensus_evidence`, перераспределенной для WSV. Предыдущие регистры `sumeragi.npos.reconfig.evidence_horizon_blocks` (блоки `7200` по умолчанию) должны быть изменены для того, чтобы постоянный архив был открыт, но повторная регистрация была выполнена для операторов. Свидетельства в пределах горизонта также учитывают `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и косую задержку `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`); руководство может отменить штрафы с помощью `CancelConsensusEvidencePenalty` до того, как будет применено сокращение.

Las ofensas reconocidas se maean uno a uno a `EvidenceKind`; Лос-дискриминанты были установлены и изменены для модели данных:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** — фирма-валидатор хеширует в конфликте из-за неправильного кортежа `(phase,height,view,epoch)`.
- **InvalidQc** — сборщик сплетен и сертификат фиксации, содержащий форму детерминированных проверок (например, растровое изображение фирмы vacio).
- **InvalidProposal** — лидер предложения блокирует que Falla La Validacion Estructural (например, rompe la Regla de Locked-Chain).
- **Цензура** — подписанные квитанции об отправке показывают транзакцию, которая никогда не предлагалась/не совершалась.

Операторы и инструменты могут проверять и ретранслировать полезную нагрузку по маршрутам:

- Torii: `GET /v1/sumeragi/evidence` и `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, y `... submit --evidence-hex <payload>`.

La gobernanza должна переработать байты доказательств как каноническая процедура:1. **Запомните полезную нагрузку** до того, как он окажется. Архивируйте байты Norito вместе с метаданными высоты/вида.
2. **Подготовьтесь к наказанию** вставьте полезную нагрузку в референдум или инструкцию sudo (например, `Unregister::peer`). Выброс полезной нагрузки повторно действителен; Доказательства в неправильной форме или рансия могут быть определены.
3. **Программируйте топологию сегментации**, чтобы нарушитель проверки не мог быть немедленно повторно обработан. Типичные Flujos encolan `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)` с обновленным списком.
4. **Результаты проверки** через `/v1/sumeragi/evidence` и `/v1/sumeragi/status`, чтобы гарантировать, что контадор доказательств будет доставлен и что правительство обратится за помощью.

### Secuenciacion de consenso conjunto

Согласие на объединение гарантирует, что объединение валидаторов станет окончательным блоком фронта до того, как новое объединение станет сторонником. Среда выполнения включает правила через параметры:

- `SumeragiParameter::NextMode` и `SumeragiParameter::ModeActivationHeight` должны быть подтверждены в **mismo bloque**. `mode_activation_height` должен быть ограничен мэром, который меняет блок, который грузит обновление, пропорционально всем меньшим блокам отставания.
- `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) — это защита конфигурации, которая предотвращает передачу управления с задержкой в работе:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`) задерживает сокращение консенсуса, чтобы руководство могло отменить штрафы до их применения.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Среда выполнения и CLI отображают параметры, организованные через `/v1/sumeragi/params` и `iroha --output-format text ops sumeragi params`, для того, чтобы операторы подтверждали изменения активации и списки валидаторов.
- Автоматизация управления всегда будет:
  1. Завершить решение об удалении (или восстановлении) в соответствии с доказательствами.
  2. Введите следующую реконфигурацию с `mode_activation_height = h_current + activation_lag_blocks`.
  3. Монитор `/v1/sumeragi/status` должен быть таким же, как `effective_consensus_mode`, но сначала снова.

Другой сценарий, который автоматически проверяется или применяется косая черта **не требуется**, намерен активировать с задержкой выбора и пропустить параметры передачи; эти транзакции повторяются и дежан ла красный в предыдущем режиме.

## Особенности телеметрии- Показатели Prometheus экспортной активности правительства:
  - `governance_proposals_status{status}` (датчик) rastrea conteos de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (счетчик) увеличивается при входе в защищенные пространства имен или разрешении повторного развертывания.
  - `governance_manifest_activations_total{event}` (счетчик) регистрации вставок манифеста (`event="manifest_inserted"`) и привязок пространства имен (`event="instance_bound"`).
- `/status` включает объект `governance`, который отображает информацию о свойствах, отчеты об итогах защищенных пространств имен и список активаций манифеста (пространство имен, идентификатор контракта, хэш кода/ABI, высоту блока, временную метку активации). Операторы могут проконсультироваться с этим кампо, чтобы подтвердить, что объявления актуализируются и что шлюзы пространств имен защищены и применяются.
- Установка Grafana (`docs/source/grafana_governance_constraints.json`) и книга телеметрии в `telemetry.md` должны использоваться в качестве кабельных оповещений для атак, активаций манифестов или неожиданных срабатываний защиты пространств имен во время обновлений во время выполнения.