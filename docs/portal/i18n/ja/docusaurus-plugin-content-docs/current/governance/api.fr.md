---
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ステータス: ブルイヨン/エスキスは、統治の実装を伴ったものを注ぎます。プヴァンチェンジャーペンダントの実装。 RBAC の規範の決定と政治の制約。 Torii 署名者/取引総額 `authority` および `private_key` は、`/transaction` を構成するクライアントの署名者です。

アペルク
- JSON のエンドポイントを使用します。 `tx_instructions` を含むトランザクションの生産性と応答性を注ぐ - 指示の一覧表:
  - `wire_id`: 命令の種類を登録する識別子
  - `payload_hex`: ペイロード Norito (16 進数) のバイト数
- Si `authority` と `private_key` は 4 つあります (投票用紙 DTO に関する `private_key` を参照)、Torii は、トランザクションとレンボイのクアンド ミーム `tx_instructions` に署名します。
- シノン、クライアントが SignedTransaction の権限とchain_id、署名、POST 対 `/transaction` を組み立てています。
- クーベルチュール SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` renvoie `GovernanceProposalResult` (レ シャンのステータス/種類を正規化)、`ToriiClient.get_governance_referendum_typed` renvoie `GovernanceReferendumResult`、`ToriiClient.get_governance_tally_typed` renvoie `GovernanceTally`、`ToriiClient.get_governance_locks_typed` renvoie `GovernanceLocksResult`、`ToriiClient.get_governance_unlock_stats_typed` renvoie `GovernanceUnlockStats`、et `ToriiClient.list_governance_instances_typed` renvoie `GovernanceInstancesPage`、重要なアクセス権タイプREADME の使用例を確認してください。
- クライアント Python レジェ (`iroha_torii_client`): `ToriiClient.finalize_referendum` et `ToriiClient.enact_proposal` バンドル タイプ `GovernanceInstructionDraft` (カプセル化ファイル スケレット `tx_instructions` デ Torii)、明らかJSON の解析マニュアルとスクリプトの構成要素を最終化/実行します。
- JavaScript (`@iroha/iroha-js`): `ToriiClient` は、提案、参照、集計、ロック、統計のロック解除、メンテナンスなどのヘルパー タイプを公開します。 `listGovernanceInstances(namespace, options)` およびエンドポイント評議会 (`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、 `governancePersistCouncil`、`getGovernanceCouncilAudit`) クライアント Node.js のページ作成者 `/v1/gov/instances/{ns}` と、存在するインスタンスのリストを並行してワークフロー VRF でパイロットします。

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
      "権限": "i105…?",
      "秘密キー": "...?"
    }
  - 応答 (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 検証: 正規表現 `abi_hash` は、`abi_version` 4 つと一貫性のないものです。 `abi_version = "v1"` を注ぎ、`hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` に参加してください。

API コントラット (デプロイ)
- POST `/v1/contracts/deploy`
  - リクエスト: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - 構成: calcule `code_hash` depuis le corps du Program IVM et `abi_hash` depuis l'en-tete `abi_version`、puis soumet `RegisterSmartContractCode` (manifeste) et `RegisterSmartContractBytes` (バイト `.to` が完了) `authority` を注ぎます。
  - 応答: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - 嘘をつく：
    - GET `/v1/contracts/code/{code_hash}` -> マニフェスト ストックを発行
    - GET `/v1/contracts/code-bytes/{code_hash}` -> レンボイ `{ code_b64 }`
- POST `/v1/contracts/instance`
  - リクエスト: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - コンポーネント: `ActivateContractInstance` 経由で `(namespace, contract_id)` をマッピングするバイトコード 4 つのアクティブな即時ファイルを展開します。
  - 応答: { "ok": true、"namespace": "apps"、"contract_id": "calc.v1"、"code_hash_hex": "..."、"abi_hash_hex": "..." }サービスダリアス
- POST `/v1/aliases/voprf/evaluate`
  - リクエスト: { "blinded_element_hex": "..." }
  - 応答: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` 評価の実装を反映します。有効値: `blake2b512-mock`。
  - 注: アップリケの模擬決定を評価する Blake2b512 のドメイン `iroha.alias.voprf.mock.v1` の分離。前に、Iroha を使用してパイプライン VOPRF をテストする必要があります。
  - エラー: HTTP `400` 入力 16 進数形式。 Torii 封筒を受け取りました Norito `ValidationFail::QueryFailed::Conversion` デコーダーのメッセージエラー。
- POST `/v1/aliases/resolve`
  - リクエスト: { "エイリアス": "GB82 WEST 1234 5698 7654 32" }
  - 応答: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - 注: ランタイム ISO ブリッジ ステージング (`[iso_bridge.account_aliases]` および `iroha_config`) が必要です。 Torii は、前衛的なルックアップでのエイリアスとリタイラント ファイルのスペースなどの情報を正規化します。戻り値 404 ファイルのエイリアスが存在せず、503 ファイルのランタイム ISO ブリッジが無効になっています。
- POST `/v1/aliases/resolve_index`
  - リクエスト: { "インデックス": 0 }
  - 応答: { "index": 0、"alias": "GB82WEST12345698765432"、"account_id": "i105..."、"source": "iso_bridge" }
  - 注: 別名インデックスのインデックスは、構成の決定順序を割り当てます (0 ベース)。 Les client peuvent metre en cache hors ligne pour construire des pistes d'audit pour les Evenements d'attestation d'alias。

キャップ・ド・タイユ・ド・コード
- パラメータカスタム: `max_contract_code_bytes` (JSON u64)
  - オンチェーンの在庫とコードを制御する最大自動制御 (バイト)。
  - デフォルト: 16 MiB。 Les noeuds rejettent `RegisterSmartContractBytes` lorsque la taille de l'image `.to` depasse le cap avec une erreur d'invariant。
  - `SetParameter(Custom)` avec `id = "max_contract_code_bytes"` およびペイロード数値を介して調整者を操作します。

- POST `/v1/gov/ballots/zk`
  - リクエスト: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注:
    - `owner`、`amount` および `duration_blocks` を含む回路入力の入力は、VK 構成に対して事前検証され、`election_id` avec を監視します。 ce `owner`。 LA方向レストキャッシュ(`unknown`);数量/有効期限は一週間を逃すことはありません。モノトーンの再投票: フォントの量と有効期限 (アップリケの最大値(金額, 前の金額) と最大(有効期限, 前の有効期限))。
    - 有効期限切れの診断結果 `BallotRejected` を拒否するために、ZK は再投票します。
    - 異議申し立ての実行 `ZK_VOTE_VERIFY_BALLOT` 前処理 `SubmitBallot`;ホストは、安全なロックを解除する必要があります。

- POST `/v1/gov/ballots/plain`
  - リクエスト: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注: 再投票は延長期間であり、新しい投票は期限が切れます。 Le `owner` はトランザクションの権限を持っています。ラデュレ ミニマル エスト `conviction_step_blocks`。- POST `/v1/gov/finalize`
  - リクエスト: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105...?", "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - オンチェーンでの効果 (足場の実際): 制定者は、提案を展開する承認を承認し、`ContractManifest` 最小限の権限を与えられます。宣言が存在するということは、`code_hash` と `abi_hash` は異なるということです。制定は拒否されます。
  - 注:
    - 選挙 ZK を注ぎ、反対意見を表明する人 `ZK_VOTE_VERIFY_TALLY` 前執行者 `FinalizeElection`;ホストは、固有の使用法をアンラッチする必要があります。 `FinalizeReferendum` 国民投票を拒否する ZK は、最終的な結果を集計する必要があります。
    - La cloture automatique a `h_end` emet 承認/拒否されたユニークメント プール レファレンダム プレーン。 ZK の住民投票は終了し、結果を最終決定し、`FinalizeReferendum` を実行します。
    - 承認+拒否を利用した投票率の検証。投票率を棄権してください。

- POST `/v1/gov/enact`
  - リクエスト: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { " lower": 0, "upper": 0 }?, "authority": "i105...?", "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注: Torii トランザクション署名者 quand `authority`/`private_key` ソント フォーニス。 sinon il renvoie un squelette は、署名と依頼クライアントを注ぎます。事前画像オプションと情報を瞬時に提供します。

- `/v1/gov/proposals/{id}` を取得
  - パス `{id}`: ID 提案 16 進数 (64 文字)
  - 応答: { "見つかった": bool、"提案": { ... }? }

- `/v1/gov/locks/{rid}` を取得
  - パス `{rid}`: 住民投票の文字列 ID
  - 応答: { "found": bool、"referendum_id": "rid"、"locks": { ... }? }

- `/v1/gov/council/current` を取得
  - 応答: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 注: renvoie le Councilpersiste si present;シノンはフォールバックを決定し、ステークの資産構成と設定を決定します (VRF のスペックを確認するための VRF とオンチェーンの直接の永続化のミロワール)。

- POST `/v1/gov/council/derive-vrf` (機能: gov_vrf)
  - リクエスト: { "committee_size": 21、"epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 構成: `chain_id`、`epoch` およびブロックのビーコン ハッシュの入力を比較して、事前 VRF の候補を検証します。タイブレーカーの平均バイト数を試行します。 renvoie les トップ `committee_size` メンバー。ネ・パーシステ・パス。
  - 応答: { "epoch": N、"members": [{ "account_id": "..." } ...]、"total_candidates": M、"verified": K }
  - 注: 通常 = pk en G1、proof en G2 (96 バイト)。 Small = pk en G2、proof en G1 (48 バイト)。ドメインなどの入力は `chain_id` を含めて分離されません。

### デフォルトの統治 (iroha_config `gov.*`)

Le Council フォールバックは、Torii Quand aucun rosterpersiste n'existe est パラメータを `iroha_config` 経由で利用します。

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

環境に相当するものをオーバーライドします。

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

`parliament_committee_size` メンバーのフォールバック制限および議会評議会の永続的な制限、`parliament_term_blocks` シードの導出 (`epoch = floor(height / term_blocks)`)、`parliament_min_stake` 最小ステークの課し(en Unites minimumes) sur l'asset d'eligibite, et `parliament_eligibility_asset_id` selectne quel solde d'asset est scanne lors de la construction du set de candidats.

検証 VK はバイパスなし: 投票要求の検証は `Active` 平均バイト インラインで、環境は検証をテストするためのトグルです。

RBAC
- オンチェーンでの実行には権限が必要です:
  - 提案: `CanProposeContractDeployment{ contract_id }`
  - 投票用紙: `CanSubmitGovernanceBallot{ referendum_id }`
  - 制定: `CanEnactGovernance`
  - 評議会運営（将来）：`CanManageParliament`名前空間の保護者
- カスタム `gov_protected_namespaces` (文字列の JSON 配列) パラメータは、ネームスペース リストを使用してアクティブな入場許可をデプロイします。
- クライアントは、トランザクション ファイルのデプロイと名前空間の保護のメタデータを含めることができません。
  - `gov_namespace`: ファイル名前空間 cible (例: "apps")
  - `gov_contract_id`: 名前空間の制御ロジックの ID
- `gov_manifest_approvers`: 検証用アカウント ID の JSON 配列オプション。マニフェストの宣言は定足数 > 1 を宣言し、トランザクションの権限とマニフェストの満足度を満たすリストの計算リストを要求します。
- `missing_manifest`、`non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected` などのオペレーターの区別のための遠隔測定による入院の公開`runtime_hook_rejected`。
- テレメトリは、`governance_manifest_quorum_total{outcome}` (値 `satisfied` / `rejected`) を介して法執行を公開し、監査人による承認マンカンテスを注ぎます。
- ルールのマニフェストを公開するネームスペースの許可リストを適用します。トランザクションの修正 `gov_namespace` は、`gov_contract_id` を実行し、名前空間は、マニフェストの設定 `protected_namespaces` を実行します。 Les soumissions `RegisterSmartContractCode` sans cette metadata sont rejetees lorsque la protection est active.
- ガバナンスの承認を課すために、タプル `(namespace, contract_id, code_hash, abi_hash)` が存在します。検証エコーのエラーが許可されていません。

ランタイムアップグレードのフック
- ランタイム アップグレードの説明書 `hooks.runtime_upgrade` のマニフェスト (`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`)。
- シャン・デュ・フック:
  - `allow` (ブール値、デフォルト `true`): Quand `false`、拒否されるランタイム アップグレードの手順を示します。
  - `require_metadata` (ブール値、デフォルト `false`): `metadata_key` で指定されたエントリ メタデータを指定します。
  - `metadata_key` (文字列): フックの名目メタデータ アップリケ。デフォルトの `gov_upgrade_id` は、メタデータを許可リストに含める必要があります。
  - `allowed_ids` (文字列の配列): メタデータのオプションの許可リスト (トリミング後)。 Rejette quand la valeur fournie n'est pas listee.
- 現在のフック、ファイル アップリケの入場、政治のメタデータ、ファイルのトランザクションの入り口を確認します。メタデータは適切に管理されており、許可リストに含まれるエラーが許可されていないことが決定されています。
- `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` 経由のテレメトリ トレース結果。
- メタデータ `gov_upgrade_id=<value>` (マニフェストの定義) とマニフェストの承認と検証の要件を含むトランザクションは満たされています。

コモディティのエンドポイント
- POST `/v1/gov/protected-namespaces` - アップリケ `gov_protected_namespaces` の方向性。
  - リクエスト: { "名前空間": ["アプリ", "システム"] }
  - 応答: { "ok": true、"applied": 1 }
  - 注: 管理者/テストの運命。トークン API の構成が必要です。生産を行っており、トランザクション署名者 avec `SetParameter(Custom)` を優先します。ヘルパー CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 名前空間を制御するインスタンスを取得し、確認します:
    - Torii ストック バイトコード プール チャク `code_hash`、およびダイジェスト Blake2b-32 は、au `code_hash` に対応します。
    - Le Manifeste Stocke sous `/v1/contracts/code/{code_hash}` rapporte des valeurs `code_hash` および `abi_hash` 対応者。
    - 統治計画は、`(namespace, contract_id, code_hash, abi_hash)` を使用して提案 ID のミーム ハッシュを取得して存在するように制定されています。
  - JSON の avec `results[]` との関係 (問題、マニフェスト/コード/提案の再開) と、抑制の再開 (`--no-summary`) を並べ替えます。
  - 監査者は名前空間を保護し、検証者はワークフローを構築し、管理者による制御を展開します。
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - JSON のメタデータを設定するには、デプロイメントと名前空間の保護者、`gov_manifest_approvers` オプションを含めて、マニフェストの定足数を満たす必要があります。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` を必要とするロックのヒント、および `owner`、`amount` および `duration_blocks` を含む 4 つのアンサンブルのヒント。
  - 正規アカウント ID を検証し、32 バイトの nullifier ヒントを正規化し、ヒントを `public_inputs_json` (追加のオーバーライド用に `--public <path>`) にマージします。
  - 無効化子は、証明コミットメント (パブリック入力) に `domain_tag`、`chain_id`、および `election_id` を加えたものから導出されます。 `--nullifier` は、提供されたときに証明に対して検証されます。
  - 維持管理を再開し、`fingerprint=<hex>` を決定し、`CastZkBallot` をエンコードし、ヒントをデコードします (`owner`、`amount`、`duration_blocks`、`direction`)シ・フルニス）。
  - レ レスポンス CLI アノテント `tx_instructions[]` avec `payload_fingerprint_hex` に加えて、デ シャンプ デコードを実行して、ダウンストリームの検証ファイル スケレットを再実装せずにデコード Norito を実行します。
  - イベント `LockCreated`/`LockExtended` でロックのヒントが表示され、投票 ZK une fois que le 回路がミーム valeurs を公開します。
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - 別名 `--lock-amount`/`--lock-duration-blocks` は、スクリプトのパリテを注ぐ ZK フラグの名前を表します。
  - 出撃履歴書の参照 `vote --mode zk` には、指示の暗号化およびチャンピオンの投票用紙の指紋が含まれます (`owner`、`amount`、`duration_blocks`、`direction`)、確認を完了してくださいラピッド アバント シグネチャー デュ スケレット。

インスタンスのリスト
- GET `/v1/gov/instances/{ns}` - 名前空間に注がれるアクティブなインスタンスのリストを取得します。
  - クエリパラメータ:
    - `contains`: `contract_id` のスチェーン フィルター (大文字と小文字を区別します)
    - `hash_prefix`: `code_hash_hex` の 16 進数のプレフィックスをフィルターします (小文字)
    - `offset` (デフォルト 0)、`limit` (デフォルト 100、最大 10_000)
    - `order`: `cid_asc` (デフォルト)、`cid_desc`、`hash_asc`、`hash_desc`
  - 応答: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - ヘルパー SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) または `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。

バレイヤージュ ダンロック (オペレーター/監査)
- `/v1/gov/unlocks/stats` を取得
  - 応答: { "height_current": H、"expired_locks_now": n、"referenda_with_expired": m、"last_掃引高さ": S }
  - 注: `last_sweep_height` ブロックの自動更新と、最近のロックの有効期限が切れ、持続します。 `expired_locks_now` ロック平均 `expiry_height <= height_current` のスキャンレジストレーションの推定計算。
- POST `/v1/gov/ballots/zk-v1`
  - リクエスト (DTO スタイル v1):
    {
      "権限": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      "バックエンド": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "オーナー": "i105…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (機能: `zk-ballot`)
  - JSON `BallotProof` を直接受け入れ、squelette `CastZkBallot` を受け入れます。
  - リクエスト:
    {
      "権限": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      「投票用紙」: {
        "バックエンド": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // Base64 du conteneur ZK1 ou H2*
        "root_hint": null, // オプションの 32 バイトの 16 進文字列 (資格ルート)
        "owner": null, // AccountId オプションの回線コミット所有者
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
    - 投票結果 `public_inputs_json` は `CastZkBallot` に割り当てられます。
    - エンベロープのバイト数は、命令のペイロードをベース64で再エンコードします。
    - 応答 `reason` は `submitted transaction` および Torii 投票用紙を渡します。
    - CET エンドポイントの最も有効な一意のファイル機能 `zk-ballot` はアクティブです。

検証パルクール CastZkBallot
- `CastZkBallot` は、preuve Base64 のフォーニーとリジェットのペイロードをデコードし、形式を確認します (`BallotRejected` avec `invalid or empty proof`)。
- 住民投票の検証 (`vk_ballot`) が存在し、登録のデフォルトと登録が存在するため、`Active` とバイトのインライン転送が行われます。
- 検証ストックのバイト数は、平均 `hash_vk` を再ハッシュします。コミットメントの不一致を指摘し、事前の実行検証を行ってレジストリを不正に保護しています (`BallotRejected` avec `verifying key commitment mismatch`)。
- Les bytes de preuve Sont は、`zk::verify_backend` 経由でバックエンド登録をディスパッチします。転写ファイルは、`BallotRejected` avec `invalid proof` などの命令エコー決定を無効にします。
- 証明では、投票コミットメントと資格ルートを公開入力として公開する必要があります。ルートは選挙の `eligible_root` と一致する必要があり、派生した無効化子は提供されたヒントと一致する必要があります。
- Les preuves reussies emettent `BallotAccepted`;重複の無効化、ルートの適格性、または回帰の制限をロックし、製造レゾンを拒否し、既存のドキュメントを決定します。

## モーベーズの検証とコンセンサスの結合

### スラッシュとジェイルのワークフロー

`Evidence` は Norito でエンコードされたプロトコルのコンセンサスを検証します。 Chaque ペイロードは、メモワール `EvidenceStore` で到着し、編集せず、マップ `consensus_evidence` で WSV を参照して実体化します。登録と古代の `sumeragi.npos.reconfig.evidence_horizon_blocks` (デフォルト `7200` ブロック) は、管理者が保管するアーカイブを保持し、操作者を保持します。範囲内の証拠は、`sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) およびスラッシュ遅延 `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) も考慮します。ガバナンスは、スラッシュが適用される前に `CancelConsensusEvidencePenalty` でペナルティをキャンセルできます。

犯罪行為は再開され、`EvidenceKind` で報告されます。 les discriminants は安定したものではなく、par le データ モデルを課します。

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

- **DoublePrepare/DoubleCommit** - ミーム タプル `(phase,height,view,epoch)` の衝突とハッシュの署名を検証します。
- **InvalidQc** - ゴシップをまとめたり、証明書をコミットしたりする必要はありません。補助的な検証を決定する必要はありません (例、署名ビデオのビットマップ)。
- **無効な提案** - リーダーは、検証構造のブロック化を提案します (例: ロックチェーン規則の違反)。
- **検閲** - 署名された提出受領書には、決して提案/コミットされていないトランザクションが示されています。

オペレータと監視員の検査官、およびペイロードの再ブロードキャスト:

- Torii: `GET /v1/sumeragi/evidence` および `GET /v1/sumeragi/evidence/count`。
- CLI: `iroha ops sumeragi evidence list`、`... count`、および `... submit --evidence-hex <payload>`。

La gouvernance doit traiter les bytes d'evidence comme preuve canonique:1. **期限切れになる前に**コレクタ ファイル ペイロード**。アーカイバー ファイル バイト Norito ブルート 平均メタデータの高さ/ビュー。
2. **刑罰の準備者** は、国民投票および命令 sudo による安全なペイロードを作成します (例、`Unregister::peer`)。 L'実行はペイロードを再有効化します。証拠は形式的には古く、決定論を拒否します。
3. **計画的なトポロジー デ スイヴィ** は、即座に検証を行うことができます。 `SetParameter(Sumeragi::NextMode)` と `SetParameter(Sumeragi::ModeActivationHeight)` の平均名簿が 1 週間で欠落しています。
4. **結果を監査** (`/v1/sumeragi/evidence` および `/v1/sumeragi/status` 経由) して、前衛的な証拠とアップリケの証拠を確認します。

### コンセンサスジョイントのシーケンス

Le consensus join garantit que l'ensemble de validateurs sortant を最終決定し、前衛ブロック que le nouvel ensemble 提案者を開始します。ランタイムはパラメータ設定を介して規則を適用します:

- `SumeragiParameter::NextMode` と `SumeragiParameter::ModeActivationHeight` は **ミーム ブロック** にコミットします。 `mode_activation_height` doit etre strictement superieur a la hauteur du block qui a porte la mise a jour, donnant au moins un block de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) ラグゼロをハンドオフするための最も重要な構成:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) はコンセンサス スラッシュを遅らせ、ペナルティが適用される前にガバナンスがキャンセルできるようにします。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- `/v1/sumeragi/params` および `iroha --output-format text ops sumeragi params` を介してステージングされたランタイムおよび CLI のパラメータを公開し、オペレーターのアクティベーションおよび検証の名簿を確認します。
- 統治の自動化:
  1. 証拠に基づいた支持者に対する撤回決定 (再統合) の最終決定者。
  2. Enfiler une reconfiguration de suvi avec `mode_activation_height = h_current + activation_lag_blocks`。
  3. 監視者 `/v1/sumeragi/status` jusqu'a ce que `effective_consensus_mode` bascule a la hauteurAttendue。

スクリプトを十分に検証し、アップリケを削除し、** 実行して**、タイムラグをゼロにし、ハンドオフのパラメーターを有効化してください。 CES 取引は、拒否権やレゾー、モードの先例に基づいて行われます。

## テレメトリの表面

- Les metriques Prometheus exportent l'activite de governance:
  - `governance_proposals_status{status}` (ゲージ) ステータスに応じた提案書コンペティションのスーツ。
  - `governance_protected_namespace_total{outcome}` (カウンター) 名前空間の保護者の許可を増分し、展開を拒否することを受け入れます。
  - `governance_manifest_activations_total{event}` (カウンター) マニフェストの挿入 (`event="manifest_inserted"`) および名前空間のバインディング (`event="instance_bound"`) を登録します。
- `/status` には、オブジェクト `governance` の提案、名前空間の保護者およびアクティベーションの最近のマニフェストの報告書 (名前空間、契約 ID、コード/ABI ハッシュ、ブロックの高さ、アクティベーション タイムスタンプ) の参照が含まれます。操作者は、1 週間のミスでの制定や、保護者が課すネームスペースのゲートなどを監視します。
- テンプレート Grafana (`docs/source/grafana_governance_constraints.json`) および `telemetry.md` テレメトリーのランブックは、プロポーザルのブロック、マニフェストのアクティベーション、アップグレード ランタイムの拒否、およびプロジェクトの警告を通知します。