---
lang: zh-hant
direction: ltr
source: docs/source/fastpq_metal_kernels.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0022f5f9c53445d26876f0097635092b5c685d332bfa25b13243c584d358dfe
source_last_modified: "2026-01-05T09:28:12.006723+00:00"
translation_last_reviewed: 2026-02-07
title: FASTPQ Metal Kernel Suite
translator: machine-google-reviewed
---

# FASTPQ 金屬內核套件

Apple Silicon 後端提供了一個 `fastpq.metallib`，其中包含每個
證明者使用的金屬著色語言 (MSL) 內核。這個註釋解釋了
可用的入口點、它們的線程組限制和確定性
保證 GPU 路徑與標量後備可互換。

規範的實現位於
`crates/fastpq_prover/metal/kernels/` 編譯為
只要 macOS 上啟用了 `fastpq-gpu`，就會出現 `crates/fastpq_prover/build.rs`。
運行時元數據 (`metal_kernel_descriptors`) 鏡像以下信息，以便
基準測試和診斷可以通過編程方式顯示相同的事實。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:1】【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/build.rs:1】【crates/fastpq_prover/src/metal.rs:248】

## 內核清單|切入點|運營|線程組上限 |瓷磚舞台帽|筆記|
| ----------- | --------- | ---------------- | -------------- | -----|
| `fastpq_fft_columns` |跟踪列上的正向 FFT | 256 線程 | 32 個階段 |在第一階段使用共享內存塊，並在規劃器請求 IFFT 模式時應用反向縮放。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:223】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_fft_post_tiling` |達到圖塊深度後完成 FFT/IFFT/LDE | 256 線程 | — |直接從設備內存中運行剩餘的蝴蝶，並在返回主機之前處理最終的陪集/逆因子。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:447】【crates/fastpq_prover/src/metal.rs:262】
| `fastpq_lde_columns` |跨列低度擴展 | 256 線程 | 32 個階段 |將係數複製到評估緩衝區中，使用配置的陪集執行平鋪階段，並在需要時將最終階段留給`fastpq_fft_post_tiling`。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:341】【crates/fastpq_prover/src/metal.rs:262】
| `poseidon_trace_fused` |一次性哈希列併計算深度 1 父級 | 256 線程 | — |運行與 `poseidon_hash_columns` 相同的吸收/排列，將葉消化直接存儲到輸出緩衝區中，並立即將每個 `(left,right)` 對折疊在 `fastpq:v1:trace:node` 域下，以便 `(⌈columns / 2⌉)` 父母在葉切片之後著陸。奇數列計數複製設備上的最終葉子，消除了第一個 Merkle 層的後續內核和 CPU 回退。 【crates/fastpq_prover/metal/kernels/poseidon2.metal:384】【crates/fastpq_prover/src/metal.rs:2407】
| `poseidon_permute` | Poseidon2 排列 (STATE_WIDTH = 3) | 256 線程 | — |線程組將輪常量/MDS 行緩存在線程組內存中，將 MDS 行複製到每個線程寄存器中，並在 4 狀態塊中處理狀態，以便每個輪常量獲取在前進之前在多個狀態中重複使用。回合保持完全展開，每個通道仍然走多個狀態，保證每次調度 ≥ 4096 個邏輯線程。 `FASTPQ_METAL_POSEIDON_LANES` / `FASTPQ_METAL_POSEIDON_BATCH` 固定啟動寬度和每通道批次，無需重建 Metallib。 【crates/fastpq_prover/metal/kernels/poseidon2.metal:1】【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】

描述符可在運行時通過
`fastpq_prover::metal_kernel_descriptors()` 用於想要顯示的工具
相同的元數據。

## 確定性金發姑娘算法- 所有內核都在 Goldilocks 字段上工作，並使用中定義的幫助程序
  `field.metal`（模塊化加/乘/減，逆，`pow5`）。 【crates/fastpq_prover/metal/kernels/field.metal:1】
- FFT/LDE 階段重用 CPU 規劃器生成的相同旋轉表。
  `compute_stage_twiddles` 預先計算每個階段和主機的一次旋轉
  每次調度前通過緩衝槽 1 上傳數組，保證
  GPU路徑使用相同的統一根。 【crates/fastpq_prover/src/metal.rs:1527】
- LDE 的陪集乘法被融合到最後階段，因此 GPU 永遠不會
  與 CPU 跡線佈局不同；主機對評估緩衝區進行零填充
  在調度之前，保持填充行為確定性。 【crates/fastpq_prover/metal/kernels/ntt_stage.metal:288】【crates/fastpq_prover/src/metal.rs:898】

## Metallib 生成

`build.rs` 將各個 `.metal` 源編譯為 `.air` 對象，然後
將它們鏈接到 `fastpq.metallib`，導出上面列出的每個入口點。
將 `FASTPQ_METAL_LIB` 設置為該路徑（構建腳本執行此操作
自動）允許運行時確定性地加載庫，無論
`cargo` 放置構建工件的位置。 【crates/fastpq_prover/build.rs:45】

為了與 CI 運行保持一致，您可以手動重新生成庫：

```bash
export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
xcrun metal -std=metal3.0 -O3 -c crates/fastpq_prover/metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
```

## 線程組大小啟發式

`metal_config::fft_tuning` 線程設備執行寬度和每個線程的最大線程數
將線程組放入規劃器中，以便運行時調度遵守硬件限制。
隨著日誌大小的增加，默認限制為 32/64/128/256 通道，並且
平鋪深度現在從 `log_len ≥ 12` 的五個階段變為四個階段，然後保持
一旦跟踪交叉，共享內存傳遞將在 12/14/16 階段處於活動狀態
`log_len ≥ 18/20/22` 在將工作交給後平舖內核之前。操作員
覆蓋（`FASTPQ_METAL_FFT_LANES`、`FASTPQ_METAL_FFT_TILE_STAGES`）流經
`FftArgs::threadgroup_lanes`/`local_stage_limit` 並由內核應用
以上，無需重建metallib。 【crates/fastpq_prover/src/metal_config.rs:12】【crates/fastpq_prover/src/metal.rs:599】

使用 `fastpq_metal_bench` 捕獲解析的調整值並驗證
之前已執行多遍內核（JSON 中的 `post_tile_dispatches`）
發送基準測試包。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】