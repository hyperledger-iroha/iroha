---
lang: zh-hant
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 環境 → 配置遷移跟踪器

該跟踪器總結了出現的面向生產的環境變量切換
通過 `docs/source/agents/env_var_inventory.{json,md}` 和預期的遷移
`iroha_config` 的路徑（或顯式僅限開發/測試範圍）。


注意：當新的 **生產** 環境時，`ci/check_env_config_surface.sh` 現在會失敗
墊片相對於 `AGENTS_BASE_REF` 出現，除非 `ENV_CONFIG_GUARD_ALLOW=1` 是
設置；在使用覆蓋之前，在此處記錄有意添加的內容。

## 已完成遷移- **IVM ABI 選擇退出** — 刪除了 `IVM_ALLOW_NON_V1_ABI`；編譯器現在拒絕
  非 v1 ABI 無條件地通過單元測試來保護錯誤路徑。
- **IVM 調試橫幅環境墊片** — 刪除了 `IVM_SUPPRESS_BANNER` 環境選擇退出；
  橫幅抑制仍然可以通過編程設置器實現。
- **IVM 緩存/大小調整** — 線程緩存/驗證器/GPU 大小調整
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`）並刪除了運行時環境墊片。主持人現在致電
  `ivm::ivm_cache::configure_limits` 和 `ivm::zk::set_prover_threads`，測試使用
  `CacheLimitsGuard` 而不是 env 覆蓋。
- **連接隊列根** — 添加了 `connect.queue.root`（默認值：
  `~/.iroha/connect`）到客戶端配置並通過 CLI 進行線程化，
  JS 診斷。 JS 幫助程序解析配置（或顯式 `rootDir`）並
  僅通過 `allowEnvOverride` 在開發/測試中尊重 `IROHA_CONNECT_QUEUE_ROOT`；
  模板記錄了旋鈕，因此操作員不再需要環境覆蓋。
- **Izanami 網絡選擇加入** — 添加了顯式 `allow_net` CLI/config 標誌
  Izanami 混沌工具；現在運行需要 `allow_net=true`/`--allow-net` 和
- **IVM 橫幅蜂鳴聲** — 將 `IROHA_BEEP` env shim 替換為配置驅動
  `ivm.banner.{show,beep}` 切換（默認值：true/true）。啟動橫幅/蜂鳴聲
  接線現在僅在生產中讀取配置；開發/測試版本仍然很榮幸
  手動切換的環境覆蓋。
- **DA 假脫機覆蓋（僅測試）** — `IROHA_DA_SPOOL_DIR` 覆蓋現在
  被 `cfg(test)` 助手圍起來；生產代碼始終來源線軸
  配置中的路徑。
- **加密內在函數** — 已替換 `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` 帶配置驅動
  `crypto.sm_intrinsics` 策略 (`auto`/`force-enable`/`force-disable`) 和
  移除了 `IROHA_SM_OPENSSL_PREVIEW` 防護裝置。主機應用該策略的位置為
  啟動、工作台/測試可以通過 `CRYPTO_SM_INTRINSICS` 和 OpenSSL 選擇加入
  預覽現在僅尊重配置標誌。
  Izanami 已經需要 `--allow-net`/persisted 配置，測試現在依賴於
  該旋鈕而不是環境環境切換。
- **FastPQ GPU 調整** — 添加了 `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  配置旋鈕（默認值：`None`/`None`/`false`/`false`/`false`）並通過 CLI 解析將它們線程化
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` 墊片現在充當開發/測試後備，並且
  配置加載後將被忽略（即使配置未設置它們）；文檔/庫存是
  刷新以標記遷移。 【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  （`IVM_DECODE_TRACE`、`IVM_DEBUG_WSV`、`IVM_DEBUG_COMPACT`、`IVM_DEBUG_INVALID`、
  `IVM_DEBUG_REGALLOC`、`IVM_DEBUG_METAL_ENUM`、`IVM_DEBUG_METAL_SELFTEST`、
  `IVM_FORCE_METAL_ENUM`、`IVM_FORCE_METAL_SELFTEST_FAIL`、`IVM_FORCE_CUDA_SELFTEST_FAIL`、
  `IVM_DISABLE_METAL`、`IVM_DISABLE_CUDA`）現在通過共享的調試/測試版本進行門控
  幫助器，以便生產二進製文件忽略它們，同時保留用於本地診斷的旋鈕。環境
  重新生成了庫存以反映僅限開發/測試的範圍。- **FASTPQ 夾具更新** — `FASTPQ_UPDATE_FIXTURES` 現在僅出現在 FASTPQ 集成中
  測試；生產源不再讀取環境切換，庫存僅反映測試
  範圍。
- **清單刷新 + 範圍檢測** — env 庫存工具現在將 `build.rs` 文件標記為
  構建範圍並跟踪 `#[cfg(test)]`/集成線束模塊，以便僅測試切換（例如，
  `IROHA_TEST_*`、`IROHA_RUN_IGNORED`）和 CUDA 構建標誌顯示在生產計數之外。
  2025 年 12 月 7 日重新生成了清單（518 個引用/144 個變量），以保持環境配置保護差異為綠色。
- **P2P 拓撲環境 shim 釋放防護** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` 現在觸發確定性
  發布版本中的啟動錯誤（僅在調試/測試中發出警告），因此生產節點僅依賴於
  `network.peer_gossip_period_ms`。重新生成了環境庫存以反映守衛和
  更新後的分類器現在將 `cfg!` 保護的切換範圍限定為調試/測試。

## 高優先級遷移（生產路徑）

- _無（使用 cfg!/debug 檢測刷新清單；P2P shim 強化後 env-config 保護綠色）。 _

## 僅開發/測試切換到圍欄

- 當前掃描（2025 年 12 月 7 日）：僅構建 CUDA 標誌 (`IVM_CUDA_*`) 的範圍為 `build` 和
  線束開關（`IROHA_TEST_*`、`IROHA_RUN_IGNORED`、`IROHA_SKIP_BIND_CHECKS`）現已註冊為
  庫存中的 `test`/`debug`（包括 `cfg!` 保護墊片）。不需要額外的圍欄；
  當墊片是臨時的時，將未來添加的內容保留在帶有 TODO 標記的 `cfg(test)`/僅限工作台的幫助程序後面。

## 構建時環境（保持原樣）

- 貨物/功能環境（`CARGO_*`、`OUT_DIR`、`DOCS_RS`、`PROFILE`、`CUDA_HOME`、
  `CUDA_PATH`、`JSONSTAGE1_CUDA_ARCH`、`FASTPQ_SKIP_GPU_BUILD` 等）保留
  構建腳本問題並且超出了運行時配置遷移的範圍。

## 下一步行動

1) 在配置表面更新後運行 `make check-env-config-surface` 以捕獲新的生產環境墊片
   及早分配子系統所有者/ETA。  
2）每次掃描後刷新庫存（`make check-env-config-surface`）
   跟踪器與新的護欄保持對齊，並且環境配置防護差異保持無噪音。