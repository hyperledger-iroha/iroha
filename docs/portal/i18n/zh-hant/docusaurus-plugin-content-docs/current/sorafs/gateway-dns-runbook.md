---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS 網關和 DNS 啟動運行手冊

此門戶副本反映了規范運行手冊
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
它捕獲了去中心化 DNS 和網關的操作護欄
工作流，以便網絡、運營和文檔主管可以排練
2025 年 3 月啟動之前的自動化堆棧。

## 範圍和可交付成果

- 通過演練確定性來綁定 DNS (SF-4) 和網關 (SF-5) 里程碑
  主機派生、解析器目錄發布、TLS/GAR 自動化和證據
  捕獲。
- 保留啟動輸入（議程、邀請、出席跟踪器、GAR 遙測）
  快照）與最新的所有者分配同步。
- 為治理審查者生成可審計的工件包：解析器
  目錄發行說明、網關探測日誌、一致性工具輸出以及
  Docs/DevRel 摘要。

## 角色和職責

|工作流程 |職責|所需文物|
|------------|--------------------------------|--------------------|
|網絡 TL（DNS 堆棧）|維護確定性主機計劃、運行 RAD 目錄版本、發布解析器遙測輸入。 | `artifacts/soradns_directory/<ts>/`，`docs/source/soradns/deterministic_hosts.md` 的差異，RAD 元數據。 |
|運營自動化主管（網關）|執行 TLS/ECH/GAR 自動化演練，運行 `sorafs-gateway-probe`，更新 PagerDuty 掛鉤。 | `artifacts/sorafs_gateway_probe/<ts>/`，探測 JSON，`ops/drill-log.md` 條目。 |
| QA 協會和工具工作組 |運行 `ci/check_sorafs_gateway_conformance.sh`、整理裝置、存檔 Norito 自我認證包。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|文檔/開發版本 |記錄會議記錄、更新設計預讀和附錄，並在此門戶中發布證據摘要。 |更新了 `docs/source/sorafs_gateway_dns_design_*.md` 文件和部署說明。 |

## 輸入和先決條件

- 確定性主機規範 (`docs/source/soradns/deterministic_hosts.md`) 和
  解析器證明腳手架 (`docs/source/soradns/resolver_attestation_directory.md`)。
- 網關文物：操作員手冊、TLS/ECH 自動化助手、
  直接模式指導以及 `docs/source/sorafs_gateway_*` 下的自我認證工作流程。
- 工具：`cargo xtask soradns-directory-release`，
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` 和 CI 助手
  （`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`）。
- 秘密：GAR 發布密鑰、DNS/TLS ACME 憑證、PagerDuty 路由密鑰、
  Torii 用於解析器獲取的身份驗證令牌。

## 飛行前檢查清單

1. 通過更新確認與會者和議程
   `docs/source/sorafs_gateway_dns_design_attendance.md` 並循環
   當前議程（`docs/source/sorafs_gateway_dns_design_agenda.md`）。
2. 舞台文物根源，例如
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` 和
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. 刷新固定裝置（GAR 清單、RAD 證明、網關一致性包）和
   確保 `git submodule` 狀態與最新的排練標籤匹配。
4. 驗證機密（Ed25519 發布密鑰、ACME 帳戶文件、PagerDuty 令牌）是否
   顯示並匹配保管庫校驗和。
5. 事先對遙測目標（Pushgateway 端點、GAR Grafana 板）進行冒煙測試
   到鑽頭。

## 自動化排練步驟

### 確定性主機映射和 RAD 目錄發布

1. 針對建議的清單運行確定性主機派生助手
   設置並確認沒有漂移
   `docs/source/soradns/deterministic_hosts.md`。
2. 生成解析器目錄包：

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3.在裡面記錄打印的目錄ID、SHA-256和輸出路徑
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` 和開球
   分鐘。

### DNS 遙測捕獲

- 尾部解析器透明度日誌使用≥10分鐘
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- 導出 Pushgateway 指標並在運行時存檔 NDJSON 快照
  身份證號目錄。

### 網關自動化演練

1. 執行 TLS/ECH 探測：

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 運行一致性工具 (`ci/check_sorafs_gateway_conformance.sh`) 並
   自認證助手 (`scripts/sorafs_gateway_self_cert.sh`) 刷新
   Norito 證明捆綁包。
3. 捕獲 PagerDuty/Webhook 事件以證明自動化路徑始終有效
   結束。

### 證據包裝

- 使用時間戳、參與者和探測哈希值更新 `ops/drill-log.md`。
- 將工件存儲在運行 ID 目錄下並發布執行摘要
  在 Docs/DevRel 會議紀要中。
- 在啟動審核之前鏈接治理票證中的證據包。

## 會議引導和證據移交

- **主持人時間表：**  
  - T‑24h — 項目管理在 `#nexus-steering` 中發布提醒 + 議程/出勤快照。  
  - T-2h — Networking TL 刷新 GAR 遙測快照並在 `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` 中記錄增量。  
  - T‑15m — Ops Automation 驗證探針準備情況並將活動運行 ID 寫入 `artifacts/sorafs_gateway_dns/current`。  
  - 在通話期間 - 主持人分享此操作手冊並指派一名實時抄寫員； Docs/DevRel 捕獲內聯操作項。
- **分鐘模板：** 複製骨架
  `docs/source/sorafs_gateway_dns_design_minutes.md`（也反映在門戶中
  包）並為每個會話提交一個填充實例。包括與會者名冊、
  決策、行動項目、證據哈希和突出風險。
- **證據上傳：** 從排練中壓縮 `runbook_bundle/` 目錄，
  附上渲染的會議記錄 PDF，在會議記錄 + 議程中記錄 SHA-256 哈希值，
  然後在上傳土地後 ping 治理審核者別名
  `s3://sora-governance/sorafs/gateway_dns/<date>/`。

## 證據快照（2025 年 3 月啟動）

路線圖和治理中引用的最新排練/現場製品
分鐘住在 `s3://sora-governance/sorafs/gateway_dns/` 桶下。哈希值
下面鏡像了規范清單 (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。

- **試運行 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - 捆綁包 tarball：`b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - 會議紀要 PDF：`cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **現場研討會 — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _（待上傳：`gateway_dns_minutes_20250303.pdf` — Docs/DevRel 將在渲染的 PDF 放入捆綁包後附加 SHA-256。）_

## 相關材料

- [網關操作手冊](./operations-playbook.md)
- [SoraFS 可觀測性計劃](./observability-plan.md)
- [去中心化 DNS 和網關跟踪器](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)