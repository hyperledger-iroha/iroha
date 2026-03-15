---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Сообщение: Создано/записано в журнале "Старый мир". Он был главой государства. Создание базы данных RBAC в режиме онлайн. Его имя: Стив Сейлор/Страна, штат Калифорния, США `authority` и `private_key`, Он был установлен в `/transaction`.

نظرة عامة
- Вы можете просмотреть JSON. Для получения дополнительной информации обратитесь к `tx_instructions` - مصفوفة من تعليمة هيكلية Ответ на вопрос:
  - `wire_id`: معرّف السجل لنوع التعليمة
  - `payload_hex`: Код Norito (шестнадцатеричный)
- اذا تم توفير `authority` и `private_key` (также `private_key` в бюллетенях DTO) и Torii Он был установлен в `tx_instructions`.
- Для создания подписи SignedTransaction необходимо указать полномочия и Chain_id для отправки в POST. Это `/transaction`.
- Поддержка SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` или `GovernanceProposalResult` (отображение статуса/вида), и `ToriiClient.get_governance_referendum_typed`, `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed`, `GovernanceTally`, `ToriiClient.get_governance_locks_typed`, `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed`. `GovernanceUnlockStats`, و`ToriiClient.list_governance_instances_typed` и `GovernanceInstancesPage`, а затем Уинстон набрал عبر سطح الحوكمة مع Ознакомьтесь с README.
- В Python используется (`iroha_torii_client`): `ToriiClient.finalize_referendum` и `ToriiClient.enact_proposal`, введенный `GovernanceInstructionDraft` напечатан (записано). `tx_instructions` вместо Torii), а также файл JSON, который необходимо завершить и принять.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` позволяет помощникам набрать команду للمقترحات, والاستفتاءات, وtallies, وlocks, واحصاءات unlock. На `listGovernanceInstances(namespace, options)` в совете (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) на сайте Используйте Node.js в приложении `/v2/gov/instances/{ns}` и в VRF-файле. العقود الموجودة.

نقاط النهاية

- ПОСТ `/v2/gov/proposals/deploy-contract`
  - Формат (JSON):
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
  - Формат (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Сообщение: Зарегистрируйтесь `abi_hash` для `abi_version`. التطابق. Для `abi_version = "v1"` используйте `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API-интерфейс (развертывание)
- ПОСТ `/v2/contracts/deploy`
  - الطلب: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Код: `code_hash` в جسم برنامج IVM и `abi_hash` в ترويسة. `abi_version`, `RegisterSmartContractCode` (манифест) и `RegisterSmartContractBytes` (запись `.to`) `authority`.
  - الرد: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Ответ:
    - GET `/v2/contracts/code/{code_hash}` -> Открыть манифест
    - GET `/v2/contracts/code-bytes/{code_hash}` -> يعيد `{ code_b64 }`
- ПОСТ `/v2/contracts/instance`
  - الطلب: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Сообщение: Установлено приложение `ActivateContractInstance`.
  - الرد: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

خدمة الاسماء المستعارة
- ПОСТ `/v2/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  - Сообщение: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` находится в разработке. Сообщение: `blake2b512-mock`.
  - Сообщение: высмеивает Дэвиса Blake2b512 на сайте `iroha.alias.voprf.mock.v1`. Он был открыт для использования в VOPRF Iroha.
  - Сообщение: HTTP `400` соответствует шестнадцатеричному значению. Установите Torii на Norito `ValidationFail::QueryFailed::Conversion` для декодера.
- ПОСТ `/v2/aliases/resolve`
  - الطلب: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - الرد: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Добавлено: установлен мост ISO (`[iso_bridge.account_aliases]` или `iroha_config`). يقوم Torii. قبل البحث. В 404 году он был запущен в 2017 году и 503 был запущен во время выполнения с использованием моста ISO.
- ПОСТ `/v2/aliases/resolve_index`
  - Сообщение: { "индекс": 0 }
  - الرد: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Название: Написано с участием Пьера Стоуна в Колумбии (отсчет от 0). يمكن للعملاء تخزين الردود offline لبناء مسارات تدقيق لاحداث аттестация الخاصة بالاسماء.

حد حجم الكود
- Дополнительный файл: `max_contract_code_bytes` (JSON u64)
  - Написано в фильме "Убийца" (Бантан) в фильме "Старый мир".
  - Размер: 16 МБ. Инвариант `RegisterSmartContractBytes` используется в качестве инварианта.
  - Установите флажок `SetParameter(Custom)` и `id = "max_contract_code_bytes"` в исходном состоянии.- ПОСТ `/v2/gov/ballots/zk`
  - الطلب: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - الرد: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Ответ:
    - عندما تتضمن المدخلات العامة للدائرة `owner` и `amount` и `duration_blocks`, وتتحقق Посетите ВК и создайте замок на `election_id`. `owner`. Дополнительный файл (`unknown`); Указанная сумма/срок действия. Монотонный: сумма с истечением срока действия (срок действия max(amount, prev.amount) иmax(expiry, prev.expiry)).
    - Сумма платежа ZK, указанная в заказе, до истечения срока действия может быть получена в ближайшее время. `BallotRejected`.
    - установите флажок `ZK_VOTE_VERIFY_BALLOT` или установите `SubmitBallot`; Установите защелку на место.

- ПОСТ `/v2/gov/ballots/plain`
  - الطلب: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Да|Нет|Воздержаться" }
  - الرد: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Сообщение: Вы можете указать сумму, указанную в бюллетене, до истечения срока действия. Он был установлен на `owner`. Установите флажок `conviction_step_blocks`.

- ПОСТ `/v2/gov/finalize`
  - الطلب: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Добавлено сообщение (открыто): Вы можете развернуть файл `ContractManifest` ادنى. بمفتاح `code_hash` и `abi_hash` Введено в действие. В манифесте появится `code_hash` и `abi_hash`, который будет открыт.
  - Ответ:
    - Установите ZK, установите флажок `ZK_VOTE_VERIFY_TALLY` для `FinalizeElection`; Установите защелку на место. يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء tally للانتخابات.
    - الاغلاق التلقائي عند `h_end` يصدر Утверждено/Отклонено فقط للاستفتاءات Plain; تبقى استفتاءات ZK Закрыто.
    - Явка تستخدم одобрить+отклонить فقط؛ воздержались из-за явки избирателей.

- ПОСТ `/v2/gov/enact`
  - الطلب: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Код: Torii для проверки подлинности `authority`/`private_key`; Уилла Хейлала Лэрри Уинстона. Прообраз اختيارية وحاليا معلوماتية.

- ПОЛУЧИТЬ `/v2/gov/proposals/{id}`
  - المسار `{id}`: шестнадцатеричное число символов (64 символа).
  - الرد: { "найдено": bool, "предложение": { ... }? }

- ПОЛУЧИТЬ `/v2/gov/locks/{rid}`
  - Код `{rid}`: строка отсутствует.
  - الرد: { "Найдено": bool, "referendum_id": "rid", "locks": { ... }? }- ПОЛУЧИТЬ `/v2/gov/council/current`
  - الرد: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Место: Совет города в Калининграде. Нападающий Дэниел Стоун стал игроком VRF Он используется для VRF в режиме онлайн).

- POST `/v2/gov/council/derive-vrf` (функция: gov_vrf)
  - الطلب: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Сообщение: было создано в VRF в Лос-Анджелесе в рамках программы `chain_id`. و`epoch` — хеш-код Размер файла bytes الخرج desc مع كاسرات تعادل؛ Он был создан `committee_size` в стране. لا يتم الحفظ.
  - الرد: { "эпоха": N, "участники": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Формат: Normal = pk для G1, доказательство для G2 (96 байт). Small = pk в G2, доказательство в G1 (48 байт). Установите флажок `chain_id`.

### Дополнительная информация (iroha_config `gov.*`)

Совет директоров الاحتياطي الذي يستخخدمه Torii входит в список участников команды `iroha_config`:

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

Информационные сообщения:

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

`parliament_committee_size` — резервный вариант резервного копирования, созданный в рамках совета штата Нью-Йорк, و`parliament_term_blocks`. Вы можете получить семя семян (`epoch = floor(height / term_blocks)`) и `parliament_min_stake` для получения дополнительной ставки. (Пансион) على اصل الاهلية, و`parliament_eligibility_asset_id`, созданный для того, чтобы сделать это. Дэнни Уилсон.

Обход ВКонтакте: просмотр бюллетеней для голосования `Active` ببايتات inline, и вы можете использовать его в качестве источника информации.

РБАК
- Сообщение о том, что происходит с вами:
  - Предложения: `CanProposeContractDeployment{ contract_id }`
  - Бюллетени: `CanSubmitGovernanceBallot{ referendum_id }`
  - Принятие: `CanEnactGovernance`
  - Управление советом (مستقبلا): `CanManageParliament`

Пространства имен
- Добавление `gov_protected_namespaces` (массив JSON со строками) для шлюзования и развертывания пространств имен.
- Вы можете просмотреть метаданные, чтобы развернуть все пространства имен:
  - `gov_namespace`: пространство имен الهدف (مثلا "приложения").
  - `gov_contract_id`: изменение пространства имен.
- `gov_manifest_approvers`: массив JSON содержит идентификаторы учетных записей. Вы должны создать манифест и кворум, чтобы получить доступ к допуску, чтобы получить кворум. الحسابات الخاص بالmanifest.
- تكشف التليمترية عدادات допуск عبر `governance_manifest_admission_total{result}` لتمييز القبولات الناجحة عن مسارات `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` و`runtime_hook_rejected`.
- Для обеспечения соблюдения требований `governance_manifest_quorum_total{outcome}` (`satisfied` / `rejected`) Это произошло в 2017 году.
- Создавать белый список пространств имен для манифестов. Для создания `gov_namespace` в интерфейсе `gov_contract_id`, перейдите в пространство имен `protected_namespaces` манифест. Он был создан `RegisterSmartContractCode` и содержит метаданные, которые можно просмотреть.
- Входной билет принят в действие в соответствии с `(namespace, contract_id, code_hash, abi_hash)`; Недопустимо использование NotPermited.Хуки для выполнения во время выполнения
- В первом манифесте отображается время выполнения `hooks.runtime_upgrade` для запуска процесса выполнения (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- крючок:
  - `allow` (bool, например `true`): запустите `false` при помощи `false`. Время выполнения.
  - `require_metadata` (bool, например `false`): метаданные отображаются в формате `metadata_key`.
  - `metadata_key` (строка): метаданные и крючок. Добавьте `gov_upgrade_id` в список разрешенных метаданных.
  - `allowed_ids` (массив из строк): список разрешенных метаданных для метаданных (обрезка). Он сказал, что он хочет сделать это.
- Он зацепил Крюка, когда он получил доступ к метаданным "Старый Стокгольм", чтобы получить доступ к метаданным. метаданные могут быть добавлены в список разрешенных и запрещены.
- Установите флажок `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Запустите крючок и загрузите метаданные `gov_upgrade_id=<value>` (можно открыть в манифесте). Был создан манифест кворума مدققين بواسطة الخاص بال.

Конечная точка
- POST `/v2/gov/protected-namespaces` - يطبق `gov_protected_namespaces` для проверки.
  - الطلب: { "namespaces": ["apps", "system"] }
  - الرد: { "ОК": правда, "применено": 1 }
  - Название: مخصص للادارة/الاختبار؛ Токен API доступен для скачивания. Установите флажок `SetParameter(Custom)`.Использование CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Чтобы просмотреть пространство имен, введите:
    - Torii соответствует байт-коду `code_hash` и дайджесту Blake2b-32 `code_hash`.
    - манифест `/v2/contracts/code/{code_hash}` и `code_hash` и `abi_hash`.
    - В соответствии с Законом о хешировании введен в действие код `(namespace, contract_id, code_hash, abi_hash)` для хеширования идентификатора предложения الذي تستخدمه العقدة.
  - Создание файла JSON `results[]` для проверки (проблемы, манифест/код/предложение) Был установлен флажок (`--no-summary`).
  - Создание пространств имен и возможность развертывания локальных пространств имен.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Добавление метаданных JSON для развертывания пространств имен в файле `gov_manifest_approvers`. الاختيارية لتلبية قواعد quorum لللmanifest.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — установите флажок `min_bond_amount > 0` и установите флажок `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`. Установите флажок `owner` и `amount` и `duration_blocks`.
  — Проверяет канонические идентификаторы учетных записей, канонизирует 32-байтовые подсказки обнулителя и объединяет подсказки в `public_inputs_json` (с `--public <path>` для дополнительных переопределений).
  - Обнулитель получается из обязательства доказательства (общедоступный вклад) плюс `domain_tag`, `chain_id` и `election_id`; `--nullifier` проверяется на соответствие при поставке.
  - Найдите подсказки для `fingerprint=<hex>` и подсказки для `CastZkBallot`. Проверьте (`owner`, `amount`, `duration_blocks`, `direction`).
  - Запустите CLI для `tx_instructions[]` и `payload_fingerprint_hex`, чтобы загрузить его с помощью `payload_fingerprint_hex`. Он был создан в 2008 году в 18NT00000006X.
  - توفير намекает на замок, чтобы получить доступ к `LockCreated`/`LockExtended`, чтобы просмотреть бюллетени ZK بمجرد ان. تكشف الدائرة القيم نفسها.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Псевдонимы `--lock-amount`/`--lock-duration-blocks` используются для флагов الخاصة بـ ZK لتحقيق تماثل السكربتات.
  - Чтобы получить отпечаток пальца `vote --mode zk`, необходимо проверить бюллетень для голосования. (`owner`, `amount`, `duration_blocks`, `direction`).

قائمة المثيلات
- GET `/v2/gov/instances/{ns}` - скопируйте пространство имен.
  - Параметры запроса:
    - `contains`: Загрузка подстроки в `contract_id` (с учетом регистра)
    - `hash_prefix`: تصفية بحسب بادئة hex لـ `code_hash_hex` (строчные буквы)
    - `offset` (по умолчанию 0), `limit` (по умолчанию 100, максимум 10_000)
    - `order`: используется для `cid_asc` (по умолчанию), `cid_desc`, `hash_asc`, `hash_desc`.
  - الرد: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Вспомогательный SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) и `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Разблокировка مسح (المشغل/التدقيق)
- ПОЛУЧИТЬ `/v2/gov/unlocks/stats`
  - الرد: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Код: `last_sweep_height` для блокировки блокировки. `expired_locks_now` включает блокировку `expiry_height <= height_current`.
- ПОСТ `/v2/gov/ballots/zk-v1`
  - Обновление (DTO نمط v1):
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
  - الرد: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- ПОСТ `/v2/gov/ballots/zk-v1/ballot-proof` (характеристика: `zk-ballot`)
  - Файл JSON `BallotProof` создается в формате `CastZkBallot`.
  - Вопрос:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "голосование": {
        "бэкэнд": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 для ZK1 и H2*
        "root_hint": null, // необязательная 32-байтовая шестнадцатеричная строка (корень соответствия)
        "owner": null, // AccountId
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
  - Ответ:
    - Для голосования `root_hint`/`owner`/`nullifier` используйте бюллетень для голосования `public_inputs_json` لـ `CastZkBallot`.
    - Загрузка байтов в конверт в base64 и загрузка полезной нагрузки.
    - Выберите `reason` или `submitted transaction` для голосования Torii.
    - Конечная точка связана с функцией `zk-ballot`.

Сообщение от CastZkBallot
- `CastZkBallot` используется в базе данных base64 и может быть использован в качестве исходного кода. (`BallotRejected` или `invalid or empty proof`).
- Для голосования на референдуме (`vk_ballot`) Встроенное значение يكون السجل و`Active` содержит встроенные байты.
- байты, необходимые для хеширования `hash_vk`; В 1980-х годах он был рожден в 1980-х годах, когда его пригласили на работу в США. Это (`BallotRejected` или `verifying key commitment mismatch`).
- байты данных для внутренней обработки `zk::verify_backend`; Установите флажок `BallotRejected` или `invalid proof`, чтобы установить его.
- Доказательства должны раскрывать обязательства по голосованию и основу права на участие в качестве общественного мнения; корень должен соответствовать `eligible_root` выборов, а производный обнулитель должен соответствовать любой предоставленной подсказке.
- Установите флажок `BallotAccepted`; нуллификаторы и нуллификаторы для блокировки и блокировки Он сказал Сэнсэюсу в 2017 году.## Сэнсэй Слон и ее сын

### سير عمل нанесение порезов и тюремное заключение

Код `Evidence` установлен в Norito, который был отправлен в США. Он был создан в `EvidenceStore` в режиме реального времени, а также в 2017 году в США. Загрузите `consensus_evidence` в WSV. Для этого необходимо установить `sumeragi.npos.reconfig.evidence_horizon_blocks` (название `7200`) на сайте `sumeragi.npos.reconfig.evidence_horizon_blocks`. Он родился в Лос-Анджелесе. Свидетельства в пределах горизонта также учитывают `sumeragi.npos.reconfig.activation_lag_blocks` (по умолчанию `1`) и косую задержку `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`); руководство может отменить штрафы с помощью `CancelConsensusEvidencePenalty` до того, как будет применено сокращение.

الانتهاكات المعترف بها تقابل واحدا لواحد مع `EvidenceKind`; Ниже приводится сообщение о том, что:

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

- **DoublePrepare/DoubleCommit** - хэши создаются для кортежа `(phase,height,view,epoch)`.
- **InvalidQc** - Отменяется сертификат фиксации, полученный в результате проверки подлинности (растровое изображение отображается в виде растрового изображения).
- **InvalidProposal** - قدم قائد بلوكا يفشل التحقق التحقق التحقق البنيوي (مثلا يكسر قاعدة lock-chain).
- **Цензура** — подписанные квитанции об отправке показывают транзакцию, которая никогда не предлагалась/не совершалась.

В 2007 году он сказал:

- Torii: `GET /v2/sumeragi/evidence` и `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, و`... submit --evidence-hex <payload>`.

Введите в список байтов доказательства, полученные в результате:

1. **جمع الحمولة** قبل ان تتقادم. Задайте байты Norito для метаданных и высоты/представления.
2. **Выполните команду** для проведения референдума по поводу sudo (например, `Unregister::peer`). تعيد عملية التنفيذ التحقق من الحمولة؛ доказательства المشوهة او القديمة ترفض حتميا.
3. **Получите информацию о ** Создан состав `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)`.
4. **Для проверки** عبر `/v2/sumeragi/evidence` و`/v2/sumeragi/status` для проверки доказательств طبقت الازالة.

### تسلسل الاجماع المشترك

В 2007 году он был выбран в качестве президента США. В 2007 году он был отправлен в Палестину. Во время выполнения можно выполнить следующие действия:

- Написано на `SumeragiParameter::NextMode` и `SumeragiParameter::ModeActivationHeight` на **نفس البلوك**. Он был создан в честь `mode_activation_height`, когда он был отправлен в США в 2007 году. У него было отставание.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) и защитник Тэхен передаёт мяч с задержкой:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (по умолчанию `259200`) задерживает сокращение консенсуса, чтобы руководство могло отменить штрафы до их применения.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Среда выполнения и CLI проиндексировали проиндексированные `/v2/sumeragi/params` и `iroha --output-format text ops sumeragi params`. Это произошло в 2017 году в Уэльсе.
- Сообщение от Тэхена в честь Дня святого Валентина:
  1. انهاء قرار الازالة (او الاستعادة) المدعوم بـ доказательства.
  2. Установите флажок `mode_activation_height = h_current + activation_lag_blocks`.
  3. Установите `/v2/sumeragi/status` на `effective_consensus_mode` и замените его.

С Скиннером в фильме "Сердце" и "Слэшер" ** и с Дэвидом Джонсом отстают. Передача мяча Он сказал, что хочет, чтобы это произошло.

## اسطح التليمترية- Код Prometheus для проверки:
  - `governance_proposals_status{status}` (манометр) необходимо установить на место.
  - `governance_protected_namespace_total{outcome}` (счетчик) вызывается при входе в пространство имен.
  - `governance_manifest_activations_total{event}` (счетчик) в манифесте манифеста (`event="manifest_inserted"`) и пространстве имен (`event="instance_bound"`).
- `/status` используется для `governance` для изменения пространств имен. Создание манифеста (пространство имен, идентификатор контракта, код/хеш ABI, высота блока, временная метка активации). В соответствии с законодательными актами, манифестирует новые пространства имен. مطبقة.
- Grafana (`docs/source/grafana_governance_constraints.json`) и Runbook для `telemetry.md` при запуске. Создание манифеста пространства имен и создание пространств имен. Свободная среда выполнения.