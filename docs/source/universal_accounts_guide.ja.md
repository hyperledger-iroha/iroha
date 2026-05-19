<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
lang: ja
direction: ltr
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ユニバーサルアカウントガイド

このガイドでは、UAID (ユニバーサル アカウント ID) ロールアウト要件を以下から抽出します。
Nexus ロードマップを作成し、それらをオペレーターと SDK に重点を置いたウォークスルーにパッケージ化します。
UAID の導出、ポートフォリオ/マニフェスト検査、規制テンプレート、
すべての「iroha アプリのスペースディレクトリマニフェスト」に添付する必要がある証拠
publish` run (roadmap reference: `roadmap.md:2209`)。

## 1. UAID クイックリファレンス- UAID は `uaid:<hex>` リテラルで、`<hex>` は Blake2b-256 ダイジェストです。
  LSB は `1` に設定されます。正規型は次の場所に存在します。
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`。
- アカウント レコード (`Account` および `AccountDetails`) には、オプションの `uaid` が含まれるようになりました。
  フィールドを使用することで、アプリケーションは特注のハッシュを行わずに識別子を学習できるようになります。
- 隠し関数識別子ポリシーは、任意の正規化された入力をバインドできます
  (電話番号、電子メール、アカウント番号、パートナー文字列) から `opaque:` ID
  UAID 名前空間の下にあります。オンチェーンのピースは `IdentifierPolicy`、
  `IdentifierClaimRecord`、および `opaque_id -> uaid` インデックス。
- Space Directory は、各 UAID を結び付ける `World::uaid_dataspaces` マップを維持します。
  アクティブなマニフェストによって参照されるデータスペース アカウントに。 Torii はそれを再利用します
  `/portfolio` および `/uaids/*` API のマップ。
- `POST /v1/accounts/onboard` は、デフォルトのスペース ディレクトリ マニフェストを公開します。
  グローバル データスペースが存在しない場合は、UAID がすぐにバインドされます。
  オンボーディング権限者は `CanPublishSpaceDirectoryManifest{dataspace=0}` を保持している必要があります。
- すべての SDK は、UAID リテラルを正規化するためのヘルパーを公開します (例:
  Android SDK では `UaidLiteral`)。ヘルパーは生の 64 16 進ダイジェストを受け入れます
  (LSB=1) または `uaid:<hex>` リテラルを使用し、同じ Norito コーデックを再利用します。
  ダイジェストは言語をまたいで移動することはできません。

## 1.1 隠し識別子ポリシー

UAID は 2 番目の ID レイヤーのアンカーになりました。- グローバル `IdentifierPolicyId` (`<kind>#<business_rule>`) は、
  名前空間、パブリック コミットメント メタデータ、リゾルバー検証キー、および
  正規入力正規化モード (`Exact`、`LowercaseTrimmed`、
  `PhoneE164`、`EmailAddress`、または `AccountNumber`)。
- クレームは、1 つの派生 `opaque:` 識別子を 1 つの UAID と 1 つの UAID にバインドします。
  そのポリシーでは正規の `AccountId` ですが、チェーンは
  署名された `IdentifierResolutionReceipt` が添付されている場合に請求します。
- 解決策は `resolve -> transfer` フローのままです。 Torii は不透明を解決します
  を処理し、正規の `AccountId` を返します。転送は依然として
  `uaid:` または `opaque:` リテラルを直接使用するのではなく、正規アカウントを使用します。
- ポリシーで BFV 入力暗号化パラメータを公開できるようになりました。
  `PolicyCommitment.public_parameters`。存在する場合、Torii はそれらをアドバタイズします。
  `GET /v1/identifier-policies`、クライアントは BFV でラップされた入力を送信する可能性があります
  平文の代わりに。プログラムされたポリシーは、BFV パラメータをラップします。
  正規の `BfvProgrammedPublicParameters` バンドルも公開されています。
  パブリック `ram_fhe_profile`;従来の生の BFV ペイロードはその上にアップグレードされます
  コミットメントが再構築されるときの正規バンドル。
- 識別子ルートは同じ Torii アクセス トークンとレート制限を通過します。
  他のアプリ側のエンドポイントとしてチェックします。それらは通常のバイパスではありません
  API ポリシー。

## 1.2 用語

名前の分割は意図的に行われています。- `ram_lfe` は、外側の隠し関数の抽象化です。ポリシーをカバーします
  登録、コミットメント、パブリックメタデータ、実行レシート、および
  検証モード。
- `BFV` は、Brakerski/Fan-Vercauteren 準同型暗号化方式であり、
  暗号化された入力を評価するための一部の `ram_lfe` バックエンド。
- `ram_fhe_profile` は BFV 固有のメタデータであり、全体の 2 番目の名前ではありません
  特徴。これは、ウォレットと
  検証者は、ポリシーがプログラムされたバックエンドを使用する場合をターゲットにする必要があります。

具体的には:

- `RamLfeProgramPolicy` および `RamLfeExecutionReceipt` は LFE 層タイプです。
- `BfvParameters`、`BfvCiphertext`、`BfvProgrammedPublicParameters`、および
  `BfvRamProgramProfile` は FHE 層タイプです。
- `HiddenRamFheProgram` および `HiddenRamFheInstruction` は、
  プログラムされたバックエンドによって実行される非表示の BFV プログラム。彼らはそこに留まります
  FHE 側では、暗号化された実行メカニズムについて説明しているため、
  外部ポリシーまたはレシートの抽象化。

## 1.3 アカウント ID とエイリアス

ユニバーサル アカウントのロールアウトでは、正規のアカウント ID モデルは変更されません。- `AccountId` は正規のドメインレス アカウントのサブジェクトのままです。
- `AccountAlias` 値は、そのサブジェクト上の別個の SNS バインディングです。あ
  `merchant@banka.paynet` などのドメイン修飾エイリアスおよびデータスペース ルート エイリアス
  `merchant@paynet` などは、両方とも同じ正規の `AccountId` に解決できます。
- 正規アカウント登録は常に `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`;ドメイン修飾またはドメイン実体化はありません
  登録パス。
- ドメイン所有権、エイリアス権限、その他のドメインスコープの動作はライブです
  アカウント ID 自体ではなく、独自の状態と API で。
- パブリック アカウント ルックアップはその分割に従います。エイリアス クエリはパブリックのままですが、
  正規アカウント ID は純粋な `AccountId` のままです。

オペレーター、SDK、テストの実装ルール: 正規のものから開始
`AccountId`、次にエイリアスのリース、データスペース/ドメインのアクセス許可、およびその他の権限を追加します。
ドメイン所有状態は別途。偽のエイリアス由来のアカウントを合成しないでください
または、エイリアスまたは
ルートはドメインセグメントを運びます。

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

## 2. UAID の導出と検証

UAID を取得するには 3 つの方法がサポートされています。

1. **ワールド ステートまたは SDK モデルから読み取ります。** 任意の `Account`/`AccountDetails`
   Torii 経由でクエリされたペイロードには、`uaid` フィールドが設定されるようになりました。
   参加者はユニバーサルアカウントにオプトインしました。
2. **UAID レジストリをクエリします。** Torii が公開します
   `GET /v1/space-directory/uaids/{uaid}` データスペース バインディングを返します
   スペース ディレクトリ ホストが保持するマニフェスト メタデータ (「
   ペイロード サンプルについては `docs/space-directory.md` §3)。
3. **決定論的に導出します。** 新しい UAID をオフラインでブートストラップするときは、ハッシュします。
   正規の参加者シードに Blake2b-256 を付け、結果に接頭辞を付けます。
   `uaid:`。以下のスニペットは、次のドキュメントに記載されているヘルパーを反映しています。
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```リテラルは常に小文字で保存し、ハッシュする前に空白を正規化してください。
`iroha app space-directory manifest scaffold` や Android などの CLI ヘルパー
`UaidLiteral` パーサーは同じトリミング ルールを適用するため、ガバナンス レビューを行うことができます。
アドホック スクリプトを使用せずに値をクロスチェックします。

## 3. UAID の保有物とマニフェストの検査

`iroha_core::nexus::portfolio` の決定論的ポートフォリオ アグリゲーター
UAID を参照するすべてのアセット/データスペースのペアを表示します。オペレーターとSDK
次のサーフェスを通じてデータを消費できます。

|表面 |使い方 |
|----------|----------|
| `GET /v1/accounts/{uaid}/portfolio` |データスペース → 資産 → 残高の概要を返します。 `docs/source/torii/portfolio_api.md` に記載されています。 |
| `GET /v1/space-directory/uaids/{uaid}` | UAID に関連付けられたデータスペース ID とアカウント リテラルをリストします。 |
| `GET /v1/space-directory/uaids/{uaid}/manifests` |監査用に完全な `AssetPermissionManifest` 履歴を提供します。 |
| `iroha app space-directory bindings fetch --uaid <literal>` |バインディング エンドポイントをラップし、オプションで JSON をディスクに書き込む CLI ショートカット (`--json-out`)。 |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` |証拠パックのマニフェスト JSON バンドルを取得します。 |

CLI セッションの例 (`iroha.json` の `torii_api_url` 経由で設定された Torii URL):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

レビュー中に使用されるマニフェスト ハッシュと一緒に JSON スナップショットを保存します。の
Space Directory ウォッチャーは、マニフェストが発生するたびに `uaid_dataspaces` マップを再構築します。
アクティブ化、期限切れ、または取り消しを行うため、これらのスナップショットは証明する最速の方法です。
特定のエポックでどのバインディングがアクティブであったか。## 4. 証拠を伴う公開能力マニフェスト

新しい許可がロールアウトされるたびに、以下の CLI フローを使用します。各ステップでは、次のことを行う必要があります。
ガバナンスの承認のために記録された証拠バンドルに記録されます。

1. **マニフェスト JSON をエンコード**して、レビュー担当者が事前に決定論的ハッシュを確認できるようにする
   提出:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. Norito ペイロード (`--manifest`) または
   JSON 説明 (`--manifest-json`)。 Torii/CLI の受信を記録し、さらに
   `PublishSpaceDirectoryManifest` 命令ハッシュ:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **SpaceDirectoryEvent の証拠をキャプチャします。**
   `SpaceDirectoryEvent::ManifestActivated` にイベント ペイロードを含めます
   このバンドルにより、監査人は変更がいつ反映されたかを確認できます。

4. **監査バンドルを生成**して、マニフェストをそのデータスペース プロファイルに関連付け、
   テレメトリフック:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **Torii** (`bindings fetch` および `manifests fetch`) を介してバインディングを確認し、
   上記のハッシュ + バンドルを使用してこれらの JSON ファイルをアーカイブします。

証拠チェックリスト:

- [ ] 変更承認者によって署名されたマニフェスト ハッシュ (`*.manifest.hash`)。
- [ ] パブリッシュ呼び出しの CLI/Torii 受信 (標準出力または `--json-out` アーティファクト)。
- [ ] `SpaceDirectoryEvent` ペイロードはアクティブ化を証明します。
- [ ] データスペース プロファイル、フック、およびマニフェスト コピーを含む監査バンドル ディレクトリ。
- [ ] バインディング + アクティベーション後に Torii からフェッチされたマニフェスト スナップショット。これは、SDK を提供しながら、`docs/space-directory.md` §3.2 の要件を反映しています。
所有者は、リリース レビュー中に参照できる 1 つのページを所有します。

## 5. 規制当局/地域のマニフェストテンプレート

機能マニフェストを作成する際の開始点としてリポジトリ内フィクスチャを使用する
規制当局または地域監督者向け。これらは、許可/拒否のスコープを設定する方法を示しています。
ルールを作成し、レビュー担当者が期待するポリシーノートを説明します。

|治具 |目的 |ハイライト |
|----------|----------|---------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | ESMA/ESRB 監査フィード。 |規制当局の UAID をパッシブに保つため、リテール転送で拒否勝利を伴う `compliance.audit::{stream_reports, request_snapshot}` の読み取り専用許可。 |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | JFSA監視レーン。 |上限付きの `cbdc.supervision.issue_stop_order` 許可 (1 日あたりのウィンドウ + `max_amount`) と `force_liquidation` の明示的な拒否を追加して、二重制御を強制します。 |

これらのフィクスチャのクローンを作成する場合は、以下を更新します。

1. 有効にする参加者とレーンに一致する `uaid` および `dataspace` ID。
2. ガバナンス スケジュールに基づく `activation_epoch`/`expiry_epoch` ウィンドウ。
3. 規制当局のポリシー参照を含む `notes` フィールド (MiCA 記事、JFSA
   円形など）。
4. 許容範囲 (`PerSlot`、`PerMinute`、`PerDay`) およびオプション
   `max_amount` の上限により、SDK はホストと同じ制限を適用します。

## 6. SDK コンシューマー向けの移行メモドメインごとのアカウント ID を参照していた既存の SDK 統合は、
前述の UAID 中心のサーフェス。アップグレード中にこのチェックリストを使用してください。

  アカウントID。 Rust/JS/Swift/Android の場合、これは最新のものにアップグレードすることを意味します。
  ワークスペース クレートまたは Norito バインディングを再生成しています。
- **API 呼び出し:** ドメイン スコープのポートフォリオ クエリを次のように置き換えます。
  `GET /v1/accounts/{uaid}/portfolio` およびマニフェスト/バインディング エンドポイント。
  `GET /v1/accounts/{uaid}/portfolio` はオプションの `asset_id` クエリを受け入れます
  ウォレットに単一のアセット インスタンスのみが必要な場合のパラメータ。クライアントヘルパーなど
  `ToriiClient.getUaidPortfolio` (JS) および Android として
  `SpaceDirectoryClient` はすでにこれらのルートをラップしています。オーダーメイドよりも好きです
  HTTPコード。
- **キャッシュとテレメトリ:** 未加工ではなく、UAID + データスペースによってエントリをキャッシュします。
  アカウント ID を取得し、UAID リテラルを示すテレメトリを発行して、操作を実行できるようにします。
  ログをスペース ディレクトリの証拠と並べます。
- **エラー処理:** 新しいエンドポイントは厳密な UAID 解析エラーを返します
  `docs/source/torii/portfolio_api.md` に記載されています。それらのコードを表面化する
  逐語的に伝えられるため、サポート チームは再現手順を行わずに問題を優先順位付けできます。
- **テスト:** 上記のフィクスチャ (および独自の UAID マニフェスト) を接続します。
  Norito ラウンドトリップとマニフェスト評価を証明するための SDK テスト スイートへの組み込み
  ホストの実装と一致します。

## 7. 参考文献- `docs/space-directory.md` — より深いライフサイクルの詳細を含むオペレーター プレイブック。
- `docs/source/torii/portfolio_api.md` — UAID ポートフォリオの REST スキーマと
  マニフェストエンドポイント。
- `crates/iroha_cli/src/space_directory.rs` — で参照される CLI 実装
  このガイド。
- `fixtures/space_directory/capability/*.manifest.json` — レギュレーター、小売店、および
  CBDC マニフェスト テンプレートをクローン作成する準備ができました。
