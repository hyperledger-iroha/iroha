<!-- Auto-generated stub for Chinese (Traditional) (zh-hant) translation. Replace this content with the full translation. -->

---
lang: zh-hant
direction: ltr
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus 跨資料空間本地網路證明

此 Runbook 執行 Nexus 整合證明：

- 啟動具有兩個受限私有資料空間的 4 對等本地網路（`ds1`、`ds2`），
- 將帳戶流量路由到每個資料空間，
- 在每個資料空間中建立資產，
- 跨資料空間雙向執行原子交換結算，
- 透過提交資金不足的分支並檢查餘額保持不變來證明回滾語義。

規格測試是：
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`。

## 快速運行

使用儲存庫根目錄中的包裝器腳本：

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

預設行為：

- 僅執行跨資料空間證明測試，
- 集 `NORITO_SKIP_BINDINGS_SYNC=1`，
- 集 `IROHA_TEST_SKIP_BUILD=1`，
- 使用 `--test-threads=1`，
- 通過 `--nocapture`。

## 有用的選項

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` 保留臨時對等目錄 (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) 用於取證。
- `--all-nexus` 運行 `mod nexus::`（完整的 Nexus 整合子集），而不僅僅是驗證測試。

## CI 門

CI 助手：

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

訂定目標：

```bash
make check-nexus-cross-dataspace
```

該閘執行確定性證明包裝器，並且如果跨資料空間原子
交換場景回歸。

## 手動等效指令

針對性的驗證測試：

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

完整 Nexus 子集：

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## 預期的證據訊號- 測試通過。
- 對於故意失敗的資金不足的和解分支，會出現一個預期警告：
  `settlement leg requires 10000 but only ... is available`。
- 最終餘額斷言在以下情況後成功：
  - 成功的遠期互換，
  - 成功的反向交換，
  - 資金不足的掉期失敗（回溯未改變的餘額）。

## 目前驗證快照

截至 **2026 年 2 月 19 日**，此工作流程已通過：

- 針對性測試：`1 passed; 0 failed`，
- 完整 Nexus 子集：`24 passed; 0 failed`。