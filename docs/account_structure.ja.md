# アカウント構造 RFC

**ステータス:** 承認済み (ADDR-1)  
**対象者:** データ モデル、torii、Nexus、ウォレット、ガバナンス チーム  
**関連する問題:** 未定

## 概要

このドキュメントでは、に実装されている配送先アカウントのアドレス指定スタックについて説明します。
`AccountAddress` (`crates/iroha_data_model/src/account/address.rs`) と
コンパニオンツール。それは以下を提供します:

- チェックサム付き、人間向けの **Iroha Base58 アドレス (I105)** によって生成されます。
  `AccountAddress::to_i105` チェーン判別式をアカウントにバインドします
  コントローラーであり、決定論的な相互運用に適したテキスト形式を提供します。
- 暗黙的なデフォルト ドメインとローカル ダイジェストのドメイン セレクター。
  将来の Nexus-backed ルーティング用に予約されたグローバル レジストリ セレクター タグ (
  レジストリ ルックアップは **まだ出荷されていません**)。

## モチベーション

現在、ウォレットとオフチェーン ツールは生の `alias@domain` (rejected legacy form) ルーティング エイリアスに依存しています。これ
には 2 つの大きな欠点があります。

1. **ネットワーク バインディングがありません。** 文字列にはチェックサムやチェーン プレフィックスがないため、ユーザーは
   すぐにフィードバックが得られずに、間違ったネットワークからアドレスを貼り付ける可能性があります。の
   トランザクションは最終的に拒否されるか (チェーンの不一致)、最悪の場合は成功します。
   宛先がローカルに存在する場合、意図しないアカウントに対して。
2. **ドメインの衝突。** ドメインは名前空間のみであり、それぞれのドメインで再利用できます。
   チェーン。サービスのフェデレーション (カストディアン、ブリッジ、クロスチェーン ワークフロー)
   チェーン A の `finance` はチェーン A の `finance` と無関係であるため、脆くなります。
   チェーンB。

コピー/ペーストのエラーを防ぐ、人に優しいアドレス形式が必要です
そしてドメイン名から権威チェーンへの決定論的なマッピング。

## 目標

- データ モデルに実装された I105 Base58 エンベロープと
  `AccountId` および `AccountAddress` に従う正規の解析/エイリアス ルール。
- 構成されたチェーン判別式を各アドレスに直接エンコードし、
  ガバナンス/レジストリ プロセスを定義します。
- 現状を壊さずにグローバル ドメイン レジストリを導入する方法を説明する
  展開を行い、正規化/スプーフィング防止ルールを指定します。

## 非目標

- クロスチェーン資産移転の実装。ルーティング層は、
  ターゲットチェーン。
- グローバル ドメイン発行のガバナンスを最終決定します。この RFC はデータに焦点を当てています
  プリミティブのモデル化とトランスポート。

## 背景

### 現在のルーティング エイリアス

```
AccountId {
    domain: DomainId,   // wrapper over Name (ASCII-ish string)
    controller: AccountController // single PublicKey or multisig policy
}

Display: canonical I105 literal (no `@domain` suffix)
Parse accepts:
- Encoded account identifiers only: I105.
- Runtime parsers reject canonical hex (`0x...`), any `@<domain>` suffix, and alias literals such as `label@domain`.

Multihash hex is canonical: varint bytes are lowercase hex, payload bytes are uppercase hex,
and `0x` prefixes are not accepted.

This text form is now treated as an **account alias**: a routing convenience
that points to the canonical [`AccountAddress`](#2-canonical-address-codecs).
It remains useful for human readability and domain-scoped governance, but it is
no longer considered the authoritative account identifier on-chain.
```

`ChainId` は `AccountId` の外に住んでいます。ノードはトランザクションの `ChainId` をチェックします
許可時の設定に対する違反 (`AcceptTransactionFail::ChainIdMismatch`)
外国取引を拒否しますが、アカウント文字列自体には何も含まれていません。
ネットワークのヒント。

### ドメイン識別子

`DomainId` は `Name` (正規化された文字列) をラップし、ローカル チェーンにスコープされます。
各チェーンは `wonderland`、`finance` などを個別に登録できます。

### ネクサスコンテキスト

Nexus は、コンポーネント間の調整 (レーン/データスペース) を担当します。それ
現在、クロスチェーン ドメイン ルーティングの概念はありません。

## 提案されたデザイン

### 1. 決定的連鎖判別式

`iroha_config::parameters::actual::Common` は以下を公開するようになりました:

```rust
pub struct Common {
    pub chain: ChainId,
    pub chain_discriminant: u16, // globally coordinated
    // ... existing fields
}
```

- **制約:**
  - アクティブなネットワークごとに一意。署名されたパブリックレジストリを通じて管理されます
    明示的に予約された範囲 (例: `0x0000–0x0FFF` test/dev、`0x1000–0x7FFF`)
    コミュニティ割り当て、`0x8000–0xFFEF` ガバナンス承認済み、`0xFFF0–0xFFFF`
    予約済み）。
  - 実行中のチェーンに対して不変です。これを変更するにはハードフォークと
    レジストリの更新。
- **ガバナンスとレジストリ (計画中):** マルチシグネチャ ガバナンス セットは、
  識別子を人間のエイリアスにマッピングする署名付き JSON レジストリを維持し、
  CAIP-2 識別子。このレジストリは、出荷されたランタイムにはまだ含まれていません。
- **使用法:** ステートアドミッション、Torii、SDK、ウォレット API を介してスレッド化されるため、
  すべてのコンポーネントはそれを埋め込んだり検証したりできます。 CAIP-2 への曝露は依然として将来的なものである
  相互運用タスク。

### 2. 正規のアドレス コーデック

Rustデータモデルは単一の正規ペイロード表現を公開します
(`AccountAddress`) は、人間向けのいくつかの形式として出力できます。 I105は
共有および正規出力に推奨されるアカウント形式。圧縮された
`sora` フォームは、かなアルファベットが使用される UX の 2 番目に優れた Sora 専用オプションです。
価値を追加します。 Canonical hex は引き続きデバッグ補助として使用されます。

- **I105 (Iroha Base58)** – チェーンを埋め込む Base58 エンベロープ
  差別的な。デコーダはペイロードをプロモートする前にプレフィックスを検証します。
  正規形。
- **Sora 圧縮ビュー** – によって構築された **105 個の記号**からなる Sora 専用のアルファベット
  58字に半角イロハ詩（ヰ、ヱ含む）を付ける
  I105セット。文字列はセンチネル `sora` で始まり、Bech32m 由来のコードが埋め込まれます。
  チェックサムを使用し、ネットワーク プレフィックスを省略します (Sora Nexus はセンチネルによって暗示されます)。

```
  I105  : 123456789ABCDEFGHJKMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
  Iroha : ｲﾛﾊﾆﾎﾍﾄﾁﾘﾇﾙｦﾜｶﾖﾀﾚｿﾂﾈﾅﾗﾑｳヰﾉｵｸﾔﾏｹﾌｺｴﾃｱｻｷﾕﾒﾐｼヱﾋﾓｾｽ
  ```
- **正規 16 進数** – デバッグしやすい正規バイトの `0x…` エンコード
  封筒。

`AccountAddress::parse_encoded` は、I105 (推奨)、圧縮 (`sora`、2 番目に優れた)、または正規の 16 進数を自動検出します。
(`0x...` のみ。裸の 16 進数は拒否されます) デコードされたペイロードと検出されたペイロードの両方を入力して返します。
`AccountAddress`。鳥居は ISO 20022 補足のために `parse_encoded` を呼び出します
メタデータが決定性を維持できるように、正規の 16 進形式をアドレス指定して保存します。
元の表現に関係なく。

#### 2.1 ヘッダーバイトレイアウト (ADDR-1a)

すべての正規ペイロードは `header · controller` としてレイアウトされます。の
`header` は、どのパーサー ルールがそのバイトに適用されるかを伝達する 1 バイトです。
フォローしてください：

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

したがって、最初のバイトには、ダウンストリーム デコーダーのスキーマ メタデータがパックされます。

|ビット |フィールド |許可される値 |違反時のエラー |
|-----|----------|----------------|----------|
| 7-5 | `addr_version` | `0` (v1)。値 `1-7` は、将来のリビジョンのために予約されています。 | `0-7` 以外の値は `AccountAddressError::InvalidHeaderVersion` をトリガーします。実装は、ゼロ以外のバージョンを現在サポートされていないものとして扱わなければなりません。 |
| 4-3 | `addr_class` | `0` = 単一キー、`1` = マルチシグ。 |他の値では `AccountAddressError::UnknownAddressClass` が発生します。 |
| 2-1 | `norm_version` | `1` (標準 v1)。値 `0`、`2`、`3` は予約されています。 | `0-3` 以外の値は `AccountAddressError::InvalidNormVersion` を引き起こします。 |
| 0 | `ext_flag` | `0` でなければなりません。 |ビットを設定すると `AccountAddressError::UnexpectedExtensionFlag` が発生します。 |

Rust エンコーダは、単一キー コントローラ (バージョン 0、クラス 0、
ノルム v1、拡張フラグはクリアされています）および `0x0A` マルチシグ コントローラー用（バージョン 0、
クラス 1、標準 v1、拡張フラグはクリアされます)。

#### 2.2 Domainless payload semantics

Canonical payload bytes are domainless: the wire layout is `header · controller`
with no selector segment, no implicit default-domain reconstruction, and no
public decode fallback for legacy scoped-account literals.

Explicit domain context is modeled separately as `ScopedAccountId { account,
domain }` or separate API fields; it is not encoded into `AccountId` payload
bytes.

| Tag | Meaning | Payload | Notes |
|-----|---------|---------|-------|
| `0x00` | Domainless canonical scope | none | Canonical account payloads are domainless; explicit domain context lives outside the address payload. |
| `0x01` | Local domain digest | 12 bytes | Digest = `blake2s_mac(key = "SORA-LOCAL-K:v1", canonical_label)[0..12]`. |
| `0x02` | Global registry entry | 4 bytes | Big-endian `registry_id`; reserved until the global registry ships. |

Domain labels are canonicalised (UTS-46 + STD3 + NFC) before hashing. Unknown tags raise `AccountAddressError::UnknownDomainTag`. When validating an address against a domain, mismatched selectors raise `AccountAddressError::DomainMismatch`.

```
legacy selector segment
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind, see table)│
└──────────┴──────────────────────────────────────────────┘
```

When present, the selector is immediately adjacent to the controller payload, so
a decoder can walk the wire format in order: read the tag byte, read the
tag-specific payload, then move on to the controller bytes.

**Legacy selector examples**

- *Implicit default* (`tag = 0x00`). No payload. Example canonical hex for the default
  domain using the deterministic test key:
  `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`.
- *Local digest* (`tag = 0x01`). Payload is the 12-byte digest. Example (`treasury` seed
  `0x01`): `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`.
- *Global registry* (`tag = 0x02`). Payload is a big-endian `registry_id:u32`. The bytes
  that follow the payload are identical to the implicit-default case; the selector simply
  replaces the normalised domain string with a registry pointer. Example using
  `registry_id = 0x0000_002A` (decimal 42) and the deterministic default controller:
  `0x02020000002a000120641297079357229f295938a4b5a333de35069bf47b9d0704e45805713d13c201`.

#### 2.3 コントローラー ペイロード エンコーディング (ADDR-1a)

コントローラー ペイロードは、ドメイン セレクターの後に追加される別のタグ付き共用体です。

|タグ |コントローラー |レイアウト |メモ |
|-----|-----------|----------|----------|
| `0x00` |単一のキー | `curve_id:u8` · `key_len:u8` · `key_bytes` | `curve_id=0x01` は今日の Ed25519 にマッピングされます。 `key_len` は `u8` にバインドされます。値が大きいほど、`AccountAddressError::KeyPayloadTooLong` が発生します (そのため、255 バイトを超える単一キーの ML‑DSA 公開キーはエンコードできず、マルチシグを使用する必要があります)。 |
| `0x01` |マルチシグ | `version:u8` · `threshold:u16` · `member_count:u8` · (`curve_id:u8` · `weight:u16` · `key_len:u16` · `key_bytes`)\* |最大 255 人のメンバーをサポートします (`CONTROLLER_MULTISIG_MEMBER_MAX`)。不明な曲線により `AccountAddressError::UnknownCurve` が発生します。不正なポリシーは `AccountAddressError::InvalidMultisigPolicy` としてバブルします。 |

マルチシグ ポリシーは、CTAP2 スタイルの CBOR マップと正規ダイジェストも公開します。
ホストと SDK はコントローラーを決定的に検証できます。参照
スキーマの場合は `docs/source/references/multisig_policy_schema.md` (ADDR-1c)、
検証ルール、ハッシュ手順、およびゴールデン フィクスチャ。

すべてのキーバイトは、`PublicKey::to_bytes` によって返されたとおりに正確にエンコードされます。デコーダは `PublicKey` インスタンスを再構築し、バイトが宣言された曲線と一致しない場合は `AccountAddressError::InvalidPublicKey` を発生させます。

> **Ed25519 正規強制 (ADDR-3a):** 曲線 `0x01` キーは、署名者が発行した正確なバイト文字列にデコードする必要があり、小次数のサブグループ内にあってはなりません。ノードは非正規エンコーディング (`2^255-19` を法として削減された値など) や ID 要素などの弱点を拒否するようになりました。そのため、SDK はアドレスを送信する前に一致検証エラーを検出する必要があります。

##### 2.3.1 曲線識別子レジストリ (ADDR-1d)

| ID (`curve_id`) |アルゴリズム |フィーチャーゲート |メモ |
|-----------------|-----------|--------------|------|
| `0x00` |予約済み | — |放出してはなりません。デコーダは `ERR_UNKNOWN_CURVE` を表示します。 |
| `0x01` | Ed25519 | — |正規 v1 アルゴリズム (`Algorithm::Ed25519`);デフォルト設定で有効になっています。 |
| `0x02` | ML‑DSA (ダイリチウム3) | — | Dilithium3 公開キー バイト (1952 バイト) を使用します。 `key_len` は `u8` であるため、単一キー アドレスは ML-DSA をエンコードできません。マルチシグは `u16` の長さを使用します。 |
| `0x03` | BLS12‑381（ノーマル） | `bls` |公開キーは G1 (48 バイト)、署名は G2 (96 バイト) にあります。 |
| `0x04` | secp256k1 | — | SHA-256 上の決定論的 ECDSA。公開鍵は 33 バイトの SEC1 圧縮形式を使用し、署名は正規の 64 バイト `r∥s` レイアウトを使用します。 |
| `0x05` | BLS12‑381(小) | `bls` |公開キーは G2 (96 バイト)、署名は G1 (48 バイト) にあります。 |
| `0x0A` | GOST R 34.10-2012 (256、セット A) | `gost` | `gost` 機能が有効になっている場合にのみ使用できます。 |
| `0x0B` | GOST R 34.10‑2012 (256、セット B) | `gost` | `gost` 機能が有効になっている場合にのみ使用できます。 |
| `0x0C` | GOST R 34.10‑2012 (256、セット C) | `gost` | `gost` 機能が有効になっている場合にのみ使用できます。 |
| `0x0D` | GOST R 34.10‑2012 (512、セット A) | `gost` | `gost` 機能が有効になっている場合にのみ使用できます。 |
| `0x0E` | GOST R 34.10‑2012 (512、セット B) | `gost` | `gost` 機能が有効になっている場合にのみ使用できます。 |
| `0x0F` | SM2 | `sm` | DistID 長 (u16 BE) + DistID バイト + 65 バイトの SEC1 非圧縮 SM2 キー。 `sm` が有効な場合にのみ使用できます。 |

スロット `0x06–0x09` は追加のカーブに割り当てられていないままになります。新しいものを導入する
アルゴリズムにはロードマップの更新と、SDK/ホストの対応範囲の一致が必要です。エンコーダ
`ERR_UNSUPPORTED_ALGORITHM` を使用して、サポートされていないアルゴリズムを拒否しなければなりません。
デコーダは、`ERR_UNKNOWN_CURVE` を保持するために不明な ID で高速に失敗しなければなりません (MUST)。
フェイルクローズ動作。

正規レジストリ (機械可読な JSON エクスポートを含む) は以下にあります。
[`docs/source/references/address_curve_registry.md`](source/references/address_curve_registry.md)。
曲線識別子が残るように、ツールはそのデータセットを直接使用する必要があります (SHOULD)
SDK とオペレーターのワークフロー全体で一貫性があります。

- **SD​​K ゲーティング:** SDK はデフォルトで Ed25519 のみの検証/エンコーディングになります。スウィフトが暴露する
  コンパイル時フラグ (`IROHASWIFT_ENABLE_MLDSA`、`IROHASWIFT_ENABLE_GOST`、
  `IROHASWIFT_ENABLE_SM`); Java/Android SDK に必要なもの
  `AccountAddress.configureCurveSupport(...)`; JavaScript SDK が使用する
  `configureCurveSupport({ allowMlDsa: true, allowGost: true, allowSm2: true })`。
  secp256k1 サポートは利用可能ですが、JS/Android ではデフォルトで有効になっていません。
  SDK; Ed25519 以外のコントローラを発行する場合、呼び出し元は明示的にオプトインする必要があります。
- **ホスト ゲーティング:** `Register<Account>` は、署名者がアルゴリズムを使用しているコントローラーを拒否します
  ノードの `crypto.allowed_signing` リストに欠落しています **または** 曲線識別子が欠落しています
  `crypto.curves.allowed_curve_ids` なので、クラスターはサポートをアドバタイズする必要があります (構成 +
  Genesis) は、ML‑DSA/GOST/SM コントローラーを登録する前に必要です。 BLSコントローラー
  アルゴリズムはコンパイル時に常に許可されます (コンセンサス キーはアルゴリズムに依存します)。
  デフォルト設定では Ed25519 + secp256k1 が有効になります。【crates/iroha_core/src/smartcontracts/isi/domain.rs:32】

##### 2.3.2 マルチシグ コントローラーのガイダンス

`AccountController::Multisig` はポリシーをシリアル化します。
`crates/iroha_data_model/src/account/controller.rs` とスキーマを適用します
[`docs/source/references/multisig_policy_schema.md`](source/references/multisig_policy_schema.md) に記載されています。
主要な実装の詳細:

- ポリシーは、事前に `MultisigPolicy::validate()` によって正規化および検証されます。
  埋め込まれている。しきい値は ≥1 かつ ≤Σ の重みである必要があります。重複したメンバーは
  `(algorithm || 0x00 || key_bytes)` で並べ替えた後、決定的に削除されます。
- バイナリ コントローラ ペイロード (`ControllerPayload::Multisig`) はエンコードされます。
  `version:u8`、`threshold:u16`、`member_count:u8`、次に各メンバーの
  `(curve_id, weight:u16, key_len:u16, key_bytes)`。まさにこれです
  `AccountAddress::canonical_bytes()` は、I105 (推奨)/sora (2 番目に優れた) ペイロードに書き込みます。
- ハッシュ (`MultisigPolicy::digest_blake2b256()`) は、Blake2b-256 を使用します。
  `iroha-ms-policy` パーソナライゼーション文字列を使用して、ガバナンス マニフェストを
  I105 に埋め込まれたコントローラ バイトと一致する決定的なポリシー ID。
- フィクスチャ カバレッジは `fixtures/account/address_vectors.json` にあります (ケース
  `addr-multisig-*`)。ウォレットと SDK は正規の I105 文字列をアサートする必要があります
  以下で、エンコーダが Rust 実装と一致していることを確認します。

|ケースID |しきい値 / メンバー | I105 リテラル (接頭辞 `0x02F1`) | Sora 圧縮 (`sora`) リテラル |メモ |
|----------|---------------------|----------------------------|----------------------|----------|
| `addr-multisig-council-threshold3` | `≥3` 体重、メンバー `(2,1,1)` | `SRfSHsrH3tEmYaaAYyD248F3vfT1oQ3WEGS22MaD8W9bLefF7rsoKLYGcpbcM9EcSus5ZhCAZU7ztn2BCsyeCAdfRncAVmVsipd4ibk6CBLF3Nrzcw8P7VKJg6mtFgEhWVTjfDkUMoc63oeEmaWyV6cyiphwk8ZgKAJUe4TyVtmKm1WWcg7qZ6i` | `sora3vﾑ2zkaoUwﾋﾅGﾘﾚyﾂe3ﾖfﾙヰｶﾘﾉwｷnoWﾛYicaUr3ﾔｲﾖ2Ado3TﾘYQﾉJqﾜﾇｳﾑﾐd8dDjRGｦ3Vﾃ9HcﾀMヰR8ﾎﾖgEqGｵEｾDyc5ﾁ1ﾔﾉ31sUﾑﾀﾖaｸxﾘ3ｲｷMEuFｺｿﾉBQSVQnxﾈeJzrXLヰhｿｹ5SEEﾅPﾂﾗｸdヰﾋ1bUGHｲVXBWNNJ6K` |評議会ドメインのガバナンス定足数。 |
| `addr-multisig-wonderland-threshold2` | `≥2`、メンバー `(1,2)` | `3xsmkps1KPBn9dtpE5qHRhHEZCpiAe8d9j6H9A42TV6kc1TpaqdwnSksKgQrsSEHznqvWKBMc1os69BELzkLjsR7EV2gjV14d9JMzo97KEmYoKtxCrFeKFAcy7ffQdboV1uRt` | `sora2ﾖZﾘeｴAdx3ﾂﾉﾔXhnｹﾀ2ﾉｱﾋxﾅﾄﾌヱwﾐmﾊvEﾐCﾏﾎｦ1ﾑHﾋso2GKﾔﾕﾁwﾂﾃP6ﾁｼﾙﾖｺ9ｻｦbﾈ4wFdﾑFヰ3HaﾘｼMｷﾌHWtｷﾋLﾙﾖQ4D3XﾊﾜXmpktﾚｻ5ﾅﾅﾇ1gkﾏsCFQGH9` |デュアルシグネチャーワンダーランドの例 (ウェイト 1 + 2)。 |
| `addr-multisig-default-quorum3` | `≥3`、メンバー `(1,1,1,1)` | `nA2bDNhMqXz7ERkHNoEWbvJGyR1aDRsw32LaUWLgbK3vcpzohmdFCLvdotxUWWDY3aZeX4ptLk4Z6TjF5ossnJm8VrNo6daxmGTkqUyP4MxJxiNyPFxsEE5DLnsoLWUcxaWNpZ76tmkbiGS31Gv8tejKpuiHUMaQ1s5ohWyZvDnpycNkBK8AEfGJqn5yc9zAzfWbVhpDwkPj8ScnzvH1Echr5` | `soraﾐ38ﾅｴｸﾜ8ﾃzwBrqﾘｺ4yﾄv6kqJp1ｳｱﾛｿrzﾄﾃﾘﾒRﾗtV9ｼﾔPｽcヱEﾌVVVｼﾘｲZAｦﾓﾅｦeﾒN76vﾈcuｶuﾛL54rzﾙﾏX2zMﾌRLﾃﾋpﾚpｲcHﾑﾅﾃﾔzｵｲVfAﾃﾚﾎﾚCヰﾔｲｽｦw9ﾔﾕ8bGGkﾁ6sNｼaｻRﾖﾜYﾕﾚU18ﾅHヰﾌuMeﾊtﾂrｿj95Ft8ﾜ3fﾄkNiｴuﾈrCﾐQt8ヱｸｸmﾙﾒgUbﾑEKTTCM` |基本ガバナンスに使用される暗黙的なデフォルトのドメイン クォーラム。

#### 2.4 障害ルール (ADDR-1a)

- 必要なヘッダーとセレクターよりも短いペイロード、または残りのバイトがあるペイロードは、`AccountAddressError::InvalidLength` または `AccountAddressError::UnexpectedTrailingBytes` を出力します。
- 予約された `ext_flag` を設定するヘッダー、またはサポートされていないバージョン/クラスを宣伝するヘッダーは、`UnexpectedExtensionFlag`、`InvalidHeaderVersion`、または `UnknownAddressClass` を使用して拒否されなければなりません。
- 不明なセレクター/コントローラー タグにより、`UnknownDomainTag` または `UnknownControllerTag` が発生します。
- サイズが大きすぎるキーマテリアルまたは不正な形式のキーマテリアルにより、`KeyPayloadTooLong` または `InvalidPublicKey` が発生します。
- 255 メンバーを超えるマルチシグ コントローラーでは `MultisigMemberOverflow` が発生します。
- IME/NFKC 変換: 半角のそらかなは、デコードを中断することなく全角形式に正規化できますが、ASCII `sora` センチネルと I105 の数字/文字は ASCII のままでなければなりません。全角または大文字小文字を折り畳んだセンチネルは `ERR_MISSING_COMPRESSED_SENTINEL` を表示し、全角 ASCII ペイロードは `ERR_INVALID_COMPRESSED_CHAR` を発生させ、チェックサムの不一致は `ERR_CHECKSUM_MISMATCH` として発生します。 `crates/iroha_data_model/src/account/address.rs` のプロパティ テストはこれらのパスをカバーするため、SDK とウォレットは決定的な障害に依存できます。
- `address@domain` (rejected legacy form) エイリアスの Torii および SDK 解析では、I105 (優先)/sora (2 番目に良い) 入力がエイリアスのフォールバック前に失敗した場合 (例: チェックサムの不一致、ドメイン ダイジェストの不一致)、同じ `ERR_*` コードが出力されるようになりました。そのため、クライアントは散文文字列から推測することなく、構造化された理由を伝えることができます。
- 12 バイト未満のローカル セレクター ペイロードは `ERR_LOCAL8_DEPRECATED` を表示し、従来の Local‑8 ダイジェストからのハード カットオーバーを維持します。
- Domainless canonical I105 literals decode directly to a domainless `AccountId`. Use `ScopedAccountId` only when an interface requires explicit domain context.

#### 2.5 規範的なバイナリ ベクトル

- **暗黙的なデフォルト ドメイン (`default`、シード バイト `0x00`)**  
  正規の 16 進数: `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29`。  
  内訳: `0x02` ヘッダー、`0x00` セレクター (暗黙のデフォルト)、`0x00` コントローラー タグ、`0x01` カーブ ID (Ed25519)、`0x20` キーの長さ、その後に 32 バイトのキー ペイロードが続きます。
- **ローカル ドメイン ダイジェスト (`treasury`、シード バイト `0x01`)**  
  正規の 16 進数: `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c`。  
  内訳: `0x02` ヘッダー、セレクター タグ `0x01` とダイジェスト `b1 8f e9 c1 ab ba c4 5b 3e 38 fc 5d`、その後に単一キー ペイロード (`0x00` タグ、`0x01` 曲線 ID、`0x20` 長さ、32 バイト Ed25519キー）。

単体テスト (`account::address::tests::parse_encoded_accepts_all_formats`) は、`AccountAddress::parse_encoded` を介して以下の V1 ベクトルをアサートし、ツールが 16 進数、I105 (推奨)、および圧縮 (`sora`、次善の) 形式にわたる正規のペイロードに依存できることを保証します。 `cargo run -p iroha_data_model --example address_vectors` を使用して拡張フィクスチャ セットを再生成します。

|ドメイン |シードバイト |正規の 16 進数 |圧縮 (`sora`) |
|-----------|-----------|----------------------------------------------------------------------|------------|
|デフォルト | `0x00` | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` |
|財務省 | `0x01` | `0x0201b18fe9c1abbac45b3e38fc5d0001208a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
|ワンダーランド | `0x02` | `0x0201b8ae571b79c5a80f5834da2b0001208139770ea87d175f56a35466c34c7ecccb8d8a91b4ee37a25df60f5b8fc9b394` | `sora5ｻwﾓyRｿqﾏnMﾀﾙヰKoﾒﾇﾓQｺﾛyｼ3ｸFHB2F5LyPﾐTMZkｹｼw67ﾋVﾕｻr8ﾉGﾇeEnｻVRNKCS` |
|いろは | `0x03` | `0x0201de8b36819700c807083608e2000120ed4928c628d1c2c6eae90338905995612959273a5c63f93636c14614ac8737d1` | `sora5ｻﾜxﾀ7Vｱ7QFeｷMﾂLﾉﾃﾏﾓﾀTﾚgSav3Wnｱｵ4ｱCKｷﾛMﾘzヰHiﾐｱ6ﾃﾉﾁﾐZmﾇ2fiﾎX21P4L` |
|アルファ | `0x04` | `0x020146be2154ae86826a3fef0ec0000120ca93ac1705187071d67b83c7ff0efe8108e8ec4530575d7726879333dbdabe7c` | `sora5ｻ9JヱﾈｿuwU6ｴpﾔﾂﾈRqRTds1HﾃﾐｶLVﾍｳ9ﾔhｾNｵVｷyucEﾒGﾈﾏﾍ9sKeﾉDzrｷﾆ742WG1` |
|オメガ | `0x05` | `0x0201390d946885bc8416b3d30c9d0001206e7a1cdd29b0b78fd13af4c5598feff4ef2a97166e3ca6f2e4fbfccd80505bf1` | `sora5ｻ3zrﾌuﾚﾄJﾑXQhｸTyN8pzwRkWxmjVﾗbﾚﾕヰﾈoｽｦｶtEEﾊﾐ6GPｿﾓﾊｾEhvPｾｻ3XAJ73F` |
|ガバナンス | `0x06` | `0x0201989eb45a80940d187e2c908f0001208a875fff1eb38451577acd5afee405456568dd7c89e090863a0557bc7af49f17` | `sora5ｻiｵﾁyVﾕｽbFpDHHuﾇﾉdﾗｲﾓﾄRﾋAW3frUCｾ5ｷﾘTwdﾚnｽtQiLﾏｼｶﾅXgｾZmﾒヱH58H4KP` |
|バリデータ | `0x07` | `0x0201e4ffa58704c69afaeb7cc2d7000120ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c` | `sora5ｻﾀLDH6VYﾑNAｾgﾉVﾜtxﾊRXLｹﾍﾔﾌLd93GﾔGeｴﾄYrs1ﾂHｸkYxｹwｿyZﾗxyﾎZoXT1S4N` |
|探検家 | `0x08` | `0x02013b35422c65c2a83c99c523ad0001201398f62c6d1a457c51ba6a4b5f3dbd2f69fca93216218dc8997e416bd17d93ca` | `sora5ｻ4nmｻaﾚﾚPvNLgｿｱv6MHDeEyﾀovﾉJcpvrﾖ6ﾈCQcCNﾇﾜhﾚﾖyFdTwｸｶHEｱ9rWU8FMB` |
|ソラネット | `0x09` | `0x0201047d9ea7f5d5dbec3f7bfc58000120fd1724385aa0c75b64fb78cd602fa1d991fdebf76b13c58ed702eac835e9f618` | `sora5ｱｸヱVQﾂcﾁヱRﾓcApｲﾁﾅﾒvﾌﾏfｾNnﾛRJsｿDhﾙuHaﾚｺｦﾌﾍﾈeﾆﾎｺN1UUDｶ6ﾎﾄﾛoRH8JUL` |
|きつね | `0x0A` | `0x0201e91933de397fd7723dc9a76c00012043a72e714401762df66b68c26dfbdf2682aaec9f2474eca4613e424a0fbafd3c` | `sora5ｻﾚｺヱkfFJfSﾁｼJwﾉLvbpSｷﾔMWFMrbｳｸｲｲyヰKGJﾉｻ4ｹﾕrｽhｺｽzSDヰXAN62AD7RGNS` |
|だ | `0x0B` | `0x02016838cf5bb0ce0f3d4f380e1c00012066be7e332c7a453332bd9d0a7f7db055f5c5ef1a06ada66d98b39fb6810c473a` | `sora5ｻNﾒ5SﾐRﾉﾐﾃ62ｿ1ｶｷWFKyF1BcAﾔvｼﾐHqﾙﾐPﾏｴヰ5tｲﾕvnﾙT6ﾀW7mﾔ7ﾇﾗﾂｳ25CXS93` |

レビュー者: データ モデル WG、暗号化 WG — ADDR-1a の範囲が承認されました。

##### Sora Nexus 参照エイリアス

Sora Nexus ネットワークのデフォルトは `chain_discriminant = 0x02F1`
(`iroha_config::parameters::defaults::common::CHAIN_DISCRIMINANT`)。の
したがって、`AccountAddress::to_i105` ヘルパーと `to_i105` ヘルパーは次のように出力します。
すべての正規ペイロードに対して一貫したテキスト形式。から選択された備品
`fixtures/account/address_vectors.json` (経由で生成
`cargo xtask address-vectors`) を簡単な参照のために以下に示します。

|アカウント/セレクター | I105 リテラル (接頭辞 `0x02F1`) | Sora 圧縮 (`sora`) リテラル |
|---------------------|--------------------------------|--------------------------|
| `default` ドメイン (暗黙的なセレクター、シード `0x00`) | `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw` | `sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE` (明示的なルーティング ヒントを提供する場合は、オプションの `@default` サフィックス) |
| `treasury` (ローカル ダイジェスト セレクター、シード `0x01`) | `34mSYnCXkCzHXm31UDHh7SJfGvC4QPEhwim8z7sys2iHqXpCwCQkjL8KHvkFLSs1vZdJcb37r` | `sora5ｻu6rﾀCヰTGwﾏ1ﾅヱﾌQｲﾖﾇqCｦヰﾓZQCZRDSSﾅMｱﾙヱｹﾁｸ8ｾeﾄﾛ6C8bZuwﾗｹCZｦRSLQFU` |
|グローバル レジストリ ポインター (`registry_id = 0x0000_002A`、`treasury` と同​​等) | `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF` | `sorakXｹ6NｻﾍﾀﾖSﾜﾖｱ3ﾚ5WﾘﾋQﾅｷｦxgﾛｸcﾁｵﾋkﾋvﾏ8SPﾓﾀｹdｴｴｲW9iCM6AEP` |

これらの文字列は、CLI (`iroha tools address convert`) によって出力された文字列と一致します。鳥井
応答 (`canonical I105 literal rendering`) と SDK ヘルパー、つまり UX のコピー/ペースト
フローはそれらをそのまま信頼できます。 `<address>@<domain>` (rejected legacy form) は、明示的なルーティング ヒントが必要な場合にのみ追加します。サフィックスは正規の出力の一部ではありません。

#### 2.6 相互運用性のためのテキスト エイリアス (計画中)

- **チェーンエイリアスのスタイル:** `ih:<chain-alias>:<alias@domain>` (ログと人間の場合)
  エントリー。ウォレットはプレフィックスを解析し、埋め込まれたチェーンを検証してブロックする必要があります。
  不一致。
- **CAIP-10 フォーム:** `iroha:<caip-2-id>:<i105-addr>` (チェーンに依存しない)
  統合。このマッピングは出荷された製品では**まだ実装されていません**
  ツールチェーン。
- **マシン ヘルパー:** Rust、TypeScript/JavaScript、Python、
  I105 と圧縮形式をカバーする Kotlin (`AccountAddress::to_i105`、
  `AccountAddress::parse_encoded`、および同等の SDK)。 CAIP-10 ヘルパーは、
  今後の仕事。

#### 2.7 決定的な I105 エイリアス

- **プレフィックス マッピング:** `chain_discriminant` を I105 ネットワーク プレフィックスとして再利用します。
  `encode_i105_prefix()` (`crates/iroha_data_model/src/account/address.rs` を参照)
  `<64` の値に対して 6 ビットのプレフィックス (シングル バイト) と 14 ビットの 2 バイトを出力します。
  大規模なネットワーク用のフォーム。権限のある割り当ては次の場所にあります
  [`address_prefix_registry.md`](source/references/address_prefix_registry.md);
  SDK は、衝突を避けるために、一致する JSON レジストリの同期を維持しなければなりません。
- **アカウント資料:** I105 は、によって構築された正規ペイロードをエンコードします。
  `AccountAddress::canonical_bytes()` - ヘッダー バイト、ドメイン セレクター、および
  コントローラーのペイロード。追加のハッシュ手順はありません。 I105 には、
  Rust によって生成されたバイナリ コントローラ ペイロード (単一キーまたはマルチシグ)
  マルチシグ ポリシー ダイジェストに使用される CTAP2 マップではなく、エンコーダです。
- **エンコーディング:** `encode_i105()` はプレフィックス バイトを正規文字列と連結します。
  ペイロードを作成し、Blake2b-512 から派生した 16 ビット チェックサムを追加します。
  接頭辞 `I105PRE` (`b"I105PRE" || prefix || payload`)。結果は、`bs58` によって Base58 でエンコードされます。
  CLI/SDK ヘルパーは同じプロシージャを公開しており、`AccountAddress::parse_encoded`
  `decode_i105` を介してそれを反転します。

#### 2.8 規範的なテキストテストベクトル

`fixtures/account/address_vectors.json` には完全な I105 (推奨) と圧縮 (`sora`、2 番目に優れた) が含まれています
すべての正規ペイロードのリテラル。ハイライト:

- **`addr-single-default-ed25519` (Sora Nexus、プレフィックス `0x02F1`)。**  
  I105 `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`、圧縮 (`sora`)
  `sora2QG…U4N5E5`。鳥居は、`AccountId` からこれらの正確な文字列を出力します。
  `Display` 実装 (標準 I105) および `AccountAddress::to_i105`。
- **`addr-global-registry-002a` (レジストリ セレクタ → 財務)。**  
  I105 `3oE9sLeRGP49Cu7mQ1nF4wtKAm29BG4TGLiRsaXe7mhbMP5WZ113nNW1N6RbqF`、圧縮 (`sora`)
  `sorakX…CM6AEP`。レジストリ セレクターが引き続きデコードされることを示します。
  対応するローカル ダイジェストと同じ正規ペイロード。
- **失敗ケース (`i105-prefix-mismatch`)。**  
  ノード上でプレフィックス `NETWORK_PREFIX + 1` でエンコードされた I105 リテラルを解析する
  デフォルトのプレフィックスの結果を期待すると、
  `AccountAddressError::UnexpectedNetworkPrefix { expected: 753, found: 754 }`
  ドメインルーティングが試行される前に。 `i105-checksum-mismatch` フィクスチャ
  Blake2b チェックサムに対して改ざん検出を実行します。

#### 2.9 コンプライアンス対策

ADDR‑2には、ポジティブとネガティブをカバーする再生可能なフィクスチャバンドルが同梱されています
標準 16 進数、I105 (推奨)、圧縮 (`sora`、半角/全角)、暗黙的なシナリオ
デフォルト セレクター、グローバル レジストリ エイリアス、およびマルチシグネチャ コントローラー。の
正規の JSON は `fixtures/account/address_vectors.json` にあり、
で再生成されました:

```
cargo xtask address-vectors --out fixtures/account/address_vectors.json
# verify without writing:
cargo xtask address-vectors --verify
```

アドホック実験 (異なるパス/形式) の場合、サンプル バイナリはそのままです。
利用可能:

```
cargo run -p iroha_data_model --example account_address_vectors > fixtures/account/address_vectors.json
```

`crates/iroha_data_model/tests/account_address_vectors.rs` での Rust 単体テスト
`crates/iroha_torii/tests/account_address_vectors.rs` と JS を組み合わせて、
Swift および Android ハーネス (`javascript/iroha_js/test/address.test.js`、
`IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift`、
`java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java`)、
SDK 間でのコーデックのパリティと Torii アドミッションを保証するために、同じフィクスチャを使用します。

### 3. グローバルに固有のドメインと正規化

参照: [`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md)
Torii、データモデル、SDK 全体で使用される正規の Norm v1 パイプライン用。

`DomainId` をタグ付きタプルとして再定義します。

```
DomainId {
    name: Name,
    authority: GlobalDomainAuthority, // new enum
}

enum GlobalDomainAuthority {
    LocalChain,                  // default for the local chain
    External { chain_discriminant: u16 },
}
```

`LocalChain` は、現在のチェーンによって管理されているドメインの既存の名前をラップします。
ドメインがグローバル レジストリを通じて登録されると、所有権が永続化されます。
チェーンの判別式。表示/解析は今のところ変更されていませんが、
拡張された構造により、ルーティングの決定が可能になります。

#### 3.1 正規化とスプーフィング防御

Norm v1 は、すべてのコンポーネントがドメインの前に使用する必要がある正規のパイプラインを定義します
名前は永続化されるか、`AccountAddress` に埋め込まれます。完全なウォークスルー
[`docs/source/references/address_norm_v1.md`](source/references/address_norm_v1.md) に存在します。
以下の概要は、ウォレット、トリイ、SDK、ガバナンスのステップをまとめています。
ツールは実装する必要があります。

1. **入力検証。** 空の文字列、空白、および予約された文字列を拒否します。
   区切り文字 `@`、`#`、`$`。これは、によって強制される不変式と一致します。
   `Name::validate_str`。
2. **Unicode NFC 構成** ICU 支援の NFC 正規化を標準的に適用する
   同等のシーケンスは決定論的に折りたたまれます (例: `e\u{0301}` → `é`)。
3. **UTS-46 正規化。** UTS-46 を介して NFC 出力を実行します。
   `use_std3_ascii_rules = true`、`transitional_processing = false`、および
   DNS 長の強制が有効になっています。結果は、小文字の A ラベル シーケンスになります。
   STD3 ルールに違反する入力はここで失敗します。
4. **長さの制限。** DNS スタイルの境界を適用します。各ラベルは 1 ～ 63 である必要があります。
   ステップ 3 の後、完全なドメインは 255 バイトを超えてはなりません。
5. **オプションの紛らわしいポリシー** UTS‑39 スクリプト チェックは次のように追跡されます。
   ノルム v2;オペレータはそれらを早期に有効にすることができますが、チェックに失敗した場合は中止する必要があります
   処理中。

すべてのステージが成功すると、小文字の A ラベル文字列がキャッシュされ、
アドレスエンコーディング、構成、マニフェスト、およびレジストリ検索。ローカルダイジェスト
セレクターは 12 バイトの値を `blake2s_mac(key = "SORA-LOCAL-K:v1",
canonical_label)[0..12]` ステップ 3 の出力を使用します。他のすべての試行 (混合
大文字、大文字、生の Unicode 入力) は構造化されたものでは拒否されます。
`ParseError`s は、名前が指定された境界にあります。

これらのルールを実証する正規フィクスチャ (punycode ラウンドトリップを含む)
および無効な STD3 シーケンス — にリストされています。
`docs/source/references/address_norm_v1.md` は SDK CI にミラーリングされます
ベクトル スイートは ADDR-2 で追跡されます。

### 4. Nexus ドメインのレジストリとルーティング

- **レジストリ スキーマ:** Nexus は署名付きマップ `DomainName -> ChainRecord` を維持します
  `ChainRecord` には、チェーン判別のオプションのメタデータ (RPC) が含まれます。
  エンドポイント）、および権限の証明（例：ガバナンスマルチ署名）。
- **同期メカニズム:**
  - チェーンは署名されたドメイン クレームを Nexus に送信します (生成中または経由で)
    ガバナンス指導）。
  - Nexus は定期的なマニフェスト (署名付き JSON とオプションの Merkle ルート) を公開します。
    HTTPS およびコンテンツ アドレス ストレージ (IPFS など) 経由で。クライアントは、
    最新のマニフェストを使用して署名を検証します。
- **検索フロー:**
  - 鳥居は、`DomainId` を参照するトランザクションを受け取ります。
  - ドメインがローカルで不明な場合、Torii はキャッシュされた Nexus マニフェストをクエリします。
  - マニフェストが外部チェーンを示している場合、トランザクションは次のように拒否されます。
    決定的な `ForeignDomain` エラーとリモート チェーン情報。
  - Nexus にドメインがない場合、Torii は `UnknownDomain` を返します。
- **トラスト アンカーとローテーション:** ガバナンス キーはマニフェストに署名します。回転とか
  失効は新しいマニフェスト エントリとして公開されます。クライアントはマニフェストを強制します
  TTL (例: 24 時間) を設定し、その期間を超える古いデータの参照を拒否します。
- **失敗モード:** マニフェストの取得が失敗した場合、Torii はキャッシュされた状態にフォールバックします。
  TTL内のデータ。 TTL を超えると `RegistryUnavailable` が発行され、拒否されます
  不整合な状態を避けるためのクロスドメインルーティング。

### 4.1 レジストリの不変性、エイリアス、および廃棄 (ADDR-7c)

Nexus は **追加専用マニフェスト** を公開しているため、すべてのドメインまたはエイリアスの割り当てが
監査して再生することができます。オペレータは、に記載されているバンドルを扱う必要があります。
[アドレス マニフェスト Runbook](source/runbooks/address_manifest_ops.md) として
唯一の真実の情報源: マニフェストが見つからない場合、または検証に失敗した場合、鳥井は次のことを行う必要があります。
影響を受けるドメインの解決を拒否します。

自動化サポート: `cargo xtask address-manifest verify --bundle <current_dir> --previous <previous_dir>`
に記載されているチェックサム、スキーマ、および以前のダイジェスト チェックを再生します。
ランブック。 `sequence` を表示するには、変更チケットにコマンド出力を含めます。
`previous_digest` リンクはバンドルを公開する前に検証されました。

#### マニフェストヘッダーと署名契約

|フィールド |要件 |
|------|-----------|
| `version` |現在 `1` です。一致する仕様の更新でのみバンプします。 |
| `sequence` |パブリケーションごとに **ちょうど** 1 ずつ増加します。 Torii キャッシュは、ギャップや回帰のあるリビジョンを拒否します。 |
| `generated_ms` + `ttl_hours` |キャッシュの鮮度を確立します (デフォルトは 24 時間)。次の公開前に TTL が期限切れになると、鳥井は `RegistryUnavailable` に切り替わります。 |
| `previous_digest` |以前のマニフェスト本体の BLAKE3 ダイジェスト (16 進数)。検証者は `b3sum` を使用してそれを再計算し、不変性を証明します。 |
| `signatures` |マニフェストは Sigstore (`cosign sign-blob`) 経由で署名されます。運用は、ロールアウト前に `cosign verify-blob --bundle manifest.sigstore manifest.json` を実行し、ガバナンス ID/発行者の制約を適用する必要があります。 |

リリース オートメーションは `manifest.sigstore` と `checksums.sha256` を発行します
JSON 本文の横にあります。 SoraFS にミラーリングするときにファイルをまとめて保存するか、
HTTP エンドポイントにより、監査人は検証手順をそのまま再現できます。

#### エントリの種類

|タイプ |目的 |必須フィールド |
|------|------|------|
| `global_domain` |ドメインがグローバルに登録され、チェーン識別子と I105 プレフィックスにマップされる必要があることを宣言します。 | `{ "domain": "<label>", "chain": "sora:nexus:global", "i105_prefix": 753, "selector": "global" }` |
| `tombstone` |エイリアス/セレクターを永久に廃止します。 Local‑8 ダイジェストを消去する場合、またはドメインを削除する場合に必要です。 | `{ "selector": {…}, "reason_code": "LOCAL8_RETIREMENT" \| …, "ticket": "<governance id>", "replaces_sequence": <number> }` |

`global_domain` エントリには、オプションで `manifest_url` または `sorafs_cid` を含めることができます。
ウォレットを署名付きチェーンメタデータに向けますが、正規タプルは残ります
`{domain, chain, discriminant/i105_prefix}`。 `tombstone` レコードは**引用する必要があります**
廃止されたセレクターと、許可したチケット/ガバナンス アーティファクト
変更により、監査証跡がオフラインで再構築可能になります。

#### エイリアス/トゥームストーンのワークフローとテレメトリ

1. **ドリフトを検出します。** `torii_address_local8_total{endpoint}` を使用します。
   `torii_address_local8_domain_total{endpoint,domain}`、
   `torii_address_collision_total{endpoint,kind="local12_digest"}`、
   `torii_address_collision_domain_total{endpoint,domain}`、
   `torii_address_domain_total{endpoint,domain_kind}`、および
   `torii_address_invalid_total{endpoint,reason}` (レンダリング形式)
   `dashboards/grafana/address_ingest.json`) ローカル送信を確認し、
   Local-12 の衝突は、トゥームストーンが提案されるまでゼロのままです。の
   ドメインごとのカウンターにより、所有者は開発/テスト ドメインのみが Local‑8 を発行することを証明できます。
   トラフィック（および Local‑12 コリジョンが既知のステージング ドメインにマッピングされること）
   **Domain Kind Mix (5m)** パネルが含まれているため、SRE はその割合をグラフ化できます。
   `domain_kind="local12"` トラフィックは残り、`AddressLocal12Traffic`
   アラートは、
   退職の門。
2. **正規ダイジェストを取得します。** 実行します。
   `iroha tools address convert <address> --format json --expect-prefix 753`
   (または `fixtures/account/address_vectors.json` を介して消費します
   `scripts/account_fixture_helper.py`) を使用して、正確な `digest_hex` をキャプチャします。
   CLI は、I105、`i105`、および正規の `0x…` リテラルを受け入れます。追加する
   `@<domain>` は、マニフェストのラベルを保持する必要がある場合のみ。
   JSON 概要では、`input_domain` フィールドを介してそのドメインが表示されます。
   `legacy  suffix` は、変換されたエンコーディングを `<address>@<domain>` (rejected legacy form) として再生します。
   マニフェストの差分 (このサフィックスはメタデータであり、正規のアカウント ID ではありません)。
   改行指向のエクスポートの場合は、次を使用します
   `iroha tools address normalize --input <file> legacy-selector input mode` でローカルを一括変換します
   スキップ中の正規 I105 (推奨)、圧縮 (`sora`、2 番目に良い)、16 進数、または JSON 形式へのセレクター
   非ローカル行。監査人がスプレッドシートに適した証拠を必要とする場合は、次のコマンドを実行します。
   `iroha tools address audit --input <file> --format csv` は CSV 概要を出力します
   (`input,status,format,domain_kind,…`) ローカル セレクターを強調表示します。
   正規のエンコーディング、および同じファイル内の解析エラー。
3. **マニフェスト エントリを追加します。** `tombstone` レコード (およびフォローアップ) を下書きします。
   `global_domain` グローバル レジストリに移行するときのレコード) を検証します
   署名をリクエストする前に、マニフェストに `cargo xtask address-vectors` を付けてください。
4. **検証して公開します。** ランブックのチェックリストに従います (ハッシュ、Sigstore、
   シーケンス単調性) を確認してから、バンドルを SoraFS にミラーリングします。今の鳥居
   バンドルが到着した直後に、I105 (推奨)/sora (2 番目に優れた) リテラルを正規化します。
5. **監視とロールバック。** Local-8 および Local-12 コリジョン パネルをそのままにしておきます。
   30日間はゼロ。回帰が発生した場合は、以前のマニフェストを再公開します
   テレメトリが安定するまでは、影響を受ける非実稼働環境でのみ実行されます。

上記の手順はすべて、ADDR‑7c の必須証拠です。
`cosign` 署名バンドル、または `previous_digest` 値が一致しない必要があります。
は自動的に拒否され、オペレーターは検証ログを添付する必要があります。
彼らの変更チケット。

### 5. ウォレットと API の人間工学

- **デフォルトの表示:** ウォレットには I105 アドレス (短い、チェックサム付き) が表示されます。
  さらに、レジストリからフェッチされたラベルとして解決されたドメインが追加されます。ドメインは
  変更される可能性がある説明的なメタデータとして明確にマークされていますが、I105 は
  安定したアドレス。
- **入力の正規化:** Torii と SDK は I105 (推奨)/sora (2 番目に優れた)/0x を受け入れます。
  アドレスに加えて、`alias@domain` (rejected legacy form)、`uaid:…`、および
  `opaque:…` フォームを作成し、出力用に I105 に正規化します。ありません
  厳密モードの切り替え。生の電話/電子メール識別子は台帳から外しておく必要があります
  UAID/不透明マッピング経由。
- **エラー防止:** ウォレットは I105 プレフィックスを解析し、チェーン判別を強制します。
  期待。チェーンの不一致によりハード障害が引き起こされ、実用的な診断が行われます。
- **コーデック ライブラリ:** 公式 Rust、TypeScript/JavaScript、Python、Kotlin
  ライブラリは、I105 エンコーディング/デコーディングと圧縮 (`sora`) のサポートを提供します。
  断片化された実装を避けます。 CAIP-10 変換はまだ出荷されていません。

#### アクセシビリティと安全な共有に関するガイダンス

- 製品表面の実装ガイダンスはライブで追跡されます。
  `docs/portal/docs/reference/address-safety.md`;いつでもそのチェックリストを参照してください
  これらの要件をウォレットまたはエクスプローラーの UX に適応させます。
- **安全な共有フロー:** アドレスをコピーまたは表示するサーフェスは、デフォルトで I105 形式になり、完全な文字列と同じペイロードから派生した QR コードの両方を表示する隣接する「共有」アクションを公開するため、ユーザーは目視またはスキャンによってチェックサムを確認できます。切り捨てが避けられない場合 (小さい画面など)、文字列の先頭と末尾を保持し、明確な省略記号を追加し、クリップボードへのコピーを介して完全なアドレスにアクセスできるようにして、誤って切り取られるのを防ぎます。
- **IME セーフガード:** アドレス入力は、IME/IME スタイルのキーボードからの合成アーティファクトを拒否しなければなりません (MUST)。 ASCII のみの入力を強制し、全角またはカナ文字が検出されたときにインライン警告を表示し、検証前に結合マークを削除するプレーンテキストの貼り付けゾーンを提供するため、日本人と中国人のユーザーは進行状況を失うことなく IME を無効にできます。
- **スクリーン リーダーのサポート:** 先頭の Base58 プレフィックス数字を説明する視覚的に隠されたラベル (`aria-label`/`aria-describedby`) を提供し、I105 ペイロードを 4 文字または 8 文字のグループに分割するため、支援技術はランオン文字列の代わりにグループ化された文字を読み取ります。礼儀正しいライブ リージョンを通じてコピー/共有の成功を発表し、QR プレビューに説明的な代替テキスト (「チェーン 0x02F1 の <エイリアス> の I105 アドレス」) が含まれていることを確認します。
- **Sora のみの圧縮の使用:** `i105` 圧縮ビューには必ず「Sora のみ」というラベルを付け、コピーする前に明示的な確認を行ってゲートします。 SDKとウォレットは、チェーン判別式がSora Nexus値ではない場合、圧縮された出力の表示を拒否しなければならず、資金の誤ったルーティングを避けるために、ユーザーをネットワーク間送金のためにI105に戻す必要があります。

## 実装チェックリスト

- **I105 エンベロープ:** プレフィックスは、コンパクト形式を使用して `chain_discriminant` をエンコードします。
  `encode_i105_prefix()` の 6/14 ビット スキーム、本文は正規のバイトです
  (`AccountAddress::canonical_bytes()`)、チェックサムは最初の 2 バイトです
  Blake2b-512 (`b"I105PRE"` || 接頭辞 || 本文) の。完全なペイロードは Base58 です。
  `bs58` 経由でエンコードされます。
- **レジストリ契約:** 署名付き JSON (およびオプションの Merkle ルート) の公開
  `{discriminant, i105_prefix, chain_alias, endpoints}` 24 時間 TTL と
  回転キー。
- **ドメイン ポリシー:** ASCII `Name` 今日; i18n を有効にする場合は、UTS-46 を適用します。
  正規化と、紛らわしいチェックのための UTS-39。最大ラベル (63) を強制し、
  合計 (255) の長さ。
- **テキストヘルパー:** Rust で I105 ↔ 圧縮 (`i105`) コーデックを出荷、
  共有テスト ベクトルを使用した TypeScript/JavaScript、Python、Kotlin (CAIP-10)
  マッピングは今後の作業として残ります)。
- **CLI ツール:** `iroha tools address convert` を介して決定論的なオペレーター ワークフローを提供します。
  (`crates/iroha_cli/src/address.rs` を参照)。I105/`0x…` リテラルを受け入れます。
  オプションの `<address>@<domain>` (rejected legacy form) ラベル。デフォルトは Sora Nexus プレフィックス (`753`) を使用した I105 出力です。
  オペレーターが明示的に要求した場合にのみ、Sora のみの圧縮アルファベットを出力します。
  `--format i105` または JSON サマリー モード。このコマンドは、プレフィックスの期待を強制します。
  解析し、指定されたドメイン (JSON の `input_domain`) と `legacy  suffix` フラグを記録します
  変換されたエンコーディングを `<address>@<domain>` (rejected legacy form) として再生するため、マニフェストの差分は人間工学に基づいたままになります。
- **ウォレット/エクスプローラー UX:** [アドレス表示ガイドライン](source/sns/address_display_guidelines.md) に従ってください。
  ADDR-6 に同梱 - デュアル コピー ボタンを提供し、I105 を QR ペイロードとして保持し、警告を表示します
  ユーザーは、圧縮された `i105` フォームは Sora 専用であり、IME の書き換えの影響を受けやすいことを認識しています。
- **Torii の統合:** TTL を考慮したキャッシュ Nexus マニフェスト、エミット
  `ForeignDomain`/`UnknownDomain`/`RegistryUnavailable` を決定的に、そして
  keep strict account-literal parsing canonical-I105-only (reject compressed and any `@domain` suffix) with canonical I105 output.

### 鳥居応答フォーマット

- `GET /v2/accounts` は、オプションの `canonical I105 rendering` クエリ パラメータを受け入れ、
  `POST /v2/accounts/query` は、JSON エンベロープ内の同じフィールドを受け入れます。
  サポートされている値は次のとおりです。
  - `i105` (デフォルト) — 応答は正規の I105 Base58 ペイロードを出力します (例:
    `6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw`)。
  - `i105_default` — 応答は Sora のみの `i105` 圧縮ビューを生成します。
    フィルタ/パスパラメータを正規に保ちます。
- 無効な値は `400` (`QueryExecutionFail::Conversion`) を返します。これにより、
  ウォレットとエクスプローラーは、Sora のみの UX の圧縮文字列をリクエストします。
  I105 を相互運用可能なデフォルトとして維持します。
- アセットホルダーのリスト (`GET /v2/assets/{definition_id}/holders`) とその JSON
  対応する封筒 (`POST …/holders/query`) も `canonical I105 rendering` を尊重します。
  `items[*].account_id` フィールドは、
  パラメータ/エンベロープ フィールドが `i105_default` に設定され、アカウントがミラーリングされます
  エンドポイントを使用して、エクスプローラーがディレクトリ間で一貫した出力を表示できるようにします。
- **テスト:** エンコーダー/デコーダーのラウンドトリップ、間違ったチェーンの単体テストを追加
  失敗とマニフェスト検索。 Torii と SDK の統合範囲を追加
  I105 フローの場合はエンドツーエンドです。

## エラーコードレジストリ

アドレス エンコーダとデコーダは、次のような方法で障害を明らかにします。
`AccountAddressError::code_str()`。次の表に安定したコードを示します。
SDK、ウォレット、および Torii サーフェスは人間が判読できるものと並行して表示される必要がある
メッセージと推奨される修復ガイダンス。

### 正規の構築

|コード |失敗 |推奨される修復 |
|------|------|----------------------|
| `ERR_UNSUPPORTED_ALGORITHM` |エンコーダーは、レジストリまたはビルド機能でサポートされていない署名アルゴリズムを受け取りました。 |アカウントの構築を、レジストリと構成で有効になっている曲線に制限します。 |
| `ERR_KEY_PAYLOAD_TOO_LONG` |署名キーのペイロードの長さがサポートされている制限を超えています。 |シングルキー コントローラは `u8` の長さに制限されています。大きな公開鍵にはマルチシグを使用します (ML‑DSA など)。 |
| `ERR_INVALID_HEADER_VERSION` |アドレス ヘッダーのバージョンがサポートされている範囲外です。 | V1 アドレスのヘッダー バージョン `0` を出力します。新しいバージョンを採用する前にエンコーダをアップグレードしてください。 |
| `ERR_INVALID_NORM_VERSION` |正規化バージョンフラグが認識されません。 |正規化バージョン `1` を使用し、予約ビットの切り替えを避けてください。 |
| `ERR_INVALID_I105_PREFIX` |要求された I105 ネットワーク プレフィックスをエンコードできません。 |チェーン レジストリで公開されている `0..=16383` の範囲内でプレフィックスを選択してください。 |
| `ERR_CANONICAL_HASH_FAILURE` |正規ペイロードのハッシュ化に失敗しました。 |操作を再試行してください。エラーが続く場合は、ハッシュ スタックの内部バグとして扱います。 |

### フォーマットのデコードと自動検出

|コード |失敗 |推奨される修復 |
|------|------|----------------------|
| `ERR_INVALID_I105_ENCODING` | I105 文字列にアルファベット以外の文字が含まれています。 |アドレスが公開されている I105 アルファベットを使用しており、コピー/ペースト中に切り捨てられていないことを確認してください。 |
| `ERR_INVALID_LENGTH` |ペイロードの長さが、セレクター/コントローラーの予想される標準サイズと一致しません。 |選択したドメイン セレクターとコントローラー レイアウトの完全な正規ペイロードを指定します。 |
| `ERR_CHECKSUM_MISMATCH` | I105 (推奨) または圧縮 (`sora`、2 番目に良い) チェックサム検証が失敗しました。 |信頼できるソースからアドレスを再生成します。これは通常、コピー/貼り付けエラーを示します。 |
| `ERR_INVALID_I105_PREFIX_ENCODING` | I105 プレフィックス バイトの形式が不正です。 |準拠したエンコーダを使用してアドレスを再エンコードします。先頭の Base58 バイトを手動で変更しないでください。 |
| `ERR_INVALID_HEX_ADDRESS` |正規の 16 進数形式をデコードできませんでした。 |公式エンコーダによって生成された、`0x` 接頭辞付きの偶数長の 16 進文字列を指定します。 |
| `ERR_MISSING_COMPRESSED_SENTINEL` |圧縮形式は `sora` で始まりません。 |デコーダに渡す前に、圧縮された Sora アドレスに必要なセンチネルをプレフィックスとして付けます。 |
| `ERR_COMPRESSED_TOO_SHORT` |圧縮された文字列には、ペイロードとチェックサムに十分な桁がありません。 |切り詰められたスニペットの代わりに、エンコーダーによって出力された完全な圧縮文字列を使用します。 |
| `ERR_INVALID_COMPRESSED_CHAR` |圧縮されたアルファベット以外の文字が見つかりました。 |文字を、公開されている半角/全角テーブルの有効な Base-105 グリフに置き換えます。 |
| `ERR_INVALID_COMPRESSED_BASE` |エンコーダがサポートされていない基数を使用しようとしました。 |エンコーダに対してバグを報告します。 V1 では、圧縮アルファベットは基数 105 に固定されています。 |
| `ERR_INVALID_COMPRESSED_DIGIT` |数字の値が圧縮されたアルファベットのサイズを超えています。 |各桁が `0..105)` 以内であることを確認し、必要に応じてアドレスを再生成します。 |
| `ERR_UNSUPPORTED_ADDRESS_FORMAT` |自動検出で入力形式を認識できませんでした。 |パーサーを呼び出すときは、I105 (推奨)、圧縮 (`sora`)、または正規の `0x` 16 進文字列を指定します。 |

### ドメインとネットワークの検証

|コード |失敗 |推奨される修復 |
|------|------|----------------------|
| `ERR_DOMAIN_MISMATCH` |ドメイン セレクターが予期されたドメインと一致しません。 |目的のドメインに対して発行されたアドレスを使用するか、期待値を更新します。 |
| `ERR_INVALID_DOMAIN_LABEL` |ドメインラベルが正規化チェックに失敗しました。 |エンコード前に UTS-46 の非移行処理を使用してドメインを正規化します。 |
| `ERR_UNEXPECTED_NETWORK_PREFIX` |デコードされた I105 ネットワーク プレフィックスが設定値と異なります。 |ターゲット チェーンのアドレスに切り替えるか、予期される識別子/プレフィックスを調整します。 |
| `ERR_UNKNOWN_ADDRESS_CLASS` |アドレス クラス ビットが認識されません。 |デコーダを新しいクラスを理解できるリリースにアップグレードするか、ヘッダー ビットの改ざんを避けてください。 |
| `ERR_UNKNOWN_DOMAIN_TAG` |ドメイン セレクター タグが不明です。 |新しいセレクター タイプをサポートするリリースに更新するか、V1 ノードで実験的なペイロードの使用を避けてください。 |
| `ERR_UNEXPECTED_EXTENSION_FLAG` |予約された拡張ビットが設定されました。 |予約ビットをクリアします。将来の ABI で導入されるまで、ゲートされたままになります。 |
| `ERR_UNKNOWN_CONTROLLER_TAG` |コントローラーのペイロード タグが認識されません。 |新しいコントローラー タイプを解析する前に認識できるようにデコーダーをアップグレードします。 |
| `ERR_UNEXPECTED_TRAILING_BYTES` |正規ペイロードには、デコード後の末尾バイトが含まれていました。 |正規ペイロードを再生成します。文書化された長さのみが存在する必要があります。 |

### コントローラーのペイロードの検証

|コード |失敗 |推奨される修復 |
|------|------|----------------------|
| `ERR_INVALID_PUBLIC_KEY` |キーバイトが宣言されたカーブと一致しません。 |キー バイトが、選択したカーブに必要なとおりに正確にエンコードされていることを確認します (例: 32 バイト Ed25519)。 |
| `ERR_UNKNOWN_CURVE` |曲線識別子が登録されていません。 |追加の曲線が承認され、レジストリで公開されるまで、曲線 ID `1` (Ed25519) を使用してください。 |
| `ERR_MULTISIG_MEMBER_OVERFLOW` |マルチシグ コントローラーがサポートされている数を超えるメンバーを宣言しています。 |エンコードする前に、マルチシグ メンバーシップを文書化された制限まで減らします。 |
| `ERR_INVALID_MULTISIG_POLICY` |マルチシグ ポリシー ペイロードの検証 (しきい値/重み/スキーマ) に失敗しました。 | CTAP2 スキーマ、重み制限、およびしきい値制約を満たすようにポリシーを再構築します。 |

## 検討された代替案

- **純粋な Base58Check (ビットコイン スタイル)。** チェックサムは単純ですが、エラー検出は弱い
  Blake2b 由来の I105 チェックサムよりも優れています (`encode_i105` は 512 ビット ハッシュを切り捨てます)
  また、16 ビット判別式の明示的なプレフィックス セマンティクスがありません。
- **ドメイン文字列にチェーン名を埋め込みます (例: `finance@chain`)。** ブレーク
- **アドレスを変更せずに Nexus ルーティングのみに依存します。** ユーザーは引き続き
  あいまいな文字列をコピー/ペーストします。アドレス自体にコンテキストを伝える必要があります。
- **Bech32m エンベロープ。** QR フレンドリーで、人間が判読できるプレフィックスを提供しますが、
  出荷時の I105 実装とは異なる可能性があります (`AccountAddress::to_i105`)
  すべてのフィクスチャ/SDK を再作成する必要があります。現在のロードマップでは I105 + が維持されます。
  将来の研究を継続しながら、圧縮 (`sora`) をサポート
  Bech32m/QR レイヤ (CAIP-10 マッピングは延期されます)。

## 未解決の質問

- `u16` 判別式と予約された範囲が長期的な需要をカバーしていることを確認します。
  それ以外の場合は、Variant エンコーディングを使用して `u32` を評価します。
- レジストリ更新のためのマルチシグネチャ ガバナンス プロセスとその方法を最終決定する
  取り消し/期限切れの割り当ては処理されます。
- 正確なマニフェスト署名スキーム (Ed25519 マルチ署名など) を定義し、
  Nexus ディストリビューションのトランスポート セキュリティ (HTTPS ピニング、IPFS ハッシュ形式)。
- 移行のためにドメイン エイリアス/リダイレクトをサポートするかどうか、およびその方法を決定します。
  決定論を壊すことなくそれらを表面化すること。
- 言霊/IVM コントラクトが I105 ヘルパーにアクセスする方法を指定します (`to_address()`、
  `parse_address()`)、およびオンチェーン ストレージが CAIP-10 を公開する必要があるかどうか
  マッピング (現在では I105 が正規です)。
- 外部レジストリ (I105 レジストリ、
  CAIP 名前空間ディレクトリ）を使用して、より広範なエコシステムの連携を実現します。

## 次のステップ

1. I105 エンコーディングは `iroha_data_model` (`AccountAddress::to_i105`,
   `parse_encoded`);フィクスチャ/テストのすべての SDK への移植を続行し、すべての SDK をパージします。
   Bech32m プレースホルダー。
2. `chain_discriminant` を使用して構成スキーマを拡張し、適切なスキーマを導き出す
  既存のテスト/開発セットアップのデフォルト。 **(完了: `common.chain_discriminant`
  現在は `iroha_config` で出荷され、デフォルトはネットワークごとの `0x02F1` です
  オーバーライドします。)**
3. Nexus レジストリ スキーマと概念実証マニフェスト パブリッシャーの草案を作成します。
4. ウォレットプロバイダーとカストディアンから人的要因に関するフィードバックを収集する
   (HRP の命名、表示形式)。
5. ドキュメント (`docs/source/data_model.md`、Torii API ドキュメント) を更新したら、
   実装パスがコミットされています。
6. 公式コーデック ライブラリ (Rust/TS/Python/Kotlin) を規範的なテストとともに出荷する
   成功例と失敗例をカバーするベクトル。
