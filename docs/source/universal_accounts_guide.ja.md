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
  `merchant@hbl.sbp` などのドメイン修飾エイリアスおよびデータスペース ルート エイリアス
  `merchant@sbp` などは、両方とも同じ正規の `AccountId` に解決できます。
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

現在の Torii ルート:|ルート |目的 |
|------|-----------|
| `GET /v1/ram-lfe/program-policies` |アクティブおよび非アクティブな RAM-LFE プログラム ポリシーとそのパブリック実行メタデータ (オプションの BFV `input_encryption` パラメーターおよびプログラムされたバックエンド `ram_fhe_profile` を含む) をリストします。 |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | `{ input_hex }` または `{ encrypted_input }` のいずれかを受け入れ、選択したプログラムのステートレスな `RamLfeExecutionReceipt` と `{ output_hex, output_hash, receipt_hash }` を返します。現在の Torii ランタイムは、プログラムされた BFV バックエンドのレシートを発行します。 |
| `POST /v1/ram-lfe/receipts/verify` |ステートレスでは、公開されたオンチェーン プログラム ポリシーに対して `RamLfeExecutionReceipt` を検証し、オプションで呼び出し元が指定した `output_hex` がレシート `output_hash` と一致するかどうかをチェックします。 |
| `GET /v1/identifier-policies` |アクティブおよび非アクティブな隠し関数ポリシーの名前空間とそのパブリック メタデータをリストします。これには、オプションの BFV `input_encryption` パラメータ、暗号化されたクライアント側入力に必要な `normalization` モード、およびプログラムされた BFV ポリシーの `ram_fhe_profile` が含まれます。 |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | `{ input }` または `{ encrypted_input }` のいずれかを受け入れます。プレーンテキスト `input` はサーバー側で正規化されます。 BFV `encrypted_input` は、公開されたポリシー モードに従ってすでに正規化されている必要があります。次に、エンドポイントは `opaque:` ハンドルを派生し、生の `signature_payload_hex` と解析された `signature_payload` の両方を含む、`ClaimIdentifier` がオンチェーンで送信できる署名付きレシートを返します。 || `POST /v1/identifiers/resolve` | `{ input }` または `{ encrypted_input }` のいずれかを受け入れます。プレーンテキスト `input` はサーバー側で正規化されます。 BFV `encrypted_input` は、公開されたポリシー モードに従ってすでに正規化されている必要があります。エンドポイントは、アクティブなクレームが存在する場合、識別子を `{ opaque_id, receipt_hash, uaid, account_id, signature }` に解決し、正規の署名付きペイロードも `{ signature_payload_hex, signature_payload }` として返します。 |
| `GET /v1/identifiers/receipts/{receipt_hash}` |確定的な受信ハッシュにバインドされた永続的な `IdentifierClaimRecord` を検索するため、オペレーターと SDK は完全な識別子インデックスをスキャンすることなく、クレームの所有権を監査したり、リプレイ / 不一致の失敗を診断したりできます。 |

Torii のインプロセス実行ランタイムは次のように構成されています。
`torii.ram_lfe.programs[*]`、`program_id` によってキー設定されます。識別子がルーティングされるようになりました
別の `identifier_resolver` の代わりに、同じ RAM-LFE ランタイムを再利用します。
構成面。

現在の SDK サポート:- `normalizeIdentifierInput(value, normalization)` は Rust と一致します
  `exact`、`lowercase_trimmed`、`phone_e164` の正規化子、
  `email_address`、および `account_number`。
- `ToriiClient.listIdentifierPolicies()` は、BFV を含むポリシー メタデータをリストします。
  ポリシーが公開するときの入力暗号化メタデータとデコードされたメタデータ
  `input_encryption_public_parameters_decoded` 経由の BFV パラメータ オブジェクト。
  プログラムされたポリシーは、デコードされた `ram_fhe_profile` も公開します。その分野は
  意図的にBFVスコープ化: ウォレットが予想されるレジスタを検証できるようにします
  カウント、レーン数、正規化モード、および最小暗号文係数
  クライアント側の入力を暗号化する前に、プログラムされた FHE バックエンドを実行します。
- `getIdentifierBfvPublicParameters(policy)` および
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` ヘルプ
  JS 呼び出し元は公開された BFV メタデータを消費し、ポリシー対応リクエストを構築します
  ポリシー ID と正規化ルールを再実装せずに本体を作成します。
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` および
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` さあ、
  JS ウォレットは、完全な BFV Norito 暗号文エンベロープをローカルで構築します。
  事前に構築された 16 進数の暗号文を配布する代わりに、ポリシー パラメータを公開します。
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  隠された識別子を解決し、署名された受信ペイロードを返します。
  `receipt_hash`、`signature_payload_hex`、および
  `signature_payload`。
- `ToriiClient.issueIdentifierClaimReceipt(アカウント ID, { ポリシー ID, 入力 |
  encryptedInput })` issues the signed receipt needed by `ClaimIdentifier`。
- `verifyIdentifierResolutionReceipt(receipt, policy)` は返されたものを検証します
  クライアント側のポリシーリゾルバーキーに対する受信、および`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` は
  後の監査/デバッグ フローのために永続化されたクレーム レコード。
- `IrohaSwift.ToriiClient` が `listIdentifierPolicies()` を公開するようになりました。
  `resolveIdentifier(policyId:input:encryptedInputHex:)`、
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`、
  および `getIdentifierClaimByReceiptHash(_)`、プラス
  同じ電話/メール/アカウント番号の `ToriiIdentifierNormalization`
  正規化モード。
- `ToriiIdentifierLookupRequest` および
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` ヘルパーは、型付き Swift リクエスト サーフェスを提供します。
  呼び出しの解決とクレーム受信が可能になり、Swift ポリシーで BFV を導出できるようになりました。
  `encryptInput(...)` / `encryptedRequest(input:...)` 経由でローカルに暗号文を送信します。
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` はそれを検証します
  最上位の受信フィールドは署名されたペイロードと一致し、
  送信前にクライアント側でリゾルバー署名を行います。
- Android SDK の `HttpClientTransport` が公開されるようになりました
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, input,
  encryptedInputHex)`, `issueIdentifierClaimReceipt(accountId、policyId、
  入力、encryptedInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  さらに、同じ正規化ルールの `IdentifierNormalization`。
- `IdentifierResolveRequest` および
  `IdentifierPolicySummary.plaintextRequest(...)` /
  `.encryptedRequest(...)` ヘルパーは、型指定された Android リクエスト サーフェスを提供します。
  一方 `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` BFV 暗号文エンベロープを導出する
  公開されたポリシーパラメータからローカルで。
  `IdentifierResolutionReceipt.verifySignature(policy)` は返されたものを検証します
  クライアント側のリゾルバー署名。

現在の命令セット:- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (受信限定、生の `opaque_id` クレームは拒否されます)
- `RevokeIdentifier`

現在、`iroha_crypto::ram_lfe` には 3 つのバックエンドが存在します。

- 歴史的なコミットメントに拘束された `HKDF-SHA3-512` PRF、および
- BFV で暗号化された識別子を使用する BFV ベースの秘密アフィン評価器
  スロットを直接操作します。 `iroha_crypto` をデフォルトでビルドした場合
  `bfv-accel` 機能、BFV リング乗算は正確な決定論を使用します
  内部の CRT-NTT バックエンド。その機能を無効にすると、フォールバックされます
  同一の出力を持つスカラー スクールブック パス、および
- 命令駆動型の
  暗号化されたレジスタと暗号文メモリ上の RAM スタイルの実行トレース
  不透明な識別子と受信ハッシュを導出する前のレーン。プログラムされた
  バックエンドはアフィン パスよりも強力な BFV モジュラス フロアを必要とするようになりました。
  そのパブリックパラメータは、
  ウォレットと検証者によって消費される RAM-FHE 実行プロファイル。

ここで BFV は、で実装された Brakerski/Fan-Vercauteren FHE スキームを意味します。
`crates/iroha_crypto/src/fhe_bfv.rs`。暗号化実行メカニズムです
外側の非表示の名前ではなく、アフィンおよびプログラムされたバックエンドによって使用されます。
関数の抽象化。Torii は、ポリシーコミットメントによって公開されたバックエンドを使用します。 BFV バックエンドの場合
アクティブである場合、平文リクエストは正規化され、その後サーバー側で暗号化されます。
評価。アフィン バックエンドに対する BFV `encrypted_input` リクエストが評価されます
直接、クライアント側ですでに正規化されている必要があります。プログラムされたバックエンド
暗号化された入力を正規化してリゾルバの決定論的な BFV に戻します
シークレットRAMプログラムを実行する前にエンベロープを削除するため、受信ハッシュが残ります
意味的に同等の暗号文間で安定しています。

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
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
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