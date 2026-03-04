---
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aa25c267f36e3245866776d5149039e1b9833407a84126d66a21cf5296e51414
source_last_modified: "2025-12-29T18:16:35.135788+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-bootstrap-plan
title: Sora Nexus bootstrap & observability
description: Operational plan for bringing the core Nexus validator cluster online before layering SoraFS and SoraNet services.
translator: machine-google-reviewed
---

:::注意規範來源
此頁面鏡像 `docs/source/soranexus_bootstrap_plan.md`。保持兩個副本對齊，直到本地化版本登陸門戶。
:::

# Sora Nexus 引導程序和可觀測性計劃

## 目標
- 使用治理密鑰、Torii API 和共識監控建立基礎 Sora Nexus 驗證器/觀察者網絡。
- 在啟用 SoraFS/SoraNet 搭載部署之前驗證核心服務（Torii、共識、持久性）。
- 建立 CI/CD 工作流程和可觀察性儀表板/警報，以確保網絡健康。

## 先決條件
- HSM 或 Vault 中提供治理密鑰材料（理事會多重簽名、委員會密鑰）。
- 主/輔助區域中的基準基礎設施（Kubernetes 集群或裸機節點）。
- 更新了引導程序配置（`configs/nexus/bootstrap/*.toml`），反映了最新的共識參數。

## 網絡環境
- 運行兩個具有不同網絡前綴的 Nexus 環境：
- **Sora Nexus（主網）** – 生產網絡前綴 `nexus`，託管規範治理和 SoraFS/SoraNet 搭載服務（鏈 ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`）。
- **Sora Testus（測試網）** – 暫存網絡前綴 `testus`，鏡像主網配置以進行集成測試和預發布驗證（鏈 UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`）。
- 為每個環境維護單獨的創世文件、治理密鑰和基礎設施足跡。 Testus 充當升級到 Nexus 之前所有 SoraFS/SoraNet 部署的試驗場。
- CI/CD 管道應首先部署到 Testus，執行自動冒煙測試，並在檢查通過後需要手動升級到 Nexus。
- 參考配置包位於 `configs/soranexus/nexus/`（主網）和 `configs/soranexus/testus/`（測試網）下，每個包含示例 `config.toml`、`genesis.json` 和 Torii 准入目錄。

## 步驟 1 – 配置審查
1. 審核現有文檔：
   - `docs/source/nexus/architecture.md`（共識，Torii 佈局）。
   - `docs/source/nexus/deployment_checklist.md`（基礎要求）。
   - `docs/source/nexus/governance_keys.md`（密鑰保管程序）。
2. 驗證創世文件 (`configs/nexus/genesis/*.json`) 與當前驗證者名單和質押權重是否一致。
3、確認網絡參數：
   - 共識委員會規模和法定人數。
   - 區塊間隔/最終性閾值。
   - Torii 服務端口和 TLS 證書。

## 步驟 2 – Bootstrap 集群部署
1. 配置驗證節點：
   - 使用持久卷部署 `irohad` 實例（驗證器）。
   - 確保網絡防火牆規則允許節點之間達成共識和 Torii 流量。
2. 在每個使用 TLS 的驗證器上啟動 Torii 服務 (REST/WebSocket)。
3. 部署觀察者節點（只讀）以獲得額外的彈性。
4. 運行引導腳本（`scripts/nexus_bootstrap.sh`）來分發創世幣、啟動共識並註冊節點。
5. 執行冒煙測試：
   - 通過 Torii (`iroha_cli tx submit`) 提交測試交易。
   - 通過遙測驗證區塊生產/最終性。
   - 檢查驗證者/觀察者之間的賬本複制。

## 步驟 3 – 治理和密鑰管理
1. 加載理事會多重簽名配置；確認可以提交並批准治理提案。
2. 安全存儲共識/委員會密鑰；配置帶有訪問日誌記錄的自動備份。
3. 設置緊急密鑰輪換程序 (`docs/source/nexus/key_rotation.md`) 並驗證運行手冊。

## 步驟 4 – CI/CD 集成
1.配置管道：
   - 構建並發布驗證器/Torii 圖像（GitHub Actions 或 GitLab CI）。
   - 自動配置驗證（lint genesis、驗證簽名）。
   - 用於登台和生產集群的部署管道（Helm/Kustomize）。
2. 在 CI 中實施冒煙測試（啟動臨時集群，運行規範事務套件）。
3. 添加失敗部署的回滾腳本和文檔 Runbook。

## 步驟 5 – 可觀察性和警報
1. 每個區域部署監控堆棧（Prometheus + Grafana + Alertmanager）。
2. 收集核心指標：
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - 通過 Loki/ELK 記錄 Torii 和共識服務。
3. 儀表板：
   - 共識健康（區塊高度、最終性、對等狀態）。
   - Torii API 延遲/錯誤率。
   - 治理交易和提案狀態。
4. 警報：
   - 區塊生產停滯（>2 個區塊間隔）。
   - 對等計數低於法定人數。
   - Torii 錯誤率峰值。
   - 治理提案隊列積壓。

## 步驟 6 – 驗證和移交
1. 運行端到端驗證：
   - 提交治理提案（例如參數更改）。
   - 通過理事會批准進行處理，以確保治理管道發揮作用。
   - 運行賬本狀態差異以確保一致性。
2. 記錄待命運行手冊（事件響應、故障轉移、擴展）。
3. 向 SoraFS/SoraNet 團隊傳達準備情況；確認搭載部署可以指向 Nexus 節點。

## 實施清單
- [ ] 創世/配置審核已完成。
- [ ] 驗證者和觀察者節點以健康共識部署。
- [ ] 治理密鑰已加載，建議已測試。
- [ ] CI/CD 管道正在運行（構建 + 部署 + 冒煙測試）。
- [ ] 可觀察性儀表板帶有警報。
- [ ] 將文檔交付給下游團隊。