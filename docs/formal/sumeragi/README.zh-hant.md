<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e89f83a4ce35b7cab8d3bfcee27eafb761f6a281c445a7cae13ae9d228760fe7
source_last_modified: "2026-04-30T20:10:10.884040+00:00"
translation_last_reviewed: 2026-05-01
translator: machine-google-reviewed
---

# Sumeragi 正式模型 (TLA+ / Apalache)

該目錄包含 Sumeragi 安全性和活性的有界正式模型。

## 範圍

`Sumeragi.tla` 捕獲提交路徑：
- 階段進展（`Propose`、`Prepare`、`CommitVote`、`NewView`、`Committed`），
- 投票和法定人數門檻（`CommitQuorum`、`ViewQuorum`），
- NPoS 式提交保護的加權權益法定人數 (`StakeQuorum`)，
- RBC 因果關係 (`Init -> Chunk -> Ready -> Deliver`) 以及標題/摘要證據，
- 商品及服務稅和對誠實進步行動的弱公平假設。

`SumeragiFrontierRecovery.tla` 捕捉了圍繞一期的專注 Taira 懸掛課程
待處理的連續邊界區塊：
- 低於法定人數或達到法定人數的提交投票證據，
- 投票隊列積壓和本地流失，
- 遺失與本地有效負載狀態，
- 新的與陳舊的前沿恢復所有權，
- 仲裁重新安排標記/視窗節奏，
- 可以鞏固當地前沿的未來前沿/新觀點證據，
- 確定性 GST 後提交、重傳、有界視圖旋轉以及
  零證據下降結果。

兩種模型都有意抽象化有線格式、ECDSA/簽名
驗證和完整的網路詳細資訊。

## 文件- `Sumeragi.tla`：協定模型與屬性。
- `Sumeragi_fast.cfg`：較小的 CI 友善參數集。
- `Sumeragi_deep.cfg`：更大的應力參數集。
- `SumeragiFrontierRecovery.tla`：重點邊境恢復模型。
- `SumeragiFrontierRecovery_fast.cfg`：較小的 CI 友善前緣參數集。
- `SumeragiFrontierRecovery_deep.cfg`：更大的前緣積壓/視窗/視圖綁定集。
- `SumeragiFrontierRecovery_wide.cfg`：手動更寬邊界設定。
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`：預期失敗陳舊所有者突變。
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`：預期失敗投票隊列突變。

## 屬性

不變量：
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

時間屬性：
- `EventuallyCommit` (`[] (gst => <> committed)`)，採用 GST 後公平性編碼
  在 `Next` 中運作（啟用逾時/故障搶佔防護）
  進展行動）。這使得模型可以透過 Apalache 0.52.x 進行檢查，
  不支援檢查時間屬性內的 `WF_` 公平運算子。

邊界恢復不變量：
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`，排除終端
  GST 後狀態，其中 `pending /\ voteBacked /\ ~committed` 沒有恢復，
  提交、重傳、旋轉或有界丟棄轉換。邊界恢復時間屬性：
- `PostGstVoteBackedFrontierEventuallyResolves`：GST之後，每個未解決的問題
  投票支持的待定邊界狀態最終達到提交、有效負載
  恢復、仲裁重傳、未來前緣錨定或有界視圖
  旋轉。
- `RecoveredPayloadEventuallyAdvances`：一個投票支持的邊境國家，
  恢復後的有效負載不能在沒有提交的情況下永遠保持掛起狀態，
  重傳、重新錨定或輪換。
- `QuorumRetransmitEventuallyLeavesPending`：一旦仲裁重新傳輸已觸發
  對於投票支持的邊境州，待處理的包裝器最終必須清除。
- `FutureFrontierEvidenceEventuallyReanchors`：後來的前沿/新觀點證據
  必須清除掛起的包裝器或作為前緣錨點使用。

## 假設圖

邊界模型有意是有限的。這些是實現
它抽象的表面：|模型概念|實作面|
| --- | --- |
| `pending`、`contiguous`、`payloadState` | `PendingBlock` 處理和 `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs` 中的本地有效負載檢查，以及 Docker/reatedCated。 |
| `commitVotes`，`queuedVotes` |由 `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` 和 `reschedule_ignores_quorum_timeout_vote_queue_backlog` 在 `crates/iroha_core/src/sumeragi/main_loop/tests.rs` 中執行提交計票和投票入口門控。 |
| `recoveryOwner` |活動/過時前沿所有者狀態在 `frontier_slot_has_active_owner_state_for_view(...)` 中，過時所有者產量在 `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)` 中，並取代清理在 `drop_superseded_contiguous_frontier_owner_state(...)` 中。 |
| `quorumRescheduleArmed`，`quorumWindowAge` | `reschedule_stale_pending_blocks_with_now(...)` 中投票支持的法定人數重新安排節奏；回歸覆蓋範圍包括 `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`。 |
| `payloadRecovered` | `request_frontier_owner_body_repair(...)`、`handle_frontier_body_gap_with_topology(...)` 和 `stale_frontier_rbc_repair_is_actionable(...)` 中的精確前沿身體修復和陳舊紅血球修復入院。 |
| `quorumRetransmitted`、`rotated` |仲裁重傳目標選擇 `rebroadcast_pending_block_updates(...)` 以及 `reschedule_stale_pending_blocks_with_now(...)` 中的確定性視圖變更呼叫。 |
| `futureFrontierEvidence` | `on_pacemaker_propose_ready(...)` 中的未來新視圖/更高前沿法定人數證據，由 `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists` 涵蓋。 |

## 運行

從儲存庫根目錄：

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
```

運行程序為每種模式設定顯式 Apalache `--length`：|模式|長度 |預期用途 |
| --- | ---: | --- |
| `fast` | 10 | 10 CI 提交路徑檢查 |
| `deep` | 10 | 10更大的提交路徑檢查 |
| `frontier-fast` | 10 | 10 CI邊境檢查|
| `frontier-deep` | 12 | 12更大的邊境檢查|
| `frontier-wide` | 14 | 14手動/每晚邊境壓力檢查 |

`APALACHE_LENGTH=<n>` 在本地探索時會覆寫每個模式的預設值
反例或擴大有界證明。

### 可重現的本機設定（不需要 Docker）

安裝此儲存庫使用的固定本機 Apalache 工具鏈：

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

運行程式會自動偵測此安裝：
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`。
安裝後，`ci/check_sumeragi_formal.sh` 應該可以在沒有額外環境變數的情況下工作：

```bash
bash ci/check_sumeragi_formal.sh
```

預期失敗突變有意位於正常 CI 之外。他們應該
在 Apalache 下失敗，但在更改模型時很有用：

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

如果 Apalache 不在 `PATH` 中，您可以：

- 將 `APALACHE_BIN` 設定為可執行路徑，或者
- 使用 Docker 後備（當 `docker` 可用時預設為啟用）：
  - 影像：`APALACHE_DOCKER_IMAGE`（預設 `ghcr.io/apalache-mc/apalache:0.52.2`）
  - 需要正在執行的 Docker 守護程式
  - 使用 `APALACHE_ALLOW_DOCKER=0` 停用回退。

範例：

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## 註釋- 此模型補充（而非取代）可執行 Rust 模型測試
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  和
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`。
- 檢查受 `.cfg` 檔案中的常數值限制。
- PR CI 透過 `.github/workflows/pr.yml` 執行這些檢查
  `ci/check_sumeragi_formal.sh`。