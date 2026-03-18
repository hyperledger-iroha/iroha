---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Статус: Раскуньо/эсбоко для сопровождения в качестве тарифов на реализацию управления. В качестве формы возможного применения во время реализации. Детерминизм и политика RBAC ограничивает нормативы; Torii может выполнять транзакции/подсчеты, когда `authority` и `private_key` работают, если клиенты строят и подпункт для `/transaction`.

Визао жераль
- Конечные точки Todos переименовываются в JSON. Для потоков, которые производят транзакции, в качестве ответа включается `tx_instructions` - массив данных или другие инструкции:
  - `wire_id`: идентификатор регистрации для указания типа инструкции.
  - `payload_hex`: байты полезной нагрузки Norito (шестнадцатеричный)
- Если `authority` и `private_key` используются в форме fornecidos (или `private_key` в DTO избирательных бюллетеней), Torii Assina e Submete a Transacao e Ainda Retorna `tx_instructions`.
- В противоположном случае, клиенты смонтировали подписанную транзакцию, используя полномочия и идентификатор цепочки, depois assinam и fazem POST для `/transaction`.
- Кобертура SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` возврат `GovernanceProposalResult` (нормализованный статус/вид кампоса), `ToriiClient.get_governance_referendum_typed` возврат `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` возврат `GovernanceTally`, `ToriiClient.get_governance_locks_typed` реторна `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` реторна `GovernanceUnlockStats`, и `ToriiClient.list_governance_instances_typed` реторна `GovernanceInstancesPage`, я получаю доступ к общему уровню управления с примерами у нас нет README.
- Клиентский уровень Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` и `ToriiClient.enact_proposal` возвращают пакеты типов `GovernanceInstructionDraft` (инкапсуляция или преобразование `tx_instructions` в Torii), удаление синтаксического анализа. руководство по JSON-скриптам, составленным Fluxos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` отображает вспомогательные советы для предложений, референдумов, подсчетов, блокировок, статистики разблокировки и агоры `listGovernanceInstances(namespace, options)` для большинства конечных точек ОС (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), чтобы клиенты Node.js могли перейти на страницу `/v1/gov/instances/{ns}` и управлять рабочими процессами с VRF одновременно с составлением списка существующих экземпляров контрактов.

Конечные точки

- ПОСТ `/v1/gov/proposals/deploy-contract`
  - Реквизиция (JSON):
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
  - Подтверждено: канонизация `abi_hash` для `abi_version` fornecido и rejeitam расхождения. Пункт `abi_version = "v1"`, или мужество за победу и `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API контрактов (развертывание)
- ПОСТ `/v1/contracts/deploy`
  - Requisicao: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Содержимое: расчет `code_hash`, часть корпоративной программы IVM и `abi_hash`, часть заголовка `abi_version`, подмета `RegisterSmartContractCode` (манифест) и `RegisterSmartContractBytes`. (полные байты `.to`) под именем `authority`.
  - Ответ: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Сообщение:
    - GET `/v1/contracts/code/{code_hash}` -> возврат или армазенадо манифеста
    - GET `/v1/contracts/code-bytes/{code_hash}` -> реторна `{ code_b64 }`
- ПОСТ `/v1/contracts/instance`
  - Requisicao: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Комментарий: развертывание байт-кода и немедленная активация отображения `(namespace, contract_id)` через `ActivateContractInstance`.
  - Ответ: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Служба псевдонимов
- ПОСТ `/v1/aliases/voprf/evaluate`
  - Реквизит: { "blinded_element_hex": "..." }
  - Ответ: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отображает инструмент, который можно использовать. Настоящая доблесть: `blake2b512-mock`.
  - Примечание: доступен макет детерминированного приложения Blake2b512 с отдельным доменом `iroha.alias.voprf.mock.v1`. Предназначен для тестирования или конвейера производства VOPRF, интегрированного в Iroha.
  - Ошибки: HTTP `400` неверный шестнадцатеричный ввод данных. Torii возвращает конверт Norito `ValidationFail::QueryFailed::Conversion` с сообщением об ошибке декодера.
- ПОСТ `/v1/aliases/resolve`
  - Реквизиция: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Ответ: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Примечания: запросить промежуточную версию моста ISO во время выполнения (`[iso_bridge.account_aliases]` em `iroha_config`). Torii нормализовать псевдонимы, удалить расширения и преобразовать для более быстрого поиска. Возвращается 404, когда псевдоним устарел, и 503, когда мост времени выполнения ISO устарел.
- ПОСТ `/v1/aliases/resolve_index`
  - Реквизит: { "индекс": 0 }
  - Ответ: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Примечания: индексы псевдонимов атрибутов детерминированной формы порядка конфигурации (на основе 0). Клиенты могут кэшировать ответы в автономном режиме, чтобы создавать три аудитории для событий подтверждения псевдонима.

Ограничение таманьо де кодиго
- Пользовательский параметр: `max_contract_code_bytes` (JSON u64)
  - Управление или максимальное разрешение (в байтах) для активации кода контракта в цепочке.
  - По умолчанию: 16 МБ. Наши пользователи `RegisterSmartContractBytes`, когда изображение `.to` превышает предел, связанный с нарушением инварианта.
  - Операции выполняются через `SetParameter(Custom)` с `id = "max_contract_code_bytes"` и числом полезных данных.- ПОСТ `/v1/gov/ballots/zk`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Примечания:
    - Когда общедоступные входы делают схему, включающую `owner`, `amount` и `duration_blocks`, и проверьте соответствие конфигурации ВК, не кричите ли вы или не блокируете управление для `election_id` как есть `owner`. Постоянное скрытое указание (`unknown`); сумма апенаса/срок действия по умолчанию. Повторное голосование происходит монотонно: количество и срок действия apenas aumentam (o no aplica max(amount, prev.amount) e max(expiry, prev.expiry)).
    - Повторно проголосовать за то, чтобы уменьшить сумму или истечение срока действия, если сервер не был диагностирован `BallotRejected`.
    - Выполнение контрато-де-чамара `ZK_VOTE_VERIFY_BALLOT` до загрузки `SubmitBallot`; хосты импоэм um latch de uma unica vez.

- ПОСТ `/v1/gov/ballots/plain`
  - Реквизит: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Да|Нет|Воздержаться" }
  - Ответ: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Примечания: повторное голосование в расширенном порядке - новое голосование не может уменьшить сумму или истечение срока действия блокировки существования. O `owner` должен стать авторитетным органом по транзакциям. Минимум Дюрасао и `conviction_step_blocks`.

- ПОСТ `/v1/gov/finalize`
  - Requisicao: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito в цепочке (фактически эшафот): объявить предложение о развертывании подтверждения, вставленное в `ContractManifest` minimo с Chave `code_hash` com или `abi_hash`, и отправить предложение как принятое. Если манифест существует для `code_hash` с `abi_hash`, он может быть принят и обновлен.
  - Примечания:
    - Для хороших ZK, os caminhos do contrato devem chamar `ZK_VOTE_VERIFY_TALLY` до выполнения `FinalizeElection`; OS Hosts impoem um latch de uso unico. `FinalizeReferendum` повторил референдум ZK, который подсчитал итоги окончательного решения.
    - Автоматическое добавление в `h_end` сообщения «Одобрено/Отклонено» для референдумов Plain; референдумы ZK permanecem Closed, которые завершили подсчет результатов, были отправлены и `FinalizeReferendum` были выполнены.
    - При проверке явки мы можем утвердить+отклонить; воздержались от протеста против явки.- ПОСТ `/v1/gov/enact`
  - Requisicao: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Примечания: Torii подпадает под транзакцию, когда `authority`/`private_key` sao fornecidos; Caso Contrario retorna um esqueleto para clientes assinarem e submeterem. Прообраз и дополнительная информация.

- ПОЛУЧИТЬ `/v1/gov/proposals/{id}`
  - Путь `{id}`: шестнадцатеричный идентификатор предложения (64 символа).
  - Ответ: { "найдено": bool, "предложение": { ... }? }

- ПОЛУЧИТЬ `/v1/gov/locks/{rid}`
  - Путь `{rid}`: строка идентификатора референдума.
  - Ответ: { "найдено": bool, "referendum_id": "rid", "locks": { ... }? }

- ПОЛУЧИТЬ `/v1/gov/council/current`
  - Ответ: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notas: retorna o Council persistido quando Presente; В противном случае получается детерминированный резервный вариант использования актива или актива ставки и пороговые значения (например, конкретный VRF, который обеспечивает VRF в постоянном производстве в цепочке).

- POST `/v1/gov/council/derive-vrf` (функция: gov_vrf)
  - Requisicao: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Содержимое: проверка VRF каждого кандидата против ввода канонических производных `chain_id`, `epoch` и последнего хэша блока; ordena por bytes desaya desc com тай-брейки; retorna os top `committee_size` membros. Нао упорствует.
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk em G1, доказательство em G2 (96 байт). Small = pk em G2, доказательство em G1 (48 байт). Входы разделены на домены и включают `chain_id`.

### Настройки управления по умолчанию (iroha_config `gov.*`)

Совет по использованию резервного варианта Torii, когда существующий список сохраняется и параметризуется через `iroha_config`:

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

Переопределения эквивалентов окружающей среды:

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

`parliament_committee_size` ограничивает количество резервных элементов, которые возвращаются, когда совет сохраняется, `parliament_term_blocks` определяет комплимент эпохи, используемый для получения семени (`epoch = floor(height / term_blocks)`), `parliament_min_stake` применение или минимальная ставка (em unidades minimas) нет Элегибилиданый актив, и `parliament_eligibility_asset_id` выберите, какой актив и будет найден, чтобы построить или объединить кандидатов.

Проверка управления ВК в обход этого пункта: проверка бюллетеня всегда требует ввода `Active` в строковых байтах, и окружающая среда не должна зависеть от переключателей проверки для простой проверки.

РБАК
- Разрешения Execucao на цепочку exige:
  - Предложения: `CanProposeContractDeployment{ contract_id }`
  - Бюллетени: `CanSubmitGovernanceBallot{ referendum_id }`
  - Принятие: `CanEnactGovernance`
  - Управление советом (будущее): `CanManageParliament`Защита пространств имен
- Пользовательский параметр `gov_protected_namespaces` (массив строк JSON) позволяет разрешить вход для развертывания списков пространств имен.
- Клиенты могут включать в себя части метаданных транзакций для развертывания защищенных пространств имен:
  - `gov_namespace`: пространство имен alvo (например, «приложения»).
  - `gov_contract_id`: идентификатор контракта, логический в пространстве имен.
- `gov_manifest_approvers`: необязательный массив JSON для идентификаторов учетных записей валидаторов. Когда манифест объявляет о большем кворуме, для допуска требуется разрешение на транзакцию больше, чем в списках для удовлетворения или кворума в манифесте.
- Телеметрия показывает контадорам допуск через `governance_manifest_admission_total{result}` для того, чтобы отдельные операторы допускали успехи в управлении `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` e `runtime_hook_rejected`.
- Телеметрия показывает или контролирует принудительное исполнение через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`), чтобы операторы аудита подтвердили ошибочные действия.
- Применяется список разрешенных пространств имен, публикуемых в своих манифестах. Qualquer Transacao, который определяет `gov_namespace`, создает `gov_contract_id`, а пространство имен, которое создается в сочетании с `protected_namespaces`, делает манифест. Отправленные `RegisterSmartContractCode` содержат метаданные, которые могут быть изменены, когда они защищены этой способностью.
- Допуск наложен на то, что существует предложение правительства, принятое для кортежа `(namespace, contract_id, code_hash, abi_hash)`; В противном случае может произойти ошибка NotPermited.

Перехватчики обновления среды выполнения
- В манифестах устройства объявлен `hooks.runtime_upgrade` для инструкций по обновлению среды выполнения (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Кампос делает крючок:
  - `allow` (bool, по умолчанию `true`): когда `false`, все это в качестве инструкций по обновлению среды выполнения, которые необходимо обновить.
  - `require_metadata` (bool, по умолчанию `false`): запросить ввод конкретных метаданных для `metadata_key`.
  - `metadata_key` (строка): имя приложения для метаданных. По умолчанию `gov_upgrade_id`, когда требуются метаданные и список разрешений.
  - `allowed_ids` (массив строк): дополнительный список разрешенных значений метаданных (после обрезки). Rejeita quando o valor fornecido nao esta listado.
- Когда есть крючок, допустите приложение к политике метаданных перед транзакцией, входящей в файл. Метаданные доступны, они доступны на закрытом этапе или в списке разрешенных, где возникает детерминированная ошибка NotPermitted.
- Результаты Telemetria rastreia через `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, которые удовлетворяют требованиям или включают в себя метаданные `gov_upgrade_id=<value>` (или могут быть определены в манифесте), одновременно с подтверждением подтверждения exigidas pelo quorum do манифеста.

Конечная точка удобства
- POST `/v1/gov/protected-namespaces` - применение `gov_protected_namespaces` прямо нет нет.
  - Реквизиты: { "пространства имен": ["приложения", "система"] }
  - Ответ: { "ок": true, "применено": 1 }
  - Примечания: предназначено для администратора/тестирования; запросите токен API, который настроен. Для производства необходимо отправить транзакцию с помощью `SetParameter(Custom)`.Помощники CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Контракты для пространства имен и конференций:
    - Байт-код Torii для каждого `code_hash`, и его дайджест Blake2b-32 соответствует `code_hash`.
    - О манифесте, подписанном `/v1/contracts/code/{code_hash}`, отчете `code_hash` и корреспондентах `abi_hash`.
    - Существует предложение правительства, принятое в пункте `(namespace, contract_id, code_hash, abi_hash)`, полученное из одного хеш-кода предложения, которое не используется в США.
  - Создайте связанный JSON с `results[]` для договора (проблемы, резюме манифеста/кода/предложения) больше, чем резюме, которое вы знаете (`--no-summary`).
  - Используйте для проверки пространств имен, защиты или проверки потоков управления развертыванием.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Создайте или создайте JSON-метаданные, используемые при развертывании субметров в защищенных пространствах имен, включая опции `gov_manifest_approvers` для удовлетворения требований кворума в манифесте.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — подсказки блокировки, необходимые для `min_bond_amount > 0`, и некоторые подсказки, которые позволяют включить `owner`, `amount` и `duration_blocks`.
  — Проверяет канонические идентификаторы учетных записей, канонизирует 32-байтовые подсказки обнулителя и объединяет подсказки в `public_inputs_json` (с `--public <path>` для дополнительных переопределений).
  - Обнулитель получается из обязательства доказательства (общедоступный вклад) плюс `domain_tag`, `chain_id` и `election_id`; `--nullifier` проверяется на соответствие при поставке.
  - O резюме ума linha agora exibe `fingerprint=<hex>`, определенное производное от `CastZkBallot`, кодифицированное вместе с декодированными подсказками (`owner`, `amount`, `duration_blocks`, `direction`, когда это произошло).
  - В ответ на CLI с анотатом `tx_instructions[]` с `payload_fingerprint_hex` больше всего декодированных файлов для последующей проверки или повторной реализации декодированного Norito.
  - Fornecer подсказывает, что блокировка разрешает отсутствие событий `LockCreated`/`LockExtended` для избирательных бюллетеней ZK, которые позволяют использовать схему, исключающую важные значения.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Псевдонимы `--lock-amount`/`--lock-duration-blocks` используются в именах флагов ZK для обеспечения безопасности сценария.
  - В резюме указано `vote --mode zk`, включая отпечатки пальцев, кодифицированные инструкции и поля для легального голосования (`owner`, `amount`, `duration_blocks`, `direction`), предлагается подтвердить Rapida Antes de Assinar или esqueleto.Список экземпляров
- GET `/v1/gov/instances/{ns}` - список экземпляров контративных действий для пространства имен.
  - Параметры запроса:
    - `contains`: фильтрация подстроки `contract_id` (с учетом регистра)
    - `hash_prefix`: фильтр по шестнадцатеричному префиксу `code_hash_hex` (строчные буквы)
    - `offset` (по умолчанию 0), `limit` (по умолчанию 100, максимум 10_000)
    - `order`: um de `cid_asc` (по умолчанию), `cid_desc`, `hash_asc`, `hash_desc`
  - Ответ: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Вспомогательный SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) или `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Варредура де разблокировки (Операдор/Аудитория)
- ПОЛУЧИТЬ `/v1/gov/unlocks/stats`
  - Ответ: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Примечания: `last_sweep_height` отражает изменение последнего блока, когда замки истекли для варок и сохранялись. `expired_locks_now` и рассчитывается по различным регистрам блокировки с помощью `expiry_height <= height_current`.
- ПОСТ `/v1/gov/ballots/zk-v1`
  - Реквизиция (стиль DTO v1):
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
  - Введите JSON `BallotProof` прямо и отправьте запрос `CastZkBallot`.
  - Реквизиция:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "голосование": {
        "бэкэнд": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 выполняет контейнер ZK1 или H2*
        "root_hint": null, // необязательная 32-байтовая шестнадцатеричная строка (корень соответствия)
        "владелец": ноль, // AccountId необязательно, когда владелец схемы компрометирован
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
    - На сервере карты `root_hint`/`owner`/`nullifier` можно проголосовать за `public_inputs_json` в `CastZkBallot`.
    - Байты перекодируются в конверте в формате Base64 для полезной нагрузки инструкций.
    - Ответ `reason` нужно сделать для `submitted transaction`, когда Torii подметка или бюллетень.
    - Это конечная точка, поэтому она отключена, когда функция `zk-ballot` уже доступна.Проверка подлинности CastZkBallot
- `CastZkBallot` декодирует проверочное base64 и удаляет различные или некорректные полезные нагрузки (`BallotRejected` с `invalid or empty proof`).
- Хост может разрешить проверяющему голосу участвовать в голосовании на референдуме (`vk_ballot`) или указать параметры управления по умолчанию и указать существующий реестр, используя `Active` и содержащий байты в строке.
- Байты проверенной версии с повторным хэшированием с помощью `hash_vk`; какое-либо несоответствие обязательства прервать выполнение перед проверкой для защиты от входа в реестр фальсификаций (`BallotRejected` с `verifying key commitment mismatch`).
- Байты подтверждения, отправленные на серверную часть, зарегистрированные через `zk::verify_backend`; транскрипции недействительны, как `BallotRejected`, как `invalid proof`, и являются инструкциями по детерминированной форме.
- Доказательства должны раскрывать обязательства по голосованию и основу права на участие в качестве общественного мнения; корень должен соответствовать `eligible_root` выборов, а производный обнулитель должен соответствовать любой предоставленной подсказке.
- Provas bem-sucedidas emitem `BallotAccepted`; нуллификаторы дубликатов, корни устаревшей элегантности или регресс блокировки, продолжающей производство по мере того, как существует существующее описание, предшествующее этому документу.

## Mau comportamento de validadores e consenso conjunto

### Fluxo-де-рубка и тюремное заключение

По согласованию `Evidence` кодифицируется в Norito, когда он действительно соответствует протоколу. Полезная нагрузка, записанная в виде `EvidenceStore` в памяти, неизданная и материализованная без карты `consensus_evidence`, повторно используется для WSV. Большинство регистров, которые `sumeragi.npos.reconfig.evidence_horizon_blocks` (блоки по умолчанию `7200`), являются rejeitados для любого или ограниченного архива, а также для отмены и регистрации для операций. Свидетельства в пределах горизонта также учитывают `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и косую задержку `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`); руководство может отменить штрафы с помощью `CancelConsensusEvidencePenalty` до того, как будет применено сокращение.

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind`; Модель данных os Discriminantes sao estaveis e impostos pelo:

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

- **DoublePrepare/DoubleCommit** - проверка конфликтующих хэшей для одного кортежа `(phase,height,view,epoch)`.
- **InvalidQc** - сплетни о сертификате фиксации cuja forma falha em checagens deterministicas (например, растровое изображение подписавших сторон).
- **InvalidProposal** - лидер проп os um bloco que falha validacao estrutural (например, viola a regra de locked-chain).
- **Цензура** — подписанные квитанции об отправке показывают транзакцию, которая никогда не предлагалась/не совершалась.

Операторы и инструментальные подъемники проверяют и ретранслируют полезную нагрузку через:

- Torii: `GET /v1/sumeragi/evidence` и `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, и `... submit --evidence-hex <payload>`.

Управление должно обрабатывать байты доказательств как канонические доказательства:1. **Убрать полезную нагрузку** до истечения срока действия. Архивируйте байты Norito brutos junto с метаданными высоты/вида.
2. **Подготовьте наказание**, встроив его в референдум или инструктируя sudo (например, `Unregister::peer`). Повторная проверка полезной нагрузки; доказательства неправильной формы или устаревшие и определенные.
3. **Запланируйте топологию сопровождения**, чтобы проверяющий информатор не мог немедленно вернуться. Fluxos типичные файлы в файлах `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)` в составе или в настроенном списке.
4. **Результаты проверки** через `/v1/sumeragi/evidence` и `/v1/sumeragi/status` для гарантии того, что контадор доказательств будет отправлен и что управление будет применено и удалено.

### Последовательность совместного согласия

Согласие на объединение гарантирует, что объединение валидаторов заявления завершит блокирование границ перед тем, как новое объединение придет в надлежащем порядке. Во время выполнения можно применить обновление через параметры parareados:

- `SumeragiParameter::NextMode` и `SumeragiParameter::ModeActivationHeight` были подтверждены отсутствием **блока**. `mode_activation_height` должен быть установлен главным образом для того, чтобы изменить блок, который можно настроить, прежде чем он будет блокироваться с задержкой.
- `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и защита конфигурации, препятствующая передаче управления с нулевой задержкой:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`) задерживает сокращение консенсуса, чтобы руководство могло отменить штрафы до их применения.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Во время выполнения и параметры CLI, передаваемые через `/v1/sumeragi/params` и `iroha --output-format text ops sumeragi params`, для того, чтобы операторы подтверждали изменения активности и списки валидаторов.
- Автоматическое управление всегда будет:
  1. Завершите принятие решения об удалении (или реинтеграции) в соответствии с доказательствами.
  2. Выполните перенастройку аккомпанемента с помощью `mode_activation_height = h_current + activation_lag_blocks`.
  3. Монитор `/v1/sumeragi/status` съел троакар `effective_consensus_mode`, и его снова удалось избежать.

Qualquer скрипт, который вращает валидаторы или аппликацию, разрезает **nao deve**, чтобы активировать нулевую задержку или пропустить параметры передачи; tais transacoes sao rejeitadas e deixam a rede no modo anterior.

## Особенности телеметрии- Метрики Prometheus для экспорта активности правительства:
  - `governance_proposals_status{status}` (датчик) влияет на статус предложений.
  - `governance_protected_namespace_total{outcome}` (счетчик) увеличивается при входе в защищенные пространства имен или при разрешении развертывания.
  - `governance_manifest_activations_total{event}` (счетчик) регистрации вставок манифеста (`event="manifest_inserted"`) и привязок пространства имен (`event="instance_bound"`).
- `/status` включает объект `governance`, который содержит предложения, связанные со всеми защищенными пространствами имен и списками последних активных манифестов (пространство имен, идентификатор контракта, хеш-код/ABI, высота блока, временная метка активации). Операторы могут проконсультироваться по этому поводу для подтверждения того, что акты актуализируются и что шлюзы пространств имен защищены и отправлены приложениями.
- Шаблон Grafana (`docs/source/grafana_governance_constraints.json`) и книга телеметрии в `telemetry.md` поддерживаются в качестве предупреждений о предварительных предложениях, активируемых манифестах или ненадежных пространствах имен, защищенных во время обновлений во время выполнения.