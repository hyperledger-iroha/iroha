---
id: constant-rate-profiles
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraNet constant-rate profiles
sidebar_label: Constant-Rate Profiles
description: SNNet-17B1 preset catalogue for core/home production relays plus the SNNet-17A2 null dogfood profile, with tick->bandwidth math, CLI helpers, and MTU guardrails.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

SNNet-17B 引入了固定速率传输通道，因此中继在 1,024 B 单元中移动流量，无论
有效负载大小。操作员从三个预设中进行选择：

- **核心** - 数据中心或专业托管的中继，可以专用 >=30 Mbps 来覆盖
  交通。
- **home** - 仍然需要匿名获取的住宅或低上行链路运营商
  隐私关键电路。
- **null** - SNNet-17A2 狗粮预设。它保留了相同的 TLV/信封，但扩展了
  低带宽分级的刻度和上限。

## 预设摘要

|简介 |刻度线（毫秒）|细胞（B）|车道帽 |虚拟地板|每通道有效负载 (Mb/s) |有效负载上限 (Mb/s) |上行链路百分比上限 |建议上行链路 (Mb/s) |邻居帽 |自动禁用触发器 (%) |
|------|---------|----------|----------|--------------|--------------------------|------------------------|----------------------|----------------------------|--------------|----------------------------|
|核心| 5.0 | 1024 | 1024 12 | 12 4 | 1.64 | 1.64 19.50 | 19.50 65 | 65 30.0 | 30.0 8 | 85 | 85
|首页 | 10.0 | 1024 | 1024 4 | 2 | 0.82 | 0.82 4.00 | 40 | 40 10.0 | 2 | 70 | 70
|空 | 20.0 | 20.0 1024 | 1024 2 | 1 | 0.41 | 0.41 0.75 | 0.75 15 | 15 5.0 | 1 | 55 | 55

- **车道上限** - 最大并发恒定速率邻居。继电器拒绝额外电路一次
  达到上限并递增 `soranet_handshake_capacity_reject_total`。
- **虚拟楼层** - 即使在实际情况下也能保持虚拟交通的最小车道数
  需求较低。
- **有效负载上限** - 应用上限后专用于恒定速率通道的上行链路预算
  分数。即使有额外的带宽可用，运营商也不应超出此预算。
- **自动禁用触发器** - 持续饱和百分比（每个预设的平均值），导致
  运行时掉落到虚拟地板上。容量在恢复阈值后恢复
  （对于 `core` 为 75%，对于 `home` 为 60%，对于 `null` 为 45%）。

**重要提示：** `null` 预设仅用于暂存和功能测试；它不符合
生产电路所需的隐私保证。

## 勾选->带宽表

每个有效负载单元携带 1,024 B，因此 KiB/sec 列等于每个有效负载单元发射的单元数
第二。使用帮助程序通过自定义刻度来扩展表格。

|刻度线（毫秒）|单元/秒 |有效负载 KiB/秒 |有效负载 Mb/s |
|----------|----------|-----------------|--------------|
| 5.0 | 200.00 | 200.00 | 1.64 | 1.64
| 7.5 | 7.5 133.33 | 133.33 | 1.09 | 1.09
| 10.0 | 100.00 | 100.00 | 0.82 | 0.82
| 15.0 | 15.0 66.67 | 66.67 66.67 | 66.67 0.55 | 0.55
| 20.0 | 20.0 50.00 | 50.00 | 0.41 | 0.41

公式：

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

CLI 助手：

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

`--format markdown` 为预设摘要和可选的刻度作弊生成 GitHub 样式的表格
工作表，以便您可以将确定性输出粘贴到门户中。与 `--json-out` 配对进行存档
治理证据的渲染数据。

## 配置和覆盖

`tools/soranet-relay` 在配置文件和运行时覆盖中公开预设：

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

配置密钥接受 `core`、`home` 或 `null`（默认为 `core`）。 CLI 覆盖对于
暂存演练或 SOC 请求可暂时减少占空比，而无需重写配置。

## MTU 护栏

- 有效负载单元使用 1,024 B 加上约 96 B 的 Norito+噪声帧和最小的 QUIC/UDP 标头，
  将每个数据报保持在 IPv6 1,280 B 最小 MTU 以下。
- 当隧道（WireGuard/IPsec）添加额外封装时，您**必须**减少 `padding.cell_size`
  所以 `cell_size + framing <= 1,280 B`。中继验证器强制执行
  `padding.cell_size <= 1,136 B`（1,280 B - 48 B UDP/IPv6 开销 - 96 B 成帧）。
- `core` 配置文件应固定 >=4 个邻居，即使在闲置时也是如此，因此虚拟通道始终覆盖
  PQ守卫。 `home` 配置文件可能会限制钱包/聚合器的恒定速率电路，但必须适用
  三个遥测窗口的饱和度超过 70% 时的背压。

## 遥测和警报

继电器根据预设导出以下指标：

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

出现以下情况时发出警报：

1. 虚拟比率低于预设下限（`core >= 4/8`、`home >= 2/2`、`null >= 1/1`）的时间超过
   两个窗户。
2. `soranet_constant_rate_ceiling_hits_total` 的增长速度超过每五分钟一次点击。
3. `soranet_constant_rate_degraded` 在计划演练之外翻转为 `1`。

在事件报告中记录预设标签和邻居列表，以便审核员可以证明恒定速率
政策符合路线图要求。