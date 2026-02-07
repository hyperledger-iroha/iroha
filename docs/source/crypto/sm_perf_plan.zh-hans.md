---
lang: zh-hans
direction: ltr
source: docs/source/crypto/sm_perf_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 493c3c0f6a991b2a5d04f33f97b7e97bff372271c5c57751ff41f5e86d43cbc7
source_last_modified: "2025-12-29T18:16:35.944695+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## SM 绩效捕获和基线计划

状态：已起草 — 2025-05-18  
所有者：Performance WG（领导）、Infra Ops（实验室调度）、QA Guild（CI 门控）  
相关路线图任务：SM-4c.1a/b、SM-5a.3b、FASTPQ Stage 7 跨设备捕获

### 1. 目标
1. 在 `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json` 中记录 Neoverse 中位数。当前基线是从 `artifacts/sm_perf/2026-03-lab/neoverse-proxy-macos/`（CPU 标签 `neoverse-proxy-macos`）下的 `neoverse-proxy-macos` 捕获导出的，对于 aarch64 macOS/Linux，SM3 比较容差扩大到 0.70。当裸机时间打开时，在 Neoverse 主机上重新运行 `scripts/sm_perf_capture_helper.sh --matrix --cpu-label neoverse-n2-b01 --output artifacts/sm_perf/<date>/neoverse-n2-b01` 并将聚合中位数提升到基线。  
2. 收集匹配的 x86_64 中位数，以便 `ci/check_sm_perf.sh` 可以保护两个主机类。  
3. 发布可重复的捕获程序（命令、工件布局、审阅者），以便未来的性能门不依赖于部落知识。

### 2. 硬件可用性
当前工作区中只能访问 Apple Silicon (macOS arm64) 主机。 `neoverse-proxy-macos` 捕获将导出为临时 Linux 基线，但捕获裸机 Neoverse 或 x86_64 中位数仍需要在 `INFRA-2751` 下跟踪的共享实验室硬件，以便在实验室窗口打开后由性能工作组运行。剩余的捕获窗口现在已在工件树中预订和跟踪：

- Neoverse N2 裸机（东京机架 B）于 2026 年 3 月 12 日预订。操作员将重用第 3 部分中的命令并将工件存储在 `artifacts/sm_perf/2026-03-lab/neoverse-b01/` 下。
- x86_64 Xeon（苏黎世机架 D）于 2026 年 3 月 19 日预订，禁用 SMT 以减少噪音；文物将以 `artifacts/sm_perf/2026-03-lab/xeon-d01/` 的身份登陆。
- 两次运行落地后，将中位数提升到基线 JSON 中，并在 `ci/check_sm_perf.sh` 中启用 CI 门（目标切换日期：2026-03-25）。

在此日期之前，只有 macOS arm64 基线可以在本地刷新。### 3. 捕获程序
1. **同步工具链**  
   ```bash
   rustup override set $(cat rust-toolchain.toml)
   cargo fetch
   ```
2. **生成捕获矩阵**（每个主机）  
   ```bash
   scripts/sm_perf_capture_helper.sh --matrix \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}
   ```
   帮助程序现在在目标目录下写入 `capture_commands.sh` 和 `capture_plan.json`。该脚本为每种模式设置 `raw/*.json` 捕获路径，以便实验室技术人员可以确定性地对运行进行批处理。
3. **运行捕获**  
   从 `capture_commands.sh` 执行每个命令（或手动运行等效命令），确保每个模式都通过 `--capture-json` 发出结构化 JSON blob。始终通过 `--cpu-label "<model/bin>"`（或 `SM_PERF_CPU_LABEL=<label>`）提供主机标签，以便捕获元数据和后续基线记录生成中位数的确切硬件。助手已经提供了适当的路径；对于手动运行，模式是：
   ```bash
   SM_PERF_CAPTURE_LABEL=auto \
   scripts/sm_perf.sh --mode auto \
     --cpu-label "neoverse-n2-lab-b01" \
     --capture-json artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/auto.json
   ```
4. **验证结果**  
   ```bash
   scripts/sm_perf_check \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json
   ```
   确保运行之间的方差保持在 ±3% 以内。如果没有，请重新运行受影响的模式并在日志中记录重试情况。
5. **提升中位数**  
   使用 `scripts/sm_perf_aggregate.py` 计算中位数并将其复制到基线 JSON 文件中：
   ```bash
   scripts/sm_perf_aggregate.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/raw/*.json \
     --output artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json
   ```
   辅助组通过 `metadata.mode` 捕获，验证每个组共享
   相同的 `{target_arch, target_os}` 三重，并发出包含一个条目的 JSON 摘要
   每个模式。应该落在基线文件中的中位数位于
   `modes.<mode>.benchmarks`，同时附带 `statistics` 块记录
   审阅者和 CI 的完整样本列表、最小/最大、平均值和总体标准差。
   一旦聚合文件存在，您就可以自动编写基线 JSON（使用
   标准公差图）通过：
   ```bash
   scripts/sm_perf_promote_baseline.py \
     artifacts/sm_perf/2025-07-lab/${HOSTNAME}/aggregated.json \
     --out-dir crates/iroha_crypto/benches \
     --target-os unknown_linux_gnu \
     --overwrite
   ```
   覆盖 `--mode` 以限制为子集或 `--cpu-label` 以固定
   当聚合源省略时记录 CPU 名称。
   每个架构的两台主机完成后，更新：
   - `sm_perf_baseline_aarch64_unknown_linux_gnu_{scalar,auto,neon_force}.json`
   - `sm_perf_baseline_x86_64_unknown_linux_gnu_{scalar,auto}.json`（新）

   `aarch64_unknown_linux_gnu_*` 文件现在反映 `m3-pro-native`
   捕获（保留CPU标签和元数据注释），因此`scripts/sm_perf.sh`可以
   自动检测 aarch64-unknown-linux-gnu 主机，无需手动标记。当
   裸机实验室运行完成，重新运行 `scripts/sm_perf.sh --mode 
   --write-baseline crates/iroha_crypto/benches/sm_perf_baseline_aarch64_unknown_linux_gnu_.json`
   用新的捕获覆盖临时中位数并标记真实值
   主机标签。

   > 参考：2025 年 7 月 Apple Silicon 捕获（CPU 标签 `m3-pro-local`）为
   > 存档于 `artifacts/sm_perf/2025-07-lab/takemiyacStudio.lan/{raw,aggregated.json}`。
   > 当您发布 Neoverse/x86 工件时，请镜像该布局，以便审阅者
   > 可以一致地区分原始/聚合输出。

### 4. 工件布局和签核
```
artifacts/sm_perf/
  2025-07-lab/
    neoverse-b01/
      raw/
      aggregated.json
      run-log.md
    neoverse-b02/
      …
    xeon-d01/
    xeon-d02/
```
- `run-log.md` 记录命令哈希、git 修订、操作员和任何异常。
- 聚合的 JSON 文件直接输入到基线更新中，并附加到 `docs/source/crypto/sm_perf_baseline_comparison.md` 中的性能审查中。
- QA Guild 在基线更改之前审查工件并在“性能”部分下的 `status.md` 中签字。### 5. CI 门控时间表
|日期 |里程碑|行动|
|------|------------|--------|
| 2025-07-12 | Neoverse 捕获完成 |更新 `sm_perf_baseline_aarch64_*` JSON 文件，在本地运行 `ci/check_sm_perf.sh`，打开附加了工件的 PR。 |
| 2025-07-24 | x86_64 捕获完成 |在`ci/check_sm_perf.sh`中添加新的基线文件+门控；确保跨架构 CI 通道消耗它们。 |
| 2025-07-27 | CI 执行 |启用 `sm-perf-gate` 工作流程在两个主机类上运行；如果回归超出配置的容差，则合并失败。 |

### 6. 依赖关系和通信
- 通过 `infra-ops@iroha.tech` 协调实验室访问更改。  
- 性能工作组在捕获运行时在 `#perf-lab` 频道中发布每日更新。  
- QA Guild 准备比较差异 (`scripts/sm_perf_compare.py`)，以便审阅者可以可视化增量。  
- 基线合并后，使用捕获完成注释更新 `roadmap.md`（SM-4c.1a/b、SM-5a.3b）和 `status.md`。

通过该计划，SM 加速工作获得了可重复的中值、CI 门控和可追踪的证据线索，满足“保留实验室窗口和捕获中值”行动项目。

### 7. CI 门和局部烟雾

- `ci/check_sm_perf.sh` 是规范的 CI 入口点。它为 `SM_PERF_MODES` 中的每种模式（默认为 `scalar auto neon-force`）解析为 `scripts/sm_perf.sh` 并设置 `CARGO_NET_OFFLINE=true`，以便工作台在 CI 映像上确定性地运行。  
- `.github/workflows/sm-neon-check.yml` 现在调用 macOS arm64 运行程序上的门，因此每个拉取请求都通过本地使用的相同帮助程序来执行标量/自动/霓虹灯三重奏；一旦 x86_64 占领土地并且 Neoverse 代理基线通过裸机运行刷新，互补的 Linux/Neoverse 通道就会加入。  
- 操作员可以在本地覆盖模式列表：`SM_PERF_MODES="scalar" bash ci/check_sm_perf.sh` 将运行修剪为单次通过以进行快速冒烟测试，而其他参数（例如 `--tolerance 0.20`）将直接转发到 `scripts/sm_perf.sh`。  
- `make check-sm-perf` 现在包裹了大门，以方便开发人员； CI 作业可以直接调用脚本，而 macOS 开发人员则依赖 make 目标。  
- 一旦 Neoverse/x86_64 基线落地，相同的脚本将通过 `scripts/sm_perf.sh` 中已有的主机自动检测逻辑获取适当的 JSON，因此除了为每个主机池设置所需的模式列表之外，工作流程中不需要额外的接线。

### 8.季度刷新助手- 运行 `scripts/sm_perf_quarterly.sh --owner "<name>" --cpu-label "<label>" [--quarter YYYY-QN] [--output-root artifacts/sm_perf]` 以创建四分之一标记的目录，例如 `artifacts/sm_perf/2026-Q1/<label>/`。该帮助程序包装 `scripts/sm_perf_capture_helper.sh --matrix` 并发出 `capture_commands.sh`、`capture_plan.json` 和 `quarterly_plan.json`（所有者 + 季度元数据），以便实验室操作员无需手写计划即可安排运行。
- 在目标主机上执行生成的 `capture_commands.sh`，将原始​​输出与 `scripts/sm_perf_aggregate.py --output <dir>/aggregated.json` 聚合，并通过 `scripts/sm_perf_promote_baseline.py --out-dir crates/iroha_crypto/benches --overwrite` 将中位数提升到基线 JSON。重新运行 `ci/check_sm_perf.sh` 以确认容差保持绿色。
- 当硬件或工具链发生变化时，刷新 `docs/source/crypto/sm_perf_baseline_comparison.md` 中的比较容差/注释，如果新中位数稳定，则收紧 `ci/check_sm_perf.sh` 容差，并将任何仪表板/警报阈值与新基线对齐，以便操作警报保持有意义。
- 提交 `quarterly_plan.json`、`capture_plan.json`、`capture_commands.sh` 和聚合 JSON 以及基线更新；将相同的工件附加到状态/路线图更新中以实现可追溯性。