---
lang: zh-hant
direction: ltr
source: docs/source/config/acceleration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 03275f401b49b62d8ccb361358235e5964b1ca791a68dcada0fd763bb6a4941b
source_last_modified: "2026-01-31T19:25:45.072378+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## 加速和 Norito 啟發式參考

`iroha_config` 中的 `[accel]` 塊穿過
`crates/irohad/src/main.rs:1895` 轉換為 `ivm::set_acceleration_config`。各位主持人
在實例化虛擬機之前應用相同的旋鈕，因此操作員可以確定性地
選擇允許使用哪些 GPU 後端，同時保持標量/SIMD 後備可用。
Swift、Android 和 Python 綁定通過橋接層加載相同的清單，因此
記錄這些默認值可以解除 WP6-C 在硬件加速積壓中的阻礙。

### `accel`（硬件加速）

下表反映了 `docs/source/references/peer.template.toml` 和
`iroha_config::parameters::user::Acceleration`定義，暴露環境
覆蓋每個鍵的變量。

|關鍵|環境變量 |默認|描述 |
|-----|---------|---------|-------------|
| `enable_simd` | `ACCEL_ENABLE_SIMD` | `true` |啟用 SIMD/NEON/AVX 執行。當 `false` 時，VM 強制使用標量後端進行向量運算和 Merkle 哈希，以簡化確定性奇偶校驗捕獲。 |
| `enable_cuda` | `ACCEL_ENABLE_CUDA` | `true` |在編譯時啟用 CUDA 後端並且運行時通過所有黃金向量檢查。 |
| `enable_metal` | `ACCEL_ENABLE_METAL` | `true` |在 macOS 版本上啟用 Metal 後端。即使為 true，如果發生奇偶校驗不匹配，Metal 自檢仍然可以在運行時禁用後端。 |
| `max_gpus` | `ACCEL_MAX_GPUS` | `0`（自動）|限制運行時初始化的物理 GPU 數量。 `0` 表示“匹配硬件扇出”，並由 `GpuManager` 箝位。 |
| `merkle_min_leaves_gpu` | `ACCEL_MERKLE_MIN_LEAVES_GPU` | `8192` | Merkle 葉散列卸載到 GPU 之前所需的最少葉數。低於此閾值的值會在 CPU 上繼續進行哈希處理，以避免 PCIe 開銷 (`crates/ivm/src/byte_merkle_tree.rs:49`)。 |
| `merkle_min_leaves_metal` | `ACCEL_MERKLE_MIN_LEAVES_METAL` | `0`（繼承全局）| GPU 閾值的 Metal 特定覆蓋。當 `0` 時，Metal 繼承 `merkle_min_leaves_gpu`。 |
| `merkle_min_leaves_cuda` | `ACCEL_MERKLE_MIN_LEAVES_CUDA` | `0`（繼承全局）| GPU 閾值的 CUDA 特定覆蓋。當`0`時，CUDA繼承`merkle_min_leaves_gpu`。 |
| `prefer_cpu_sha2_max_leaves_aarch64` | `ACCEL_PREFER_CPU_SHA2_MAX_AARCH64` | `0`（內部 32768）|限制 ARMv8 SHA-2 指令應勝過 GPU 哈希的樹大小。 `0` 保留 `32_768` 葉 (`crates/ivm/src/byte_merkle_tree.rs:59`) 的編譯默認值。 |
| `prefer_cpu_sha2_max_leaves_x86` | `ACCEL_PREFER_CPU_SHA2_MAX_X86` | `0`（內部 32768）|與上面相同，但適用於使用 SHA-NI (`crates/ivm/src/byte_merkle_tree.rs:63`) 的 x86/x86_64 主機。 |

`enable_simd` 還控制 RS16 擦除編碼（Torii DA 攝取 + 工具）。禁用它以
強制標量奇偶校驗生成，同時保持硬件輸出的確定性。

配置示例：

```toml
[accel]
enable_simd = true
enable_cuda = true
enable_metal = true
max_gpus = 2
merkle_min_leaves_gpu = 12288
merkle_min_leaves_metal = 8192
merkle_min_leaves_cuda = 16384
prefer_cpu_sha2_max_leaves_aarch64 = 65536
```

最後五個鍵的零值意味著“保留編譯的默認值”。主辦方不得
設置衝突的覆蓋（例如，禁用 CUDA，同時強制僅使用 CUDA 閾值），
否則請求將被忽略，後端繼續遵循全局策略。

### 檢查運行時狀態運行 `cargo xtask acceleration-state [--format table|json]` 對應用的快照
配置以及 Metal/CUDA 運行時健康位。該命令拉取
當前 `ivm::acceleration_config`、奇偶校驗狀態和粘性錯誤字符串（如果
後端被禁用），因此操作可以將結果直接輸入奇偶校驗
儀表板或事件回顧。

```
$ cargo xtask acceleration-state
Acceleration Configuration
--------------------------
enable_simd: yes
enable_metal: yes
enable_cuda: no
max_gpus: 1
merkle_min_leaves_gpu: 8192
merkle_min_leaves_metal: 8192
merkle_min_leaves_cuda: auto
prefer_cpu_sha2_max_leaves_aarch64: auto
prefer_cpu_sha2_max_leaves_x86: auto

Backend Status
--------------
Backend Supported  Configured  Available  ParityOK  Last error
SIMD    yes        yes         yes        yes       -
Metal   yes        yes         yes        yes       -
CUDA    no         no          no         no        policy disabled (no CUDA libraries present)
```

當快照需要通過自動化攝取時，使用 `--format json`（JSON
包含表中所示的相同字段）。

`acceleration_runtime_errors()` 現在指出了 SIMD 回退到標量的原因：
`disabled by config`、`forced scalar override`、`simd unsupported on hardware` 或
`simd unavailable at runtime` 當檢測成功但執行仍然運行時
沒有向量。清除覆蓋或重新啟用策略會刪除該消息
在支持 SIMD 的主機上。

### 奇偶校驗

在僅 CPU 和加速之間翻轉 `AccelerationConfig` 以證明確定性結果。
`poseidon_instructions_match_across_acceleration_configs` 回歸運行
Poseidon2/6 操作碼兩次 — 首先將 `enable_cuda`/`enable_metal` 設置為 `false`，然後
兩者都啟用，並在存在 GPU 時斷言相同的輸出以及 CUDA 奇偶校驗。 【crates/ivm/tests/crypto.rs:100】
捕獲 `acceleration_runtime_status()` 並記錄後端是否運行
已在實驗室日誌中配置/可用。

```rust
let baseline = ivm::acceleration_config();
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: false,
    enable_metal: false,
    ..baseline
});
// run CPU-only parity workload
ivm::set_acceleration_config(AccelerationConfig {
    enable_cuda: true,
    enable_metal: true,
    ..baseline
});

When isolating SIMD/NEON differences, set `enable_simd = false` to force scalar
execution. The `disabling_simd_forces_scalar_and_preserves_outputs` regression
forces the scalar backend and asserts vector ops stay bit-identical to the
SIMD-enabled baseline on the same host while surfacing the `simd` status/error
fields via `acceleration_runtime_status`/`acceleration_runtime_errors`.【crates/ivm/tests/acceleration_simd.rs:9】
```

### GPU 默認值和啟發式

默認情況下，`MerkleTree` GPU 卸載在 `8192` 離開時啟動，CPU SHA-2
每個架構的首選項閾值保持在 `32_768` 葉。當 CUDA 和
Metal可用或已被健康檢查禁用，VM自動下降
回到 SIMD/標量哈希，上面的數字不會影響確定性。

`max_gpus` 限制輸入 `GpuManager` 的池大小。將 `max_gpus = 1` 設置為開啟
多 GPU 主機使遙測變得簡單，同時仍允許加速。運營商可以
使用此開關可為 FASTPQ 或 CUDA Poseidon 作業保留剩餘設備。

### 下一步加速目標和預算

最新的FastPQ金屬走線（`fastpq_metal_bench_20k_latest.json`，32K行×16
列，5 迭代）顯示 Poseidon 列哈希主導 ZK 工作負載：

- `poseidon_hash_columns`：CPU 平均值 **3.64s** 與 GPU 平均值 **3.55s** (1.03×)。
- `lde`：CPU 平均值 **1.75s** 與 GPU 平均值 **1.57s** (1.12×)。

IVM/Crypto 將在下一次加速掃描中針對這兩個內核。基準預算：

- 保持標量/SIMD 奇偶校驗等於或低於 CPU 均值以上，並捕獲
  `acceleration_runtime_status()` 伴隨每次運行，因此 Metal/CUDA 可用性是
  記錄預算數字。
- 一旦調諧金屬，`poseidon_hash_columns` 的目標加速≥1.3×，`lde` 的目標加速≥1.2×
  和 CUDA 內核落地，無需更改輸出或遙測標籤。

將 JSON 跟踪和 `cargo xtask acceleration-state --format json` 快照附加到
未來的實驗室運行，以便 CI/回歸可以斷言預算和後端運行狀況，同時
比較僅 CPU 與加速運行。

### Norito 啟發式（編譯時默認值）Norito 的佈局和壓縮啟發式方法存在於 `crates/norito/src/core/heuristics.rs` 中
並被編譯到每個二進製文件中。它們在運行時不可配置，但會暴露
這些輸入可幫助 SDK 和運營團隊預測 GPU 運行後 Norito 的行為方式
壓縮內核已啟用。
工作區現在構建 Norito，並且默認啟用 `gpu-compression` 功能，
所以 GPU zstd 後端被編譯進去；運行時可用性仍然取決於硬件，
幫助程序庫 (`libgpuzstd_*`/`gpuzstd_cuda.dll`) 和 `allow_gpu_compression`
配置標誌。使用 `cargo build -p gpuzstd_metal --release` 構建 Metal 助手並
將 `libgpuzstd_metal.dylib` 放在加載程序路徑上。當前 Metal 助手運行 GPU
匹配查找/序列生成並使用板條箱內確定性 zstd 框架
主機上的編碼器（Huffman/FSE+框架組件）；解碼使用板條箱內的幀
對不支持的幀使用 CPU zstd 回退的解碼器，直到連接 GPU 塊解碼。

|領域 |默認 |目的|
|--------|---------|---------|
| `min_compress_bytes_cpu` | `256` 字節 |在此之下，有效負載完全跳過 zstd 以避免開銷。 |
| `min_compress_bytes_gpu` | `1_048_576` 字節 (1MiB) |當 `norito::core::hw::has_gpu_compression()` 為 true 時，等於或高於此限制的有效負載會切換到 GPU zstd。 |
| `zstd_level_small` / `zstd_level_large` | `1` / `3` |分別針對 <32KiB 和 ≥32KiB 有效負載的 CPU 壓縮級別。 |
| `zstd_level_gpu` | `1` |保守的 GPU 級別可在填充命令隊列時保持延遲一致。 |
| `large_threshold` | `32_768` 字節 | “小”和“大”CPU zstd 級別之間的大小邊界。 |
| `aos_ncb_small_n` | `64` 行 |在此行數下方，自適應編碼器會探測 AoS 和 NCB 佈局以選擇最小的有效負載。 |
| `combo_no_delta_small_n_if_empty` | `2` 行 |當 1-2 行包含空單元格時，防止啟用 u32/id 增量編碼。 |
| `combo_id_delta_min_rows` / `combo_u32_delta_min_rows` | `2` |僅當至少有兩行時，增量才會生效。 |
| `combo_enable_id_delta` / `combo_enable_u32_delta_names` / `combo_enable_u32_delta_bytes` | `true` |默認情況下，對於行為良好的輸入啟用所有增量變換。 |
| `combo_enable_name_dict` | `true` |當命中率證明內存開銷合理時，允許使用每列字典。 |
| `combo_dict_ratio_max` | `0.40` |當超過 40% 的行不同時禁用字典。 |
| `combo_dict_avg_len_min` | `8.0` |在構建字典之前要求平均字符串長度≥8（短別名保持內聯）。 |
| `combo_dict_max_entries` | `1024` |字典條目的硬上限以保證有限的內存使用。 |

這些啟發式方法使啟用 GPU 的主機與僅使用 CPU 的主機保持一致：選擇器
從不做出改變線路格式的決定，並且閾值是固定的
每個版本。當分析發現更好的收支平衡點時，Norito 會更新
規範 `Heuristics::canonical` 實現和 `docs/source/benchmarks.md` plus
`status.md` 將更改與版本證據一起記錄。GPU zstd 幫助程序強制執行相同的 `min_compress_bytes_gpu` 截止，即使
直接調用（例如通過 `norito::core::gpu_zstd::encode_all`），所以很小
無論 GPU 可用性如何，有效負載始終保留在 CPU 路徑上。

### 故障排除和奇偶校驗檢查表

- 使用 `cargo xtask acceleration-state --format json` 快照運行時狀態並保留
  輸出以及任何失敗的日誌；該報告顯示已配置/可用的後端
  加上奇偶校驗/最後一個錯誤字符串。
- 在本地重新運行加速奇偶校驗回歸以排除漂移：
  `cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  （僅運行 CPU，然後加速）。記錄運行的 `acceleration_runtime_status()`。
- 如果後端自檢失敗，請以僅 CPU 模式保持節點在線（`enable_metal =
  false`, `enable_cuda = false`) 並使用捕獲的奇偶校驗輸出打開事件
  而不是強制啟動後端。結果必須在不同模式下保持確定性。
- **CUDA 奇偶校驗煙霧（實驗室 NV 硬件）：** 運行
  `ACCEL_ENABLE_CUDA=1 cargo test -p ivm poseidon_instructions_match_across_acceleration_configs -- --nocapture`
  在 sm_8x 硬件上，捕獲 `cargo xtask acceleration-state --format json`，並附加
  基準測試工件的狀態快照（包括 GPU 型號/驅動程序）。