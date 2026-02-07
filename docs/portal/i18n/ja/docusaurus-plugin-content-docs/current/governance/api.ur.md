---
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

名前: گورننس نفاذی کاموں کے ساتھ چلنے والا ڈرافٹ/اسکیچ۔ عملدرآمد کے دوران ساختیں بدل سکتی ہیں۔決定論 RBAC پالیسی معیاری پابندیاں ہیں؛ جب `authority` اور `private_key` فراہم ہوں تو Torii ٹرانزیکشن سائن/سبمٹ کر سکتا ہے، ورنہ کلائنٹس بنا کر `/transaction` پر سبمٹ کرتے ہیں۔

ああ
- エンドポイント JSON を使用する`tx_instructions` شامل ہوتے ہیں - スケルトン スケルトン 指示配列:
  - `wire_id`: 命令レジストリ識別子
  - `payload_hex`: Norito ペイロード バイト (16 進数)
- 回答 `authority` 回答 `private_key` (投票 DTO 投票 `private_key`) 投票 Torii 投票 DTO 投票 `private_key` سبمٹ کرتا ہے اور پھر بھی `tx_instructions` واپس کرتا ہے۔
- 認証権限、chain_id 認証、SignedTransaction 認証、`/transaction` 認証、POST 認証やあ
- SDK の対象範囲:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` (ステータス/種類フィールドの正規化) `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` واپس کرتا ہے، `ToriiClient.get_governance_tally_typed` `GovernanceTally` واپس کرتا ہے، `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` واپس کرتا ہے،ガバナンス サーフェス入力されたアクセス ملتا ہے اور README میں 使用例 دیے گئے ہیں۔
- Python 軽量クライアント (`iroha_torii_client`): `ToriiClient.finalize_referendum` اور `ToriiClient.enact_proposal` 型付き `GovernanceInstructionDraft` バンドル واپس کرتے ہیں (Torii کی `tx_instructions` スケルトン ラップ スクリプト フローの完成/実行 手動 JSON 解析
- JavaScript (`@iroha/iroha-js`): `ToriiClient` 提案、住民投票、集計、ロック、統計のロック解除 `listGovernanceInstances(namespace, options)` 議会エンドポイントの型付きヘルパー(`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、`governancePersistCouncil`、`getGovernanceCouncilAudit`) Node.js クライアント `/v1/gov/instances/{ns}` ページ付けVRF を利用したワークフローの説明 コントラクト インスタンスのリストの説明

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
  - 検証: ノード فراہم کردہ `abi_version` کے `abi_hash` کو 正規化 کرتے ہیں اور 不一致 پر 拒否 کرتے ہیں۔ `abi_version = "v1"` の値 `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ہے۔

契約 API (デプロイ)
- POST `/v1/contracts/deploy`
  - リクエスト: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - 動作: IVM پروگرام باڈی سے `code_hash` اور ヘッダー `abi_version` سے `abi_hash` نکالتا ہے، پھر `RegisterSmartContractCode` (マニフェスト) `RegisterSmartContractBytes` (`.to` バイト) `authority` طرف سے سبمٹ کرتا ہے۔
  - 応答: { "ok": true、"code_hash_hex": "..."、"abi_hash_hex": "..." }
  - 関連:
    - GET `/v1/contracts/code/{code_hash}` -> マニフェスト ذخیرہ شدہ マニフェスト واپس کرتا ہے
    - GET `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }`
- POST `/v1/contracts/instance`
  - リクエスト: { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - 動作: バイトコード デプロイメント `ActivateContractInstance` マッピング `(namespace, contract_id)` マッピング
  - 応答: { "ok": true、"namespace": "apps"、"contract_id": "calc.v1"、"code_hash_hex": "..."、"abi_hash_hex": "..." }エイリアスサービス
- POST `/v1/aliases/voprf/evaluate`
  - リクエスト: { "blinded_element_hex": "..." }
  - 応答: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` エバリュエーターの実装値: `blake2b512-mock`۔
  - 注: 決定論的モック評価器 جو Blake2b512 کو ドメイン分離 `iroha.alias.voprf.mock.v1` کے ساتھ apply کرتا ہے۔テスト ツール テスト ツール プロダクション VOPRF パイプライン Iroha ワイヤ 接続 جائے۔
  - エラー: 不正な形式の 16 進数入力 HTTP `400`۔ Torii Norito `ValidationFail::QueryFailed::Conversion` エンベロープ اور デコーダ エラー メッセージ واپس کرتا ہے۔
- POST `/v1/aliases/resolve`
  - リクエスト: { "エイリアス": "GB82 WEST 1234 5698 7654 32" }
  - 応答: { "エイリアス": "GB82WEST12345698765432"、"アカウントID": "ih58..."、"インデックス": 0、"ソース": "iso_bridge" }
  - 注: ISO ブリッジ ランタイム ステージング (`iroha_config` の `[iso_bridge.account_aliases]`)۔ Torii ホワイトスペース ہٹا کر اور 大文字 بنا کر ルックアップ کرتا ہے۔エイリアス نہ ہو تو 404 اور ISO ブリッジ ランタイム بند ہو تو 503 دیتا ہے۔
- POST `/v1/aliases/resolve_index`
  - リクエスト: { "インデックス": 0 }
  - 応答: { "index": 0、"alias": "GB82WEST12345698765432"、"account_id": "ih58..."、"source": "iso_bridge" }
  - 注: エイリアス インデックスの構成順序 مطابق 決定的 طریقے سے assign ہوتے ہیں (0 ベース)۔オフライン キャッシュ エイリアス構成証明イベント 監査証跡

コードサイズの上限
- カスタムパラメータ: `max_contract_code_bytes` (JSON u64)
  - オンチェーンコントラクトコードストレージ (バイト数)
  - デフォルト: 16 MiB۔ノード `.to` イメージ `.to` ノード `RegisterSmartContractBytes` 不変違反エラー 拒否 拒否
  - 演算子 `SetParameter(Custom)` کے ذریعے `id = "max_contract_code_bytes"` اور 数値ペイロード دے کر ایڈجسٹ کر سکتے ہیں۔

- POST `/v1/gov/ballots/zk`
  - リクエスト: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注:
    - 回路のパブリック入力 `owner`、`amount`、`duration_blocks` 証明構成された VK ノードの検証`election_id` ガバナンス ロック セキュリティ ロック方向 چھپی رہتی ہے (`unknown`);数量/有効期限 اپڈیٹ ہوتے ہیں۔再投票単調 ہیں: 金額 期限切れ صرف بڑھتے ہیں (node max(amount, prev.amount) اور max(expiry, prev.expiry) لگاتا ہے)۔
    - ZK 再投票 金額 有効期限 サーバー側 `BallotRejected` 診断 拒否 ہوتے ہیں۔
    - 契約の実行 `SubmitBallot` エンキュー سے پہلے `ZK_VOTE_VERIFY_BALLOT` کال کرنا لازم ہے؛ホストはワンショット ラッチを強制します

- POST `/v1/gov/ballots/plain`
  - リクエスト: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 応答: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 注: 再投票は延長のみです - 投票用紙のロック、金額、有効期限、有効期限`owner` トランザクション権限 برابر ہونا چاہئے۔ `conviction_step_blocks` ہے۔

- POST `/v1/gov/finalize`
  - リクエスト: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58...?", "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - オンチェーン効果 (現在のスキャフォールド): 提案のデプロイ、制定、`code_hash` キー付き最小 `ContractManifest` 、 شامل ہوتا ہے جس میں متوقع `abi_hash` ہوتا ہے اور 提案 成立 ہو جاتا ہے۔ اگر `code_hash` کے لئے مختلف `abi_hash` والا マニフェスト پہلے سے ہو تو 制定案拒否 ہوتا ہے۔
  - 注:
    - ZK 選挙 契約パス `FinalizeElection` 接続 `ZK_VOTE_VERIFY_TALLY` 接続ホストはワンショット ラッチを強制します`FinalizeReferendum` ZK 国民投票 否決 ہے 選挙集計確定 جائے۔
    - 自動終了 `h_end` 単純な住民投票 承認/拒否の発行 کرتا ہے؛ ZK 住民投票終了 فہتے ہیں جب تک 最終集計提出 نہ ہو اور `FinalizeReferendum` 実行 نہ ہو۔
    - 投票率チェック承認+拒否棄権投票率 میں شمار نہیں ہوتا۔- POST `/v1/gov/enact`
  - リクエスト: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { " lower": 0, "upper": 0 }?、 "authority": "ih58…?"、 "private_key": "...?" }
  - 応答: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - 注: `authority`/`private_key` فراہم ہوں تو Torii 署名済みトランザクション سبمٹ کرتا ہے؛スケルトン واپس کرتا ہے جسے کلائنٹ سائن اور سبمٹ کرے۔プリ画像 اختیاری اور فی الحال معلوماتی ہے۔

- `/v1/gov/proposals/{id}` を取得
  - パス `{id}`: プロポーザル ID 16 進数 (64 文字)
  - 応答: { "見つかった": bool、"提案": { ... }? }

- `/v1/gov/locks/{rid}` を取得
  - パス `{rid}`: 国民投票 ID 文字列
  - 応答: { "found": bool、"referendum_id": "rid"、"locks": { ... }? }

- `/v1/gov/council/current` を取得
  - 応答: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - 注: 評議会の決定 設定されたステーク資産のしきい値 確定的フォールバックの導出 (VRF 仕様ライブ VRF プルーフをオンチェーンで永続的に反映します。

- POST `/v1/gov/council/derive-vrf` (機能: gov_vrf)
  - リクエスト: { "committee_size": 21、"epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 動作: VRF 証明 `chain_id`、`epoch` ブロック ハッシュ ビーコン、正規入力、検証やあ出力バイト、記述、タイブレーカー、ソート、分類トップ `committee_size` メンバー واپس کرتا ہے۔続けてください
  - 応答: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - 注: 通常 = G1 で pk、G2 で校正 (96 バイト)。 Small = G2 の pk、G1 の校正 (48 バイト)。入力ドメイン区切りの ہیں اور `chain_id` شامل ہے۔

### ガバナンスのデフォルト (iroha_config `gov.*`)

Torii 永続化された名簿の保持とフォールバック評議会 `iroha_config` のパラメータ化 パラメータ化:

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

同等の環境は次のようにオーバーライドします。

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

`parliament_committee_size` フォールバック メンバーの制限制限 `parliament_term_blocks` エポック長の定義 シード導出(`epoch = floor(height / term_blocks)`) `parliament_min_stake` 適格資産 ステーク (最小単位) が適用されます `parliament_eligibility_asset_id`候補者を確認し、資産残高をスキャンし、スキャンします。

ガバナンス VK 検証 バイパス 投票: 投票用紙検証 `Active` キー検証 インライン バイト 環境 テスト専用トグルحصار نہیں کرنا چاہئے۔

RBAC
- オンチェーン実行の権限の説明:
  - 提案: `CanProposeContractDeployment{ contract_id }`
  - 投票用紙: `CanSubmitGovernanceBallot{ referendum_id }`
  - 制定: `CanEnactGovernance`
  - 評議会運営（将来）：`CanManageParliament`

保護された名前空間
- カスタム パラメータ `gov_protected_namespaces` (文字列の JSON 配列) リストされた名前空間
- クライアントは、保護された名前空間をデプロイし、トランザクション メタデータ キーを作成します。
  - `gov_namespace`: ターゲット名前空間 (意味: "アプリ")
  - `gov_contract_id`: 名前空間の論理コントラクト ID
- `gov_manifest_approvers`: バリデーターアカウント ID のオプションの JSON 配列。レーンマニフェスト定足数 > 1 宣言定足数 入場許可 取引権限 リストされたアカウント دونوں درکار ہوتے ہیں マニフェスト定足数 پورا ہو سکے۔
- テレメトリー `governance_manifest_admission_total{result}` 総合入場カウンター オペレーターが許可 `missing_manifest`、`non_validator_authority`、 `quorum_rejected`、`protected_namespace_rejected`、`runtime_hook_rejected` سے فرق کر سکیں۔
- テレメトリ `governance_manifest_quorum_total{outcome}` (値 `satisfied` / `rejected`) 施行パス دکھاتی ہے 承認の欠落 監査 ہو سکے۔
- レーンはマニフェストをマニフェストし、名前空間の許可リストを適用します。名前空間 `gov_namespace` 名前空間 `protected_namespaces` سیٹ میں ہونا چاہئے۔ `RegisterSmartContractCode` 提出物 メタデータ بغیر 保護を有効にする 拒否する ہیں۔
- 入学許可、施行、施行、`(namespace, contract_id, code_hash, abi_hash)`、ガバナンス提案の制定、施行。検証 NotPermitted エラーが発生しました 失敗しましたランタイムアップグレードフック
- レーン マニフェスト `hooks.runtime_upgrade` は、ランタイム アップグレード命令 (`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、`CancelRuntimeUpgrade`) を宣言します。
- フックフィールド:
  - `allow` (ブール値、デフォルト `true`): `false` ランタイム アップグレード命令が拒否されました。
  - `require_metadata` (ブール値、デフォルト `false`): `metadata_key` کے مطابق メタデータ エントリ درکار ہے۔
  - `metadata_key` (文字列): フックの強制メタデータ名デフォルト `gov_upgrade_id` メタデータが必要です 許可リストが必要です
  - `allowed_ids` (文字列の配列): メタデータ値、オプションの許可リスト (トリミング)評価値 評価 評価 拒否 拒否
- フック キューの入場許可 キューのセキュリティ キュー セキュリティ メタデータ ポリシーの適用 メタデータ ポリシーの適用メタデータが欠落しています。値がホワイトリストにあります。値が確定的 NotPermitted エラーです。
- テレメトリー `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` 結果の追跡
- フック メタデータ `gov_upgrade_id=<value>` (マニフェスト定義キー) マニフェスト定足数 マニフェスト定足数バリデーターの承認数

コンビニエンスエンドポイント
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` を適用します。
  - リクエスト: { "名前空間": ["アプリ", "システム"] }
  - 応答: { "ok": true、"applied": 1 }
  - 注: 管理者/テスト中API トークンを構成する生産 `SetParameter(Custom)` 署名済みトランザクション ترجیح دیں۔

CLI ヘルパー
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 名前空間のコントラクト インスタンスのフェッチとクロスチェックの実行:
    - Torii ہر `code_hash` کے لئے バイトコード ذخیرہ کرتا ہے، اور اس کا Blake2b-32 ダイジェスト `code_hash` سے match کرتا ❁❁❁❁
    - `/v1/contracts/code/{code_hash}` میں موجود マニフェストと一致する `code_hash` اور `abi_hash` 値 رپورٹ کرتا ہے۔
    - `(namespace, contract_id, code_hash, abi_hash)` 制定されたガバナンス提案 موجود ہے جو اسی 提案 ID ハッシュ化 سے 導出 ہوتا ہے جو ノード استعمال کرتا ہے۔
  - `results[]` کے ساتھ JSON رپورٹ دیتا ہے (問題、マニフェスト/コード/提案の概要) اور ایک لائن کا خلاصہ (اگر `--no-summary` نہ) ہو)۔
  - 保護された名前空間 監査 ガバナンス管理されたデプロイ ワークフロー 管理
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - 保護された名前空間のデプロイメント JSON メタデータ スケルトン オプションの `gov_manifest_approvers` マニフェスト クォーラム ルール پوری ہوں۔
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` ہونے پر ロックのヒント لازم ہیں، اور فراہم کیے گئے کسی بھی ヒント سیٹ میں `owner`、`amount` 、 `duration_blocks` 、 شامل ہونا ضروری ہے۔
  - 正規アカウント ID を検証し、32 バイトの nullifier ヒントを正規化し、ヒントを `public_inputs_json` (追加のオーバーライド用に `--public <path>`) にマージします。
  - 無効化子は、証明コミットメント (パブリック入力) に `domain_tag`、`chain_id`、および `election_id` を加えたものから導出されます。 `--nullifier` は、提供されたときに証明に対して検証されます。
  - 決定的 `fingerprint=<hex>` エンコードされた `CastZkBallot` 解読されたヒントの導出(`owner`、`amount`、`duration_blocks`、`direction` جب فراہم ہوں)۔
  - CLI 応答 `tx_instructions[]` `payload_fingerprint_hex` デコードされたフィールドの注釈付け ダウンストリーム ツールのスケルトン Norito デコードの検証ありがとうございます
  - ロックのヒント ノード ZK の投票結果 `LockCreated`/`LockExtended` イベントが放出される 回路の値が公開される
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` エイリアス ZK フラグ کے ناموں کو ミラー کرتے ہیں تاکہ スクリプト パリティ ہو۔
  - 概要出力 `vote --mode zk` エンコードされた命令フィンガープリント、読み取り可能な投票フィールド (`owner`、`amount`、`duration_blocks`、`direction`) ہے، جس سے 署名 سے پہلے فوری تصدیق ہو جاتی ہے۔

インスタンスのリスト
- GET `/v1/gov/instances/{ns}` - 名前空間 アクティブなコントラクト インスタンス
  - クエリパラメータ:
    - `contains`: `contract_id` 部分文字列フィルタ (大文字と小文字を区別します)
    - `hash_prefix`: `code_hash_hex` 16 進数プレフィックス مطابق フィルター (小文字)
    - `offset` (デフォルト 0)、`limit` (デフォルト 100、最大 10_000)
    - `order`: `cid_asc` (デフォルト)、`cid_desc`、`hash_asc`、`hash_desc`
  - 応答: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK ヘルパー: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript)、`ToriiClient.list_governance_instances_typed("apps", ...)` (Python)、ロック解除スイープ (オペレーター/監査)
- `/v1/gov/unlocks/stats` を取得
  - 応答: { "height_current": H、"expired_locks_now": n、"referenda_with_expired": m、"last_スイープ_高さ": S }
  - メモ: `last_sweep_height` ブロックの高さ ブロックの高さ دکھاتا ہے 期限切れのロックのスイープ اور 永続化 کئے گئے۔ `expired_locks_now` ロック レコードをスキャンします。 `expiry_height <= height_current` ہو۔
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
  - `BallotProof` JSON 文字列 `CastZkBallot` スケルトン واپس کرتا ہے۔
  - リクエスト:
    {
      "権限": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      「投票用紙」: {
        "バックエンド": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // ZK1 、 H2* コンテナ、base64
        "root_hint": null, // オプションの 32 バイトの 16 進文字列 (資格ルート)
        "owner": null, // オプションの AccountId と回線所有者のコミット
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
    - サーバーはオプション `root_hint`/`owner`/`nullifier` 投票用紙 `CastZkBallot` 地図 `public_inputs_json` 地図
    - エンベロープバイト、命令ペイロード、base64 エンコード、命令ペイロード、エンコード
    - جب Torii 投票用紙の提出 ہے تو `reason` بدل کر `submitted transaction` ہو جاتا ہے۔
    - エンドポイントの確認 دستیاب ہے جب `zk-ballot` 機能が有効になりました ہو۔

CastZkBallot 検証パス
- `CastZkBallot` Base64 証明デコード ペイロード拒否 (`BallotRejected` `invalid or empty proof`)。
- 主催者住民投票 (`vk_ballot`) ガバナンスのデフォルト、投票用紙の検証、主要な解決の結果、記録、`Active` ہو، اورインラインバイト
- 保存された検証キー バイト `hash_vk` ハッシュ ハッシュコミットメントの不一致、検証、実行、レジストリ エントリの改ざん、(`BallotRejected` と `verifying key commitment mismatch`)。
- 証明バイト `zk::verify_backend` 登録されたバックエンド ディスパッチ ہیں؛無効なトランスクリプト `BallotRejected` と `invalid proof` の命令は決定的に失敗します。
- 証明では、投票コミットメントと資格ルートを公開入力として公開する必要があります。ルートは選挙の `eligible_root` と一致する必要があり、派生した無効化子は提供されたヒントと一致する必要があります。
- 証明が成功すると `BallotAccepted` が出力されます。重複した無効化子、古い資格のルート、ロックの回帰、拒否の理由、拒否の理由、ロックの回帰

## バリデーターの不正行為に関する共同合意

### 斬撃、投獄のワークフロー

コンセンサス バリデータ テスト Norito でエンコードされた `Evidence` が出力されます。メモリ内のペイロード `EvidenceStore` میں آتا ہے اور اگر پہلے نہ دیکھا گیا ہو تو WSV でバックアップされた `consensus_evidence` マップ میں 実体化 ہوやあ`sumeragi.npos.reconfig.evidence_horizon_blocks` (デフォルトの `7200` ブロック) 拒否 拒否 拒否 アーカイブ 制限付きアーカイブ 拒否 演算子 ログ٩یا جاتا ہے۔範囲内の証拠は、`sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) およびスラッシュ遅延 `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) も考慮します。ガバナンスは、スラッシュが適用される前に、`CancelConsensusEvidencePenalty` でペナルティをキャンセルできます。

認識された違反 `EvidenceKind` 1 対 1 マップ ہوتے ہیں؛判別式のデータ モデルの強制執行:

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
```- **DoublePrepare/DoubleCommit** - バリデーター نے اسی `(phase,height,view,epoch)` کے لئے متضاد ハッシュ پر دستخط کئے۔
- **InvalidQc** - アグリゲーターが証明書のゴシップをコミットする 決定論的チェックが失敗する (署名者ビットマップ)
- **InvalidProposal** - リーダーが提案しました 構造検証が失敗しました (ロック チェーン ルールが失敗しました)۔
- **検閲** - 署名された提出受領書には、決して提案/コミットされていないトランザクションが示されています。

オペレーターは、ペイロードのツールを作成し、再ブロードキャストを検査し、次の作業を行います。

- Torii: `GET /v1/sumeragi/evidence` または `GET /v1/sumeragi/evidence/count`。
- CLI: `iroha ops sumeragi evidence list`、`... count`、`... submit --evidence-hex <payload>`。

ガバナンスと証拠バイトと正規証明と処理との関係:

1. **ペイロードの期限** 期限切れの期限 پہلے۔ raw Norito バイト 高さ/表示メタデータ アーカイブ アーカイブ
2. **ペナルティステージ** ペイロード 国民投票 sudo 命令 埋め込み (مثلا `Unregister::peer`)۔実行ペイロードの検証不正な形式の古い証拠は決定的に拒否します。
3. **フォローアップ トポロジ スケジュール** 問題のあるバリデータを確認しました。処理フロー `SetParameter(Sumeragi::NextMode)` اور `SetParameter(Sumeragi::ModeActivationHeight)` 更新された名簿 کے ساتھ キュー کئے جاتے ہیں۔
4. **監査、監査、** `/v1/sumeragi/evidence`、`/v1/sumeragi/status`、証拠カウンター、ガバナンス、削除、削除

### 共同合意配列決定

共同コンセンサス 送信バリデータ設定 境界ブロックの終了 完了 提案設定 完了実行時のパラメータ、ルールの適用、およびルールの適用:

- `SumeragiParameter::NextMode` اور `SumeragiParameter::ModeActivationHeight` کو **اسی بلاک** میں commit ہونا چاہیے۔ `mode_activation_height` 更新情報、高さ、正確な情報、正確な情報、遅れ、遅延、遅延など
- `sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) 構成ガード: ゼロラグ ハンドオフ:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) はコンセンサス スラッシュを遅らせ、ペナルティが適用される前にガバナンスがキャンセルできるようにします。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- ランタイム CLI ステージング パラメータ `/v1/sumeragi/params` `iroha --output-format text ops sumeragi params` ステータス オペレータ アクティベーション高さ バリデータ リストصدیق کر سکیں۔
- ガバナンスの自動化:
  1. 証拠の削除 (復元) 確定する
  2. `mode_activation_height = h_current + activation_lag_blocks` フォローアップ再構成キュー کرنا چاہیے۔
  3. `/v1/sumeragi/status` نگرانی کرنی چاہیے جب تک `effective_consensus_mode` متوقع 高さ پر switch نہ ہو جائے۔

スクリプトバリデータの回転 スラッシング適用 **ゼロラグアクティベーション** ハンドオフパラメータ 省略トランザクション拒否 ہو جاتی ہیں اور نیٹ ورک پچھلے モード میں رہتا ہے۔

## テレメトリ サーフェス

- Prometheus メトリクス ガバナンス アクティビティのエクスポート:
  - `governance_proposals_status{status}` (ゲージ) 提案 ステータス ステータス トラック トラック
  - `governance_protected_namespace_total{outcome}` (カウンタ) 増加 増加 ہوتا ہے 保護された名前空間 承認 デプロイ 許可 拒否 拒否
  - `governance_manifest_activations_total{event}` (カウンター) マニフェスト挿入 (`event="manifest_inserted"`) 名前空間バインディング (`event="instance_bound"`) レコード
- `/status` オブジェクト `governance` オブジェクトのプロポーザル数、保護された名前空間の合計レポート、最近のマニフェスト アクティベーション (名前空間、コントラクト ID、コード/ABI ハッシュ、ブロック高さ、アクティベーション)タイムスタンプ) リスト演算子 フィールド 投票 投票 評価 制定 マニフェスト 保護された名前空間ゲート فیں۔
- Grafana テンプレート (`docs/source/grafana_governance_constraints.json`) `telemetry.md` テレメトリ Runbook の問題、提案のスタック、マニフェストのアクティブ化の不足、 ランタイムのアップグレード、予期しない保護された名前空間の拒否アラート ワイヤー 連絡先