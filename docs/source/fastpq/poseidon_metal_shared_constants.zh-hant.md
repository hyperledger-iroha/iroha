---
lang: zh-hant
direction: ltr
source: docs/source/fastpq/poseidon_metal_shared_constants.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4cbbc93e4212320422b8cbfcd8c563419d5ddaf5dad9e84a7878a439892ed081
source_last_modified: "2025-12-29T18:16:35.955568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 波塞冬金屬共享常數

Metal 內核、CUDA 內核、Rust 證明器和每個 SDK 固定裝置必須共享
完全相同的 Poseidon2 參數以保持硬件加速
散列確定性。本文檔記錄了規範快照，如何
重新生成它，以及 GPU 管道如何攝取數據。

## 快照清單

這些參數以 `PoseidonSnapshot` RON 文檔的形式發布。副本有
保持版本控制，因此 GPU 工具鍊和 SDK 不依賴於構建時
代碼生成。

|路徑|目的| SHA-256 |
|------|---------|---------|
| `artifacts/offline_poseidon/constants.ron` |從 `fastpq_isi::poseidon::{ROUND_CONSTANTS, MDS}` 生成的規範快照； GPU 構建的真實來源。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `IrohaSwift/Fixtures/offline_poseidon/constants.ron` |鏡像規範快照，以便 Swift 單元測試和 XCFramework 煙霧線束加載 Metal 內核期望的相同常量。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |
| `java/iroha_android/src/test/resources/offline_poseidon/constants.ron` | Android/Kotlin 設備共享相同的奇偶校驗和序列化測試清單。 | `99bef7760fcc80c2d4c47e720cf28a156f106a0fa389f2be55a34493a0ca4c21` |

每個消費者在將常量連接到 GPU 之前都必須驗證哈希值
管道。當清單發生更改（新的參數集或配置文件）時，SHA 和
下游鏡像必須同步更新。

## 再生

該清單是通過運行 `xtask` 從 Rust 源生成的
幫手。該命令寫入規範文件和 SDK 鏡像：

```bash
cargo xtask offline-poseidon-fixtures --tag iroha.offline.receipt.merkle.v1
```

使用 `--constants <path>`/`--vectors <path>` 覆蓋目標或
僅重新生成規範快照時的 `--no-sdk-mirror`。幫手會
當省略該標誌時，將工件鏡像到 Swift 和 Android 樹中，
這使得 CI 的哈希值保持一致。

## 提供 Metal/CUDA 構建

- `crates/fastpq_prover/metal/kernels/poseidon2.metal` 和
  `crates/fastpq_prover/cuda/fastpq_cuda.cu` 必須從
  每當表發生變化時就會顯示出來。
- 舍入和 MDS 常量被分級為連續的 `MTLBuffer`/`__constant`
  與清單佈局匹配的段：`round_constants[round][state_width]`
  接下來是 3x3 MDS 矩陣。
- `fastpq_prover::poseidon_manifest()` 加載並驗證快照
  運行時（Metal 預熱期間），因此診斷工具可以斷言
  著色器常量通過以下方式匹配發布的哈希值
  `fastpq_prover::poseidon_manifest_sha256()`。
- SDK 夾具讀取器（Swift `PoseidonSnapshot`、Android `PoseidonSnapshot`）和
  Norito 離線工具依賴於相同的清單，這會阻止僅使用 GPU
  參數叉。

## 驗證

1. 重新生成清單後，運行 `cargo test -p xtask` 來執行
   Poseidon 夾俱生成單元測試。
2. 在本文檔以及任何監控的儀表板中記錄新的 SHA-256
   GPU 文物。
3. `cargo test -p fastpq_prover poseidon_manifest_consistency`解析
   `poseidon2.metal` 和 `fastpq_cuda.cu` 在構建時並斷言它們
   序列化常量與清單匹配，保留 CUDA/Metal 表和
   鎖步的規範快照。將清單與 GPU 構建指令放在一起可以提供 Metal/CUDA
工作流程確定性握手：內核可以自由優化其內存
佈局，只要它們攝取共享常量 blob 並公開散列
用於奇偶校驗的遙測。