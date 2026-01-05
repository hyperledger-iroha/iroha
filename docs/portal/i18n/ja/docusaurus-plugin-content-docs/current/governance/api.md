---
id: api
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

ステータス: ガバナンス実装タスクに伴うドラフト/スケッチ。実装中に形は変更される可能性があります。決定性とRBACポリシーは規範制約です。`authority` と `private_key` が提供される場合、Torii はトランザクションに署名して送信できます。それ以外はクライアントが組み立てて `/transaction` に送信します。

概要
- すべてのエンドポイントはJSONを返します。トランザクション生成フローのレスポンスには `tx_instructions` - 1つ以上の命令スケルトン配列が含まれます:
  - `wire_id`: 命令タイプのレジストリ識別子
  - `payload_hex`: Norito payload バイト (hex)
- `authority` と `private_key` (または ballot DTO の `private_key`) が提供されると、Torii は署名・送信し、`tx_instructions` も返します。
- それ以外は、クライアントが authority と chain_id を用いて SignedTransaction を構築し、署名して `/transaction` にPOSTします。
- SDKカバレッジ:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` は `GovernanceProposalResult` (status/kind を正規化) を返し、`ToriiClient.get_governance_referendum_typed` は `GovernanceReferendumResult`、`ToriiClient.get_governance_tally_typed` は `GovernanceTally`、`ToriiClient.get_governance_locks_typed` は `GovernanceLocksResult`、`ToriiClient.get_governance_unlock_stats_typed` は `GovernanceUnlockStats`、`ToriiClient.list_governance_instances_typed` は `GovernanceInstancesPage` を返し、ガバナンス面の型付きアクセスをREADMEの使用例とともに提供します。
- Python軽量クライアント (`iroha_torii_client`): `ToriiClient.finalize_referendum` と `ToriiClient.enact_proposal` は `GovernanceInstructionDraft` 型のバンドル (Toriiの `tx_instructions` スケルトンをラップ) を返し、Finalize/Enact のフローを組み合わせるスクリプトの手動JSON解析を回避します。
- JavaScript (`@iroha/iroha-js`): `ToriiClient` は proposals/referenda/tallies/locks/unlock stats の型付きヘルパーに加え、`listGovernanceInstances(namespace, options)` と council用エンドポイント (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) を提供し、Node.js クライアントが `/v1/gov/instances/{ns}` をページングし、既存の契約インスタンス一覧と並行して VRF ベースのワークフローを扱えるようにします。

エンドポイント

- POST `/v1/gov/proposals/deploy-contract`
  - リクエスト (JSON):
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
  - レスポンス (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 検証: ノードは指定された `abi_version` の `abi_hash` を正規化し、不一致を拒否します。`abi_version = "v1"` の期待値は `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` です。

契約API (deploy)
- POST `/v1/contracts/deploy`
  - リクエスト: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - 振る舞い: IVMプログラム本体から `code_hash` を、ヘッダ `abi_version` から `abi_hash` を計算し、`RegisterSmartContractCode` (manifest) と `RegisterSmartContractBytes` (完全な `.to` バイト) を `authority` として送信します。
  - レスポンス: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - 関連:
    - GET `/v1/contracts/code/{code_hash}` -> 保存された manifest を返す
    - GET `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }` を返す
- POST `/v1/contracts/instance`
  - リクエスト: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - 振る舞い: 供給された bytecode をデプロイし、`ActivateContractInstance` で `(namespace, contract_id)` マッピングを即時有効化します。
  - レスポンス: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

エイリアスサービス
- POST `/v1/aliases/voprf/evaluate`
  - リクエスト: { "blinded_element_hex": "..." }
  - レスポンス: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` は評価器の実装を示します。現行値: `blake2b512-mock`。
  - 注記: ドメイン分離 `iroha.alias.voprf.mock.v1` を使う Blake2b512 の決定的モック評価器。プロダクションのVOPRFパイプラインがIrohaに接続されるまでのテスト用。
  - エラー: 不正なhex入力はHTTP `400`。Toriiは Norito の `ValidationFail::QueryFailed::Conversion` エンベロープとデコーダのエラーメッセージを返します。
- POST `/v1/aliases/resolve`
  - リクエスト: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - レスポンス: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - 注記: ISO bridge runtime staging (`[iso_bridge.account_aliases]` in `iroha_config`) が必要。Toriiは空白を削除して大文字化してから照合します。存在しない場合は404、ISO bridge runtimeが無効の場合は503を返します。
- POST `/v1/aliases/resolve_index`
  - リクエスト: { "index": 0 }
  - レスポンス: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - 注記: エイリアスのインデックスは設定順に決定的に割り当てられます (0-based)。クライアントはレスポンスをオフラインでキャッシュし、エイリアスのアテステーションイベントの監査トレイルを構築できます。

コードサイズ上限
- カスタムパラメータ: `max_contract_code_bytes` (JSON u64)
  - オンチェーンで保存できる契約コードサイズ上限 (バイト) を制御します。
  - デフォルト: 16 MiB。`.to` イメージ長が上限を超える場合、ノードは `RegisterSmartContractBytes` を不変条件違反で拒否します。
  - オペレータは `SetParameter(Custom)` で `id = "max_contract_code_bytes"` と数値payloadを送信して調整できます。

- POST `/v1/gov/ballots/zk`
  - リクエスト: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - レスポンス: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - 注記:
    - 回路の公開入力が `owner`、`amount`、`duration_blocks` を含み、証明が設定されたVKで検証できる場合、ノードは `election_id` に対するガバナンスロックを `owner` で作成/延長します。方向は隠蔽 (`unknown`) され、amount/expiry だけ更新します。再投票は単調で、amount と expiry は増えるだけです (ノードは max(amount, prev.amount) と max(expiry, prev.expiry) を適用)。
    - amount/expiry を減らそうとする ZK 再投票は `BallotRejected` 診断でサーバ側が拒否します。
    - コントラクト実行は `SubmitBallot` をキューする前に `ZK_VOTE_VERIFY_BALLOT` を呼び出す必要があり、ホストはワンショット・ラッチを強制します。

- POST `/v1/gov/ballots/plain`
  - リクエスト: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - レスポンス: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - 注記: 再投票は延長のみで、新しい ballot は既存ロックの amount/expiry を減らせません。`owner` はトランザクションの authority と一致する必要があります。最小期間は `conviction_step_blocks` です。

- POST `/v1/gov/finalize`
  - リクエスト: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "alice@wonderland?", "private_key": "...?" }
  - レスポンス: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - オンチェーン効果 (現在のスキャフォールド): 承認済みのdeploy提案をenactすると、`code_hash` をキーにした最小 `ContractManifest` を `abi_hash` 期待値で挿入し、提案を Enacted にします。`code_hash` に異なる `abi_hash` の manifest が既に存在する場合、enactment は拒否されます。
  - 注記: ZK選挙では、コントラクト経路は `FinalizeElection` の前に `ZK_VOTE_VERIFY_TALLY` を呼ぶ必要があり、ホストはワンショット・ラッチを強制します。

- POST `/v1/gov/enact`
  - リクエスト: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "alice@wonderland?", "private_key": "...?" }
  - レスポンス: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注記: `authority`/`private_key` が提供されるとToriiが署名済みトランザクションを送信します。それ以外はクライアントが署名・送信するためのスケルトンを返します。preimage は任意で現状は情報用途です。

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: proposal id hex (64 chars)
  - レスポンス: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: referendum id string
  - レスポンス: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - レスポンス: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 注記: 永続化された council があればそれを返し、なければ設定された stake asset としきい値を使って決定的なフォールバックを導出します (VRFの実証がオンチェーンに保存されるまで仕様を反映)。

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - リクエスト: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 振る舞い: 各候補のVRF証明を `chain_id`、`epoch`、最新ブロックハッシュのビーコンから導出した正規入力で検証し、出力バイト降順でタイブレークし、上位 `committee_size` を返します。永続化はしません。
  - レスポンス: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - 注記: Normal = pkはG1、proofはG2 (96 bytes)。Small = pkはG2、proofはG1 (48 bytes)。入力はドメイン分離され `chain_id` を含みます。

### ガバナンスのデフォルト (iroha_config `gov.*`)

Torii が永続化された roster を持たない場合のフォールバック council は `iroha_config` でパラメータ化されます:

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

環境変数の同等オーバーライド:

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

`parliament_committee_size` は council が永続化されていない場合に返されるフォールバックメンバー数を制限し、`parliament_term_blocks` はシード導出のエポック長 (`epoch = floor(height / term_blocks)`)、`parliament_min_stake` は資格資産の最小stake (最小単位) を強制し、`parliament_eligibility_asset_id` は候補集合の構築時にスキャンする資産残高を選択します。

ガバナンスVK検証にバイパスはありません。ballot検証は常に `Active` の検証鍵とインラインバイトを必要とし、環境はテスト用トグルで検証を回避してはいけません。

RBAC
- オンチェーン実行に必要な権限:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (将来): `CanManageParliament`

保護されたNamespaces
- カスタムパラメータ `gov_protected_namespaces` (JSON array of strings) により、列挙されたnamespaceへのdeployをadmissionでゲートします。
- 保護namespaceへのdeployでは、クライアントはトランザクションmetadataキーを含める必要があります:
  - `gov_namespace`: 対象namespace (例: "apps")
  - `gov_contract_id`: namespace内の論理contract id
- `gov_manifest_approvers`: オプションのJSON array (validatorのaccount ID)。lane manifest がquorum>1を宣言する場合、admissionはトランザクションauthorityに加えて一覧アカウントによるquorum満足を要求します。
- テレメトリは `governance_manifest_admission_total{result}` を通じてadmissionの合否を公開し、`missing_manifest`、`non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected`、`runtime_hook_rejected` を識別できます。
- テレメトリは `governance_manifest_quorum_total{outcome}` (値 `satisfied` / `rejected`) により、承認不足の監査を可能にします。
- Lanes は manifest に公開された namespace allowlist を適用します。`gov_namespace` を設定するトランザクションは `gov_contract_id` を必須とし、namespace は manifest の `protected_namespaces` セットに含まれている必要があります。このmetadataがない `RegisterSmartContractCode` は保護有効時に拒否されます。
- Admission は `(namespace, contract_id, code_hash, abi_hash)` に対する Enacted なガバナンス提案の存在を強制し、存在しない場合は NotPermitted エラーで検証失敗します。

Runtime Upgrade Hooks
- lane manifest は `hooks.runtime_upgrade` を宣言して runtime upgrade 命令 (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) をゲートできます。
- Hookフィールド:
  - `allow` (bool, default `true`): `false` の場合、すべてのruntime upgrade命令を拒否します。
  - `require_metadata` (bool, default `false`): `metadata_key` で指定されたmetadataエントリを要求します。
  - `metadata_key` (string): hookが強制するmetadata名。metadataが必要またはallowlistが存在する場合のデフォルトは `gov_upgrade_id`。
  - `allowed_ids` (array of strings): metadata値のallowlist (trim後)。値がリストにない場合は拒否します。
- Hookが存在する場合、キューadmissionはトランザクションがキューに入る前にmetadataポリシーを強制します。metadata欠落、空値、allowlist外値は決定的な NotPermitted エラーになります。
- テレメトリは `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` を通じて結果を追跡します。
- Hook条件を満たすトランザクションは `gov_upgrade_id=<value>` (またはmanifestで定義されたキー) を含み、manifest quorumに必要なvalidator承認も併せて必要です。

便利なエンドポイント
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` をノードに直接適用します。
  - リクエスト: { "namespaces": ["apps", "system"] }
  - レスポンス: { "ok": true, "applied": 1 }
  - 注記: 管理/テスト向け。設定されていればAPIトークンが必要です。本番では `SetParameter(Custom)` を署名して送信する方を推奨します。

CLIヘルパー
- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - namespace の契約インスタンスを取得し、次をクロスチェックします:
    - Torii が各 `code_hash` の bytecode を保存し、その Blake2b-32 digest が `code_hash` と一致すること。
    - `/v1/contracts/code/{code_hash}` の manifest が `code_hash` と `abi_hash` の一致を報告すること。
    - ノードが使う proposal-id ハッシュと同一の導出で `(namespace, contract_id, code_hash, abi_hash)` の enacted ガバナンス提案が存在すること。
  - 契約ごとの `results[]` (issues, manifest/code/proposal のサマリ) を含む JSON レポートと、抑制されない限り1行サマリ (`--no-summary`) を出力。
  - 保護されたnamespaceの監査やガバナンス制御デプロイフローの検証に有用。
- `iroha gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - 保護namespaceへのデプロイ時に使うmetadata JSONスケルトンを出力し、manifest quorum を満たすための `gov_manifest_approvers` を任意で含めます。
- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --salt-hex <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`
  - 正規の account id を検証し、32バイトの非ゼロsaltを強制し、`public_inputs_json` にヒントをマージします (`--public <path>` で追加上書き)。
  - 1行サマリは、`CastZkBallot` から導出された決定的な `fingerprint=<hex>` と、デコードされたヒント (`owner`, `amount`, `duration_blocks`, `direction` 提供時) を表示します。
  - CLIレスポンスは `tx_instructions[]` に `payload_fingerprint_hex` とデコード済みフィールドを付与し、下流ツールがNoritoデコードを再実装せずスケルトンを検証できます。
  - ロックヒントを提供すると、回路が同じ値を公開した際にZK ballotに対して `LockCreated`/`LockExtended` イベントが発行されます。
- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` エイリアスは ZK のフラグ名と揃えてスクリプト互換性を確保します。
  - サマリ出力は `vote-zk` と同様に、エンコード済み命令の fingerprint と人間可読の ballot フィールド (`owner`, `amount`, `duration_blocks`, `direction`, `salt_hex` がある場合) を含み、署名前の迅速確認を提供します。

インスタンス一覧
- GET `/v1/gov/instances/{ns}` - namespace のアクティブな契約インスタンスを一覧表示します。
  - Query params:
    - `contains`: `contract_id` の部分一致でフィルタ (case-sensitive)
    - `hash_prefix`: `code_hash_hex` のhexプレフィックスでフィルタ (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - レスポンス: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK helper: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) または `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Unlock sweep (オペレータ/監査)
- GET `/v1/gov/unlocks/stats`
  - レスポンス: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - 注記: `last_sweep_height` は期限切れロックがスイープされ永続化された最新のブロック高。`expired_locks_now` は `expiry_height <= height_current` のロックレコードをスキャンして算出します。
- POST `/v1/gov/ballots/zk-v1`
  - リクエスト (v1スタイルDTO):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint_hex": "...64hex?",
      "owner": "alice@wonderland?",
      "salt_hex": "...64hex?"
    }
  - レスポンス: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - `BallotProof` JSONを直接受け取り、`CastZkBallot` スケルトンを返します。
  - リクエスト:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // ZK1 または H2* コンテナのbase64
        "root_hint": null,                // オプションの32バイトhex (eligibility root)
        "owner": null,                    // 回路が owner をコミットする場合のAccountId
        "salt": null                      // nullifier導出用の32バイトhex
      }
    }
  - レスポンス:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - 注記:
    - サーバは ballot の `root_hint`/`owner`/`salt` を `CastZkBallot` の `public_inputs_json` にマップします。
    - envelope bytes は命令payloadのためにbase64で再エンコードされます。
    - Torii が ballot を送信すると `reason` は `submitted transaction` に変わります。
    - この endpoint は `zk-ballot` feature が有効な場合のみ利用可能です。

CastZkBallot 検証パス
- `CastZkBallot` は提供された base64 proof をデコードし、空または不正なpayloadを拒否します (`BallotRejected` with `invalid or empty proof`)。
- ホストは referendum (`vk_ballot`) またはガバナンスデフォルトから ballot 検証鍵を解決し、レコードが存在し `Active` でインラインバイトを持つことを要求します。
- 保存された検証鍵バイトは `hash_vk` で再ハッシュされ、コミットメント不一致は検証前に実行を中断して改ざんされたレジストリエントリを防ぎます (`BallotRejected` with `verifying key commitment mismatch`)。
- proof bytes は `zk::verify_backend` により登録済みバックエンドへ送られ、無効なトランスクリプトは `BallotRejected` の `invalid proof` として決定的に失敗します。
- 成功したproofは `BallotAccepted` を発行し、重複nullifier、古いeligibility root、lockの退行は本書の既存の拒否理由が適用されます。

## バリデータ不正と共同コンセンサス

### Slashing と Jailing のフロー

コンセンサスはバリデータがプロトコル違反した際に Norito エンコードされた `Evidence` を発行します。各payloadはメモリ内の `EvidenceStore` に入り、未見ならWSVバックの `consensus_evidence` に具体化されます。`sumeragi.npos.reconfig.evidence_horizon_blocks` (デフォルト `7200` ブロック) より古い記録はアーカイブを有限に保つため拒否されますが、その拒否は運用者向けにログ化されます。

認識される違反は `EvidenceKind` と1対1で対応し、識別子はデータモデルによって安定的に強制されます:

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

- **DoublePrepare/DoubleCommit** - 同じ `(phase,height,view,epoch)` で矛盾するハッシュに署名。
- **DoubleExecVote** - 異なる post-state root を広告する実行投票の衝突。
- **InvalidCommitCertificate** - 集約者が deterministic チェックに失敗するcommit certificateをゴシップ (例: 空の署名ビットマップ)。
- **InvalidProposal** - リーダが locked-chain ルールを破るなど、構造検証に失敗するブロックを提案。

運用者とツールは次でペイロードを確認・再送できます:

- Torii: `GET /v1/sumeragi/evidence` と `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.

ガバナンスは evidence bytes を正規の証拠として扱う必要があります:

1. **ペイロード収集** 期限切れ前に回収し、Norito生バイトをheight/viewメタデータと共に保存。
2. **ペナルティ段取り** payload を referendum や sudo 命令 (例: `Unregister::peer`) に埋め込みます。実行時に再検証され、不正/古い evidence は決定的に拒否されます。
3. **フォローアップトポロジ** 違反バリデータが即時復帰できないようにします。典型的には `SetParameter(Sumeragi::NextMode)` と `SetParameter(Sumeragi::ModeActivationHeight)` を更新 roster と共にキューします。
4. **結果監査** `/v1/sumeragi/evidence` と `/v1/sumeragi/status` を監視し、evidence カウンタが進み除外が実行されたことを確認します。

### 共同コンセンサスのシーケンス

共同コンセンサスは、旧バリデータセットが境界ブロックを確定してから新セットが提案を開始することを保証します。ランタイムは次のペアでルールを強制します:

- `SumeragiParameter::NextMode` と `SumeragiParameter::ModeActivationHeight` は**同じブロック**でコミットされる必要があります。`mode_activation_height` は更新ブロックより厳密に大きく、最低1ブロックの遅延を提供します。
- `sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) はゼロラグの引き継ぎを防ぐ設定ガードです:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- ランタイムとCLIは `/v1/sumeragi/params` と `iroha sumeragi params --summary` で staged パラメータを公開し、運用者が活性化高さとバリデータロスターを確認できます。
- ガバナンス自動化は常に:
  1. evidenceに基づく除外/復帰決定を確定する。
  2. `mode_activation_height = h_current + activation_lag_blocks` で後続リコンフィグをキューする。
  3. `/v1/sumeragi/status` を監視し、`effective_consensus_mode` が想定高さで切り替わるまで確認する。

バリデータローテーションやslashingを行うスクリプトは**ゼロラグ有効化やhand-offパラメータの省略を行ってはいけません**。それらは拒否され、ネットワークは旧モードのままになります。

## テレメトリ面

- Prometheusメトリクスがガバナンス活動をエクスポート:
  - `governance_proposals_status{status}` (gauge) は提案数を状態別に追跡。
  - `governance_protected_namespace_total{outcome}` (counter) は保護namespaceのadmissionが許可/拒否されたときに増加。
  - `governance_manifest_activations_total{event}` (counter) は manifest 挿入 (`event="manifest_inserted"`) と namespace バインド (`event="instance_bound"`) を記録。
- `/status` は `governance` オブジェクトを含み、提案数、保護namespace合計、最近のmanifest活性化 (namespace, contract id, code/ABI hash, block height, activation timestamp) を報告します。運用者はこのフィールドを監視し、enactmentがmanifestを更新し保護namespaceゲートが適用されていることを確認できます。
- Grafanaテンプレート (`docs/source/grafana_governance_constraints.json`) と `telemetry.md` のテレメトリrunbookは、提案の停滞、manifest活性化の欠落、runtime upgrade中の保護namespace拒否の予期せぬ増加に対するアラートの配線方法を示します。
