---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_operator_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dffc2cf6c6e59f54d1fc22136ba93f75466509c699a4361a381bf7e0ce0d1dda
source_last_modified: "2025-12-29T18:16:35.943754+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SM 功能推出和遙測清單

此清單可幫助 SRE 和運營商團隊啟用 SM (SM2/SM3/SM4) 功能
一旦審核和合規門被清除，就可以安全設置。遵循本文檔
以及 `docs/source/crypto/sm_program.md` 中的配置簡介和
`docs/source/crypto/sm_compliance_brief.md` 中的法律/出口指南。

## 1. 飛行前準備
- [ ] 確認工作區發行說明將 `sm` 顯示為僅驗證或簽名，
      取決於推出階段。
- [ ] 驗證隊列正在運行從包含以下內容的提交構建的二進製文件
      SM 遙測計數器和配置旋鈕。 （目標發布待定；跟踪
      在推廣票中。）
- [ ] 在暫存節點上運行 `scripts/sm_perf.sh --tolerance 0.25`（每個目標
      架構）並存檔摘要輸出。該腳本現在自動選擇
      標量基線作為加速模式的比較目標
      （SM3 NEON工作落地時，`--compare-tolerance`默認為5.25）；
      如果是主要的或比較的，則調查或阻止推出
      守衛失敗。在 Linux/aarch64 Neoverse 硬件上捕獲時，通過
      `--baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_<mode>.json --write-baseline`
      用主機的捕獲覆蓋導出的 `m3-pro-native` 中位數
      發貨前。
- [ ] 確保 `status.md` 和部署票記錄了合規性備案
      在需要它們的司法管轄區運行的任何節點（請參閱合規簡介）。
- [ ] 如果驗證器將 SM 簽名密鑰存儲在中，則準備 KMS/HSM 更新
      硬件模塊。

## 2. 配置更改
1. 運行 xtask 幫助程序以生成 SM2 密鑰清單和準備粘貼的代碼片段：
   ```bash
   cargo xtask sm-operator-snippet \
     --distid CN12345678901234 \
     --json-out sm2-key.json \
     --snippet-out client-sm2.toml
   ```
   當您只需要檢查輸出時，使用 `--snippet-out -`（以及可選的 `--json-out -`）將輸出流式傳輸到標準輸出。
   如果您更喜歡手動執行較低級別的 CLI 命令，則等效流程為：
   ```bash
   cargo run -p iroha_cli --features sm -- \
     crypto sm2 keygen \
     --distid CN12345678901234 \
     --output sm2-key.json

   cargo run -p iroha_cli --features sm -- \
     crypto sm2 export \
     --private-key-hex "$(jq -r .private_key_hex sm2-key.json)" \
     --distid CN12345678901234 \
     --snippet-output client-sm2.toml \
     --emit-json --quiet
   ```
   如果 `jq` 不可用，請打開 `sm2-key.json`，複製 `private_key_hex` 值，並將其直接傳遞給導出命令。
2. 將生成的代碼片段添加到每個節點的配置中（顯示的值
   僅驗證階段；根據環境進行調整併保持鍵排序，如圖所示）：
```toml
[crypto]
default_hash = "sm3-256"
allowed_signing = ["ed25519", "sm2"]   # remove "sm2" to stay in verify-only mode
sm2_distid_default = "1234567812345678"
# enable_sm_openssl_preview = true  # optional: only when deploying the OpenSSL/Tongsuo path
```
3. 重新啟動節點並按預期確認 `crypto.sm_helpers_available` 和（如果啟用了預覽後端）`crypto.sm_openssl_preview_enabled` 表面：
   - `/status` JSON (`"crypto":{"sm_helpers_available":true,"sm_openssl_preview_enabled":true,...}`)。
   - 每個節點的渲染 `config.toml`。
4. 更新清單/創世條目以將 SM 算法添加到允許列表，如果
   簽名將在稍後推出。使用 `--genesis-manifest-json` 時
   沒有預先簽名的創世塊，`irohad` 現在播種運行時加密
   直接從清單的 `crypto` 塊獲取快照 - 確保清單是
   在向前推進之前檢查您的變革計劃。## 3. 遙測與監控
- 抓取 Prometheus 端點並確保出現以下計數器/儀表：
  - `iroha_sm_syscall_total{kind="verify"}`
  - `iroha_sm_syscall_total{kind="hash"}`
  - `iroha_sm_syscall_total{kind="seal|open",mode="gcm|ccm"}`
  - `iroha_sm_openssl_preview`（0/1 儀表報告預覽切換狀態）
  - `iroha_sm_syscall_failures_total{kind="verify|hash|seal|open",reason="..."}`
- 啟用 SM2 簽名後掛鉤簽名路徑；添加計數器
  `iroha_sm_sign_total` 和 `iroha_sm_sign_failures_total`。
- 創建 Grafana 儀表板/警報：
  - 故障計數器中的尖峰（窗口 5m）。
  - SM 系統調用吞吐量突然下降。
  - 節點之間的差異（例如，啟用不匹配）。

## 4. 推出步驟
|相|行動|筆記|
|--------|---------|--------|
|僅驗證 |將 `crypto.default_hash` 更新為 `sm3-256`，保留 `allowed_signing` 不帶 `sm2`，監視驗證計數器。 |目標：在不冒共識分歧風險的情況下執行 SM 驗證路徑。 |
|混合簽名試點|允許有限的 SM 簽名（驗證器的子集）；監控簽名計數器和延遲。 |確保回退到 Ed25519 仍然可用；如果遙測顯示不匹配，則停止。 |
| GA 簽名 |擴展 `allowed_signing` 以包含 `sm2`、更新清單/SDK 並發布最終 Runbook。 |需要封閉的審計結果、更新的合規文件和穩定的遙測。 |

### 準備情況審核
- **僅驗證準備情況 (SM-RR1)。 ** 召集發布工程師、加密工作組、運營和法律。要求：
  - `status.md` 註明合規性備案狀態 + OpenSSL 出處。
  - `docs/source/crypto/sm_program.md` / `sm_compliance_brief.md` / 此清單在上一個版本窗口內更新。
  - `defaults/genesis` 或特定於環境的清單顯示 `crypto.allowed_signing = ["ed25519","sm2"]` 和 `crypto.default_hash = "sm3-256"`（如果仍處於第一階段，則為不帶 `sm2` 的僅驗證變體）。
  - `scripts/sm_openssl_smoke.sh` + `scripts/sm_interop_matrix.sh` 日誌附加到部署票證上。
  - 遙測儀表板 (`iroha_sm_*`) 已審核穩態行為。
- **簽署飛行員準備就緒 (SM-RR2)。 ** 附加登機口：
  - RustCrypto SM 堆棧關閉的審計報告或由安全部門簽署的用於補償控制的 RFC。
  - 操作員運行手冊（特定於設施）更新了簽名回退/回滾步驟。
  - 試點隊列的 Genesis 清單包括 `allowed_signing = ["ed25519","sm2"]`，並且允許列錶鏡像在每個節點配置中。
  - 記錄退出/回滾計劃（將 `allowed_signing` 切換回 Ed25519、恢復清單、重置儀表板）。
- **GA 準備就緒 (SM-RR3)。 ** 需要積極的試點報告、所有驗證者管轄區的更新合規性文件、簽署的遙測基線以及來自 Release Eng + Crypto WG + Ops/Legal triad 的發布票證批准。## 5. 包裝和合規檢查表
- **捆綁 OpenSSL/Tongsuo 工件。 ** 將 OpenSSL/Tongsuo 3.0+ 共享庫 (`libcrypto`/`libssl`) 與每個驗證程序包一起發送，或記錄確切的系統依賴項。在發布清單中記錄版本、構建標誌和 SHA256 校驗和，以便審核員可以跟踪供應商構建。
- **在 CI 期間進行驗證。 ** 添加一個 CI 步驟，針對每個目標平台上的打包工件執行 `scripts/sm_openssl_smoke.sh`。如果啟用了預覽標誌但提供程序無法初始化（缺少標頭、不支持的算法等），則作業必定會失敗。
- **發布合規性說明。 ** 使用捆綁的提供商版本、出口管制參考（GM/T、GB/T）以及 SM 算法所需的任何司法管轄區特定文件更新發行說明/`status.md`。
- **操作員運行手冊更新。 ** 記錄升級流程：暫存新的共享對象，使用 `crypto.enable_sm_openssl_preview = true` 重新啟動對等點，確認 `/status` 字段和 `iroha_sm_openssl_preview` 儀表翻轉到 `true`，並在預覽遙測數據在機群中出現偏差時保留回滾計劃（翻轉配置標誌或恢復包）。
- **證據保留。 ** 將 OpenSSL/Tongsuo 包的構建日誌和簽名證明與驗證器發布工件一起存檔，以便將來的審計可以重現來源鏈。

## 6. 事件響應
- **驗證失敗峰值：** 回滾到沒有 SM 支持的版本或刪除 `sm2`
  從 `allowed_signing`（根據需要恢復 `default_hash`）並故障轉移到上一個
  調查期間釋放。捕獲失敗的有效負載、比較哈希值和節點日誌。
- **性能回歸：** 將 SM 指標與 Ed25519/SHA2 基線進行比較。
  如果ARM內部路徑導致發散，則設置`crypto.sm_intrinsics = "force-disable"`
  （功能切換待實施）並報告調查結果。
- **遙測差距：** 如果計數器丟失或未更新，請提出問題
  反對發布工程；在出現間隙之前不要繼續進行更廣泛的推廣
  已解決。

## 7.清單模板
- [ ] 配置已暫存且對等點已重新啟動。
- [ ] 遙測計數器可見並已配置儀表板。
- [ ] 記錄合規/法律步驟。
- [ ] 推出階段由 Crypto WG/Release TL 批准。
- [ ] 已完成推出後審查並記錄結果。

在部署票證中維護此清單，並在以下情況下更新 `status.md`：
機隊在階段之間進行轉換。