---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Ответ: Как выбрать или использовать عملدرآمد کے دوران ساختیں بدل سکتی ہیں۔ Детерминизм `authority` и `private_key` могут быть использованы для Torii. سکتا ہے، ورنہ کلائنٹس بنا کر `/transaction` پر سبمٹ کرتے ہیں۔

جائزہ
- Просмотр конечных точек в формате JSON Если вы хотите использовать `tx_instructions`, вы можете использовать его в качестве источника питания. Скелеты инструкций в массиве:
  - `wire_id`: инструкция по указанию идентификатора реестра.
  - `payload_hex`: Norito байты полезной нагрузки (шестнадцатеричные)
- Выберите `authority` или `private_key` (для голосования DTO или `private_key`) или Torii. Если вы хотите использовать `tx_instructions`, вы можете использовать его.
- Чтобы получить полномочия для Chain_id и SignedTransaction, выберите `/transaction`. پر POST کرتے ہیں۔
- Покрытие SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` واپس کرتا ہے (поля статуса/типа могут нормализовать کرتا ہے), `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` Встроенная система `ToriiClient.get_governance_tally_typed` `GovernanceTally` Дополнительная информация `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats` или `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage`. کرتا ہے، جس سے پورے поверхность управления پر типизированный доступ ملتا ہے اور README میں примеры использования دیے گئے ہیں۔
- Облегченный клиент Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` или `ToriiClient.enact_proposal`, типизированный `GovernanceInstructionDraft`, пакеты и дополнительные возможности (Torii). `tx_instructions` скелет, обертка, скрипты, завершение/введение потоков, ручной синтаксический анализ JSON, и др.
- JavaScript (`@iroha/iroha-js`): предложения `ToriiClient`, референдумы, подсчеты, блокировки, статистика разблокировки и типизированные помощники, а также `listGovernanceInstances(namespace, options)` для конечных точек совета. (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) Доступны клиенты Node.js `/v1/gov/instances/{ns}` для разбивки на страницы Использование рабочих процессов с поддержкой VRF и список экземпляров контракта.

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
  - Проверка: узлы могут быть обнаружены `abi_version` или `abi_hash`, чтобы канонизировать или отклонить несоответствие. ہیں۔ `abi_version = "v1"` کے لئے متوقع value `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ہے۔API контрактов (развертывание)
- ПОСТ `/v1/contracts/deploy`
  - Запрос: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Поведение: IVM в заголовке `code_hash` `abi_version` в `abi_hash` в заголовке IVM `RegisterSmartContractCode` (манифест) и `RegisterSmartContractBytes` (за исключением `.to` байт) `authority`. ہے۔
  - Ответ: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Связано:
    - GET `/v1/contracts/code/{code_hash}` -> ذخیرہ شدہ Manifest واپس کرتا ہے
    - GET `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }` в исходном коде.
- ПОСТ `/v1/contracts/instance`
  - Запрос: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Поведение: развертывание байт-кода или отображение `ActivateContractInstance` или отображение `(namespace, contract_id)`. ہے۔
  - Ответ: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Служба псевдонимов
- ПОСТ `/v1/aliases/voprf/evaluate`
  - Запрос: { "blinded_element_hex": "..." }
  - Ответ: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - Реализация оценщика `backend`. Значение имени: `blake2b512-mock`۔.
  - Примечания: детерминированный макетный оценщик جو Blake2b512 для разделения доменов `iroha.alias.voprf.mock.v1` для применения کرتا ہے۔ یہ испытательные инструменты کے لئے ہے جب تک производственный трубопровод VOPRF Iroha میں проволока نہ ہو جائے۔
  - Ошибки: неправильный шестнадцатеричный ввод HTTP `400`۔. Torii Norito `ValidationFail::QueryFailed::Conversion` сообщение об ошибке декодера конверта واپس کرتا ہے۔
- ПОСТ `/v1/aliases/resolve`
  - Запрос: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Ответ: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Примечания: Промежуточный этап выполнения моста ISO درکار ہے (`[iso_bridge.account_aliases]` в `iroha_config`)۔ Torii пробелы ہٹا کر اور в верхнем регистре بنا کر search کرتا ہے۔ псевдоним от 404 до 404 в среде выполнения моста ISO от 503 до 503
- ПОСТ `/v1/aliases/resolve_index`
  - Запрос: { "индекс": 0 }
  - Ответ: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Примечания: порядок конфигурации индексов псевдонимов کے مطابق детерминированный طریقے سے назначить ہوتے ہیں (на основе 0). Автономный кэш и события аттестации псевдонимов, журналы аудита и многое другое.

Код Размер ограничения
- Пользовательский параметр: `max_contract_code_bytes` (JSON u64).
  - Хранение кода контракта в сети.
  - По умолчанию: 16 МБ۔ `.to` image حد سے بڑی ہو تو nodes `RegisterSmartContractBytes` کو ошибка нарушения инварианта или отклонить ошибку
  - Операторы `SetParameter(Custom)` کے ذریعے `id = "max_contract_code_bytes"` اور числовая полезная нагрузка دے کر ایڈجسٹ کر سکتے ہیں۔- ПОСТ `/v1/gov/ballots/zk`
  - Запрос: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Ответ: { "ОК": правда, "принято": правда, "tx_instructions": [{...}] }
  - Примечания:
    - Схема для общедоступных входов `owner`, `amount`, `duration_blocks` для проверки конфигурации VK и подтверждения для проверки. В узле `election_id` можно отключить блокировку управления. направление چھپی رہتی ہے (`unknown`); Сумма/срок действия اپڈیٹ ہوتے ہیں۔ повторное голосование монотонное ہیں: сумма до истечения срока действия صرف بڑھتے ہیں (node ​​max(amount, prev.amount) اور max(expiry, prev.expiry) لگاتا ہے)۔
    - ZK повторно голосует за сумму до истечения срока действия или отклоняет серверную диагностику `BallotRejected`.
    - Выполнение контракта может быть поставлено в очередь `SubmitBallot` или `ZK_VOTE_VERIFY_BALLOT` для завершения процесса. применение одноразовой блокировки хостов کرتے ہیں۔

- ПОСТ `/v1/gov/ballots/plain`
  - Запрос: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "владелец": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Да|Нет|Воздержаться" }
  - Ответ: { "ОК": правда, "принято": правда, "tx_instructions": [{...}] }
  - Примечания: повторное голосование только для продления. `owner` — полномочия по транзакциям, необходимые для управления транзакциями. کم از کم مدت `conviction_step_blocks` ہے۔

- ПОСТ `/v1/gov/finalize`
  - Запрос: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Эффект on-chain (текущий эшафот): Вы можете развернуть предложение и ввести в действие `code_hash` с минимальным ключом `ContractManifest`, чтобы установить его. متوقع `abi_hash` ہوتا ہے اور предложение принято ہو جاتا ہے۔ `code_hash` может быть использован `abi_hash` в манифесте, если законодательный акт отклонен.
  - Примечания:
    - Выборы ZK и пути контракта `FinalizeElection` سے پہلے `ZK_VOTE_VERIFY_TALLY` کال کرنا لازمی ہے؛ применение одноразовой блокировки хостов کرتے ہیں۔ `FinalizeReferendum` ZK референдумы могут быть отклонены или подведены итоги выборов نہ ہو جائے۔
    - Автоматическое закрытие `h_end` پر صرف Обычные референдумы کے لئے Утверждено/Отклонено. Референдумы ZK закрыты
    - Проверка явки: одобрение+отклонение воздержаться от явки میں شمار نہیں ہوتا۔

- ПОСТ `/v1/gov/enact`
  - Запрос: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Ответ: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Примечания: جب `authority`/`private_key` فراہم ہوں تو Torii подписал транзакцию سبمٹ کرتا ہے؛ ورنہ وہ скелет и его скелет прообраз اختیاری اور فی الحال معلوماتی ہے۔- ПОЛУЧИТЬ `/v1/gov/proposals/{id}`
  - Путь `{id}`: шестнадцатеричный идентификатор предложения (64 символа).
  - Ответ: { "найдено": bool, "предложение": { ... }? }

- ПОЛУЧИТЬ `/v1/gov/locks/{rid}`
  - Путь `{rid}`: строка идентификатора референдума.
  - Ответ: { "найдено": bool, "referendum_id": "rid", "locks": { ... }? }

- ПОЛУЧИТЬ `/v1/gov/council/current`
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." }, ...] }
  - Примечания. کرتا ہے (Спецификации VRF کو اس وقت تک отражают کرتا ہے جب تک живые доказательства VRF в цепочке сохраняются نہ ہں)۔

- POST `/v1/gov/council/derive-vrf` (функция: gov_vrf)
  - Запрос: { "committee_size": 21, "эпоха": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Поведение: наличие VRF-доказательства `chain_id`, `epoch` для блокировки хеш-маяка и канонического ввода данных. проверить наличие ہے؛ выходные байты по описанию и тай-брейкеры по сортировке лучшие участники `committee_size` واپس کرتا ہے۔ Persist نہیں کرتا۔
  - Ответ: { "эпоха": N, "участники": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Примечания: Нормальный = pk в G1, доказательство в G2 (96 байт). Small = pk в G2, доказательство в G1 (48 байт). Входы разделены доменами.

### Настройки управления по умолчанию (iroha_config `gov.*`)

Torii для сохранения списка и резервного совета `iroha_config` для параметризации параметров:

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

Эквивалентные переопределения среды:

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

`parliament_committee_size` резервные члены, чтобы ограничить ограничение, или совет сохраняется, и `parliament_term_blocks`, определите длину эпохи, чтобы определить происхождение семени. لئے استعمال ہوتا ہے (`epoch = floor(height / term_blocks)`), `parliament_min_stake` актив, отвечающий критериям участия в доле (наименьшие единицы), обеспечивает соблюдение требований `parliament_eligibility_asset_id` может предоставить кандидатам возможность проверить баланс активов и выполнить сканирование.

Проверка управления ВКонтакте Возможность обхода проверки: проверка бюллетеней `Active` проверка ключа и встроенные байты для проверки сред только для тестирования переключает

РБАК
- Дополнительные разрешения на выполнение в цепочке:
  - Предложения: `CanProposeContractDeployment{ contract_id }`
  - Бюллетени: `CanSubmitGovernanceBallot{ referendum_id }`
  - Принятие: `CanEnactGovernance`
  - Управление советом (в будущем): `CanManageParliament`Защищенные пространства имен
- Специальный параметр `gov_protected_namespaces` (массив строк JSON) отображает пространства имен, которые позволяют развернуть входной шлюз и получить доступ к ним.
- Клиенты, использующие защищенные пространства имен, могут развертывать ключи метаданных транзакций, а также:
  - `gov_namespace`: целевое пространство имен (описание: «приложения»).
  - `gov_contract_id`: пространство имен для идентификатора логического контракта.
- `gov_manifest_approvers`: дополнительный массив JSON идентификаторов учетных записей валидатора. Кворум манифеста полосы движения > 1 объявить, что требуется допуск, полномочия по транзакциям и перечисленные учетные записи. سکے۔
- Телеметрия `governance_manifest_admission_total{result}` обеспечивает целостные счетчики приема, а операторы допускают `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, или `runtime_hook_rejected` سے فرق کر سکیں۔
- Телеметрия `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`).
- Дорожки отображают список разрешенных пространств имен и принудительное соблюдение ограничений. Если вы хотите использовать `gov_namespace`, выберите пространство имен `gov_contract_id`, чтобы просмотреть пространство имен и манифест. `protected_namespaces` سیٹ میں ہونا چاہئے۔ `RegisterSmartContractCode` отправляет метаданные, включает защиту, отклоняет или отклоняет.
- Прием должен обеспечить соблюдение требований `(namespace, contract_id, code_hash, abi_hash)` کے لئے Принятое предложение по управлению موجود ہو؛ Ошибка проверки NotPermited или ошибка неудачи ہوتا ہے۔

Приемы обновления среды выполнения
- Манифесты Lane `hooks.runtime_upgrade` объявляют о наличии инструкций по обновлению среды выполнения (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) и воротах. سکے۔
- Поля крючков:
  - `allow` (bool, по умолчанию `true`): `false` ہو تو تمام инструкции по обновлению во время выполнения отклоняются ہو جاتے ہیں۔
  - `require_metadata` (bool, по умолчанию `false`): `metadata_key` کے مطابق запись метаданных درکار ہے۔
  - `metadata_key` (строка): перехватчик для принудительного имени метаданных. По умолчанию `gov_upgrade_id` Требуется метаданные.
  - `allowed_ids` (массив строк): значения метаданных и необязательный список разрешений (обрезать или удалить). Выберите значение, которое вам нужно, или отклоните.
- Перехватчик позволяет включить вход в очередь и настроить очередь, чтобы обеспечить соблюдение политики метаданных. Отсутствуют метаданные, значения, список разрешенных значений, детерминированные значения NotPermited.
- Телеметрия `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` کے ذریعے отслеживать результаты کرتی ہے۔
- Перехватчик может быть использован для метаданных `gov_upgrade_id=<value>` (ключ, определенный в манифесте). Кворум манифеста, наличие валидаторов и одобрений, а также наличие кворума

Удобство конечной точки
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` کو براہ راست node پر apply کرتا ہے۔
  - Запрос: { "пространства имен": ["приложения", "система"] }
  - Ответ: { «ОК»: правда, «применено»: 1 }
  - Примечания: администрирование/тестирование کے لئے ہے؛ Как настроить токен API или использовать токен API производство کے لئے `SetParameter(Custom)` کے ساتھ подписанная транзакция ترجیح دیں۔Помощники CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Пространство имен и экземпляры контракта извлекают или перекрестно проверяют:
    - Torii ہر `code_hash` کے لئے bytecode ذخیرہ کرتا ہے، اور اس کا Blake2b-32 дайджест `code_hash` سے match کرتا ہے۔
    - `/v1/contracts/code/{code_hash}` определяет соответствие манифеста `code_hash` и значения `abi_hash` رپورٹ کرتا ہے۔
    - `(namespace, contract_id, code_hash, abi_hash)` کے لئے принятое предложение по управлению موجود ہے جو اسی хеширование идентификатора предложения سے получение ہوتا ہے جو node استعمال کرتا ہے۔
  - `results[]` کے ساتھ JSON رپورٹ دیتا ہے (проблемы, сводки манифеста/кода/предложения) `--no-summary` نہ ہو)۔
  - Защищенные пространства имен, аудит, рабочие процессы развертывания, контролируемые управлением, и контрольные проверки.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Защищенные пространства имен, развертывание и скелет метаданных JSON, дополнительные функции `gov_manifest_approvers` и дополнительные правила кворума манифеста.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` ہونے کسی بھی подсказки для блокировки подсказок. Если `owner`, `amount` или `duration_blocks`, это может быть сделано в случае необходимости.
  — Проверяет канонические идентификаторы учетных записей, канонизирует 32-байтовые подсказки обнулителя и объединяет подсказки в `public_inputs_json` (с `--public <path>` для дополнительных переопределений).
  - Обнулитель получается из обязательства доказательства (общедоступный вклад) плюс `domain_tag`, `chain_id` и `election_id`; `--nullifier` проверяется на соответствие при поставке.
  - Детерминированный `fingerprint=<hex>` и закодированный `CastZkBallot` для получения декодированных подсказок (`owner`, `amount`, `duration_blocks`, `direction` جب فراہم ہوں)۔
  - Ответы CLI `tx_instructions[]` или `payload_fingerprint_hex` для декодированных полей или аннотаций, а также скелета последующих инструментов. Norito декодирование کے проверить کر سکے۔
  - Подсказки по блокировке دینے سے node ZK избирательные бюллетени کے لئے `LockCreated`/`LockExtended` события выдают کر سکتا ہے جب цепи и значения выставляют کرے۔
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` псевдонимы ZK-флаги или зеркало, или контроль четности сценариев.
  - Сводный вывод `vote --mode zk`, закодированный отпечаток инструкции и читаемые поля голосования (`owner`, `amount`, `duration_blocks`, `direction`). ہے، جس سے подпись سے پہلے فوری تصدیق ہو جاتی ہے۔

Листинг экземпляров
- GET `/v1/gov/instances/{ns}` - пространство имен для экземпляров активных контрактов.
  - Параметры запроса:
    - `contains`: `contract_id` — подстрока или фильтр (с учетом регистра)
    - `hash_prefix`: `code_hash_hex` — шестнадцатеричный префикс и фильтр (строчные)
    - `offset` (по умолчанию 0), `limit` (по умолчанию 100, максимум 10_000)
    - `order`: `cid_asc` (по умолчанию), `cid_desc`, `hash_asc`, `hash_desc`
  - Ответ: { "пространство имен": "ns", "экземпляры": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Помощник SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) или `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Разблокировать проверку (оператор/аудит)
- ПОЛУЧИТЬ `/v1/gov/unlocks/stats`
  - Ответ: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Примечания: `last_sweep_height` سب سے حالیہ высота блока دکھاتا ہے جہاں просроченные блокировки развертки اور persist کئے گئے۔ `expired_locks_now` для блокировки записей и сканирования, а также для блокировки `expiry_height <= height_current`.
- ПОСТ `/v1/gov/ballots/zk-v1`
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

- ПОСТ `/v1/gov/ballots/zk-v1/ballot-proof` (характеристика: `zk-ballot`)
  - `BallotProof` JSON-файл для создания скелета `CastZkBallot`
  - Запрос:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "голосование": {
        "бэкэнд": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // контейнер ZK1 или H2* в base64
        "root_hint": null, // необязательная 32-байтовая шестнадцатеричная строка (корень соответствия)
        «владелец»: null, // необязательный AccountId и владелец канала фиксирует کرے
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
    - Дополнительный сервер `root_hint`/`owner`/`nullifier` для голосования и `CastZkBallot` или `public_inputs_json` для отображения карты.
    - Конверт байтов и полезная нагрузка инструкций в формате base64 или в кодировании данных
    - جب Torii отправьте бюллетень для голосования или `reason` для `submitted transaction` ہو جاتا ہے۔
    - یہ конечная точка صرف تب دستیاب ہے جب Функция `zk-ballot` включена ہو۔Путь проверки CastZkBallot
- `CastZkBallot` может использоваться для декодирования base64 с доказательством, если вы хотите отклонить полезные нагрузки (`BallotRejected` с `invalid or empty proof`).
- Референдум хоста (`vk_ballot`) یا управление по умолчанию سے голосование с проверкой ключа решения کرتا ہے اور تقاضا کرتا ہے کہ запись موجود ہو، `Active` ہو، اور inline bytes رکھتا ہو۔
- Сохраненные байты проверочного ключа, например `hash_vk`, или хеш-код, или хеш-код. несоответствие обязательств ہو تو проверка سے پہلے выполнение روک دیا جاتا ہے تاکہ подделанные записи реестра سے بچا جا سکے (`BallotRejected` с `verifying key commitment mismatch`).
- Байты доказательства `zk::verify_backend` کے ذریعے зарегистрированный бэкенд или отправка ہوتے ہیں؛ неверные транскрипты `BallotRejected` с `invalid proof` کے ساتھ ظاہر ہوتے ہیں اور инструкция детерминированно завершается сбоем ہوتی ہے۔
- Доказательства должны раскрывать обязательства по голосованию и основу права на участие в качестве общественного мнения; корень должен соответствовать `eligible_root` выборов, а производный обнулитель должен соответствовать любой предоставленной подсказке.
- Успешные доказательства `BallotAccepted` излучают کرتے ہیں؛ повторяющиеся обнулители, устаревшие корни приемлемости, регрессии блокировок и другие причины отклонения.

## Неправильное поведение валидатора при совместном консенсусе

### Slashing اور Рабочий процесс тюремного заключения

Консенсус в валидаторе, который может быть использован для Norito, закодированный `Evidence`, испускает کرتا ہے۔ Полезная нагрузка в памяти `EvidenceStore` позволяет использовать карту `consensus_evidence` с поддержкой WSV. میں материализоваться ہو جاتا ہے۔ `sumeragi.npos.reconfig.evidence_horizon_blocks` (блоки `7200` по умолчанию) Отклонить или отклонить архив, ограниченный операторами کے لئے log کیا جاتا ہے۔ Свидетельства в пределах горизонта также учитывают `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и косую задержку `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`); руководство может отменить штрафы с помощью `CancelConsensusEvidencePenalty` до того, как будет применено сокращение.

Признанные правонарушения `EvidenceKind` سے индивидуальная карта ہوتے ہیں؛ Дискриминанты для модели данных и обеспечения соблюдения требований:

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

- **DoublePrepare/DoubleCommit** - валидатор в формате `(phase,height,view,epoch)` позволяет получить хэши, необходимые для проверки.
- **InvalidQc** - агрегатор проверяет сплетни о сертификатах фиксации, если детерминированные проверки завершаются неудачей (изображение растрового изображения подписывающего лица).
- **InvalidProposal** - лидер предлагает предложение структурной проверки или неудачное решение (правило заблокированной цепочки используется)۔
- **Цензура** — подписанные квитанции об отправке показывают транзакцию, которая никогда не предлагалась/не совершалась.

Операторы используют полезные нагрузки для проверки и ретрансляции, а также:

- Torii: `GET /v1/sumeragi/evidence` или `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, или `... submit --evidence-hex <payload>`.

Управление, байты свидетельства, каноническое доказательство, лечение или лечение:1. ** Срок действия полезной нагрузки** Срок действия истекает в ближайшее время. необработанный Norito байт, высота/вид метаданных, архив, метаданные, архив.
2. **Стадия штрафа** Полезная нагрузка для референдума или инструкция sudo для встраивания команды (مثلا `Unregister::peer`)۔ Полезная нагрузка выполнения может быть проверена искаженные یا устаревшие доказательства решительно отвергнуть ہوتی ہے۔
3. **Расписание последующей топологии کریں** Если валидатор, нарушивший правила, не работает Потоки `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)` обновленный список, очередь, очередь, обновление
4. **Проверка аудита** `/v1/sumeragi/evidence` или `/v1/sumeragi/status` - счетчик доказательств, контроль управления и удаление. کیا ہو۔

### Совместное секвенирование

Совместный консенсус может быть установлен исходящим валидатором, установлен граничным блоком, финализирован, завершен, установлен, предложен, или установлен. Параметры среды выполнения и правила применения правил:

- `SumeragiParameter::NextMode` или `SumeragiParameter::ModeActivationHeight` کو **اسی بلاک** میں commit ہونا چاہیے۔ `mode_activation_height` کو update لانے والے بلاک کی height سے строго بڑا ہونا چاہیے، کم از کم ایک بلاک lag ملے۔
- `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) защита конфигурации и переключение с нулевой задержкой.
- `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`) задерживает сокращение консенсуса, чтобы руководство могло отменить штрафы до их применения.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Промежуточные параметры среды выполнения CLI, такие как `/v1/sumeragi/params` или `iroha --output-format text ops sumeragi params`, могут быть заданы значения высоты активации операторов и списки валидаторов. کی تصدیق کر سکیں۔
- Автоматизация управления:
  1. доказательства پر مبنی удаление (یا восстановление на работе) فیصلہ Finalize کرنا چاہیے۔
  2. `mode_activation_height = h_current + activation_lag_blocks` — создание очереди последующей реконфигурации.
  3. `/v1/sumeragi/status` для регулировки высоты и переключателя `effective_consensus_mode` для переключателя высоты.

Валидаторы скриптов вращаются или применяются слэши **Активация с нулевой задержкой** Параметры передачи управления опускаются или используются Транзакции отклоняются в зависимости от режима выбора режима.

## Поверхности телеметрии

- Prometheus экспорт показателей активности управления:
  - `governance_proposals_status{status}` (датчик) предложения, статус, статус, трек, трек.
  - `governance_protected_namespace_total{outcome}` (счетчик) для приращения защищенных пространств имен, допуска развертывания, разрешения или отклонения.
  - `governance_manifest_activations_total{event}` (счетчик) вставки манифеста (`event="manifest_inserted"`) и привязки пространства имен (`event="instance_bound"`) запись کرتا ہے۔
- `/status` или `governance` объект, отображающий количество предложений, отражающее общее количество защищенных пространств имен, отчет о последних активациях манифеста (пространство имен, идентификатор контракта, код/хеш ABI, высота блока, временная метка активации) Операторы, поля и опросы, а также законодательные акты и манифесты и шлюзы защищенного пространства имен.
- Шаблон Grafana (`docs/source/grafana_governance_constraints.json`) и `telemetry.md` для телеметрии Runbook, зависших предложений, отсутствия активаций манифеста, обновлений среды выполнения и неожиданных событий. отклонение защищенного пространства имен и оповещения, проводное соединение и т. д.