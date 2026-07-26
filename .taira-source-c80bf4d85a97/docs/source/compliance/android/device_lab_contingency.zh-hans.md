---
lang: zh-hans
direction: ltr
source: docs/source/compliance/android/device_lab_contingency.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4016b82d86dc61a9de5e345950d02aeadf26db4cc26777c60db336c57479ba15
source_last_modified: "2025-12-29T18:16:35.923121+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 设备实验室应急日志

在此记录 Android 设备实验室应急计划的每次激活。
包含足够的详细信息以供合规性审查和未来的准备审核。

|日期 |触发|采取的行动|后续行动|业主|
|------|---------|----------------|------------|--------|
| 2026-02-11 | Pixel8 Pro 通道中断并延迟 Pixel8a 交付后，运力下降至 78%（请参阅 `android_strongbox_device_matrix.md`）。 |将 Pixel7 通道提升为主要 CI 目标，借用共享 Pixel6 车队，安排 Firebase 测试实验室对零售钱包样本进行烟雾测试，并根据 AND6 计划聘请外部 StrongBox 实验室。 |更换 Pixel8 Pro 有故障的 USB-C 集线器（截止日期为 2026 年 2 月 15 日）；确认 Pixel8a 抵达并重新设定容量报告基准。 |硬件实验室负责人 |
| 2026-02-13 | Pixel8 Pro 集线器更换并获得 GalaxyS24 批准，容量恢复至 85%。 |将 Pixel7 通道返回到辅助通道，重新启用带有标签 `pixel8pro-strongbox-a` 和 `s24-strongbox-a` 的 `android-strongbox-attestation` Buildkite 作业，更新准备矩阵 + 证据日志。 |监控 Pixel8a 交付预计时间（仍在等待中）；记录备用轮毂库存。 |硬件实验室负责人 |