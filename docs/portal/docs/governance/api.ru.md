---
lang: ru
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77a3a111d5dc132351c92b586389766a2d183c8bb60aed68b18032e10421c92
source_last_modified: "2025-12-04T07:55:53.675646+00:00"
translation_last_reviewed: 2026-01-01
---

Статус: черновик/набросок для сопровождения задач по реализации governance. Формы могут меняться в ходе разработки. Детерминизм и политика RBAC являются нормативными ограничениями; Torii может подписывать/отправлять транзакции при наличии `authority` и `private_key`, иначе клиенты собирают и отправляют в `/transaction`.

Обзор
- Все endpoints возвращают JSON. Для потоков, которые создают транзакции, ответы включают `tx_instructions` - массив одной или нескольких инструкций-скелетов:
  - `wire_id`: реестровый идентификатор типа инструкции
  - `payload_hex`: байты payload Norito (hex)
- Если `authority` и `private_key` предоставлены (или `private_key` в DTO ballots), Torii подписывает и отправляет транзакцию и все равно возвращает `tx_instructions`.
- Иначе клиенты собирают SignedTransaction со своей authority и chain_id, затем подписывают и POST в `/transaction`.
- Покрытие SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` возвращает `GovernanceProposalResult` (нормализует поля status/kind), `ToriiClient.get_governance_referendum_typed` возвращает `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` возвращает `GovernanceTally`, `ToriiClient.get_governance_locks_typed` возвращает `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` возвращает `GovernanceUnlockStats`, и `ToriiClient.list_governance_instances_typed` возвращает `GovernanceInstancesPage`, обеспечивая типизированный доступ по всей поверхности governance с примерами использования в README.
- Легкий Python клиент (`iroha_torii_client`): `ToriiClient.finalize_referendum` и `ToriiClient.enact_proposal` возвращают типизированные bundles `GovernanceInstructionDraft` (оборачивают Torii skeleton `tx_instructions`), избегая ручного JSON-парсинга при сборке Finalize/Enact flows.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` предоставляет типизированные helpers для proposals, referenda, tallies, locks, unlock stats, и теперь `listGovernanceInstances(namespace, options)` плюс council endpoints (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), чтобы клиенты Node.js могли пагинировать `/v1/gov/instances/{ns}` и запускать VRF-backed workflows наряду с существующим списком контрактных инстансов.

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - Request (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "alice@wonderland?",
      "private_key": "...?"
    }
  - Response (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validation: ноды канонизируют `abi_hash` для заданного `abi_version` и отвергают несовпадения. Для `abi_version = "v1"` ожидаемое значение - `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

Contracts API (deploy)
- POST `/v1/contracts/deploy`
  - Request: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - Behavior: вычисляет `code_hash` по телу IVM программы и `abi_hash` по заголовку `abi_version`, затем отправляет `RegisterSmartContractCode` (manifest) и `RegisterSmartContractBytes` (полные `.to` байты) от имени `authority`.
  - Response: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Related:
    - GET `/v1/contracts/code/{code_hash}` -> возвращает сохраненный manifest
    - GET `/v1/contracts/code-bytes/{code_hash}` -> возвращает `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Request: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Behavior: деплоит предоставленный bytecode и сразу активирует маппинг `(namespace, contract_id)` через `ActivateContractInstance`.
  - Response: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Alias Service
- POST `/v1/aliases/voprf/evaluate`
  - Request: { "blinded_element_hex": "..." }
  - Response: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценщика. Текущее значение: `blake2b512-mock`.
  - Notes: детерминированный mock оценщик, применяющий Blake2b512 с domain separation `iroha.alias.voprf.mock.v1`. Предназначен для тестового tooling, пока production VOPRF pipeline не подключен к Iroha.
  - Errors: HTTP `400` при некорректном hex вводе. Torii возвращает Norito envelope `ValidationFail::QueryFailed::Conversion` с сообщением декодера.
- POST `/v1/aliases/resolve`
  - Request: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Response: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - Notes: требует ISO bridge runtime staging (`[iso_bridge.account_aliases]` в `iroha_config`). Torii нормализует alias, удаляя пробелы и приводя к верхнему регистру. Возвращает 404 при отсутствии alias и 503, когда ISO bridge runtime отключен.
- POST `/v1/aliases/resolve_index`
  - Request: { "index": 0 }
  - Response: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - Notes: индексы alias назначаются детерминированно по порядку конфигурации (0-based). Клиенты могут кэшировать ответы offline для построения audit trail по событиям аттестации alias.

Code Size Cap
- Custom параметр: `max_contract_code_bytes` (JSON u64)
  - Управляет максимальным допустимым размером (в байтах) on-chain хранения кода контрактов.
  - Default: 16 MiB. Ноды отклоняют `RegisterSmartContractBytes`, когда размер `.to` изображения превышает лимит, с ошибкой invariant violation.
  - Операторы могут изменять через `SetParameter(Custom)` с `id = "max_contract_code_bytes"` и числовым payload.

- POST `/v1/gov/ballots/zk`
  - Request: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes:
    - Когда публичные входы схемы включают `owner`, `amount` и `duration_blocks`, и proof верифицируется с настроенным VK, нода создает или продлевает governance lock для `election_id` с этим `owner`. Направление скрыто (`unknown`); обновляются только amount/expiry. Повторные голоса монотонны: amount и expiry только увеличиваются (нода применяет max(amount, prev.amount) и max(expiry, prev.expiry)).
    - ZK re-votes, пытающиеся уменьшить amount или expiry, отклоняются сервером с диагностикой `BallotRejected`.
    - Исполнение контракта должно вызвать `ZK_VOTE_VERIFY_BALLOT` до постановки `SubmitBallot`; хосты enforce одноразовый latch.

- POST `/v1/gov/ballots/plain`
  - Request: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notes: re-votes только на расширение - новый ballot не может уменьшить amount или expiry существующего lock. `owner` должен совпадать с authority транзакции. Минимальная длительность - `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Request: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "alice@wonderland?", "private_key": "...?" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - On-chain effect (current scaffold): enact утвержденного deploy proposal вставляет минимальный `ContractManifest`, привязанный к `code_hash`, с ожидаемым `abi_hash` и помечает proposal как Enacted. Если manifest уже существует для `code_hash` с другим `abi_hash`, enactment отклоняется.
  - Notes:
    - Для ZK elections контракты должны вызвать `ZK_VOTE_VERIFY_TALLY` до `FinalizeElection`; хосты enforce одноразовый latch. `FinalizeReferendum` отклоняет ZK-референдумы, пока tally выборов не финализирован.
    - Автозакрытие на `h_end` эмитит Approved/Rejected только для Plain-референдумов; ZK-референдумы остаются Closed, пока не будет отправлен финализированный tally и выполнен `FinalizeReferendum`.
    - Проверки turnout используют только approve+reject; abstain не учитывается в turnout.

- POST `/v1/gov/enact`
  - Request: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "alice@wonderland?", "private_key": "...?" }
  - Response: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notes: Torii отправляет подписанную транзакцию при наличии `authority`/`private_key`; иначе возвращает skeleton для подписи и отправки клиентом. Preimage опционален и сейчас носит информационный характер.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: proposal id hex (64 chars)
  - Response: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: referendum id string
  - Response: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - Response: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notes: возвращает persisted council при наличии; иначе деривирует детерминированный fallback, используя настроенный stake asset и thresholds (зеркалит спецификацию VRF до тех пор, пока VRF proofs не будут сохранены on-chain).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - Request: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Behavior: проверяет VRF proof каждого кандидата на каноническом input, полученном из `chain_id`, `epoch` и beacon последнего block hash; сортирует по output bytes desc с tiebreakers; возвращает top `committee_size` участников. Не сохраняется.
  - Response: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notes: Normal = pk в G1, proof в G2 (96 bytes). Small = pk в G2, proof в G1 (48 bytes). Inputs доменно разделены и включают `chain_id`.

### Governance defaults (iroha_config `gov.*`)

Fallback council, используемый Torii при отсутствии persisted roster, параметризуется через `iroha_config`:

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

`parliament_committee_size` ограничивает число fallback членов при отсутствии council, `parliament_term_blocks` задает длину эпохи для derivation seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` требует минимальный stake (в минимальных единицах) на eligibility asset, а `parliament_eligibility_asset_id` выбирает, какой баланс asset сканируется при построении набора кандидатов.

Governance VK verification без bypass: проверка ballots всегда требует `Active` verifying key с inline bytes, и окружения не должны полагаться на test toggles, чтобы пропускать проверку.

RBAC
- On-chain выполнение требует разрешений:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (future): `CanManageParliament`

Protected Namespaces
- Custom параметр `gov_protected_namespaces` (JSON array of strings) включает admission gating для deploys в перечисленные namespaces.
- Клиенты должны включать metadata keys транзакции для deploys, направленных в protected namespaces:
  - `gov_namespace`: целевой namespace (например, "apps")
  - `gov_contract_id`: логический contract id внутри namespace
- `gov_manifest_approvers`: опциональный JSON array account IDs валидаторов. Когда lane manifest задает quorum > 1, admission требует authority транзакции плюс перечисленные аккаунты для удовлетворения quorum манифеста.
- Телеметрия предоставляет admission counters через `governance_manifest_admission_total{result}`, чтобы операторы различали успешные admits от `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, и `runtime_hook_rejected`.
- Телеметрия показывает enforcement path через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`), позволяя операторам аудировать недостающие approvals.
- Lanes enforce namespace allowlist, опубликованный в manifests. Любая транзакция, устанавливающая `gov_namespace`, должна предоставить `gov_contract_id`, а namespace должен присутствовать в set `protected_namespaces` манифеста. `RegisterSmartContractCode` submissions без этой metadata отвергаются, когда защита включена.
- Admission требует, чтобы существовал Enacted governance proposal для tuple `(namespace, contract_id, code_hash, abi_hash)`; иначе валидация завершается ошибкой NotPermitted.

Runtime Upgrade Hooks
- Lane manifests могут объявлять `hooks.runtime_upgrade` для gate runtime upgrade инструкций (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Поля hook:
  - `allow` (bool, default `true`): когда `false`, все runtime-upgrade инструкции отклоняются.
  - `require_metadata` (bool, default `false`): требует entry metadata, указанную `metadata_key`.
  - `metadata_key` (string): имя metadata, enforced hook. Default `gov_upgrade_id`, когда metadata требуется или есть allowlist.
  - `allowed_ids` (array of strings): опциональный allowlist значений metadata (после trim). Отклоняет, когда предоставленное значение не входит в список.
- Когда hook присутствует, admission очереди enforce metadata политику до попадания транзакции в очередь. Отсутствующая metadata, пустые значения или значения вне allowlist дают детерминированный NotPermitted.
- Телеметрия отслеживает outcomes через `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, удовлетворяющие hook, должны включать metadata `gov_upgrade_id=<value>` (или ключ, определенный manifest) вместе с любыми validator approvals, требуемыми manifest quorum.

Convenience Endpoint
- POST `/v1/gov/protected-namespaces` - применяет `gov_protected_namespaces` напрямую на ноде.
  - Request: { "namespaces": ["apps", "system"] }
  - Response: { "ok": true, "applied": 1 }
  - Notes: предназначен для admin/testing; требует API token при конфигурации. В production предпочтительнее подписанная транзакция с `SetParameter(Custom)`.

CLI Helpers
- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - Получает контрактные инстансы для namespace и проверяет, что:
    - Torii хранит bytecode для каждого `code_hash`, и его Blake2b-32 digest соответствует `code_hash`.
    - Manifest под `/v1/contracts/code/{code_hash}` сообщает совпадающие `code_hash` и `abi_hash`.
    - Существует enacted governance proposal для `(namespace, contract_id, code_hash, abi_hash)`, деривированный тем же proposal-id hashing, что использует нода.
  - Выводит JSON отчет с `results[]` по каждому контракту (issues, manifest/code/proposal summaries) и однострочным summary, если не подавлено (`--no-summary`).
  - Полезно для аудита protected namespaces или проверки governance-controlled deploy workflows.
- `iroha gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - Выдает JSON skeleton metadata для деплоя в protected namespaces, включая опциональные `gov_manifest_approvers` для соблюдения quorum правил манифеста.
- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --nullifier-hex <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — lock hints обязательны при `min_bond_amount > 0`, и любой набор hints должен включать `owner`, `amount` и `duration_blocks`.
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier-hex` is validated against the proof when supplied.
  - Однострочный summary теперь содержит детерминированный `fingerprint=<hex>` из кодированного `CastZkBallot` вместе с декодированными hints (`owner`, `amount`, `duration_blocks`, `direction` при наличии).
  - CLI ответы аннотируют `tx_instructions[]` полем `payload_fingerprint_hex` и декодированными полями, чтобы downstream tooling могло проверить skeleton без повторной реализации Norito decoding.
  - Предоставление lock hints позволяет ноде эмитить `LockCreated`/`LockExtended` для ZK ballots, когда схема раскрывает те же значения.
- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Алиасы `--lock-amount`/`--lock-duration-blocks` отражают имена ZK флагов для parity в скриптах.
  - Summary вывод отражает `vote-zk`, включая fingerprint закодированной инструкции и читаемые ballot поля (`owner`, `amount`, `duration_blocks`, `direction`), давая быстрое подтверждение перед подписью skeleton.

Instances Listing
- GET `/v1/gov/instances/{ns}` - список активных контрактных инстансов для namespace.
  - Query params:
    - `contains`: фильтр по substring `contract_id` (case-sensitive)
    - `hash_prefix`: фильтр по hex prefix `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - Response: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK helper: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) или `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Unlock Sweep (Operator/Audit)
- GET `/v1/gov/unlocks/stats`
  - Response: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notes: `last_sweep_height` отражает последний block height, когда expired locks были swept и persist. `expired_locks_now` рассчитывается путем сканирования lock records с `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - Request (v1-style DTO):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint_hex": "...64hex?",
      "owner": "alice@wonderland?",
      "nullifier_hex": "...64hex?"
    }
  - Response: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - Принимает `BallotProof` JSON напрямую и возвращает skeleton `CastZkBallot`.
  - Request:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 контейнера ZK1 или H2*
        "root_hint": null,                // optional 32-byte array of bytes (eligibility root)
        "owner": null,                    // optional AccountId когда circuit фиксирует owner
        "nullifier": null                 // optional 32-byte array of bytes (nullifier hint)
      }
    }
  - Response:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notes:
    - Сервер маппит optional `root_hint`/`owner`/`nullifier` из ballot в `public_inputs_json` для `CastZkBallot`.
    - Envelope bytes повторно кодируются в base64 для payload инструкции.
    - `reason` меняется на `submitted transaction`, когда Torii отправляет ballot.
    - Этот endpoint доступен только при включенном feature `zk-ballot`.

CastZkBallot Verification Path
- `CastZkBallot` декодирует предоставленную base64 proof и отклоняет пустые/битые payloads (`BallotRejected` с `invalid or empty proof`).
- Хост разрешает ballot verifying key из referendum (`vk_ballot`) или governance defaults и требует, чтобы запись существовала, была `Active` и содержала inline bytes.
- Сохраненные verifying-key bytes повторно хешируются с `hash_vk`; любое несовпадение commitment останавливает выполнение до проверки для защиты от подмененных registry entries (`BallotRejected` с `verifying key commitment mismatch`).
- Proof bytes отправляются в зарегистрированный backend через `zk::verify_backend`; invalid transcripts приводят к `BallotRejected` с `invalid proof` и инструкция детерминированно падает.
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- Успешные proofs эмитят `BallotAccepted`; повторные nullifiers, устаревшие eligibility roots или регрессии lock продолжают давать существующие причины отказа, описанные ранее.

## Ненадлежащее поведение валидаторов и совместный консенсус

### Процесс slashing и jailing

Консенсус эмитит Norito-encoded `Evidence` при нарушениях протокола валидатором. Каждый payload попадает в in-memory `EvidenceStore` и, если он новый, материализуется в WSV-backed `consensus_evidence`. Записи старше `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` blocks) отклоняются, чтобы архив оставался ограниченным, но отказ логируется для операторов.

Распознанные нарушения отображаются один-к-одному на `EvidenceKind`; дискриминанты стабильны и принудительно зафиксированы в data model:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidCommitCertificate,
    EvidenceKind::InvalidProposal,
    EvidenceKind::DoubleExecVote,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - валидатор подписал конфликтующие хэши для одного `(phase,height,view,epoch)`.
- **DoubleExecVote** - конфликтные execution votes объявляют разные post-state roots.
- **InvalidCommitCertificate** - агрегатор разослал commit certificate, чья форма не проходит детерминированные проверки (например, пустой signer bitmap).
- **InvalidProposal** - лидер предложил блок, который проваливает структурную валидацию (например, нарушает locked-chain rule).

Операторы и инструменты могут просматривать и повторно рассылать payloads через:

- Torii: `GET /v1/sumeragi/evidence` и `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.

Governance должна рассматривать evidence bytes как каноническое доказательство:

1. **Собрать payload** до истечения срока. Архивировать сырые Norito bytes вместе с height/view metadata.
2. **Подготовить штраф** встроив payload в referendum или sudo instruction (например, `Unregister::peer`). Исполнение повторно валидирует payload; malformed или stale evidence отклоняется детерминированно.
3. **Запланировать follow-up топологию** чтобы нарушивший валидатор не смог сразу вернуться. Типовые потоки ставят `SetParameter(Sumeragi::NextMode)` и `SetParameter(Sumeragi::ModeActivationHeight)` с обновленным roster.
4. **Аудит результатов** через `/v1/sumeragi/evidence` и `/v1/sumeragi/status`, чтобы убедиться, что счетчик evidence вырос и governance выполнила удаление.

### Последовательность joint-consensus

Joint consensus гарантирует, что исходный набор валидаторов финализирует пограничный блок до того, как новый набор начнет предлагать. Runtime enforce правило через парные параметры:

- `SumeragiParameter::NextMode` и `SumeragiParameter::ModeActivationHeight` должны быть committed в **том же блоке**. `mode_activation_height` должен быть строго больше высоты блока, который внес обновление, обеспечивая минимум один блок лаг.
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) - это конфигурационный guard, который предотвращает hand-off без лагов:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Runtime и CLI показывают staged параметры через `/v1/sumeragi/params` и `iroha sumeragi params --summary`, чтобы операторы подтвердили activation heights и roster валидаторов.
- Автоматизация governance всегда должна:
  1. Финализировать решение об исключении (или восстановлении), поддержанное evidence.
  2. Поставить follow-up reconfiguration с `mode_activation_height = h_current + activation_lag_blocks`.
  3. Мониторить `/v1/sumeragi/status` до переключения `effective_consensus_mode` на ожидаемой высоте.

Любой скрипт, который ротирует валидаторов или применяет slashing, **не должен** пытаться zero-lag activation или опускать hand-off параметры; такие транзакции отклоняются и оставляют сеть в предыдущем режиме.

## Telemetry surfaces

- Prometheus метрики экспортируют активность governance:
  - `governance_proposals_status{status}` (gauge) отслеживает количество proposals по статусу.
  - `governance_protected_namespace_total{outcome}` (counter) увеличивается при allow/reject admission для protected namespaces.
  - `governance_manifest_activations_total{event}` (counter) фиксирует вставки manifest (`event="manifest_inserted"`) и namespace bindings (`event="instance_bound"`).
- `/status` включает объект `governance`, который зеркалит счетчики proposals, сообщает totals по protected namespaces и перечисляет недавние manifest activations (namespace, contract id, code/ABI hash, block height, activation timestamp). Операторы могут опрашивать это поле, чтобы убедиться, что enactments обновили manifests и что protected namespace gates соблюдаются.
- Grafana template (`docs/source/grafana_governance_constraints.json`) и telemetry runbook в `telemetry.md` показывают, как настроить оповещения о застрявших proposals, пропущенных manifest activations или неожиданных отказах protected namespaces во время runtime upgrades.
