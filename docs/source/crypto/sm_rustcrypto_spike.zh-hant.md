---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ RustCrypto SM 集成尖峰的註釋。

# RustCrypto SM 尖峰筆記

## 目標
驗證引入 RustCrypto 的 `sm2`、`sm3` 和 `sm4` 包（加上 `rfc6979`、`ccm`、`gcm`）作為可選依賴項可以在`iroha_crypto` crate 並在將功能標誌連接到更廣泛的工作空間之前產生可接受的構建時間。

## 建議的依賴關係圖

|板條箱 |建議版本 |特點|筆記|
|--------|--------------------|----------|--------|
| `sm2` | `0.13`（RustCrypto/簽名）| `std` |取決於 `elliptic-curve`；驗證 MSRV 與工作區匹配。 |
| `sm3` | `0.5.0-rc.1`（RustCrypto/哈希）|默認 | API 與 `sha2` 並行，與現有的 `digest` 特徵集成。 |
| `sm4` | `0.5.1`（RustCrypto/塊密碼）|默認 |適用於密碼特徵； AEAD 包裝推遲到稍後的峰值。 |
| `rfc6979` | `0.4` |默認 |重用於確定性隨機數推導。 |

*版本反映截至 2024 年 12 月的當前版本；著陸前與 `cargo search` 確認。 *

## 明顯變更（草案）

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

後續：引腳 `elliptic-curve` 以匹配 `iroha_crypto` 中已有的版本（當前為 `0.13.8`）。

## 尖峰清單
- [x] 向 `crates/iroha_crypto/Cargo.toml` 添加可選依賴項和功能。
- [x] 在 `cfg(feature = "sm")` 後面創建 `signature::sm` 模塊，並使用佔位符結構來確認接線。
- [x] 運行 `cargo check -p iroha_crypto --features sm` 確認編譯；記錄構建時間和新的依賴項計數 (`cargo tree --features sm`)。
- [x] 使用 `cargo check -p iroha_crypto --features sm --locked` 確認僅標準姿勢；不再支持 `no_std` 版本。
- [x] `docs/source/crypto/sm_program.md` 中的文件結果（計時、依賴樹增量）。

## 捕捉觀察結果
- 與基線相比的額外編譯時間。
- `cargo builtinsize` 的二進制大小影響（如果可測量）。
- 任何 MSRV 或功能衝突（例如，與 `elliptic-curve` 次要版本）。
- 發出的警告（不安全代碼、const-fn 門控）可能需要上游補丁。

## 待處理項目
- 在膨脹工作區依賴關係圖之前等待加密工作組批准。
- 確認是否向供應商 crate 進行審查或依賴 crates.io（可能需要鏡像）。
- 在標記清單完成之前，根據 `sm_lock_refresh_plan.md` 協調 `Cargo.lock` 刷新。
- 一旦獲得批准即可使用 `scripts/sm_lock_refresh.sh` 來重新生成鎖定文件和依賴關係樹。

## 2025-01-19 尖峰日誌
- 在 `iroha_crypto` 中添加了可選依賴項（`sm2 0.13`、`sm3 0.5.0-rc.1`、`sm4 0.5.1`、`rfc6979 0.4`）和 `sm` 功能標誌。
- 存根 `signature::sm` 模塊以在編譯期間執行哈希/分組密碼 API。
- `cargo check -p iroha_crypto --features sm --locked` 現在可以解析依賴關係圖，但會因 `Cargo.lock` 更新要求而中止；存儲庫策略禁止鎖定文件編輯，因此編譯運行將保持掛起狀態，直到我們協調允許的鎖定刷新。## 2026-02-12 尖峰日誌
- 解決了之前的鎖文件阻止程序 - 依賴關係已被捕獲 - 因此 `cargo check -p iroha_crypto --features sm --locked` 成功（在開發 Mac 上冷構建 7.9 秒；增量重新運行 0.23 秒）。
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` 在 1.0 秒內通過，確認可選功能在僅 `std` 配置中編譯（不保留 `no_std` 路徑）。
- 啟用 `sm` 功能的依賴項增量引入了 11 個 crate：`base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、 `primeorder`、`sm2`、`sm3`、`sm4` 和 `sm4-gcm`。 （`rfc6979` 已經是基線圖的一部分。）
- 對於未使用的 NEON 策略助手，構建警告仍然存在；保持原樣，直到計量平滑運行時重新啟用這些代碼路徑。