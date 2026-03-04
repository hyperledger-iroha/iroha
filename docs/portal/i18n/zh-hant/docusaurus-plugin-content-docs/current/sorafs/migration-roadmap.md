---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Migration Roadmap"
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 改編自 [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md)。

# SoraFS 遷移路線圖 (SF-1)

本文檔實施了中捕獲的遷移指南
`docs/source/sorafs_architecture_rfc.md`。它將 SF-1 可交付成果擴展為
執行就緒的里程碑、門控標準和所有者清單，以便存儲、
工件託管到 SoraFS 支持的出版物。

路線圖是有意確定性的：每個里程碑都指定了所需的內容
工件、命令調用和證明步驟，以便下游管道
產生相同的輸出，治理保留可審計的跟踪。

## 里程碑概述

|里程碑|窗口|主要目標|必鬚髮貨 |業主|
|------------|--------|----------------|------------|--------|
| **M1 – 確定性執行** |第 7-12 週 |當管道採用期望標誌時，強制執行簽名的裝置和階段別名證明。 |每晚固定裝置驗證、理事會簽署的清單、別名註冊表暫存條目。 |存儲、治理、SDK |

里程碑狀態在 `docs/source/sorafs/migration_ledger.md` 中跟踪。全部
對此路線圖的更改必須更新分類帳以保持治理和發布
工程同步。

## 工作流

### 2. 確定性固定採用

|步驟|里程碑|描述 |所有者 |輸出|
|------|------------|-------------|---------|--------|
|固定排練| M0 |每週進行一次演練，將本地塊摘要與 `fixtures/sorafs_chunker` 進行比較。在 `docs/source/sorafs/reports/` 下發布報告。 |存儲提供商| `determinism-<date>.md` 帶有通過/失敗矩陣。 |
|強制簽名 | M1 |如果簽名或清單發生偏差，`ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` 會失敗。開發優先權需要 PR 附帶的治理豁免。 |工具工作組 | CI 日誌、豁免票證鏈接（如果適用）。 |
|期望旗幟| M1 |管道調用 `sorafs_manifest_stub` 並明確期望引腳輸出： |文檔 CI |更新了引用期望標誌的腳本（請參閱下面的命令塊）。 |
|註冊表優先固定| M2| `sorafs pin propose` 和 `sorafs pin approve` 包裝清單提交； CLI 默認為 `--require-registry`。 |治理行動|註冊表 CLI 審核日誌、失敗提案的遙測。 |
|可觀測性平價 | M3 |當塊庫存與註冊表清單不一致時，Prometheus/Grafana 儀表板會發出警報；警報連接到待命的操作人員。 |可觀察性|儀表板鏈接、警報規則 ID、GameDay 結果。 |

#### 規範發布命令

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

將摘要、大小和 CID 值替換為記錄在中的預期引用
工件的遷移分類帳條目。

### 3. 別名轉換和通信

|步驟|里程碑|描述 |所有者 |輸出|
|------|------------|-------------|---------|--------|
|暫存中的別名證明 | M1 |在 Pin 註冊表暫存環境中註冊別名聲明，並將 Merkle 證明附加到清單 (`--alias`)。 |治理，文檔 |證明包存儲在帶有別名的清單 + 賬本註釋旁邊。 |
|證據執行 | M2|網關拒絕沒有新 `Sora-Proof` 標頭的清單； CI 獲得 `sorafs alias verify` 步來獲取證明。 |網絡|網關配置補丁+ CI 輸出捕獲驗證成功。 |

### 4.溝通與審計

- **賬本規則：**每次狀態變化（夾具漂移、註冊表提交、
  別名激活）必須附加註明日期的註釋
  `docs/source/sorafs/migration_ledger.md`。
- **治理會議紀要：** 理事會會議批准 PIN 註冊更改或
  別名策略必須引用此路線圖和分類賬。
- **外部通訊：** DevRel 在每個里程碑發布狀態更新（博客 +
  變更日誌摘錄）強調確定性保證和別名時間表。

## 依賴性和風險

|依賴|影響 |緩解措施 |
|------------|--------|------------|
| Pin 註冊合同可用性 |阻止 M2 引腳優先推出。 |在 M2 之前進行階段合約並進行重播測試；保持包絡回退直至無回歸。 |
|理事會簽名密鑰 |艙單信封和登記處批准所必需的。 |簽字儀式記錄在`docs/source/sorafs/signing_ceremony.md`中；旋轉帶有重疊和分類帳註釋的密鑰。 |
| SDK 發布節奏 |客戶必須在 M3 之前遵守別名證明。 |將 SDK 發布窗口與里程碑門保持一致；將遷移清單添加到發布模板。 |

剩餘風險和緩解措施反映在 `docs/source/sorafs_architecture_rfc.md` 中
並在調整時應相互參考。

## 退出標準清單

|里程碑|標準|
|------------|----------|
| M1 | - 每晚固定工作連續七天呈綠色。 <br /> - 在 CI 中驗證暫存別名證明。 <br /> - 治理批准預期標誌政策。 |

## 變革管理

1. 通過 PR 更新此文件提出調整**和**
   `docs/source/sorafs/migration_ledger.md`。
2. PR 描述中支持治理會議紀要和 CI 證據的鏈接。
3. 合併時，通知存儲 + DevRel 郵件列表以及摘要和預期
   操作員的動作。

遵循此過程可確保 SoraFS 部署保持確定性，
參與 Nexus 發布的團隊之間可審核且透明。