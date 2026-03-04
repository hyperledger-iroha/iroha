---
id: nexus-transition-notes
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Nexus transition notes
description: Mirror of `docs/source/nexus_transition_notes.md`, covering Phase B transition evidence, audit schedule, and mitigations.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus 過渡註釋

本日誌追踪了揮之不去的 **B 階段 — Nexus 過渡基礎** 工作
直到多通道啟動清單完成。它補充了里程碑
`roadmap.md` 中的條目並將 B1–B4 引用的證據保留在一個地方
因此治理、SRE 和 SDK 領導可以共享相同的事實來源。

## 範圍和節奏

- 涵蓋路由跟踪審計和遙測護欄 (B1/B2)、
  治理批准的配置增量集（B3），以及多通道啟動
  排練跟進（B4）。
- 替換以前住在這裡的臨時節奏音符；截至 2026 年
  Q1 審核詳細報告位於
  `docs/source/nexus_routed_trace_audit_report_2026q1.md`，而此頁面擁有
  運行時間表和緩解寄存器。
- 在每個路由跟踪窗口、治理投票或啟動後更新表
  排練。每當文物移動時，都會鏡像此頁面內的新位置
  因此下游文檔（狀態、儀表板、SDK 門戶）可以鏈接到穩定的
  錨。

## 證據快照（2026 年第一季度至第二季度）

|工作流程 |證據|所有者 |狀態 |筆記|
|------------|----------|----------|--------|--------|
| **B1 — 路由跟踪審計** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`、`docs/examples/nexus_audit_outcomes/` | @telemetry-ops，@governance | ✅ 完成（2026 年第一季度）|記錄了三個審核窗口； `TRACE-CONFIG-DELTA` 的 TLS 滯後在第二季度重新運行期間關閉。 |
| **B2 — 遙測修復和護欄** | `docs/source/nexus_telemetry_remediation_plan.md`、`docs/source/telemetry.md`、`dashboards/alerts/nexus_audit_rules.yml` | @sre-core，@telemetry-ops | ✅ 完整 |已發貨警報包、diff bot 策略和 OTLP 批量大小調整（`nexus.scheduler.headroom` 日誌 + Grafana 餘量面板）；沒有公開的豁免。 |
| **B3 — 配置增量批准** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`、`defaults/nexus/config.toml`、`defaults/nexus/genesis.json` | @release-eng、@governance | ✅ 完整 | GOV-2026-03-19 投票已捕獲；簽名的捆綁包提供下面提到的遙測包。 |
| **B4 — 多車道發射排練** | `docs/source/runbooks/nexus_multilane_rehearsal.md`、`docs/source/project_tracker/nexus_rehearsal_2026q1.md`、`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`、`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`、`artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core，@sre-core | ✅ 完成（2026 年第 2 季度）|第二季度金絲雀重新運行關閉了 TLS 延遲緩解措施；驗證器清單 + `.sha256` 捕獲槽範圍 912–936、工作負載種子 `NEXUS-REH-2026Q2` 以及重新運行時記錄的 TLS 配置文件哈希。 |

## 每季度路由跟踪審核計劃

|跟踪 ID |窗口 (UTC) |結果|筆記|
|----------|--------------|---------|--------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | ✅ 通過 |隊列准入 P95 遠低於 ≤750 毫秒的目標。無需採取任何行動。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | ✅ 通過 | OTLP 重放哈希值附加到 `status.md`； SDK diff bot 奇偶校驗確認零漂移。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | ✅ 已解決 | TLS 配置文件滯後在第二季度重新運行期間關閉； `NEXUS-REH-2026Q2` 的遙測包記錄 TLS 配置文件哈希 `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`（請參閱 `artifacts/nexus/tls_profile_rollout_2026q2/`）和零落後者。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12–10:14 | ✅ 通過 |工作負載種子 `NEXUS-REH-2026Q2`；遙測包 + `artifacts/nexus/rehearsals/2026q1/` 下的清單/摘要（槽位範圍 912–936），議程位於 `artifacts/nexus/rehearsals/2026q2/` 中。 |

未來幾個季度應該添加新行並移動
當表增長超出當前範圍時已完成附錄條目
季度。從路由跟踪報告或治理會議記錄中引用此部分
使用 `#quarterly-routed-trace-audit-schedule` 錨點。

## 緩解和積壓項目

|項目 |描述 |業主|目標|狀態/註釋|
|------|-------------|--------|--------------------|----------------|
| `NEXUS-421` |完成 `TRACE-CONFIG-DELTA` 期間滯後的 TLS 配置文件的傳播，捕獲重新運行證據，並關閉緩解日誌。 | @release-eng，@sre-core | 2026 年第 2 季度路由跟踪窗口 | ✅ 已關閉 — 在 `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` 中捕獲的 TLS 配置文件哈希 `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`；重播確認沒有掉隊者。 |
| `TRACE-MULTILANE-CANARY` 準備 |安排第二季度排練，將固定裝置連接到遙測包，並確保 SDK 線束重用經過驗證的助手。 | @telemetry-ops，SDK 程序 |規劃電話 2026-04-30 | ✅ 已完成 — 議程存儲在 `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` 中，並包含插槽/工作負載元數據；跟踪器中記錄了安全帶的重複使用情況。 |
|遙測包摘要輪換 |在每次排練/發布之前運行 `scripts/telemetry/validate_nexus_telemetry_pack.py`，並在配置增量跟踪器旁邊記錄摘要。 | @遙測操作 |每個候選版本 | ✅ 已完成 — `telemetry_manifest.json` + `.sha256` 在 `artifacts/nexus/rehearsals/2026q1/` 中發出（時隙範圍 `912-936`，種子 `NEXUS-REH-2026Q2`）；複製到跟踪器和證據索引中的摘要。 |

## 配置 Delta Bundle 集成

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` 仍然是
  規範差異摘要。當新的 `defaults/nexus/*.toml` 或創世發生變化時
  著陸後，首先更新該跟踪器，然後在此處鏡像高光。
- 簽名的配置包提供排練遙測包。包裝，已驗證
  作者：`scripts/telemetry/validate_nexus_telemetry_pack.py`，必鬚髮布
  與配置增量證據一起，以便操作員可以重放確切的
  B4期間使用的文物。
- Iroha 2 個捆綁包保持無通道：現在使用 `nexus.enabled = false` 進行配置
  拒絕通道/數據空間/路由覆蓋，除非啟用 Nexus 配置文件
  (`--sora`)，因此從單通道模板中剝離 `nexus.*` 部分。
- 保持治理投票日誌 (GOV-2026-03-19) 與跟踪器和
  此註釋以便將來的投票可以復制格式而無需重新發現
  批准儀式。

## 啟動排練後續行動

- `docs/source/runbooks/nexus_multilane_rehearsal.md` 捕獲金絲雀計劃，
  參與者名冊和回滾步驟；每當車道刷新運行手冊
  拓撲或遙測導出器發生變化。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 列出了所有文物
  在 4 月 9 日排練期間進行了檢查，現在帶有第二季度的準備筆記/議程。
  將未來的排練附加到同一個跟踪器，而不是一次性開放
  跟踪器保持證據的單調性。
- 發布 OTLP 收集器片段和 Grafana 導出（請參閱 `docs/source/telemetry.md`）
  每當出口商批次指導發生變化時；第一季度更新提高了
  批量大小為 256 個樣本，以防止出現餘量警報。
- 多通道 CI/測試證據現在存在於
  `integration_tests/tests/nexus/multilane_pipeline.rs` 並運行在
  `Nexus Multilane Pipeline` 工作流程
  (`.github/workflows/integration_tests_multilane.yml`)，取代已退役的
  `pytests/nexus/test_multilane_pipeline.py` 參考；保留哈希值
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) 同步
  刷新排練包時使用跟踪器。

## 運行時通道生命週期

- 運行時通道生命週期計劃現在驗證數據空間綁定並在以下情況下中止
  Kura/分層存儲協調失敗，目錄保持不變。的
  幫助者修剪已退役通道的緩存通道中繼，以便合併賬本合成
  不重複使用過時的證明。
- 通過 Nexus 配置/生命週期幫助程序應用計劃（`State::apply_lane_lifecycle`、
  `Queue::apply_lane_lifecycle`) 無需重新啟動即可添加/刪除通道；路由，
  TEU 快照和艙單註冊表會在計劃成功後自動重新加載。
- 操作員指南：當計劃失敗時，檢查是否丟失數據空間或存儲
  無法創建的根（分層冷根/Kura Lane 目錄）。修復
  返迴路徑並重試；成功的計劃重新發射車道/數據空間遙測數據
  diff，以便儀表板反映新的拓撲。

## NPoS 遙測和背壓證據

PhaseB 的啟動排練回顧要求確定性遙測捕獲
證明 NPoS 起搏器和八卦層保持在其背壓範圍內
限制。集成線束位於
`integration_tests/tests/sumeragi_npos_performance.rs` 練習那些
場景並發出 JSON 摘要 (`sumeragi_baseline_summary::<scenario>::…`)
每當新指標落地時。使用以下命令在本地運行它：

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

設置 `SUMERAGI_NPOS_STRESS_PEERS`、`SUMERAGI_NPOS_STRESS_COLLECTORS_K`，或
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` 探索更高應力的拓撲；的
默認鏡像 B4 中使用的 1s/`k=3` 收集器配置文件。

|場景/測試 |覆蓋範圍|關鍵遙測|
| ---| ---| ---|
| `npos_baseline_1s_k3_captures_metrics` |在序列化證據包之前，使用排練塊時間來阻止 12 輪，以記錄 EMA 延遲包絡、隊列深度和冗餘發送量規。 | `sumeragi_phase_latency_ema_ms`、`sumeragi_collectors_k`、`sumeragi_redundant_send_r`、`sumeragi_bg_post_queue_depth*`。 |
| `npos_queue_backpressure_triggers_metrics` |淹沒事務隊列以確保准入延遲確定性地啟動，並且隊列導出容量/飽和計數器。 | `sumeragi_tx_queue_depth`、`sumeragi_tx_queue_capacity`、`sumeragi_tx_queue_saturated`、`sumeragi_pacemaker_backpressure_deferrals_total`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_pacemaker_jitter_within_band` |對起搏器抖動進行採樣並查看超時，直到證明已強制執行配置的 ±125‰ 頻帶。 | `sumeragi_pacemaker_jitter_ms`、`sumeragi_pacemaker_view_timeout_target_ms`、`sumeragi_pacemaker_jitter_frac_permille`。 |
| `npos_rbc_store_backpressure_records_metrics` |將大型 RBC 有效負載推至軟/硬存儲限制，以顯示會話和字節計數器在不超出存儲的情況下爬升、後退和穩定。 | `sumeragi_rbc_store_pressure`、`sumeragi_rbc_store_sessions`、`sumeragi_rbc_store_bytes`、`sumeragi_rbc_backpressure_deferrals_total`。 |
| `npos_redundant_send_retries_update_metrics` |強制重新傳輸，以便冗餘發送比率計量器和目標收集器計數器前進，證明復古請求的遙測是端到端有線的。 | `sumeragi_collectors_targeted_current`、`sumeragi_redundant_sends_total`。 |
| `npos_rbc_chunk_loss_fault_reports_backlog` |刪除確定性間隔的塊以驗證積壓監視器會引發故障，而不是默默地耗盡有效負載。 | `sumeragi_rbc_backlog_sessions_pending`、`sumeragi_rbc_backlog_chunks_total`、`sumeragi_rbc_backlog_chunks_max`。 |將線束打印的 JSON 行與 Prometheus 刮擦一起附加
每當治理要求提供反壓證據時，就會在運行期間捕獲
警報與排練拓撲相匹配。

## 更新清單

1. 當季度滾動時，附加新的路由跟踪窗口並淘汰舊的窗口。
2. 在每次 Alertmanager 跟進後更新緩解表，即使
   操作是關閉票證。
3. 當配置增量發生變化時，更新跟踪器、此註釋和遙測數據
   將摘要列表打包在同一拉取請求中。
4. 在此處鏈接任何新的排練/遙測工件，以便了解未來的路線圖狀態
   更新可以引用單個文檔，而不是分散的臨時註釋。

## 證據索引

|資產|地點 |筆記|
|--------|----------|--------|
|路由跟踪審計報告（2026 年第一季度）| `docs/source/nexus_routed_trace_audit_report_2026q1.md` | PhaseB1 證據的規範來源；鏡像為 `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` 下的門戶。 |
|配置增量跟踪器 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |包含 TRACE-CONFIG-DELTA 差異摘要、審閱者姓名縮寫和 GOV-2026-03-19 投票日誌。 |
|遙測修復計劃| `docs/source/nexus_telemetry_remediation_plan.md` |記錄與 B2 相關的警報包、OTLP 批量大小和出口預算護欄。 |
|多車道排練跟踪器 | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` |列出 4 月 9 日的排練工件、驗證器清單/摘要、第二季度準備筆記/議程和回滾證據。 |
|遙測包清單/摘要（最新）| `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) |記錄插槽範圍 912–936、種子 `NEXUS-REH-2026Q2` 以及治理包的工件哈希值。 |
| TLS 配置文件清單 | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) |在第二季度重新運行期間捕獲的已批准 TLS 配置文件的哈希值；在路由跟踪附錄中引用。 |
|跟踪多車道金絲雀議程 | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` |第二季度排練的規劃說明（窗口、時段範圍、工作負載種子、操作所有者）。 |
|啟動排練手冊| `docs/source/runbooks/nexus_multilane_rehearsal.md` |暫存→執行→回滾的操作清單；當車道拓撲或出口商指導發生變化時更新。 |
|遙測包驗證器 | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 Retro 引用的 CLI；每當包發生變化時，存檔摘要都會與跟踪器一起進行。 |
|多車道回歸 | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` |證明 `nexus.enabled = true` 的多通道配置，保留 Sora 目錄哈希，並在發布工件摘要之前通過 `ConfigLaneRouter` 提供通道本地 Kura/合併日誌路徑 (`blocks/lane_{id:03}_{slug}`)。 |