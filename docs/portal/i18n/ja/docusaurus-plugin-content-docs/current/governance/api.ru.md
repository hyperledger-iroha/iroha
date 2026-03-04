---
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

重要: ガバナンスを維持します。 Формы могут меняться в ходе разработки. RBAC являются нормативными ограничениями 。 Torii может подписывать/отправлять транзакции при наличии `authority` и `private_key`, иначе клиенты `/transaction` を参照してください。

Обзор
- エンドポイントを JSON で保存します。 Сотоков, которые создают транзакции, ответы включают `tx_instructions` - массив одной или нескольких説明:
  - `wire_id`: реестровый идентификатор типа инструкции
  - `payload_hex`: ペイロード Norito (16 進数)
- `authority` と `private_key` の組み合わせ (`private_key` と DTO 投票)、Torii の組み合わせと отправляет транзакцию и все равно возвращает `tx_instructions`。
- SignedTransaction は、権限とchain_id、および POST と `/transaction` を関連付けます。
- SDK の例:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` と `GovernanceProposalResult` (ステータス/種類)、`ToriiClient.get_governance_referendum_typed` と`GovernanceReferendumResult`、`ToriiClient.get_governance_tally_typed` と `GovernanceTally`、`ToriiClient.get_governance_locks_typed` と `GovernanceLocksResult`、`ToriiClient.get_governance_unlock_stats_typed` と`GovernanceUnlockStats`、`ToriiClient.list_governance_instances_typed` は `GovernanceInstancesPage`、ガバナンスを維持します。 README を参照してください。
- Python クラス (`iroha_torii_client`): `ToriiClient.finalize_referendum` および `ToriiClient.enact_proposal` バンドル `GovernanceInstructionDraft` (оборачивают Torii スケルトン `tx_instructions`)、JSON-парсинга при сборке フローを終了/適用します。
- JavaScript (`@iroha/iroha-js`): `ToriiClient` 提案、住民投票、集計、ロック、統計のロック解除などのヘルパー `listGovernanceInstances(namespace, options)` 市議会エンドポイント(`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、`governancePersistCouncil`、`getGovernanceCouncilAudit`)、Node.js での `/v1/gov/instances/{ns}` およびVRF ベースのワークフローを利用できるようになります。

エンドポイント

- POST `/v1/gov/proposals/deploy-contract`
  - リクエスト (JSON):
    {
      "名前空間": "アプリ",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "窓": { "下": 12345, "上": 12400 },
      "権限": "ih58…?",
      "秘密キー": "...?"
    }
  - 応答 (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 検証: `abi_hash` と заданного `abi_version` および отвергают несовпадения。 `abi_version = "v1"` ожидаемое значение - `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`。

契約 API (デプロイ)
- POST `/v1/contracts/deploy`
  - リクエスト: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - 動作: `code_hash` 、 IVM 、 `abi_hash` 、 заголовку `abi_version`、затем `RegisterSmartContractCode` (マニフェスト) および `RegisterSmartContractBytes` (`.to` および `authority`)。
  - 応答: { "ok": true、"code_hash_hex": "..."、"abi_hash_hex": "..." }
  - 関連:
    - GET `/v1/contracts/code/{code_hash}` -> マニフェストを取得します
    - `/v1/contracts/code-bytes/{code_hash}` を取得 -> `{ code_b64 }` を取得します
- POST `/v1/contracts/instance`
  - リクエスト: { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - 動作: バイトコードと `(namespace, contract_id)` через `ActivateContractInstance`。
  - 応答: { "ok": true、"namespace": "apps"、"contract_id": "calc.v1"、"code_hash_hex": "..."、"abi_hash_hex": "..." }エイリアスサービス
- POST `/v1/aliases/voprf/evaluate`
  - リクエスト: { "blinded_element_hex": "..." }
  - 応答: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценщика。バージョン: `blake2b512-mock`。
  - 注: детерминированный モック оценщик、применяющий Blake2b512 с ドメイン分離 `iroha.alias.voprf.mock.v1`。ツール、プロダクション VOPRF パイプライン、Iroha を参照してください。
  - エラー: HTTP `400` は 16 進数です。 Torii は、Norito エンベロープ `ValidationFail::QueryFailed::Conversion` を受信します。
- POST `/v1/aliases/resolve`
  - リクエスト: { "エイリアス": "GB82 WEST 1234 5698 7654 32" }
  - 応答: { "エイリアス": "GB82WEST12345698765432"、"アカウントID": "ih58..."、"インデックス": 0、"ソース": "iso_bridge" }
  - 注: ISO ブリッジ ランタイム ステージング (`[iso_bridge.account_aliases]` と `iroha_config`)。 Torii нормализует エイリアス、удаляя пробелы и приводя к верхнему регистру。エイリアス 404 と 503、ISO ブリッジ ランタイムのバージョン。
- POST `/v1/aliases/resolve_index`
  - リクエスト: { "インデックス": 0 }
  - 応答: { "index": 0、"alias": "GB82WEST12345698765432"、"account_id": "ih58..."、"source": "iso_bridge" }
  - 注: индексы エイリアス назначаются детерминированно по порядку конфигурации (0 ベース)。エイリアスを使用して、オフラインで監査証跡を取得できます。

コードサイズの上限
- カスタム形式: `max_contract_code_bytes` (JSON u64)
  - Управляет максимальным допустимым размером (в байтах) オンチェーン хранения кода контрактов。
  - デフォルト: 16 MiB。 `RegisterSmartContractBytes`、`.to` が不変条件違反です。
  - `SetParameter(Custom)` と `id = "max_contract_code_bytes"` および числовым ペイロードを受け取ります。

- POST `/v1/gov/ballots/zk`
  - リクエスト: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注:
    - `owner`、`amount`、`duration_blocks`、および証拠を確認してください。 VK では、ガバナンス ロック `election_id` と `owner` がサポートされています。 Направление скрыто (`unknown`); обновляются только 金額/有効期限。例: 金額と有効期限 только увеличиваются (нода применяет max(amount, prev.amount) и max(expiry, prev.expiry))。
    - ZK 再投票、金額と有効期限、отклоняются сервером с диагностикой `BallotRejected`。
    - Исполнение контракта должно вызвать `ZK_VOTE_VERIFY_BALLOT` до постановки `SubmitBallot`; хосты は、одноразовый ラッチを強制します。

- POST `/v1/gov/ballots/plain`
  - リクエスト: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注: 再投票は、ロックを解除するために行われます。 - 投票用紙は、金額と有効期限がロックされます。 `owner` должен совпадать с 権限 транзакции. Минимальная длительность - `conviction_step_blocks`。

- POST `/v1/gov/finalize`
  - リクエスト: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58...?", "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - オンチェーン効果 (現在のスキャフォールド): утвержденного のデプロイ提案を制定する вставляет минимальный `ContractManifest`, привязанный к `code_hash`, с ожидаемым `abi_hash` и помечает 提案 как 制定されました。マニフェストは `code_hash` と `abi_hash`、制定は отклоняется です。
  - 注:
    - ZK 選挙 контракты должны вызвать `ZK_VOTE_VERIFY_TALLY` до `FinalizeElection`; хосты は、одноразовый ラッチを強制します。 `FinalizeReferendum` отклоняет ZK-референдумы、пока tally выборов не финализирован.
    - Автозакрытие на `h_end` эмитит 承認/拒否 только для Plain-референдумов; ZK-референдумы остаются 閉業しました。 Єинализированный tally и выполнен `FinalizeReferendum`。
    - 投票率 используют только 承認+拒否;投票率を棄権する。- POST `/v1/gov/enact`
  - リクエスト: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { " lower": 0, "upper": 0 }?、 "authority": "ih58…?"、 "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注: Torii отправляет подписанную транзакцию при наличии `authority`/`private_key`;スケルトンがあれば、それは отправки клиентом です。プリ画像 опционален и сейчас носит информационный характер.

- `/v1/gov/proposals/{id}` を取得
  - パス `{id}`: プロポーザル ID 16 進数 (64 文字)
  - 応答: { "見つかった": bool、"提案": { ... }? }

- `/v1/gov/locks/{rid}` を取得
  - パス `{rid}`: 国民投票 ID 文字列
  - 応答: { "found": bool、"referendum_id": "rid"、"locks": { ... }? }

- `/v1/gov/council/current` を取得
  - 応答: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 注: 永続的な評議会の記録。フォールバック、ステーク資産、しきい値 (VRF と VRF、VRF のしきい値)証明はオンチェーン上で行われます)。

- POST `/v1/gov/council/derive-vrf` (機能: gov_vrf)
  - リクエスト: { "committee_size": 21、"epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 動作: VRF プルーフ каждого кандидата на каноническом 入力、полученном из `chain_id`、`epoch` およびビーコン ブロックハッシュ;出力バイトの説明とタイブレーカー。トップ `committee_size` участников。 Не сохраняется。
  - 応答: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - 注: 通常 = pk × G1、proof × G2 (96 バイト)。 Small = pk × G2、proof × G1 (48 バイト)。 доменно разделены и включают `chain_id` を入力します。

### ガバナンスのデフォルト (iroha_config `gov.*`)

フォールバック評議会、используемый Torii は永続名簿、параметризуется через `iroha_config`:

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

Эквивалентные окружения:

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

`parliament_committee_size` ограничивает число フォールバック членов при отсутствии Council、`parliament_term_blocks` задает длину эпохи для 派生シード(`epoch = floor(height / term_blocks)`)、`parliament_min_stake` требует минимальный stake (в минимальных единицах) на 適格資産、`parliament_eligibility_asset_id` выбирает、какой資産はすべての資産に含まれています。

ガバナンス VK 検証のバイパス: 投票用紙の確認 `Active` キーの検証、インライン バイト、テスト トグル、テストの切り替え、чтобыそうです。

RBAC
- オンチェーンのメッセージ:
  - 提案: `CanProposeContractDeployment{ contract_id }`
  - 投票用紙: `CanSubmitGovernanceBallot{ referendum_id }`
  - 制定: `CanEnactGovernance`
  - 評議会運営（将来）：`CanManageParliament`

保護された名前空間
- カスタム `gov_protected_namespaces` (文字列の JSON 配列) およびアドミッション ゲートは、名前空間をデプロイします。
- メタデータ キーのデプロイ、保護された名前空間の定義:
  - `gov_namespace`: целевой 名前空間 (например、「アプリ」)
  - `gov_contract_id`: логический 契約 ID внутри 名前空間
- `gov_manifest_approvers`: JSON 配列アカウント ID です。レーン マニフェスト задает quorum > 1、入場 требует authority транзакции плюс перечисленные аккаунты для удовлетворения quorum манифеста.
- 入場カウンターは `governance_manifest_admission_total{result}`、`missing_manifest` を許可します。 `non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected`、および `runtime_hook_rejected`。
- 施行パス через `governance_manifest_quorum_total{outcome}` (`satisfied` / `rejected`) を参照してください。承認。
- レーンは、名前空間の許可リスト、マニフェストを適用します。名前空間 `gov_namespace`、名前空間 `gov_contract_id`、名前空間`protected_namespaces` を設定します。 `RegisterSmartContractCode` の提出物は、メタデータ отвергаются、когда защита включена を含みます。
- 入場料、タプル `(namespace, contract_id, code_hash, abi_hash)` の制定されたガバナンス提案。許可されていません。ランタイムアップグレードフック
- レーン マニフェスト `hooks.runtime_upgrade` ゲート ランタイム アップグレード (`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`)。
- フック:
  - `allow` (ブール値、デフォルト `true`): `false`、ランタイム アップグレードを実行します。
  - `require_metadata` (ブール値、デフォルト `false`): エントリ メタデータ、указанную `metadata_key`。
  - `metadata_key` (文字列): メタデータ、強制フック。デフォルトの `gov_upgrade_id`、メタデータの許可リスト。
  - `allowed_ids` (文字列の配列): 許可リストのメタデータ (トリム)。 Отклоняет、когда предоставленное значение не входит в список.
- フックはフック、入場はメタデータを強制します。メタデータ、許可リスト、NotPermitted を参照してください。
- 結果は `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` です。
- フック、メタデータ `gov_upgrade_id=<value>` (マニフェスト、マニフェスト) のフック、検証ツール承認、マニフェスト定足数。

コンビニエンスエンドポイント
- POST `/v1/gov/protected-namespaces` - применяет `gov_protected_namespaces` напрямую на ноде.
  - リクエスト: { "名前空間": ["アプリ", "システム"] }
  - 応答: { "ok": true、"applied": 1 }
  - 注: 管理者/テスト中。 API トークンが必要です。生産は `SetParameter(Custom)` です。

CLI ヘルパー
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Получает контрактные инстансы для 名前空間とファイル、что:
    - Torii バイトコード каждого `code_hash`、Blake2b-32 ダイジェスト соответствует `code_hash`。
    - マニフェストは `/v1/contracts/code/{code_hash}` と `code_hash` および `abi_hash` です。
    - `(namespace, contract_id, code_hash, abi_hash)` でガバナンス提案が制定され、提案 ID ハッシュが適用されます。
  - Выводит JSON отчет с `results[]` по каждому контракту (問題、マニフェスト/コード/提案概要) および однострочным 概要、если не подавлено (`--no-summary`)。
  - 保護された名前空間とガバナンス制御されたデプロイ ワークフローをサポートします。
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - JSON スケルトン メタデータと保護された名前空間、`gov_manifest_approvers` とクォーラムの組み合わせを確認します。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — ロックヒント `min_bond_amount > 0`、およびロックヒント должен включать `owner`、`amount` および`duration_blocks`。
  - 正規アカウント ID を検証し、32 バイトの nullifier ヒントを正規化し、ヒントを `public_inputs_json` (追加のオーバーライド用に `--public <path>`) にマージします。
  - 無効化子は、証明コミットメント (パブリック入力) に `domain_tag`、`chain_id`、および `election_id` を加えたものから導出されます。 `--nullifier` は、提供されたときに証明に対して検証されます。
  - 概要 теперь содержит детерминированный `fingerprint=<hex>` из кодированного `CastZkBallot` вместе с декодированными ヒント (`owner`、`amount`、`duration_blocks`、`direction` при наличии)。
  - CLI による `tx_instructions[]` および `payload_fingerprint_hex` および декодированными полями、ダウンストリーム ツールのスケルトン ビューNorito デコード中です。
  - ロックのヒントを表示 `LockCreated`/`LockExtended` для ZK 投票、когда схема раскрывает те же значения。
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Алиасы `--lock-amount`/`--lock-duration-blocks` отражают имена ZK флагов для parity в скриптах.
  - 概要 вывод отражает `vote --mode zk`、指紋認証、投票用紙 (`owner`、 `amount`、`duration_blocks`、`direction`)、давая быстрое подтверждение перед подписью スケルトン。

インスタンスのリスト
- GET `/v1/gov/instances/{ns}` - 名前空間を取得します。
  - クエリパラメータ:
    - `contains`: 部分文字列 `contract_id` (大文字と小文字が区別されます)
    - `hash_prefix`: фильтр по 16 進プレフィックス `code_hash_hex` (小文字)
    - `offset` (デフォルト 0)、`limit` (デフォルト 100、最大 10_000)
    - `order`: `cid_asc` (デフォルト)、`cid_desc`、`hash_asc`、`hash_desc`
  - 応答: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK ヘルパー: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) または `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。ロック解除スイープ (オペレーター/監査)
- `/v1/gov/unlocks/stats` を取得
  - 応答: { "height_current": H、"expired_locks_now": n、"referenda_with_expired": m、"last_スイープ_高さ": S }
  - 注: `last_sweep_height` ブロックの高さ、期限切れのロックがスイープされ、永続化されます。 `expired_locks_now` ロック レコード с `expiry_height <= height_current`。
- POST `/v1/gov/ballots/zk-v1`
  - リクエスト (v1 スタイル DTO):
    {
      "権限": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      "バックエンド": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "オーナー": "ih58…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (機能: `zk-ballot`)
  - `BallotProof` JSON とスケルトン `CastZkBallot`。
  - リクエスト:
    {
      "権限": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      「投票用紙」: {
        "バックエンド": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // Base64 контейнера ZK1 または H2*
        "root_hint": null, // オプションの 32 バイトの 16 進文字列 (資格ルート)
        "owner": null, // オプションの AccountId когда circui фиксирует owner
        "nullifier": null // オプションの 32 バイトの 16 進文字列 (nullifier ヒント)
      }
    }
  - 応答:
    {
      「わかりました」: 本当、
      「受け入れられました」: true、
      "reason": "トランザクション スケルトンを構築する",
      "tx_instructions": [
        { "wire_id": "CastZkBallot"、"payload_hex": "..." }
      】
    }
  - 注:
    - オプションの `root_hint`/`owner`/`nullifier` または `public_inputs_json` と `CastZkBallot` の投票。
    - エンベロープ バイトは、base64 ペイロードと кодируются を組み合わせます。
    - `reason` меняется на `submitted transaction`、когда Torii отправляет 投票。
    - エンドポイントは機能 `zk-ballot` をサポートします。

CastZkBallot 検証パス
- `CastZkBallot` は、base64 証明およびペイロード (`BallotRejected` と `invalid or empty proof`) をサポートします。
- 投票用紙検証キーと国民投票 (`vk_ballot`) とガバナンスのデフォルト、および требует、чтобы запись существовала、была `Active` およびインラインバイトです。
- キー バイトの検証を行っています。 `hash_vk`;コミットメント останавливает выполнение до проверки для защиты от подмененных レジストリ エントリ (`BallotRejected` с) `verifying key commitment mismatch`)。
- 証明バイト отправляются в зарегистрированный バックエンド через `zk::verify_backend`;無効なトランスクリプト к `BallotRejected` с `invalid proof` инструкция детерминированно падает。
- 証明では、投票コミットメントと資格ルートを公開入力として公開する必要があります。ルートは選挙の `eligible_root` と一致する必要があり、派生した無効化子は提供されたヒントと一致する必要があります。
- 証明 эмитят `BallotAccepted`;無効化子、適格性ルート、ロックのロックを解除する必要があります。

## Ненадлежащее поведение валидаторов и совместный консенсус

### 斬撃と投獄

Консенсус эмитит Norito でエンコードされた `Evidence` が表示されます。ペイロードは、メモリ内の `EvidenceStore` と、WSV でバックアップされた `consensus_evidence` の両方に対応しています。 `sumeragi.npos.reconfig.evidence_horizon_blocks` (デフォルトの `7200` ブロック) отклоняются、чтобы архив оставался ограниченным、но отказ Погируется для операторов.範囲内の証拠は、`sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) およびスラッシュ遅延 `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) も考慮します。ガバナンスは、スラッシュが適用される前に `CancelConsensusEvidencePenalty` でペナルティをキャンセルできます。

Распознанные нарузения отображаются один-к-одному на `EvidenceKind`;データ モデルの詳細:

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

- **DoublePrepare/DoubleCommit** - `(phase,height,view,epoch)` を実行します。
- **InvalidQc** - コミット証明書、メッセージ、メッセージ、メッセージ (署名者ビットマップ)。
- **InvalidProposal** - лидер предложил блок、который проваливает структурную валидацию (ロック チェーン ルール)。
- **検閲** - 署名された提出受領書には、決して提案/コミットされていないトランザクションが示されています。ペイロードの説明:

- Torii: `GET /v1/sumeragi/evidence` または `GET /v1/sumeragi/evidence/count`。
- CLI: `iroha ops sumeragi evidence list`、`... count`、`... submit --evidence-hex <payload>`。

ガバナンス証拠バイト数:

1. **ペイロード** が表示されます。高さ/ビューのメタデータを取得するには、Norito バイトを使用します。
2. **ペイロードと住民投票、および sudo 命令 (например、`Unregister::peer`)。ペイロードを確認します。不正な形式と古い証拠 отклоняется детерминированно。
3. **Запланировать のフォローアップ топологию** を確認してください。 `SetParameter(Sumeragi::NextMode)` と `SetParameter(Sumeragi::ModeActivationHeight)` の名簿を確認してください。
4. **Аудит результатов** через `/v1/sumeragi/evidence` и `/v1/sumeragi/status`, чтобы убедиться, что счетчик 証拠とガバナンスだよ。

### 共同合意

共同合意 гарантирует, что исходный набор валидаторов финализирует пограничный блок до того, как новый набор начнетジャンク。ランタイムは次のことを強制します:

- `SumeragiParameter::NextMode` および `SumeragiParameter::ModeActivationHeight` は **том же блоке** によってコミットされました。 `mode_activation_height` должен быть строго бользе высоты блока, который внес обновление, обеспечивая минимум один блок лаг.
- `sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) - ハンドオフのメッセージ:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) はコンセンサス スラッシュを遅らせ、ペナルティが適用される前にガバナンスがキャンセルできるようにします。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- ランタイム、CLI、ステージング、`/v1/sumeragi/params`、`iroha --output-format text ops sumeragi params`、アクティベーションの高さと名簿。
- ガバナンスの概要:
  1. Финализировать реб исключении (или восстановлении)、証拠。
  2. フォローアップ再構成 с `mode_activation_height = h_current + activation_lag_blocks`。
  3. Мониторить `/v1/sumeragi/status` до переключения `effective_consensus_mode` на ожидаемой высоте.

スラッシュ、** должен** のゼロラグ アクティベーション、ハンドオフの機能。 такие транзакции отклоняются и оставляют сеть в предыдущем режиме.

## テレメトリ サーフェス

- Prometheus メソッド ガバナンス:
  - `governance_proposals_status{status}` (ゲージ) отслеживает количество 提案 по статусу。
  - `governance_protected_namespace_total{outcome}` (カウンター) 保護された名前空間の許可/拒否を指定します。
  - `governance_manifest_activations_total{event}` (カウンター) マニフェスト (`event="manifest_inserted"`) および名前空間バインディング (`event="instance_bound"`)。
- `/status` は、`governance`、保護された名前空間と合計の合計を表示します。マニフェストのアクティベーション (名前空間、コントラクト ID、コード/ABI ハッシュ、ブロックの高さ、アクティベーション タイムスタンプ)。 Операторы могут опразивать это поле、чтобы убедиться、что 制定 обновили マニフェスト、что 保護された名前空間ゲート соблюдаются。
- Grafana テンプレート (`docs/source/grafana_governance_constraints.json`) とテレメトリ Runbook と `telemetry.md` の提案、マニフェストのアクティベーションと保護された名前空間、およびランタイムのアップグレードを実行します。