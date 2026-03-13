---
lang: he
direction: rtl
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

סטאטוס: черновик/набросок для сопровождения задач по реализации ממשל. Формы могут меняться в ходе разработки. Детерминизм и политика RBAC являются нормативными ограничениями; Torii יש להציע/לפרט את האפשרויות ל-`authority` ו-`private_key`, ибичина. отправляют в `/transaction`.

Обзор
- נקודות קצה נוספות возвращают JSON. Для потоков, которые создают транзакции, ответы включают `tx_instructions` - массив одной или несколь инструкций-скелетов:
  - `wire_id`: реестровый идентификатор типа инструкции
  - `payload_hex`: байты מטען Norito (hex)
- Если `authority` ו-`private_key` (או `private_key` בקלפי DTO), Torii הפצות транзакцию и все равно возвращает `tx_instructions`.
- Иначе клиенты собирают SignedTransaction со своей Authority и chain_id, затем подписывают и POST в `/transaction`.
- Покрытие SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` возвращает `GovernanceProposalResult` (нормализует поля status/kind), `ToriiClient.get_governance_referendum_typed` возвращает `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` возвращает `GovernanceTally`, `ToriiClient.get_governance_locks_typed` возвращает `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` возвращает `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` возвращает `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` вос0 `ToriiClient.list_governance_instances_typed` возвращает `GovernanceInstancesPage`, обеспечивая типизированный доступ по всей поверхности ממשל привис.
- Легкий Python клиент (`iroha_torii_client`): `ToriiClient.finalize_referendum` ו-`ToriiClient.enact_proposal` возвращают типизированные חבילות Prometheus Torii שלד `tx_instructions`), избегая ручного JSON-парсинга при сборке סיום/הפעלת זרימות.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` предоставляет типизированные עוזרים להצעות, משאל משאל, תיאום, מנעולים, סטטיסטיקות ביטול נעילה, и теперь Norito endpoints Council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), чтобы клиенты Node.js могли пагинировать и0060 זרימות עבודה מגובות VRF нду с существующим списком контрактных инстансов.

נקודות קצה

- POST `/v2/gov/proposals/deploy-contract`
  - בקשה (JSON):
    {
      "namespace": "אפליקציות",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "i105...?",
      "private_key": "...?"
    }
  - תגובה (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - אימות: ноды канонизируют `abi_hash` ל-заданного `abi_version` ו- отвергают несовпадения. Для `abi_version = "v1"` ожидаемое значение - `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.Contracts API (פריסה)
- POST `/v2/contracts/deploy`
  - בקשה: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - התנהגות: вычисляет `code_hash` по телу IVM программы ו `abi_hash` по заголовку Sumeragi по заголовку Sumeragi `RegisterSmartContractCode` (מניפסט) ו-`RegisterSmartContractBytes` (פולני `.to` байты) от имени `authority`.
  - תגובה: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - קשור:
    - קבל `/v2/contracts/code/{code_hash}` -> מניפסט возвращает сохраненный
    - קבל `/v2/contracts/code-bytes/{code_hash}` -> возвращает `{ code_b64 }`
- POST `/v2/contracts/instance`
  - בקשה: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - התנהגות: деплоит предоставленный bytecode и сразу активирует маппинг `(namespace, contract_id)` через `ActivateContractInstance`.
  - תגובה: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

שירות כינוי
- POST `/v2/aliases/voprf/evaluate`
  - בקשה: { "blinded_element_hex": "..." }
  - תגובה: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценщика. טכניקה: `blake2b512-mock`.
  - הערות: детерминированный mock оценщик, применяющий Blake2b512 с הפרדת תחום `iroha.alias.voprf.mock.v1`. Предназначен для тестового כלי עבודה, ייצור צינור VOPRF не подключен к Iroha.
  - שגיאות: HTTP `400` при некорректном hex вводе. Torii возвращает Norito מעטפת `ValidationFail::QueryFailed::Conversion` с сообщением декодера.
- POST `/v2/aliases/resolve`
  - בקשה: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - תגובה: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - הערות: צור זמן ריצה של גשר ISO (`[iso_bridge.account_aliases]` в `iroha_config`). Torii כינוי נורמאלי, удаляя пробелы и приводя к верхнему регистру. Возвращает 404 при отсутствии alias и 503, когда ISO bridge runtime отключен.
- POST `/v2/aliases/resolve_index`
  - בקשה: { "אינדקס": 0 }
  - תגובה: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - הערות: индексы כינוי назначаются детерминированно по порядку конфигурации (מבוסס 0). Клиенты могут кэшировать ответы לא מקוון для построения מסלול ביקורת по событиям аттестации כינוי.

כובע גודל קוד
- פרמטר מותאם אישית: `max_contract_code_bytes` (JSON u64)
  - Управляет максимальным допустимым размером (в байтах) хранения кода контрактов בשרשרת.
  - ברירת מחדל: 16 MiB. Ноды отклоняют `RegisterSmartContractBytes`, когда размер `.to` изображения превышает лимит, с ошибкой inviolant.
  - Операторы могут изменять через `SetParameter(Custom)` с `id = "max_contract_code_bytes"` и числовым מטען.- POST `/v2/gov/ballots/zk`
  - בקשה: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - תגובה: { "OK": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות:
    - Когда публичные входы схемы включают `owner`, `amount` ו-`duration_blocks`, и proof верисфиц מנעול ממשל ל-`election_id` בשיטת `owner`. Направление скрыто (`unknown`); обновляются только סכום/תפוגה. Повторные голоса монотонны: סכום и expiry только увеличиваются (нода применяет max(amount, prev.amount) и max(expiry, prev.expiry)).
    - ZK הצבעות מחדש, пытающиеся уменьшить סכום или תפוגה, отклоняются сервером с диагностикой `BallotRejected`.
    - Исполнение контракта должно вызвать `ZK_VOTE_VERIFY_BALLOT` до постановки `SubmitBallot`; хосты לאכוף בריח одноразовый.

- POST `/v2/gov/ballots/plain`
  - בקשה: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "directionay"}: "Aye|tainN
  - תגובה: { "OK": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות: הצבעות חוזרות только на расширение - новый הקלפי не может уменьшить סכום или expiry существующего נעילת. `owner` должен совпадать с Authority транзакции. Минимальная длительность - `conviction_step_blocks`.

- POST `/v2/gov/finalize`
  - בקשה: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - אפקט על השרשרת (פיגום נוכחי): חקיקת הצעה לפרוס утвержденного вставляет минимальный `ContractManifest`, привязанный к `code_hash`, с оNIж10м с оNIж10X, с оNIж10X и помечает ההצעה как נחקק. Если manifest уже существует для `code_hash` с другим `abi_hash`, enactment отклоняется.
  - הערות:
    - Для ZK בחירות контракты должны вызвать `ZK_VOTE_VERIFY_TALLY` до `FinalizeElection`; хосты לאכוף בריח одноразовый. `FinalizeReferendum` отклоняет ZK-референдумы, пока countly выборов не финализирован.
    - Автозакрытие на `h_end` эмитит אושר/נדחה только для Plain-референдумов; ZK-референдумы остаются סגור, לא ניתן לראות את המספר הפיננסי והמוכר של `FinalizeReferendum`.
    - שיעור ההצבעה Проверки используют только approve+reject; נמנע не учитывается בשיעור ההצבעה.

- POST `/v2/gov/enact`
  - בקשה: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - הערות: Torii отправляет подписанную транзакцию при наличии `authority`/`private_key`; иначе возвращает שלד для подписи и отправки клиентом. Preimage опционален и сейчас носит информационный характер.- קבל את `/v2/gov/proposals/{id}`
  - נתיב `{id}`: hex זיהוי הצעה (64 תווים)
  - תגובה: { "נמצא": bool, "הצעה": { ... }? }

- קבל את `/v2/gov/locks/{rid}`
  - נתיב `{rid}`: מחרוזת מזהה משאל עם
  - תגובה: { "נמצא": bool, "referendum_id": "לשחרר", "מנעולים": { ... }? }

- קבל `/v2/gov/council/current`
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - הערות: возвращает המועצה התמידית при наличии; иначе деривирует детерминированный fallback, используя настроенный נכס סיכון וספים сохранены על השרשרת).

- POST `/v2/gov/council/derive-vrf` (תכונה: gov_vrf)
  - בקשה: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "רגיל|קטן", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - התנהגות: проверяет VRF proof каждого кандидата на каноническом קלט, полученном из `chain_id`, `epoch` ו-beacon hasпледе; сортирует по פלט בתים desc с שוברי שוויון; возвращает top `committee_size` участников. Не сохраняется.
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - הערות: רגיל = pk × G1, הוכחה × G2 (96 בתים). קטן = pk × G2, הוכחה × G1 (48 בתים). כניסות доменно разделены и включают `chain_id`.

### ברירות מחדל של ממשל (iroha_config `gov.*`)

Council Fallback, используемый Torii при отсутствии סגל מתמשך, параметризуется через `iroha_config`:

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

`parliament_committee_size` ограничивает число fallback членов при отсутствии Council, `parliament_term_blocks` задает длину эпохи для גזירה 30X9), (000000138X) `parliament_min_stake` требует минимальный מניות (в минимальных единицах) על נכס זכאות, а `parliament_eligibility_asset_id` выбирает, калаканс, asset при построении набора кандидатов.

ממשל VK אימות ללא עקיפת: проверка קלפי всегда требует `Active` אימות מפתח с בתים מוטבעים, и окружения не должны полагаться, בדיקות пропускать проверку.

RBAC
- מוצר זמין על השרשרת:
  - הצעות: `CanProposeContractDeployment{ contract_id }`
  - קלפי: `CanSubmitGovernanceBallot{ referendum_id }`
  - חקיקה: `CanEnactGovernance`
  - הנהלת המועצה (עתיד): `CanManageParliament`מרחבי שמות מוגנים
- פרמטר מותאם אישית `gov_protected_namespaces` (מערך מחרוזות JSON) включает דלת כניסה לפורסת מרחבי שמות.
- Клиенты должны включать מפתחות מטא-נתונים транзакции для פריסה, направленных במרחבי שמות מוגנים:
  - `gov_namespace`: מרחב שמות целевой (לפני, "אפליקציות")
  - `gov_contract_id`: логический חוזה מזהה внутри מרחב שמות
- `gov_manifest_approvers`: опциональный חשבונות חשבונות מערך JSON валидаторов. מניפסט Когда ליין задает מניין > 1, קבלה требует סמכות транзакции плюс перечисленные аккаунты для удовлетворения quorum манифете.
- Телеметрия предоставляет דלפקי כניסה через `governance_manifest_admission_total{result}`, чтобы операторы различали успешные admits от Prometheus, Prometheus,000X `quorum_rejected`, `protected_namespace_rejected`, ו-`runtime_hook_rejected`.
- נתיב אכיפה Телеметрия показывает через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`), позволяя опарать אישורים недостающие.
- נתיבים אוכפים את רשימת ההיתרים של מרחב השמות, опубликованный в מניפסטים. Любая транзакция, устанавливающая `gov_namespace`, должна предоставить `gov_contract_id`, a space name должен предоставить `protected_namespaces` מניפסטה. הגשות `RegisterSmartContractCode` ללא מטא נתונים отвергаются, когда защита включена.
- קבלה требует, чтобы существовал הצעת ממשל שנחקקה עבור tuple `(namespace, contract_id, code_hash, abi_hash)`; иначе валидация завершается ошибкой NotPermitted.

ווי שדרוג זמן ריצה
- Lane manifests могут объявлять `hooks.runtime_upgrade` לשדרוג זמן ריצה של שער инструкций (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- וו פוליה:
  - `allow` (bool, ברירת המחדל `true`): когда `false`, все שדרוג זמן ריצה инструкции отклоняются.
  - `require_metadata` (bool, ברירת המחדל `false`): требует מטא נתונים של כניסה, указанную `metadata_key`.
  - `metadata_key` (מחרוזת): имя metadata, הוק מאולץ. ברירת מחדל `gov_upgrade_id`, когда metadata требуется или есть רשימת ההיתרים.
  - `allowed_ids` (מערך של מחרוזות): опциональный רשימת ההיתרים значений metadata (после trim). Отклоняет, когда предоставленное значение не входит в список.
- Когда hook присутствует, קבלה очереди לאכוף מטא נתונים политику до попадания транзакции в очередь. Отсутствующая מטא נתונים, пустые значения или значения вне רשימת ההיתרים дают детерминированный NotPermitted.
- Телеметрия отслеживает תוצאות через `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, удовлетворяющие הוק, должны включать metadata `gov_upgrade_id=<value>` (או תקליט, אופציונלי אופציונלי, מניפסט תקף של אפליקציות, מניפסט תקף) מניין מניפסט требуемыми.

נקודת קצה של נוחות
- POST `/v2/gov/protected-namespaces` - מוצר `gov_protected_namespaces` פורסם כעת.
  - בקשה: { "מרחבי שמות": ["אפליקציות", "מערכת"] }
  - תגובה: { "בסדר": true, "applied": 1 }
  - הערות: предназначен для admin/testing; требует API token при конфигурации. В ייצור предпочтительнее подписанная транзакция с `SetParameter(Custom)`.CLI עוזרי
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Получает контрактные инстансы למרחב שמות и проверяет, что:
    - Torii хранит bytecode для каждого `code_hash`, ו-Eго Blake2b-32 digest соответствует `code_hash`.
    - Manifest под `/v2/contracts/code/{code_hash}` сообщает совпадающие `code_hash` ו-`abi_hash`.
    - הצעת ממשל חוקקה על ידי Существует ל-`(namespace, contract_id, code_hash, abi_hash)`, деривированный тем же offer-id hashing, что использует нода.
  - Выводит JSON отчет с `results[]` по каждому контракту (בעיות, מניפסט/קוד/סיכומי הצעה) ותקציר אופנתי, еслианло по100X (100X).
  - ניתן להשתמש במרחבי שמות מוגנים או להגדיר זרימות עבודה מבוקרות של ממשל.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - צור מטא-נתונים של JSON שלד ל-деплоя в מרחבי שמות מוגנים, включая опциональные `gov_manifest_approvers` ל- соблюдения quorum правил манифест.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` - רמזים לנעילה обязательны при `min_bond_amount > 0`, и любой набор hints должен включать `results[]`, I010000190X, I010000190X, I010000190X, I010000190X, I010000190X, `duration_blocks`.
  - מאמת מזהי חשבונות קנוניים, מבצע קנוניזציה של רמזים לביטול של 32 בתים וממזג את הרמזים לתוך `public_inputs_json` (עם `--public <path>` לעקיפות נוספות).
  - המבטל נגזר מהתחייבות ההוכחה (קלט ציבורי) בתוספת `domain_tag`, `chain_id` ו-`election_id`; `--nullifier` מאומת כנגד ההוכחה כאשר היא מסופקת.
  - תקציר אופטימלי теперь содержит детерминированный `fingerprint=<hex>` из кодированного `CastZkBallot` вместаннированный с дированный (`owner`, `amount`, `duration_blocks`, `direction` לבנים).
  - CLI ответы аннотируют `tx_instructions[]` полем `payload_fingerprint_hex` и декодированными полями, чтобы downstream tooling моглиб приовно реализации Norito פענוח.
  - רמזים למנעול Предоставление позволяет ноде эмитить `LockCreated`/`LockExtended` לקלפי ZK, когда схемаетрывя раски.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Алиасы `--lock-amount`/`--lock-duration-blocks` отражают имена ZK флагов для parity в скриптах.
  - סיכום вывод отражает `vote --mode zk`, включая טביעת אצבע закодированной инструкции и читаемые קלפי поля (Prometheus,Prometheus, `duration_blocks`, `direction`), давая быстрое подтверждение перед подписью שלד.

רישום מופעים
- קבל `/v2/gov/instances/{ns}` - список активных контрактных инстансов למרחב שמות.
  - פרמטרים של שאילתה:
    - `contains`: фильтр по תת-מחרוזת `contract_id` (תלוי רישיות)
    - `hash_prefix`: фильтр по קידומת hex `code_hash_hex` (אותיות קטנות)
    - `offset` (ברירת מחדל 0), `limit` (ברירת מחדל 100, מקסימום 10_000)
    - `order`: `cid_asc` (ברירת מחדל), `cid_desc`, `hash_asc`, `hash_desc`
  - תגובה: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - עוזר SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) או `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).ביטול נעילת סריקה (מפעיל/ביקורת)
- קבל את `/v2/gov/unlocks/stats`
  - תגובה: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - הערות: `last_sweep_height` отражает последний גובה בלוק, когда מנעולים שפג תוקפם были swep и נמשכים. `expired_locks_now` рассчитывается путем сканирования נעילת רשומות с `expiry_height <= height_current`.
- POST `/v2/gov/ballots/zk-v1`
  - בקשה (DTO בסגנון v1):
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "owner": "i105...?",
      "nullifier": "blake2b32:...64hex?"
    }
  - תגובה: { "OK": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v2/gov/ballots/zk-v1/ballot-proof` (תכונה: `zk-ballot`)
  - פרינט `BallotProof` JSON שלד ושלד `CastZkBallot`.
  - בקשה:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "הצבעה": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 קונטרול ZK1 או H2*
        "root_hint": null, // מחרוזת hex אופציונלית של 32 בתים (שורש זכאות)
        "owner": null, // AccountId אופציונלי когда מעגל фиксирует הבעלים
        "nullifier": null // מחרוזת hex אופציונלית של 32 בתים (רמז לבטל)
      }
    }
  - תגובה:
    {
      "בסדר": נכון,
      "מקובל": נכון,
      "reason": "בנה שלד עסקה",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - הערות:
    - מפת שרת אופציונלית `root_hint`/`owner`/`nullifier` בקלפי ב-`public_inputs_json` ל-`CastZkBallot`.
    - מעטפה בתים повторно кодируются в base64 לעזר מטען инструкции.
    - `reason` меняется на `submitted transaction`, когда Torii отправляет קלפי.
    - Этот נקודת קצה доступен только при включенном תכונה `zk-ballot`.נתיב אימות CastZkBallot
- `CastZkBallot` декодирует предоставленную base64 proof и отклоняет пустые/битые מטענים (`BallotRejected` с Sumeragi).
- Хост разрешает מפתח אימות הצבעה из משאל עם (`vk_ballot`) או ברירות מחדל של ממשל и требует, чтобы запись существовала, буществовала, була I1025080X содержала בתים מוטבעים.
- Сохраненные אימות-key bytes повторно хешируются с `hash_vk`; любое несовпадение התחייבות останавливает выполнение до проверки для защиты от подмененных ערכי הרישום (Prometheus ).
- Proof bytes отправляются в зарегистрированный backend через `zk::verify_backend`; תמלילים לא חוקיים приводят к `BallotRejected` с `invalid proof` и инструкция детерминированно падает.
- על ההוכחה לחשוף התחייבות קלפי ושורש זכאות כתשומות ציבוריות; השורש חייב להתאים ל-`eligible_root` של הבחירות, והמבטל הנגזר חייב להתאים לכל רמז שסופק.
- Успешные הוכחות эмитят `BallotAccepted`; повторные nullifiers, устаревшие שורשי זכאות או נעילה продолжают давать существующие причины откеразан, .

## Ненадлежащее поведение валидаторов и совместный консенсус

### המשך חיתוך וכלא

Консенсус эмитит Norito מקודד `Evidence` при нарушениях протокола валидатором. Каждый מטען попадает в in-memory `EvidenceStore` и, если он новый, материализуется ב-WSV בגיבוי `consensus_evidence`. Записи старше `sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת המחדל של `7200` בלוקים) отклоняются, чтобы архив оставался ограничензятим, для операторов. הראיות באופק מכבדות גם את `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) ואת עיכוב החיתוך `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`); ממשל יכול לבטל קנסות עם `CancelConsensusEvidencePenalty` לפני שהחתך חל.

Распознанные нарушения отображаются один-к-одному на `EvidenceKind`; дискриминанты стабильны и принудительно зафиксированы במודל נתונים:

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

- **DoublePrepare/DoubleCommit** - валидатор подписал конфликтующие хэши для одного `(phase,height,view,epoch)`.
- **InvalidQc** - אישור התחייבות агрегатор разослал, чья форма не проходит детерминированные проверки (например, пустой signer bitmap).
- **InvalidProposal** - лидер предложил блок, который проваливает структурную валидацию (например, нарушает חוק שרשרת נעולה).
- **צנזורה** - קבלות הגשה חתומות מציגות עסקה שמעולם לא הוצעה/בוצעה.

Операторы и инструменты могут просматривать и повторно рассылать מטענים через:

- Torii: `GET /v2/sumeragi/evidence` ו-`GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.

ממשל должна рассматривать ראיות בתים как каноническое доказательство:1. **Собрать מטען** до истечения срока. Архивировать сырые Norito בתים вместе с גובה/תצוגה מטא נתונים.
2. **Подготовить штраф** встроив מטען в משאל или הוראות סודו (например, `Unregister::peer`). Исполнение повторно валидирует מטען; פגום או ראיות מעופשות отклоняется детерминированно.
3. **Запланировать מעקב топологию** чтобы нарушивший валидатор не смог сразу вернуться. Типовые потоки ставят `SetParameter(Sumeragi::NextMode)` ו-`SetParameter(Sumeragi::ModeActivationHeight)` с обновленным סגל.
4. **Аудит результатов** через `/v2/sumeragi/evidence` ו-`/v2/sumeragi/status`, чтобы убедиться, что счетчик הוכחות вырос и ממשל.

### Последовательность קונצנזוס משותף

קונצנזוס משותף гарантирует, что исходный набор валидаторов финализирует пограничный блок до того, как новный набор. אכיפת זמן ריצה правило через парные параметры:

- `SumeragiParameter::NextMode` ו-`SumeragiParameter::ModeActivationHeight` должны быть committed в **том же блоке**. `mode_activation_height` должен быть строго больше высоты блока, который внес обновление, обеспечивая минимимум.
- `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) - זה שומר קונפיגורטיבי, מעביר מסירה ללא פגישות:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`) מעכב את קיצוץ הקונצנזוס כך שהממשל יכול לבטל עונשים לפני שהם יחולו.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- זמן ריצה ו-CLI מציגים מופעים מבוימים של `/v2/sumeragi/params` ו-`iroha --output-format text ops sumeragi params`, ניתנים להורדה של גבהי הפעלה וסגל.
- Автоматизация ממשל всегда должна:
  1. Финализировать решение об исключении (или восстановлении), поддержанное ראיות.
  2. התקן מעקב אחר תצורה מחדש с `mode_activation_height = h_current + activation_lag_blocks`.
  3. Мониторить `/v2/sumeragi/status` לשירות `effective_consensus_mode` ב-ожидаемой высоте.

Любой скрипт, который ротирует валидаторов или применяет slashing, **לא תקף** пытаться הפעלת יד אפס או אופטימיזציה; такие транзакции отклоняются и оставляют сеть в предыдущем режиме.

## משטחי טלמטריה

- Prometheus שיטות ממשל למטרות ספורט:
  - `governance_proposals_status{status}` (מד) отслеживает количество הצעות по статусу.
  - `governance_protected_namespace_total{outcome}` (מונה) увеличивается при לאפשר/לדחות קבלה למרחבי שמות מוגנים.
  - `governance_manifest_activations_total{event}` (מונה) фиксирует вставки מניפסט (`event="manifest_inserted"`) и כריכות מרחב שמות (`event="instance_bound"`).
- `/status` включает объект `governance`, который зеркалит счетчики הצעות, сообщает סכומים по מרחבי שמות מוגנים במניפסטים הפעלות (מרחב שם, מזהה חוזה, קוד/גיבוש ABI, גובה בלוק, חותמת זמן הפעלה). Операторы могут опрашивать это поле, чтобы убедиться, что enactments обновили מניפסטים и что space names ports соблюдаются.
- תבנית Grafana (`docs/source/grafana_governance_constraints.json`) ו-telmetry runbook ב-`telemetry.md`. הפעלות או מרחבי שמות מוגנים ללא שדרוגי זמן ריצה.