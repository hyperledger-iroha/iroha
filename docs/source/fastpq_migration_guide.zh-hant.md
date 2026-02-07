---
lang: zh-hant
direction: ltr
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-05T09:28:12.006936+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#！ FASTPQ 生產遷移指南

本運行手冊介紹瞭如何驗證 Stage6 生產 FASTPQ 證明器。
作為此遷移計劃的一部分，確定性佔位符後端已被刪除。
它補充了 `docs/source/fastpq_plan.md` 中的分階段計劃，並假設您已經跟踪
`status.md` 中的工作區狀態。

## 受眾和範圍
- 驗證器操作員在臨時或主網環境中推出生產驗證器。
- 發布工程師創建將隨生產後端一起提供的二進製文件或容器。
- SRE/可觀測性團隊連接新的遙測信號和警報。

超出範圍：Kotodama 合同編寫和 IVM ABI 更改（請參閱 `docs/source/nexus.md` 了解
執行模型）。

## 特徵矩陣
|路徑|貨運功能啟用|結果 |何時使用 |
| ---- | ----------------------- | ------ | ----------- |
|生產驗證機（默認）| _無_ | Stage6 FASTPQ 後端，帶有 FFT/LDE 規劃器和 DEEP-FRI 管道。 【crates/fastpq_prover/src/backend.rs:1144】 |所有生產二進製文件的默認值。 |
|可選 GPU 加速 | `fastpq_prover/fastpq-gpu` |啟用具有自動 CPU 回退功能的 CUDA/Metal 內核。 【crates/fastpq_prover/Cargo.toml:9】【crates/fastpq_prover/src/fft.rs:124】 |具有支持的加速器的主機。 |

## 構建過程
1. **僅 CPU 構建**
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   默認編譯生產後端；不需要額外的功能。

2. **支持 GPU 的構建（可選）**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   GPU 支持需要 SM80+ CUDA 工具包，並在構建期間提供 `nvcc`。 【crates/fastpq_prover/Cargo.toml:11】

3. **自檢**
   ```bash
   cargo test -p fastpq_prover
   ```
   每個發布版本運行一次，以在打包之前確認 Stage6 路徑。

### Metal 工具鏈準備 (macOS)
1. 在構建之前安裝 Metal 命令行工具：`xcode-select --install`（如果缺少 CLI 工具）和 `xcodebuild -downloadComponent MetalToolchain` 以獲取 GPU 工具鏈。構建腳本直接調用 `xcrun metal`/`xcrun metallib`，如果二進製文件不存在，將會快速失敗。 【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:121】
2. 要在 CI 之前驗證管道，您可以在本地鏡像構建腳本：
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   當成功時，構建會發出 `FASTPQ_METAL_LIB=<path>`；運行時讀取該值以確定性地加載metallib。 【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】
3、不使用Metal工具鏈交叉編譯時設置`FASTPQ_SKIP_GPU_BUILD=1`；構建打印警告，規劃器保留在 CPU 路徑上。 【crates/fastpq_prover/build.rs:45】【crates/fastpq_prover/src/backend.rs:195】
4. 如果 Metal 不可用（缺少框架、不支持 GPU 或空 `FASTPQ_METAL_LIB`），節點會自動回退到 CPU；構建腳本清除環境變量，規劃器記錄降級。 【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:293】【crates/fastpq_prover/src/backend.rs:605】### 發布清單（第 6 階段）
保持 FASTPQ 發布票證處於鎖定狀態，直到完成並附加以下所有項目。

1. **亞秒級證明指標** — 檢查新捕獲的 `fastpq_metal_bench_*.json` 並
   確認 `benchmarks.operations` 條目，其中 `operation = "lde"`（以及鏡像
   `report.operations` 示例）針對 20000 行工作負載（32768 填充
   行）。在簽署清單之前，超出上限的捕獲需要重新運行。
2. **簽名清單** — 運行
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   因此釋放票同時帶有清單及其獨立簽名
   （`artifacts/fastpq_bench_manifest.sig`）。審閱者之前驗證摘要/簽名對
   促進發布。 【xtask/src/fastpq.rs:128】【xtask/src/main.rs:845】矩陣清單（構建
   通過 `scripts/fastpq/capture_matrix.sh`）已經對 20k 行地板進行了編碼，並且
   調試回歸。
3. **證據附件** — 上傳 Metal 基準 JSON、stdout 日誌（或 Instruments 跟踪）、
   CUDA/Metal 清單輸出，以及發布票證的獨立簽名。檢查清單條目
   應鏈接到所有工件以及用於簽名的公鑰指紋，以便下游審計
   可以重播驗證步驟。 【artifacts/fastpq_benchmarks/README.md:65】### 金屬驗證工作流程
1. 在啟用 GPU 的構建之後，確認 `FASTPQ_METAL_LIB` 指向 `.metallib` (`echo $FASTPQ_METAL_LIB`)，以便運行時可以確定性地加載它。 【crates/fastpq_prover/build.rs:188】
2. 在強制啟用 GPU 通道的情況下運行奇偶校驗套件：\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`。如果檢測失敗，後端將執行 Metal 內核並記錄確定性 CPU 回退。 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/metal.rs:418】
3. 捕獲儀表板的基準示例：\
   找到已編譯的 Metal 庫 (`fd -g 'fastpq.metallib' target/release/build | head -n1`)，
   通過 `FASTPQ_METAL_LIB` 導出它，然後運行\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`。
  規範的 `fastpq-lane-balanced` 配置文件現在將每次捕獲填充到 32,768 行 (215)，因此 JSON 攜帶 `rows` 和 `padded_rows` 以及 Metal LD​​E 延遲；如果 `zero_fill` 或隊列設置使 GPU LDE 超出 AppleM 系列主機上的 950 毫秒（<1 秒）目標，請重新運行捕獲。將生成的 JSON/日誌與其他發布證據一起存檔；每晚 macOS 工作流程執行相同的運行並上傳其工件進行比較。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【.github/workflows/fastpq-metal-nightly.yml:1】
  當您需要僅限 Poseidon 的遙測（例如，記錄 Instruments 跟踪）時，請將 `--operation poseidon_hash_columns` 添加到上面的命令中；該工作台仍將尊重 `FASTPQ_GPU=gpu`，發出 `metal_dispatch_queue.poseidon`，並包含新的 `poseidon_profiles` 塊，因此發布包明確記錄了 Poseidon 瓶頸。
  現在證據包括 `zero_fill.{bytes,ms,queue_delta}` 和 `kernel_profiles`（每個內核
  佔用率、估計 GB/s 和持續時間統計數據），因此可以繪製 GPU 效率圖表，而無需
  重新處理原始跟踪，以及 `twiddle_cache` 塊（命中/未命中 + `before_ms`/`after_ms`）
  證明緩存的旋轉上傳有效。 `--trace-dir` 重新啟動線束
  `xcrun xctrace record` 和
  將帶時間戳的 `.trace` 文件與 JSON 一起存儲；您仍然可以提供定制服務
  `--trace-output`（可選 `--trace-template` / `--trace-seconds`）
  自定義位置/模板。 JSON記錄`metal_trace_{template,seconds,output}`用於審計。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:177】每次捕穫後運行 `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json`，以便發布包含 Grafana 板/警報包（`dashboards/grafana/fastpq_acceleration.json`、`dashboards/alerts/fastpq_acceleration_rules.yml`）的主機元數據（現在包括 `metadata.metal_trace`）。現在，報告為每個操作攜帶一個 `speedup` 對象（`speedup.ratio`、`speedup.delta_ms`），包裝器提升 `zero_fill_hotspots`（字節、延遲、導出的 GB/s 和 Metal 隊列增量計數器），將 `kernel_profiles` 展平為`benchmarks.kernel_summary`，保持 `twiddle_cache` 塊完整，複製新的 `post_tile_dispatches` 塊/摘要，以便審閱者可以證明在捕獲期間運行的多通道內核，現在將 Poseidon 微基准證據匯總到 `benchmarks.poseidon_microbench` 中，以便儀表板可以引用標量與默認延遲，而無需重新解析原始報告。清單門讀取相同的塊並拒絕忽略它的 GPU 證據包，每當跳過後平鋪路徑或配置錯誤。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】【xtask/src/fastpq.rs:280】
  Poseidon2 Metal 內核共享相同的旋鈕：`FASTPQ_METAL_POSEIDON_LANES`（32-256，2 的冪）和 `FASTPQ_METAL_POSEIDON_BATCH`（每通道 1-32 狀態）讓您無需重建即可固定啟動寬度和每通道工作；主機在每次調度之前通過 `PoseidonArgs` 將這些值線程化。默認情況下，運行時會檢查 `MTLDevice::{is_low_power,is_headless,location}`，以使離散 GPU 偏向 VRAM 分層啟動（報告 ≥ 48GiB 時為 `256×24`，報告 32GiB 時為 `256×20`，否則為 `256×16`），而低功耗 SoC 仍保留在 `256×8`（及更早版本）上128/64 通道部分堅持每通道 8/6 個狀態），因此大多數操作員永遠不需要手動設置環境變量。 【crates/fastpq_prover/src/metal_config.rs:78】【crates/fastpq_prover/src/metal.rs:1971】 `fastpq_metal_bench` 現在在 `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` 下重新執行自身並發出信號`poseidon_microbench` 塊記錄啟動配置文件以及測量的加速與標量通道的比較，因此發布包可以證明新內核實際上縮小了 `poseidon_hash_columns`，並且它包含 `poseidon_pipeline` 塊，因此 Stage7 證據捕獲了塊深度/重疊旋鈕以及新的佔用層。對於正常運行，請保持環境未設置；該工具會自動管理重新執行，如果子捕獲無法運行則記錄失敗，並在設置 `FASTPQ_GPU=gpu` 但沒有可用的 GPU 後端時立即退出，因此靜默的 CPU 回退永遠不會潛入性能人工製品。 【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988】包裝器拒絕缺少 `metal_dispatch_queue.poseidon` 增量、共享 `column_staging` 計數器或 `poseidon_profiles`/`poseidon_microbench` 證據塊的 Poseidon 捕獲，因此操作員必須刷新無法證明重疊分段或標量與默認值的任何捕獲加速。 【scripts/fastpq/wrap_benchmark.py:732】當您需要儀表板或 CI 增量的獨立 JSON 時，請運行 `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`；幫助器接受包裝的工件和原始 `fastpq_metal_bench*.json` 捕獲，使用默認/標量計時、調整元數據和記錄的加速來發出 `benchmarks/poseidon/poseidon_microbench_<timestamp>.json`。 【scripts/fastpq/export_poseidon_microbench.py:1】
  通過執行 `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` 來完成運行，以便 Stage6 發布清單強制實施 `<1 s` LDE 上限，並發出隨發布票證一起提供的簽名清單/摘要包。 【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】
4. 在推出前驗證遙測：curl Prometheus 端點 (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) 並檢查 `telemetry::fastpq.execution_mode` 日誌中是否有意外的 `resolved="cpu"`條目。 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】
5. 通過有意強制（`FASTPQ_GPU=cpu` 或 `zk.fastpq.execution_mode = "cpu"`）來記錄 CPU 後備路徑，以便 SRE 劇本與確定性行為保持一致。 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】6. 可選調整：默認情況下，主機為短跡線選擇 16 個通道，為中跡線選擇 32 個通道，一次 `log_len ≥ 10/14` 為 64/128，當 `log_len ≥ 18` 時為 256，現在，它為小跡線選擇 5 個階段的共享內存塊，為 `log_len ≥ 12` 一次選擇 4 個通道，為 `log_len ≥ 12` 保留 12/14/16 個階段。 `log_len ≥ 18/20/22` 在將工作踢到後平舖內核之前。在運行上述步驟之前導出 `FASTPQ_METAL_FFT_LANES`（8 和 256 之間的 2 的冪）和/或 `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) 以覆蓋這些啟發式方法。 FFT/IFFT 和 LDE 列批量大小均源自解析的線程組寬度（每次調度約 2048 個邏輯線程，上限為 32 列，現在隨著域的增長逐漸下降到 32→16→8→4→2→1），而 LDE 路徑仍然強制執行其域上限；當您需要跨主機進行逐位比較時，設置 `FASTPQ_METAL_FFT_COLUMNS` (1–32) 以固定確定性 FFT 批量大小，並設置 `FASTPQ_METAL_LDE_COLUMNS` (1–32) 以將相同的覆蓋應用於 LDE 調度程序。 LDE 切片深度也反映了 FFT 啟發式 — 在將寬蝴蝶交給後切片內核之前，`log₂ ≥ 18/20/22` 的跟踪僅運行 12/10/8 共享內存階段 — 並且您可以通過 `FASTPQ_METAL_LDE_TILE_STAGES` (1–32) 覆蓋該限制。運行時通過 Metal 內核參數對所有值進行線程化，限制不支持的覆蓋，並記錄解析的值，以便實驗保持可重複性，而無需重建 Metallib；基準 JSON 顯示已解析的調整和通過 LDE 統計信息捕獲的主機零填充預算 (`zero_fill.{bytes,ms,queue_delta}`)，因此隊列增量直接與每個捕獲相關聯，現在添加了 `column_staging` 塊（批次扁平化、flatten_ms、wait_ms、wait_ratio），以便審閱者可以驗證雙緩衝管道引入的主機/設備重疊。當 GPU 拒絕報告零填充遙測時，該工具現在會從主機端緩衝區清除中合成確定性時序，並將其註入到 `zero_fill` 塊中，因此釋放證據永遠不會在沒有字段.【crates/fastpq_prover/src/metal_config.rs:15】【crates/fastpq_prover/src/metal.rs:742】【crates/fastpq_prover/src/bin/fastpq_met al_bench.rs:575】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】【板條箱/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. 多隊列調度在獨立 Mac 上是自動的：當 `Device::is_low_power()` 返回 false 或 Metal 設備報告插槽/外部位置時，主機實例化兩個 `MTLCommandQueue`，僅在工作負載承載 ≥16 列（按扇出縮放）時扇出，並跨隊列循環列批處理，以便長跟踪使兩個 GPU 通道保持繁忙，而不會影響確定性。每當您需要跨機器進行可重複捕獲時，請使用 `FASTPQ_METAL_QUEUE_FANOUT`（1–4 個隊列）和 `FASTPQ_METAL_COLUMN_THRESHOLD`（扇出前的最小總列數）覆蓋該策略；奇偶校驗測試強制這些覆蓋，以便多 GPU Mac 保持覆蓋，並且解析的扇出/閾值記錄在隊列深度遙測旁邊。 【crates/fastpq_prover/src/metal.rs:620】【crates/fastpq_prover/src/metal.rs:900】【crates/fastpq_prover/src/metal.rs:2254】### 歸檔證據
|文物 |捕捉|筆記|
|----------|---------|--------|
| `.metallib` 捆綁包 | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` 和 `xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` 後面是 `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` 和 `export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`。 |證明 Metal CLI/工具鏈已安裝並為此提交生成了確定性庫。 【crates/fastpq_prover/build.rs:98】【crates/fastpq_prover/build.rs:188】 |
|環境快照 |構建後的 `echo $FASTPQ_METAL_LIB`；保留您的發行票的絕對路徑。 |輸出為空意味著 Metal 被禁用；記錄運輸工件上 GPU 通道仍然可用的有價值的文件。 【crates/fastpq_prover/build.rs:188】【crates/fastpq_prover/src/metal.rs:43】 |
| GPU 奇偶校驗日誌 | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` 並存檔包含 `backend="metal"` 或降級警告的代碼段。 |演示內核在升級構建之前運行（或確定性回退）。 【crates/fastpq_prover/src/backend.rs:114】【crates/fastpq_prover/src/backend.rs:195】 |
|基準輸出 | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`；通過 `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]` 包裝並簽名。 |包裝的 JSON 記錄 `speedup.ratio`、`speedup.delta_ms`、FFT 調諧、填充行 (32,768)、豐富的 `zero_fill`/`kernel_profiles`、扁平的 `kernel_summary`、驗證的`metal_dispatch_queue.poseidon`/`poseidon_profiles` 塊（當使用 `--operation poseidon_hash_columns` 時）和跟踪元數據，因此 GPU LDE 平均值保持 ≤950ms，Poseidon 保持 <1s；將捆綁包和生成的 `.json.asc` 簽名與發布票一起保留，以便儀表板和審核員可以驗證工件，而無需重新運行工作負載。 【crates/fastpq_prover/src/bin/fastpq_metal_bench.rs:697】【scripts/fastpq/wrap_benchmark.py:714】【scripts/fastpq/wrap_benchmark.py:732】|
|替補清單 | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`。 |驗證兩個 GPU 工件，如果 LDE 均值突破 `<1 s` 上限則失敗，記錄 BLAKE3/SHA-256 摘要，並發出簽名清單，以便發布清單在沒有可驗證指標的情況下無法推進。 【xtask/src/fastpq.rs:1】【artifacts/fastpq_benchmarks/README.md:65】 |
| CUDA 捆綁包 |在 SM80 實驗室主機上運行 `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json`，將 JSON 包裝/簽名為 `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json`（使用 `--label device_class=xeon-rtx-sm80`，以便儀表板選擇正確的類），添加 `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt` 的路徑，並保留 `.json`/`.asc` 對重新生成清單之前的金屬製品。簽入的 `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` 說明了審核員期望的確切捆綁格式。 【scripts/fastpq/wrap_benchmark.py:714】【artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt:1】 |
|遙測證明 | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` 加上啟動時發出的 `telemetry::fastpq.execution_mode` 日誌。 |確認 Prometheus/OTEL 在啟用流量之前公開 `device_class="<matrix>", backend="metal"`（或降級日誌）。 【crates/iroha_telemetry/src/metrics.rs:8887】【crates/fastpq_prover/src/backend.rs:174】 ||強制CPU鑽|使用 `FASTPQ_GPU=cpu` 或 `zk.fastpq.execution_mode = "cpu"` 運行一小批並捕獲降級日誌。 |使 SRE Runbook 與確定性回退路徑保持一致，以防發布中期需要回滾。 【crates/fastpq_prover/src/backend.rs:308】【crates/iroha_config/src/parameters/user.rs:1357】 |
|跟踪捕獲（可選）|使用 `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` 重複奇偶校驗測試並保存發出的調度跟踪。 |保留佔用/線程組證據，以便以後進行分析審查，而無需重新運行基準測試。 【crates/fastpq_prover/src/metal.rs:346】【crates/fastpq_prover/src/backend.rs:208】 |

多語言 `fastpq_plan.*` 文件引用了此清單，因此登台和生產操作員遵循相同的證據線索。 【docs/source/fastpq_plan.md:1】

## 可重複的構建
使用固定容器工作流程來生成可重現的 Stage6 工件：

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

幫助程序腳本構建 `rust:1.88.0-slim-bookworm` 工具鏈映像（以及用於 GPU 的 `nvidia/cuda:12.2.2-devel-ubuntu22.04`），在容器內運行構建，並將 `manifest.json`、`sha256s.txt` 和編譯的二進製文件寫入目標輸出目錄。 【scripts/fastpq/repro_build.sh:1】【scripts/fastpq/run_inside_repro_build.sh:1】【scripts/fastpq/docker/Dockerfile.gpu:1】

環境覆蓋：
- `FASTPQ_RUST_IMAGE`、`FASTPQ_RUST_TOOLCHAIN` – 固定明確的 Rust 基礎/標籤。
- `FASTPQ_CUDA_IMAGE` – 生成 GPU 工件時交換 CUDA 基礎。
- `FASTPQ_CONTAINER_RUNTIME` – 強制特定的運行時間；默認 `auto` 嘗試 `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`。
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` – 用於運行時自動檢測的逗號分隔優先順序（默認為 `docker,podman,nerdctl`）。

## 配置更新
1. 在 TOML 中設置運行時執行模式：
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   該值通過 `FastpqExecutionMode` 解析並在啟動時線程到後端。 【crates/iroha_config/src/parameters/user.rs:1357】【crates/irohad/src/main.rs:1733】

2. 如果需要，在啟動時覆蓋：
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   CLI 覆蓋會在節點啟動之前改變已解析的配置。 【crates/irohad/src/main.rs:270】【crates/irohad/src/main.rs:1733】

3. 開發者可以通過導出來臨時強制檢測，而無需觸及配置
   啟動二進製文件之前的 `FASTPQ_GPU={auto,cpu,gpu}`；記錄覆蓋並且管道
   仍然顯示解析模式。 【crates/fastpq_prover/src/backend.rs:208】【crates/fastpq_prover/src/backend.rs:401】

## 驗證清單
1. **啟動日誌**
   - 期望來自目標 `telemetry::fastpq.execution_mode` 的 `FASTPQ execution mode resolved`
     `requested`、`resolved` 和 `backend` 標籤。 【crates/fastpq_prover/src/backend.rs:208】
   - 自動 GPU 檢測時，來自 `fastpq::planner` 的輔助日誌報告最終通道。
   - 當 Metallib 成功加載時，Metal 主機會顯示 `backend="metal"`；如果編譯或加載失敗，構建腳本會發出警告，清除 `FASTPQ_METAL_LIB`，規劃器在停留之前記錄 `GPU acceleration unavailable` CPU.【crates/fastpq_prover/build.rs:29】【crates/fastpq_prover/src/backend.rs:174】【crates/fastpq_prover/src/backend.rs:195】【crates/fastpq_prover/src/metal.rs:43】2. **Prometheus 指標**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   計數器通過 `record_fastpq_execution_mode` 遞增（現在標記為
   `{device_class,chip_family,gpu_kind}`) 每當節點解析其執行時
   模式.【crates/iroha_telemetry/src/metrics.rs:8887】
   - 對於金屬覆蓋確認
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     與部署儀表板一起遞增。 【crates/iroha_telemetry/src/metrics.rs:5397】
   - 使用 `irohad --features fastpq-gpu` 編譯的 macOS 節點另外公開
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     和
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` 所以 Stage7 儀表板
     可以從實時 Prometheus 抓取中跟踪佔空比和隊列餘量。 【crates/iroha_telemetry/src/metrics.rs:4436】【crates/irohad/src/main.rs:2345】

3. **遙測導出**
   - OTEL 構建發出具有相同標籤的 `fastpq.execution_mode_resolutions_total`；確保您的
     當 GPU 應處於活動狀態時，儀表板或警報會監視意外的 `resolved="cpu"`。

4. **健全性證明/驗證**
   - 通過 `iroha_cli` 或集成工具運行小批量，並在
     同行使用相同的參數編譯。

## 故障排除
- **解析模式將 CPU 保留在 GPU 主機上** — 檢查二進製文件是否是使用以下命令構建的
  `fastpq_prover/fastpq-gpu`，CUDA庫在加載器路徑上，並且`FASTPQ_GPU`不強制
  `cpu`。
- **Metal 在 Apple Silicon 上不可用** — 驗證 CLI 工具是否已安裝 (`xcode-select --install`)，重新運行 `xcodebuild -downloadComponent MetalToolchain`，並確保構建生成非空 `FASTPQ_METAL_LIB` 路徑；空值或缺失值會根據設計禁用後端。 【crates/fastpq_prover/build.rs:166】【crates/fastpq_prover/src/metal.rs:43】
- **`Unknown parameter` 錯誤** — 確保證明者和驗證者使用相同的規範目錄
  由 `fastpq_isi` 發出；表面不匹配為 `Error::UnknownParameter`。 【crates/fastpq_prover/src/proof.rs:133】
- **意外的 CPU 回退** — 檢查 `cargo tree -p fastpq_prover --features` 並
  確認 GPU 版本中存在 `fastpq_prover/fastpq-gpu`；驗證 `nvcc`/CUDA 庫位於搜索路徑上。
- **遙測計數器丟失** — 驗證節點是否使用 `--features telemetry` 啟動（默認）
  並且 OTEL 導出（如果啟用）包括指標管道。 【crates/iroha_telemetry/src/metrics.rs:8887】

## 後備程序
確定性佔位符後端已被刪除。如果回歸需要回滾，
重新部署以前已知的良好發布工件並在重新發布 Stage6 之前進行調查
二進製文件。記錄變更管理決策並確保前滾僅在變更後完成
回歸是可以理解的。

3. 監控遙測以確保 `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` 反映預期
   佔位符執行。

## 硬件基線
|簡介 |中央處理器|圖形處理器 |筆記|
| -------- | --- | --- | -----|
|參考（第6階段）| AMD EPYC7B12（32 核）、256GiB RAM | NVIDIA A10040GB (CUDA12.2) | 20000行合成批次必須完成≤1000ms。 【docs/source/fastpq_plan.md:131】 |
|僅 CPU | ≥32 個物理核心，AVX2 | – | 20000 行預計約為 0.9–1.2 秒；保留 `execution_mode = "cpu"` 以實現確定性。 |## 回歸測試
- `cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu`（在 GPU 主機上）
- 可選的黃金夾具檢查：
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

在您的操作手冊中記錄與此清單的任何偏差，並在執行後更新 `status.md`
遷移窗口完成。