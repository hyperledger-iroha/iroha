---
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/gar-operator-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: GAR Operator Onboarding
sidebar_label: GAR Operator Onboarding
description: Checklist to activate SNNet-9 compliance policies with attestation digests and evidence capture.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

使用此簡介來推出具有可重複的 SNNet-9 合規性配置，
審計友好的流程。將其與管轄權審查配對，以便每個運營商
使用相同的摘要和證據佈局。

## 步驟

1. **組裝配置**
   - 導入 `governance/compliance/soranet_opt_outs.json`。
   - 將您的 `operator_jurisdictions` 與已發布的證明摘要合併
     在[管轄權審查](gar-jurisdictional-review)中。
2. **驗證**
   - `cargo test -p sorafs_orchestrator -- compliance_policy_parses_from_json`
   - `cargo test -p sorafs_orchestrator -- compliance_example_config_parses`
   - 可選：`cargo xtask soranet-privacy-report --max-suppression-ratio 0.2 --ndjson <privacy-log.ndjson>`
3. **獲取證據**
   - 存儲在 `artifacts/soranet/compliance/<YYYYMMDD>/` 下：
     - `config.json`（最終合規塊）
     - `attestations.json`（URI + 摘要）
     - 驗證日誌
     - 簽名 PDF/Norito 信封的參考
4. **激活**
   - 標記推出 (`gar-opt-out-<date>`)，重新部署 Orchestrator/SDK 配置，
     並確認 `compliance_*` 事件在日誌中按預期發出。
5. **平倉**
   - 向管理委員會提交證據包。
   - 在 GAR 日誌中記錄激活窗口 + 批准者。
   - 從管轄權審查表中安排下次審查日期。

## 快速清單

- [ ] `jurisdiction_opt_outs` 與規範目錄匹配。
- [ ] 準確複製證明摘要。
- [ ] 驗證命令運行並存檔。
- [ ] 證據包存儲在 `artifacts/soranet/compliance/<date>/` 中。
- [ ] 推出標籤 + GAR 日誌已更新。
- [ ] 下次回顧提醒設置。

## 另請參閱

- [GAR 管轄權審查](gar-jurisdictional-review)
- [GAR 合規手冊（來源）](../../../source/soranet/gar_compliance_playbook.md)