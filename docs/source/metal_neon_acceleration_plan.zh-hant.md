---
lang: zh-hant
direction: ltr
source: docs/source/metal_neon_acceleration_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 628eb2c7776bf818a310dd4bae51e3fc655f92e885d0cd9da7ff487fd9128102
source_last_modified: "2025-12-29T18:16:35.976997+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Metal & NEON 加速計劃 (Swift & Rust)

本文檔記錄了啟用確定性硬件的共享計劃
加速（Metal GPU + NEON/Accelerate SIMD + StrongBox 集成）
Rust 工作區和 Swift SDK。它解決了跟踪的路線圖項目
在 **硬件加速工作流 (macOS/iOS)** 下並提供交接
Rust IVM 團隊、Swift 橋所有者和遙測工具的工件。

> 最後更新: 2026-01-12  
> 所有者：IVM Performance TL，Swift SDK 負責人

## 目標

1. 通過 Metal 在 Apple 硬件上重用 Rust GPU 內核 (Poseidon/BN254/CRC64)
   計算著色器與 CPU 路徑的確定性奇偶校驗。
2. 暴露端到端加速開關 (`AccelerationConfig`) 以便 Swift 應用程序
   可以選擇 Metal/NEON/StrongBox，同時保留 ABI/奇偶校驗保證。
3. 使用 CI + 儀表板來顯示奇偶校驗/基準數據和標記
   CPU 與 GPU/SIMD 路徑之間的回歸。
4. 在 Android (AND2) 和 Swift 之間共享 StrongBox/secure-enclave 課程
   (IOS4) 保持簽名流程確定性一致。

**更新（CRC64 + Stage-1 刷新）：** CRC64 GPU 助手現已連接到具有 192KiB 默認截止值的 `norito::core::hardware_crc64`（通過 `NORITO_GPU_CRC64_MIN_BYTES` 或顯式助手路徑 `NORITO_CRC64_GPU_LIB` 覆蓋），同時保留 SIMD 和標量回退。 JSON Stage-1 轉換重新進行了基準測試（`examples/stage1_cutover` → `benchmarks/norito_stage1/cutover.csv`），將標量轉換保持在 4KiB，並將 Stage-1 GPU 默認值調整為 192KiB (`NORITO_STAGE1_GPU_MIN_BYTES`)，因此小文檔保留在 CPU 上，而大負載可以分攤 GPU 啟動成本。

## 可交付成果和所有者

|里程碑|可交付成果 |所有者 |目標|
|------------|-------------|----------|--------|
| Rust WP2-A/B |鏡像 CUDA 內核的金屬著色器接口 | IVM 性能 TL | 2026 年 2 月 |
| Rust WP2-C |金屬 BN254 奇偶校驗測試和 CI 通道 | IVM 性能 TL | 2026 年第二季度 |
|斯威夫特IOS6 |橋接開關有線 (`connect_norito_set_acceleration_config`) + SDK API + 示例 |斯威夫特大橋業主|完成（2026 年 1 月）|
|斯威夫特IOS5 |演示配置用法的示例應用程序/文檔 |斯威夫特 DX TL | 2026 年第二季度 |
|遙測|帶有加速奇偶校驗 + 基準指標的儀表板提要 | Swift 程序 PM / 遙測 | 2026 年第二季度試點數據 |
| CI | XCFramework 煙霧線束在設備池上對比 CPU 與 Metal/NEON | Swift 質量保證主管 | 2026 年第二季度 |
|保險箱 |硬件支持的簽名奇偶校驗測試（共享向量）| Android 加密 TL / Swift 安全 | 2026 年第三季度 |

## 接口和 API 合約### 鐵鏽 (`ivm::AccelerationConfig`)
- 保留現有字段（`enable_simd`、`enable_metal`、`enable_cuda`、`max_gpus`、閾值）。
- 添加顯式 Metal 預熱以避免首次使用延遲 (Rust #15875)。
- 提供返回儀表板狀態/診斷的奇偶校驗 API：
  - 例如`ivm::vector::metal_status()` -> {已啟用，奇偶校驗，最後一個錯誤}。
- 輸出基準測試指標（Merkle 樹時序、CRC 吞吐量）
  `ci/xcode-swift-parity` 的遙測掛鉤。
- Metal 主機現在加載已編譯的 `fastpq.metallib`，調度 FFT/IFFT/LDE
  和 Poseidon 內核，並且每當
  Metallib 或設備隊列不可用。

### C FFI (`connect_norito_bridge`)
- 新結構 `connect_norito_acceleration_config`（已完成）。
- Getter 覆蓋範圍現在包括 `connect_norito_get_acceleration_config`（僅配置）和 `connect_norito_get_acceleration_state`（配置 + 奇偶校驗）以鏡像 setter。
- SPM/CocoaPods 使用者的標頭註釋中的文檔結構佈局。

### 斯威夫特 (`AccelerationSettings`)
- 默認值：Metal 啟用，CUDA 禁用，閾值為零（繼承）。
- 忽略負值； `apply()` 由 `IrohaSDK` 自動調用。
- `AccelerationSettings.runtimeState()` 現在顯示 `connect_norito_get_acceleration_state`
  有效負載（配置 + Metal/CUDA 奇偶校驗狀態），以便 Swift 儀表板發出相同的遙測數據
  如 Rust (`supported/configured/available/parity`)。當以下情況時，助手返回 `nil`
  缺少橋樑以保持測試的可移植性。
- `AccelerationBackendStatus.lastError` 複製禁用/錯誤原因
  `connect_norito_get_acceleration_state` 並在字符串被釋放後釋放本機緩衝區
  實現後，移動奇偶校驗儀表板可以註釋為什麼 Metal/CUDA 被禁用
  每個主機。
- `AccelerationSettingsLoader` (`IrohaSwift/Sources/IrohaSwift/AccelerationSettingsLoader.swift`,
  現在在 `IrohaSwift/Tests/IrohaSwiftTests/AccelerationSettingsLoaderTests.swift` 下測試）
  以與 Norito 演示相同的優先級順序解析操作符清單：honor
  `NORITO_ACCEL_CONFIG_PATH`，搜索捆綁 `acceleration.{json,toml}` / `client.{json,toml}`，
  記錄所選來源，並回退到默認值。應用程序不再需要定制加載器
  鏡像 Rust `iroha_config` 表面。
- 更新示例應用程序和自述文件以顯示切換和遙測集成。

### 遙測（儀表板 + 導出器）
- 奇偶校驗源 (mobile_parity.json)：
  - `acceleration.metal/neon/strongbox` -> {啟用、奇偶校驗、perf_delta_pct}。
  - 接受 `perf_delta_pct` 基準 CPU 與 GPU 比較。
  - `acceleration.metal.disable_reason` 鏡子 `AccelerationBackendStatus.lastError`
    因此 Swift 自動化可以以與 Rust 相同的保真度來標記禁用的 GPU
    儀表板。
- CI 源 (mobile_ci.json)：
  - `acceleration_bench.metal_vs_cpu_merkle_ms` -> {CPU，金屬}
  - `acceleration_bench.neon_crc64_throughput_mb_s` -> 雙。
- 導出器必須從 Rust 基準測試或 CI 運行中獲取指標（例如，運行
  Metal/CPU microbench 作為 `ci/xcode-swift-parity` 的一部分）。### 配置旋鈕和默認值 (WP6-C)
- `AccelerationConfig` 默認值：macOS 版本上的 `enable_metal = true`、編譯 CUDA 功能時的 `enable_cuda = true`、`max_gpus = None`（無上限）。 Swift `AccelerationSettings` 包裝器通過 `connect_norito_set_acceleration_config` 繼承相同的默認值。
- Norito Merkle 啟發式（GPU 與 CPU）：`merkle_min_leaves_gpu = 8192` 支持對葉子數≥8192 的樹進行 GPU 哈希處理；除非明確設置，否則後端覆蓋（`merkle_min_leaves_metal`、`merkle_min_leaves_cuda`）默認為相同閾值。
- CPU 優先啟發式（SHA2 ISA 存在）：在 AArch64 (ARMv8 SHA2) 和 x86/x86_64 (SHA-NI) 上，CPU 路徑在 `prefer_cpu_sha2_max_leaves_* = 32_768` 離開之前保持優先狀態；高於該值時，GPU 閾值適用。這些值可通過 `AccelerationConfig` 進行配置，並且只能根據基准證據進行調整。

## 測試策略

1. **單元奇偶校驗測試 (Rust)**：確保 Metal 內核與 CPU 輸出匹配
   確定性向量；在 `cargo test -p ivm --features metal` 下運行。
   `crates/fastpq_prover/src/metal.rs` 現在提供僅限 macOS 的奇偶校驗測試
   針對標量參考練習 FFT/IFFT/LDE 和 Poseidon。
2. **Swift Smoke Harness**：擴展 IOS6 測試運行器來執行 CPU 與 Metal
   在模擬器和 StrongBox 設備上進行編碼 (Merkle/CRC64)；比較
   結果和日誌奇偶校驗狀態。
3. **CI**：更新`norito_bridge_ios.yml`（已調用`make swift-ci`）推送
   工件的加速指標；確保運行確認 Buildkite
   `ci/xcframework-smoke:<lane>:device_tag` 發佈線束更改之前的元數據，
   並在奇偶性/基準漂移上失敗。
4. **儀表板**：新字段現在在 CLI 輸出中呈現。確保出口商生產
   實時翻轉儀表板之前的數據。

## WP2-A 金屬著色器計劃（波塞冬管道）

第一個 WP2 里程碑涵蓋了 Poseidon Metal 內核的規劃工作
反映了 CUDA 的實現。該計劃將工作分成幾個核心，
主機調度和共享常量暫存，以便以後的工作可以純粹專注於
實施和測試。

### 內核範圍

1. `poseidon_permute`：排列 `state_count` 獨立狀態。每個線程
   擁有 `STATE_CHUNK`（4 個狀態）並使用以下命令運行所有 `TOTAL_ROUNDS` 迭代
   在調度時上演的線程組共享輪常量。
2. `poseidon_hash_columns`：讀取稀疏的`PoseidonColumnSlice`目錄並
   對每一列執行 Merkle 友好的哈希（與 CPU 的匹配）
   `PoseidonColumnBatch` 佈局）。它使用相同的線程組常量緩衝區
   作為置換內核，但在 `(states_per_lane * block_count)` 上循環
   輸出，以便內核可以分攤隊列提交。
3. `poseidon_trace_fused`：計算跟踪表的父/葉摘要
   在一次通過中。融合內核消耗 `PoseidonFusedArgs`，因此主機
   可以描述非連續區域和 `leaf_offset`/`parent_offset`，並且
   它與其他內核共享所有回合/MDS 表。

### 命令調度和主機合約- 每個內核調度都通過 `MetalPipelines::command_queue` 運行，這
  強制執行自適應調度程序（目標~2 ms）和隊列扇出控制
  通過 `FASTPQ_METAL_QUEUE_FANOUT` 暴露和
  `FASTPQ_METAL_COLUMN_THRESHOLD`。 `with_metal_state` 中的預熱路徑
  預先編譯所有三個 Poseidon 內核，因此第一次調度不會
  支付管道創建罰款。
- 線程組大小調整反映了現有的 Metal FFT/LDE 默認值：目標是
  每次提交 8,192 個線程，每組硬上限為 256 個線程。的
  主機可以通過以下方式降低低功耗設備的 `states_per_lane` 乘法器
  撥打環境覆蓋 (`FASTPQ_METAL_POSEIDON_STATES_PER_BATCH`
  在 WP2-B 中添加）而不修改著色器邏輯。
- 列分段遵循 FFT 已使用的相同雙緩衝池
  管道。 Poseidon 內核接受原始指針到暫存緩衝區
  並且永遠不會觸及全局堆分配，這保持了內存確定性
  與 CUDA 主機對齊。

### 共享常量

- `PoseidonSnapshot` 清單中描述
  `docs/source/fastpq/poseidon_metal_shared_constants.md` 現在是規範的
  舍入常數和 MDS 矩陣的來源。均為金屬 (`poseidon2.metal`)
  每當清單出現時，必須重新生成 CUDA (`fastpq_cuda.cu`) 內核
  變化。
- WP2-B 將添加一個小型主機加載器，在運行時讀取清單並
  將 SHA-256 發送到遙測中 (`acceleration.poseidon_constants_sha`)，因此
  奇偶校驗儀表板可以斷言著色器常量與發布的一致
  快照。
- 在預熱期間，我們將把 `TOTAL_ROUNDS x STATE_WIDTH` 常量複製到
  `MTLBuffer` 並為每個設備上傳一次。然後每個內核複製數據
  在處理其塊之前進入線程組內存，確保確定性
  即使多個命令緩衝區同時運行也能進行排序。

### 驗證掛鉤

- 單元測試（`cargo test -p fastpq_prover --features fastpq-gpu`）將增長
  哈希嵌入的著色器常量並將它們與
  在執行 GPU 夾具套件之前檢查清單的 SHA。
- 現有的內核統計信息切換（`FASTPQ_METAL_TRACE_DISPATCH`，
  `FASTPQ_METAL_QUEUE_FANOUT`，隊列深度遙測）成為必需證據
  對於 WP2 退出：每次測試運行都必須證明調度程序從未違反
  配置扇出並且融合跟踪內核將隊列保持在
  自適應窗口。
- Swift XCFramework 煙霧線束和 Rust 基準測試運行程序將啟動
  導出 `acceleration.poseidon.permute_p90_ms{cpu,metal}` 以便 WP2-D 可以繪製圖表
  Metal 與 CPU 的差異無需重新發明新的遙測源。

## WP2-B Poseidon 清單加載器和自檢奇偶校驗- `fastpq_prover::poseidon_manifest()` 現在嵌入和解析
  `artifacts/offline_poseidon/constants.ron`，計算其 SHA-256
  (`poseidon_manifest_sha256()`)，並根據 CPU 驗證快照
  在任何 GPU 工作運行之前的poseidon 表。 `build_metal_context()` 記錄
  在預熱期間進行摘要，以便遙測出口商可以發布
  `acceleration.poseidon_constants_sha`。
- 清單解析器拒絕不匹配的寬度/速率/輪數元組和
  確保清單 MDS 矩陣等於標量實現，防止
  重新生成規範表時會出現無聲漂移。
- 添加了 `crates/fastpq_prover/tests/poseidon_manifest_consistency.rs`，其中
  解析嵌入在 `poseidon2.metal` 中的 Poseidon 表並
  `fastpq_cuda.cu` 並斷言兩個內核序列化完全相同
  常量作為清單。如果有人編輯著色器/CUDA，CI 現在會失敗
  文件而不重新生成規范清單。
- 未來的奇偶校驗掛鉤 (WP2-C/D) 可以重用 `poseidon_manifest()` 來暫存
  將常量舍入到 GPU 緩衝區中並通過 Norito 公開摘要
  遙測數據。

## WP2-C BN254 金屬管道和奇偶校驗測試- **範圍和差距：** 主機調度程序、奇偶校驗工具和 `bn254_status()` 已上線，`crates/fastpq_prover/metal/kernels/bn254.metal` 現在實現 Montgomery 原語以及線程組同步的 FFT/LDE 循環。每個調度在具有每階段屏障的單個線程組內運行整個列，因此內核並行執行分階段清單。遙測現在已連接，並且調度程序覆蓋得到認可，因此我們可以使用與 Goldilocks 內核相同的證據來控制默認的部署。
- **內核要求：** ✅ 重用分階段的旋轉/陪集清單，轉換輸入/輸出一次，並執行每列線程組內的所有 radix-2 階段，因此我們不需要多調度同步。蒙哥馬利助手在 FFT/LDE 之間保持共享，因此僅循環幾何形狀發生變化。
- **主機接線：** ✅ `crates/fastpq_prover/src/metal.rs` 階段規範肢體，零填充 LDE 緩衝區，每列選擇一個線程組，並公開 `bn254_status()` 用於選通。遙測不需要額外的主機更改。
- **構建防護：** `fastpq.metallib` 附帶平舖內核，因此如果著色器漂移，CI 仍然會快速失敗。任何未來的優化都停留在遙測/功能門之後，而不是編譯時開關。
- **奇偶校驗裝置：** ✅ `bn254_parity` 測試繼續將 GPU FFT/LDE 輸出與 CPU 裝置進行比較，並且現在在 Metal 硬件上實時運行；如果出現新的內核代碼路徑，請記住篡改清單測試。
- **遙測和基準測試：** `fastpq_metal_bench` 現在發出：
  - `bn254_dispatch` 塊總結了每個調度線程組寬度、邏輯線程計數以及 FFT/LDE 單列批次的管道限制；和
  - `bn254_metrics` 塊，記錄 CPU 基線的 `acceleration.bn254_{fft,ifft,lde,poseidon}_ms` 以及運行的 GPU 後端。
  基準包裝器將兩個映射複製到每個包裝的工件中，以便 WP2-D 儀表板攝取標記的延遲/幾何，而無需對原始操作數組進行逆向工程。 `FASTPQ_METAL_THREADGROUP` 現在也適用於 BN254 FFT/LDE 調度，使旋鈕可用於性能掃描。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1448】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:3155】【scripts/fastpq/wrap_benchmark.py:1037】

## 開放問題（2027 年 5 月解決）1. **Metal資源清理：** `warm_up_metal()`復用線程本地
   `OnceCell` 現在具有冪等/回歸測試
   （`crates/ivm/src/vector.rs::warm_up_metal_reuses_cached_state` /
   `warm_up_metal_is_noop_on_non_metal_targets`)，因此應用程序生命週期轉換
   可以安全地調用預熱路徑，而不會洩漏或雙重初始化。
2. **基準基線：** 金屬通道必須保持在 CPU 的 20% 以內
   FFT/IFFT/LDE 的基線以及 Poseidon CRC/Merkle 助手的 15% 以內；
   當 `acceleration.*_perf_delta_pct > 0.20`（或缺失）時應觸發警報
   在移動奇偶校驗提要中。在 20k 跡線束中觀察到的 IFFT 回歸
   現在由 WP2-D 中提到的隊列覆蓋修復來控制。
3. **StrongBox 後備：** Swift 遵循 Android 後備劇本：
   在支持運行手冊中記錄證明失敗
   (`docs/source/sdk/swift/support_playbook.md`) 並自動切換到
   記錄的 HKDF 支持的軟件路徑以及審計日誌記錄；奇偶校驗向量
   通過現有的 OA 設備保持共享。
4. **遙測存儲：** 加速捕獲和設備池證明
   存檔在 `configs/swift/` 下（例如，
   `configs/swift/xcframework_device_pool_snapshot.json`) 和出口商
   應鏡像相同的佈局（`artifacts/swift/telemetry/acceleration/*.json`
   或 `.prom`），因此 Buildkite 註釋和門戶儀表板可以攝取
   無需臨時抓取即可饋送。

## 後續步驟（2026 年 2 月）

- [x] Rust：land Metal 主機集成 (`crates/fastpq_prover/src/metal.rs`) 和
      公開 Swift 的內核接口；文檔交接與跟踪
      斯威夫特橋樑筆記。
- [x] Swift：公開 SDK 級加速設置（2026 年 1 月完成）。
- [x] 遙測：`scripts/acceleration/export_prometheus.py` 現在轉換
      `cargo xtask acceleration-state --format json` 輸出為 Prometheus
      文本文件（帶有可選的 `--instance` 標籤），以便 CI 運行可以附加 GPU/CPU
      啟用、閾值和奇偶校驗/禁用原因直接寫入文本文件
      收藏家無需定制刮擦。
- [x] Swift QA：`scripts/acceleration/acceleration_matrix.py` 聚合多個
      加速狀態捕獲到由設備鍵入的 JSON 或 Markdown 表中
      標籤，為煙霧線束提供確定性的“CPU vs Metal/CUDA”矩陣
      與示例應用程序一起上傳。 Markdown 輸出反映了
      構建風箏證據格式，以便儀表板可以攝取相同的人工製品。
- [x] 更新 status.md 現在 `irohad` 運送了隊列/零填充導出器並且
      env/config 驗證測試涵蓋 Metal 隊列覆蓋，因此 WP2-D
      遙測+綁定附有實時證據。 【crates/irohad/src/main.rs:2664】【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【status.md:1546】

遙測/導出幫助程序命令：

```bash
# Prometheus textfile from a single capture
cargo xtask acceleration-state --format json > artifacts/acceleration_state_macos_m4.json
python3 scripts/acceleration/export_prometheus.py \
  --input artifacts/acceleration_state_macos_m4.json \
  --output artifacts/acceleration_state_macos_m4.prom \
  --instance macos-m4

# Aggregate multiple captures into a Markdown matrix
python3 scripts/acceleration/acceleration_matrix.py \
  --state macos-m4=artifacts/acceleration_state_macos_m4.json \
  --state sim-m3=artifacts/acceleration_state_sim_m3.json \
  --format markdown \
  --output artifacts/acceleration_matrix.md
```

## WP2-D 發布基準和綁定說明- **20k 行發布捕獲：** 在 macOS14 上記錄了新的 Metal 與 CPU 基準測試
  （arm64，通道平衡參數，填充 32,768 行跡線，兩列批次）和
  將 JSON 包簽入 `fastpq_metal_bench_20k_release_macos14_arm64.json`。
  該基準導出每個操作的時間加上 Poseidon 微基准證據，以便
  WP2-D 具有與新的 Metal 隊列啟發法相關的 GA 質量製品。標題
  增量（完整表位於 `docs/source/benchmarks.md` 中）：

  |運營| CPU 平均值（毫秒）|金屬平均值（毫秒）|加速|
  |----------|--------------|-----------------|---------|
  | FFT（32,768 個輸入）| 12.741 | 12.741 10.963 | 10.963 1.16× |
  | IFFT（32,768 個輸入）| 17.499 | 17.499 25.688 | 25.688 0.68× *（回歸：隊列扇出受到限制以保持確定性；需要後續調整）* |
  | LDE（262,144 個輸入）| 68.389 | 68.389 65.701 | 65.701 1.04× |
  | Poseidon 哈希列（524,288 個輸入）| 1,728.835 | 1,728.835 1,447.076 | 1,447.076 1.19× |

  每次捕獲記錄 `zero_fill` 計時（33,554,432 字節為 9.651ms）和
  `poseidon_microbench` 條目（默認通道 596.229ms 與標量 656.251ms，
  1.10× 加速），因此儀表板消費者可以區分隊列壓力和
  主要業務。
- **綁定/文檔交叉鏈接：** `docs/source/benchmarks.md` 現在引用
  發布 JSON 和重現器命令，Metal 隊列覆蓋已驗證
  通過 `iroha_config` env/manifest 測試，`irohad` 實時發布
  `fastpq_metal_queue_*` 儀表板因此儀表板可以標記 IFFT 回歸，而無需
  臨時日誌抓取。 Swift 的 `AccelerationSettings.runtimeState` 暴露了
  JSON 捆綁包中提供的相同遙測有效負載，關閉 WP2-D
  具有可重複接受基線的綁定/文檔差距。 【crates/iroha_config/tests/fastpq_queue_overrides.rs:1】【crates/irohad/src/main.rs:2664】
- **IFFT 隊列修復：** 逆 FFT 批次現在會跳過多隊列調度
  工作負載勉強達到扇出閾值（通道平衡的 16 列）
  profile），刪除上面提到的 Metal-vs-CPU 回歸，同時保留
  FFT/LDE/Poseidon 多隊列路徑上的大列工作負載。