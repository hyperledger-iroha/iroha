---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Статут: Brouillon/esquisse pour concompagner les taches d'implementation de la gouvernance. Подвеска-переключатель форм Les formes peuvent, реализация. Le determinisme et la politique RBAC является противоречащим нормативам; Torii может быть подписантом/сообщителем транзакций, когда `authority` и `private_key` работают, а клиенты конструируют и соуметтируют `/transaction`.

Аперку
– Все конечные точки отсылают к JSON. Для потоков, которые производят транзакции, ответы включают `tx_instructions` — таблицу или дополнительные инструкции:
  - `wire_id`: идентификатор регистрации типа инструкции.
  - `payload_hex`: байты полезной нагрузки Norito (шестнадцатеричный)
- Если `authority` и `private_key` sont fournis (или `private_key` в бюллетенях DTO), Torii подписывает и совершает транзакцию и отсылает quand meme `tx_instructions`.
- Синон, клиенты собираются в SignedTransaction с полномочиями и идентификатором цепочки, могут быть подписаны и POST версии `/transaction`.
- Couverture SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` отсылка `GovernanceProposalResult` (нормализовать статус/вид полей), `ToriiClient.get_governance_referendum_typed` отсылка `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` отсылка `GovernanceTally`, `ToriiClient.get_governance_locks_typed` отправка `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` отправка `GovernanceUnlockStats` и `ToriiClient.list_governance_instances_typed` отправка `GovernanceInstancesPage`, импозантный тип доступа для всех поверхность управления с примерами использования в README.
- Клиентский код Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` и `ToriiClient.enact_proposal` отсылает к типам пакетов `GovernanceInstructionDraft` (qui encapsulent le squeette `tx_instructions` от Torii), исключает синтаксический анализ Вручную в формате JSON и включающие сценарии Flux Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` предоставляет типы помощников для предложений, референдумов, подсчетов, блокировок, статистики разблокировки и обслуживания `listGovernanceInstances(namespace, options)` плюс советы конечных точек (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) до тех пор, пока клиенты Node.js будут иметь возможность просматривать страницы `/v2/gov/instances/{ns}` и управлять рабочими процессами VRF в параллельном листинге существующих экземпляров контрактов.

Конечные точки

- ПОСТ `/v2/gov/proposals/deploy-contract`
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
  - Проверка: канонические данные `abi_hash` для l'`abi_version` Fourni и отвергнутые несоответствия. Налейте `abi_version = "v1"`, значение присутствует `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.Контракты API (развертывание)
- ПОСТ `/v2/contracts/deploy`
  - Запрос: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Формат: вычислите `code_hash` из корпуса программы IVM и `abi_hash` из `abi_version`, затем выберите `RegisterSmartContractCode` (манифест) и др. `RegisterSmartContractBytes` (байты `.to` завершаются) вместо `authority`.
  - Ответ: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Ложь:
    - GET `/v2/contracts/code/{code_hash}` -> отправка манифеста
    - GET `/v2/contracts/code-bytes/{code_hash}` -> отправка `{ code_b64 }`
- ПОСТ `/v2/contracts/instance`
  - Запрос: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Сопровождение: развертывание байт-кода и немедленное активное сопоставление файлов `(namespace, contract_id)` через `ActivateContractInstance`.
  - Ответ: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Сервисный псевдоним
- ПОСТ `/v2/aliases/voprf/evaluate`
  - Запрос: { "blinded_element_hex": "..." }
  - Ответ: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценки. Актуальная стоимость: `blake2b512-mock`.
  - Примечания: оценщик макетно определил, что приложение Blake2b512 с разделением домена `iroha.alias.voprf.mock.v1`. Предыдущий этап тестирования был основан на том, что производственный конвейер VOPRF опирается на Iroha.
  - Ошибки: HTTP `400` при входном шестнадцатеричном формате. Torii отправил конверт Norito `ValidationFail::QueryFailed::Conversion` с сообщением об ошибке декодера.
- ПОСТ `/v2/aliases/resolve`
  - Запрос: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Ответ: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Примечания: требуется создание моста ISO во время выполнения (`[iso_bridge.account_aliases]` и `iroha_config`). Torii нормализует псевдонимы в удаленных пробелах и в больших количествах перед поиском. Верните 404, если псевдоним отсутствует, а 503, если мост ISO во время выполнения неактивен.
- ПОСТ `/v2/aliases/resolve_index`
  - Запрос: { "индекс": 0 }
  - Ответ: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Примечания: индекс псевдонима назначает определенный порядок конфигурации (отсчет от 0). Клиенты могут иметь доступ к кэшу вне очереди, чтобы построить трассы для аудита для вечеринок аттестации псевдонимов.

Кап-де-тайль де код
- Пользовательский параметр: `max_contract_code_bytes` (JSON u64).
  - Максимальный авторизованный контроль (в байтах) для хранения кода контракта в цепочке.
  - По умолчанию: 16 МБ. Les noeuds rejettent `RegisterSmartContractBytes` lorsque la Taille de l'image `.to` depasse le cap avec une erreur d'invariant.
  - Операторы могут настроить через `SetParameter(Custom)` с `id = "max_contract_code_bytes"` и цифровую полезную нагрузку.- ПОСТ `/v2/gov/ballots/zk`
  - Запрос: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Примечания:
    - Когда общедоступные входы цепи включают `owner`, `amount` и `duration_blocks`, а также предварительная проверка конфигурации VK, новый критерий или поддержка управления для `election_id` с `election_id` `owner`. Направление оставшегося кэша (`unknown`); Seuls сумма/срок действия не пропал в течение дня. Повторное голосование звучит монотонно: количество и срок действия не шрифт qu'augmenter (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Повторное голосование ZK с указанием суммы возврата или истечения срока действия, которое отклоняется на сервере с диагностикой `BallotRejected`.
    - L'execution du contrat doit appeler `ZK_VOTE_VERIFY_BALLOT` avant d'enfiler `SubmitBallot`; Хосты наложили защелку на отдельную кнопку.

- ПОСТ `/v2/gov/ballots/plain`
  - Запрос: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Да|Нет|Воздержаться" }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Примечания: повторное голосование происходит в отдельном продлении - новый бюллетень не может быть уменьшен, если сумма или истечение срока действия уже существует. Le `owner` должен соответствовать полномочиям транзакции. Минимальная длительность составляет `conviction_step_blocks`.

- ПОСТ `/v2/gov/finalize`
  - Запрос: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Эффект в цепочке (действующий эшафот): введите утвержденное предложение о развертывании, вставьте минимальный `ContractManifest` в минимальный код `code_hash` с l'`abi_hash`, а затем отметьте предложение принято. Если манифест существует для `code_hash` с другим `abi_hash`, этот закон отвергается.
  - Примечания:
    - Для выборов ZK, les chemins de contrat doivent appeler `ZK_VOTE_VERIFY_TALLY` avant d'executer `FinalizeElection`; хосты навязывают уникальное использование. `FinalizeReferendum` отмените референдумы ZK, чтобы подсчет не был завершен.
    - Автоматическое закрытие `h_end` с уникальным одобрением/отклонением для референдумов Plain; Референдумы ZK остаются закрытыми, просто потому что они подсчитывают завершение soit soumis et que `FinalizeReferendum` так, чтобы оно было выполнено.
    - Проверки явки с использованием последовательного утверждения+отклонения; воздержитесь от голосования за явку.- ПОСТ `/v2/gov/enact`
  - Запрос: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Примечания: Torii означает подписание транзакции, когда `authority`/`private_key` sont fournis; просто отослать сквелет для подписи и запроса клиента. Прообраз — это опция и информативность для мгновенного воспроизведения.

- ПОЛУЧИТЬ `/v2/gov/proposals/{id}`
  - Путь `{id}`: шестнадцатеричный идентификатор предложения (64 символа).
  - Ответ: { "найдено": bool, "предложение": { ... }? }

- ПОЛУЧИТЬ `/v2/gov/locks/{rid}`
  - Путь `{rid}`: идентификатор строки референдума.
  - Ответ: { "найдено": bool, "referendum_id": "rid", "locks": { ... }? }

- ПОЛУЧИТЬ `/v2/gov/council/current`
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." }, ...] }
  - Примечания: отсылка к совету сохраняется и присутствует; Синон получает резервный детерминированный актив с конфигурацией актива и ле seuils (мир спецификации VRF просто так, как будто VRF полностью сохраняется в цепочке).

- POST `/v2/gov/council/derive-vrf` (функция: gov_vrf)
  - Запрос: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Поддержка: проверьте предварительный VRF-адрес кандидата против канонического ввода, полученного из `chain_id`, `epoch` и маяка последнего хэш-блока; три числа по количеству байтов по выборке с тай-брейками; отправьте топ-членам `committee_size`. Не упорствуй.
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Примечания: Нормальный = pk en G1, доказательство en G2 (96 байт). Small = pk en G2, доказательство en G1 (48 байт). Входные данные разделены по домену и включают `chain_id`.

### Управление по умолчанию (iroha_config `gov.*`)

Резервный вариант совета использует параметр Torii, когда список не существует, параметр через `iroha_config`:

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

Переопределяет эквиваленты среды:

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

`parliament_committee_size` ограничивает число резервных отправителей, когда совет не сохраняется, `parliament_term_blocks` определяет длительность использования для получения семени (`epoch = floor(height / term_blocks)`), `parliament_min_stake` устанавливает минимальную ставку (en unites) минимальные) по критериям соответствия активам и `parliament_eligibility_asset_id`, выбранным для того, чтобы продать актив - это сканирование для строительства набора кандидатов.

Проверка управления VK без обхода: проверка избирательного бюллетеня требует использования cle `Active` с встроенными байтами, и окружение не требует применения переключателей теста для проверки.

РБАК
- Для выполнения цепочки требуются разрешения:
  - Предложения: `CanProposeContractDeployment{ contract_id }`
  - Бюллетени: `CanSubmitGovernanceBallot{ referendum_id }`
  - Принятие: `CanEnactGovernance`
  - Управление советом (в будущем): `CanManageParliament`Пространства имен протеже
- Пользовательский параметр `gov_protected_namespaces` (массив строк JSON), активный для допуска к развертыванию файлов в списках пространств имен.
- Клиенты не включают метаданные транзакций для развертываний и протеже пространств имен:
  - `gov_namespace`: пространство имен cible (например, «приложения»)
  - `gov_contract_id`: логический договор в пространстве имен.
- `gov_manifest_approvers`: опция массива JSON для идентификаторов учетных записей валидаторов. Когда манифест полосы объявит кворум > 1, требуется допуск к полномочиям транзакции плюс списки счетов для удовлетворения кворума манифеста.
- Телеметрия выявляет комиссий по допуску через `governance_manifest_admission_total{result}`, чтобы отдельные операторы допускали повторное использование химиков `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` и т. д. `runtime_hook_rejected`.
- Телеметрия выявляет процесс правоприменения через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`) для проверки одобрения людей.
- Переулки применяются к списку опубликованных пространств имен в манифестах. Вся транзакция, которая исправляет `gov_namespace`, делает `gov_contract_id`, а пространство имен создается устройством в наборе `protected_namespaces` в манифесте. Les soumissions `RegisterSmartContractCode` без метаданных, которые были отклонены или активна защита.
- L'admission навязывает qu'une proposition de gouvernance, принятый для кортежа `(namespace, contract_id, code_hash, abi_hash)`; При проверке появляется сообщение об ошибке NotPermited.

Перехватчики обновления среды выполнения
- Манифесты объявления полосы движения `hooks.runtime_upgrade` для получения инструкций по обновлению среды выполнения (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Чемпионские поля:
  - `allow` (bool, по умолчанию `true`): quand `false`, содержит инструкции по обновлению среды выполнения, которые нельзя отклонить.
  - `require_metadata` (bool, по умолчанию `false`): позволяет указать входные метаданные для параметра `metadata_key`.
  - `metadata_key` (строка): имя приложения метаданных по ловушке. По умолчанию `gov_upgrade_id`, когда требуются метаданные или какой список разрешений присутствует.
  - `allowed_ids` (массив строк): параметры разрешенного списка метаданных значений (после обрезки). Rejette quand la valeur fournie n'est pas listee.
- Когда присутствует крючок, доступ к файлу применяется к политическим метаданным перед входом в транзакцию в файле. Метаданные недействительны, значения, указанные в виде или за пределами разрешенного списка, вызывают ошибку NotPermitted.
- Телеметрическое отслеживание результатов через `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, удовлетворяющие крючку, включают метаданные `gov_upgrade_id=<value>` (или определение в манифесте) и апробации валидаторов, необходимые для кворума манифеста.Конечная точка товара
- POST `/v2/gov/protected-namespaces` - аппликация `gov_protected_namespaces` в направлении sur le noeud.
  - Запрос: { "пространства имен": ["приложения", "система"] }
  - Ответ: { "ОК": правда, "применено": 1 }
  - Примечания: назначение администратора/тестирования; Требуйте API токена для настройки. Для производства предпочитайте транзакцию, подписанную `SetParameter(Custom)`.

Помощники CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Восстановите экземпляры контракта для пространства имен и проверьте, что:
    - Torii содержит байт-код для части `code_hash`, и его дайджест Blake2b-32 соответствует `code_hash`.
    - Le Manifeste Stocke Sous `/v2/contracts/code/{code_hash}` Доклад о ценностях `code_hash` и `abi_hash` корреспондентов.
    - Введено в действие предложение по управлению, существующее для `(namespace, contract_id, code_hash, abi_hash)`, производное от хэширования мема с идентификатором предложения, которое можно использовать.
  - Сортировка JSON с `results[]` по контракту (проблемы, резюме манифеста/кода/предложения), а также резюме и подавление линии (`--no-summary`).
  - Утилита для аудита защищенных пространств имен или проверки рабочих процессов развертывания элементов управления.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Использование метаданных JSON в формате JSON для развертываний в пространствах имен, включая опции `gov_manifest_approvers` для удовлетворения правил кворума манифеста.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — подсказки по блокировке необходимы для `min_bond_amount > 0`, и весь ансамбль подсказок включает в себя `owner`, `amount` и `duration_blocks`.
  — Проверяет канонические идентификаторы учетных записей, канонизирует 32-байтовые подсказки обнулителя и объединяет подсказки в `public_inputs_json` (с `--public <path>` для дополнительных переопределений).
  - Обнулитель получается из обязательства доказательства (общедоступный вклад) плюс `domain_tag`, `chain_id` и `election_id`; `--nullifier` проверяется на соответствие при поставке.
  - Возобновление на прямой линии выставляется на обслуживание `fingerprint=<hex>`, что определяет получение `CastZkBallot`, кодируя те же подсказки, которые декодируются (`owner`, `amount`, `duration_blocks`, `direction` si Фурнис).
  - Ответы CLI с аннотацией `tx_instructions[]` с `payload_fingerprint_hex` плюс декодирование полей для того, чтобы последующие действия проверяли squelette без повторной реализации декодирования Norito.
  - Доступ к подсказкам блокировки для новых событий `LockCreated`/`LockExtended` для голосования ZK - это то, что схема выставляет ценные мемы.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Псевдоним `--lock-amount`/`--lock-duration-blocks` отражает имена флагов ZK для части сценариев.
  - Возобновление вылазки отображается в формате `vote --mode zk`, включая отпечатки пальцев, кодированные инструкции и списки полей для голосования (`owner`, `amount`, `duration_blocks`, `direction`), для быстрого подтверждения. авангардная подпись дю сквелетт.Листинг экземпляров
- GET `/v2/gov/instances/{ns}` - список экземпляров активных контрактов для пространства имен.
  - Параметры запроса:
    - `contains`: фильтр по цепочке `contract_id` (с учетом регистра)
    - `hash_prefix`: фильтр по шестнадцатеричному префиксу `code_hash_hex` (строчные буквы)
    - `offset` (по умолчанию 0), `limit` (по умолчанию 100, максимум 10_000)
    - `order`: от `cid_asc` (по умолчанию), `cid_desc`, `hash_asc`, `hash_desc`
  - Ответ: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Вспомогательный SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) или `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Balayage d'unlocks (Оператор/Аудит)
- ПОЛУЧИТЬ `/v2/gov/unlocks/stats`
  - Ответ: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Примечания: `last_sweep_height` отражает высокий уровень блока плюс недавние блокировки или срок действия блокировок истекает, когда вы выходите из строя и сохраняется. `expired_locks_now` рассчитан на сканирование регистраций блокировки с `expiry_height <= height_current`.
- ПОСТ `/v2/gov/ballots/zk-v1`
  - Запрос (стиль DTO v1):
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

- ПОСТ `/v2/gov/ballots/zk-v1/ballot-proof` (характеристика: `zk-ballot`)
  - Примите JSON `BallotProof` напрямую и отправьте фрагмент `CastZkBallot`.
  - Запрос:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "голосование": {
        "бэкэнд": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 содержимого ZK1 или H2*
        "root_hint": null, // необязательная 32-байтовая шестнадцатеричная строка (корень соответствия)
        «владелец»: null, // Опция AccountId, если схема фиксации владельца
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
    - Карта сервера `root_hint`/`owner`/`nullifier` с опциями голосования для версии `public_inputs_json` для `CastZkBallot`.
    - Байты конверта перекодируются в base64 для полезной нагрузки инструкции.
    - Ответ `reason` передается `submitted transaction` и Torii проходит голосование.
    - Эта конечная точка является уникальной, если функция `zk-ballot` активна.Парки проверки CastZkBallot
- `CastZkBallot` декодирует предварительную базу данных base64 и удаляет полезные данные в видах или неправильных формах (`BallotRejected` с `invalid or empty proof`).
- Хост-результат подтверждения голосования по результатам референдума (`vk_ballot`) или параметры управления по умолчанию и параметры регистрации существуют, поэтому `Active` и передача байтов в реальном времени.
- Байты кода проверки акций перехэшируются с `hash_vk`; Заявлено несоответствие обязательств, приостановленных перед проверкой для защиты входных данных от ошибок реестра (`BallotRejected` с `verifying key commitment mismatch`).
- Предварительные байты отправляются на серверную часть при регистрации через `zk::verify_backend`; Транскрипции недействительны, заменены на `BallotRejected` с `invalid proof` и соответствуют инструкциям по детерминированию.
- Доказательства должны раскрывать обязательства по голосованию и основу права на участие в качестве общественного мнения; корень должен соответствовать `eligible_root` выборов, а производный обнулитель должен соответствовать любой предоставленной подсказке.
- Les preuves reussies emettent `BallotAccepted`; нуллификаторы дубликатов, корни сроков приемлемости или регрессия блокировки продолжения производства по причинам отказа от существующих декритов плюс верхний предел этого документа.

## Mauvaise проводник валидаторов и объединение консенсуса

### Рабочий процесс рубки и заключения в тюрьму

Консенсусный код `Evidence` закодирован в Norito для проверки нарушения протокола. Часть полезной нагрузки поступает в `EvidenceStore` в памяти и, если ее редактировать, материализуется в карте `consensus_evidence`, добавленной в WSV. Регистрация плюс старые `sumeragi.npos.reconfig.evidence_horizon_blocks` (блоки `7200` по умолчанию) не отменяются для сохранения архива, но больше всего это откат для операторов. Свидетельства в пределах горизонта также учитывают `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и косую задержку `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`); руководство может отменить штрафы с помощью `CancelConsensusEvidencePenalty` до того, как будет применено сокращение.

Преступления возобновляются, и они снова отображаются в одном месте на `EvidenceKind`; Дискриминанты являются стабильными и налагают стандартную модель данных:

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

- **DoublePrepare/DoubleCommit** — проверка знака хэшей и конфликта для кортежа мемов `(phase,height,view,epoch)`.
- **InvalidQc** — агрегатор сплетен и сертификат фиксации не формируются в виде эхо-сигнала при дополнительных проверках (например, растровое изображение подписей видео).
- **InvalidProposal** — лидер предлагает блок, который отражает структуру проверки (например, viole la regle de locked-chain).
- **Цензура** — подписанные квитанции об отправке показывают транзакцию, которая никогда не предлагалась/не совершалась.

Операторы и дополнительные инспекторы и ретранслируют полезные нагрузки через:

- Torii: `GET /v2/sumeragi/evidence` и `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count` и `... submit --evidence-hex <payload>`.

La gouvernance doit предатель les bytes d'evidence comme preuve canonique:1. **Соберите полезную нагрузку** до истечения срока действия. Архиватор байтов Norito обрабатывает высоту/вид метаданных.
2. **Подготовка штрафа** и установка полезной нагрузки на референдум или инструкцию sudo (например, `Unregister::peer`). Выполнение повторно подтверждает полезную нагрузку; Доказательства в неправильной форме или устаревшие — это отвергнутая детерминированность.
3. **Планировщик топологии суиви** для того, чтобы немедленная проверка была невозможна. Типичные потоки вошли в список `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)` в списке, который был написан в течение дня.
4. **Аудит результатов** через `/v2/sumeragi/evidence` и `/v2/sumeragi/status` для подтверждения того, что сбор доказательств осуществляется заранее и что управление осуществляется с помощью повторного применения.

### Последовательность консенсусного соединения

Совместная консенсусная гарантия того, что ансамбль валидаторов определит завершение пограничного блока до того, как новый ансамбль начнет предлагать. Среда выполнения устанавливает правила через видимые параметры:

- `SumeragiParameter::NextMode` и `SumeragiParameter::ModeActivationHeight` doivent etre фиксируются в **meme bloc**. `mode_activation_height` doit etre strictement superieur a la hauteur du bloc qui a porte la mise a jour, donnant au moins un bloc de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) представляет собой предварительную конфигурацию, которая обеспечивает передачу управления с нулевой задержкой:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`) задерживает сокращение консенсуса, чтобы руководство могло отменить штрафы до их применения.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Среда выполнения и CLI раскрывают параметры, созданные через `/v2/sumeragi/params` и `iroha --output-format text ops sumeragi params`, в зависимости от того, что операторы подтверждают высокую активацию и списки валидаторов.
- Автоматизация управления doit toujours:
  1. Окончательное решение о возвращении (или реинтеграции) в поддержку по доказательствам.
  2. Выполните реконфигурацию своего устройства с `mode_activation_height = h_current + activation_lag_blocks`.
  3. Surveiller `/v2/sumeragi/status` просто так, как `effective_consensus_mode`, поднимается в а-ля высокомерный посетитель.

Весь сценарий, который выполняет проверку валидаторов или аппликацию, **не делает шаг**, чтобы активировать нулевую задержку или измерить параметры передачи; эти транзакции были отклонены и не разрешены в рамках прецедента.

## Поверхности телеметрии- Метрики Prometheus экспортируют деятельность по управлению:
  - `governance_proposals_status{status}` (датчик) соответствует номинальному статусу конкурентов.
  - `governance_protected_namespace_total{outcome}` (счетчик) увеличивает количество допусков к пространствам имен, принятых или отменяющих развертывание.
  - `governance_manifest_activations_total{event}` (счетчик) регистрирует вставки манифеста (`event="manifest_inserted"`) и привязки пространства имен (`event="instance_bound"`).
- `/status` включает объект `governance`, который отображает списки предложений, сообщает все защищённые пространства имен и список последних активаций манифеста (пространство имен, идентификатор контракта, хэш кода/ABI, высоту блока, временную метку активации). Операторы могут быть призваны подтвердить, что постановления, принятые в течение дня, манифесты и те ворота в пространства имен, которые навязывают протеже.
- Шаблон Grafana (`docs/source/grafana_governance_constraints.json`) и книга телеметрии в `telemetry.md` с ежедневными комментариями, кабельными оповещениями для блокировок предложений, активаций манифестных заявок или отклонениями от участия в пространствах имен защитников во время выполнения обновлений.