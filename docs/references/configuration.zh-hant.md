---
lang: zh-hant
direction: ltr
source: docs/references/configuration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cff283a14bf65f185f81539f8fbcd78ddcc6447c5e9045e1b46493051febaf6a
source_last_modified: "2025-12-29T18:16:35.913045+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 加速

`[accel]` 部分控制 IVM 和助手的可選硬件加速。全部
加速路徑具有確定性的 CPU 回退；如果後端失敗了
運行時自檢會自動禁用，並在 CPU 上繼續執行。

- `enable_cuda`（默認值：true） - 編譯並可用時使用 CUDA。
- `enable_metal`（默認值：true） - 在 macOS 上使用 Metal（如果可用）。
- `max_gpus`（默認值：0）——要初始化的最大 GPU 數量； `0` 表示自動/無上限。
- `merkle_min_leaves_gpu`（默認值：8192）——卸載 Merkle 的最小葉數
  葉散列到 GPU。僅對於速度異常快的 GPU 而言較低。
- 高級（可選；通常繼承合理的默認值）：
  - `merkle_min_leaves_metal`（默認：繼承`merkle_min_leaves_gpu`）。
  - `merkle_min_leaves_cuda`（默認：繼承`merkle_min_leaves_gpu`）。
  - `prefer_cpu_sha2_max_leaves_aarch64`（默認值：32768） – 優先選擇 CPU SHA-2，直到 ARMv8 上具有 SHA2 的這麼多葉子。
  - `prefer_cpu_sha2_max_leaves_x86`（默認值：32768） – 在 x86/x86_64 上優先使用 CPU SHA-NI 最多這麼多葉子。

註釋
- 確定性第一：加速度永遠不會改變可觀察的輸出；後端
  在 init 上運行黃金測試，並在檢測到不匹配時回退到標量/SIMD。
- 通過 `iroha_config` 配置；避免生產中的環境變量。