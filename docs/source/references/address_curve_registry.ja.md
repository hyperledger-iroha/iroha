---
title: アカウント曲線レジストリ
description: アカウントコントローラの曲線識別子と署名アルゴリズムのカノニカル対応表。
---

# アカウント曲線レジストリ

アカウントアドレスは、コントローラを 8 ビットの曲線識別子で始まる
タグ付きペイロードとしてエンコードします。バリデータ、SDK、ツールは、
曲線識別子がリリース間で安定し、実装間で決定論的にデコードできるよう
共有レジストリに依存します。

以下の表は、割り当て済み `curve_id` の規範的な参照です。機械可読な
コピーは [`address_curve_registry.json`](address_curve_registry.json) として
同梱されます。自動化ツールは JSON 版を消費し、フィクスチャ生成時に
`version` フィールドを固定（pin）するべきです。

## 登録済み曲線

| ID (`curve_id`) | アルゴリズム | フィーチャーゲート | 状態 | 公開鍵エンコード | 備考 |
|-----------------|--------------|---------------------|------|------------------|------|
| `0x01` (1) | `ed25519` | — | 本番 | 32 バイトの圧縮 Ed25519 公開鍵 | V1 のカノニカル曲線。全 SDK ビルドはこの識別子をサポートしなければなりません。 |
| `0x02` (2) | `ml-dsa` | — | 本番（設定ゲート） | Dilithium3 公開鍵（1952 バイト） | すべてのビルドで利用可能。コントローラペイロードを出力する前に `crypto.allowed_signing` と `crypto.curves.allowed_curve_ids` で有効化してください。 |
| `0x03` (3) | `bls_normal` | `bls` | 本番（フィーチャーゲート） | 48 バイトの圧縮 G1 公開鍵 | コンセンサスバリデータに必須。`allowed_signing`/`allowed_curve_ids` に無くても admission は BLS コントローラを許可します。 |
| `0x04` (4) | `secp256k1` | — | 本番 | 33 バイトの SEC1 圧縮鍵 | SHA-256 上の決定論的 ECDSA。署名は 64 バイトのカノニカル `r∥s` 形式を使用します。 |
| `0x05` (5) | `bls_small` | `bls` | 本番（フィーチャーゲート） | 96 バイトの圧縮 G2 公開鍵 | 署名は小さく鍵は大きい BLS のコンパクト署名プロファイル。 |
| `0x0A` (10) | `gost3410-2012-256-paramset-a` | `gost` | 予約 | 64 バイト little-endian の TC26 param set A 点 | `gost` フィーチャーで有効化（ガバナンス承認後）。 |
| `0x0B` (11) | `gost3410-2012-256-paramset-b` | `gost` | 予約 | 64 バイト little-endian の TC26 param set B 点 | TC26 B パラメータセットを反映。`gost` フィーチャーゲートでブロック。 |
| `0x0C` (12) | `gost3410-2012-256-paramset-c` | `gost` | 予約 | 64 バイト little-endian の TC26 param set C 点 | 将来のガバナンス承認待ち。 |
| `0x0D` (13) | `gost3410-2012-512-paramset-a` | `gost` | 予約 | 128 バイト little-endian の TC26 param set A 点 | 512 ビット GOST 曲線の需要が出るまで予約。 |
| `0x0E` (14) | `gost3410-2012-512-paramset-b` | `gost` | 予約 | 128 バイト little-endian の TC26 param set B 点 | 512 ビット GOST 曲線の需要が出るまで予約。 |
| `0x0F` (15) | `sm2` | `sm` | 予約 | DistID 長（u16 BE）+ DistID バイト列 + 65 バイトの SEC1 非圧縮 SM2 鍵 | `sm` フィーチャーが preview を脱したときに利用可能。 |

### 利用ガイドライン

- **Fail closed:** エンコーダは未対応アルゴリズムを `ERR_UNSUPPORTED_ALGORITHM`
  で拒否しなければなりません。デコーダは本レジストリに無い識別子に対し
  `ERR_UNKNOWN_CURVE` を返さなければなりません。
- **フィーチャーゲート:** BLS/GOST/SM2 は列挙したビルド時フィーチャーの
  背後にあります。これらの曲線でアドレスを発行する前に、
  `iroha_config.crypto.allowed_signing` の該当エントリとビルド機能を
  有効化してください。
- **admission 例外:** BLS コントローラは、`allowed_signing`/`allowed_curve_ids`
  に無くてもコンセンサスバリデータ向けに許可されます。
- **config + manifest の整合:** `iroha_config.crypto.allowed_curve_ids`
  （および `ManifestCrypto.allowed_curve_ids`）で、クラスターが受け入れる
  曲線識別子を公開してください。admission は `allowed_signing` と合わせて
  このリストを強制します。
- **決定論的エンコード:** 公開鍵は署名実装が返すバイト列をそのまま使用
  します（Ed25519 圧縮バイト、ML‑DSA 公開鍵、BLS 圧縮点など）。SDK は
  送信前に検証エラーを提示すべきです。
- **manifest の整合:** Genesis/コントローラの manifest は同一の識別子を
  使用し、admission がクラスター能力を超えるコントローラを拒否できる
  ようにします。

## 能力ビットマップの告知

`GET /v2/node/capabilities` は、`crypto.curves` 配下に `allowed_curve_ids`
とパックされた `allowed_curve_bitmap` 配列を公開します。bitmap は 64 ビット
lane の little-endian 配列（最大 4 値で `u8` の 0–255 をカバー）です。ビット
`i` が立っている場合、曲線識別子 `i` が admission 方針で許可されています。

- 例: `{ allowed_curve_ids: [1, 15] }` ⇒ `allowed_curve_bitmap: [32770]`
  （`(1 << 1) | (1 << 15) = 32770`）。
- `63` を超える曲線は後続 lane にビットを立てます。末尾のゼロ lane は省略
  されるため、`curve_id = 130` も有効にした場合は
  `allowed_curve_bitmap = [32768, 0, 4]`（ビット 15 と 130）になります。

ダッシュボードやヘルスチェックには bitmap を優先してください。単一ビットの
確認で能力を判定でき、配列全体の走査が不要です。順序付き識別子が必要な
ツールは `allowed_curve_ids` を引き続き利用できます。両方を公開することで、
運用者と SDK 向けの決定論的ビットマップ公開という **ADDR-3** 要件を満たします。

## バリデーションチェックリスト

コントローラを取り込む全コンポーネント（Torii、admission、SDK エンコーダ、
オフラインツール）は、受理前に同一の決定論的チェックを適用する必要があります。
以下の手順は必須の検証ロジックです。

1. **クラスター方針を解決:** アカウントペイロード先頭の `curve_id` を解析し、
   `iroha_config.crypto.allowed_curve_ids`（および対応する
   `ManifestCrypto.allowed_curve_ids`）に含まれない識別子を拒否します。BLS
   コントローラは例外で、ビルドに含まれていれば allowlist に無くても許可
   されます。これにより、未承認の preview 曲線が受理されるのを防ぎます。
2. **エンコード長の強制:** キーを展開/復元する前に、ペイロード長がアルゴリズムの
   正規サイズと一致するか確認し、失敗した値は拒否します。
3. **アルゴリズム固有のデコード:** `iroha_crypto` と同じデコーダ
   （`ed25519_dalek`, `pqcrypto_dilithium`, `w3f_bls`/`blstrs`, `sm2`, TC26
   ヘルパー等）を使い、全実装で同一のサブグループ/点検証を行います。
4. **署名サイズの検証:** admission と SDK は下表の署名長を必ず適用し、短すぎる
   あるいは長すぎる署名は検証前に拒否します。

| アルゴリズム | `curve_id` | 公開鍵バイト数 | 署名バイト数 | 重要チェック |
|--------------|------------|----------------|--------------|--------------|
| `ed25519` | `0x01` | 32 | 64 | 非カノニカル圧縮点を拒否、cofactor クリア（小位数点の排除）、署名検証時に `s < L` を保証。 |
| `ml-dsa` (Dilithium3) | `0x02` | 1952 | 3309 | 1952 バイト以外の payload をデコード前に拒否し、Dilithium3 公開鍵を解析して pqcrypto‑dilithium で署名検証。 |
| `bls_normal` | `0x03` | 48 | 96 | カノニカルな圧縮 G1 公開鍵と圧縮 G2 署名のみ受理。単位元や非カノニカル表現は拒否。 |
| `secp256k1` | `0x04` | 33 | 64 | SEC1 圧縮点のみ受理し、展開後に非カノニカル/無効点を拒否。署名は 64 バイト `r∥s` のカノニカル形式（low‑`s` 正規化は署名側）。 |
| `bls_small` | `0x05` | 96 | 48 | カノニカルな圧縮 G2 公開鍵と圧縮 G1 署名のみ受理。単位元や非カノニカル表現は拒否。 |
| `gost3410-2012-256-paramset-a` | `0x0A` | 64 | 64 | `(x||y)` little-endian 座標として解釈し、各座標 `< p` を確認、単位元を拒否、署名検証時に 32 バイト `r`/`s` のカノニカル limb を要求。 |
| `gost3410-2012-256-paramset-b` | `0x0B` | 64 | 64 | param set A と同等の検証（TC26 B のドメインパラメータ）。 |
| `gost3410-2012-256-paramset-c` | `0x0C` | 64 | 64 | param set A と同等の検証（TC26 C のドメインパラメータ）。 |
| `gost3410-2012-512-paramset-a` | `0x0D` | 128 | 128 | `(x||y)` を 64 バイト limb として解釈し `< p` を確認、単位元を拒否、署名に 64 バイト `r`/`s` limb を要求。 |
| `gost3410-2012-512-paramset-b` | `0x0E` | 128 | 128 | param set A と同等の検証（TC26 B の 512 ビットドメイン）。 |
| `sm2` | `0x0F` | 2 + distid + 65 | 64 | distid 長（u16 BE）をデコードし、DistID バイト列を検証、SEC1 非圧縮点を解析、GM/T 0003 のサブグループ規則を適用し、設定済み DistID を使って SM2 のカノニカル `(r, s)` を要求。 |

各行は [`address_curve_registry.json`](address_curve_registry.json) 内の
`validation` オブジェクトに対応します。JSON を利用するツールは
`public_key_bytes`、`signature_bytes`、`checks` を参照して同じ検証を自動化でき
ます。可変長エンコード（例: SM2）は `public_key_bytes` を null に設定し、
`checks` に長さルールを記述します。

## 新しい曲線識別子の申請

1. アルゴリズム仕様（エンコード、検証、エラー処理）を策定し、ガバナンス
   承認を取得します。
2. 本ドキュメントと `address_curve_registry.json` を更新する PR を提出します。
   新しい識別子は一意で、`0x01..=0xFE` の範囲に収める必要があります。
3. 本番展開前に SDK、Norito フィクスチャ、オペレーター向け文書を更新します。
4. セキュリティ/オブザーバビリティ担当と調整し、テレメトリ、runbook、
   admission ポリシーが新アルゴリズムを反映するようにします。
