---
lang: zh-hant
direction: ltr
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2025-12-29T18:16:35.965528+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha 3 長凳套件

Iroha 3 工作台套件乘以我們在質押期間依賴的熱路徑、費用
計費、證明驗證、調度和證明端點。它運行為
`xtask` 具有確定性固定裝置的命令（固定種子、固定密鑰材料、
和穩定的請求負載），因此結果可以跨主機重現。

## 運行套件

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

標誌：

- `--iterations` 控制每個場景樣本的迭代（默認值：64）。
- `--sample-count` 重複每個場景以計算中位數（默認值：5）。
- `--json-out|--csv-out|--markdown-out` 選擇輸出工件（全部可選）。
- `--threshold` 將中位數與基線界限進行比較（設置 `--no-threshold`
  跳過）。
- `--flamegraph-hint` 用 `cargo flamegraph` 註釋 Markdown 報告
  用於分析場景的命令。

CI 膠水位於 `ci/i3_bench_suite.sh` 中，默認為上面的路徑；設置
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` 用於調整夜間運行時間。

## 場景

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — 付款人與讚助商借方
  和不足拒絕。
- `staking_bond` / `staking_slash` — 有和沒有的綁定/取消綁定隊列
  砍伐。
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  對提交證書、JDG 證明和橋接進行簽名驗證
  證明有效負載。
- `commit_cert_assembly` — 提交證書的摘要彙編。
- `access_scheduler` — 衝突感知訪問集調度。
- `torii_proof_endpoint` — Axum 證明端點解析 + 驗證往返。

每個場景都會記錄每次迭代的中值納秒、吞吐量和
用於快速回歸的確定性分配計數器。閾值生活在
`benchmarks/i3/thresholds.json`；當硬件發生變化時，碰撞會限制在那裡
將新工件與報告一起提交。

## 故障排除

- 收集證據時固定 CPU 頻率/調速器以避免噪音回歸。
- 使用 `--no-threshold` 進行探索性運行，然後在基線達到後重新啟用
  神清氣爽。
- 要分析單個場景，請設置 `--iterations 1` 並在下重新運行
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`。