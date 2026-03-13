---
id: developer-deployment
lang: zh-hant
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deployment Notes
sidebar_label: Deployment Notes
description: Checklist for promoting the SoraFS pipeline from CI to production.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

# 部署注意事項

SoraFS 打包工作流程強化了確定性，因此從 CI 轉向
生產主要需要操作護欄。使用此清單時
將工具推廣到真正的網關和存儲提供商。

## 飛行前

- **註冊表對齊** — 確認分塊配置文件和清單引用
  相同的 `namespace.name@semver` 元組 (`docs/source/sorafs/chunker_registry.md`)。
- **准入政策** — 審查已簽名的提供商廣告和別名證明
  `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) 需要。
- **Pin 註冊表操作手冊** — 保留 `docs/source/sorafs/runbooks/pin_registry_ops.md`
  對於恢復場景（別名輪換、複製失敗）很方便。

## 環境配置

- 網關必須啟用證明流端點 (`POST /v2/sorafs/proof/stream`)
  因此 CLI 可以發出遙測摘要。
- 使用中的默認值配置 `sorafs_alias_cache` 策略
  `iroha_config` 或 CLI 幫助程序 (`sorafs_cli manifest submit --alias-*`)。
- 通過安全秘密管理器提供流令牌（或 Torii 憑證）。
- 啟用遙測導出器（`torii_sorafs_proof_stream_*`，
  `torii_sorafs_chunk_range_*`）並將它們運送到您的 Prometheus/OTel 堆棧。

## 推出策略

1. **藍/綠艙單**
   - 使用 `manifest submit --summary-out` 存檔每次部署的響應。
   - 密切關注 `torii_sorafs_gateway_refusals_total` 以捕獲功能
     早期不匹配。
2. **證明驗證**
   - 將 `sorafs_cli proof stream` 中的故障視為部署阻礙因素；延遲
     峰值通常表明提供商受到限製或層配置錯誤。
   - `proof verify` 應成為引腳後冒煙測試的一部分，以確保 CAR
     由提供商託管的仍然與清單摘要匹配。
3. **遙測儀表板**
   - 將 `docs/examples/sorafs_proof_streaming_dashboard.json` 導入 Grafana。
   - 為引腳註冊表健康分層附加面板
     (`docs/source/sorafs/runbooks/pin_registry_ops.md`) 和塊範圍統計信息。
4. **多源支持**
   - 按照中的分階段推出步驟進行操作
     開機時 `docs/source/sorafs/runbooks/multi_source_rollout.md`
     編排器，並將記分板/遙測工件存檔以供審核。

## 事件處理

- 按照 `docs/source/sorafs/runbooks/` 中的升級路徑進行操作：
  - `sorafs_gateway_operator_playbook.md` 用於網關中斷和流令牌
    精疲力盡。
  - 當發生復制爭議時，`dispute_revocation_runbook.md`。
  - `sorafs_node_ops.md` 用於節點級維護。
  - `multi_source_rollout.md` 用於協調器覆蓋、對等黑名單和
    分階段推出。
- 通過現有的GovernanceLog記錄證明失敗和延遲異常
  PoR 跟踪器 API，以便治理可以評估提供商的績效。

## 後續步驟

- 一旦集成協調器自動化（`sorafs_car::multi_fetch`）
  多源獲取協調器 (SF-6b) 登陸。
- 跟踪 SF-13/SF-14 下的 PDP/PoTR 升級； CLI 和文檔將演變為
  一旦這些證據穩定下來，就會出現表面的截止日期和等級選擇。

通過將這些部署說明與快速入門和 CI 配方相結合，團隊
可以從本地實驗轉移到生產級 SoraFS 管道
可重複、可觀察的過程。