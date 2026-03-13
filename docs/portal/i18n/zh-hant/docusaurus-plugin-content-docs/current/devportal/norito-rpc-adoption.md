---
id: norito-rpc-adoption
lang: zh-hant
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 規範的規劃說明位於 `docs/source/torii/norito_rpc_adoption_schedule.md` 中。  
> 此門戶副本提煉了 SDK 作者、運營商和審閱者的推出期望。

## 目標

- 在 AND4 生產切換之前對齊二進制 Norito-RPC 傳輸上的每個 SDK（Rust CLI、Python、JavaScript、Swift、Android）。
- 保持階段門、證據包和遙測掛鉤的確定性，以便治理可以審核部署。
- 使用路線圖 NRPC-4 調用的共享助手來輕鬆捕獲固定裝置和金絲雀證據。

## 階段時間線

|相|窗口|範圍 |退出標準 |
|--------|--------|--------|---------------|
| **P0 – 實驗室平價** | 2025 年第二季度 | Rust CLI + Python Smoke 套件在 CI 中運行 `/v2/norito-rpc`，JS 幫助程序通過單元測試，Android 模擬線束練習雙重傳輸。 | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` 和 `javascript/iroha_js/test/noritoRpcClient.test.js` 在 CI 中呈綠色； Android 線束連接至 `./gradlew test`。 |
| **P1 – SDK 預覽** | Q32025 |簽入共享夾具包，`scripts/run_norito_rpc_fixtures.sh --sdk <label>` 在 `artifacts/norito_rpc/` 中記錄日誌 + JSON，SDK 示例中公開可選的 Norito 傳輸標誌。 |已簽名的夾具清單、自述文件更新顯示選擇加入的使用情況、IOS2 標誌後面可用的 Swift 預覽 API。 |
| **P2 – 分期/AND4 預覽** | Q12026 |暫存 Torii 池更喜歡 Norito，Android AND4 預覽客戶端和 Swift IOS2 奇偶校驗套件默認為二進制傳輸，填充遙測儀表板 `dashboards/grafana/torii_norito_rpc_observability.json`。 | `docs/source/torii/norito_rpc_stage_reports.md` 捕獲金絲雀，`scripts/telemetry/test_torii_norito_rpc_alerts.sh` 通過，Android 模擬線束重播捕獲成功/錯誤情況。 |
| **P3 – 正式發布** | Q42026 | Norito 成為所有 SDK 的默認傳輸； JSON 仍然是一種停電後備方案。發布作業歸檔每個標籤的奇偶工件。 |發布 Rust/JS/Python/Swift/Android 的清單包 Norito 煙霧輸出；強制執行 Norito 與 JSON 錯誤率 SLO 的警報閾值； `status.md` 和發行說明引用了 GA 證據。 |

## SDK 可交付成果和 CI 掛鉤

- **Rust CLI 和集成工具** – 擴展 `iroha_cli pipeline` 煙霧測試，以在 `cargo xtask norito-rpc-verify` 落地後強制 Norito 傳輸。使用 `cargo test -p integration_tests -- norito_streaming`（實驗室）和 `cargo xtask norito-rpc-verify`（登台/GA）進行防護，將工件存儲在 `artifacts/norito_rpc/` 下。
- **Python SDK** – 默認釋放煙霧 (`python/iroha_python/scripts/release_smoke.sh`) 為 Norito RPC，保留 `run_norito_rpc_smoke.sh` 作為 CI 入口點，並在 `python/iroha_python/README.md` 中進行文檔奇偶校驗處理。 CI 目標：`PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`。
- **JavaScript SDK** – 穩定 `NoritoRpcClient`，讓治理/查詢助手在 `toriiClientConfig.transport.preferred === "norito_rpc"` 時默認為 Norito，並捕獲 `javascript/iroha_js/recipes/` 中的端到端樣本。 CI 必須在發布之前運行 `npm test` 以及 dockerized `npm run test:norito-rpc` 作業；來源在 `javascript/iroha_js/artifacts/` 下上傳 Norito 煙霧日誌。
- **Swift SDK** – 在 IOS2 標誌後面連接 Norito 橋接傳輸，鏡像夾具節奏，並確保 Connect/Norito 奇偶校驗套件在 `docs/source/sdk/swift/index.md` 中引用的 Buildkite 通道內運行。
- **Android SDK** – AND4 預覽客戶端和模擬 Torii 工具採用 Norito，重試/退避遙測記錄在 `docs/source/sdk/android/networking.md` 中。該線束通過 `scripts/run_norito_rpc_fixtures.sh --sdk android` 與其他 SDK 共享固定裝置。

## 證據和自動化

- `scripts/run_norito_rpc_fixtures.sh` 包裝 `cargo xtask norito-rpc-verify`，捕獲 stdout/stderr，並發出 `fixtures.<sdk>.summary.json`，因此 SDK 所有者可以將確定性工件附加到 `status.md`。使用 `--sdk <label>` 和 `--out artifacts/norito_rpc/<stamp>/` 保持 CI 包整潔。
- `cargo xtask norito-rpc-verify` 強制架構哈希奇偶校驗 (`fixtures/norito_rpc/schema_hashes.json`)，如果 Torii 返回 `X-Iroha-Error-Code: schema_mismatch`，則失敗。將每個失敗與 JSON 後備捕獲配對以進行調試。
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` 和 `dashboards/grafana/torii_norito_rpc_observability.json` 定義 NRPC-2 的警報合約。每次儀表板編輯後運行腳本並將 `promtool` 輸出存儲在金絲雀包中。
- `docs/source/runbooks/torii_norito_rpc_canary.md` 描述了分階段和生產演習；每當夾具哈希或警報門發生變化時更新它。

## 審稿人清單

在勾選 NRPC-4 里程碑之前，請確認：

1. 最新的裝置包哈希值與 `fixtures/norito_rpc/schema_hashes.json` 和 `artifacts/norito_rpc/<stamp>/` 下記錄的相應 CI 工件相匹配。
2. SDK 自述文件/門戶文檔描述瞭如何強制 JSON 回退並引用 Norito 傳輸默認值。
3. 遙測儀表板顯示帶有警報鏈接的雙堆棧錯誤率面板，並且 Alertmanager 試運行 (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) 連接到跟踪器。
4. 這裡的採用時間表與跟踪器條目 (`docs/source/torii/norito_rpc_tracker.md`) 匹配，路線圖 (NRPC-4) 引用相同的證據包。

遵守時間表可以保持跨 SDK 行為的可預測性，並允許治理審核 Norito-RPC 的採用，而無需定制請求。