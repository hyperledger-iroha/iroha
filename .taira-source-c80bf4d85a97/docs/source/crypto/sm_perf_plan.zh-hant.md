---
lang: zh-hant
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2025-12-29T18:16:35.944695+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## SM 績效捕獲和基線計劃

狀態：已起草 — 2025-05-18  
所有者：Performance WG（領導）、Infra Ops（實驗室調度）、QA Guild（CI 門控）  
相關路線圖任務：SM-4c.1a/b、SM-5a.3b、FASTPQ Stage 7 跨設備捕獲

### 1. 目標
1. 在 `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json` 中記錄 Neoverse 中位數。當前基線是從 `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/`（CPU 標籤 `neoverse-proxy-macos`）下的 `neoverse-proxy-macos` 捕獲導出的，對於 aarch64 macOS/Linux，SM3 比較容差擴大到 0.70。當裸機時間打開時，在 Neoverse 主機上重新運行 `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` 並將聚合中位數提升到基線。  
2. 收集匹配的 x86_64 中位數，以便 `ci/check_sm_perf.sh` 可以保護兩個主機類。  
3. 發布可重複的捕獲程序（命令、工件佈局、審閱者），以便未來的性能門不依賴於部落知識。

### 2. 硬件可用性
當前工作區中只能訪問 Apple Silicon (macOS arm64) 主機。 `neoverse-proxy-macos` 捕獲將導出為臨時 Linux 基線，但捕獲裸機 Neoverse 或 x86_64 中位數仍需要在 `INFRA-2751` 下跟踪的共享實驗室硬件，以便在實驗室窗口打開後由性能工作組運行。剩餘的捕獲窗口現在已在工件樹中預訂和跟踪：

- Neoverse N2 裸機（東京機架 B）於 2026 年 3 月 12 日預訂。操作員將重用第 3 部分中的命令並將工件存儲在 `artifacts/sm_perf/2026-03-lab/neoverse-b01/` 下。
- x86_64 Xeon（蘇黎世機架 D）於 2026 年 3 月 19 日預訂，禁用 SMT 以減少噪音；文物將以 `artifacts/sm_perf/2026-03-lab/xeon-d01/` 的身份登陸。
- 兩次運行落地後，將中位數提升到基線 JSON 中，並在 `ci/check_sm_perf.sh` 中啟用 CI 門（目標切換日期：2026-03-25）。

在此日期之前，只有 macOS arm64 基線可以在本地刷新。### 3. 捕獲程序
1. **同步工具鏈**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **生成捕獲矩陣**（每個主機）  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   幫助程序現在在目標目錄下寫入 `capture_commands.sh` 和 `capture_plan.json`。該腳本為每種模式設置 `raw/*.json` 捕獲路徑，以便實驗室技術人員可以確定性地對運行進行批處理。
3. **運行捕獲**  
   從 `capture_commands.sh` 執行每個命令（或手動運行等效命令），確保每個模式都通過 `--capture-json` 發出結構化 JSON blob。始終通過 `--cpu-label "<model/bin>"`（或 `SM_PERF_CPU_LABEL=<label>`）提供主機標籤，以便捕獲元數據和後續基線記錄生成中位數的確切硬件。助手已經提供了適當的路徑；對於手動運行，模式是：
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **驗證結果**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   確保運行之間的方差保持在 ±3% 以內。如果沒有，請重新運行受影響的模式並在日誌中記錄重試情況。
5. **提升中位數**  
   使用 `scripts/sm_perf_aggregate.py` 計算中位數並將其複製到基線 JSON 文件中：
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   輔助組通過 `metadata.mode` 捕獲，驗證每個組共享
   相同的 `{target_arch, target_os}` 三重，並發出包含一個條目的 JSON 摘要
   每個模式。應該落在基線文件中的中位數位於
   `modes.<mode>.benchmarks`，同時附帶 `statistics` 塊記錄
   審閱者和 CI 的完整樣本列表、最小/最大、平均值和總體標準差。
   一旦聚合文件存在，您就可以自動編寫基線 JSON（使用
   標準公差圖）通過：
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   覆蓋 `--mode` 以限制為子集或 `--cpu-label` 以固定
   當聚合源省略時記錄 CPU 名稱。
   每個架構的兩台主機完成後，更新：
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json`（新）

   `aarch64_unknown_linux_gnu_*` 文件現在反映 `m3-pro-native`
   捕獲（保留CPU標籤和元數據註釋），因此`scripts/sm_perf.sh`可以
   自動檢測 aarch64-unknown-linux-gnu 主機，無需手動標記。當
   裸機實驗室運行完成，重新運行 `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   用新的捕獲覆蓋臨時中位數並標記真實值
   主機標籤。

   > 參考：2025 年 7 月 Apple Silicon 捕獲（CPU 標籤 `m3-pro-local`）為
   > 存檔於 `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`。
   > 當您發布 Neoverse/x86 工件時，請鏡像該佈局，以便審閱者
   > 可以一致地區分原始/聚合輸出。

### 4. 工件佈局和簽核
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` 記錄命令哈希、git 修訂、操作員和任何異常。
- 聚合的 JSON 文件直接輸入到基線更新中，並附加到 `docs/source/crypto/sm_perf_baseline_comparison.md` 中的性能審查中。
- QA Guild 在基線更改之前審查工件並在“性能”部分下的 `status.md` 中籤字。### 5. CI 門控時間表
|日期 |里程碑|行動|
|------|------------|--------|
| 2025-07-12 | Neoverse 捕獲完成 |更新 `sm_perf_baseline_aarch64_*` JSON 文件，在本地運行 `ci/check_sm_perf.sh`，打開附加了工件的 PR。 |
| 2025-07-24 | x86_64 捕獲完成 |在`ci/check_sm_perf.sh`中添加新的基線文件+門控；確保跨架構 CI 通道消耗它們。 |
| 2025-07-27 | CI 執行 |啟用 `sm-perf-gate` 工作流程在兩個主機類上運行；如果回歸超出配置的容差，則合併失敗。 |

### 6. 依賴關係和通信
- 通過 `infra-ops@iroha.tech` 協調實驗室訪問更改。  
- 性能工作組在捕獲運行時在 `#perf-lab` 頻道中發布每日更新。  
- QA Guild 準備比較差異 (`scripts/sm_perf_compare.py`)，以便審閱者可以可視化增量。  
- 基線合併後，使用捕獲完成註釋更新 `roadmap.md`（SM-4c.1a/b、SM-5a.3b）和 `status.md`。

通過該計劃，SM 加速工作獲得了可重複的中值、CI 門控和可追踪的證據線索，滿足“保留實驗室窗口和捕獲中值”行動項目。

### 7. CI 門和局部煙霧

- `ci/check_sm_perf.sh` 是規範的 CI 入口點。它為 `SM_PERF_MODES` 中的每種模式（默認為 `scalar auto neon-force`）解析為 `scripts/sm_perf.sh` 並設置 `CARGO_NET_OFFLINE=true`，以便工作台在 CI 映像上確定性地運行。  
- `.github/workflows/sm-neon-check.yml` 現在調用 macOS arm64 運行程序上的門，因此每個拉取請求都通過本地使用的相同幫助程序來執行標量/自動/霓虹燈三重奏；一旦 x86_64 佔領土地並且 Neoverse 代理基線通過裸機運行刷新，互補的 Linux/Neoverse 通道就會加入。  
- 操作員可以在本地覆蓋模式列表：`SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` 將運行修剪為單次通過以進行快速冒煙測試，而其他參數（例如 `--tolerance 0.20`）將直接轉發到 `scripts/sm_perf.sh`。  
- `make check-sm-perf` 現在包裹了大門，以方便開發人員； CI 作業可以直接調用腳本，而 macOS 開發人員則依賴 make 目標。  
- 一旦 Neoverse/x86_64 基線落地，相同的腳本將通過 `scripts/sm_perf.sh` 中已有的主機自動檢測邏輯獲取適當的 JSON，因此除了為每個主機池設置所需的模式列表之外，工作流程中不需要額外的接線。

### 8.季度刷新助手- 運行 `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` 以創建四分之一標記的目錄，例如 `artifacts/sm_perf/2026-Q1/<label>/`。該幫助程序包裝 `scripts/sm_perf_capture_helper.sh --matrix` 並發出 `capture_commands.sh`、`capture_plan.json` 和 `quarterly_plan.json`（所有者 + 季度元數據），以便實驗室操作員無需手寫計劃即可安排運行。
- 在目標主機上執行生成的 `capture_commands.sh`，將原始​​輸出與 `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` 聚合，並通過 `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite` 將中位數提升到基線 JSON。重新運行 `ci/check_sm_perf.sh` 以確認容差保持綠色。
- 當硬件或工具鏈發生變化時，刷新 `docs/source/crypto/sm_perf_baseline_comparison.md` 中的比較容差/註釋，如果新中位數穩定，則收緊 `ci/check_sm_perf.sh` 容差，並將任何儀表板/警報閾值與新基線對齊，以便操作警報保持有意義。
- 提交 `quarterly_plan.json`、`capture_plan.json`、`capture_commands.sh` 和聚合 JSON 以及基線更新；將相同的工件附加到狀態/路線圖更新中以實現可追溯性。