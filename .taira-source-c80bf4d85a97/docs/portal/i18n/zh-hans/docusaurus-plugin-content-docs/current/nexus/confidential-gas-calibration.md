---
slug: /nexus/confidential-gas-calibration
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 机密气体校准基线

该分类账跟踪机密气体校准的验证输出
基准。每行记录了使用捕获的发布质量测量集
[机密资产和 ZK 转移](./confidential-assets#calibration-baselines--acceptance-gates) 中描述的过程。

|日期 (UTC) |提交 |简介 | `ns/op` | `gas/op` | `ns/gas` |笔记|
| ---| ---| ---| ---| ---| ---| ---|
| 2025年10月18日 | 3c70a7d3 | 3c70a7d3基线霓虹灯| 2.93e5 | 2.93e5 1.57e2 | 1.57e2 1.87e3 | 1.87e3达尔文 25.0.0 arm64e（主机信息）； `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`； `cargo test -p iroha_core bench_repro -- --ignored`； `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`； `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |待定 |基线 simd 中性 | — | — | — |计划在 CI 主机 `bench-x86-neon0` 上运行 x86_64 中立；请参阅票 GAS-214。工作台窗口完成后将添加结果（预合并清单目标版本 2.1）。 |
| 2026-04-13 |待定 |基线 avx2 | — | — | — |使用与中性运行相同的提交/构建进行后续 AVX2 校准；需要主机 `bench-x86-avx2a`。 GAS-214 涵盖了与 `baseline-neon` 进行增量比较的两次运行。 |

`ns/op` 聚合 Criterion 测量的每条指令的中位挂钟；
`gas/op` 是相应调度成本的算术平均值
`iroha_core::gas::meter_instruction`； `ns/gas` 将纳秒总和除以
九个指令样本集的气体总和。

*注意。* 当前的arm64主机不会发出标准`raw.csv`摘要
盒子；在标记之前使用 `CRITERION_OUTPUT_TO=csv` 或上游修复程序重新运行
发布，以便附上验收清单所需的工件。
如果 `--save-baseline` 之后仍然缺少 `target/criterion/`，则收集运行
在 Linux 主机上或将控制台输出序列化到发行包中作为
暂时的权宜之计。作为参考，最新运行的arm64控制台日志
住在 `docs/source/confidential_assets_calibration_neon_20251018.log`。

同一运行中的每条指令中位数 (`cargo bench -p iroha_core --bench isi_gas_calibration`)：

|说明 |中位数 `ns/op` |时间表 `gas` | `ns/gas` |
| ---| ---| ---| ---|
|注册域名 | 3.46e5 | 3.46e5 200 | 200 1.73e3 | 1.73e3
|注册帐号 | 3.15e5 | 3.15e5 200 | 200 1.58e3 | 1.58e3
|注册资产定义| 3.41e5 | 3.41e5 200 | 200 1.71e3 | 1.71e3
|设置AccountKV_small | 3.28e5 | 3.28e5 67 | 67 4.90e3 | 4.90e3
|授予帐户角色 | 3.33e5 | 3.33e5 96 | 96 3.47e3 | 3.47e3
|撤销帐户角色 | 3.12e5 | 3.12e5 96 | 96 3.25e3 | 3.25e3
|执行触发器_空_参数| 1.42e5 | 1.42e5 224 | 224 6.33e2 | 6.33e2
|薄荷资产 | 1.56e5 | 1.56e5 150 | 150 1.04e3 | 1.04e3
|转让资产 | 3.68e5 | 3.68e5 180 | 180 2.04e3 | 2.04e3

时间表列由 `gas::tests::calibration_bench_gas_snapshot` 强制执行
（九个指令集中总共 1,413 个 Gas）并且如果将来的补丁出现问题将会跳闸
无需更新校准装置即可更改计量。