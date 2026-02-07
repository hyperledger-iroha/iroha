---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_vectors.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd46471945188bcb95c8ee411c48acc8915a92b408df196caa65bf25f0596732
source_last_modified: "2026-01-05T18:22:23.402400+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ SM2/SM3/SM4 集成工作的參考測試向量。

# SM Vectors 舞台筆記

本文檔匯總了公開可用的已知答案測試，這些測試在自動導入腳本落地之前為 SM2/SM3/SM4 工具播種。機器可讀副本位於：

- `crates/iroha_crypto/tests/fixtures/sm_known_answers.toml`（附件向量，RFC 8998 案例，附件示例 1）。
- `fixtures/sm/sm2_fixture.json`（Rust/Python/JavaScript 測試使用的共享確定性 SDK 固定裝置）。
- `crates/iroha_crypto/tests/fixtures/wycheproof_sm2.json` — 在 Apache-2.0 下鏡像的精心策劃的 52 例語料庫（確定性固定裝置 + 合成位翻轉/消息/尾部截斷負數），同時上游 SM2 套件待定。 `crates/iroha_crypto/tests/sm2_wycheproof.rs` 在可能的情況下使用標準 SM2 驗證器驗證這些向量，並在需要時回退到附件域的純 BigInt 實現。

## 針對 OpenSSL / 通所 / GmSSL 的 SM2 簽名驗證

附件示例 1 (Fp-256) 使用身份 `ALICE123@YAHOO.COM` (ENTLA 0x0090)、消息 `"message digest"` 和公鑰，如下所示。複製粘貼 OpenSSL/Tongsuo 工作流程是：

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

* OpenSSL 3.x 記錄了 `distid:` / `hexdistid:` 選項。某些 OpenSSL 1.1.1 版本將旋鈕公開為 `sm2_id:` - 使用 `openssl pkeyutl -help` 中出現的任何一個。
* GmSSL 導出相同的 `pkeyutl` 表面；舊版本也接受 `-pkeyopt sm2_id:...`。
* LibreSSL（macOS/OpenBSD 上的默認設置）**不**實現 SM2/SM3，因此上面的命令會失敗。使用 OpenSSL ≥ 1.1.1、Tongsuo 或 GmSSL。

DER 幫助程序發出 `3044022040F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D102206FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7`，與附件簽名匹配。

附件還打印用戶信息哈希和生成的摘要：

```
ZA = F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A
e  = SM3(ZA || "message digest")
   = B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76
```

您可以通過 OpenSSL 確認：

```bash
echo -n 'F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A' \
  | xxd -r -p > za.bin
printf "message digest" >> za.bin
openssl dgst -sm3 -binary za.bin | xxd -p -c 256
# -> b524f552cd82b8b028476e005c377fb19a87e6fc682d48bb5d42e3d9b9effe76
```

對曲線方程進行 Python 健全性檢查：

```python
p = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFF", 16)
a = int("FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC", 16)
b = int("28E9FA9E9D9F5E344D5A9E4BCF6509A7F39789F515AB8F92DDBCBD414D940E93", 16)
x = int("0AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A", 16)
y = int("7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857", 16)
assert (pow(y, 2, p) - (pow(x, 3, p) + a * x + b)) % p == 0
```

## SM3 哈希向量

|輸入|十六進制編碼 |摘要（十六進制）|來源 |
|--------|--------------|--------------|--------|
| `""`（空字符串）| `""` | `1ab21d8355cfa17f8e61194831e81a8f22bec8c728fefb747ed035eb5082aa2b` | GM/T 0004-2012附件A.1|
| `"abc"` | `616263` | `66c7f0f462eeedd9d1f2d46bdc10e4e24167c4875cf2f7a2297da02b8f4ba8e0` | GM/T 0004-2012附件A.2|
| `"abcd"` 重複 16 次（64 字節）| ×16 | `61626364` `debe9ff92275b8a138604889c18e5a4d6fdb70e5387e5765293dcba39c0c5732` | GB/T 32905-2016附錄A |

## SM4 分組密碼 (ECB) 向量

|密鑰（十六進制）|明文（十六進制）|密文（十六進制）|來源 |
|------------|------------------|------------------|--------|
| `0123456789abcdeffedcba9876543210` | `0123456789abcdeffedcba9876543210` | `681edf34d206965e86b3e94f536e4246` | GM/T 0002-2012附件A.1|
| `0123456789abcdeffedcba9876543210` | `000102030405060708090a0b0c0d0e0f` | `59b50808d3dcf921fa30b5b3c1dddc19` | GM/T 0002-2012附件A.2|
| `0123456789abcdeffedcba9876543210` | `ffeeddccbbaa99887766554433221100` | `1c3b3f56186b70819d3f5aa11fe2c8b6` | GM/T 0002-2012附件A.3|

## SM4-GCM 認證加密

|關鍵|四 |亞德 |明文|密文|標籤 |來源 |
|-----|----|-----|------------|------------|-----|--------|
| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `d9313225f88406e5a55909c5aff5269a` | `42831ec2217774244b7221b784d0d49c` | `4d5c2af327cd64a62cf35abd2ba6fab4` | RFC 8998 附錄 A.2 |

## SM4-CCM 認證加密|關鍵|隨機數 |亞德 |明文|密文|標籤 |來源 |
|-----|--------|-----|------------|------------|-----|--------|
| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `202122232425262728292a2b2c2d2e2f` | `7162015b4dac2555` | `4d26de5a` | RFC 8998 附錄 A.3 |

### Wycheproof 負面案例 (SM4 GCM/CCM)

這些案例為 `crates/iroha_crypto/tests/sm3_sm4_vectors.rs` 中的回歸套件提供信息。每個案例都必須驗證失敗。

|模式| TC ID |描述 |關鍵|隨機數 |亞德 |密文|標籤 |筆記|
|------|--------|-------------|-----|--------|-----|------------|-----|--------|
|氣相色譜法 | 1 |標籤位翻轉| `0123456789abcdeffedcba9876543210` | `00001234567800000000abcd` | `feedfacedeadbeeffeedfacedeadbeefabaddad2` | `42831ec2217774244b7221b784d0d49c` | `5d5c2af327cd64a62cf35abd2ba6fab4` | Wycheproof 衍生的無效標籤 |
| CCM| 17 | 17標籤位翻轉| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de5a` | Wycheproof 衍生的無效標籤 |
| CCM| 18 |截斷的標籤（3 字節）| `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2555` | `5d26de` |確保短標籤無法通過身份驗證 |
| CCM| 19 | 19密文位翻轉 | `404142434445464748494a4b4c4d4e4f` | `10111213141516` | `000102030405060708090a0b0c0d0e0f` | `7162015b4dac2554` | `5d26de5a` |檢測被篡改的有效負載 |

## SM2 確定性簽名參考

|領域 |值（除非另有說明，否則為十六進制）|來源 |
|--------|--------------------------|--------|
|曲線參數| `sm2p256v1`（a = `FFFFFFFEFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF00000000FFFFFFFFFFFFFFFC` 等）| GM/T 0003.5-2012附錄A|
|用戶 ID (`distid`) | ASCII `"ALICE123@YAHOO.COM"` (ENTLA 0x0090) | GM/T 0003 附錄 D |
|公鑰| `040AE4C7798AA0F119471BEE11825BE46202BB79E2A5844495E97C04FF4DF2548A7C0240F88F1CD4E16352A73C17B7F16F07353E53A176D684A9FE0C6BB798E857` | GM/T 0003 附錄 D |
|留言 | `"message digest"`（十六進制 `6d65737361676520646967657374`）| GM/T 0003 附錄 D |
|扎 | `F4A38489E32B45B6F876E3AC2168CA392362DC8F23459C1D1146FC3DBFB7BC9A` | GM/T 0003 附錄 D |
| `e = SM3(ZA || M)` | `B524F552CD82B8B028476E005C377FB19A87E6FC682D48BB5D42E3D9B9EFFE76` | GM/T 0003 附錄 D |
|簽名 `(r,s)` | `40F1EC59F793D9F49E09DCEF49130D4194F79FB1EED2CAA55BACDB49C4E755D1`、`6FC6DAC32C5D5CF10C77DFB20F7C2EB667A457872FB09EC56327A67EC7DEEBE7` | GM/T 0003 附錄 D |
|多編解碼器（臨時）| `8626550012414C494345313233405941484F4F2E434F4D040AE4…`（`sm2-pub`，變體 `0x1306`）|源自附件示例 1 |
|前綴多重哈希 | `sm2:8626550012414C494345313233405941484F4F2E434F4D040AE4…` |派生（匹配 `sm_known_answers.toml`）|

SM2 多哈希有效負載編碼為 `distid_len (u16 BE) || distid bytes || SEC1 uncompressed key (65 bytes)`。

### Rust SDK 確定性簽名裝置 (SM-3c)

結構化向量數組包含 Rust/Python/JavaScript 奇偶校驗負載
因此每個客戶端都使用共享種子簽署相同的 SM2 消息，並且
區分標識符。|領域|值（除非另有說明，否則為十六進制）|筆記|
|--------|--------------------------|--------|
|區分ID | `"iroha-sdk-sm2-fixture"` |跨 Rust/Python/JS SDK 共享 |
|種子| `"iroha-rust-sdk-sm2-deterministic-fixture"`（十六進制 `69726F68612D727573742D73646B2D736D322D64657465726D696E69737469632D66697874757265`）|輸入到 `Sm2PrivateKey::from_seed` |
|私鑰 | `E64AE95930A2324DCF11BB1657FBBC2CD2BEF7515664BE7A15CA6AE5BCE0B7CA` |確定性地源自種子 |
|公鑰（SEC1 未壓縮）| `0494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` |匹配確定性推導 |
|公鑰多重哈希 | `862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_string()` 的輸出 |
|前綴多重哈希 | `sm2:862658001569726F68612D73646B2D736D322D666978747572650494642A475196A8D8FED24B92D68E447325DD6C73808D88C037AA3CDDE4D40D573AD27FF3A5D08794355E935A4F7E68859B3F0706B6A8182762D423E33A6F9B61` | `PublicKey::to_prefixed_string()` 的輸出 |
|扎 | `6B0229C3FBFE7A5DC6EE27228E18E512752DCCE9191EB381242A7342D568EDDE` |通過 `Sm2PublicKey::compute_z` 計算 |
|留言 | `"Rust SDK SM2 signing fixture v1"`（十六進制 `527573742053444B20534D32207369676E696E672066697874757265207631`）| SDK 奇偶校驗測試的規範有效負載 |
|簽名 `(r, s)` | `4E026F2E0FB75A6418C95C046D70DB7B95558BE30FB2001906EBE2091FA1AF76`、`299CFF374026D9E0C49C6BD4A99A1473BA17EFF429B3FAD083B1577F7559BDF5` |通過確定性簽名產生 |

- 跨SDK消耗：
  - `fixtures/sm/sm2_fixture.json` 現在公開 `vectors` 數組。 Rust 加密回歸套件 (`crates/iroha_crypto/tests/sm2_fixture_vectors.rs`)、Rust 客戶端幫助程序 (`crates/iroha/src/sm.rs`)、Python 綁定 (`python/iroha_python/tests/test_crypto.py`) 和 JavaScript SDK (`javascript/iroha_js/test/crypto.sm2.fixture.test.js`) 都會解析這些裝置。
  - `crates/iroha/tests/sm_signing.rs` 執行確定性簽名並驗證鏈上多重哈希/多重編解碼器輸出是否與裝置匹配。
  - 准入時間回歸套件 (`crates/iroha_core/tests/admission_batching.rs`) 斷言 SM2 有效負載被拒絕，除非 `allowed_signing` 包括 `sm2` *並且* `default_hash` 是 `sm3-256`，涵蓋端到端的配置約束。
- 追加カバreジ:異常系（無效な曲線、異常な `r/s`、`distid` 改ざん）は `crates/iroha_crypto/tests/sm2_fuzz.rs` no property テsutoで網羅済みです。 附件示例 1の正規ベクトルは `sm_known_answers.toml` に多言語対応の多編解碼器形式で引き続き提供しています。
- Rust 代碼現在公開 `Sm2PublicKey::compute_z`，因此可以通過編程方式生成 ZA 裝置；有關附錄 D 回歸，請參閱 `sm2_compute_z_matches_annex_example`。

## 下一步行動
- 監控准入時間回歸 (`admission_batching.rs`)，以確保配置門控繼續強制實施 SM2 啟用邊界。
- 擴大 Wycheproof SM4 GCM/CCM 案例的覆蓋範圍，並為 SM2 簽名驗證導出基於屬性的模糊目標。 ✅（在 `sm3_sm4_vectors.rs` 中捕獲的無效案例子集）。
- LLM 提示輸入替代 ID：*“當區分 ID 設置為 1234567812345678 而不是 ALICE123@YAHOO.COM 時，為附件示例 1 提供 SM2 簽名，並概述新的 ZA/e 值。”*