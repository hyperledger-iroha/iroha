---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_lock_refresh_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3065571b34a226a5871c4fb68063f9419e48074b20096de215f440bdf54a4e59
source_last_modified: "2025-12-29T18:16:35.943236+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！安排 SM 尖峰所需的 Cargo.lock 刷新的過程。

# SM 功能 `Cargo.lock` 刷新計劃

`iroha_crypto` 的 `sm` 功能尖峰最初無法完成 `cargo check`，而強制執行 `--locked`。本說明記錄了經批准的 `Cargo.lock` 更新的協調步驟，並跟踪該需求的當前狀態。

> **2026-02-12 更新：** 最近的驗證顯示可選的 `sm` 功能現在使用現有鎖定文件構建（`cargo check -p iroha_crypto --features sm --locked` 在 7.9 秒冷/0.23 秒熱中成功）。依賴項集已包含 `base64ct`、`ghash`、`opaque-debug`、`pem-rfc7468`、`pkcs8`、`polyval`、`primeorder`、`sm2`、 `sm3`、`sm4` 和 `sm4-gcm`，因此不需要立即鎖定刷新。請保留以下過程，以備將來出現依賴項或新的可選包時使用。

## 為什麼需要刷新
- 尖峰的早期迭代需要添加鎖定文件中缺少的可選包。當前的鎖定快照已包含 RustCrypto 堆棧（`sm2`、`sm3`、`sm4`、支持編解碼器和 AES 幫助程序）。
- 存儲庫策略仍然阻止機會主義的鎖定文件編輯；如果將來需要升級依賴項，則以下過程仍然適用。
- 保留此計劃，以便團隊可以在引入新的 SM 相關依賴項或現有依賴項需要版本更新時執行受控刷新。

## 建議的協調步驟
1. **在 Crypto WG + Release Eng 同步中提出請求（所有者：@crypto-wg Lead）。 **
   - 參考 `docs/source/crypto/sm_program.md` 並註意該功能的可選性質。
   - 確認沒有並發的鎖文件更改窗口（例如，依賴性凍結）。
2. **準備帶鎖差異的補丁（所有者：@release-eng）。 **
   - 執行 `scripts/sm_lock_refresh.sh`（批准後）以僅更新所需的 crate。
   - 捕獲 `cargo tree -p iroha_crypto --features sm` 輸出（腳本發出 `target/sm_dep_tree.txt`）。
3. **安全審查（所有者：@security-reviews）。 **
   - 驗證新的包/版本是否符合審核註冊和許可期望。
   - 在供應鏈跟踪器中記錄哈希值。
4. **合併窗口執行。 **
   - 提交僅包含鎖定文件增量、依賴關係樹快照（作為工件附加）和更新的審核註釋的 PR。
   - 在合併之前確保 CI 使用 `cargo check -p iroha_crypto --features sm` 運行。
5. **後續任務。 **
   - 更新 `docs/source/crypto/sm_program.md` 行動項目清單。
   - 通知 SDK 團隊該功能可以使用 `--features sm` 進行本地編譯。## 時間表和所有者
|步驟|目標|業主|狀態 |
|------|--------|--------|--------|
|請求下次加密貨幣工作組電話會議的議程位置 | 2025-01-22 |加密貨幣工作組負責人 | ✅ 已完成（審查結論秒殺可以繼續進行，無需刷新）|
|草稿選擇性 `cargo update` 命令 + 健全性差異 | 2025-01-24 |發布工程| ⚪ 待機（如果出現新箱子則重新激活）|
|新貨箱安全審查| 2025-01-27 |安全評論 | ⚪ 待機（刷新恢復時重新使用審核清單）|
|合併鎖定文件更新 PR | 2025-01-29 |發布工程| ⚪ 待機 |
|更新 SM 程序文檔清單 |合併後 |加密貨幣工作組負責人 | ✅ 通過 `docs/source/crypto/sm_program.md` 條目解決 (2026-02-12) |

## 註釋
- 將任何未來的刷新限制在上面列出的 SM 相關 crate（以及支持 `rfc6979` 等幫助程序），避免工作區範圍內的 `cargo update`。
- 如果任何傳遞依賴項引入 MSRV 漂移，請在合併之前將其顯示出來。
- 合併後，啟用臨時 CI 作業來監控 `sm` 功能的構建時間。