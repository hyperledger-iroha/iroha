---
lang: ja
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T10:35:36.441790+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! SM2/SM3/SM4 統合作業用の参照テスト ベクトル。

# SM ベクターのステージングノート

このドキュメントは、自動インポート スクリプトが到着する前に SM2/SM3/SM4 ハーネスをシードする、公開されている既知の回答テストをまとめたものです。機械可読コピーは次の場所に存在します。

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml` (付録ベクトル、RFC 8998 ケース、付録例 1)。
- `fixtures/sm/sm2_fixture.json` (Rust/Python/JavaScript テストによって消費される共有決定論的 SDK フィクスチャ)。
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — 上流の SM2 スイートが保留中である間に、Apache-2.0 でミラーリングされた厳選された 52 ケースのコーパス (決定論的フィクスチャ + 合成されたビットフリップ/メッセージ/テールトランケーションネガ)。 `crates/iroha_crypto/tests/sm2_wycheproof.rs` は、可能な場合には標準の SM2 検証機能を使用してこれらのベクトルを検証し、必要に応じて Annex ドメインの純粋な BigInt 実装にフォールバックします。

## OpenSSL / Tongsuo / GmSSL に対する SM2 署名検証

付録例 1 (Fp-256) では、アイデンティティ `ALICE123@YAHOO.COM` (ENTLA 0x0090)、メッセージ `"message digest"`、および以下に示す公開鍵を使用します。コピーアンドペーストの OpenSSL/Tongsuo ワークフローは次のとおりです。

```bash
# 1. Public key (SubjectPublicKeyInfo)
cat > pubkey.pem <<'PEM'
-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoEcz1UBgi0DQgAECuTHeYqg8RlHG+4RglvkYgK7eeKlhESV6XwE/03y
VIp8AkD4jxzU4WNSpzwXt/FvBzU+U6F21oSp/gxrt5joVw==
-----END PUBLIC KEY-----
PEM

# 2. Message (no trailing newline)
printf "message digest" > msg.bin

# 3. Signature (DER form)
cat > sig.b64 <<'EOF'
MEQCIEDx7Fn3k9n0ngnc70kTDUGU95+x7tLKpVus20nE51XRAiBvxtrDLF1c8Qx337IPfC62Z6RX
hy+wnsVjJ6Z+x97r5w==
EOF
base64 -d sig.b64 > sig.der

# 4. Verify (expects "Signature Verified Successfully")
openssl pkeyutl -verify -pubin -inkey pubkey.pem \
  -in msg.bin -sigfile sig.der \
  -rawin -digest sm3 \
  -pkeyopt distid:ALICE123@YAHOO.COM
```

* OpenSSL 3.x には、`distid:` / `hexdistid:` オプションが記載されています。一部の OpenSSL 1.1.1 ビルドでは、ノブが `sm2_id:` として公開されます。`openssl pkeyutl -help` に表示されるものを使用します。
* GmSSL は同じ `pkeyutl` サーフェスをエクスポートします。古いビルドは `-pkeyopt sm2_id:...` も受け入れました。
* LibreSSL (macOS/OpenBSD のデフォルト) は SM2/SM3 を実装していない**ため、上記のコマンドは失敗します。 OpenSSL ≥ 1.1.1、Tongsuo、または GmSSL を使用します。

DER ヘルパーは、付録の署名と一致する `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` を発行します。

この付録では、ユーザー情報のハッシュとその結果のダイジェストも出力します。

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

OpenSSL で確認できます。

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

Python の曲線方程式の健全性チェック:

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 ハッシュ ベクトル

|入力 | 16 進エンコーディング |ダイジェスト (16 進数) |出典 |
|------|--------------|--------------|----------|
| `""` (空の文字列) | `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012 付属書 A.1 |
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012 付属書 A.2 |
| `"abcd"` 16 回繰り返されました (64 バイト) | `61626364` ×16 | `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016 付録 A |

## SM4 ブロック暗号 (ECB) ベクトル

|キー (16 進数) |平文 (16 進数) |暗号文 (16 進数) |出典 |
|----------|------|----------|----------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012 付属書 A.1 |
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012 付属書 A.2 |
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012 付属書 A.3 |

## SM4-GCM 認証暗号化

|キー | Ⅳ | AAD |平文 |暗号文 |タグ |出典 |
|-----|----|-----|----------|---------------|-----|----------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 付録 A.2 |

## SM4-CCM 認証暗号化|キー |ノンス | AAD |平文 |暗号文 |タグ |出典 |
|-----|----------|-----|----------|-----------|-----|----------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 付録 A.3 |

### ウィッチプルーフ ネガティブ ケース (SM4 GCM/CCM)

これらのケースは、`crates/iroha_crypto/tests/sm3_sm4_vectors.rs` の回帰スイートに通知されます。各ケースは検証に失敗する必要があります。

|モード | TC ID |説明 |キー |ノンス | AAD |暗号文 |タグ |メモ |
|------|-------|---------------|-----|-----------|-----|------------|-----|------|
| GCM | 1 |タグビット反転 | `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof 由来の無効なタグ |
| CCM | 17 |タグビット反転 | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof 由来の無効なタグ |
| CCM | 18 |切り詰められたタグ (3 バイト) | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` |短いタグが認証に失敗することを保証します。
| CCM | 19 |暗号文ビット反転 | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` |改ざんされたペイロードを検出 |

## SM2 決定論的署名リファレンス

|フィールド |値 (注記がない限り 16 進数) |出典 |
|------|--------------------------|----------|
|曲線パラメータ | `sm2p256v1` (a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` など) | GM/T 0003.5-2012 付録 A |
|ユーザーID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 付録 D |
|公開鍵 | `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 付録 D |
|メッセージ | `"message digest"` (16 進数 `6d65737361676520646967657374`) | GM/T 0003 付録 D |
| ZA | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 付録 D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 付録 D |
|署名 `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`、`6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 付録 D |
|マルチコーデック（暫定） | `8626550012414C494345313233405941484F4F2E434F4D040AE4…` (`sm2-pub`、バリアント `0x1306`) |付録例 1 から派生 |
|プレフィックス付きマルチハッシュ | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` |派生 (`sm_known_answers.toml` と一致) |

SM2 マルチハッシュ ペイロードは `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)` としてエンコードされます。

### Rust SDK 決定論的署名フィクスチャ (SM-3c)

構造化ベクトル配列には、Rust/Python/JavaScript パリティ ペイロードが含まれています
したがって、すべてのクライアントは共有シードを使用して同じ SM2 メッセージに署名します。
区別する識別子。|フィールド |値 (注記がない限り 16 進数) |メモ |
|------|--------------------------|------|
|識別ID | `"iroha-sdk-sm2-fixture"` | Rust/Python/JS SDK 間で共有 |
|シード | `"iroha-rust-sdk-sm2-deterministic-fixture"` (16 進数 `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`) | `Sm2PrivateKey::from_seed` への入力 |
|秘密鍵 | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` |シードから決定論的に派生 |
|公開鍵 (SEC1 非圧縮) | `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` |決定論的な導出と一致します。
|公開鍵マルチハッシュ | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` の出力 |
|プレフィックス付きマルチハッシュ | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` の出力 |
| ZA | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` | `Sm2PublicKey::compute_z` | 経由で計算
|メッセージ | `"Rust SDK SM2 signing fixture v1"` (16 進数 `527573742053444B20534D32207369676E696E672066697874757265207631`) | SDK パリティ テストの正規ペイロード |
|署名 `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`、`299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` |決定論的署名によって生成される |

- SDK 間の消費:
  - `fixtures/sm/sm2_fixture.json` は `vectors` 配列を公開するようになりました。 Rust 暗号回帰スイート (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`)、Rust クライアント ヘルパー (`crates/iroha/src/sm.rs`)、Python バインディング (`python/iroha_python/tests/test_crypto.py`)、および JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) はすべて、これらのフィクスチャを解析します。
  - `crates/iroha/tests/sm_signing.rs` は決定論的署名を実行し、オンチェーンのマルチハッシュ/マルチコーデック出力がフィクスチャと一致することを検証します。
  - アドミッション時間回帰スイート (`crates/iroha_core/tests/admission_batching.rs`) は、`allowed_signing` に `sm2` が含まれ、*かつ* `default_hash` が `sm3-256` でない限り、SM2 ペイロードが拒否されることをアサートし、構成制約をエンドツーエンドでカバーします。
- 追加カバレッジ: 異常系（有効なピーク、異常な `r/s`、`distid` 改ざん）は `crates/iroha_crypto/tests/sm2_fuzz.rs` のプロパティ テストで完了です。付録例 1 の正規速度は `sm_known_answers.toml` に多言語対応のマルチコーデック形式で引き続き提供しています。
- Rust コードは `Sm2PublicKey::compute_z` を公開するようになり、ZA フィクスチャをプログラムで生成できるようになりました。付属書 D の回帰については、`sm2_compute_z_matches_annex_example` を参照してください。

## 次のアクション
- アドミッション時間の回帰 (`admission_batching.rs`) を監視して、構成ゲーティングが SM2 有効化境界を強制し続けることを確認します。
- Wycheproof SM4 GCM/CCM ケースの適用範囲を拡張し、SM2 署名検証用のプロパティベースのファズ ターゲットを導き出します。 ✅ (`sm3_sm4_vectors.rs` でキャプチャされた無効なケースのサブセット)。
- LLM による代替 ID のプロンプト: *「識別 ID が ALICE123@YAHOO.COM ではなく 1234567812345678 に設定されている場合は、付録例 1 の SM2 署名を提供し、新しい ZA/e 値の概要を説明します。」*