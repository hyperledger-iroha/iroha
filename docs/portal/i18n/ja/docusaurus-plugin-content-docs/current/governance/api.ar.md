---
lang: ja
direction: ltr
source: docs/portal/docs/governance/api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

翻訳: مسودة/تصور لمرافقة مهام تنفيذ الحوكمة.最高のパフォーマンスを見せてください。 RBAC قيود معيارية؛ يمكن لتوريي توقيع/ارسال المعاملات عندما يتم توفير `authority` و`private_key`، والا يبني العملاء `/transaction` です。

ログインしてください
- JSON 形式。重要な情報: `tx_instructions` - 重要な情報:
  - `wire_id`: معرّف السجل لنوع التعليمة
  - `payload_hex`: Norito (16 進数)
- 回答 `authority` و`private_key` (`private_key` DTO 投票) Torii 評価`tx_instructions`。
- 認証済み SignedTransaction 認証権限 وchain_id 認証 認証済み POST 認証 `/transaction`。
- SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` يعيد `GovernanceProposalResult` (ステータス/種類) و`ToriiClient.get_governance_referendum_typed` يعيد `GovernanceReferendumResult`، و`ToriiClient.get_governance_tally_typed` يعيد `GovernanceTally`، و`ToriiClient.get_governance_locks_typed` يعيد `GovernanceLocksResult`، و`ToriiClient.get_governance_unlock_stats_typed` يعيد `GovernanceUnlockStats`، و`ToriiClient.list_governance_instances_typed` يعيد `GovernanceInstancesPage`، فارضا وصولا بنمط と入力した عبر سطح الحوكمة مع امثلة استخدام في README。
- Python خفيف (`iroha_torii_client`): `ToriiClient.finalize_referendum` و`ToriiClient.enact_proposal` يعيدان حزم `GovernanceInstructionDraft` 型付き (تغلف هيكل `tx_instructions` من Torii)، لتجنب تحليل JSON اليدوي عند تركيب سكربتات 確定/制定。
- JavaScript (`@iroha/iroha-js`): `ToriiClient` يعرض ヘルパーは、次のように入力しました。 `listGovernanceInstances(namespace, options)` 評議会 (`getGovernanceCouncilCurrent`、`governanceDeriveCouncilVrf`、`governancePersistCouncil`、`getGovernanceCouncilAudit`) は、Node.js を評価します。 `/v1/gov/instances/{ns}` が VRF をサポートします。

ナタリー

- POST `/v1/gov/proposals/deploy-contract`
  - 説明 (JSON):
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
  - ファイル (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - 対応: `abi_hash` 対応 `abi_version` 対応。 ـ `abi_version = "v1"` القيمة المتوقعة هي `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`。

API (デプロイ)
- POST `/v1/contracts/deploy`
  - 説明: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - 説明: `code_hash` セキュリティ IVM و`abi_hash` セキュリティ `abi_version` セキュリティ`RegisterSmartContractCode` (マニフェスト) و`RegisterSmartContractBytes` (`.to` كاملة) نيابة عن `authority`。
  - 説明: { "ok": true、"code_hash_hex": "..."、"abi_hash_hex": "..." }
  - 連絡先:
    - GET `/v1/contracts/code/{code_hash}` -> マニフェストを取得します
    - GET `/v1/contracts/code-bytes/{code_hash}` -> يعيد `{ code_b64 }`
- POST `/v1/contracts/instance`
  - 説明: { "authority": "ih58..."、"private_key": "..."、"namespace": "apps"、"contract_id": "calc.v1"、"code_b64": "..." }
  - 表示: `(namespace, contract_id)` および `ActivateContractInstance` 。
  - 説明: { "ok": true、"namespace": "apps"、"contract_id": "calc.v1"、"code_hash_hex": "..."、"abi_hash_hex": "..." }

認証済み
- POST `/v1/aliases/voprf/evaluate`
  - 説明: { "blinded_element_hex": "..." }
  - 説明: { "evaluated_element_hex": "...128hex", "バックエンド": "blake2b512-mock" }
    - `backend` يعكس تنفيذ المقيم。番号: `blake2b512-mock`。
  - ملاحظات: مقيم 模擬 حتمي يطبق Blake2b512 مع فصل مجال `iroha.alias.voprf.mock.v1`。ログインしてください。VOPRF は Iroha です。
  - HTTP `400` 16 進数。 Torii Norito `ValidationFail::QueryFailed::Conversion` デコーダ。
- POST `/v1/aliases/resolve`
  - 名前: { "エイリアス": "GB82 WEST 1234 5698 7654 32" }
  - 名前: { "エイリアス": "GB82WEST12345698765432", "アカウント ID": "ih58...", "インデックス": 0, "ソース": "iso_bridge" }
  - ISO ブリッジ (`[iso_bridge.account_aliases]` في `iroha_config`)。 يقوم Torii بتطبيع الاسماء عبر ازالة الفراغات وتحويلها الى احرف كبيرة قبل البحث。 يعيد 404 عندما يكون الاسم غير موجود و503 عندما يكون ランタイム الخاص بـ ISO ブリッジ معطلا。
- POST `/v1/aliases/resolve_index`
  - 説明: { "インデックス": 0 }
  - 説明: { "インデックス": 0、"エイリアス": "GB82WEST12345698765432"、"アカウントID": "ih58..."、"ソース": "iso_bridge" }
  - ملاحظات: مؤشرات الاسماء تعين بشكل حتمي حسب ترتيب التكوين (0 ベース)。オフラインで認証を行うことができます。حد حجم الكود
- 名前: `max_contract_code_bytes` (JSON u64)
  - يتحكم في الحد الاقصى المسموح (بالبايت) لتخزين كود العقود على السلسلة。
  - バージョン: 16 MiB。 `RegisterSmartContractBytes` は不変です。
  - يمكن للمشغلين التعديل عبر `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` وحمولة رقمية.

- POST `/v1/gov/ballots/zk`
  - 説明: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - 説明: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 重要:
    - عندما تتضمن المدخلات العامة للدائرة `owner` و`amount` و`duration_blocks`، وتتحقق البرهان من VKロックを解除してください。`election_id` と `owner` をロックしてください。評価 (`unknown`)金額/有効期限 فقط。単調: 金額 وexpiry تزداد فقط (تطبق العقدة max(amount, prev.amount) وmax(expiry, prev.expiry))。
    - 金額と有効期限を確認してください。`BallotRejected`。
    - يجب على تنفيذ العقد استدعاء `ZK_VOTE_VERIFY_BALLOT` قبل ادراج `SubmitBallot`;ラッチがかかっています。

- POST `/v1/gov/ballots/plain`
  - 説明: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - 説明: { "ok": true、"accepted": true、"tx_instructions": [{...}] }
  - 重要: 投票数 - 投票数、有効期限、金額。 يجب ان يساوي `owner` سلطة المعاملة。 `conviction_step_blocks` です。

- POST `/v1/gov/finalize`
  - 説明: { "referendum_id": "r1"、"proposal_id": "...64hex"、"authority": "ih58...?"、"private_key": "...?" }
  - 説明: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - バージョン (バージョン): تنفيذ اقتراحdeploy معتمد يدرج `ContractManifest` ادنى بمفتاح `code_hash` مع `abi_hash` المتوقع ويضع الاقتراح بحالة 制定されました。マニフェスト `code_hash` `abi_hash` を確認してください。
  - 重要:
    - ،خابات ZK، يجب على مسارات العقد استدعاء `ZK_VOTE_VERIFY_TALLY` قبل تنفيذ `FinalizeElection`;ラッチがかかっています。 يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء 集計 للانتخابات。
    - 承認/拒否 承認/拒否 平文ZK は閉店しました、`FinalizeReferendum`。
    - 投票率 承認+拒否 فقط؛投票率を棄権する。

- POST `/v1/gov/enact`
  - 説明: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { " lower": 0, "upper": 0 }?、 "authority": "ih58...?"、 "private_key": "...?" }
  - 説明: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - メッセージ: يقدم Torii الموقعة عندما تتوفر `authority`/`private_key`; هيكلا لتوقيع العملاء وارساله。プリ画像 اختيارية وحاليا معلوماتية。

- `/v1/gov/proposals/{id}` を取得
  - `{id}`: 16 進数 (64 文字)
  - 説明: { "見つかった": bool、"提案": { ... }? }

- `/v1/gov/locks/{rid}` を取得
  - 文字列 `{rid}`: 文字列
  - 説明: { "found": bool、"referendum_id": "rid"、"locks": { ... }? }

- `/v1/gov/council/current` を取得
  - 説明: { "エポック": N, "メンバー": [{ "アカウント ID": "..." }, ...] }
  - ملاحظات: يعيد Council المحفوظ اذا كان موجودا؛ والا يشتق بديلا حتميا باستخدام اصل stake المضبوط والعتبات (يعكس مواصفات VRF حتى تثبت ادلة VRF (英語)。

- POST `/v1/gov/council/derive-vrf` (機能: gov_vrf)
  - 説明: { "committee_size": 21、"epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - 説明: VRF の評価 - 評価 `chain_id` و`epoch`ハッシュ値バイト数 説明 説明ويعيد اعلى `committee_size` من الاعضاء。ああ、そうです。
  - 形式: { "エポック": N、"メンバー": [{ "アカウント ID": "..." } ...]、"合計候補数": M、"検証済み": K }
  - 意味: 通常 = PK と G1、証明と G2 (96 バイト)。 Small = PK G2 証明 G1 (48 バイト)。 `chain_id` です。

### افتراضات الحوكمة (iroha_config `gov.*`)

評議会 الاحتياطي الذي يستخدمه Torii عندما لا يوجد 名簿 محفوظ عبر `iroha_config`:

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

回答:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
````parliament_committee_size` يحد عدد اعضاء フォールバック المعادين عندما لا يوجد Council محفوظ، و`parliament_term_blocks` يحدد طول الحقبةシード (`epoch = floor(height / term_blocks)`) と `parliament_min_stake` は、ステーク (`epoch = floor(height / term_blocks)`) を取得します。 عرض المزيد بناء مجموعة المرشحين.

VK バイパス: 投票用紙 يتطلب دائما مفتاح تحقق `Active` ببايتات inline، ولا يجب انを確認してください。

RBAC
- 回答:
  - 提案: `CanProposeContractDeployment{ contract_id }`
  - 投票用紙: `CanSubmitGovernanceBallot{ referendum_id }`
  - 制定: `CanEnactGovernance`
  - 評議会運営 (مستقبلا): `CanManageParliament`

名前空間
- `gov_protected_namespaces` (JSON 配列文字列) ゲート制御、名前空間のデプロイ。
- メタデータの作成と名前空間のデプロイ:
  - `gov_namespace`: 名前空間 الهدف (「アプリ」)
  - `gov_contract_id`: 名前空間
- `gov_manifest_approvers`: アカウント ID の JSON 配列。マニフェスト マニフェスト 定足数 定足数 入学許可 入学許可 入学許可定足数のマニフェストを表示します。
- 入学許可 `governance_manifest_admission_total{result}` 入学許可 `missing_manifest` و`non_validator_authority` و`quorum_rejected` و`protected_namespace_rejected` و`runtime_hook_rejected`。
- 施行規則 `governance_manifest_quorum_total{outcome}` (`satisfied` / `rejected`) の施行ありがとうございます。
- 許可リストと名前空間マニフェストのリスト。 `gov_namespace` يجب ان توفر `gov_contract_id`، ويجب ان يظهر 名前空間 في مجموعة `protected_namespaces` للmanifest。 `RegisterSmartContractCode` メタデータが含まれています。
- 入学許可 اقتراح حوكمة 制定 للتركيبة `(namespace, contract_id, code_hash, abi_hash)`؛許可されていません。

フックランタイム
- マニフェスト `hooks.runtime_upgrade` ランタイム (`ProposeRuntimeUpgrade`、`ActivateRuntimeUpgrade`、 `CancelRuntimeUpgrade`)。
- フック:
  - `allow` (ブール値、`true`): `false` ランタイム。
  - `require_metadata` (ブール値、`false`): メタデータ `metadata_key`。
  - `metadata_key` (文字列): フックのメタデータ。 `gov_upgrade_id` メタデータ許可リスト。
  - `allowed_ids` (配列文字列): ホワイトリスト メタデータ (トリム)。 عندما لا تكون القيمة المقدمة مدرجة.
- フック、入場、メタデータ、メタデータ。メタデータの許可リストの許可リストの許可なし。
- 評価 `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`。
- メタデータ `gov_upgrade_id=<value>` (マニフェスト) メタデータ `gov_upgrade_id=<value>` (マニフェスト)マニフェストの定足数。

エンドポイントの説明
- POST `/v1/gov/protected-namespaces` - يطبق `gov_protected_namespaces` مباشرة على العقدة。
  - 説明: { "名前空間": ["アプリ", "システム"] }
  - 結果: { "ok": true、"適用済み": 1 }
  - ملاحظات: مخصص للادارة/الاختبار؛ API トークンを使用します。 `SetParameter(Custom)` を参照してください。CLI の使用
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - 名前空間の名前空間:
    - Torii バイトコード `code_hash` ダイジェスト Blake2b-32 `code_hash`。
    - マニフェスト `/v1/contracts/code/{code_hash}` يبلغ بقيم `code_hash` و`abi_hash` متطابقة。
    - يوجد اقتراح حوكمة は、`(namespace, contract_id, code_hash, abi_hash)` مشتق بنفس ハッシュ提案 ID を制定しました。
  - يخرج تقرير JSON يحتوي `results[]` لكل عقد (issues ملخصات マニフェスト/コード/提案) بالاضافة الى ملخص سطر واحد الا ذا تم تعطيله (`--no-summary`)。
  - 名前空間を展開し、展開します。
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - JSON メタデータの展開、名前空間のデプロイ、`gov_manifest_approvers` の名前空間の展開マニフェストの定足数を満たしています。
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تلميحات القفل مطلوبة عندما يكون `min_bond_amount > 0`، وأي مجموعة تلميحات مقدمة يجب أن `owner` و`amount` و`duration_blocks`。
  - 正規アカウント ID を検証し、32 バイトの nullifier ヒントを正規化し、ヒントを `public_inputs_json` (追加のオーバーライド用に `--public <path>`) にマージします。
  - 無効化子は、証明コミットメント (パブリック入力) に `domain_tag`、`chain_id`、および `election_id` を加えたものから導出されます。 `--nullifier` は、提供されたときに証明に対して検証されます。
  - الملخص في سطر واحد يعرض الان `fingerprint=<hex>` حتميا مشتقا من `CastZkBallot` المشفر مع ヒント المفككة (`owner`、`amount`、`duration_blocks`、`direction`)。
  - CLI の評価 تعليقات على `tx_instructions[]` مع `payload_fingerprint_hex` بالاضافة الى حقول مفكوكة كي تتحقق Norito を参照してください。
  - ロックのヒント、`LockCreated`/`LockExtended`、ZK の投票用紙のロックのヒントああ、それは。
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - エイリアス `--lock-amount`/`--lock-duration-blocks` は、フラグと ZK を示します。
  - 指紋認証 `vote --mode zk` 指紋認証 投票用紙 (`owner`, `amount`、`duration_blocks`、`direction`) を参照してください。

और देखें
- GET `/v1/gov/instances/{ns}` - 名前空間を取得します。
  - クエリパラメータ:
    - `contains`: 部分文字列 `contract_id` (大文字と小文字が区別されます)
    - `hash_prefix`: 16 進数、`code_hash_hex` (小文字)
    - `offset` (デフォルト 0)、`limit` (デフォルト 100、最大 10_000)
    - `order`: `cid_asc` (デフォルト)、`cid_desc`、`hash_asc`、`hash_desc`
  - 説明: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - ヘルパー SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) と `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)。

مسح ロック解除 (المشغل/التدقيق)
- `/v1/gov/unlocks/stats` を取得
  - 説明: { "height_current": H、"expired_locks_now": n、"referenda_with_expired": m、"last_sweet_height": S }
  - メッセージ: `last_sweep_height` يعكس اخر ارتفاع بلوك تم فيه مسح はロックを解除します。 `expired_locks_now` は、ロック `expiry_height <= height_current` をロックします。
- POST `/v1/gov/ballots/zk-v1`
  - バージョン (DTO バージョン v1):
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
  - 説明: { "ok": true、"accepted": true、"tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (機能: `zk-ballot`)
  - JSON `BallotProof` 、 `CastZkBallot` 。
  - 説明:
    {
      "権限": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "秘密キー": "...?",
      "election_id": "ref-1",
      「投票用紙」: {
        "バックエンド": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // Base64 لحاوية ZK1 او H2*
        "root_hint": null, // オプションの 32 バイトの 16 進文字列 (資格ルート)
        "owner": null, // AccountId اختياري عندما تلتزم الدائرة بـ 所有者
        "nullifier": null // オプションの 32 バイトの 16 進文字列 (nullifier ヒント)
      }
    }
  - 名前:
    {
      「わかりました」: 本当、
      「受け入れられました」: true、
      "reason": "トランザクション スケルトンを構築する",
      "tx_instructions": [
        { "wire_id": "CastZkBallot"、"payload_hex": "..." }
      】
    }
  - 重要:
    - ログイン `root_hint`/`owner`/`nullifier` 投票用紙 `public_inputs_json` `CastZkBallot`。
    - バイト数とエンベロープ、base64、ペイロード数。
    - 投票 `reason` 投票 `submitted transaction` 投票 Torii 投票。
    - エンドポイント متاح فقط عندما يكون 機能 `zk-ballot` مفعلا。

重要なキャストZkBallot
- `CastZkBallot` يفك ترميز برهان base64 المقدم ويرفض الحمولات الفارغة او المشوهة (`BallotRejected` مع `invalid or empty proof`)。
- 投票用紙国民投票 (`vk_ballot`) 投票結果 - 投票結果 (`vk_ballot`) 投票結果インラインのバイト数。
- バイト ハッシュ `hash_vk`; عدم تطابق للالتزام يوقف التنفيذ قبل التحقق للحماية من ادخالات سجل معبث بها (`BallotRejected` と `verifying key commitment mismatch`)。
- バイト、バックエンド、`zk::verify_backend`; `BallotRejected` `invalid proof` を確認してください。
- 証明では、投票コミットメントと資格ルートを公開入力として公開する必要があります。ルートは選挙の `eligible_root` と一致する必要があり、派生した無効化子は提供されたヒントと一致する必要があります。
- 回答: `BallotAccepted`; nullifiers を無効化します。 無効化します。 無効化します。 هذا المستند.

## سوء سلوك المدققين والتوافق المشترك

### 斬撃 投獄

صدر الاجماع `Evidence` مرمزا بـ Norito عندما ينتهك مدقق البروتوكول。評価 `EvidenceStore` 評価 `consensus_evidence` WSV です。 يتم رفض السجلات الاقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks` (الافتراضي `7200` بلوك) كي يبقى الارشيف محدودا، لكن और देखें範囲内の証拠は、`sumeragi.npos.reconfig.activation_lag_blocks` (デフォルト `1`) およびスラッシュ遅延 `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) も考慮します。ガバナンスは、スラッシュが適用される前に、`CancelConsensusEvidencePenalty` でペナルティをキャンセルできます。

`EvidenceKind`。回答:

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

- **DoublePrepare/DoubleCommit** - ハッシュ タプル `(phase,height,view,epoch)`。
- **InvalidQc** - コミット証明書 (ビットマップ موقعين فارغ) です。
- **無効な提案** - ロックチェーン (ロックチェーン)。
- **検閲** - 署名された提出受領書には、決して提案/コミットされていないトランザクションが示されています。

重要なポイント:

- Torii: `GET /v1/sumeragi/evidence` و`GET /v1/sumeragi/evidence/count`。
- CLI: `iroha ops sumeragi evidence list`、`... count`、و`... submit --evidence-hex <payload>`。

バイト数と証拠の数:

1. **جمع الحمولة** قبل ان تتقادم.バイト Norito メタデータ 高さ/ビュー。
2. **تجهيز العقوبة** عبر تضمين الحمولة في 国民投票 او تعليمة sudo (مثل `Unregister::peer`)。 تعيد عملية التنفيذ التحقق من الحمولة؛証拠が必要です。
3. ** جدولة طوبولوجيا المتابعة** حتى لا يتمكن المدقق المخالف من العودة فورا. `SetParameter(Sumeragi::NextMode)` و`SetParameter(Sumeragi::ModeActivationHeight)` の名簿。
4. **評価 `/v1/sumeragi/evidence` و`/v1/sumeragi/status` 証拠の証拠 تقدم وان الحوكمة طبقتああ。

### 回答を表示します。

ضمن الاجماع المشترك ان تقوم مجموعة المدققين الخارجة بانهاء بلوك الحد قبل ان تبداありがとうございます。ランタイムのバージョン:- يجب ان يتم الالتزام بـ `SumeragiParameter::NextMode` و`SumeragiParameter::ModeActivationHeight` في **نفس البلوك**。 يجب ان تكون `mode_activation_height` اكبر تماما من ارتفاع البلوك الذي حمل التحديث، بما يوفر على الاقل遅れています。
- `sumeragi.npos.reconfig.activation_lag_blocks` (`1`) ガード時間とハンドオフ時間のラグ:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (デフォルト `259200`) はコンセンサス スラッシュを遅らせ、ペナルティが適用される前にガバナンスがキャンセルできるようにします。

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- ランタイム、CLI、ステージング、`/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params` の実行時間。ありがとうございます。
- ログイン:
  1. انهاء قرار الازالة (او الاستعادة) المدعوم بـ 証拠。
  2. جدولة اعادة تهيئة متابعة مع `mode_activation_height = h_current + activation_lag_blocks`.
  3. مراقبة `/v1/sumeragi/status` حتى يتبدل `effective_consensus_mode` عند الارتفاع المتوقع.

セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティ セキュリティあなたのことを忘れないでください。

## いいえ

- 情報 Prometheus 番号:
  - `governance_proposals_status{status}` (ゲージ) 。
  - `governance_protected_namespace_total{outcome}` (カウンター) يزيد عندما يسمح او يرفض 入場許可 لنشر ضمن 名前空間 محمية.
  - `governance_manifest_activations_total{event}` (カウンター) マニフェスト (`event="manifest_inserted"`) 名前空間 (`event="instance_bound"`)。
- `/status` يتضمن كائنا `governance` يعكس تعداد المقترحات، ويبلغ عن اجمالي 名前空間 المحمية، ويسردマニフェスト番号 (ネームスペース、契約 ID、コード/ABI ハッシュ、ブロック高さ、アクティベーション タイムスタンプ)。これは、名前空間とマニフェストをマニフェストすることによって実現されます。
- قالب Grafana (`docs/source/grafana_governance_constraints.json`) وrunbook التليمترية في `telemetry.md` يوضحان كيفية ربط التنبيهات للمقترحاتマニフェストと名前空間、ランタイム。