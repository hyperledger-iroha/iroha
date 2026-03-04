---
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ステータス: 統治者としてのラスクーニョ/エスボコ・パラ・アコンパナール。実装方法としては、持続期間が必要です。決定主義と政治の RBAC は規範を制限します。 Torii は、`authority` と `private_key` が、`/transaction` のサブメーターを制御するために、顧客と反対側のクライアントを制御します。

ヴィサオ・ジェラル
- Todos OS エンドポイントは JSON に戻ります。 `tx_instructions` を含む応答として、次のような指示が表示されます:
  - `wire_id`: 情報登録時の識別情報
  - `payload_hex`: ペイロード Norito (16 進数) のバイト数
- `authority` と `private_key` はフォルネシド (投票の DTO は `private_key` です)、Torii はトランザクションを管理するための `tx_instructions` です。
- 反対意見があり、クライアントが SignedTransaction を使用して、権限、chain_id、デポジット アッシナム、ファゼム POST パラ `/transaction` を使用しました。
- コベルトゥーラ デ SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` レトルナ `GovernanceProposalResult` (正規化ステータス/種類)、`ToriiClient.get_governance_referendum_typed` レトルナ `GovernanceReferendumResult`、`ToriiClient.get_governance_tally_typed` レトルナ`GovernanceTally`、`ToriiClient.get_governance_locks_typed` レトルナ `GovernanceLocksResult`、`ToriiClient.get_governance_unlock_stats_typed` レトルナ `GovernanceUnlockStats`、e `ToriiClient.list_governance_instances_typed` レトルナ `GovernanceInstancesPage`、インポンド アセッソ ティパド エム トダ政府の権限に関する例には README はありません。
- クライアント Python レベル (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` レトルナム バンドル ヒント `GovernanceInstructionDraft` (カプセルランドまたはエスケレート `tx_instructions` は Torii)、 evitando は、Fluxos を構成する JSON Quando スクリプトの解析マニュアルを Finalize/Enact にします。
- JavaScript (`@iroha/iroha-js`): `ToriiClient` 提案ヘルパーの情報提供、参照、集計、ロック、統計のロック解除、アゴラ `listGovernanceInstances(namespace, options)` 主要な OS エンドポイントの評議会 (`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、 `governancePersistCouncil`、`getGovernanceCouncilAudit`) クライアントの Node.js のページ `/v1/gov/instances/{ns}` と VRF のワークフローを制御して、既存のインスタンスのリストを作成します。

エンドポイント

- POST `/v1/gov/proposals/deploy-contract`
  - 要求 (JSON):
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
  - レスポスタ (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validaco: OS nos canonizam `abi_hash` para o `abi_version` fornecido e rejeitam divergentias。パラ `abi_version = "v1"`、有効なエスペラード `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`。

API のデプロイ (デプロイ)
- POST `/v1/contracts/deploy`
  - 要求: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - コンポルタメント: 計算 `code_hash` プログラムの一部 IVM e `abi_hash` ヘッダーの一部 `abi_version`、サブメタのデポジット `RegisterSmartContractCode` (マニフェスト) `RegisterSmartContractBytes` (バイト `.to` 完全) `authority` の名前。
  - レスポスタ: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - レラシオナード:
    - GET `/v1/contracts/code/{code_hash}` -> レトルナまたはマニフェスト アルマゼナド
    - GET `/v1/contracts/code-bytes/{code_hash}` -> レトルナ `{ code_b64 }`
- POST `/v1/contracts/instance`
  - 必須: { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - コンポート: `ActivateContractInstance` 経由で `(namespace, contract_id)` のバイトコードをデプロイし、すぐにマップを作成します。
  - レスポスタ: { "ok": true、"namespace": "apps"、"contract_id": "calc.v1"、"code_hash_hex": "..."、"abi_hash_hex": "..." }別名サービス
- POST `/v1/aliases/voprf/evaluate`
  - 要求: { "blinded_element_hex": "..." }
  - レスポスタ: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` は、アバリアドールの実装を反映します。武勇値: `blake2b512-mock`。
  - 注意: アプリケーション Blake2b512 com separacao de dominio `iroha.alias.voprf.mock.v1` を疑似決定的に確認してください。 Iroha を統合したパイプライン VOPRF のテスト用ツールの宛先。
  - エラー: HTTP `400` em 入力の 16 進数が不正です。 Torii レトルナ ウム エンベロープ Norito `ValidationFail::QueryFailed::Conversion` は、デコーダのエラーメッセージを表示します。
- POST `/v1/aliases/resolve`
  - 要求: { "エイリアス": "GB82 WEST 1234 5698 7654 32" }
  - レスポスタ: { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - 注: ランタイム ISO ブリッジ ステージング (`[iso_bridge.account_aliases]` em `iroha_config`) を要求します。 Torii 正規化エイリアスは、検索を行う前に、エスパーコスとコンバートを削除します。 Retorna 404 quando または alias esta ausente e 503 quando または runtime ISO ブリッジ esta desabilitado。
- POST `/v1/aliases/resolve_index`
  - 要求: { "インデックス": 0 }
  - レスポスタ: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - 注: 形式を決定する別名インデックス (0 ベース)。クライアントは、オフラインで視聴者とイベントを作成し、エイリアスで記録を保存します。

タマンホ・デ・コディゴの限界
- パラメータカスタム: `max_contract_code_bytes` (JSON u64)
  - オンチェーンの制御装置で最大の許可 (em バイト) を制御します。
  - デフォルト: 16 MiB。 `RegisterSmartContractBytes` は画像を取得することができません。`.to` は、不変のエラー メッセージの制限を超えています。
  - `SetParameter(Custom)` com `id = "max_contract_code_bytes"` eum ペイロード numerico 経由の Operadores podem ajustar。

- POST `/v1/gov/ballots/zk`
  - 要求: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - レスポスタ: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注意事項:
    - Quando os 入力は、`owner`、`amount`、`duration_blocks`、VK 構成と比較した検証、`election_id` com esse の政府ロックの基準を含む回路を公開します。 `owner`。永続的な眼球 (`unknown`)。アペナスの量と有効期限。 sao monotonic: amount e expiry apenas aumentam (o no aplica max(amount, prev.amount) e max(expiry, prev.expiry)) を再投票します。
    - ZK que tentem reduzir の金額と有効期限を再投票します。サーバー診断 com 診断 `BallotRejected` がありません。
    - 実行は、`ZK_VOTE_VERIFY_BALLOT` antes de enfileirar `SubmitBallot` と対照的に行われます。ホストは、UM ユニカ ベスをラッチします。

- POST `/v1/gov/ballots/plain`
  - 要求: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - レスポスタ: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注: 再投票は延長期間内に行われます。新しい投票は、ロックが存在する有効期限内に行われます。 O `owner` は取引上の権限を持っています。デュラカオ ミニマ e `conviction_step_blocks`。

- POST `/v1/gov/finalize`
  - 要求: { "referendum_id": "r1"、"proposal_id": "...64hex"、"authority": "ih58...?"、"private_key": "...?" }
  - レスポスタ: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito オンチェーン (足場そのもの): promulgar uma proposta dedeploy aprovada insere um `ContractManifest` minimo com chave `code_hash` com o `abi_hash` esperado e marca a proposta como が制定されました。マニフェストは `code_hash` com `abi_hash` と異なり、制定と再構築が必要です。
  - 注意事項:
    - Para eleicoes ZK, os caminhos do contrato devem Chamar `ZK_VOTE_VERIFY_TALLY` antes de executar `FinalizeElection`; OS は Unico でラッチを実行します。 `FinalizeReferendum` 再審査 ZK は最終的な評価を求めました。
    - O auto-fechamento em `h_end` は、承認/拒否された apenas para reviewendos を出力します。 Referendos ZK permanecem は、最終的な処理を終了し、`FinalizeReferendum` を実行しました。
    - 選挙参加者として、承認+拒否を行います。投票率を棄権してください。- POST `/v1/gov/enact`
  - Requisicao: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { " lower": 0, "upper": 0 }?、 "authority": "ih58...?"、 "private_key": "...?" }
  - レスポスタ: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注: Torii は、トランザクション `authority`/`private_key` フォルネシドスをサブメテします。クライアントに対する反論やサブメーターの要求も同様です。プレイメージとオプションの情報。

- `/v1/gov/proposals/{id}` を取得
  - パス `{id}`: 提案 ID 16 進数 (64 文字)
  - レスポスタ: { "見つかった": bool、"提案": { ... }? }

- `/v1/gov/locks/{rid}` を取得
  - パス `{rid}`: 国民投票の ID 文字列
  - レスポスタ: { "found": bool、"referendum_id": "rid"、"locks": { ... }? }

- `/v1/gov/council/current` を取得
  - レスポスタ: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - メモ: 議会の議題は持続的に存在します。フォールバックの決定性と資産のステーク設定のしきい値を決定します (VRF がオンチェーンで VRF を生成し、VRF を特定します)。

- POST `/v1/gov/council/derive-vrf` (機能: gov_vrf)
  - 要求: { "committee_size": 21、"epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - コンポルタメント: `chain_id`、`epoch` の入力カノニコ デリバドを検証し、VRF の候補を検証し、ブロックの究極のハッシュを実行します。タイブレーカーのバイト数を確認できます。レトルナOSトップ`committee_size`メンバー。ナオは粘ります。
  - レスポスタ: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - 注: 通常 = pk em G1、proof em G2 (96 バイト)。 Small = pk em G2、proof em G1 (48 バイト)。 `chain_id` を含む、ポート セパラドスを入力します。

### 統治のデフォルト (iroha_config `gov.*`)

O 評議会フォールバック usado pelo Torii quando nao 存在名簿の持続性とパラメーター `iroha_config` 経由:

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

アンビエンテ相当のものをオーバーライドします。

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

`parliament_committee_size` メンバーのフォールバック リストの数を制限し、議会が永続的に保持するようにします。`parliament_term_blocks` は、米国のシード権を無償で定義します (`epoch = floor(height / term_blocks)`)、`parliament_min_stake` は、ステークを最小限に抑えます (em unidades) minimas) エレジビリダーデの資産はありません。`parliament_eligibility_asset_id` は資産の選択と候補の組み合わせを決定します。

政府の VK 検証をバイパスします: `Active` COM バイトがインラインで検証され、テスト用に切り替えられたデバイスに依存します。

RBAC
- オンチェーンでの権限の実行:
  - 提案: `CanProposeContractDeployment{ contract_id }`
  - 投票用紙: `CanSubmitGovernanceBallot{ referendum_id }`
  - 制定: `CanEnactGovernance`
  - 評議会運営（futuro）：`CanManageParliament`

名前空間プロテギド
- パラメトロ カスタム `gov_protected_namespaces` (文字列の JSON 配列) ハビリタ アドミッション ゲートパラは、em 名前空間リストをデプロイします。
- クライアントは、トランザクション用のメタデータを含めて、名前空間プロテジドをデプロイします。
  - `gov_namespace`: 名前空間 alvo (例: "apps")
  - `gov_contract_id`: コントラクトIDlogico dentro do名前空間
- `gov_manifest_approvers`: 有効なアカウント ID のオプションの JSON 配列。レーンマニフェストは、定足数の主な宣言を宣言し、マニフェストの定足数を満たしているかどうかを確認するための権限を要求します。
- `governance_manifest_admission_total{result}` パラ ケ オペラドールの入場をテレメトリーで確認し、`missing_manifest`、`non_validator_authority`、`quorum_rejected`、`protected_namespace_rejected` e を許可します。 `runtime_hook_rejected`。
- `governance_manifest_quorum_total{outcome}` (値 `satisfied` / `rejected`) を介してテレメトリーが強制執行され、オペラドールの監査がファルタンテスに適用されます。
- レーンは、マニフェストを公開するネームスペースのホワイトリストに適用されます。 Qualquer トランザクションは、`gov_namespace` を定義し、`gov_contract_id` を定義します。名前空間は、`protected_namespaces` のマニフェストと結合しません。 `RegisterSmartContractCode` sem essa メタデータを提出して、安全な保護を確保してください。
- 政府の提案に対する承認は無効です パラオタプル `(namespace, contract_id, code_hash, abi_hash)` を制定しました。反対に、ファルハが無効であることを確認してください。ランタイムアップグレードのフック
- ランタイム アップグレード手順に関するレーン ポデム宣言 `hooks.runtime_upgrade` のマニフェスト (`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`)。
- カンポス ドゥ フック:
  - `allow` (ブール値、デフォルト `true`): `false` は、ランタイム アップグレードの指示として使用されます。
  - `require_metadata` (ブール値、デフォルト `false`): `metadata_key` に固有のメタデータのエントリを要求します。
  - `metadata_key` (文字列): メタデータ アプリケーションのフック名。デフォルトの `gov_upgrade_id` quando メタデータはホワイトリストに必要です。
  - `allowed_ids` (文字列の配列): メタデータの値の許可リスト (アポス トリム)。 Rejeita quando or valor fornecido nao esta listado.
- 提出した書類を提出し、フィラデルフィアの申請とメタデータのメタデータを提出し、書類の提出を求めます。メタデータの有効性、許可リストの作成、およびエラー NotPermitted の決定性。
- `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` によるテレメトリア ラストレイアの結果。
- トランザクションは、メタデータ `gov_upgrade_id=<value>` (マニフェストを定義する) を満たしているか、マニフェストの定足数を確認する必要があります。

便利なエンドポイント
- POST `/v1/gov/protected-namespaces` - aplica `gov_protected_namespaces` いいえ、いいえ。
  - 必須: { "名前空間": ["アプリ", "システム"] }
  - レスポスタ: { "ok": true、"applied": 1 }
  - 注: 管理者/テストの宛先。 API 設定用のトークンを要求します。生産者は、`SetParameter(Custom)` を使用して、環境を事前に確認してください。

ヘルパー CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 名前空間と会議のインスタンスを作成します:
    - Torii armazena バイトコード パラ Cada `code_hash`、Seu ダイジェスト Blake2b-32 は `code_hash` に対応します。
    - マニフェスト アルマゼナド、`/v1/contracts/code/{code_hash}` レポート、`code_hash`、`abi_hash` 特派員。
    - `(namespace, contract_id, code_hash, abi_hash)` で制定された政府提案書が存在します。提案 ID のハッシュ化が必要です。
  - 関係性に関する JSON com `results[]` をコントラト (問題、マニフェスト/コード/提案の履歴) を送信し、問題を解決する (`--no-summary`)。
  - 監査用の名前空間プロテジドや管理システムの展開制御の検証用に使用します。
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - メタデータの JSON を使用して、サブメーターのデプロイメント、名前空間プロテジド、`gov_manifest_approvers` を含む、マニフェストの定足数を満たすためのオプションを発行します。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` を含むロック ヒント、`owner`、`amount`、`duration_blocks` を含むヒントの結合。
  - 正規アカウント ID を検証し、32 バイトの nullifier ヒントを正規化し、ヒントを `public_inputs_json` (追加のオーバーライド用に `--public <path>`) にマージします。
  - 無効化子は、証明コミットメント (パブリック入力) に `domain_tag`、`chain_id`、および `election_id` を加えたものから導出されます。 `--nullifier` は、提供されたときに証明に対して検証されます。
  - O resumo de uma linha agora exibe `fingerprint=<hex>` deterministico derivado do `CastZkBallot` codificado junto comヒント decodificados (`owner`、`amount`、`duration_blocks`、 `direction` クアンドフォルネシドス）。
  - CLI アノタム `tx_instructions[]` com `payload_fingerprint_hex` として、ダウンストリームの検証を実行して、Norito の解読を再実装します。
  - フォルネサーは、`LockCreated`/`LockExtended` パラ投票 ZK の回路を使用するためのイベントの発行を許可しないことを示唆しています。
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - OS エイリアス `--lock-amount`/`--lock-duration-blocks` は、OS のフラグ ZK パラメータの名前です。
  - `vote --mode zk` は投票法規の文書に指紋を含めます (`owner`、`amount`、`duration_blocks`、`direction`)、迅速な対応を確認します。インスタンスのリスト
- GET `/v1/gov/instances/{ns}` - 名前空間パラメータのインスタンスをリストします。
  - クエリパラメータ:
    - `contains`: `contract_id` の部分文字列のフィルター (大文字と小文字を区別します)
    - `hash_prefix`: `code_hash_hex` の接頭辞 16 進数のフィルター (小文字)
    - `offset` (デフォルト 0)、`limit` (デフォルト 100、最大 10_000)
    - `order`: `cid_asc` (デフォルト)、`cid_desc`、`hash_asc`、`hash_desc`
  - レスポスタ: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - ヘルパー SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) または `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。

ロック解除のヴァレドゥーラ (オペレーター/オーディトリア)
- `/v1/gov/unlocks/stats` を取得
  - レスポスタ: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_ SWEEPE_height": S }
  - 注: `last_sweep_height` は、最近のオンデ ロックの有効期限と持続性を反映します。 `expired_locks_now` は、`expiry_height <= height_current` のロック コム レジストリを計算します。
- POST `/v1/gov/ballots/zk-v1`
  - Requisicao (DTO estilo v1):
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
  - レスポスタ: { "ok": true、"accepted": true、"tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (機能: `zk-ballot`)
  - JSON `BallotProof` を参照し、`CastZkBallot` を参照してください。
  - 要求:
    {
      "権限": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      「投票用紙」: {
        "バックエンド": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // Base64 はコンテナ ZK1 または H2 を実行します*
        "root_hint": null, // オプションの 32 バイトの 16 進文字列 (資格ルート)
        "owner": null, // AccountId は、サーキットの所有者を任意に指定します
        "nullifier": null // オプションの 32 バイトの 16 進文字列 (nullifier ヒント)
      }
    }
  - レスポスタ:
    {
      「わかりました」: 本当、
      「受け入れられました」: true、
      "reason": "トランザクション スケルトンを構築する",
      "tx_instructions": [
        { "wire_id": "CastZkBallot"、"payload_hex": "..." }
      】
    }
  - 注意事項:
    - `root_hint`/`owner`/`nullifier` サーバーは、`public_inputs_json` および `CastZkBallot` に関する投票を行うことを選択します。
    - OS バイトは、エンベロープを再エンコードし、base64 パラメタやペイロードの命令を実行します。
    - レスポスタ `reason` ムダパラ `submitted transaction` クアンド Torii サブメテオ投票。
    - エステ エンドポイントなので、機能 `zk-ballot` を利用できます。

Caminho de verificacao CastZkBallot
- `CastZkBallot` は、base64 フォルネシダと不正な形式のペイロードを解読します (`BallotRejected` com `invalid or empty proof`)。
- ホストは検証を解決し、国民投票を投票し、国民投票を行います (`vk_ballot`) 統治者のデフォルトを確認し、レジストリの存在を確認し、インラインのコンテンツ バイトを確認します。
- Bytes de chave verificadora armazenados sao re-hasheados com `hash_vk`;不正な登録内容を確認するために、コミットメントが中止され、成人の登録内容が確認されません (`BallotRejected` com `verifying key commitment mismatch`)。
- `zk::verify_backend` 経由のバックエンド登録バイト、デスパチャドス、バックエンド登録。無効な転送は、`BallotRejected` com `invalid proof` の形式決定命令です。
- 証明では、投票コミットメントと資格ルートを公開入力として公開する必要があります。ルートは選挙の `eligible_root` と一致する必要があり、派生した無効化子は提供されたヒントと一致する必要があります。
- プロバス・ベム・スセディダス・エミテム`BallotAccepted`;無効化された重複ファイル、古い古いファイルの根、ロックの回帰を継続し、既存の文書の記述を再構築します。

## 検証とコンセンサスを両立させましょう

### Fluxo による斬撃と投獄O コンセンサスは、`Evidence` コードを Norito で検証し、ビオラとプロトコルを発行します。 Cada ペイロード チェガ `EvidenceStore` メモリ、編集、マテリアルなしの地図 `consensus_evidence` は WSV からの応答です。レジストロスは、`sumeragi.npos.reconfig.evidence_horizon_blocks` (デフォルトは `7200` ブロック) であり、オペランドのレジストラとレジストラの制限を保持しています。範囲内の証拠は、`sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) およびスラッシュ遅延 `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) も考慮します。ガバナンスは、スラッシュが適用される前に `CancelConsensusEvidencePenalty` でペナルティをキャンセルできます。

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind`; OS 判別データ モデル:

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

- **DoublePrepare/DoubleCommit** - メスモ タプル `(phase,height,view,epoch)` の検証アッシノウ ハッシュ競合。
- **InvalidQc** - うわさ話や、コミット証明書の形式的な変更や決定性の確認 (例: ビットマップの署名者)。
- **無効な提案** - リーダーがブロック ケ ファルハの有効性を提案しています (例: ロック チェーンの規則)。
- **検閲** - 署名された提出受領書には、決して提案/コミットされていないトランザクションが示されています。

オペレータ、ツール ポデムの検査、ペイロードの再ブロードキャストは次のとおりです。

- Torii: `GET /v1/sumeragi/evidence` および `GET /v1/sumeragi/evidence/count`。
- CLI: `iroha ops sumeragi evidence list`、`... count`、e `... submit --evidence-hex <payload>`。

政府は、正規の証拠となるバイトを管理します:

1. **コレタルまたはペイロード** 期限切れまで。 OS バイト Norito の高さ/ビューのメタデータをアーカイブします。
2. **ペナルティを準備します** ペイロード、国民投票、sudo の命令を埋め込みます (例、`Unregister::peer`)。ペイロードの再検証を実行します。証拠は不正であり、古いものであり、決定的な証拠です。
3. **アコンパンハメントに関する議題** は、有効性を主張するために、すぐに報告する必要があります。 Fluxos は、`SetParameter(Sumeragi::NextMode)` と `SetParameter(Sumeragi::ModeActivationHeight)` の名簿を登録します。
4. **監査結果** (`/v1/sumeragi/evidence` および `/v1/sumeragi/status` 経由) は、政府機関のアプリケーションや証拠を保証します。

### 合意の順序

保証を保証するためのコンセンサス、有効性を確認するための最終決定、前線のブロック、適切な新たな対応。ランタイム アプリケーションは、parametros parreados 経由でリグラを実行します。

- `SumeragiParameter::NextMode` と `SumeragiParameter::ModeActivationHeight` 開発者は **mesmo bloco** を確認していません。 `mode_activation_height` は、最も重要な問題を解決し、最新の情報を提供します。
- `sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) ハンドオフを妨げる設定をガードし、遅延をゼロにします。
- `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) はコンセンサス スラッシュを遅らせ、ペナルティが適用される前にガバナンスがキャンセルできるようにします。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- ランタイム、`/v1/sumeragi/params` および `iroha --output-format text ops sumeragi params` を介してステージングされた CLI のパラメータ説明、パラケ オペラドールの確認、有効性の確認、名簿の確認。
- 自動統治管理者:
  1. 証拠に関する決定を決定し、再統合します。
  2. com `mode_activation_height = h_current + activation_lag_blocks` で uma reconfiguracao を参照してください。
  3. モニター `/v1/sumeragi/status` が `effective_consensus_mode` トロカールを食べました。

Qualquer スクリプトの回転有効性とアップリケのスラッシュ ** ナオ デブ ** は、ラグ ゼロで動作し、ハンドオフのパラメータを省略します。旅は、前に進むことはできません。

## テレメトリの機能

- Metricas Prometheus 政府の輸出:
  - `governance_proposals_status{status}` (ゲージ) ステータスごとのラストレイア感染症。
  - `governance_protected_namespace_total{outcome}` (カウンター) ネームスペース プロテジドの追加許可をデプロイすることを許可します。
  - `governance_manifest_activations_total{event}` (カウンター) マニフェストの挿入レジストラ (`event="manifest_inserted"`)、名前空間のバインディング (`event="instance_bound"`)。
- `/status` には、提案のコンタジェンスとしてのオブジェクト `governance` のクエリ、名前空間のプロテギドと最近のマニフェストの関連情報 (名前空間、コントラクト ID、コード/ABI ハッシュ、ブロックの高さ、アクティブ化タイムスタンプ) が含まれます。オペラドールのポデム コンサルタントは、制定状況を確認するために、ネームスペースのゲートと同様に、アプリケーションを確立します。
- テンプレート Grafana (`docs/source/grafana_governance_constraints.json`) は、`telemetry.md` テレメトリアのランブックです。提案を作成する際のほとんどのアラート、マニフェストの有効性、およびランタイムの持続的なアップグレードの名前空間の管理を行います。