---
id: pq-ratchet-runbook
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/pq-ratchet-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet PQ Ratchet Fire Drill
sidebar_label: PQ Ratchet Runbook
description: On-call rehearsal steps for promoting or demoting the staged PQ anonymity policy with deterministic telemetry validation.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意規範來源
:::

## 目的

本操作手冊指導 SoraNet 分階段後量子 (PQ) 匿名策略的防火演習順序。當 PQ 供應下降時，運營商會排練升級（階段 A -> 階段 B -> 階段 C）並受控降級回階段 B/A。該演練驗證遙測掛鉤（`sorafs_orchestrator_policy_events_total`、`sorafs_orchestrator_brownouts_total`、`sorafs_orchestrator_pq_ratio_*`）並收集事件排練日誌的工件。

## 先決條件

- 具有能力加權的最新 `sorafs_orchestrator` 二進製文件（在 `docs/source/soranet/reports/pq_ratchet_validation.md` 中顯示的鑽取參考處或之後提交）。
- 訪問服務於 `dashboards/grafana/soranet_pq_ratchet.json` 的 Prometheus/Grafana 堆棧。
- 名義保護目錄快照。在練習之前獲取並驗證副本：

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./artefacts/guard_directory_pre_drill.norito \
  --expected-directory-hash <directory-hash-hex>
```

如果源目錄僅發布 JSON，請在運行輪換助手之前使用 `soranet-directory build` 將其重新編碼為 Norito 二進製文件。

- 使用 CLI 捕獲元數據和前期發行人輪換工件：

```bash
soranet-directory inspect \
  --snapshot ./artefacts/guard_directory_pre_drill.norito
soranet-directory rotate \
  --snapshot ./artefacts/guard_directory_pre_drill.norito \
  --out ./artefacts/guard_directory_post_drill.norito \
  --keys-out ./artefacts/guard_issuer_rotation --overwrite
```

- 經網絡和可觀察性待命團隊批准的更改窗口。

## 推廣步驟

1. **階段審核**

   記錄起始階段：

   ```bash
   sorafs_cli config get --config orchestrator.json sorafs.anonymity_policy
   ```

   升級前預計為 `anon-guard-pq`。

2. **晉升至B階段（多數PQ）**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   - 等待 >=5 分鐘以刷新清單。
   - 在 Grafana（`SoraNet PQ Ratchet Drill` 儀表板）中，確認“策略事件”面板顯示 `outcome=met` 和 `stage=anon-majority-pq`。
   - 捕獲屏幕截圖或面板 JSON 並將其附加到事件日誌中。

3. **晉升至C階段（嚴格PQ）**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-strict-pq
   ```

   - 驗證 `sorafs_orchestrator_pq_ratio_*` 直方圖趨勢為 1.0。
   - 確認掉電計數器保持平穩；否則請遵循降級步驟。

## 降級/限電演習

1. **引起合成PQ短缺**

   通過將守衛目錄僅修剪為經典條目來禁用 Playground 環境中的 PQ 中繼，然後重新加載 Orchestrator 緩存：

   ```bash
   sorafs_cli guard-cache prune --config orchestrator.json --keep-classical-only
   ```

2. **觀察斷電遙測**

   - 儀表板：面板“掉電率”峰值高於 0。
   - PromQL：`sum(rate(sorafs_orchestrator_brownouts_total{region="$region"}[5m]))`
   - `sorafs_fetch` 應報告 `anonymity_outcome="brownout"` 和 `anonymity_reason="missing_majority_pq"`。

3. **降級至B階段/A階段**

   ```bash
   sorafs_cli config set --config orchestrator.json \
     sorafs.anonymity_policy anon-majority-pq
   ```

   如果PQ供應仍然不足，則降級為`anon-guard-pq`。一旦限電計數器穩定並且可以重新應用促銷活動，演練就完成了。

4. **恢復守衛目錄**

   ```bash
   sorafs_cli guard-directory import \
     --config orchestrator.json \
     --input ./artefacts/guard_directory_pre_drill.json
   ```

## 遙測和文物

- **儀表板：** `dashboards/grafana/soranet_pq_ratchet.json`
- **Prometheus 警報：** 確保 `sorafs_orchestrator_policy_events_total` 掉電警報保持在配置的 SLO 以下（在任何 10 分鐘窗口內 <5%）。
- **事件日誌：** 將捕獲的遙測片段和操作員註釋附加到 `docs/examples/soranet_pq_ratchet_fire_drill.log`。
- **簽名捕獲：**使用 `cargo xtask soranet-rollout-capture` 將演練日誌和記分板複製到 `artifacts/soranet_pq_rollout/<timestamp>/`，計算 BLAKE3 摘要，並生成簽名的 `rollout_capture.json`。

示例：

```
cargo xtask soranet-rollout-capture \
  --log logs/pq_fire_drill.log \
  --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
  --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
  --key secrets/pq_rollout_signing_ed25519.hex \
  --phase ramp \
  --label "drill-2026-02-21"
```

將生成的元數據和簽名附加到治理數據包中。

## 回滾

如果演習發現真正的 PQ 短缺，請留在階段 A，通知網絡 TL，並將收集的指標以及防護目錄差異附加到事件跟踪器。使用之前捕獲的guard目錄導出來恢復正常服務。

:::提示回歸覆蓋率
`cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics` 提供了支持此演練的綜合驗證。
:::