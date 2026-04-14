<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi 正式模型 (TLA+ / Apalache)

該目錄包含 Sumeragi 提交路徑安全性和活性的有界正式模型。

## 範圍

該模型捕獲：
- 階段進展（`Propose`、`Prepare`、`CommitVote`、`NewView`、`Committed`），
- 投票和法定人數門檻（`CommitQuorum`、`ViewQuorum`），
- NPoS 式提交保護的加權權益法定人數 (`StakeQuorum`)，
- RBC 因果關係 (`Init -> Chunk -> Ready -> Deliver`) 以及標題/摘要證據，
- 商品及服務稅和對誠實進步行動的弱公平假設。

它有意抽像出線路格式、簽名和完整的網路細節。

## 文件

- `Sumeragi.tla`：協定模型與屬性。
- `Sumeragi_fast.cfg`：較小的 CI 友善參數集。
- `Sumeragi_deep.cfg`：更大的應力參數集。

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

## 運行

從儲存庫根目錄：

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### 可重現的本機設定（不需要 Docker）安裝此儲存庫使用的固定本機 Apalache 工具鏈：

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

運行程式會自動偵測此安裝：
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`。
安裝後，`ci/check_sumeragi_formal.sh` 應該可以在沒有額外環境變數的情況下工作：

```bash
bash ci/check_sumeragi_formal.sh
```

如果 Apalache 不在 `PATH` 中，您可以：

- 將 `APALACHE_BIN` 設定為可執行路徑，或者
- 使用 Docker 後備（當 `docker` 可用時預設為啟用）：
  - 影像：`APALACHE_DOCKER_IMAGE`（預設 `ghcr.io/apalache-mc/apalache:latest`）
  - 需執行 Docker 守護程式
  - 使用 `APALACHE_ALLOW_DOCKER=0` 停用回退。

範例：

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## 註釋

- 此模型補充（而非取代）可執行 Rust 模型測試
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  和
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`。
- 檢查受 `.cfg` 檔案中的常數值限制。
- PR CI 透過 `.github/workflows/pr.yml` 執行這些檢查
  `ci/check_sumeragi_formal.sh`。