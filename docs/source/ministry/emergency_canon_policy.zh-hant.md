---
lang: zh-hant
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# 緊急佳能和 TTL 政策 (MINFO-6a)

路線圖參考：**MINFO-6a — 緊急規範和 TTL 政策**。

本文檔定義了現在在 Torii 和 CLI 中提供的拒絕列表分層規則、TTL 實施和治理義務。運營商在發布新條目或調用緊急規則之前必須遵守這些規則。

## 等級定義

|等級 |默認 TTL |審查窗口 |要求|
|------|-------------|----------------|----------------|
|標準| 180 天 (`torii.sorafs_gateway.denylist.standard_ttl`) |不適用 |必須提供 `issued_at`。 `expires_at` 可以省略； Torii 默認為 `issued_at + standard_ttl` 並拒絕更長的窗口。 |
|緊急| 30 天 (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 天 (`torii.sorafs_gateway.denylist.emergency_review_window`) |需要引用預先批准的規範的非空 `emergency_canon` 標籤（例如 `csam-hotline`）。 `issued_at` + `expires_at` 必須在 30 天的窗口內提交，並且審查證據必須引用自動生成的截止日期 (`issued_at + review_window`)。 |
|永久|無有效期 |不適用 |保留給絕大多數治理決策。參賽作品必須引用非空 `governance_reference`（投票 ID、宣言哈希等）。 `expires_at` 被拒絕。 |

默認值仍然可以通過 `torii.sorafs_gateway.denylist.*` 進行配置，`iroha_cli` 鏡像限制以在 Torii 重新加載文件之前捕獲無效條目。

## 工作流程

1. **準備元數據：** 在每個 JSON 條目 (`docs/source/sorafs_gateway_denylist_sample.json`) 中包含 `policy_tier`、`issued_at`、`expires_at`（如果適用）和 `emergency_canon`/`governance_reference`。
2. **本地驗證：** 運行 `iroha app sorafs gateway lint-denylist --path <denylist.json>`，以便 CLI 在提交或固定文件之前強制執行特定於層的 TTL 和必填字段。
3. **發布證據：** 將條目中引用的規範 ID 或治理參考附加到 GAR 案例包（議程包、公投記錄等），以便審計員可以追踪決策。
4. **查看緊急條目：** 緊急規則會在 30 天內自動過期。運營商必須在 7 天的時間內完成事後審查，並將結果記錄在部門跟踪器/SoraFS 證據存儲中。
5. **重新加載 Torii：** 驗證後，通過 `torii.sorafs_gateway.denylist.path` 部署拒絕列表路徑並重新啟動/重新加載 Torii；運行時在允許條目之前強制執行相同的限制。

## 工具和參考

- 運行時策略實施位於 `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) 中，加載程序現在在解析 `torii.sorafs_gateway.denylist.*` 輸入時應用層元數據。
- CLI 驗證鏡像 `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) 內的運行時語義。當 TTL 超過配置的窗口或缺少強制性規範/管理參考時，linter 會失敗。
- 配置旋鈕在 `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) 下定義，因此如果治理批准不同的界限，操作員可以調整 TTL/審查截止日期。
- 公共樣本拒絕列表 (`docs/source/sorafs_gateway_denylist_sample.json`) 現在說明了所有三個層，應用作新條目的規範模板。這些護欄通過編纂緊急規範列表、防止無限制的 TTL 以及強制永久區塊的明確治理證據來滿足路線圖項目 **MINFO-6a** 的要求。

## 註冊自動化和證據導出

緊急規範批准必須產生確定性的註冊表快照和
Torii 加載拒絕列表之前的 diff 包。下的工具
`xtask/src/sorafs.rs` 加上 CI 線束 `ci/check_sorafs_gateway_denylist.sh`
覆蓋整個工作流程。

### 規範包生成

1. 在工作中暫存原始條目（通常是由治理審查的文件）
   目錄。
2. 通過以下方式規範化並密封 JSON：
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   該命令發出一個簽名友好的 `.json` 捆綁包，即 Norito `.to`
   信封，以及治理審核者期望的 Merkle-root 文本文件。
   將目錄存儲在 `artifacts/ministry/denylist_registry/` 下（或您的
   選擇證據桶）所以 `scripts/ministry/transparency_release.py` 可以
   稍後使用 `--artifact denylist_bundle=<path>` 獲取。
3. 在推送之前，將生成的 `checksums.sha256` 與捆綁包放在一起
   至 SoraFS/GAR。 CI 的 `ci/check_sorafs_gateway_denylist.sh` 執行相同的操作
   `pack` 幫助程序針對樣本拒絕名單，以保證工具正常運行
   每次發布。

### 差異 + 審核包

1. 使用以下命令將新包與之前的生產快照進行比較
   xtask 差異助手：
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON 報告列出了所有添加/刪除並反映了證據
   `MinistryDenylistChangeV1` 消耗的結構（由
   `docs/source/sorafs_gateway_self_cert.md` 和合規計劃）。
2. 將 `denylist_diff.json` 附加到每個佳能請求（它證明有多少
   條目被觸及，哪一層發生了變化，以及哪些證據哈希映射到了
   規範束）。
3. 當自動生成差異（CI 或發布管道）時，導出
   `denylist_diff.json` 路徑通過 `--artifact denylist_diff=<path>` 所以
   透明度清單將其與經過淨化的指標一起記錄。相同的 CI
   幫助程序接受運行 CLI 摘要步驟的 `--evidence-out <path>` 並
   將生成的 JSON 複製到請求的位置以供以後發布。

### 發布和透明度1. 將 pack + diff 工件放入季度透明度目錄中
   （`artifacts/ministry/transparency/<YYYY-Q>/denylist/`）。透明度
   然後發布助手可以包含它們：
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. 在季度報告中引用生成的bundle/diff
   (`docs/source/ministry/reports/<YYYY-Q>.md`) 並將相同的路徑附加到
   GAR 投票包，以便審計員可以重播證據線索，而無需訪問
   內部 CI。 `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   現在工件/部委/denylist_registry//denylist_evidence.json`
   執行 pack/diff/evidence 試運行（調用 `iroha_cli app sorafs gateway
   證據`在引擎蓋下），這樣自動化就可以將摘要與
   規範束。
3. 發布後，通過以下方式錨定治理有效負載
   `cargo xtask ministry-transparency anchor`（由自動調用
   `transparency_release.py`（當提供 `--governance-dir` 時），因此
   拒絕列表註冊表摘要與透明度出現在同一 DAG 樹中
   釋放。

遵循此流程將關閉“註冊自動化和證據導出”
`roadmap.md:450` 中提出的間隙，並確保每個緊急規範
決策帶有可重現的工件、JSON 差異和透明度日誌
條目。

### TTL 和佳能證據助手

生成捆綁包/差異對後，運行 CLI 證據幫助程序來捕獲
治理要求的 TTL 摘要和緊急審查截止日期：

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

該命令對源 JSON 進行哈希處理，驗證每個條目，並發出一個緊湊的
摘要包含：

- 每個 `kind` 和每個保單層（最早/最新）的條目總數
  觀察到的時間戳。
- `emergency_reviews[]` 列表，枚舉每個緊急規範及其
  描述符、有效過期時間、最大允許 TTL 以及計算得出的
  `review_due_by` 截止日期。

將 `denylist_evidence.json` 與打包的捆綁包/差異一起附上，以便審核員可以
無需重新運行 CLI 即可確認 TTL 合規性。已經產生的 CI 職位
捆綁包可以調用助手並發布證據工件（例如通過
調用 `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`)，確保
每個佳能請求都會有一致的摘要。

### Merkle 註冊證據

MINFO-6 中引入的 Merkle 註冊表要求運營商發布
根證明和每個條目的證明以及 TTL 摘要。跑步後立即
證據助手，捕獲 Merkle 文物：

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```快照 JSON 記錄了 BLAKE3 Merkle 根、葉子計數和每個
描述符/哈希對，以便 GAR 投票可以引用經過哈希處理的確切樹。
提供 `--norito-out` 將 `.to` 工件與 JSON 一起存儲，讓
網關直接通過 Norito 獲取註冊表項，無需抓取
標準輸出。 `merkle proof` 發出方向位和同級哈希值
從零開始的條目索引，可以輕鬆地為每個條目附加包含證明
GAR 備忘錄中引用的緊急規範 — 可選的 Norito 副本保留了證據
準備好在賬本上分發。接下來存儲 JSON 和 Norito 工件
TTL 摘要和 diff 捆綁包，以便透明發布和治理
錨點引用相同的根。