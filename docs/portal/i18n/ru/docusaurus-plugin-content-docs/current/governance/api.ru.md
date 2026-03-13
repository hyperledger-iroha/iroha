---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Статус: черновик/набросок для сопровождения задач по реализации управления. Формы могут измениться в ходе разработки. Детерминизм и политика RBAC являются нормативными ограничениями; Torii может подписывать/отправлять платежи при наличии `authority` и `private_key`, иначе клиенты собирают и отправляют в `/transaction`.

Обзор
- Все конечные точки возвращают JSON. Для потоков, которые определяют отслеживание, ответы включают `tx_instructions` — массив одной или нескольких инструкций-скелетов:
  - `wire_id`: регистрационный идентификатор типа инструкции.
  - `payload_hex`: байты полезной нагрузки Norito (шестнадцатеричный)
- Если `authority` и `private_key` предоставлены (или `private_key` в бюллетенях DTO), Torii подписывает и отправляет транзакцию и все равно возвращает `tx_instructions`.
- В противном случае клиенты собирают SignedTransaction со своим авторитетом и Chain_id, затем подписывают и POST в `/transaction`.
- Открытие SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` получает `GovernanceProposalResult` (нормализует поля status/вид), `ToriiClient.get_governance_referendum_typed` возвращает `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` возвращает `GovernanceTally`, `ToriiClient.get_governance_locks_typed` возвращает `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` возвращает `GovernanceUnlockStats`, а `ToriiClient.list_governance_instances_typed` возвращает `GovernanceInstancesPage`, вызывая типизированный доступ по управлению всей поверхностью с примерами использования в README.
- Легкий клиент Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` и `ToriiClient.enact_proposal` возвращают типизированные пакеты `GovernanceInstructionDraft` (оборачивают Torii скелет `tx_instructions`), исбегая ручного JSON-парсинга при сборке Завершить/ввести в действие потоки.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` предоставляет типизированные помощники для предложений, референдумов, подсчетов, блокировок, статистики разблокировки, а теперь `listGovernanceInstances(namespace, options)` плюс конечные точки совета (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), чтобы клиенты Node.js могли разбивать страницы `/v2/gov/instances/{ns}` и запускать рабочие процессы, поддерживаемые VRF, с существующим списком контрактных инстансов.

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
  - Проверка: ноды канонизируют `abi_hash` для заданного `abi_version` и отвергают несовпадения. Для `abi_version = "v1"` ожидаемое значение — `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API контрактов (развертывание)
- ПОСТ `/v2/contracts/deploy`
  - Запрос: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Поведение: высчитывает `code_hash` по телу IVM программы и `abi_hash` по заголовку `abi_version`, затем отправляет `RegisterSmartContractCode` (манифест) и `RegisterSmartContractBytes` (полные). `.to` байты) от имени `authority`.
  - Ответ: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Связано:
    - GET `/v2/contracts/code/{code_hash}` -> сохраняет сохраненный манифест
    - GET `/v2/contracts/code-bytes/{code_hash}` -> отправить `{ code_b64 }`
- ПОСТ `/v2/contracts/instance`
  - Запрос: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Поведение: деплоитный предоставленный байт-код и сразу активирует отображение `(namespace, contract_id)` через `ActivateContractInstance`.
  - Ответ: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Служба псевдонимов
- ПОСТ `/v2/aliases/voprf/evaluate`
  - Запрос: { "blinded_element_hex": "..." }
  - Ответ: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отсылка к оценщику. Текущее значение: `blake2b512-mock`.
  - Примечания: определённый макет оценщик, применяющий Blake2b512 с разделением домена `iroha.alias.voprf.mock.v1`. рекомендованы для тестируемой оснастки, пока производственный трубопровод ВОПРФ не подключен к Iroha.
  - Ошибки: HTTP `400` при некорректном шестигранном вводе. Torii отправляет Norito конверт `ValidationFail::QueryFailed::Conversion` с сообщением декодера.
- ПОСТ `/v2/aliases/resolve`
  - Запрос: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Ответ: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Примечания: требуется промежуточный этап выполнения моста ISO (`[iso_bridge.account_aliases]` в `iroha_config`). Torii нормализует псевдоним, удаляет пробелы и ведет в верхнюю часть регистра. Возвращает 404 при отсутствии псевдонима и 503, когда среда выполнения моста ISO отключена.
- ПОСТ `/v2/aliases/resolve_index`
  - Запрос: { "индекс": 0 }
  - Ответ: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Примечания: псевдонимы индексов назначаются детерминированно по порядку конфигурации (отсчет от 0). Клиенты могут кэшировать ответы в автономном режиме для построения контрольного журнала по псевдонимам аттестации событий.

Код Размер ограничения
- Пользовательский параметр: `max_contract_code_bytes` (JSON u64).
  - Управляет максимальным допустимым размером (в байтах) в цепочке хранения кодов контрактов.
  - По умолчанию: 16 МБ. Ноды отклоняют `RegisterSmartContractBytes`, когда размер `.to` изображения увеличивает лимит, с ошибкой нарушается инвариант.
  - Операторы могут изменять через `SetParameter(Custom)` с `id = "max_contract_code_bytes"` и числовым полезным грузом.- ПОСТ `/v2/gov/ballots/zk`
  - Запрос: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Ответ: { "ОК": правда, "принято": правда, "tx_instructions": [{...}] }
  - Примечания:
    - Когда публичные входные схемы включают `owner`, `amount` и `duration_blocks`, и доказательство верифицируется с настроенным ВК, нода создает или продлевает блокировку управления для `election_id` с этим `owner`. Направление скрыто (`unknown`); обновляются только сумма/срок действия. Повторные голоса монотонны: сумма и срок действия только меняются (нода предъявления max(amount, prev.amount) и max(expiry, prev.expiry)).
    - ZK переголосует, пытающиеся уменьшить сумму или срок действия, отклоняются сервером с диагностикой `BallotRejected`.
    - Исполнение контракта должно вызвать `ZK_VOTE_VERIFY_BALLOT` до размещения `SubmitBallot`; хосты применяют одноразовую защелку.

- ПОСТ `/v2/gov/ballots/plain`
  - Запрос: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "владелец": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Да|Нет|Воздержаться" }
  - Ответ: { "ОК": правда, "принято": правда, "tx_instructions": [{...}] }
  - Примечания: повторное голосование только при продлении - новый бюллетень не может уменьшить сумму или истечь срок действия существующей блокировки. `owner` должен совпадать с полномочиями по транзакциям. Минимальная длительность — `conviction_step_blocks`.

- ПОСТ `/v2/gov/finalize`
  - Запрос: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Эффект он-чейн (текущий эшафот): принять утвержденное предложение по развертыванию в результате действия `ContractManifest`, приговоренный к `code_hash`, с ожидаемым `abi_hash` и помечает предложение как Enacted. Если манифест уже существует для `code_hash` с другим `abi_hash`, действие отклоняется.
  - Примечания:
    - Для выборов ЗК контракты должны вызывать `ZK_VOTE_VERIFY_TALLY` до `FinalizeElection`; хосты применяют одноразовую защелку. `FinalizeReferendum` отклоняет ZK-референдумы, пока подсчет выборов не финализирован.
    - Автозакрытие на `h_end` эмитит Одобрено/Отклонено только для простых референдумов; ZK-референдумы остаются закрытыми, пока не будет отправлен финальный подсчет и выполнен `FinalizeReferendum`.
    - Проверки явки требуют только одобрения+отклонения; воздержаться не наблюдается за явкой.

- ПОСТ `/v2/gov/enact`
  - Запрос: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Примечания: Torii отправляет подписанную транзакцию в наличии `authority`/`private_key`; В противном случае скелет будет возвращен для использования и отправлен клиенту. Прообраз опционален и сейчас носит информационный характер.- ПОЛУЧИТЬ `/v2/gov/proposals/{id}`
  - Путь `{id}`: шестнадцатеричный идентификатор предложения (64 символа).
  - Ответ: { "найдено": bool, "предложение": { ... }? }

- ПОЛУЧИТЬ `/v2/gov/locks/{rid}`
  - Путь `{rid}`: строка идентификатора референдума.
  - Ответ: { "найдено": bool, "referendum_id": "rid", "locks": { ... }? }

- ПОЛУЧИТЬ `/v2/gov/council/current`
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." }, ...] }
  - Примечания: возвращает сохранившийся совет при наличии; В противном случае извлекается определенный резервный вариант, используя настроенный актив ставки и пороговые значения (зеркалит спецификацию VRF до тех пор, пока доказательства VRF не будут сохранены в цепочке).

- POST `/v2/gov/council/derive-vrf` (функция: gov_vrf)
  - Запрос: { "committee_size": 21, "эпоха": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Поведение: после этого VRF-проверка каждого кандидата на каноническом вводе, полученном из `chain_id`, `epoch` и маяка последнего хеша блока; сортирует по описанию выходных байтов с разрешениями конфликтов; вернуть топ участников `committee_size`. Нет.
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Примечания: Нормальный = pk в G1, доказательство в G2 (96 байт). Small = pk в G2, доказательство в G1 (48 байт). Входы доменно разделены и включают `chain_id`.

### Настройки управления по умолчанию (iroha_config `gov.*`)

Запасной совет, прогноз Torii при отсутствии постоянного списка, параметризуется через `iroha_config`:

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

Эквивалентные переменные окружения:

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

`parliament_committee_size` ограничивает число запасных членов при отсутствии совета, `parliament_term_blocks` задает длину эпохи для деривационного начального числа (`epoch = floor(height / term_blocks)`), `parliament_min_stake` требует формирования доли (в минимальных количествах) на квалификационный актив, а `parliament_eligibility_asset_id` выбирает, какой балансовый актив сканируется при построении набора кандидат.

Управление верификацией ВК без обхода: бюллетени для проверки всегда требуют `Active`, проверяющий ключ со встроенными байтами, и окружение не должно зависеть от тестовых переключателей, чтобы пропустить проверку.

РБАК
- Для выполнения внутрисетевых операций требуются разрешения:
  - Предложения: `CanProposeContractDeployment{ contract_id }`
  - Бюллетени: `CanSubmitGovernanceBallot{ referendum_id }`
  - Принятие: `CanEnactGovernance`
  - Управление советом (в будущем): `CanManageParliament`Защищенные пространства имен
- Пользовательский параметр `gov_protected_namespaces` (массив строк JSON) включает входные ворота для развертывания в перечисленных пространствах имен.
- Клиенты должны включать транзакцию ключей метаданных для развертываний, направленных в защищенные пространства имен:
  - `gov_namespace`: включить пространство имен (например, «приложения»).
  - `gov_contract_id`: логический идентификатор контракта внутри пространства имен.
- `gov_manifest_approvers`: опциональный массив идентификаторов учетных записей JSON валидаторов. Когда манифест полосы задает кворум > 1, требуется передача полномочий допуска плюс перечисленные аккаунты для повторного манифеста кворума.
- Телеметрия обеспечивает счетчики приема через `governance_manifest_admission_total{result}`, чтобы операторы имели успешные разрешения от `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` и `runtime_hook_rejected`.
- Телеметрия показывает путь принудительного исполнения через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`), вызывая операторам аудировать недостающие разрешения.
- Дорожки обеспечивают соблюдение разрешенного списка пространств имен, опубликованного в манифестах. Любая транзакция, устанавливающая `gov_namespace`, должна пройти `gov_contract_id`, пространство имен должно присутствовать в наборе манифеста `protected_namespaces`. `RegisterSmartContractCode` заявки без этих метаданных отклоняются, если защита включена.
- Для допуска требуется, чтобы существовало принятое предложение по управлению для кортежа `(namespace, contract_id, code_hash, abi_hash)`; в противном случае валидация завершается ошибкой NotPermited.

Приемы обновления среды выполнения
- Манифесты дорожек могут объявлять `hooks.runtime_upgrade` для инструкций по обновлению среды выполнения шлюза (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Поля крючка:
  - `allow` (bool, по умолчанию `true`): когда `false`, все инструкции по обновлению во время выполнения отклоняются.
  - `require_metadata` (bool, по умолчанию `false`): требует ввода метаданных, указанную `metadata_key`.
  - `metadata_key` (строка): метаданные имени, принудительный перехват. По умолчанию `gov_upgrade_id`, когда требуются метаданные или есть белый список.
  - `allowed_ids` (массив строк): дополнительные метаданные результатов белого списка (после обрезки). Отклоняет, когда указанное значение не входит в список.
- Когда крючок присутствует, очередь приема применяет политику метаданных, чтобы поочередно обрабатывать транзакции. Отсутствующие метаданные, пустые значения или значения вне белого списка дают определенный NotPermited.
- Телеметрия отслеживает результаты через `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, эффективные хуки, должны включать метаданные `gov_upgrade_id=<value>` (или ключ, текущий манифест) вместе с любыми утверждениями валидатора, требуемыми кворумом манифеста.

Удобство конечной точки
- POST `/v2/gov/protected-namespaces` - Вносится `gov_protected_namespaces` непосредственно на узел.
  - Запрос: { "пространства имен": ["приложения", "система"] }
  - Ответ: { «ОК»: правда, «применено»: 1 }
  - Примечания: предназначены для администрирования/тестирования; требует токен API при конфигурации. В производстве предпочтительная подписанная транзакция с `SetParameter(Custom)`.Помощники CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Получаются контрактные инстансы для пространства имен и, наконец, что:
    - Torii хранит байт-код для каждого `code_hash`, а его дайджест Blake2b-32 соответствует `code_hash`.
    - Манифест под `/v2/contracts/code/{code_hash}` сообщает совпадающие `code_hash` и `abi_hash`.
    - Существует принятое предложение по управлению для `(namespace, contract_id, code_hash, abi_hash)`, производное от того же хеширования идентификатора предложения, которое использует нода.
  - Выводит отчет JSON с `results[]` по каждому контракту (проблемы, сводки манифеста/кода/предложения) и однострочное резюме, если не подавлено (`--no-summary`).
  - Полезно для аудита защищенных пространств имен или проверки рабочих процессов развертывания, управляемых управлением.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  — Выдает метаданные скелета JSON для развертывания в защищенных пространствах имен, включая опциональные `gov_manifest_approvers` для соблюдения манифеста правил кворума.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — подсказки блокировки обязательны при `min_bond_amount > 0`, и любой набор подсказок должен включать `owner`, `amount` и `duration_blocks`.
  — Проверяет канонические идентификаторы учетных записей, канонизирует 32-байтовые подсказки обнулителя и объединяет подсказки в `public_inputs_json` (с `--public <path>` для дополнительных переопределений).
  - Обнулитель получается из обязательства доказательства (общедоступный вклад) плюс `domain_tag`, `chain_id` и `election_id`; `--nullifier` проверяется на соответствие при поставке.
  - Однострочный обзор теперь содержит определённый `fingerprint=<hex>` из кодированного `CastZkBallot` вместе с декодированными подсказками (`owner`, `amount`, `duration_blocks`, `direction` в наличии).
  - Ответы CLI аннотируют `tx_instructions[]` полем `payload_fingerprint_hex` и декодированные поля, чтобы последующие инструменты могли проверить скелет без повторной реализации Norito декодирования.
  - Предоставление подсказок блокировки. Позволяет узлу эмитировать `LockCreated`/`LockExtended` для бюллетеней ZK, когда схема раскрывает то же значение.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Алиасы `--lock-amount`/`--lock-duration-blocks` отражают имена флагов ZK для проверки четности в скриптах.
  - Сводный вывод отражения `vote --mode zk`, включая отпечаток пальца закодированной инструкции и читаемые поля голосования (`owner`, `amount`, `duration_blocks`, `direction`), что дает быстрое подтверждение перед подписью скелета.

Листинг экземпляров
- GET `/v2/gov/instances/{ns}` - список активных контрактных инстансов для пространства имен.
  - Параметры запроса:
    - `contains`: фильтр по подстроке `contract_id` (с учетом регистра)
    - `hash_prefix`: фильтр по шестнадцатеричному префиксу `code_hash_hex` (строчные)
    - `offset` (по умолчанию 0), `limit` (по умолчанию 100, максимум 10_000)
    - `order`: `cid_asc` (по умолчанию), `cid_desc`, `hash_asc`, `hash_desc`
  - Ответ: { "пространство имен": "ns", "экземпляры": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Помощник SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) или `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Разблокировать проверку (оператор/аудит)
- ПОЛУЧИТЬ `/v2/gov/unlocks/stats`
  - Ответ: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Примечания: `last_sweep_height` указывает высоту последнего блока, когда просроченные блокировки были очищены и сохраняются. `expired_locks_now` рассчитывается путем случайной блокировки записей с `expiry_height <= height_current`.
- ПОСТ `/v2/gov/ballots/zk-v1`
  - Запрос (DTO в стиле v1):
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
  - Ответ: { "ОК": правда, "принято": правда, "tx_instructions": [{...}] }

- ПОСТ `/v2/gov/ballots/zk-v1/ballot-proof` (характеристика: `zk-ballot`)
  - Принимает `BallotProof` JSON напрямую и возвращает скелет `CastZkBallot`.
  - Запрос:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "голосование": {
        "бэкэнд": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // контейнер base64 ZK1 или H2*
        "root_hint": null, // необязательная 32-байтовая шестнадцатеричная строка (корень соответствия)
        "owner": null, // необязательный AccountId, когда схема фиксирует владельца
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
    - Сервер маппит дополнительно `root_hint`/`owner`/`nullifier` из бюллетеня в `public_inputs_json` для `CastZkBallot`.
    - Байты конверта повторно кодируются в base64 для инструкции по полезной нагрузке.
    - `reason` меняется на `submitted transaction`, когда Torii отправляет бюллетень.
    - Эта конечная точка доступна только при включенном компоненте `zk-ballot`.Путь проверки CastZkBallot
- `CastZkBallot` декодирует предоставленное base64 доказательство и отклоняет пустые/битые полезные данные (`BallotRejected` с `invalid or empty proof`).
- Хост разрешает ключ проверки бюллетеня на основе референдума (`vk_ballot`) или настроек управления по умолчанию и требует, чтобы запись существовала, была `Active` и приводила встроенные байты.
- Сохраненные байты проверочного ключа повторно хешируются с `hash_vk`; любое несовпадение обязательства приостанавливает выполнение проверок для защиты от подмененных записей реестра (`BallotRejected` с `verifying key commitment mismatch`).
- Байты доказательства отправляются в зарегистрированный сервер через `zk::verify_backend`; недействительные расшифровки приводят к `BallotRejected` с `invalid proof` и согласно инструкции падают.
- Доказательства должны раскрывать обязательства по голосованию и основу права на участие в качестве общественного мнения; корень должен соответствовать `eligible_root` выборов, а производный обнулитель должен соответствовать любой предоставленной подсказке.
- Успешные доказательства эмитируют `BallotAccepted`; повторные обнуляющие, временные ограничения права на участие или регрессия продолжают блокировку, давая дополнительную причину отказа, описанную ранее.

## Ненадлежащее поведение валидаторов и совместный консенсус

## Процесс# порезы и тюремное заключение

Консенсус эмитит Norito в кодировке `Evidence` при нарушениях протокола валидатора. Каждая полезная нагрузка попадает в `EvidenceStore` в памяти и, если он новый, материализуется в `consensus_evidence`, поддерживаемом WSV. Записи старше `sumeragi.npos.reconfig.evidence_horizon_blocks` (блоки `7200` по умолчанию) отклоняются, архивируются до стабильного ограниченного уровня, но отказ регистрируется для операторов. Свидетельства в пределах горизонта также учитывают `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и косую задержку `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`); руководство может отменить штрафы с помощью `CancelConsensusEvidencePenalty` до того, как будет применено сокращение.

Распознанные нарушения по одному-к-одному по `EvidenceKind`; дискриминанты стабильны и показатели зафиксированы в модели данных:

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

- **DoublePrepare/DoubleCommit** - валидатор согласия на конфликтующие кэши для одного `(phase,height,view,epoch)`.
- **InvalidQc** — агрегатор разослал сертификат фиксации, чья форма не проходит определенные проверки (например, пустое растровое изображение подписывающего лица).
- **InvalidProposal** — лидер предложил блок, который проваливает структурную валидацию (например, правило защиты заблокированной цепочки).
- **Цензура** — подписанные квитанции об отправке показывают транзакцию, которая никогда не предлагалась/не совершалась.

Операторы и инструменты могут просматривать и повторно рассылать полезные данные через:

- Torii: `GET /v2/sumeragi/evidence` и `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.

Управление должно рассматривать байты доказательств как каноническое доказательство:1. **Собрать полезную нагрузку** до истечения срока. Архивировать сырые Norito байт вместе с метаданными высоты/представления.
2. **Подготовить штраф** встроив полезную нагрузку в референдуме или инструкции sudo (например, `Unregister::peer`). Исполнение повторно валидирует полезную нагрузку; искаженные или устаревшие доказательства отклоняются детерминированно.
3. **Запланировать последующую топологию**, чтобы нарушивший валидатор не смог сразу вернуться. Типовые потоки обеспечивают `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)` с обновленным списком.
4. **Результаты аудита** через `/v2/sumeragi/evidence` и `/v2/sumeragi/status`, чтобы убедиться, что показания счетчика увеличились и управление завершило удаление.

### Последовательность совместного консенсуса

Совместный консенсус гарантирует, что исходный набор валидаторов завершит пограничный блок до того, как новый набор начнет предлагать. Применение правил во время выполнения через парные параметры:

- `SumeragiParameter::NextMode` и `SumeragiParameter::ModeActivationHeight` должны быть зафиксированы в **том же блоке**. `mode_activation_height` должен быть строго выше высоты блока, который при обновлении приводит к задержке как минимум на один блок.
- `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) - это конфигурационный охранник, который владеет передачей без лагов:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`) задерживает сокращение консенсуса, чтобы руководство могло отменить штрафы до их применения.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Среда выполнения и CLI отображают промежуточные параметры через `/v2/sumeragi/params` и `iroha --output-format text ops sumeragi params`, чтобы операторы определяли высоты активации и список валидаторов.
- Автоматизация управления всегда должна:
  1. Финализировать решение об исключении (или восстановлении), поддержанное доказательство.
  2. Поставить последующую реконфигурацию с `mode_activation_height = h_current + activation_lag_blocks`.
  3. Мониторить `/v2/sumeragi/status` до переключения `effective_consensus_mode` на ожидаемой высоте.

Любой скрипт, который ротирует валидаторов или применяет косую черту, **не должен** пытаться активировать нулевую задержку или опускать параметры переключения; такие транзакции отклоняются и оставляют сеть в предыдущем режиме.

## Поверхности телеметрии

- Prometheus метрики экспортируют управление активностью:
  - `governance_proposals_status{status}` (датчик) отслеживает количество предложений по статусу.
  - `governance_protected_namespace_total{outcome}` (счетчик) увеличивается при разрешении/отклонении доступа для защищенных пространств имен.
  - `governance_manifest_activations_total{event}` (счетчик) фиксирует вставки манифеста (`event="manifest_inserted"`) и привязки пространства имен (`event="instance_bound"`).
- `/status` включает объект `governance`, который учитывает счетчики предложений, сообщает итоги по защищенным пространствам имен и пересчитывает недавние активации манифеста (пространство имен, идентификатор контракта, хеш-код/ABI, высоту блока, временную метку активации). Операторы могут спросить это поле, чтобы убедиться, что законодательные акты обновили манифесты и что шлюзы защищенного пространства имен соблюдаются.
- Шаблон Grafana (`docs/source/grafana_governance_constraints.json`) и книга задач телеметрии в `telemetry.md` отображаются как настройка уведомлений о застрявших предложениях, пропущенных активациях манифеста или неожиданных отклонениях защищенных пространств имен во время обновлений среды выполнения.