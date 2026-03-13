---
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

エスタード: ゴベルナンザの実装のためのボラドール/ボセト。ラスフォーマスプエデンカンビアデュランテラ実装。政治的決定主義により、RBAC の息子は規範を制限します。 Torii は、`authority` と `private_key` に比例した信頼/環境のトランザクションを実行します。`/transaction` を参照してください。

履歴書
- Todos は JSON を使用してエンドポイントを失います。 `tx_instructions` を含む、処理中のトランザクション、ラス レスプエスタの実行 - 安全な命令の実行:
  - `wire_id`: 指示に関する登録者の識別情報
  - `payload_hex`: ペイロード Norito (16 進数) のバイト数
- `authority` と `private_key` (投票用紙の DTO で `private_key`)、Torii は取引と開発 `tx_instructions` に比例します。
- 反逆的で、クライアントが SignedTransaction を使用しており、権限 ychain_id、ルエゴ ファームマン y hacen POST a `/transaction` を使用しています。
- コベルトゥーラ デ SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` devuelve `GovernanceProposalResult` (正規化ステータス/種類)、`ToriiClient.get_governance_referendum_typed` devuelve `GovernanceReferendumResult`、`ToriiClient.get_governance_tally_typed` devuelve `GovernanceTally`、`ToriiClient.get_governance_locks_typed` devuelve `GovernanceLocksResult`、`ToriiClient.get_governance_unlock_stats_typed` devuelve `GovernanceUnlockStats`、y `ToriiClient.list_governance_instances_typed` devuelve `GovernanceInstancesPage`、imponiendo accesotipado README の使用法に関する詳細情報を参照してください。
- Cliente ligero Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal` devuelven バンドルヒント `GovernanceInstructionDraft` (envolviendo el esqueleto `tx_instructions` de Torii)、 evitando parseo JSON マニュアル cuando スクリプト コンポーネント flujos Finalize/Enact。
- JavaScript (`@iroha/iroha-js`): `ToriiClient` 提案、住民投票、集計、ロック、統計のロック解除などのヘルパーの説明、`listGovernanceInstances(namespace, options)` 議会でのエンドポイントの評価 (`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、 `governancePersistCouncil`、`getGovernanceCouncilAudit`) クライアント Node.js のページ `/v2/gov/instances/{ns}` は、VRF のコントロール インスタンスのリストを管理します。

エンドポイント

- POST `/v2/gov/proposals/deploy-contract`
  - 要請 (JSON):
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
  - レスペスタ(JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 検証: los nodos canonizan `abi_hash` para el `abi_version` provisto y rechazan desajustes。 `abi_version = "v1"` の場合、`hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` を有効にしてください。

API のデプロイ (デプロイ)
- POST `/v2/contracts/deploy`
  - Solicitud: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - 互換性: 計算 `code_hash` プログラム IVM y `abi_hash` ヘッダー `abi_version`、luego envia `RegisterSmartContractCode` (manifyto) y `RegisterSmartContractBytes` (バイト `.to` 完全) `authority` という名前。
  - 応答: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - レラシオナード:
    - GET `/v2/contracts/code/{code_hash}` -> アルマセナドをマニフェストするデブエルブ
    - GET `/v2/contracts/code-bytes/{code_hash}` -> devuelve `{ code_b64 }`
- POST `/v2/contracts/instance`
  - Solicitud: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - 互換性: `ActivateContractInstance` 経由の `(namespace, contract_id)` のバイトコード規定とメディアのアクティベーション。
  - 応答: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }別名サービス
- POST `/v2/aliases/voprf/evaluate`
  - 要請: { "blinded_element_hex": "..." }
  - 応答: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` 評価の実装を参照してください。実際の武勇: `blake2b512-mock`。
  - 注: 評価者は、モック決定アプリケーション Blake2b512 とドミニオ `iroha.alias.voprf.mock.v1` の分離を評価しました。 Iroha のプルエバ ツール パイプライン VOPRF の生産エステケーブルを切断します。
  - エラー: HTTP `400` en 入力 16 進数形式。 Torii エンベロープのデベロップメント Norito `ValidationFail::QueryFailed::Conversion` エラー デコーダの管理。
- POST `/v2/aliases/resolve`
  - ソリチュード: { "エイリアス": "GB82 WEST 1234 5698 7654 32" }
  - 回答: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - 注: ランタイム ISO ブリッジ ステージング (`[iso_bridge.account_aliases]` および `iroha_config`) が必要です。 Torii 正規化エイリアス エリミナンド エスパシオスとパサンド、マユスキュラス アンテス デル ルックアップ。 Devuelve 404 cuando el alias は存在しません 503 cuando el runtime ISO ブリッジは deshabilitado です。
- POST `/v2/aliases/resolve_index`
  - ソリチュード: { "インデックス": 0 }
  - 回答: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - 注: 別名を指定する形式を決定するためのインデックス、設定の順序 (0 ベース)。クライアントは、オフラインで聴衆のイベントを作成し、エイリアスを確認できます。

トペ・デ・タマノ・デ・コディゴ
- パラメータカスタム: `max_contract_code_bytes` (JSON u64)
  - オンチェーンのコードを制御するための最大限の許可 (バイト)。
  - デフォルト: 16 MiB。損失ノード `RegisterSmartContractBytes` イメージ `.to` は、不変の違反エラーに関するトップを超えています。
  - ロス オペラドーレス プエデン アジャスター エンビアンド `SetParameter(Custom)` コン `id = "max_contract_code_bytes"` y ペイロード数値。

- POST `/v2/gov/ballots/zk`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注意事項:
    - `owner`、`amount`、`duration_blocks`、VK 構成と比較したプルエバ検証、およびブロックを拡張するためのノード作成機能を含む回路の公開`election_id` と `owner`。永続的な眼の方向 (`unknown`);ソロSEの実際の金額/有効期限。単調なレボタシオネス: 金額 y 有効期限ソロ オーメンタン (el nodo aplica max(amount, prev.amount) y max(expiry, prev.expiry))。
    - Las revotaciones ZK que intenten reducir amount o exiry se rechazan del lado del servidor con Diagnosis `BallotRejected`。
    - La ejecucion del contrato debe llamar `ZK_VOTE_VERIFY_BALLOT` antes de encolar `SubmitBallot`;ロスは、ソラベスに影響を与える可能性があります。

- POST `/v2/gov/ballots/plain`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注: 単独の拡張子による再発行 - 新しい投票用紙には、期限切れの期限がありません。 El `owner` は取引上の権限を持っています。最小値 `conviction_step_blocks`。- POST `/v2/gov/finalize`
  - Solicitud: { "referendum_id": "r1"、"proposal_id": "...64hex"、"authority": "i105...?"、"private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - オンチェーン効果 (実際の効果): promulgar una propuesta dedeploy aprobada inserta un `ContractManifest` minimo con clave `code_hash` con el `abi_hash` esperado y marca la propuesta como が制定されました。私は `code_hash` と `abi_hash` を区別して宣言し、それを宣言します。
  - 注意事項:
    - ZK 氏、コントラート デベン ラマール `ZK_VOTE_VERIFY_TALLY` 出国前 `FinalizeElection` を参照してください。ロスは、ソラベスに影響を与える可能性があります。 `FinalizeReferendum` 再審査請求 ZK は最終的な審査結果を報告します。
    - 自動的に自動化される `h_end` が承認/拒否されたソロパラレファレンド プレーン。ロスリファレンドス ZK permanecen は閉じられていますが、`FinalizeReferendum` を参照してください。
    - 単独の承認+拒否による投票率の調整。クエンタ・パラ・エルの投票を棄権しない。

- POST `/v2/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { " lower": 0, "upper": 0 }?、 "authority": "i105…?"、 "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注: Torii envia la transaccion farmada cuando se proporcionan `authority`/`private_key`;デ・ロ・コントラリオ・デブエルブ・アン・エスケレト・パラケ・ロス・クライエンテス・ファームメン・イ・エンヴィエン。事前画像と実際の情報はオプションです。

- `/v2/gov/proposals/{id}` を取得
  - パス `{id}`: プロパティ ID 16 進数 (64 文字)
  - 応答: { "見つかった": bool、"提案": { ... }? }

- `/v2/gov/locks/{rid}` を取得
  - パス `{rid}`: 国民投票 ID の文字列
  - 応答: { "found": bool、"referendum_id": "rid"、"locks": { ... }? }

- `/v2/gov/council/current` を取得
  - 回答: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 注意: devuelve el Councilpersistido cuando存在します。 de lo contrario deriva un respaldo determinista usando el asset de stake configurado y umbrales (refleja la especificacion VRF hasta que pruebas VRF en vivo se persistan on-chain).

- POST `/v2/gov/council/derive-vrf` (機能: gov_vrf)
  - 要請: { "committee_size": 21、"epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 互換性: `chain_id`、`epoch` およびブロックの究極ハッシュのビーコンの入力コントラ エル 入力カノニコ デリバドのプルエバ VRF を検証します。タイブレーカーに関するサリダのバイト数を確認します。デブエルブ ロス トップ `committee_size` ミエンブロス。固執しないでください。
  - 回答: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - 注: 通常 = pk en G1、proof en G2 (96 バイト)。 Small = pk en G2、proof en G1 (48 バイト)。損失入力には、`chain_id` が含まれます。

### デフォルトのデフォルト (iroha_config `gov.*`)

Torii の議会デレスパルド米国は、`iroha_config` 経由で名簿の永続的なパラメータが存在しません:

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

エントリの同等のものをオーバーライドします。

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

`parliament_committee_size` 制限付き干し草評議会の持続性、`parliament_term_blocks` シード期間の米国時間の定義 (`epoch = floor(height / term_blocks)`)、`parliament_min_stake` アプリケーションの最小化 (en)最小限の資産を選択し、候補となる資産のバランスを選択します。

VK デ ゴベルナンザの検証はバイパスされません: 投票の検証は必要ありません。`Active` コンバイト インラインで、ロス エントルノスの deben 依存関係はありません。プルエバ パラ省略検証の切り替えは行われません。

RBAC
- オンチェーンの排出には許可が必要です:
  - 提案: `CanProposeContractDeployment{ contract_id }`
  - 投票用紙: `CanSubmitGovernanceBallot{ referendum_id }`
  - 制定: `CanEnactGovernance`
  - 評議会運営（futuro）：`CanManageParliament`名前空間プロテギド
- パラメトロ カスタム `gov_protected_namespaces` (文字列の JSON 配列) ハビリタ アドミッション ゲーティング パラは、名前空間リストにデプロイされます。
- クライアントのデベンには、トランザクションパラメタデータのクラベスが含まれており、ディリギドと名前空間プロテギドをデプロイします。
  - `gov_namespace`: 名前空間オブジェクト (つまり、「アプリ」)
  - `gov_contract_id`: 契約 ID ロジックのデントロ デル ネームスペース
- `gov_manifest_approvers`: 有効なアカウント ID のオプションの JSON 配列。 Cuando un manifyto de LANE declara un quorum 市長 a uno、入場は、権限を要求する承認の取引マス ラス cuentas listadas para satisfacer el quorum del manifyto を要求します。
- テレメトリーは、`governance_manifest_admission_total{result}` 経由で入場を許可し、オペラドールの入場を許可します。`missing_manifest`、`non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected`、y `runtime_hook_rejected`。
- `governance_manifest_quorum_total{outcome}` (値 `satisfied` / `rejected`) を介してテレメトリーが法執行を説明し、ファルタンテスを監査するためのオペラドールを監視します。
- ロスレーンは、名前空間の公開マニフェストの許可リストに適用されます。 `gov_namespace` は `gov_contract_id` に比例し、名前空間は `protected_namespaces` に基づいて設定されます。 Los envios `RegisterSmartContractCode` はメタデータを保存し、保護されています。
- タプル `(namespace, contract_id, code_hash, abi_hash)` で制定された、政府の存在を無効にする許可。エラー NotPermitted に反する検証が行われます。

ランタイムアップグレードのフック
- ランタイム アップグレードのコントローラー命令 `hooks.runtime_upgrade` のマニフェストが宣言されています (`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`)。
- カンポス・デル・フック:
  - `allow` (ブール値、デフォルト `true`): `false` を参照して、ランタイム アップグレードの最後の指示を確認してください。
  - `require_metadata` (ブール値、デフォルト `false`): `metadata_key` のメタデータ固有のエントリ。
  - `metadata_key` (文字列): メタデータ アプリケーション フックの名前。デフォルトの `gov_upgrade_id` は、ホワイトリストにメタデータを必要としません。
  - `allowed_ids` (文字列の配列): メタデータの値の許可リスト (トラス トリム)。 Rechaza cuando el valor provisto no esta listado。
- クアンド・エル・フック・エスタ・プレゼンテ、コーラ・アプリケーション・ラ・ポリティカ・デ・メタデータ・アンテス・デ・ケ・ラ・トランザクション・エントレ・ア・ラ・コーラ。メタデータが faltante、valores vacios o valores の許可リストが生成され、エラー NotPermitted が決定されました。
- `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` 経由の La telemetria rastrea resultados。
- メタデータ `gov_upgrade_id=<value>` (マニフェストを定義するクラベ) を含む、クンプレン エル フック デベンのトランザクションは、マニフェストの定足数を確認するために必要な有効性を確認するために必要です。

便利なエンドポイント
- POST `/v2/gov/protected-namespaces` - ノードに対するアプリケーション `gov_protected_namespaces` の指示。
  - Solicitud: { "名前空間": ["アプリ", "システム"] }
  - 応答: { "ok": true、"applied": 1 }
  - 注: ペンサド パラ管理/テスト。トークン API の設定が必要です。パラプロダクション、プリフィエラ環境、トランザクション会社 `SetParameter(Custom)`。ヘルパー CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 名前空間と検証に関するインスタンスの取得:
    - Torii アルマセナ バイトコード パラ Cada `code_hash`、Y su ダイジェスト Blake2b-32 は `code_hash` と一致します。
    - アルマセナド バホ `/v2/contracts/code/{code_hash}` 報告値 `code_hash` と `abi_hash` は一致します。
    - `(namespace, contract_id, code_hash, abi_hash)` で制定された提案 ID のミスモ ハッシュが存在します。
  - JSON con `results[]` por contrato (問題、マニフェスト/コード/提案の履歴書) を、ライン上の一斉射撃を再開するために出力します (`--no-summary`)。
  - 監査用の名前空間プロテクトまたは検証用のデプロイ制御システムを使用します。
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - メタデータを使用した JSON の安全な展開、`gov_manifest_approvers` を含む名前空間プロテジドの展開、定足数の規則を満たすためのオプションを公開します。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — ロック ヒントは、`min_bond_amount > 0` の義務を負っています。`owner`、`amount`、`duration_blocks` を含む、適切な結合ヒントが含まれています。
  - 正規アカウント ID を検証し、32 バイトの nullifier ヒントを正規化し、ヒントを `public_inputs_json` (追加のオーバーライド用に `--public <path>`) にマージします。
  - 無効化子は、証明コミットメント (パブリック入力) に `domain_tag`、`chain_id`、および `election_id` を加えたものから導出されます。 `--nullifier` は、提供されたときに証明に対して検証されます。
  - `fingerprint=<hex>` でのデターミニスタ デリバドの再開、`CastZkBallot` でのヒントの解読 (`owner`、`amount`、`duration_blocks`、 `direction` cuando se proporcionan)。
  - 解決策 CLI アノタン `tx_instructions[]` コン `payload_fingerprint_hex` は、ダウンストリームでの解読を検証し、Norito を再実装します。
  - 証明者は、イベント `LockCreated`/`LockExtended` パラ投票 ZK ウナベス キュー エル サーキット エクスポンガ ロス ミスモス ヴァロレスのヒントを失います。
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - 別名 `--lock-amount`/`--lock-duration-blocks` は、スクリプトの ZK フラグの名前を参照します。
  - 履歴書の履歴書 `vote --mode zk` には、投票用紙の読み取り可能な指示コードやカンポスの指紋も含まれます (`owner`、`amount`、`duration_blocks`、`direction`)。緊急の確認を迅速に行います。

インスタンスリスト
- GET `/v2/gov/instances/{ns}` - 名前空間の制御インスタンスをリストします。
  - クエリパラメータ:
    - `contains`: `contract_id` の部分文字列のフィルター (大文字と小文字を区別します)
    - `hash_prefix`: `code_hash_hex` の 16 進数のフィルター (小文字)
    - `offset` (デフォルト 0)、`limit` (デフォルト 100、最大 10_000)
    - `order`: `cid_asc` (デフォルト)、`cid_desc`、`hash_asc`、`hash_desc`
  - 応答: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - ヘルパー SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) o `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。

ロック解除のバリド (オペラドール/オーディトリア)
- `/v2/gov/unlocks/stats` を取得
  - 応答: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_ SWEEP_height": S }
  - 注意: `last_sweep_height` ブロックの高さを確認し、ロックを解除し、フエロン バリドスと持続性を確認してください。 `expired_locks_now` は、`expiry_height <= height_current` をロックするためのエスカネアンド レジスタを計算します。
- POST `/v2/gov/ballots/zk-v1`
  - Solicitud (DTO estilo v1):
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
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }- POST `/v2/gov/ballots/zk-v1/ballot-proof` (機能: `zk-ballot`)
  - JSON `BallotProof` を直接開発して `CastZkBallot` にアクセスします。
  - 要請:
    {
      "権限": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      「投票用紙」: {
        "バックエンド": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // ZK1 または H2 のベース 64*
        "root_hint": null, // オプションの 32 バイトの 16 進文字列 (資格ルート)
        "owner": null, // AccountId は、所有者を任意に指定できます
        "nullifier": null // オプションの 32 バイトの 16 進文字列 (nullifier ヒント)
      }
    }
  - レスペスタ:
    {
      「わかりました」: 本当、
      「受け入れられました」: true、
      "reason": "トランザクション スケルトンを構築する",
      "tx_instructions": [
        { "wire_id": "CastZkBallot"、"payload_hex": "..." }
      】
    }
  - 注意事項:
    - `root_hint`/`owner`/`nullifier` 投票用紙 `public_inputs_json` パラ `CastZkBallot` の管理者。
    - 命令のペイロードと同様にbase64で再エンコードされたエンベロープのバイトが失われます。
    - La respuesta `reason` cambia a `submitted transaction` cuando Torii envia el ballot。
    - Este エンドポイント ソロ esta disponible cuando el feature `zk-ballot` esta habilitado。

CastZkBallot の検証手順
- `CastZkBallot` は、base64 ペイロードを無効にし、不正な形式を復号化します (`invalid or empty proof` に対して)。
- ホストはクラーベの投票結果を検証します (`vk_ballot`) デフォルトの知事登録が必要です。海 `Active`、バイトをインラインで取得します。
- Los bytes de la clave verificadora Almacenada se re-hashean con `hash_vk`;不正行為を中止し、不正行為を防止するために不正行為を防止する必要があります (`BallotRejected` と `verifying key commitment mismatch`)。
- `zk::verify_backend` 経由のバックエンド レジストラードのプルエバ シートのバイトの損失。 `BallotRejected` と `invalid proof` は、最終的な命令を無効にします。
- 証明では、投票コミットメントと資格ルートを公開入力として公開する必要があります。ルートは選挙の `eligible_root` と一致する必要があり、派生した無効化子は提供されたヒントと一致する必要があります。
- プルエバスは `BallotAccepted` を発します。無効化された重複、根元のエレジビリダード・ヴィエホスまたは回帰のロック・シグエン・プロデュース・ラス・ラゾネス・デ・レチャソ存在の説明、安全な文書。

## マラ・コンダクタ・デ・バリドーレスとコンセンサス・コンフント

### 斬撃と投獄のフルホ

`Evidence` コードを Norito で確認し、プロトコルを有効にします。 Cada ペイロード llega al `EvidenceStore` en Memorial y, si no se vio antes, se materializa en el mapa `consensus_evidence` respaldado por WSV。 `sumeragi.npos.reconfig.evidence_horizon_blocks` (デフォルト `7200` ブロック) の前のレジストロスは、オペラドールの定期的なアーカイブを保存します。範囲内の証拠は、`sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) およびスラッシュ遅延 `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) も考慮します。ガバナンスは、スラッシュが適用される前に `CancelConsensusEvidencePenalty` でペナルティをキャンセルできます。

ラス・オフェンサス・レコノシダス・マペアン・ウノ・ア・ウノ・`EvidenceKind`;ロス・ディスクリミナンテス・ソン・エステーブルスとエスタン・レフォルザドス・ポル・エル・デ・データス:

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

- **DoublePrepare/DoubleCommit** - ミスモ タプル `(phase,height,view,epoch)` と競合する検証会社ハッシュ。
- **InvalidQc** - 承認を取り消してコミット証明書を確定する必要があります (つまり、ビットマップ デ ファームマンテス ヴァシオ)。
- **InvalidProposal** - 提案がブロックされ、構造の検証が行われません (つまり、ロック チェーンの規則)。
- **検閲** - 署名された提出受領書には、決して提案/コミットされていないトランザクションが示されています。

オペレータとツールは、次の場所を検査し、再ブロードキャスト ペイロードを移動します。

- Torii: `GET /v2/sumeragi/evidence` y `GET /v2/sumeragi/evidence/count`。
- CLI: `iroha ops sumeragi evidence list`、`... count`、y `... submit --evidence-hex <payload>`。

ラ・ゴベルナンザ・デベ・トラタール・ロス・バイト・デ・証拠コモ・プルエバ・カノニカ:1. **ペイロードの再収集** 事前準備。アーカイブ ロス バイト Norito 高さ/ビューのメタデータの詳細。
2. **刑罰を準備します** 国民投票または sudo 命令のペイロードを埋め込みます (ej.、`Unregister::peer`)。ペイロードの再検証。証拠は、決定的な問題を解決します。
3. **安全なトポロジのプログラマー** は、不正な行為を行ったり、介入したりすることはありません。フルホス ティピコス エンコラン `SetParameter(Sumeragi::NextMode)` と `SetParameter(Sumeragi::ModeActivationHeight)` は、実際の名簿を確認します。
4. **Auditar resultados** via `/v2/sumeragi/evidence` y `/v2/sumeragi/status` para asegurar que el contador de証拠を参照してください。

### 同意の確認

ガランティサとのコンセンサスは、バリダドレスの重要な最終決定とフロンテラのブロックと、提案者との新たな関係を結び付けます。実行時は、parametros parreados 経由で規則を無効にします:

- `SumeragiParameter::NextMode` y `SumeragiParameter::ModeActivationHeight` デベン確認、**ミスモ ブロック**。 `mode_activation_height` 市長は、ブロックの貨物と最新情報を制限し、ブロックの遅れを防ぎます。
- `sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) 事前のハンドオフの遅延を防ぐためのガード設定:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) はコンセンサス スラッシュを遅らせ、ペナルティが適用される前にガバナンスがキャンセルできるようにします。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- `/v2/sumeragi/params` および `iroha --output-format text ops sumeragi params` を介してステージングされたランタイムと CLI 指数パラメータ、パラケ オペラドールは、有効化および有効な名簿を確認します。
- La automatizacion de gobernanza siempre debe:
  1. 証拠の削除 (または再インストール) 決定の最終決定。
  2. `mode_activation_height = h_current + activation_lag_blocks` の再構成を参照してください。
  3. Monitorear `/v2/sumeragi/status` hasta que `effective_consensus_mode` cambie a la altura esperada.

スクリプトの暗記やアップリケのスラッシュ **デベなし** 意図的なアクティベーション コンラグ チェックを省略し、ハンドオフのパラメータを削除する必要があります。前のモードでのトランザクションを確認してください。

## テレメトリの機能

- ラス メトリカス Prometheus 輸出活動の活動:
  - `governance_proposals_status{status}` (ゲージ) rastrea conteos de propuestas por estado。
  - `governance_protected_namespace_total{outcome}` (カウンター) ネームスペース プロテギドの追加許可がデプロイ解除を許可します。
  - `governance_manifest_activations_total{event}` (カウンター) マニフェストのレジストラ挿入 (`event="manifest_inserted"`)、ネームスペースのバインディング (`event="instance_bound"`)。
- `/status` には、オブジェクト `governance` のプロパティの参照、名前空間プロテギドのリスト、マニフェストのアクティベーション情報の合計レポート (名前空間、コントラクト ID、コード/ABI ハッシュ、ブロックの高さ、アクティベーション タイムスタンプ) が含まれます。ロス オペラドーレス プエデン コンサルタ エステ カンポ パラ確認 que las promulgacionesactualizaron は、名前空間プロテギドのロス ゲートを明示します。
- Una plantilla Grafana (`docs/source/grafana_governance_constraints.json`) と `telemetry.md` のテレメトリア ランブックは、ケーブル アラートのプロプエスタ アタスカダ、マニフェスト ファルタンテスのアクティベーション、またはランタイム プロテジドの持続的なアップグレードの名前空間の再起動を管理します。