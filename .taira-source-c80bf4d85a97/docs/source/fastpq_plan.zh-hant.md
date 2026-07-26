---
lang: zh-hant
direction: ltr
source: docs/source/fastpq_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8324267c90cfbaf718760c4883427e85d81edcfa180dd9f64fd31a5e219749f4
source_last_modified: "2026-01-17T04:50:15.304524+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# FASTPQ 證明者工作分解

本文檔捕獲了交付可投入生產的 FASTPQ-ISI 驗證器並將其連接到數據空間調度管道的分階段計劃。下面的每個定義都是規範性的，除非標記為 TODO。估計的健全性使用開羅風格的 DEEP-FRI 界限；如果測量的界限低於 128 位，CI 中的自動拒絕採樣測試就會失敗。

## 階段 0 — 哈希佔位符（已落地）
- 具有 BLAKE2b 承諾的確定性 Norito 編碼。
- 佔位符後端返回 `BackendUnavailable`。
- `fastpq_isi` 提供的規範參數表。

## 第 1 階段 — 跟踪生成器原型

> **狀態 (2025-11-09)：** `fastpq_prover` 現在公開規範包裝
> 助手 (`pack_bytes`、`PackedBytes`) 和確定性 Poseidon2
> 超過金發姑娘的訂購承諾。常量固定到
> `ark-poseidon2` 提交 `3f2b7fe`，關閉關於換出臨時 BLAKE2 的後續工作
> 佔位符已關閉。金色燈具 (`tests/fixtures/packing_roundtrip.json`,
> `tests/fixtures/ordering_hash.json`) 現在錨定回歸套件。

### 目標
- 為 KV 更新 AIR 實現 FASTPQ 跟踪生成器。每行必須編碼：
  - `key_limbs[i]`：規範密鑰路徑的 base-256 肢體（7 字節，小端）。
  - `value_old_limbs[i]`、`value_new_limbs[i]`：前/後值的包裝相同。
  - 選擇器列：`s_active`、`s_transfer`、`s_mint`、`s_burn`、`s_role_grant`、`s_role_revoke`、`s_meta_set`、`s_perm`。
  - 輔助列：`delta = value_new - value_old`、`running_asset_delta`、`metadata_hash`、`supply_counter`。
  - 資產列：使用 7 字節肢體的 `asset_id_limbs[i]`。
  - 每個級別的 SMT 列 `ℓ`：`path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`，以及非會員資格的 `neighbour_leaf`。
  - 元數據列：`dsid`、`slot`。
- **確定性排序。 ** 使用穩定排序按 `(key_bytes, op_rank, original_index)` 字典順序對行進行排序。 `op_rank` 映射：`transfer=0`、`mint=1`、`burn=2`、`role_grant=3`、`role_revoke=4`、`meta_set=5`。 `original_index` 是排序前從 0 開始的索引。保留生成的 Poseidon2 排序哈希（域標籤 `fastpq:v1:ordering`）。將哈希原像編碼為 `[domain_len, domain_limbs…, payload_len, payload_limbs…]`，其中長度為 u64 字段元素，因此尾隨零字節保持可區分。
- 查找見證：當存儲列 `s_perm`（`s_role_grant` 和 `s_role_revoke` 的邏輯或）為 1 時，生成 `perm_hash = Poseidon2(role_id || permission_id || epoch_u64_le)`。角色/權限 ID 是固定寬度的 32 字節 LE 字符串； epoch 是 8 字節 LE。
- 在 AIR 之前和內部強制執行不變量：選擇器互斥、按資產保存、dsid/槽常量。
- `N_trace = 2^k`（行數 `pow2_ceiling`）； `N_eval = N_trace * 2^b`，其中 `b` 是放大指數。
- 提供固定裝置和性能測試：
  - 包裝往返（`fastpq_prover/tests/packing.rs`、`tests/fixtures/packing_roundtrip.json`）。
  - 訂購穩定性哈希 (`tests/fixtures/ordering_hash.json`)。
  - 批量夾具（`trace_transfer.json`、`trace_mint.json`、`trace_duplicate_update.json`）。### AIR 列架構
|欄目組|姓名 |描述 |
| ----------------- | ------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
|活動 | `s_active` | 1 表示實際行，0 表示填充。                                                                                       |
|主要| `key_limbs[i]`、`value_old_limbs[i]`、`value_new_limbs[i]` |打包的 Goldilocks 元素（little-endian、7 字節肢體）。                                                             |
|資產| `asset_id_limbs[i]` |打包的規範資產標識符（小端、7 字節肢體）。                                                      |
|選擇器 | `s_transfer`、`s_mint`、`s_burn`、`s_role_grant`、`s_role_revoke`、`s_meta_set`、`s_perm` | 0/1。約束：Σ選擇器（包括`s_perm`）= `s_active`； `s_perm` 鏡像角色授予/撤銷行。              |
|輔助| `delta`、`running_asset_delta`、`metadata_hash`、`supply_counter` |用於約束、保護和審計跟踪的狀態。                                                           |
|表面貼裝技術| `path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`、`neighbour_leaf` |每級 Poseidon2 輸入/輸出以及非會員的鄰居見證。                                         |
|查找 | `perm_hash` |用於權限查找的 Poseidon2 哈希（僅當 `s_perm = 1` 時受限制）。                                            |
|元數據| `dsid`，`slot` |跨行恆定。                                                                                                 |### 數學與約束
- **字段打包：** 字節被分成 7 字節肢體（小端）。每肢 `limb_j = Σ_{k=0}^{6} byte_{7j+k} * 256^k`；拒絕肢體≥金發姑娘模數。
- **平衡/守恆：** 讓 `δ = value_new - value_old`。按 `asset_id` 對行進行分組。在每個資產組的第一行定義 `r_asset_start = 1`（否則為 0）並約束
  ```
  running_asset_delta = (1 - r_asset_start) * running_asset_delta_prev + δ.
  ```
  在每個資產組的最後一行斷言
  ```
  running_asset_delta = Σ (s_mint * δ) - Σ (s_burn * δ).
  ```
  轉移自動滿足約束，因為它們的 δ 值在整個組中總和為零。示例：如果 `value_old = 100` 和 `value_new = 120` 在鑄幣行上，δ = 20，因此鑄幣總和貢獻 +20，並且當沒有發生銷毀時，最終檢查解析為零。
- **填充：**介紹`s_active`。將所有行約束乘以 `s_active` 並強制使用連續前綴：`s_active[i] ≥ s_active[i+1]`。填充行 (`s_active=0`) 必須保持常量值，但在其他方面不受約束。
- **排序哈希：** Poseidon2 哈希（域 `fastpq:v1:ordering`）通過行編碼；存儲在公共 IO 中以實現可審計性。

## 第 2 階段 — STARK 證明者核心

### 目標
- 通過跟踪和查找評估向量構建 Poseidon2 Merkle 承諾。參數：速率=2、容量=1、整輪=8、部分輪=57、固定到 `ark-poseidon2` 提交 `3f2b7fe` (v0.3.0) 的常量。
- 低度擴展：評估域 `D = { g^i | i = 0 .. N_eval-1 }` 上的每一列，其中 `N_eval = 2^{k+b}` 除以 Goldilocks 的 2-adic 容量。設 `g = ω^{(p-1)/N_eval}` 與 `ω` 為金發姑娘的固定原根，`p` 為其模；使用基本子群（無陪集）。在轉錄本中記錄 `g`（標籤 `fastpq:v1:lde`）。
- 複合多項式：對於每個約束 `C_j`，形成 `F_j(X) = C_j(X) / Z_N(X)`，其度數邊界如下。
- 查找參數（權限）：來自轉錄本的示例 `γ`。跟踪產品 `Z_0 = 1`、`Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}`。表產品 `T = ∏_j (table_perm_j - γ)`。邊界約束：`Z_final / T = 1`。
- 數量為 `r ∈ {8, 16}` 的 DEEP-FRI：對於每一層，吸收帶有標籤 `fastpq:v1:fri_layer_ℓ` 的根部、樣品 `β_ℓ`（標籤 `fastpq:v1:beta_ℓ`），並通過 `H_{ℓ+1}(i) = Σ_{k=0}^{r-1} H_ℓ(r*i + k) * β_ℓ^k` 折疊。
- 證明對象（Norito 編碼）：
  ```
  Proof {
      protocol_version: u16,
      params_version: u16,
      parameter_set: String,
      public_io: PublicIO,
      trace_root: [u8; 32],
      lookup_root: [u8; 32],
      fri_layers: Vec<[u8; 32]>,
      alphas: Vec<Field>,
      betas: Vec<Field>,
      queries: Vec<QueryOpening>,
  }
  ```
- 驗證者鏡像證明者；使用黃金轉錄本在 1k/5k/20k 行跟踪上運行回歸套件。

###學位會計
|約束|劃分前的度數 |選擇器後的度數 |保證金 vs `deg(Z_N)` |
|------------------------|------------------------------------|------------------------|------------------------|
|轉移/鑄造/燒毀保存 | ≤1 | ≤1 | `deg(Z_N) - 2` |
|角色授予/撤銷查找 | ≤2 | ≤2 | `deg(Z_N) - 3` |
|元數據集 | ≤1 | ≤1 | `deg(Z_N) - 2` |
| SMT 哈希（每級）| ≤3 | ≤3 | `deg(Z_N) - 4` |
|查找盛大產品|產品關係 |不適用 |邊界約束|
|邊界根/供應總量| 0 | 0 |準確|

填充行通過 `s_active` 處理；虛擬行將跟踪擴展到 `N_trace`，而不違反約束。## 編碼和轉錄（全局）
- **字節打包：** base-256（7 字節肢體，小端）。在 `fastpq_prover/tests/packing.rs` 中進行測試。
- **字段編碼：** 規範的 Goldilocks（little-endian 64 位分支，拒絕 ≥ p）； Poseidon2 輸出/SMT 根序列化為 32 字節小端數組。
- **成績單（菲亞特-沙米爾）：**
  1. BLAKE2b 吸收 `protocol_version`、`params_version`、`parameter_set`、`public_io` 和 Poseidon2 提交標籤 (`fastpq:v1:init`)。
  2. 吸收`trace_root`、`lookup_root`（`fastpq:v1:roots`）。
  3. 導出查找挑戰 `γ` (`fastpq:v1:gamma`)。
  4. 推導組合挑戰 `α_j` (`fastpq:v1:alpha_j`)。
  5. 對於每個FRI層根，用`fastpq:v1:fri_layer_ℓ`吸收，導出`β_ℓ`（`fastpq:v1:beta_ℓ`）。
  6. 派生查詢索引 (`fastpq:v1:query_index`)。

  標籤為小寫 ASCII；驗證者在抽樣挑戰之前拒絕不匹配。金色成績單夾具：`tests/fixtures/transcript_v1.json`。
- **版本控制：** `protocol_version = 1`、`params_version` 與 `fastpq_isi` 參數集匹配。

## 查找參數（權限）
- 提交的表按 `(role_id_bytes, permission_id_bytes, epoch_le)` 字典順序排序，並通過 Poseidon2 Merkle 樹提交（`PublicIO` 中的 `perm_root`）。
- 跟踪見證使用 `perm_hash` 和選擇器 `s_perm`（角色授予/撤銷的 OR）。該元組被編碼為具有固定寬度（32、32、8 字節）的 `role_id_bytes || permission_id_bytes || epoch_u64_le`。
- 產品關係：
  ```
  Z_0 = 1
  for each row i: Z_i = Z_{i-1} * (perm_hash_i - γ)^{s_perm_i}
  T = ∏_j (table_perm_j - γ)
  ```
  邊界斷言：`Z_final / T = 1`。有關具體累加器演練，請參閱 `examples/lookup_grand_product.md`。

## 稀疏默克爾樹約束
- 定義 `SMT_HEIGHT`（級別數）。對於所有 `ℓ ∈ [0, SMT_HEIGHT)`，都會顯示列 `path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`、`neighbour_leaf`。
- Poseidon2 參數固定到 `ark-poseidon2` 提交 `3f2b7fe` (v0.3.0)；域標籤 `fastpq:v1:poseidon_node`。所有節點都使用小端字段編碼。
- 更新每個級別的規則：
  ```
  if path_bit_ℓ == 0:
      node_out_ℓ = Poseidon2(node_in_ℓ, sibling_ℓ)
  else:
      node_out_ℓ = Poseidon2(sibling_ℓ, node_in_ℓ)
  ```
- 刀片套件 `(node_in_0 = 0, node_out_0 = value_new)`；刪除集 `(node_in_0 = value_old, node_out_0 = 0)`。
- 非會員證明提供`neighbour_leaf`以顯示查詢的區間為空。有關工作示例和 JSON 佈局，請參閱 `examples/smt_update.md`。
- 邊界約束：對於前行，最終散列等於 `old_root`；對於後行，最終散列等於 `new_root`。

## 健全性參數和 SLO
| N_trace |爆炸| FRI 數量 |層 |查詢 | est位|打樣尺寸 (≤) |內存 (≤) | P95 潛伏期 (≤) |
| -------- | ------ | --------- | ------ | -------- | -------- | ---------------- | -------- | ---------------- |
| 2^15 | 2^15 8 | 8 | 5 | 52 | 52 〜190 | 300 KB | 1.5 GB | 1.5 GB 0.40 秒 (A100) |
| 2^16 | 2^16 8 | 8 | 6 | 58 | 58 〜132 | 420 KB | 2.5 GB | 2.5 GB 0.75 秒 (A100) |
| 2^17 | 2^17 16 | 16 16 | 16 5 | 64 | 64 〜142 | 550 KB | 3.5 GB | 3.5 GB 1.20 秒 (A100) |

推導遵循附錄 A。如果估計位 <128，CI 工具會生成格式錯誤的證明並失敗。## 公共 IO 架構
|領域 |字節 |編碼 |筆記|
|----------------|--------------------|----------------------------------------------------|----------------------------------------|
| `dsid` | 16 | 16小端 UUID |條目通道的數據空間 ID（默認通道的全局），使用標籤 `fastpq:v1:dsid` 進行哈希處理。 |
| `slot` | 8 |小尾數 u64 |自紀元以來的納秒。            |
| `old_root` | 32 | 32小端 Poseidon2 字段字節 |批量前SMT根。              |
| `new_root` | 32 | 32小端 Poseidon2 字段字節 | SMT批量後根。               |
| `perm_root` | 32 | 32小端 Poseidon2 字段字節 |插槽的權限表根。 |
| `tx_set_hash` | 32 | 32布萊克2b |排序的指令標識符。     |
| `parameter` |變量 | UTF-8（例如 `fastpq-lane-balanced`）|參數集名稱。                 |
| `protocol_version`，`params_version` |各 2 個 |小尾數 u16 |版本值。                      |
| `ordering_hash` | 32 | 32 Poseidon2（小端）|排序行的穩定哈希。         |

刪除由零值肢體編碼；缺席的鍵使用零葉+鄰居見證。

`FastpqTransitionBatch.public_inputs` 是 `dsid`、`slot` 和根承諾的規範載體；
批次元數據保留用於條目哈希/轉錄本計數簿記。

## 哈希編碼
- 排序哈希：Poseidon2（標籤 `fastpq:v1:ordering`）。
- 批量工件哈希：BLAKE2b over `PublicIO || proof.commitments`（標籤 `fastpq:v1:artifact`）。

## 完成 (DoD) 的階段定義
- **第一階段國防部**
  - 包裝往返測試和固定裝置合併。
  - AIR 規範 (`docs/source/fastpq_air.md`) 包括 `s_active`、資產/SMT 列、選擇器定義（包括 `s_perm`）和符號約束。
  - 排序哈希記錄在 PublicIO 中並通過固定裝置進行驗證。
  - 使用會員和非會員向量實現 SMT/查找見證生成。
  - 保存測試涵蓋轉移、薄荷、燃燒和混合批次。
- **第 2 階段國防部**
  - 實施轉錄規範；黃金成績單 (`tests/fixtures/transcript_v1.json`) 和域名標籤已驗證。
  - Poseidon2 參數提交 `3f2b7fe` 固定在證明者和驗證者中，並進行跨架構的字節序測試。
  - 健全 CI 防護激活；記錄校樣大小/RAM/延遲 SLO。
- **第 3 階段國防部**
  - 使用冪等密鑰記錄的調度程序 API（`SubmitProofRequest`、`ProofResult`）。
  - 通過重試/退避以可尋址方式存儲內容的證明工件。
  - 導出隊列深度、隊列等待時間、證明執行延遲、重試計數、後端故障計數和 GPU/CPU 利用率的遙測，以及每個指標的儀表板和警報閾值。## 第 5 階段 — GPU 加速和優化
- 目標內核：LDE (NTT)、Poseidon2 哈希、Merkle 樹構建、FRI 折疊。
- 確定性：禁用快速數學，確保跨 CPU、CUDA、Metal 的位相同輸出。 CI 必須跨設備比較證明根。
- 基準套件比較參考硬件（例如 Nvidia A100、AMD MI210）上的 CPU 與 GPU。
- 金屬後端（Apple Silicon）：
  - 構建腳本通過 `xcrun metal`/`xcrun metallib` 將內核套件（`metal/kernels/ntt_stage.metal`、`metal/kernels/poseidon2.metal`）編譯為 `fastpq.metallib`；確保 macOS 開發者工具包含 Metal 工具鏈（`xcode-select --install`，如果需要，則為 `xcodebuild -downloadComponent MetalToolchain`）。 【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:189】
  - 用於 CI 預熱或確定性打包的手動重建（鏡像 `build.rs`）：
    ```bash
    export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
    xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
    xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
    export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
    ```
    成功的構建會發出 `FASTPQ_METAL_LIB=<path>`，因此運行時可以確定性地加載 Metallib。 【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:42】
  - LDE 內核現在假定主機上的評估緩衝區已初始化為零。保留現有的 `vec![0; ..]` 分配路徑或在重用時顯式將緩衝區清零。 【crates/fastpq_prover/src/metal.rs:233】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:141】
  - 陪集乘法被融合到最後的FFT階段以避免額外的通過；對 LDE 分段的任何更改都必須保留該不變性。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:193】
  - 共享內存 FFT/LDE 內核現在停止在圖塊深度處，並將剩餘的蝴蝶和任何逆縮放交給專用的 `fastpq_fft_post_tiling` 通道。 Rust 主機通過兩個內核對相同的列批次進行線程處理，並且僅在 `log_len` 超出切片限制時才啟動切片後分派，因此隊列深度遙測、內核統計數據和回退行為保持確定性，而 GPU 完全處理寬階段工作設備上。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:654】
  - 要試驗發射形狀，請設置 `FASTPQ_METAL_THREADGROUP=<width>`；調度路徑將值限制為設備限制並記錄覆蓋，因此分析運行可以掃描線程組大小而無需重新編譯。 【crates/fastpq_prover/src/metal.rs:321】- 直接調整 FFT 圖塊：主機現在派生線程組通道（短跡線為 16 個，`log_len ≥ 6` 為 32 個，`log_len ≥ 10` 為 64 個，`log_len ≥ 14` 為 128 個，`log_len ≥ 18` 為 256 個）和圖塊深度（小跡線為 5 個階段，小跡線為 4 個階段） `log_len ≥ 12`，一旦域達到 `log_len ≥ 18/20/22`，共享內存通道現在會從請求的域加上設備的執行寬度/最大線程運行 12/14/16 個階段，然後將控制權交給後切片內核）。使用 `FASTPQ_METAL_FFT_LANES`（8 和 256 之間的 2 的冪）和 `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) 覆蓋以固定特定的發射形狀；兩個值都流過 `FftArgs`，被固定到支持的窗口，並被記錄以進行分析掃描。 【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:120】【crates/fastpq_prover/metal/kernels/ntt_stage.metal:244】
- FFT/IFFT 和 LDE 列批處理現在源自已解析的線程組寬度：主機的目標是每個命令緩衝區大約 4096 個邏輯線程，通過循環緩衝區圖塊暫存一次最多融合 64 列，並且當評估域跨越 216/218/220/222 閾值時，僅通過 64→32→16→8→4→2→1 列逐漸下降。這使得每次調度 20k 行捕獲≥64 列，同時確保長陪集仍然確定地完成。自適應調度程序仍然使列寬加倍，直到調度接近 ≈2ms 目標，並且現在只要採樣調度超過該目標 ≥30%，就會自動將批次減半，因此導致每列成本增加的通道/圖塊轉換會回落，無需手動覆蓋。 Poseidon 排列共享相同的自適應調度程序，並且 `fastpq_metal_bench` 中的 `metal_heuristics.batch_columns.poseidon` 塊現在記錄已解析的狀態計數、上限、最後持續時間和覆蓋標誌，因此隊列深度遙測可以直接與 Poseidon 調整相關聯。使用 `FASTPQ_METAL_FFT_COLUMNS` (1–64) 覆蓋以固定確定性 FFT 批量大小，並在需要 LDE 調度程序遵守固定列數時使用 `FASTPQ_METAL_LDE_COLUMNS` (1–64)； Metal bench 在每次捕獲中都會顯示已解析的 `kernel_profiles.*.columns` 條目，因此調整實驗保持可重複性。 【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/metal.rs:1402】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1284】- 多隊列調度現在在獨立 Mac 上是自動的：主機檢查 `is_low_power`、`is_headless` 和設備位置，以決定是否啟動兩個 Metal 命令隊列，僅在工作負載至少承載 16 列（由已解析的扇出擴展）時扇出，並對列批次進行循環，以便長跟踪使兩個 GPU 通道保持繁忙，而不犧牲確定性。命令緩衝區信號量現在強制執行“每個隊列兩個飛行”樓層，隊列遙測記錄全局信號量和每個隊列條目的聚合測量窗口 (`window_ms`) 以及標準化繁忙比率 (`busy_ratio`)，因此發布工件可以證明兩個隊列在同一時間跨度內保持 ≥50% 的繁忙狀態。使用 `FASTPQ_METAL_QUEUE_FANOUT`（1–4 通道）和 `FASTPQ_METAL_COLUMN_THRESHOLD`（扇出前的最小總列數）覆蓋默認值； Metal 奇偶校驗測試強制覆蓋，以便多 GPU Mac 保持覆蓋，並且解決的策略與隊列深度遙測和新的 `metal_dispatch_queue.queues[*]` 一起記錄區塊.【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:871】
- 金屬檢測現在直接探測 `MTLCreateSystemDefaultDevice`/`MTLCopyAllDevices`（在無頭 shell 上預熱 CoreGraphics），然後回退到 `system_profiler`，並且 `FASTPQ_DEBUG_METAL_ENUM` 在設置時打印枚舉設備，因此無頭 CI 運行可以解釋為什麼 `FASTPQ_GPU=gpu` 仍然降級到 CPU路徑。當覆蓋設置為 `gpu` 但未檢測到加速器時，`fastpq_metal_bench` 現在會立即出現錯誤，指針指向調試旋鈕，而不是在 CPU 上默默地繼續。這縮小了 WP2-E 中調用的“靜默 CPU 回退”類別，並為操作員提供了一個旋鈕來捕獲包裝基準內的枚舉日誌。 【crates/fastpq_prover/src/backend.rs:665】【crates/fastpq_prover/src/backend.rs:705】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】
  - Poseidon GPU 計時現在拒絕將 CPU 回退視為“GPU”數據。 `hash_columns_gpu` 報告加速器是否實際運行，`measure_poseidon_gpu` 在管道回退時丟棄樣本（並記錄警告），如果 GPU 哈希不可用，則 Poseidon microbench 子級會退出並顯示錯誤。因此，每當 Metal 執行回退時，`gpu_recorded=false` 隊列摘要仍會記錄失敗的調度窗口，並且儀表板摘要會立即標記回退。現在，當 `metal_dispatch_queue.poseidon.dispatch_count == 0` 時，包裝器 (`scripts/fastpq/wrap_benchmark.py`) 會失敗，因此如果沒有真正的 GPU Poseidon 調度，則無法對 Stage7 捆綁包進行簽名證據。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1123】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2200】【scripts/fastpq/wrap_benchmark.py:912】- 波塞冬哈希現在反映了該分期合同。 `PoseidonColumnBatch` 生成扁平化的有效負載緩衝區以及偏移/長度描述符，主機每批重新設置這些描述符的基礎並運行 `COLUMN_STAGING_PIPE_DEPTH` 雙緩衝區，因此有效負載 + 描述符上傳與 GPU 工作重疊，並且兩個 Metal/CUDA 內核都直接消耗描述符，因此每次調度在發出列摘要之前都會吸收設備上的所有填充速率塊。 `hash_columns_from_coefficients` 現在通過 GPU 工作線程流式傳輸這些批次，默認情況下在離散 GPU 上保持 64 個以上的列處於運行狀態（可通過 `FASTPQ_POSEIDON_PIPE_COLUMNS` / `FASTPQ_POSEIDON_PIPE_DEPTH` 進行調整）。 Metal 工作台記錄了 `metal_dispatch_queue.poseidon_pipeline` 下解析的管道設置 + 批次計數，`kernel_profiles.poseidon.bytes` 包括描述符流量，因此 Stage7 捕獲證明了新的 ABI端到端。 【crates/fastpq_prover/src/trace.rs:604】【crates/fastpq_prover/src/trace.rs:809】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:196 3】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:2675】【板條箱/fastpq_prover/src/metal.rs:2290】【板條箱/fastpq_prover/cuda/fastpq_cuda.cu:351】
- Stage7-P2 融合 Poseidon 哈希現在登陸兩個 GPU 後端。流工作器將連續的 `PoseidonColumnBatch::column_window()` 切片輸入 `hash_columns_gpu_fused`，後者通過管道將它們傳送到 `poseidon_hash_columns_fused`，因此每次調度都會使用規範的 `(⌈columns / 2⌉)` 父映射寫入 `leaf_digests || parent_digests`。 `ColumnDigests` 保留兩個切片，`merkle_root_with_first_level` 立即使用父層，因此 CPU 永遠不會重新計算深度 1 節點，並且 Stage7 遙測可以斷言只要融合內核，GPU 捕獲報告零“回退”父層成功。 【crates/fastpq_prover/src/trace.rs:1070】【crates/fastpq_prover/src/gpu.rs:365】【crates/fastpq_prover/src/metal.rs:2422】【crates/fastpq_prover/cuda/fastpq_cuda.cu:631】
- `fastpq_metal_bench` 現在發出一個 `device_profile` 塊，其中包含 Metal 設備名稱、註冊表 ID、`low_power`/`headless` 標誌、位置（內置、插槽、外部）、離散指示器、`hw.model` 和派生的 Apple SoC 標籤（例如，“M3”）麥克斯”）。 Stage7 儀表板使用此字段來通過 M4/M3 與離散 GPU 進行捕獲，而無需解析主機名，並且 JSON 發送到隊列/啟發式證據旁邊，因此每個發布工件都證明是哪個艦隊類產生了運行。【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2536】- FFT 主機/設備重疊現在使用雙緩衝暫存窗口：當批次 *n* 在 `fastpq_fft_post_tiling` 內完成時，主機將批次 *n+1* 壓平到第二個暫存緩衝區中，並且僅在必須回收緩衝區時才暫停。後端記錄扁平化的批次數量以及扁平化與等待 GPU 完成所花費的時間，並且 `fastpq_metal_bench` 顯示聚合的 `column_staging.{batches,flatten_ms,wait_ms,wait_ratio}` 塊，因此發布工件可以證明重疊而不是無聲的主機停頓。 JSON 報告現在還在 `column_staging.phases.{fft,lde,poseidon}` 下細分了每個階段的總數，讓 Stage7 捕獲證明 FFT/LDE/Poseidon 分段是主機綁定的還是等待 GPU 完成。 Poseidon 排列重複使用相同的池暫存緩衝區，因此 `--operation poseidon_hash_columns` 捕獲現在會發出 Poseidon 特定的 `column_staging` 增量以及隊列深度證據，而無需定制儀器。新的 `column_staging.samples.{fft,lde,poseidon}` 數組記錄每批次的 `batch/flatten_ms/wait_ms/wait_ratio` 元組，因此可以輕鬆證明 `COLUMN_STAGING_PIPE_DEPTH` 重疊保持不變（或發現主機何時開始等待 GPU完成）。 【板條箱/fastpq_prover/src/metal.rs:319】【板條箱/fastpq_prover/src/metal.rs:330】【板條箱/fastpq_prover/src/metal.rs:1813】【板條箱/fas tpq_prover/src/metal.rs:2488】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1189】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1216】- Poseidon2 加速現在作為高佔用金屬內核運行：每個線程組將輪常量和 MDS 行複製到線程組內存中，展開完整/部分輪，並在每個通道上行走多個狀態，因此每次調度至少啟動 4096 個邏輯線程。通過 `FASTPQ_METAL_POSEIDON_LANES`（32 和 256 之間的 2 的冪，限製到器件限制）和 `FASTPQ_METAL_POSEIDON_BATCH`（每通道 1-32 個狀態）覆蓋啟動形狀，以重現分析實驗，而無需重建 `fastpq.metallib`； Rust 主機在分派之前通過 `PoseidonArgs` 線程解決調整。現在，主機每次啟動都會對 `MTLDevice::{is_low_power,is_headless,location}` 進行一次快照，並自動將離散 GPU 偏向於 VRAM 分層啟動（≥48GiB 部件上為 `256×24`，32GiB 上為 `256×20`，否則為 `256×16`），而低功耗 SoC 則堅持使用 `256×8` （128/64 通道硬件的後備繼續使用每通道 8/6 個狀態），因此操作員無需觸及環境變量即可獲得 >16 狀態的管道深度。 `fastpq_metal_bench` 在 `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 下重新執行自身，以捕獲專用的 `poseidon_microbench` 塊，將標量通道與多狀態內核進行比較，以便發布工件可以引用具體的加速。同樣捕獲表面 `poseidon_pipeline` 遙測（`chunk_columns`、`pipe_depth`、`batches`、`fallbacks`），因此 Stage7 證據證明了每個 GPU 上的重疊窗口跟踪.【板條箱/fastpq_prover/metal/kernels/poseidon2.metal:1】【板條箱/fastpq_prover/src/metal_config.rs:78】【板條箱/fastpq_prover/src/metal.rs:1971】【cra tes/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【板條箱/fastpq_prover/src/trace.rs:299】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】
  - LDE 切片分段現在反映了 FFT 啟發式：一旦 `log₂(len) ≥ 18`，大量跟踪僅執行共享內存傳遞中的 12 個階段，在 log220 處下降到 10 個階段，並在 log222 處箝位到 8 個階段，以便寬蝴蝶移動到後切片內核中。每當您需要確定性深度時，請使用 `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) 覆蓋；主機僅在啟發式提前停止時啟動後平鋪調度，因此隊列深度和內核遙測保持確定性。 【crates/fastpq_prover/src/metal.rs:827】
  - 內核微優化：共享內存 FFT/LDE 塊現在重用每通道旋轉和陪集步幅，而不是為每個蝴蝶重新評估 `pow_mod*`。每個通道預先計算 `w_seed`、`w_stride` 和（需要時）每個塊的陪集步長，然後流過偏移量，大幅削減 `apply_stage_tile`/`apply_stage_global` 內部的標量乘法，並將 20k 行 LDE 均值降至約 1.55 秒（最新）啟發式（仍然高於 950 毫秒的目標，但比僅批處理調整進一步提高了約 50 毫秒）。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:164】【fastpq_metal_bench_run11.json:1】- 內核套件現在有一個專門的參考資料（`docs/source/fastpq_metal_kernels.md`），它記錄了每個入口點、`fastpq.metallib`中強制執行的線程組/圖塊限制，以及手動編譯metallib的複制步驟。 【docs/source/fastpq_metal_kernels.md:1】
  - 基準報告現在發出一個 `post_tile_dispatches` 對象，該對象記錄在專用後平舖內核中運行了多少個 FFT/IFFT/LDE 批次（每種調度計數加上階段/log2 邊界）。 `scripts/fastpq/wrap_benchmark.py` 將該塊複製到 `benchmarks.post_tile_dispatches`/`benchmarks.post_tile_summary` 中，並且清單門拒絕忽略證據的 GPU 捕獲，因此每個 20k 行工件都證明多通道內核運行設備上。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:255】【xtask/src/fastpq.rs:280】
  - 設置 `FASTPQ_METAL_TRACE=1` 以發出每次調度調試日誌（管道標籤、線程組寬度、啟動組、經過時間）以進行儀器/金屬跟踪關聯。 【crates/fastpq_prover/src/metal.rs:346】
- 調度隊列現已檢測：`FASTPQ_METAL_MAX_IN_FLIGHT` 限制並發 Metal 命令緩衝區（自動默認值通過 `system_profiler` 檢測到的 GPU 核心計數派生，當 macOS 拒絕報告設備時，至少夾緊到隊列扇出層，並使用主機並行回退）。該工作台支持隊列深度採樣，因此導出的 JSON 帶有 `metal_dispatch_queue` 對象，其中包含 `limit`、`dispatch_count`、`max_in_flight`、`busy_ms` 和 `overlap_ms` 字段作為發布證據，添加了嵌套每當僅 Poseidon 捕獲 (`--operation poseidon_hash_columns`) 運行時，`metal_dispatch_queue.poseidon` 塊就會發出 `metal_heuristics` 塊，描述已解析的命令緩衝區限制以及 FFT/LDE 批處理列（包括是否覆蓋強制值），以便審閱者可以在遙測的同時審核調度決策。 Poseidon 內核還提供從內核樣本中提取的專用 `poseidon_profiles` 塊，以便跨工件跟踪字節/線程、佔用和調度幾何結構。如果主運行無法收集隊列深度或 LDE 零填充統計信息（例如，當 GPU 調度默默地回退到 CPU 時），該工具會自動觸發單個探針調度來收集丟失的遙測數據，並且現在在 GPU 拒絕報告它們時合成主機零填充計時，因此已發布的證據始終包括 `zero_fill`區塊.【crates/fastpq_prover/src/metal.rs:2056】【crates/fastpq_prover/src/metal.rs:247】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1524】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2078】
  - 在沒有 Metal 工具鏈的情況下交叉編譯時設置 `FASTPQ_SKIP_GPU_BUILD=1`；警告記錄了跳過，規劃器繼續在CPU路徑上運行。 【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】- 運行時檢測使用 `system_profiler` 來確認 Metal 支持；如果框架、設備或 Metallib 丟失，構建腳本將清除 `FASTPQ_METAL_LIB` 並且規劃器保留在確定性 CPU 上路徑.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】【crates/fastpq_prover/src/metal.rs:43】
  - 操作員清單（金屬主機）：
    1.確認工具鏈存在，並且`FASTPQ_METAL_LIB`指向已編譯的`.metallib`（`echo $FASTPQ_METAL_LIB`在`cargo build --features fastpq-gpu`之後應該非空）。 【crates/fastpq_prover/build.rs:188】
    2. 在啟用 GPU 通道的情況下運行奇偶校驗測試：`FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release`。這會鍛煉 Metal 內核，並在檢測失敗時自動回退。 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
    3. 捕獲儀表板的基準示例：找到已編譯的 Metal 庫
       (`fd -g 'fastpq.metallib' target/release/build | head -n1`)，通過導出
       `FASTPQ_METAL_LIB`，然後運行
      `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`。
       規範的 `fastpq-lane-balanced` 集現在將每次捕獲填充到 32,768 行，因此
       JSON 反映了請求的 20k 行和驅動 GPU 的填充域
       內核。將 JSON/日誌上傳到您的證據存儲；每晚 macOS 工作流程鏡像
      這次運行並將文物存檔以供參考。報告記錄
     `fft_tuning.{threadgroup_lanes,tile_stage_limit}` 與每個操作的 `speedup` 一起，
     LDE 部分添加了 `zero_fill.{bytes,ms,queue_delta}`，因此發布的工件證明了確定性，
     主機零填充開銷，以及增量 GPU 隊列使用情況（限制、調度計數、
     高峰飛行時間、繁忙/重疊時間），以及新的 `kernel_profiles` 塊捕獲每個內核
     佔用率、估計帶寬和持續時間範圍，以便儀表板可以標記 GPU
       無需重新處理原始樣本即可進行回歸。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
       預計 Metal LDE 路徑保持在 950ms 以下（Apple M 系列硬件上的 `<1 s` 目標）；
4. 從真實的 ExecWitness 捕獲行使用遙測數據，以便儀表板可以繪製傳輸小工具的圖表
   收養。從 Torii 獲取證人
  (`iroha_cli audit witness --binary --out exec.witness`) 並解碼它
  `iroha_cli audit witness --decode exec.witness`（可選添加
  `--fastpq-parameter fastpq-lane-balanced` 斷言預期參數集； FASTPQ批次
  默認發出；僅當您需要調整輸出時才通過 `--no-fastpq-batches`）。
   現在，每個批次條目都會發出一個 `row_usage` 對象（`total_rows`、`transfer_rows`、
   `non_transfer_rows`、每個選擇器計數和 `transfer_ratio`）。存檔該 JSON 片段
   重新處理原始轉錄本。 【crates/iroha_cli/src/audit.rs:209】將新捕獲的內容與
   之前的基線為 `scripts/fastpq/check_row_usage.py`，因此如果傳輸比率或
   總行回歸：

   ```bash
   python3 scripts/fastpq/check_row_usage.py \
     --baseline artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json \
     --candidate fastpq_row_usage_2025-05-12.json \
     --max-transfer-ratio-increase 0.005 \
     --max-total-rows-increase 0
   ```用於冒煙測試的 JSON blob 示例位於 `scripts/fastpq/examples/` 中。您可以在本地運行 `make check-fastpq-row-usage`
   （包裝 `ci/check_fastpq_row_usage.sh`），CI 通過 `.github/workflows/fastpq-row-usage.yml` 運行相同的腳本來比較提交的
   `artifacts/fastpq_benchmarks/fastpq_row_usage_*.json` 快照，因此證據包無論何時都會快速失敗
   轉移行慢慢回升。如果您想要機器可讀的差異，請傳遞 `--summary-out <path>`（CI 作業上傳 `fastpq_row_usage_summary.json`）。
   當 ExecWitness 不方便時，可以使用 `fastpq_row_bench` 合成回歸樣本
   (`crates/fastpq_prover/src/bin/fastpq_row_bench.rs:1`)，它發出完全相同的 `row_usage`
   可配置選擇器計數的對象（例如 65536 行壓力測試）：

   ```bash
   cargo run -p fastpq_prover --bin fastpq_row_bench -- \
     --transfer-rows 65536 \
     --mint-rows 256 \
     --burn-rows 128 \
     --pretty \
     --output artifacts/fastpq_benchmarks/fastpq_row_usage_65k.json
   ```Stage7-3 推出捆綁包還必須通過 `scripts/fastpq/validate_row_usage_snapshot.py`，這
   強制每個 `row_usage` 條目包含選擇器計數並且
   `transfer_ratio = transfer_rows / total_rows`； `ci/check_fastpq_rollout.sh` 呼叫助手
   自動，因此缺少這些不變量的包會在強制執行 GPU 通道之前失敗。 【scripts/fastpq/validate_row_usage_snapshot.py:1】【ci/check_fastpq_rollout.sh:1】
       工作台清單門通過 `--max-operation-ms lde=950` 強制執行此操作，因此請刷新
       每當您的證據超過該界限時就進行捕獲。
      當您還需要儀器證據時，請傳遞 `--trace-dir <dir>`，以便線束
      通過 `xcrun xctrace record`（默認“金屬系統跟踪”模板）重新啟動自身並
      將帶時間戳的 `.trace` 文件與 JSON 一起存儲；您仍然可以覆蓋位置/
      使用 `--trace-output <path>` 以及可選的 `--trace-template` 手動模板 /
      `--trace-seconds`。生成的 JSON 通告 `metal_trace_{template,seconds,output}`，因此
      人工製品包總是識別捕獲的痕跡。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】
      將每個捕獲包裹起來
      `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output`
       （如果需要固定簽名身份，請添加 `--gpg-key <fingerprint>`），因此捆綁失敗
       每當 GPU LDE 平均值突破 950 毫秒目標、Poseidon 超過 1 秒或
       Poseidon 遙測塊丟失，嵌入 `row_usage_snapshot`
      在 JSON 旁邊，在 `benchmarks.poseidon_microbench` 下顯示 Poseidon 微基準摘要，
      並且仍然攜帶運行手冊和 Grafana 儀表板的元數據
    （`dashboards/grafana/fastpq_acceleration.json`）。 JSON 現在發出 `speedup.ratio` /
     每次操作 `speedup.delta_ms` 因此發布證據可以證明 GPU 與
     CPU 增益無需重新處理原始樣本，並且包裝器會復制
     零填充統計信息（加上 `queue_delta`）到 `zero_fill_hotspots`（字節、延遲、派生
     GB/s)，在 `metadata.metal_trace` 下記錄儀器元數據，線程可選
     當提供 `--row-usage <decoded witness>` 時，`metadata.row_usage_snapshot` 塊，並將
     每個內核計數器進入 `benchmarks.kernel_summary` 因此填充瓶頸，金屬隊列
     利用率、內核佔用率和帶寬回歸一目了然，無需
     深入研究原始報告。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:521】【scripts/fastpq/wrap_benchmark.py:1】【artifacts/fastpq_benchmarks/fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1】
     由於行使用快照現在與打包的工件一起傳輸，因此部署票證只需
     引用該包而不是附加第二個 JSON 片段，並且 CI 可以區分嵌入的
    驗證 Stage7 提交時直接計數。要單獨存檔微基準數據，
    運行 `python3 scripts/fastpq/export_poseidon_microbench.py --bundle artifacts/fastpq_benchmarks/<metal>.json`
    並將生成的文件存儲在 `benchmarks/poseidon/` 下。保持聚合清單最新
    `python3 scripts/fastpq/aggregate_poseidon_microbench.py --input benchmarks/poseidon --output benchmarks/poseidon/manifest.json`
    因此儀表板/CI 可以比較完整的歷史記錄，而無需手動遍歷每個文件。4. 通過curl `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`（Prometheus端點）或查找`telemetry::fastpq.execution_mode`日誌來驗證遙測；意外的 `resolved="cpu"` 條目表明儘管 GPU 意圖，主機還是回退了。 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
    5. 使用`FASTPQ_GPU=cpu`（或配置旋鈕）在維護期間強制CPU執行並確認回退日誌仍然出現；這使 SRE Runbook 與確定性路徑保持一致。 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
- 遙測和後備：
  - 執行模式日誌（`telemetry::fastpq.execution_mode`）和計數器（`fastpq_execution_mode_total{device_class="…", backend="metal"|…}`）公開了請求模式與已解決模式，因此儀表板中可以看到靜默回退。 【crates/fastpq_prover/src/backend.rs:174】【crates/iroha_telemetry/src/metrics.rs:5397】
  - `FASTPQ Acceleration Overview` Grafana 板 (`dashboards/grafana/fastpq_acceleration.json`) 可視化 Metal 採用率並鏈接回基準工件，而配對警報規則 (`dashboards/alerts/fastpq_acceleration_rules.yml`) 在持續降級時推出。
  - `FASTPQ_GPU={auto,cpu,gpu}` 覆蓋仍然受支持；未知值會引發警告，但仍會傳播到遙測進行審核。 【crates/fastpq_prover/src/backend.rs:308】【crates/fastpq_prover/src/backend.rs:349】
  - CUDA 和 Metal 必須通過 GPU 奇偶校驗測試 (`cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu`)；當metallib不存在或檢測失敗時，CI會優雅地跳過。 【crates/fastpq_prover/src/gpu.rs:49】【crates/fastpq_prover/src/backend.rs:346】
  - 金屬就緒證據（每次推出時存檔以下工件，以便路線圖審核可以證明確定性、遙測覆蓋範圍和後備行為）：|步驟|目標|命令/證據|
    | ----| ----| ------------------ |
    |構建 Metallib |確保 `xcrun metal`/`xcrun metallib` 可用，並為此提交發出確定性 `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"`； `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"`； `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"`；導出 `FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`.【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】
    |驗證環境變量 |通過檢查構建腳本記錄的環境變量來確認 Metal 保持啟用狀態 | `echo $FASTPQ_METAL_LIB`（必須返回絕對路徑；空表示後端被禁用）。 【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
    | GPU 奇偶校驗套件 |在發貨前證明內核執行（或發出確定性降級日誌）| `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` 並存儲顯示 `backend="metal"` 或後備警告的結果日誌片段。 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】
    |基準樣本|捕獲記錄 `speedup.*` 和 FFT 調整的 JSON/日誌對，以便儀表板可以獲取加速器證據 | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`；將 JSON、帶時間戳的 `.trace` 和標準輸出與發行說明一起存檔，以便 Grafana 板選擇 Metal 運行（報告記錄請求的 20k 行加上填充的 32,768 行域，以便審閱者可以確認 `<1 s` LDE目標）。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】
    |包裝並簽署報告 |如果 GPU LDE 平均時間超過 950 毫秒、Poseidon 超過 1 秒或 Poseidon 遙測塊丟失，則發布失敗，並生成簽名的工件包 | `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`；發送打包的 JSON 和生成的 `.json.asc` 簽名，以便審核員可以驗證亞秒級指標，而無需重新運行工作負載。 【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】 |
    |簽署的替補清單 |在 Metal/CUDA 捆綁包中強制執行 `<1 s` LDE 證據並捕獲簽名摘要以供發布批准 | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`；將清單+簽名附加到發布票證上，以便下游自動化可以驗證亞秒級證明指標。 【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
| CUDA 捆綁包 |使 SM80 CUDA 捕獲與 Metal 證據保持同步，以便清單涵蓋兩個 GPU 類別。 | Xeon+RTX 主機上的 `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` → `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_cuda_bench.json artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --label device_class=xeon-rtx-sm80 --sign-output`；將包裝路徑附加到 `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`，將 `.json`/`.asc` 對保留在金屬包旁邊，並在審核員需要參考時引用種子 `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`佈局.【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】
|遙測檢查 |驗證 Prometheus/OTEL 表面反映 `device_class="<matrix>", backend="metal"`（或記錄降級）| `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` 並複制啟動時發出的 `telemetry::fastpq.execution_mode` 日誌。 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】|強制後退演練|記錄 SRE 劇本的確定性 CPU 路徑 |使用 `FASTPQ_GPU=cpu` 或 `zk.fastpq.execution_mode = "cpu"` 運行一個簡短的工作負載並捕獲降級日誌，以便操作員可以排練回滾過程。 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】
    |跟踪捕獲（可選）|進行分析時，捕獲調度跟踪，以便稍後可以檢查內核通道/圖塊覆蓋 |使用 `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` 重新運行一項奇偶校驗測試，並將生成的跟踪日誌附加到您的發布工件中。 【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】

    使用發布票證歸檔證據，並在 `docs/source/fastpq_migration_guide.md` 中鏡像相同的清單，以便分期/產品部署遵循相同的劇本。 【docs/source/fastpq_migration_guide.md:1】

### 發布清單執行

將以下門添加到每個 FASTPQ 發布票證中。發布將被阻止，直到所有項目都已完成
完整併作為簽名的文物附在附件中。

1. **亞秒級證明指標** — 規範的 Metal 基準測試捕獲
   (`fastpq_metal_bench_*.json`) 必須證明 20000 行工作負載（32768 填充行）完成於
   <1秒。具體來說，`benchmarks.operations` 條目，其中 `operation = "lde"` 和匹配的
   `report.operations` 示例必須顯示 `gpu_mean_ms ≤ 950`。超出上限要求的運行
   在簽署清單之前進行調查並重新捕獲。
2. **簽署基準清單** — 記錄新的 Metal + CUDA 捆綁包後，運行
   `cargo xtask fastpq-bench-manifest … --signing-key <path>` 發出
   `artifacts/fastpq_bench_manifest.json` 和分離的簽名
   （`artifacts/fastpq_bench_manifest.sig`）。將兩個文件以及公鑰指紋附加到
   發布票據，以便審閱者可以獨立驗證摘要和簽名。 【xtask/src/fastpq.rs:1】
3. **證據附件** — 存儲原始基準 JSON、標準輸出日誌（或儀器跟踪，當
   捕獲），以及帶有發布票證的清單/簽名對。清單僅
   當票證鏈接到這些文物並且值班審核員確認了這些文物時，該票證被視為綠色
   `fastpq_bench_manifest.json`中記錄的摘要與上傳的文件匹配。 【artifacts/fastpq_benchmarks/README.md:1】

## 第 6 階段 — 強化和文檔記錄
- 佔位符後端已停用；生產管道默認情況下沒有功能切換。
- 可重複的構建（固定工具鏈、容器映像）。
- 用於跟踪、SMT、查找結構的模糊器。
- 證明者級別的煙霧測試涵蓋治理選票撥款和匯款轉賬，以在 IVM 全面推出之前保持 Stage6 固定裝置的穩定。 【crates/fastpq_prover/tests/realistic_flows.rs:1】
- 包含警報閾值、補救程序、容量規劃指南的運行手冊。
- CI 中的跨架構證明重放（x86_64、ARM64）。

### 工作台清單和釋放門

發布證據現在包括涵蓋金屬和
CUDA 基準測試包。運行：

```bash
cargo xtask fastpq-bench-manifest \
  --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json \
  --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json \
  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \
  --signing-key secrets/fastpq_bench.ed25519 \
  --out artifacts/fastpq_bench_manifest.json
```該命令驗證打包的捆綁包，強制執行延遲/加速閾值，
發出 BLAKE3 + SHA-256 摘要，並（可選）使用
Ed25519 密鑰，以便發布工具可以驗證出處。參見
`xtask/src/fastpq.rs`/`xtask/src/main.rs` 用於實施和
`artifacts/fastpq_benchmarks/README.md` 用於操作指導。

> **注意：** 省略 `benchmarks.poseidon_microbench` 的金屬捆綁現在會導致
> 明顯的一代失敗。重新運行 `scripts/fastpq/wrap_benchmark.py`
>（如果您需要獨立的，則為 `scripts/fastpq/export_poseidon_microbench.py`
> 摘要）每當波塞冬證據丟失時，就會發布清單
> 始終捕獲標量與默認值的比較。 【xtask/src/fastpq.rs:409】

`--matrix` 標誌（默認為 `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
如果存在）加載捕獲的跨設備中位數
`scripts/fastpq/capture_matrix.sh`。清單對 20000 行樓層進行編碼，
每個設備類別的每次操作延遲/加速限制，因此定制
`--require-rows`/`--max-operation-ms`/`--min-operation-speedup` 覆蓋無
除非您正在調試特定的回歸，否則需要更長的時間。

通過將包裝的基準路徑附加到矩陣來刷新矩陣
`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt` 列出並運行
`scripts/fastpq/capture_matrix.sh`。該腳本快照每個設備的中位數，
發出整合的 `matrix_manifest.json`，並打印相對路徑
`cargo xtask fastpq-bench-manifest` 將消耗。 AppleM4、Xeon+RTX 和
Neoverse+MI300 捕獲列表 (`devices/apple-m4-metal.txt`,
`devices/xeon-rtx-sm80.txt`、`devices/neoverse-mi300.txt`) 加上它們的包裝
基準捆綁包
（`fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json`，
`fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json`，
`fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json`) 現已檢查
中，因此每個版本在清單之前都強制執行相同的跨設備中位數
是簽名。 【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-metal.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/x eon-rtx-sm80.txt:1【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【artifacts/fastpq_benchmarks/fastp q_metal_bench_2025-11-07T123018Z_macos14_arm64.json:1【artifacts/fastpq_benchmarks/fastpq_cuda_bench_2025-11-12T09050 1Z_ubuntu24_x86_64.json:1】【工件/fastpq_benchmarks/fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json:1】

---

## 批評總結和公開行動

## 第 7 階段 — 艦隊採用和推廣證據

Stage7 將證明者從“記錄和基準測試”（Stage6）轉變為
“默認為生產車隊做好準備”。重點是遙測攝取，
跨設備捕獲奇偶校驗和操作員證據捆綁，以便 GPU 加速
可以被確定性地強制執行。- **第 7-1 階段 — 車隊遙測攝取和 SLO。 ** 生產儀表板
  (`dashboards/grafana/fastpq_acceleration.json`) 必須接線帶電
  Prometheus/OTel 提供 Alertmanager 覆蓋隊列深度停頓的信息，
  零填充回歸和靜默 CPU 回退。警報包保持在
  `dashboards/alerts/fastpq_acceleration_rules.yml` 並提供相同的證據
  Stage6所需的包。 【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】
  儀表板現在公開 `device_class`、`chip_family`、
  和 `gpu_kind`，讓運營商通過精確的矩陣來推動 Metal 的採用
  標籤（例如，`apple-m4-max`），按 Apple 芯片系列，或按分立與獨立芯片。
  集成 GPU 類，無需編輯查詢。
  使用 `irohad --features fastpq-gpu` 構建的 macOS 節點現在發出信號
  `fastpq_execution_mode_total{device_class,chip_family,gpu_kind,...}`，
  `fastpq_metal_queue_ratio{device_class,chip_family,gpu_kind,queue,metric}`
  （繁忙/重疊比率），以及
  `fastpq_metal_queue_depth{device_class,chip_family,gpu_kind,metric}`
  （limit、max_in_flight、dispatch_count、window_seconds）因此儀表板和
  Alertmanager 規則可以直接讀取金屬信號量佔空比/餘量
  Prometheus，無需等待基準測試包。主機現在導出
  `fastpq_zero_fill_duration_ms{device_class,chip_family,gpu_kind}` 和
  `fastpq_zero_fill_bandwidth_gbps{device_class,chip_family,gpu_kind}` 每當
  LDE 幫助程序將 GPU 評估緩衝區清零，Alertmanager 獲得了
  `FastpqQueueHeadroomLow`（10m 淨空 0.40ms超過15m）規則，因此隊列淨空和
  立即進行零填充回歸頁面操作，而不是等待
  下一個包裝基準。新的 `FastpqCpuFallbackBurst` 頁面級警報跟踪
  超過 5% 的工作負載落在 CPU 後端的 GPU 請求，
  迫使操作員捕獲證據並找出 GPU 瞬時故障的根本原因
  在重試推出之前。 【crates/irohad/src/main.rs:2345】【crates/iroha_telemetry/src/metrics.rs:4436】【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
  SLO 設置現在還通過以下方式強制執行 ≥50% 的金屬佔空比目標：
  `FastpqQueueDutyCycleDrop` 規則，求平均值
  `fastpq_metal_queue_ratio{metric="busy"}` 在 15 分鐘的滾動窗口中，並且
  每當 GPU 工作仍在調度但隊列無法保留時發出警告
  需要佔用。這使得實時遙測合同與
  強制執行 GPU 通道之前的基準測試證據。 【dashboards/alerts/fastpq_acceleration_rules.yml:1】【dashboards/alerts/tests/fastpq_acceleration_rules.test.yml:1】
- **Stage7-2 — 跨設備捕獲矩陣。 ** 新
  `scripts/fastpq/capture_matrix.sh` 構建
  來自每個設備的 `artifacts/fastpq_benchmarks/matrix/matrix_manifest.json`
  `artifacts/fastpq_benchmarks/matrix/devices/` 下的捕獲列表。蘋果M4,
  Xeon+RTX 和 Neoverse+MI300 中值現在與它們一起存在於倉庫中
  打包的捆綁包，因此 `cargo xtask fastpq-bench-manifest` 加載清單
  自動執行 20000 行下限，並應用於每個設備
  在發布捆綁包之前沒有定制 CLI 標誌的延遲/加速限制已批准。 【scripts/fastpq/capture_matrix.sh:1】【artifacts/fastpq_benchmarks/matrix/matrix_manifest.json:1】【artifacts/fastpq_benchmarks/matrix/devices/apple-m4-met al.txt:1【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】【artifacts/fastpq_benchmarks/matrix/devices/neoverse-mi300.txt:1】【xtask/src/fastpq.rs:1】
現在，匯總的不穩定原因與矩陣一起出現：通過
`--reason-summary-out` 到 `scripts/fastpq/geometry_matrix.py` 發出
由主機標籤和源鍵入的失敗/警告原因的 JSON 直方圖
摘要，因此 Stage7-2 審閱者可以在以下位置查看 CPU 回退或缺失的遙測數據：
無需掃描整個 Markdown 表即可一目了然。現在同一個助手
接受 `--host-label chip_family:Chip` （對多個密鑰重複），因此
Markdown/JSON 輸出包括精選的主機標籤列，而不是隱藏
原始摘要中的元數據，使得過濾操作系統版本或
編譯Stage7-2證據包時的Metal驅動版本。 【scripts/fastpq/geometry_matrix.py:1】
幾何掃描還將 ISO8601 `started_at` / `completed_at` 字段標記到
摘要、CSV 和 Markdown 輸出，因此捕獲包可以證明窗口
Stage7-2矩陣合併多個實驗運行時的每個主機。 【scripts/fastpq/launch_geometry_sweep.py:1】
`scripts/fastpq/stage7_bundle.py` 現在將幾何矩陣與
`row_usage/*.json` 快照到單個 Stage7 捆綁包中 (`stage7_bundle.json`
+ `stage7_geometry.md`)，通過驗證傳輸比率
`validate_row_usage_snapshot.py` 和持久主機/env/reason/source 摘要
因此，推出門票可以附加一個確定性的人工製品，而不是雜耍
每個主機表。 【scripts/fastpq/stage7_bundle.py:1】【scripts/fastpq/validate_row_usage_snapshot.py:1】
- **第 7-3 階段 — 運營商採用證據和回滾演習。 ** 新
  `docs/source/fastpq_rollout_playbook.md` 描述了工件包
  （`fastpq_bench_manifest.json`，包裝金屬/CUDA 捕獲，Grafana 導出，
  Alertmanager 快照、回滾日誌）必須隨每個推出票證一起提供
  加上分階段（試點→斜坡→默認）時間線和強制後備演習。
  `ci/check_fastpq_rollout.sh` 驗證這些捆綁包，以便 CI 強制執行 Stage7
  發布前的大門向前推進。發布管道現在可以拉相同的
  通過捆綁到 `artifacts/releases/<version>/fastpq_rollouts/…`
  `scripts/run_release_pipeline.py --fastpq-rollout-bundle <path>`，確保
  簽署的清單和推出證據保留在一起。參考包已存在
  在 `artifacts/fastpq_rollouts/20250215T101500Z/fleet-alpha/canary/` 下保留
  GitHub 工作流程 (`.github/workflows/fastpq-rollout.yml`) 綠色而真實
  審核提交的提交。

### Stage7 FFT 隊列扇出`crates/fastpq_prover/src/metal.rs` 現在實例化一個 `QueuePolicy`
每當主機報告一個命令時，就會自動生成多個 Metal 命令隊列
獨立GPU。集成 GPU 保持單隊列路徑
(`MIN_QUEUE_FANOUT = 1`)，而離散設備默認有兩個隊列，並且僅
當工作負載至少覆蓋 16 列時展開。兩種啟發式方法都可以調整
通過新的 `FASTPQ_METAL_QUEUE_FANOUT` 和 `FASTPQ_METAL_COLUMN_THRESHOLD`
環境變量，調度程序循環 FFT/LDE 批處理
在同一隊列上發出配對後平鋪調度之前的活動隊列
保留訂購保證。 【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:772】【crates/fastpq_prover/src/metal.rs:900】
節點操作員不再需要手動導出這些環境變量：
`iroha_config` 配置文件公開 `fastpq.metal_queue_fanout` 和
`fastpq.metal_queue_column_threshold` 和 `irohad` 通過以下方式應用它們
Metal 後端初始化之前的 `fastpq_prover::set_metal_queue_policy`
艦隊配置文件無需定制啟動包裝即可保持可複制性。 【crates/irohad/src/main.rs:1879】【crates/fastpq_prover/src/lib.rs:60】
現在，每當工作負載剛剛達到時，逆 FFT 批次就會堅持到單個隊列
達到扇出閾值（例如，16 列通道平衡捕獲），
恢復 WP2-D ≥1.0× 奇偶校驗，同時保留大列 FFT/LDE/Poseidon
在多隊列路徑上調度。 【crates/fastpq_prover/src/metal.rs:2018】

輔助測試執行隊列策略限制和解析器驗證，以便 CI 可以
證明 Stage7 啟發式，無需在每個構建器上安裝 GPU 硬件，
並且特定於 GPU 的測試強制扇出覆蓋以保持重放覆蓋範圍
與新的默認值同步。 【crates/fastpq_prover/src/metal.rs:2163】【crates/fastpq_prover/src/metal.rs:2236】

### Stage7-1 設備標籤和警報合同

`scripts/fastpq/wrap_benchmark.py` 現在在 macOS 捕獲上探測 `system_profiler`
在每個包裝的基準測試中託管和記錄硬件標籤，以便艦隊遙測
捕獲矩陣可以根據設備進行旋轉，無需定制電子表格。一個
20000 行金屬捕獲現在包含以下條目：

```json
"labels": {
  "device_class": "apple-m4-pro",
  "chip_family": "m4",
  "chip_bin": "pro",
  "gpu_kind": "integrated",
  "gpu_vendor": "apple",
  "gpu_bus": "builtin",
  "gpu_model": "Apple M4 Pro"
}
```

這些標籤與 `benchmarks.zero_fill_hotspots` 一起被攝取，
`benchmarks.metal_dispatch_queue`所以Grafana快照，捕獲矩陣
(`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`) 和警報管理器
所有證據都同意產生這些指標的硬件類別。的
當實驗室主機缺少 `--label` 標誌時，仍然允許手動覆蓋
`system_profiler`，但自動探測的標識符現在涵蓋 AppleM1–M4 和
開箱即用的獨立 PCIe GPU。 【scripts/fastpq/wrap_benchmark.py:1】

Linux 捕獲得到相同的處理：`wrap_benchmark.py` 現在檢查
`/proc/cpuinfo`、`nvidia-smi`/`rocm-smi` 和 `lspci`，以便 CUDA 和 OpenCL 運行
派生 `cpu_model`、`gpu_model` 和規範的 `device_class` (`xeon-rtx-sm80`
對於 Stage7 CUDA 主機，對於 MI300A 實驗室為 `neoverse-mi300`）。運營商可以
仍然覆蓋自動檢測的值，但 Stage7 證據包不再
需要手動編輯以使用正確的設備標記 Xeon/Neoverse 捕獲
元數據。在運行時，每個主機設置 `fastpq.device_class`、`fastpq.chip_family` 和
`fastpq.gpu_kind`（或相應的 `FASTPQ_*` 環境變量）到
與捕獲包中出現的矩陣標籤相同，因此 Prometheus 導出
`fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}` 和
FASTPQ 加速儀表板可以按三個軸中的任何一個進行過濾。的
Alertmanager 規則聚合在同一標籤集上，讓操作員可以繪製圖表
每個硬件配置文件的採用、降級和回退，而不是單個
車隊比例。 【crates/iroha_config/src/parameters/user.rs:1224】【dashboards/grafana/fastpq_acceleration.json:1】【dashboards/alerts/fastpq_acceleration_rules.yml:1】

遙測 SLO/警報合約現在將捕獲的指標與 Stage7 聯繫起來
蓋茨。下表總結了信號和執行點：

|信號|來源 |目標/觸發|執法|
| ------ | ------ | ---------------- | ----------- |
| GPU採用率| Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", device_class="…", chip_family="…", gpu_kind="…", backend="metal"}` | ≥95% 的每（device_class、chip_family、gpu_kind）分辨率必須落在 `resolved="gpu", backend="metal"` 上；當任何三元組在 15m 內下降到 50% 以下時的頁面 | `FastpqMetalDowngrade` 警報（頁面）【dashboards/alerts/fastpq_acceleration_rules.yml:1】 |
|後端間隙| Prometheus `fastpq_execution_mode_total{backend="none", device_class="…", chip_family="…", gpu_kind="…"}` |每個三元組必須保持為 0；任何持續 (>10m) 爆發後發出警告 | `FastpqBackendNoneBurst` 警報（警告）【dashboards/alerts/fastpq_acceleration_rules.yml:21】 |
| CPU回退率| Prometheus `fastpq_execution_mode_total{requested=~"auto|gpu", backend="cpu", device_class="…", chip_family="…", gpu_kind="…"}` |對於任何三元組，≤5% 的 GPU 請求的證明可能會登陸 CPU 後端；當三元組超過 5% ≥10m 時的頁面 | `FastpqCpuFallbackBurst` 警報（頁面）【dashboards/alerts/fastpq_acceleration_rules.yml:32】 |
|金屬隊列佔空比| Prometheus `fastpq_metal_queue_ratio{metric="busy", device_class="…", chip_family="…", gpu_kind="…"}` |每當 GPU 作業排隊時，滾動 15m 平均值必須保持 ≥50%；當 GPU 請求持續存在時利用率低於目標時發出警告 | `FastpqQueueDutyCycleDrop` 警報（警告）【dashboards/alerts/fastpq_acceleration_rules.yml:98】 |
|隊列深度和零填充預算|包裝基準 `metal_dispatch_queue` 和 `zero_fill_hotspots` 塊 |對於規範的 20000 行跟踪，`max_in_flight` 必須至少比 `limit` 低一個槽，並且 LDE 零填充平均值必須保持 ≤0.4ms (≈80GB/s)；任何回歸都會阻礙推出捆綁包 |通過 `scripts/fastpq/wrap_benchmark.py` 輸出進行審查並附加到 Stage7 證據包 (`docs/source/fastpq_rollout_playbook.md`)。 |
|運行時隊列淨空| Prometheus `fastpq_metal_queue_depth{metric="limit|max_in_flight", device_class="…", chip_family="…", gpu_kind="…"}` |每個三元組為 `limit - max_in_flight ≥ 1`； 10m後無淨空警告 | `FastpqQueueHeadroomLow` 警報（警告）【dashboards/alerts/fastpq_acceleration_rules.yml:41】 |
|運行時零填充延遲 | Prometheus `fastpq_zero_fill_duration_ms{device_class="…", chip_family="…", gpu_kind="…"}` |最新的填零樣本必須保持≤0.40ms（Stage7限制）| `FastpqZeroFillRegression` 警報（頁面）【dashboards/alerts/fastpq_acceleration_rules.yml:58】 |包裝器直接強制執行零填充行。通行證
`--require-zero-fill-max-ms 0.40` 至 `scripts/fastpq/wrap_benchmark.py` 及其
當工作台 JSON 缺乏零填充遙測或最熱時將失敗
零填充樣本超出了 Stage7 預算，從而阻止了推出捆綁包
在沒有強制證據的情況下發貨。 【scripts/fastpq/wrap_benchmark.py:1008】

#### Stage7-1 警報處理清單

上面列出的每個警報都會提供特定的待命演習，以便操作員收集
發布包所需的相同工件：

1. **`FastpqQueueHeadroomLow`（警告）。 ** 運行即時 Prometheus 查詢
   對於 `fastpq_metal_queue_depth{metric=~"limit|max_in_flight",device_class="<matrix>"}` 和
   從 `fastpq-acceleration` 捕獲 Grafana“隊列淨空”面板
   板。將查詢結果記錄在
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_headroom.prom`
   與警報 ID 一起，以便發布包證明警告是
   在隊列飢餓之前確認。 【dashboards/grafana/fastpq_acceleration.json:1】
2. **`FastpqZeroFillRegression`（頁）。 ** 檢查
   `fastpq_zero_fill_duration_ms{device_class="<matrix>"}` 並且，如果度量為
   有噪音，在最新的工作台 JSON 上重新運行 `scripts/fastpq/wrap_benchmark.py`
   刷新 `zero_fill_hotspots` 塊。附上 promQL 輸出，
   截圖，並將bench文件刷新到rollout目錄；這創造了
   `ci/check_fastpq_rollout.sh` 在發布期間期望的相同證據
   驗證。 【scripts/fastpq/wrap_benchmark.py:1】【ci/check_fastpq_rollout.sh:1】
3. **`FastpqCpuFallbackBurst`（頁）。 ** 確認
   `fastpq_execution_mode_total{requested="gpu",backend="cpu"}` 超過 5%
   地板，然後採樣 `irohad` 日誌以獲取相應的降級消息
   （`telemetry::fastpq.execution_mode resolved="cpu"`）。存儲 promQL 轉儲
   加上 `metrics_cpu_fallback.prom`/`rollback_drill.log` 中的日誌摘錄，因此
   捆綁包展示了影響和運營商的認可。
4. **證據打包。 ** 任何警報清除後，重新運行中的 Stage7-3 步驟
   部署手冊（Grafana 導出、警報快照、回滾演練）以及
   在重新附加之前通過 `ci/check_fastpq_rollout.sh` 重新驗證捆綁包
   前往發行票。 【docs/source/fastpq_rollout_playbook.md:114】

喜歡自動化的操作員可以運行
`scripts/fastpq/capture_alert_evidence.sh --device-class <label> --out <bundle-dir>`
查詢 Prometheus API 的隊列餘量、零填充和 CPU 回退
上面列出的指標；助手寫入捕獲的 JSON（前綴為
原始 promQL）轉換為 `metrics_headroom.prom`、`metrics_zero_fill.prom`，以及
`metrics_cpu_fallback.prom` 在所選的推出目錄下，因此這些文件
可以附加到捆綁包中，無需手動調用curl。`ci/check_fastpq_rollout.sh` 現在強制執行隊列餘量和零填充
直接預算。它解析由引用的每個 `metal` 基準
`fastpq_bench_manifest.json`，檢查
`benchmarks.metal_dispatch_queue.{limit,max_in_flight}` 和
`benchmarks.zero_fill_hotspots[]`，並且當淨空下降時捆綁失敗
低於一個插槽或當任何 LDE 熱點報告 `mean_ms > 0.40` 時。這保持了
CI 中的 Stage7 遙測防護，與在
Grafana快照及發布證據。 【ci/check_fastpq_rollout.sh#L1】
作為同一驗證過程的一部分，腳本現在堅持每個包裝的
基準測試帶有自動檢測的硬件標籤（`metadata.labels.device_class`
和 `metadata.labels.gpu_kind`）。缺少這些標籤的捆綁包會立即失敗，
保證發布工件、Stage7-2 矩陣清單和運行時
儀表板均引用完全相同的設備類名稱。

Grafana“最新基準”面板和相關的推出捆綁包現在引用
`device_class`、零填充預算、隊列深度快照等隨叫隨到的工程師
可以將生產遙測與標誌期間使用的確切捕獲類別相關聯
關閉。未來的矩陣條目繼承相同的標籤，這意味著 Stage7-2 設備
列表和 Prometheus 儀表板共享 AppleM4 的單一命名方案，
M3 Max 和即將推出的 MI300/RTX 捕獲。

### Stage7-1 艦隊遙測操作手冊

默認情況下啟用 GPU 通道之前請遵循此清單，以便車隊遙測
和 Alertmanager 規則反映了發布準備期間捕獲的相同證據：

1. **標籤捕獲和運行時主機。 ** `python3 scripts/fastpq/wrap_benchmark.py`
   已發出 `metadata.labels.device_class`、`chip_family` 和 `gpu_kind`
   對於每個包裝好的 JSON。使這些標籤與
   `fastpq.{device_class,chip_family,gpu_kind}`（或
   `FASTPQ_{DEVICE_CLASS,CHIP_FAMILY,GPU_KIND}` 環境變量）在 `iroha_config` 內
   所以運行時指標發布
   `fastpq_execution_mode_total{device_class="…",chip_family="…",gpu_kind="…"}`
   以及具有相同標識符的 `fastpq_metal_queue_*` 儀表
   在 `artifacts/fastpq_benchmarks/matrix/devices/*.txt` 中。上演新劇時
   類，通過重新生成矩陣清單
   `scripts/fastpq/capture_matrix.sh --devices artifacts/fastpq_benchmarks/matrix/devices`
   因此 CI 和儀表板可以理解附加標籤。
2. **驗證隊列計量和採用指標。 ** 運行 `irohad --features fastpq-gpu`
   在 Metal 主機上並抓取遙測端點以確認實時隊列
   儀表正在出口：

   ```bash
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_metal_queue_(ratio|depth)'
   curl -sf http://$IROHA_PROM/metrics | rg 'fastpq_execution_mode_total'
   ```第一個命令證明信號量採樣器正在發出 `busy`，
   `overlap`、`limit` 和 `max_in_flight` 系列和第二個顯示是否
   每個設備類別都解析為 `backend="metal"` 或回退到
   `backend="cpu"`。之前通過 Prometheus/OTel 連接抓取目標
   導入儀表板，以便 Grafana 可以立即繪製車隊視圖。
3. **安裝儀表板+警報包。 ** 導入
   `dashboards/grafana/fastpq_acceleration.json` 轉換為 Grafana（保留
   內置設備類、芯片系列和 GPU 類型模板變量）並加載
   `dashboards/alerts/fastpq_acceleration_rules.yml`一起進入Alertmanager
   及其單元測試夾具。規則包附帶 `promtool` 線束；跑
   `promtool test rules dashboards/alerts/tests/fastpq_acceleration_rules.test.yml`
   每當規則發生變化以證明 `FastpqMetalDowngrade` 且
   `FastpqBackendNoneBurst` 仍然在記錄的閾值下觸發。
4. **通過證據包進行釋放。 ** 保留
   `docs/source/fastpq_rollout_playbook.md` 生成部署時很方便
   提交，以便每個捆綁包都帶有打包的基準，Grafana 導出，
   警報包、隊列遙測證明和回滾日誌。 CI 已經強制執行
   合約：`make check-fastpq-rollout`（或調用
   `ci/check_fastpq_rollout.sh --bundle <path>`) 驗證捆綁包，重新運行
   警報測試，並在隊列淨空或零填充時拒絕註銷
   預算回歸。
5. **將警報與修復聯繫起來。 ** 當 Alertmanager 尋呼時，使用 Grafana
   板和步驟 2 中的原始 Prometheus 計數器，以確認是否
   降級源於隊列飢餓、CPU 回退或 backend=none 突發。
運行手冊位於
本文檔加上 `docs/source/fastpq_rollout_playbook.md`；更新
釋放帶有相關 `fastpq_execution_mode_total` 的票證，
`fastpq_metal_queue_ratio` 和 `fastpq_metal_queue_depth` 一起摘錄
包含指向 Grafana 面板和警報快照的鏈接，以便審閱者可以查看
到底是哪個 SLO 觸發的。

### WP2-E — 逐步金屬分析快照

`scripts/fastpq/src/bin/metal_profile.rs` 總結了包裹的 Metal 捕獲
因此可以隨著時間的推移跟踪低於 900 毫秒的目標（運行
`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin metal_profile -- <capture.json>`）。
新的 Markdown 助手
`scripts/fastpq/metal_capture_summary.py fastpq_metal_bench_20k_latest.json --label "20k snapshot (pre-override)"`
生成下面的階段表（它打印 Markdown 以及文本
摘要，以便 WP2-E 票據可以逐字嵌入證據）。跟踪兩次捕獲
現在：

> **新的 WP2-E 儀器：** `fastpq_metal_bench --gpu-probe ...` 現在發出
> 檢測快照（請求/解析執行模式，`FASTPQ_GPU`
> 覆蓋、檢測到的後端和枚舉的 Metal 設備/註冊表 ID）
> 在任何內核運行之前。每當強制 GPU 仍在運行時捕獲此日誌
> 回退到 CPU 路徑，以便證據包記錄主機看到的內容
> `MTLCopyAllDevices` 返回零以及哪些覆蓋在
> 基準測試。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:603】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2616】> **舞台捕捉助手：** `cargo xtask fastpq-stage-profile --trace --out-dir artifacts/fastpq_stage_profiles/<label>`
> 現在分別為 FFT、LDE 和 Poseidon 驅動 `fastpq_metal_bench`，
> 將原始 JSON 輸出存儲在每個階段目錄下，並發出單個
> `stage_profile_summary.json` 捆綁包，記錄 CPU/GPU 計時、隊列深度
> 遙測、列分段統計、內核配置文件和相關跟踪
> 文物。通過 `--stage fft --stage lde --stage poseidon` 來定位子集，
> `--trace-template "Metal System Trace"` 選擇特定的 xctrace 模板，
> 和 `--trace-dir` 將 `.trace` 捆綁包路由到共享位置。附上
> 摘要 JSON 加上每個 WP2-E 問題生成的跟踪文件，以便審閱者
> 可以比較隊列佔用率 (`metal_dispatch_queue.*`)、重疊率和
> 無需手動探索多個運行即可捕獲發射幾何形狀
> `fastpq_metal_bench` 調用。 【xtask/src/fastpq.rs:721】【xtask/src/main.rs:3187】

> **隊列/分期證據助手 (2026-05-09):** `scripts/fastpq/profile_queue.py` 現在
> 攝取一個或多個 `fastpq_metal_bench` JSON 捕獲並發出 Markdown 表和
> 機器可讀的摘要 (`--markdown-out/--json-out`)，因此隊列深度、重疊率和
> 主機端分段遙測可以與每個 WP2-E 製品一起使用。跑步
> `python3 scripts/fastpq/profile_queue.py fastpq_metal_bench_poseidon.json fastpq_metal_bench_20k_new.json --json-out artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.json` 生成了下表並標記了已存檔的 Metal 捕獲仍然報告
> `dispatch_count = 0` 和 `column_staging.batches = 0` — WP2-E.1 保持打開狀態，直到金屬
> 儀器在啟用遙測的情況下重建。生成的 JSON/Markdown 工件已上線
> 在 `artifacts/fastpq_benchmarks/fastpq_queue_profile_20260509.{json,md}` 下進行審核。
> 助手現在（2026-05-19）還顯示了 Poseidon 管道遙測（`pipe_depth`，
> `batches`、`chunk_columns` 和 `fallbacks`) 在 Markdown 表和 JSON 摘要中，
> 因此 WP2-E.4/6 審閱者可以證明 GPU 是否保持在流水線路徑上以及是否存在任何問題
> 在未打開原始捕獲的情況下發生回退。 【scripts/fastpq/profile_queue.py:1】> **階段概況摘要（2026-05-30）：** `scripts/fastpq/stage_profile_report.py` 消耗
> 由 `cargo xtask fastpq-stage-profile` 發出的 `stage_profile_summary.json` 捆綁包以及
> 呈現 Markdown 和 JSON 摘要，以便 WP2-E 審閱者可以將證據複製到票證中
> 無需手動轉錄時間。調用
> `python3 scripts/fastpq/stage_profile_report.py artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.json --label "m3-lab" --markdown-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.md --json-out artifacts/fastpq_stage_profiles/<stamp>/stage_profile_summary.jsonl`
> 生成列出 GPU/CPU 平均值、加速增量、跟踪覆蓋範圍的確定性表
> 每個階段的遙測間隙。 JSON 輸出鏡像表並記錄每個階段的問題標籤
>（`trace missing`、`queue telemetry missing` 等），因此治理自動化可以區分主機
> WP2-E.1 至 WP2-E.6 中引用的運行。
> **主機/設備重疊保護 (2026-06-04):** `scripts/fastpq/profile_queue.py` 現在註釋
> FFT/LDE/Poseidon 等待比率以及每級展平/等待毫秒總數並發出
> 每當 `--max-wait-ratio <threshold>` 檢測到重疊不良時就會出現問題。使用
> `python3 scripts/fastpq/profile_queue.py --max-wait-ratio 0.20 fastpq_metal_bench_20k_latest.json --markdown-out artifacts/fastpq_benchmarks/<stamp>/queue.md`
> 捕獲具有明確等待率的 Markdown 表和 JSON 包，以便 WP2-E.5 票證
> 可以顯示雙緩衝窗口是否保持 GPU 供電。純文本控制台輸出也
> 列出每相比率，使隨叫隨到的調查變得更加容易。
> **遙測保護 + 運行狀態 (2026-06-09):** `fastpq_metal_bench` 現在發出 `run_status` 塊
>（後端標籤、調度計數、原因）和新的 `--require-telemetry` 標誌運行失敗
> 每當 GPU 計時或隊列/分段遙測丟失時。 `profile_queue.py` 渲染運行
> 作為專用欄的狀態並在問題列表中顯示非 `ok` 狀態，以及
> `launch_geometry_sweep.py` 將相同的狀態線程化到警告/分類中，因此矩陣不能
> 較長的承認捕獲會默默地回退到 CPU 或跳過隊列檢測。
> **Poseidon/LDE 自動調整 (2026-06-12)：** `metal_config::poseidon_batch_multiplier()` 現已擴展
> 使用 Metal 工作集提示和 `lde_tile_stage_target()` 提高了獨立 GPU 上的圖塊深度。
> 應用的乘數和拼貼限制包含在 `metal_heuristics` 塊中
> `fastpq_metal_bench` 輸出並由 `scripts/fastpq/metal_capture_summary.py` 渲染，因此 WP2-E
> 包記錄每次捕獲中使用的確切管道旋鈕，而無需挖掘原始 JSON。 【crates/fastpq_prover/src/metal_config.rs:1】【crates/fastpq_prover/src/metal.rs:2833】【scripts/fastpq/metal_capture_summary.py:1】

|標籤|調度|忙碌 |重疊|最大深度| FFT 展平 | FFT 等待 | FFT 等待% | LDE 扁平化 | LDE 等待 | LDE 等待% |波塞冬壓扁|海神等等|波塞冬等待% |管道深度|管材批次|管道後備|
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| fastpq_metal_bench_poseidon | fastpq_metal_bench_poseidon | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |
| fastpq_metal_bench_20k_new | fastpq_metal_bench_20k_new | 0 | 0.0% | 0.0% | 0 | – | – | – | – | – | – | – | – | – | – | – | – |

#### 20k 快照（覆蓋前）

`fastpq_metal_bench_20k_latest.json`|舞台|專欄 |輸入長度| GPU 平均值（毫秒）| CPU 平均值（毫秒）| GPU 份額 |加速| Δ CPU（毫秒）|
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
|快速傅里葉變換 | 16 | 16 32768 | 130.986 毫秒（115.761–167.755）| 112.616 毫秒（95.335–132.929）| 2.4% | 0.860× | −18.370 |
|快速傅里葉變換 | 16 | 16 32768 | 129.296 毫秒（111.127–142.955）| 158.144 毫秒（126.847–237.887）| 2.4% | 1.223× | +28.848 |
| LDE | 16 | 16 262144 | 262144 1570.656 毫秒（1544.397–1584.502）| 1752.523 毫秒（1548.807–2191.930）| 29.2% | 1.116× | +181.867 |
|波塞冬 | 16 | 16 524288 | 524288 3548.329 毫秒（3519.881–3576.041）| 3642.706 毫秒（3539.055–3758.279）| 66.0% | 1.027× | +94.377 |

主要觀察結果：1. GPU 總計為 5.379 秒，比 900 毫秒目標**4.48 秒。波塞冬
   哈希仍然在運行時佔據主導地位（約 66%），LDE 內核位居第二
   位置（≈29%），因此WP2-E需要同時攻擊Poseidon管道深度和
   CPU 回退消失之前的 LDE 內存駐留/平鋪計劃。
2. 即使 IFFT 在標量上 >1.22×，FFT 仍然是回歸 (0.86×)
   路徑。我們需要一次發射幾何掃描
   （`FASTPQ_METAL_{FFT,LDE}_COLUMNS` + `FASTPQ_METAL_QUEUE_FANOUT`）了解
   是否可以在不傷害已經更好的情況下挽救 FFT 佔用率
   IFFT 時序。 `scripts/fastpq/launch_geometry_sweep.py` 助手現在驅動
   這些實驗端到端：傳遞以逗號分隔的覆蓋（例如，
   `--fft-columns 16,32 --queue-fanout 1,2` 和
   `--poseidon-lanes auto,256`），它將調用
   `fastpq_metal_bench` 對於每個組合，將 JSON 有效負載存儲在
   `artifacts/fastpq_geometry/<timestamp>/`，並保留 `summary.json` 捆綁包
   描述每次運行的隊列比率、FFT/LDE 啟動選擇、GPU 與 CPU 時序、
   以及主機元數據（主機名/標籤、平台三元組、檢測到的設備
   類、GPU 供應商/型號），因此跨設備比較具有確定性
   出處。助手現在還在旁邊寫入 `reason_summary.json`
   默認情況下summary，使用與幾何矩陣相同的分類器進行roll
   CPU 回退和遙測丟失。使用 `--host-label staging-m3` 進行標記
   來自共享實驗室的捕獲。
   配套的 `scripts/fastpq/geometry_matrix.py` 工具現在可以攝取一個或
   更多摘要捆綁包 (`--summary hostA/summary.json --summary hostB/summary.json`)
   並發出 Markdown/JSON 表，將每個啟動形狀標記為*穩定*
   （FFT/LDE/Poseidon GPU 計時捕獲）或*不穩定*（超時、CPU 回退、
   非金屬後端，或缺少遙測）與主機列一起。的
   表現在包括已解析的 `execution_mode`/`gpu_backend` 以及
   `Reason` 列，因此 CPU 回退和缺少 GPU 時序在
   即使存在計時塊，Stage7 矩陣也是如此；摘要行很重要
   穩定運行與總運行。通過 `--operation fft|lde|poseidon_hash_columns`
   當掃描需要隔離單個階段時（例如，分析
   Poseidon 單獨）並保留 `--extra-args` 空閒用於特定於工作台的標誌。
   助手接受任何
   命令前綴（默認為 `cargo run … fastpq_metal_bench`）加上可選
   `--halt-on-error` / `--timeout-seconds` 提供保護，以便性能工程師可以
   在不同的機器上重現掃描，同時收集可比的，
   Stage7 的多設備證據包。
3、`metal_dispatch_queue`報`dispatch_count = 0`，所以隊列佔用
   即使 GPU 內核運行，遙測也丟失。 Metal 運行時現在使用
   獲取/釋放隊列/列分段切換的柵欄，以便工作線程
   觀察儀器標誌，幾何矩陣報告會調用
   當 FFT/LDE/Poseidon GPU 時序缺失時，啟動形狀不穩定。保留
   將 Markdown/JSON 矩陣附加到 WP2-E 票證，以便審閱者可以看到
   一旦隊列遙測可用，哪些組合仍然失敗。`run_status` 防護和 `--require-telemetry` 標誌現在無法捕獲
   每當 GPU 計時缺失或隊列/暫存遙測缺失時，
   dispatch_count=0 次運行不會再被忽視地滑入 WP2-E 捆綁包中。
   `fastpq_metal_bench` 現在公開 `--require-gpu`，並且
   `launch_geometry_sweep.py` 默認啟用它（選擇退出
   `--allow-cpu-fallback`)，因此 CPU 回退和金屬檢測故障中止
   立即，而不是用非GPU遙測污染Stage7矩陣。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs】【scripts/fastpq/launch_geometry_sweep.py】
4. 之前的零填充指標也因為同樣的原因消失了；擊劍修復
   保持主機儀器處於活動狀態，因此下一次捕獲應包括
   `zero_fill` 塊沒有合成計時。

#### 20k 快照，`FASTPQ_GPU=gpu`

`fastpq_metal_bench_20k_refresh.json`

|舞台|專欄 |輸入長度 | GPU 平均值（毫秒）| CPU 平均值（毫秒）| GPU 份額 |加速| Δ CPU（毫秒）|
| ---| ---: | ---: | ---: | ---: | ---: | ---: | ---: |
|快速傅里葉變換 | 16 | 16 32768 | 79.951 毫秒（65.645–93.193）| 83.289 毫秒（59.956–107.585）| 0.3% | 1.042× | +3.338 |
|快速傅里葉變換 | 16 | 16 32768 | 78.605 毫秒 (69.986–83.726) | 93.898 毫秒（80.656–119.625）| 0.3% | 1.195×| +15.293 |
| LDE | 16 | 16 262144 | 262144 657.673 毫秒（619.219–712.367）| 669.537 毫秒（619.716–723.285）| 2.1% | 1.018× | +11.864 |
|波塞冬 | 16 | 16 524288 | 524288 30004.898 毫秒（27284.117–32945.253）| 29087.532 毫秒（24969.810–33020.517）| 97.4% | 0.969× | −917.366 |

觀察結果：

1. 即使使用 `FASTPQ_GPU=gpu`，此捕獲仍然反映了 CPU 回退：
   每次迭代約 30 秒，`metal_dispatch_queue` 停留在零。當
   設置了覆蓋，但主機無法發現 Metal 設備，CLI 現在退出
   在運行任何內核並打印請求/解析模式以及
   後端標籤，以便工程師可以判斷是否是檢測、權利或
   Metallib 查找導致降級。運行`fastpq_metal_bench --gpu-probe
   --rows …` with `FASTPQ_DEBUG_METAL_ENUM=1` 捕獲枚舉日誌並
   在重新運行分析器之前修復底層檢測問題。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1965】【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:2636】
2. 零填充遙測現在記錄真實樣本（32MiB 上 18.66ms），證明
   防護修復有效，但隊列增量在 GPU 調度之前仍然不存在
   成功。
3. Because the backend keeps downgrading, the Stage 7 telemetry gate is still
   阻塞：隊列淨空證據和海神重疊需要真正的 GPU
   跑。

這些捕獲現在錨定了 WP2-E 積壓工作。下一步行動：收集分析器
火焰圖和隊列日誌（一旦後端在 GPU 上執行），目標
重新審視 FFT 之前的 Poseidon/LDE 瓶頸，並解鎖後端回退
所以 Stage7 遙測擁有真實的 GPU 數據。

### 優勢
- 增量分段、跟踪優先設計、透明的 STARK 堆棧。### 高優先級行動項目
1. 實施包裝/訂購裝置並更新 AIR 規範。
2. 最終確定 Poseidon2 提交 `3f2b7fe` 並發布示例 SMT/查找向量。
3. 將工作示例（`lookup_grand_product.md`、`smt_update.md`）與夾具一起維護。
4. 添加附錄 A，記錄健全性推導和 CI 拒絕方法。

### 已解決的設計決策
- P1 中禁用 ZK（僅限正確性）；在未來階段重新審視。
- 權限表根源自治理狀態；批次將表視為只讀並通過查找證明成員身份。
- 缺少關鍵證明使用零葉加鄰居見證和規範編碼。
- 刪除語義 = 規範鍵空間內的葉值設置為零。

使用本文檔作為規範參考；將其與源代碼、固定裝置和附錄一起更新以避免漂移。

## 附錄 A — 穩健性推導

本附錄解釋了“健全性和 SLO”表的生成方式以及 CI 如何強制執行前面提到的 ≥128 位下限。

### 符號
- `N_trace = 2^k` — 排序並填充為 2 的冪後的走線長度。
- `b` — 膨脹係數 (`N_eval = N_trace × b`)。
- `r` — FRI 數量（規範集為 8 或 16）。
- `ℓ` — FRI 減少數量（`layers` 列）。
- `q` — 驗證者查詢每個證明（`queries` 列）。
- `ρ` — 列規劃器報告的有效碼率：`ρ = max_i(degree_i / domain_i)` 超過第一輪 FRI 的約束。

Goldilocks 基域具有 `|F| = 2^64 - 2^32 + 1`，因此 Fiat-Shamir 碰撞受 `q / 2^64` 限制。 Grinding增加了一個正交的`2^{-g}`因子，`g = 23`用於`fastpq-lane-balanced`，`g = 21`用於延遲配置文件。 【crates/fastpq_isi/src/params.rs:65】

### 分析界限

對於恆定速率 DEEP-FRI，統計故障概率滿足

```
p_fri ≤ Σ_{j=0}^{ℓ-1} ρ^{q} = ℓ · ρ^{q}
```

因為每層都會以相同的因子 `r` 減少多項式次數和域寬度，從而保持 `ρ` 恆定。表的 `est bits` 列報告 `⌊-log₂ p_fri⌋`； Fiat-Shamir 和磨削可提供額外的安全裕度。

### 規劃器輸出和工作計算

對代表性批次運行 Stage1 列規劃器會產生：

|參數設置| `N_trace` | `b` | `N_eval` | `ρ`（規劃師）|有效度(`ρ × N_eval`)| `ℓ` | `q` | `-log₂(ℓ · ρ^{q})` |
| ------------- | --------- | ---| -------- | ------------- | -------------------------------- | ---| ---| ------------------ |
|平衡20k批次| `2^15` | 8 | 262144 | 262144 0.077026 | 0.077026 20192 | 20192 5 | 52 | 52 190 位 |
|吞吐量 65k 批次 | `2^16` | 8 | 524288 | 524288 0.200208 | 104967 | 104967 6 | 58 | 58 132 位 |
|延遲 131k 批次 | `2^17` | 16 | 16 2097152 | 2097152 0.209492 | 439337 | 439337 5 | 64 | 64 142 位 |示例（平衡 20k 批次）：
1. `N_trace = 2^15`，所以`N_eval = 2^15 × 8 = 2^18`。
2. Planner 儀器報告 `ρ = 0.077026`，因此 `p_fri = 5 × ρ^{52} ≈ 6.4 × 10^{-58}`。
3. `-log₂ p_fri = 190 bits`，與表條目匹配。
4. Fiat-Shamir 碰撞最多添加 `2^{-58.3}`，研磨 (`g = 23`) 減去另一個 `2^{-23}`，使總穩健性保持在 160 位以上。

### CI 拒絕採樣工具

每次 CI 運行都會執行蒙特卡羅工具，以確保經驗測量值保持在分析界限的 ±0.6 位以內：
1. 繪製規範參數集並合成具有匹配行數的 `TransitionBatch`。
2. 構建跟踪，翻轉隨機選擇的約束（例如，干擾查找總乘積或 SMT 同級），並嘗試生成證明。
3. 重新運行驗證器，重新採樣 Fiat-Shamir 挑戰（包括研磨），並記錄篡改證明是否被拒絕。
4. 對每個參數集 16384 個種子重複此操作，並將觀察到的拒絕率的 99% Clopper-Pearson 下限轉換為比特。

如果測量的下限低於 128 位，則作業會立即失敗，因此在合併之前會捕獲規劃器、折疊循環或轉錄連線中的回歸。

## 附錄 B — 域根推導

Stage0 將跟踪和評估生成器固定到 Poseidon 派生常量，因此所有實現共享相同的子組。

### 程序
1. **種子選擇。 ** 將 UTF-8 標籤 `fastpq:v1:domain_roots` 吸收到 FASTPQ 中其他地方使用的 Poseidon2 海綿中（狀態寬度 = 3，速率 = 2，四個完整輪 + 57 個部分輪）。輸入重用來自 `pack_bytes` 的 `[len, limbs…]` 編碼，生成基本生成器 `g_base = 7`。 【crates/fastpq_prover/src/packing.rs:44】【scripts/fastpq/src/bin/poseidon_gen.rs:1】
2. **跡線發生器。 ** 計算 `trace_root = g_base^{(p-1)/2^{trace_log_size}} mod p` 並在半功率不為 1 時驗證 `trace_root^{2^{trace_log_size}} = 1`。
3. **LDE 生成器。 ** 對 `lde_log_size` 重複相同的求冪運算，得到 `lde_root`。
4. **陪集選擇。 ** Stage0 使用基本子組 (`omega_coset = 1`)。未來的陪集可以吸收額外的標籤，例如 `fastpq:v1:domain_roots:coset`。
5. **排列大小。 ** 顯式保留 `permutation_size`，以便調度程序永遠不會從 2 的隱式冪推斷填充規則。

### 複製和驗證
- 工具：`cargo run --manifest-path scripts/fastpq/Cargo.toml --bin poseidon_gen -- domain-roots` 發出 Rust 片段或 Markdown 表（參見 `--format table`、`--seed`、`--filter`）。 【scripts/fastpq/src/bin/poseidon_gen.rs:1】
- 測試：`canonical_sets_meet_security_target` 使規範參數集與已發布的常量保持一致（非零根、放大/元數配對、排列大小），因此 `cargo test -p fastpq_isi` 立即捕獲漂移。 【crates/fastpq_isi/src/params.rs:138】
- 事實來源：每當引入新參數包時，都會更新 Stage0 表和 `fastpq_isi/src/params.rs`。

## 附錄 C — 承諾渠道詳細信息### Streaming Poseidon 承諾流程
Stage2 定義了證明者和驗證者共享的確定性跟踪承諾：
1. **標準化轉換。 ** `trace::build_trace` 對每個批次進行排序，將其填充到 `N_trace = 2^{⌈log₂ rows⌉}`，並按以下順序發出列向量。 【crates/fastpq_prover/src/trace.rs:123】
2. **哈希列。 ** `trace::column_hashes` 通過標記為 `fastpq:v1:trace:column:<name>` 的專用 Poseidon2 海綿對列進行流式傳輸。當 `fastpq-prover-preview` 功能處於活動狀態時，相同的遍歷會回收後端所需的 IFFT/LDE 係數，因此不會分配額外的矩陣副本。 【crates/fastpq_prover/src/trace.rs:474】
3. **提升到 Merkle 樹中。 ** `trace::merkle_root` 將列摘要與標記為 `fastpq:v1:trace:node` 的 Poseidon 節點折疊，每當級別具有奇數扇出時復制最後一個葉子以避免特殊情況。 【crates/fastpq_prover/src/trace.rs:656】
4. **完成摘要。 ** `digest::trace_commitment` 使用相同的 `[len, limbs…]` 編碼為域標記 (`fastpq:v1:trace_commitment`)、參數名稱、填充維度、列摘要和 Merkle 根添加前綴，然後使用 SHA3-256 對有效負載進行哈希處理，然後將其嵌入`Proof::trace_commitment`.【crates/fastpq_prover/src/digest.rs:25】

驗證者在對 Fiat-Shamir 挑戰進行採樣之前重新計算相同的摘要，因此不匹配會在任何空缺之前中止證明。

### Poseidon 後備控制- 證明者現在公開了專用的 Poseidon 管道覆蓋（`zk.fastpq.poseidon_mode`、env `FASTPQ_POSEIDON_MODE`、CLI `--fastpq-poseidon-mode`），因此操作員可以在無法達到 Stage7 <900ms 目標的設備上將 GPU FFT/LDE 與 CPU Poseidon 哈希混合。支持的值鏡像執行模式旋鈕（`auto`、`cpu`、`gpu`），未指定時默認為全局模式。運行時通過通道配置 (`FastpqPoseidonMode`) 將此值線程化，並將其傳播到證明者 (`Prover::canonical_with_modes`)，因此覆蓋在配置中具有確定性和可審核性dumps.【crates/iroha_config/src/parameters/user.rs:1488】【crates/fastpq_prover/src/proof.rs:138】【crates/iroha_core/src/fastpq/lane.rs:123】
- 遙測通過新的 `fastpq_poseidon_pipeline_total{requested,resolved,path,device_class,chip_family,gpu_kind}` 計數器（和 OTLP twin `fastpq.poseidon_pipeline_resolutions_total`）導出解析的管道模式。因此，`sorafs`/操作員儀表板可以確認部署何時運行 GPU 融合/流水線哈希與強制 CPU 回退 (`path="cpu_forced"`) 或運行時降級 (`path="cpu_fallback"`)。 CLI 探針會自動安裝在 `irohad` 中，因此發布包和實時遙測共享相同的證據流。 【crates/iroha_telemetry/src/metrics.rs:4780】【crates/irohad/src/main.rs:2504】
- 混合模式證據也通過現有的採用門被標記到每個記分板上：證明者為每個批次發出已解析的模式+路徑標籤，並且每當證明落地時，`fastpq_poseidon_pipeline_total` 計數器與執行模式計數器一起遞增。這通過使停電可見並在優化繼續的同時為確定性降級提供乾淨的開關來滿足 WP2-E.6。 【crates/fastpq_prover/src/trace.rs:1684】【docs/source/sorafs_orchestrator_rollout.md:139】
- `scripts/fastpq/wrap_benchmark.py --poseidon-metrics metrics_poseidon.prom` 現在解析 Prometheus 刮擦（金屬或 CUDA），並將 `poseidon_metrics` 摘要嵌入每個打包的包中。幫助程序按 `metadata.labels.device_class` 過濾計數器行，捕獲匹配的 `fastpq_execution_mode_total` 樣本，並在 `fastpq_poseidon_pipeline_total` 條目丟失時使包裝失敗，因此 WP2-E.6 捆綁包始終提供可重現的 CUDA/Metal 證據，而不是臨時證據筆記.【scripts/fastpq/wrap_benchmark.py:1】【scripts/fastpq/tests/test_wrap_benchmark.py:1】

#### 確定性混合模式策略 (WP2-E.6)1. **檢測 GPU 不足。 ** 標記 Stage7 捕獲或實時 Grafana 快照顯示 Poseidon 延遲使總證明時間 >900ms 而 FFT/LDE 保持低於目標的任何設備類。操作員註釋捕獲矩陣（`artifacts/fastpq_benchmarks/matrix/devices/<label>.txt`），並在 `fastpq_poseidon_pipeline_total{device_class="<label>",path="gpu"}` 停滯時分頁待命，而 `fastpq_execution_mode_total{backend="metal"}` 仍記錄 GPU FFT/LDE 調度。 【scripts/fastpq/wrap_benchmark.py:1】【dashboards/grafana/fastpq_acceleration.json:1】
2. **僅針對受影響的主機翻轉到 CPU Poseidon。 ** 在主機本地配置中與隊列標籤一起設置 `zk.fastpq.poseidon_mode = "cpu"`（或 `FASTPQ_POSEIDON_MODE=cpu`），保留 `zk.fastpq.execution_mode = "gpu"`，以便 FFT/LDE 繼續使用加速器。在部署票證中記錄配置差異，並將每個主機的覆蓋添加到捆綁包中，作為 `poseidon_fallback.patch`，以便審閱者可以確定地重播更改。
3. **證明降級。 ** 重啟節點後立即刮掉 Poseidon 計數器：
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"'
   ```
   轉儲必須顯示 `path="cpu_forced"` 與 GPU 執行計數器同步增長。將抓取的內容存儲為 `metrics_poseidon.prom` ，位於現有 `metrics_cpu_fallback.prom` 快照旁邊，並捕獲 `poseidon_fallback.log` 中匹配的 `telemetry::fastpq.poseidon` 日誌行。
4. **監控並退出。 ** 在優化工作繼續進行的同時，保持對 `fastpq_poseidon_pipeline_total{path="cpu_forced"}` 的警報。一旦補丁使測試主機上的每個證明運行時間恢復到 900 毫秒以下，請將配置回滾到 `auto`，重新運行抓取（再次顯示 `path="gpu"`），並將之前/之後的指標附加到捆綁包以關閉混合模式演練。

**遙測合同。 **

|信號| PromQL / 來源 |目的|
|--------------------|-----------------|---------|
|波塞冬模式計數器 | `fastpq_poseidon_pipeline_total{device_class="<label>",path=~"cpu_.*"}` |確認 CPU 哈希是有意的且範圍僅限於標記的設備類。 |
|執行模式計數器 | `fastpq_execution_mode_total{device_class="<label>",backend="metal"}` |證明即使 Poseidon 降級，FFT/LDE 仍然可以在 GPU 上運行。 |
|記錄證據| `poseidon_fallback.log` 中捕獲的 `telemetry::fastpq.poseidon` 條目 |提供主機通過 Reason `cpu_forced` 解析為 CPU 散列的每個證明。 |

推出捆綁包現在必須包括 `metrics_poseidon.prom`、配置差異以及混合模式處於活動狀態時的日誌摘錄，以便治理可以與 FFT/LDE 遙測一起審核確定性後備策略。 `ci/check_fastpq_rollout.sh` 已強制執行隊列/零填充限制；一旦混合模式進入自動化發布階段，後續的門將會對 Poseidon 計數器進行健全性檢查。

Stage7 捕獲工具已經可以處理 CUDA：用 `--poseidon-metrics` 包裝每個 `fastpq_cuda_bench` 包（指向已刪除的 `metrics_poseidon.prom`），並且輸出現在帶有與 Metal 上使用的相同的管道計數器/分辨率摘要，因此治理可以在無需定制工具的情況下驗證 CUDA 回退。 【scripts/fastpq/wrap_benchmark.py:1】### 列順序
哈希管道按以下確定順序消耗列：
1. 選擇器標誌：`s_active`、`s_transfer`、`s_mint`、`s_burn`、`s_role_grant`、`s_role_revoke`、`s_meta_set`、 `s_perm`。
2. 壓縮肢體列（每個零填充至跡線長度）：`key_limb_{i}`、`value_old_limb_{i}`、`value_new_limb_{i}`、`asset_id_limb_{i}`。
3. 輔助標量：`delta`、`running_asset_delta`、`metadata_hash`、`supply_counter`、`perm_hash`、`neighbour_leaf`、`dsid`、 `slot`。
4. 每個級別 `ℓ ∈ [0, SMT_HEIGHT)` 的稀疏 Merkle 見證人：`path_bit_ℓ`、`sibling_ℓ`、`node_in_ℓ`、`node_out_ℓ`。

`trace::column_hashes` 完全按照這個順序遍歷列，因此佔位符後端和 Stage2 STARK 實現在各個版本中保持跟踪穩定。 【crates/fastpq_prover/src/trace.rs:474】

### 轉錄域標籤
Stage2 修復了下面的 Fiat-Shamir 目錄，以保持挑戰生成的確定性：

|標籤 |目的|
| --- | -------- |
| `fastpq:v1:init` | Absorb 協議版本、參數集和 `PublicIO`。 |
| `fastpq:v1:roots` |提交追踪並查找 Merkle 根。 |
| `fastpq:v1:gamma` |示例查找盛大產品挑戰。 |
| `fastpq:v1:alpha:<i>` |示例組合多項式挑戰 (`i = 0, 1`)。 |
| `fastpq:v1:lookup:product` |吸收評估的查找宏積。 |
| `fastpq:v1:beta:<round>` |嘗試 FRI 每輪的折疊挑戰。 |
| `fastpq:v1:fri_layer:<round>` |提交每個 FRI 層的 Merkle 根。 |
| `fastpq:v1:fri:final` |在打開查詢之前記錄最終的 FRI 層。 |
| `fastpq:v1:query_index:0` |確定性地導出驗證者查詢索引。 |