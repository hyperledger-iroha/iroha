---
lang: zh-hant
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9774dca76eff9ff13fcad9bf1fa7f084b95a987c392727cf0e6a74a4844e2b8e
source_last_modified: "2026-01-22T14:45:01.375247+00:00"
translation_last_reviewed: 2026-02-07
id: pq-rollout-plan
title: SNNet-16G Post-Quantum Rollout Playbook
sidebar_label: PQ Rollout Plan
description: Operational guide for promoting the SoraNet hybrid X25519+ML-KEM handshake from canary to default across relays, clients, and SDKs.
translator: machine-google-reviewed
---

:::注意規範來源
:::

SNNet-16G 完成了 SoraNet 傳輸的後量子部署。 `rollout_phase` 旋鈕使操作員能夠協調從現有 A 階段防護要求到 B 階段多數覆蓋率和 C 階段嚴格 PQ 姿勢的確定性升級，而無需為每個表面編輯原始 JSON/TOML。

本劇本涵蓋：

- 階段定義和新的配置旋鈕（`sorafs.gateway.rollout_phase`、`sorafs.rollout_phase`）連接到代碼庫（`crates/iroha_config/src/parameters/actual.rs:2230`、`crates/iroha/src/config/user.rs:251`）中。
- SDK 和 CLI 標誌映射，以便每個客戶端都可以跟踪部署。
- 中繼/客戶端金絲雀調度期望以及控制促銷的治理儀表板 (`dashboards/grafana/soranet_pq_ratchet.json`)。
- 回滾鉤子和對防火練習操作手冊的引用（[PQ 棘輪操作手冊](./pq-ratchet-runbook.md)）。

## 相位圖

| `rollout_phase` |有效匿名階段|默認效果 |典型用法 |
|-----------------|----------------------------------------|----------------|------------------------|
| `canary` | `anon-guard-pq`（A 階段）|當艦隊熱身時，每個迴路至少需要一名 PQ 警衛。 |基線和早期金絲雀週。 |
| `ramp` | `anon-majority-pq`（B 階段）|偏向 PQ 繼電器選擇，覆蓋範圍 >= 三分之二；經典中繼仍然是後備方案。 |逐個區域的中繼金絲雀； SDK 預覽切換。 |
| `default` | `anon-strict-pq`（C 階段）|實施僅限 PQ 的電路並加強降級警報。 |一旦遙測和治理簽字完成，最終晉升。 |

如果表面還設置了顯式 `anonymity_policy`，它將覆蓋該組件的相位。現在省略顯式階段遵循 `rollout_phase` 值，因此操作員可以在每個環境中翻轉階段一次並讓客戶端繼承它。

## 配置參考

### 協調器 (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

Orchestrator 加載程序在運行時解析回退階段 (`crates/sorafs_orchestrator/src/lib.rs:2229`)，並通過 `sorafs_orchestrator_policy_events_total` 和 `sorafs_orchestrator_pq_ratio_*` 來顯示它。有關可立即應用的代碼片段，請參閱 `docs/examples/sorafs_rollout_stage_b.toml` 和 `docs/examples/sorafs_rollout_stage_c.toml`。

### Rust 客戶端 / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` 現在記錄解析的階段 (`crates/iroha/src/client.rs:2315`)，因此幫助程序命令（例如 `iroha_cli app sorafs fetch`）可以報告當前階段以及默認的匿名策略。

## 自動化

兩個 `cargo xtask` 幫助程序自動生成計劃和捕獲工件。

1. **生成區域時間表**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   持續時間接受 `s`、`m`、`h` 或 `d` 後綴。該命令發出 `artifacts/soranet_pq_rollout_plan.json` 和 Markdown 摘要 (`artifacts/soranet_pq_rollout_plan.md`)，可以隨更改請求一起提供。

2. **捕獲帶有簽名的鑽孔製品**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   該命令將提供的文件複製到 `artifacts/soranet_pq_rollout/<timestamp>_<label>/`，計算每個工件的 BLAKE3 摘要，並在有效負載上寫入包含元數據和 Ed25519 簽名的 `rollout_capture.json`。使用簽署消防演習記錄的同一私鑰，以便治理可以快速驗證捕獲。

## SDK 和 CLI 標誌矩陣

|表面|金絲雀（A 階段）|坡道（B 階段）|默認（C 階段）|
|--------------------|--------------------------------|----------------|--------------------|
| `sorafs_cli` 獲取 | `--anonymity-policy stage-a` 還是靠相| `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator 配置 JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust 客戶端配置 (`iroha.toml`) | `rollout_phase = "canary"`（默認）| `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` 簽名命令 | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`，可選 `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`，可選 `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`，可選 `.ANON_STRICT_PQ` |
| JavaScript 協調器助手 | `rolloutPhase: "canary"` 或 `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
|斯威夫特 `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

所有 SDK 切換都映射到編排器 (`crates/sorafs_orchestrator/src/lib.rs:365`) 使用的同一階段解析器，因此混合語言部署與配置的階段保持同步。

## 金絲雀調度清單

1. **飛行前（T 減 2 週）**

- 確認 A 階段過去兩週的限電率 <1%，並且每個區域的 PQ 覆蓋率 >=70% (`sorafs_orchestrator_pq_candidate_ratio`)。
   - 安排批准金絲雀窗口的治理審查時段。
   - 在暫存中更新 `sorafs.gateway.rollout_phase = "ramp"`（編輯編排器 JSON 並重新部署）並試運行升級管道。

2. **接力金絲雀（T日）**

   - 通過在協調器和參與的中繼清單上設置 `rollout_phase = "ramp"`，一次升級一個區域。
   - 在 PQ Ratchet 儀表板（現在具有推出面板）中監視“每個結果的策略事件”和“掉電率”，以獲得兩倍的保護緩存 TTL。
   - 在運行之前和之後剪切 `sorafs_cli guard-directory fetch` 快照以用於審核存儲。

3. **客戶端/SDK 金絲雀（T 加 1 週）**

   - 在客戶端配置中翻轉 `rollout_phase = "ramp"` 或為指定的 SDK 群組傳遞 `stage-b` 覆蓋。
   - 捕獲遙測差異（`sorafs_orchestrator_policy_events_total` 由 `client_id` 和 `region` 分組）並將其附加到推出事件日誌中。

4. **默認促銷（T+3週）**

   - 治理結束後，將協調器和客戶端配置切換到 `rollout_phase = "default"`，並將簽名的準備清單輪換到發布工件中。

## 治理和證據清單

|相變|促銷門|證據包|儀表板和警報 |
|--------------|----------------|-----------------|---------------------|
|金絲雀 → 坡道 *（B 階段預覽）* |過去 14 天內 A 階段的管制率 <1%，每個促銷區域 `sorafs_orchestrator_pq_candidate_ratio` ≥ 0.7，Argon2 票證驗證 p95 < 50 毫秒，並且預訂了促銷的治理時段。 | `cargo xtask soranet-rollout-plan` JSON/Markdown 對、配對的 `sorafs_cli guard-directory fetch` 快照（之前/之後）、簽名的 `cargo xtask soranet-rollout-capture --label canary` 捆綁包以及引用 [PQ 棘輪運行手冊](./pq-ratchet-runbook.md) 的金絲雀分鐘。 | `dashboards/grafana/soranet_pq_ratchet.json`（策略事件 + 掉電率）、`dashboards/grafana/soranet_privacy_metrics.json`（SN16 降級比率）、`docs/source/soranet/snnet16_telemetry_plan.md` 中的遙測參考。 |
|斜坡 → 默認 *（C 階段實施）* | 30 天的 SN16 遙測老化測試滿足要求，`sn16_handshake_downgrade_total` 在基線上持平，`sorafs_orchestrator_brownouts_total` 在客戶端金絲雀期間為零，並且記錄了代理切換排練。 | `sorafs_cli proxy set-mode --mode gateway|direct` 轉錄本、`promtool test rules dashboards/alerts/soranet_handshake_rules.yml` 輸出、`sorafs_cli guard-directory verify` 日誌和簽名的 `cargo xtask soranet-rollout-capture --label default` 捆綁包。 |相同的 PQ Ratchet 板加上 `docs/source/sorafs_orchestrator_rollout.md` 和 `dashboards/grafana/soranet_privacy_metrics.json` 中記錄的 SN16 降級面板。 |
|緊急降級/回滾準備|當降級計數器激增、保護目錄驗證失敗或 `/policy/proxy-toggle` 緩衝區記錄持續降級事件時觸發。 | `docs/source/ops/soranet_transport_rollback.md`、`sorafs_cli guard-directory import` / `guard-cache prune` 日誌、`cargo xtask soranet-rollout-capture --label rollback`、事件憑單和通知模板中的清單。 | `dashboards/grafana/soranet_pq_ratchet.json`、`dashboards/grafana/soranet_privacy_metrics.json` 以及兩個警報包（`dashboards/alerts/soranet_handshake_rules.yml`、`dashboards/alerts/soranet_privacy_rules.yml`）。 |

- 使用生成的 `rollout_capture.json` 將每個工件存儲在 `artifacts/soranet_pq_rollout/<timestamp>_<label>/` 下，以便治理數據包包含記分板、promtool 跟踪和摘要。
- 將上傳證據的 SHA256 摘要（會議紀要 PDF、捕獲包、警衛快照）附加到晉升會議紀要中，以便無需訪問臨時集群即可重播議會批准。
- 參考促銷票中的遙測計劃，以證明 `docs/source/soranet/snnet16_telemetry_plan.md` 仍然是降級詞彙和警報閾值的規範來源。

## 儀表板和遙測更新

`dashboards/grafana/soranet_pq_ratchet.json` 現在附帶一個“推出計劃”註釋面板，該面板鏈接回此劇本並顯示當前階段，以便治理審查可以確認哪個階段處於活動狀態。使面板描述與配置旋鈕的未來更改保持同步。

對於警報，請確保現有規則使用 `stage` 標籤，以便金絲雀階段和默認階段觸發單獨的策略閾值 (`dashboards/alerts/soranet_handshake_rules.yml`)。

## 回滾鉤子

### 默認 → 斜坡（階段 C → 階段 B）

1. 使用 `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` 降級協調器（並在 SDK 配置中鏡像相同的階段），以便階段 B 在整個隊列範圍內恢復。
2. 通過 `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` 強制客戶端進入安全傳輸配置文件，捕獲記錄，以便 `/policy/proxy-toggle` 修復工作流程保持可審核狀態。
3. 運行 `cargo xtask soranet-rollout-capture --label rollback-default` 以在 `artifacts/soranet_pq_rollout/` 下存檔保護目錄差異、promtool 輸出和儀表板屏幕截圖。

### 坡道→金絲雀（B 階段→A 階段）

1. 使用 `sorafs_cli guard-directory import --guard-directory guards.json` 導入升級前捕獲的保護目錄快照，然後重新運行 `sorafs_cli guard-directory verify`，以便降級數據包包含哈希值。
2. 在協調器和客戶端配置上設置 `rollout_phase = "canary"`（或用 `anonymity_policy stage-a` 覆蓋），然後重播 [PQ 棘輪運行手冊](./pq-ratchet-runbook.md) 中的 PQ 棘輪演練以證明降級管道。
3. 在通知治理之前，將更新的 PQ Ratchet 和 SN16 遙測屏幕截圖以及警報結果附加到事件日誌中。

### 護欄提醒- 每當發生降級時參考 `docs/source/ops/soranet_transport_rollback.md`，並將任何臨時緩解措施記錄為部署跟踪器中的 `TODO:` 項目以進行後續工作。
- 在回滾之前和之後將 `dashboards/alerts/soranet_handshake_rules.yml` 和 `dashboards/alerts/soranet_privacy_rules.yml` 保持在 `promtool test rules` 覆蓋範圍內，以便將警報漂移與捕獲包一起記錄下來。