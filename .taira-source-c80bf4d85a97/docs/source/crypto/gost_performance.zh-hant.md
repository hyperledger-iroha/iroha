---
lang: zh-hant
direction: ltr
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2025-12-29T18:16:35.939573+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# GOST 性能工作流程

本說明記錄了我們如何跟踪和執行績效範圍
TC26 GOST 簽名後端。

## 本地運行

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

兩個目標在幕後都調用 `scripts/gost_bench.sh`，其中：

1. 執行 `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`。
2. 針對 `target/criterion` 運行 `gost_perf_check`，根據
   簽入基線 (`crates/iroha_crypto/benches/gost_perf_baseline.json`)。
3. 將 Markdown 摘要注入 `$GITHUB_STEP_SUMMARY`（如果可用）。

要在批准回歸/改進後刷新基線，請運行：

```bash
make gost-bench-update
```

或直接：

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

`scripts/update_gost_baseline.sh` 運行工作台 + 檢查器，覆蓋基線 JSON，並打印
新的中位數。始終將更新的 JSON 與決策記錄一起提交
`crates/iroha_crypto/docs/gost_backend.md`。

### 當前參考中位數

|算法|中值（微秒）|
|----------------------|-------------|
| ed25519 | 69.67 | 69.67
| gost256_paramset_a | gost256_paramset_a | 1136.96 | 1136.96
| gost256_paramset_b | gost256_paramset_b | gost256_paramset_b | gost256_paramset_b 1129.05 | 1129.05
| gost256_paramset_c | gost256_paramset_c | gost256_paramset_c 1133.25 | 1133.25
| gost512_paramset_a | gost512_paramset_a | 8944.39 |
| gost512_paramset_b | gost512_paramset_b | gost512_paramset_b | gost512_paramset_b 8963.60 |
| secp256k1 | 160.53 | 160.53

## CI

`.github/workflows/gost-perf.yml` 使用相同的腳本並運行 dudect 定時保護。
當測量的中值超出基線且超出配置的容差時，CI 失敗
（默認為 20%）或當計時保護檢測到洩漏時，會自動捕獲回歸。

## 摘要輸出

`gost_perf_check` 本地打印對照表，並追加相同內容
`$GITHUB_STEP_SUMMARY`，因此 CI 作業日誌和運行摘要共享相同的編號。