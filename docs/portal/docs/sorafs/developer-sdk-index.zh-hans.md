---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 28cc43e407412d66481f25146c19d35f0e102523d22f954be3c106231d95e891
source_last_modified: "2026-01-05T09:28:11.868629+00:00"
translation_last_reviewed: 2026-02-07
id: developer-sdk-index
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
translator: machine-google-reviewed
---

:::注意规范来源
:::

使用此中心跟踪 SoraFS 工具链附带的每种语言帮助程序。
对于 Rust 特定的代码片段，请跳转到 [Rust SDK 代码片段](./developer-sdk-rust.md)。

## 语言助手

- **Python** — `sorafs_multi_fetch_local`（本地协调器冒烟测试）和
  `sorafs_gateway_fetch`（网关E2E练习）现在接受可选
  `telemetry_region` 加上 `transport_policy` 覆盖
  （`"soranet-first"`、`"soranet-strict"` 或 `"direct-only"`），镜像 CLI
  推出旋钮。当本地 QUIC 代理启动时，
  `sorafs_gateway_fetch` 返回浏览器清单
  `local_proxy_manifest` 因此测试可以将信任包交给浏览器适配器。
- **JavaScript** — `sorafsMultiFetchLocal` 镜像 Python 帮助器，返回
  有效负载字节和接收摘要，而 `sorafsGatewayFetch` 练习
  Torii 网关、线程本地代理清单，并公开相同的内容
  遥测/传输覆盖 CLI。
- **Rust** — 服务可以直接通过嵌入调度程序
  `sorafs_car::multi_fetch`；请参阅 [Rust SDK 片段](./developer-sdk-rust.md)
  证明流助手和协调器集成的参考。
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` 重用 Torii HTTP
  执行人并获得荣誉 `GatewayFetchOptions`。将其与
  `ClientConfig.Builder#setSorafsGatewayUri` 和 PQ 上传提示
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) 上传时必须坚持
  仅 PQ 路径。

## 记分板和政策旋钮

Python (`sorafs_multi_fetch_local`) 和 JavaScript
(`sorafsMultiFetchLocal`) 帮助程序公开遥测感知调度程序记分板
CLI 使用：

- 生产二进制文件默认启用记分板；设置 `use_scoreboard=True`
  （或提供 `telemetry` 条目）在重放赛程时以便帮助程序派生
  根据广告元数据和最近的遥测快照对提供商进行加权排序。
- 设置 `return_scoreboard=True` 以接收计算出的权重以及块
  收据，以便 CI 日志可以捕获诊断信息。
- 使用 `deny_providers` 或 `boost_providers` 阵列拒绝对等点或添加
  当调度程序选择提供者时，`priority_delta`。
- 保持默认的 `"soranet-first"` 姿势，除非进行降级；供应
  `"direct-only"` 仅当合规区域必须避免中继或当
  演练 SNNet-5a 后备，并保留 `"soranet-strict"` 仅用于 PQ
  经政府批准的试点。
- 网关助手还公开 `scoreboardOutPath` 和 `scoreboardNowUnixSecs`。
  设置 `scoreboardOutPath` 以保留计算的记分板（镜像 CLI
  `--scoreboard-out` 标志），因此 `cargo xtask sorafs-adoption-check` 可以验证
  SDK工件，当灯具需要稳定时使用`scoreboardNowUnixSecs`
  `assume_now` 可重现元数据的值。在 JavaScript 帮助器中，您
  可另外设置`scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`；
  当省略标签时，它派生出 `region:<telemetryRegion>` （回退
  至 `sdk:js`）。 Python 帮助程序自动发出 `telemetry_source="sdk:python"`
  每当它保留记分板并禁用隐式元数据时。

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```