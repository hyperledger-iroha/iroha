---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7db41ca4cc25ba00f6f14e357d8871e1b27c8d6b5d489557d8416b3f6f7a39ef
source_last_modified: "2026-01-22T14:45:01.294133+00:00"
translation_last_reviewed: 2026-02-07
id: orchestrator-config
title: SoraFS Orchestrator Configuration
sidebar_label: Orchestrator Configuration
description: Configure the multi-source fetch orchestrator, interpret failures, and debug telemetry output.
translator: machine-google-reviewed
---

:::注意规范来源
:::

# 多源获取协调器指南

SoraFS 多源获取协调器驱动确定性并行
从治理支持的广告中发布的提供商集下载。这个
指南解释了如何配置协调器以及预期的故障信号
在推出期间，以及哪些遥测流公开健康指标。

## 1. 配置概述

编排器合并了三个配置源：

|来源 |目的|笔记|
|--------|---------|--------|
| `OrchestratorConfig.scoreboard` |规范化提供者权重，验证遥测新鲜度，并保留用于审计的 JSON 记分板。 |由 `crates/sorafs_car::scoreboard::ScoreboardConfig` 支持。 |
| `OrchestratorConfig.fetch` |应用运行时限制（重试预算、并发限制、验证切换）。 |映射到 `crates/sorafs_car::multi_fetch` 中的 `FetchOptions`。 |
| CLI/SDK 参数 |限制对等点的数量、附加遥测区域以及表面拒绝/提升策略。 | `sorafs_cli fetch` 直接公开这些标志； SDK 通过 `OrchestratorConfig` 将它们线程化。 |

`crates/sorafs_orchestrator::bindings` 中的 JSON 帮助程序序列化整个
配置到 Norito JSON，使其可跨 SDK 绑定和
自动化。

### 1.1 JSON 配置示例

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

通过通常的 `iroha_config` 分层保留文件（`defaults/`，用户，
实际），因此确定性部署在节点之间继承相同的限制。
对于与 SNNet-5a 部署一致的仅直接后备配置文件，
请参阅 `docs/examples/sorafs_direct_mode_policy.json` 和同伴
`docs/source/sorafs/direct_mode_pack.md` 中的指导。

### 1.2 合规性覆盖

SNNet-9 将治理驱动的合规性纳入编排器中。一个新的
Norito JSON 配置中的 `compliance` 对象捕获剥离
强制获取管道进入仅直接模式：

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` 声明 ISO-3166 alpha-2 代码，其中
  Orchestrator 实例运行。代码在期间标准化为大写
  解析。
- `jurisdiction_opt_outs` 镜像治理寄存器。当任何操作员
  管辖权出现在列表中，协调器执行
  `transport_policy=direct-only` 并发出策略回退原因
  `compliance_jurisdiction_opt_out`。
- `blinded_cid_opt_outs` 列出清单摘要（盲化 CID，编码为
  大写十六进制）。匹配有效负载还强制仅直接调度和
  表面遥测中的 `compliance_blinded_cid_opt_out` 后备。
- `audit_contacts` 记录治理期望运营商发布的 URI
  他们的 GAR 剧本。
- `attestations` 捕获支持策略的签名合规性数据包。
  每个条目定义一个可选的 `jurisdiction`（ISO-3166 alpha-2 代码）、
  `document_uri`，规范的 64 字符 `digest_hex`，发行
  时间戳 `issued_at_ms` 和可选的 `expires_at_ms`。这些文物
  流入协调器的审计清单，以便治理工具可以链接
  覆盖已签署的文件。

通过通常的配置分层提供合规性块，以便操作员
接收确定性覆盖。编排器应用合规性_after_
写入模式提示：即使 SDK 请求 `upload-pq-only`，管辖权或
明显的选择退出仍然会退回到仅直接传输，并且在没有时会快速失败
存在合规的提供商。

规范选择退出目录位于
`governance/compliance/soranet_opt_outs.json`；治理委员会发布
通过标记版本进行更新。完整的示例配置（包括
证明）可在 `docs/examples/sorafs_compliance_policy.json` 中找到，并且
操作过程被捕获在
[GAR 合规手册](../../../source/soranet/gar_compliance_playbook.md)。

### 1.3 CLI 和 SDK 旋钮

|旗帜/场|效果|
|--------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` |限制有多少提供商能够通过记分板过滤器。设置为 `None` 以使用每个符合条件的提供商。 |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` |每个块的重试次数上限。超过限制会引发 `MultiSourceError::ExhaustedRetries`。 |
| `--telemetry-json` |将延迟/故障快照注入记分板生成器。超过 `telemetry_grace_secs` 的过时遥测数据将导致提供商不符合资格。 |
| `--scoreboard-out` |保留计算的记分板（合格 + 不合格的提供商）以进行运行后检查。 |
| `--scoreboard-now` |覆盖记分牌时间戳（Unix 秒），以便夹具捕获保持确定性。 |
| `--deny-provider` / 分数政策挂钩 |确定地将提供商排除在日程安排之外，而不删除广告。对于快速响应黑名单很有用。 |
| `--boost-provider=name:delta` |调整提供商的加权循环积分，同时保持治理权重不变。 |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` |标记发出的指标和结构化日志，以便仪表板可以按地理位置或推出波进行旋转。 |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` |现在，多源协调器已成为基线，默认为 `soranet-first`。在进行降级或遵循合规指令时使用 `direct-only`，​​并为仅 PQ 试点保留 `soranet-strict`；合规性覆盖仍然是硬性上限。 |

SoraNet-first 现在是默认的发货方式，回滚必须引用相关的 SNNet 拦截器。 SNNet-4/5/5a/5b/6a/7/8/12/13 毕业后，治理将逐步推进所需的姿态（朝向 `soranet-strict`）；在此之前，只有事件驱动的覆盖才应优先考虑 `direct-only`，并且必须将它们记录在推出日志中。

上面的所有标志都接受 `--` 风格的语法 `sorafs_cli fetch` 和
面向开发人员的 `sorafs_fetch` 二进制文件。 SDK 通过类型公开相同的选项
建设者。

### 1.4 Guard 缓存管理

CLI 现在连接到 SoraNet 防护选择器，以便操作员可以锁定条目
在 SNNet-5 传输全面推出之前确定性地进行中继。三
新标志控制工作流程：

|旗帜|目的|
|------|---------|
| `--guard-directory <PATH>` |指向描述最新中继共识的 JSON 文件（如下所示的子集）。传递目录会在执行提取之前刷新防护缓存。 |
| `--guard-cache <PATH>` |保留 Norito 编码的 `GuardSet`。即使没有提供新目录，后续运行也会重用缓存。 |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` |可选覆盖要固定的门禁数量（默认 3）和保留窗口（默认 30 天）。 |
| `--guard-cache-key <HEX>` |可选的 32 字节密钥用于使用 Blake3 MAC 标记防护缓存，以便可以在重复使用之前验证文件。 |

保护目录有效负载使用紧凑的架构：

`--guard-directory` 标志现在需要 Norito 编码
`GuardDirectorySnapshotV2` 有效负载。二进制快照包含：

- `version` — 架构版本（当前为 `2`）。
- `directory_hash`、`published_at_unix`、`valid_after_unix`、`valid_until_unix` — 共识
  必须与每个嵌入证书匹配的元数据。
- `validation_phase` — 证书策略门（`1` = 允许单个 Ed25519 签名，
  `2` = 更喜欢双重签名，`3` = 需要双重签名）。
- `issuers` — `fingerprint`、`ed25519_public` 和 `mldsa65_public` 的治理发行人。
  指纹计算如下
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`。
- `relays` — SRCv2 捆绑包列表（`RelayCertificateBundleV2::to_cbor()` 输出）。每束
  携带中继描述符、功能标志、ML-KEM 策略和双 Ed25519/ML-DSA-65
  签名。

CLI 在将目录与合并之前根据声明的发行者密钥验证每个包

使用 `--guard-directory` 调用 CLI，将最新共识与
现有的缓存。选择器保留仍在范围内的固定防护装置
保留窗口和目录中的资格；更换过期的新继电器
条目。成功获取后，更新的缓存将写回路径
通过 `--guard-cache` 提供，保持后续会话的确定性。软件开发工具包
可以通过调用重现相同的行为
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` 和
将生成的 `GuardSet` 穿过 `SorafsGatewayFetchOptions`。

`ml_kem_public_hex` 使选择器能够在
SNNet-5 的推出。阶段切换（`anon-guard-pq`、`anon-majority-pq`、
`anon-strict-pq`) 现在会自动降级经典继电器：当 PQ 防护处于
可用选择器删除多余的经典引脚，以便后续会话有利于
混合握手。 CLI/SDK 摘要通过以下方式显示结果组合
`anonymity_status`/`anonymity_reason`、`anonymity_effective_policy`、
`anonymity_pq_selected`，
`anonymity_classical_selected`，`anonymity_pq_ratio`，
`anonymity_classical_ratio`，以及伴随的候选/赤字/供应增量
场，使限电和经典后备变得明确。

Guard 目录现在可以通过以下方式嵌入完整的 SRCv2 包
`certificate_base64`。编排器解码每个包，重新验证
Ed25519/ML-DSA 签名，并保留解析的证书以及
保护缓存。当证书存在时，它就成为规范来源
PQ 密钥、握手套件偏好和权重；过期的证书是
通过电路生命周期管理传播并通过
`telemetry::sorafs.guard` 和 `telemetry::sorafs.circuit`，记录了
有效性窗口、握手套件以及是否观察到双重签名
每个警卫。

使用 CLI 帮助程序使快照与发布者保持同步：```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` 在将 SRCv2 快照写入磁盘之前下载并验证它，
而 `verify` 重播来自其他来源的工件的验证管道
团队，发出反映 CLI/SDK 防护选择器输出的 JSON 摘要。

### 1.5 电路生命周期管理器

当同时提供中继目录和保护缓存时，编排器
激活电路生命周期管理器以预构建和更新 SoraNet 电路
每次获取之前。配置位于 `OrchestratorConfig` 中
(`crates/sorafs_orchestrator/src/lib.rs:305`) 通过两个新字段：

- `relay_directory`：携带 SNNet-3 目录快照，因此中间/出口跃点
  可以确定性地选择。
- `circuit_manager`：可选配置（默认启用）控制
  电路TTL。

Norito JSON 现在接受 `circuit_manager` 块：

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

SDK 通过以下方式转发目录数据
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)，并且 CLI 会在任何时候自动连接它
提供 `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`)。

每当保护元数据发生变化（端点、PQ 密钥、
或固定时间戳）或 TTL 已过。助手 `refresh_circuits`
在每次获取之前调用 (`crates/sorafs_orchestrator/src/lib.rs:1346`)
发出 `CircuitEvent` 日志，以便操作员可以跟踪生命周期决策。浸泡
测试 `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) 表现出稳定的延迟
跨越三个后卫轮换；请参阅随附的报告
`docs/source/soranet/reports/circuit_stability.md:1`。

### 1.6 本地 QUIC 代理

编排器可以选择生成本地 QUIC 代理，以便浏览器扩展
SDK 适配器不必管理证书或保护缓存密钥。的
代理绑定到环回地址，终止 QUIC 连接，并返回
Norito 清单描述了证书和可选的防护缓存密钥
客户。代理发出的传输事件通过以下方式进行计数
`sorafs_orchestrator_transport_events_total`。

通过 Orchestrator JSON 中的新 `local_proxy` 块启用代理：

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` 控制代理监听的位置（使用 `0` 端口请求
  临时端口）。
- `telemetry_label` 传播到指标中，以便仪表板可以区分
  来自获取会话的代理。
- `guard_cache_key_hex`（可选）让代理表面具有相同的键控防护
  CLI/SDK 依赖的缓存，保持浏览器扩展同步。
- `emit_browser_manifest` 切换握手是否返回清单
  扩展可以存储和验证。
- `proxy_mode` 选择代理是在本地桥接流量 (`bridge`) 还是
  仅发出元数据，因此 SDK 可以自行打开 SoraNet 电路
  （`metadata-only`）。代理默认为`bridge`；设置 `metadata-only` 时
  工作站应公开清单而不中继流。
- `prewarm_circuits`、`max_streams_per_circuit` 和 `circuit_ttl_hint_secs`
  向浏览器显示额外的提示，以便它可以预算并行流和
  了解代理如何积极地重用电路。
- `car_bridge`（可选）指向本地 CAR 存档缓存。 `extension`
  字段控制流目标省略 `*.car` 时附加的后缀；设置
  `allow_zst = true` 直接服务预压缩的 `*.car.zst` 有效负载。
- `kaigi_bridge`（可选）向代理公开假脱机的 Kaigi 路由。的
  `room_policy` 字段通告网桥是否运行在 `public` 或
  `authenticated` 模式，以便浏览器客户端可以预先选择正确的 GAR 标签。
- `sorafs_cli fetch` 暴露 `--local-proxy-mode=bridge|metadata-only` 和
  `--local-proxy-norito-spool=PATH` 覆盖，让操作员切换
  运行时模式或指向备用线轴，无需修改 JSON 策略。
- `downgrade_remediation`（可选）配置自动降级挂钩。
  启用后，协调器会监视中继遥测以发现降级突发
  并且，在 `window_secs` 内配置 `threshold` 后，强制本地
  代理到 `target_mode`（默认 `metadata-only`）。一旦降级停止
  代理在 `cooldown_secs` 之后恢复为 `resume_mode`。使用 `modes`
  数组将触发器范围限定为特定中继角色（默认为入口中继）。

当代理以桥接模式运行时，它提供两个应用程序服务：

- **`norito`** – 客户端的流目标相对于
  `norito_bridge.spool_dir`。目标被清理（没有遍历，没有绝对
  路径），并且当文件缺少扩展名时，将应用配置的后缀
  在有效负载逐字传输到浏览器之前。
- **`car`** – 流目标在 `car_bridge.cache_dir` 内解析，继承
  配置默认扩展，并拒绝压缩有效负载，除非
  `allow_zst` 已设置。成功的网桥之前回复 `STREAM_ACK_OK`
  传输存档字节，以便客户端可以进行管道验证。

在这两种情况下，代理都会提供缓存标签 HMAC（当保护缓存密钥被
握手期间存在）并记录 `norito_*` / `car_*` 遥测原因
代码，以便仪表板可以区分成功、丢失文件和清理
失败一目了然。

`Orchestrator::local_proxy().await` 公开运行句柄，以便调用者可以
读取证书 PEM、获取浏览器清单或请求一个优雅的
当应用程序退出时关闭。

启用后，代理现在提供**清单 v2** 记录。除了现有的
证书和防护缓存密钥，v2 增加：

- `alpn` (`"sorafs-proxy/1"`) 和 `capabilities` 阵列，以便客户可以确认
  他们应该使用的流协议。
- 每次握手 `session_id` 和缓存标记盐（`cache_tagging` 块）
  派生每个会话防护关联性和 HMAC 标签。
- 电路和保护选择提示（`circuit`、`guard_selection`、
  `route_hints`），因此浏览器集成可以在流之前公开更丰富的 UI
  打开。
- `telemetry_v2`，带有用于本地仪器的采样和隐私旋钮。
- 每个 `STREAM_ACK_OK` 包括 `cache_tag_hex`。客户反映了价值
  发出 HTTP 或 TCP 请求时的 `x-sorafs-cache-tag` 标头如此缓存
  守卫选择在静态时保持加密状态。

继续依赖 v1 子集。

## 2. 失败语义

协调器在单个项目之前执行严格的能力和预算检查
字节被传输。失败分为三类：

1. **资格失败（飞行前）。** 提供商缺少范围能力，
   过期的广告或陈旧的遥测数据会记录在记分牌制品中，并且
   从调度中省略。 CLI 摘要填充 `ineligible_providers`
   列出原因，以便操作员可以检查治理偏差而无需刮擦
   日志。
2. **运行时耗尽。** 每个提供程序都会跟踪连续的失败。一旦
   达到配置的 `provider_failure_threshold`，提供商被标记
   `disabled` 剩余的会话时间。如果每个提供商都转变为
   `disabled`，协调器返回
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`。
3. **确定性中止。** 硬限制表现为结构化错误：
   - `MultiSourceError::NoCompatibleProviders` — 清单需要一个块
     其余提供商无法兑现的跨度或一致性。
   - `MultiSourceError::ExhaustedRetries` — 每个块重试预算为
     消耗了。
   - `MultiSourceError::ObserverFailed` — 下游观察者（流挂钩）
     拒绝了已验证的块。

每个错误都会嵌入有问题的块索引，并且在可用时，会嵌入最终的块索引
提供商失败原因。将它们视为释放阻滞剂 - 使用相同的重试
输入将重现故障，直到底层广告、遥测或
提供者的健康状况发生变化。

### 2.1 记分牌持久化

配置 `persist_path` 时，编排器将写入最终记分板
每次跑步后。 JSON 文档包含：

- `eligibility`（`eligible` 或 `ineligible::<reason>`）。
- `weight`（为本次运行分配的归一化权重）。
- `provider` 元数据（标识符、端点、并发预算）。

将记分板快照与发布工件一起存档，以便列入黑名单和
推出决策仍然可以审计。

## 3. 遥测和调试

### 3.1 Prometheus 指标

协调器通过 `iroha_telemetry` 发出以下指标：|公制|标签|描述 |
|--------|--------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`、`region` |飞行中精心策划的获取的衡量标准。 |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`，`region` |记录端到端获取延迟的直方图。 |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`、`region`、`reason` |终端故障计数器（重试耗尽、无提供者、观察者故障）。 |
| `sorafs_orchestrator_retries_total` | `manifest_id`、`provider`、`reason` |每个提供商的重试尝试计数器。 |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`、`provider`、`reason` |导致禁用的会话级提供程序故障的计数器。 |
| `sorafs_orchestrator_policy_events_total` | `region`、`stage`、`outcome`、`reason` |按推出阶段和后备原因分组的匿名政策决策（满足与限制）计数。 |
| `sorafs_orchestrator_pq_ratio` | `region`，`stage` |所选 SoraNet 集之间 PQ 中继份额的直方图。 |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`，`stage` |记分板快照中 PQ 继电器供电比率的直方图。 |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`，`stage` |政策缺口的直方图（目标与实际 PQ 份额之间的差距）。 |
| `sorafs_orchestrator_classical_ratio` | `region`，`stage` |每个会话中使用的经典中继份额的直方图。 |
| `sorafs_orchestrator_classical_selected` | `region`、`stage` |每个会话选择的经典接力计数的直方图。 |

在调整生产旋钮之前，将指标集成到暂存仪表板中。
推荐的布局反映了 SF-6 可观测性计划：

1. **主动获取** — 如果仪表上升而没有匹配的完成，则会发出警报。
2. **重试率** — 当 `retry` 计数器超过历史基线时发出警告。
3. **提供商故障** — 当任何提供商交叉时触发寻呼机警报
   15 分钟内 `session_failure > 0`。

### 3.2 结构化日志目标

协调器将结构化事件发布到确定性目标：

- `telemetry::sorafs.fetch.lifecycle` — `start` 和 `complete` 生命周期
  带有块计数、重试和总持续时间的标记。
- `telemetry::sorafs.fetch.retry` — 重试事件（`provider`、`reason`、
  `attempts`) 用于输入手动分类。
- `telemetry::sorafs.fetch.provider_failure` — 提供商因以下原因被禁用
  重复错误。
- `telemetry::sorafs.fetch.error` — 终端故障总结为
  `reason` 和可选的提供者元数据。

将这些流转发到现有的 Norito 日志管道，以便事件响应
有单一的事实来源。生命周期事件通过以下方式暴露 PQ/经典组合
`anonymity_effective_policy`、`anonymity_pq_ratio`、
`anonymity_classical_ratio` 及其配套计数器，
使得连接仪表板变得简单，而无需抓取指标。期间
GA 推出，将生命周期/重试事件的日志级别固定到 `info`，并依赖
`warn` 表示终端错误。

### 3.3 JSON 总结

`sorafs_cli fetch` 和 Rust SDK 都返回一个结构化摘要，其中包含：

- `provider_reports` 包含成功/失败计数以及提供者是否
  禁用。
- `chunk_receipts` 详细说明哪个提供商满足每个块。
- `retry_stats` 和 `ineligible_providers` 阵列。

调试行为不当的提供商时归档摘要文件 - 收据图
直接到上面的日志元数据。

## 4. 操作清单

1. **CI 中的阶段配置。** 使用目标运行 `sorafs_fetch`
   配置，通过 `--scoreboard-out` 捕获资格视图，以及
   与之前版本的差异。任何意外的不合格提供商都会停止
   促销。
2. **验证遥测。** 确保部署导出 `sorafs.fetch.*`
   在为用户启用多源获取之前，先查看指标和结构化日志。
   缺乏指标通常表明编排器外观没有
   调用。
3. **文件覆盖。** 当申请紧急 `--deny-provider` 或
   `--boost-provider` 设置，将 JSON（或 CLI 调用）提交到您的
   更改日志。回滚必须恢复覆盖并捕获新的记分板
   快照。
4. **重新运行冒烟测试。** 修改重试预算或提供商上限后，
   重新获取规范夹具（`fixtures/sorafs_manifest/ci_sample/`）并
   验证块接收是否保持确定性。

遵循上述步骤可以使协调器行为在各个环境中重现
分阶段推出并提供事件响应所需的遥测。

### 4.1 策略覆盖

操作员可以固定主动传输/匿名阶段，而无需编辑
通过设置 `policy_override.transport_policy` 和
`policy_override.anonymity_policy` 在其 `orchestrator` JSON 中（或提供
`--transport-policy-override=` / `--anonymity-policy-override=` 至
`sorafs_cli fetch`）。当存在任一覆盖时，编排器会跳过
通常的掉电后备：如果无法满足请求的 PQ 层，则
获取失败并显示 `no providers`，而不是悄悄降级。回滚到
默认行为就像清除覆盖字段一样简单。

标准 `iroha_cli app sorafs fetch` 命令公开相同的覆盖标志，
将它们转发到网关客户端，以便临时获取和自动化脚本
共享相同的阶段固定行为。

跨 SDK 装置位于 `fixtures/sorafs_gateway/policy_override/` 下。的
CLI、Rust 客户端、JavaScript 绑定和 Swift Harness 解码
`override.json` 在其奇偶校验套件中，因此对覆盖有效负载的任何更改
必须更新该夹具并重新运行 `cargo test -p iroha`、`npm test` 和
`swift test` 以保持 SDK 一致。始终将再生的夹具连接到
更改审查，以便下游消费者可以区分覆盖合同。

治理需要为每个覆盖提供操作手册条目。记录原因，
预期持续时间，以及更改日志中的回滚触发器，通知 PQ
棘轮旋转通道，并将签名的批准附加到同一工件
存储记分板快照的包。覆盖是为了简短
紧急情况（例如 PQ 警卫限电）；长期的政策变化必须取消
通过正常的理事会投票，使节点汇聚到新的默认值上。

### 4.2 PQ 棘轮消防演习

- **运行手册：** 按照 `docs/source/soranet/pq_ratchet_runbook.md` 获取
  升级/降级演练，包括警卫目录处理和回滚。
- **仪表板：** 导入 `dashboards/grafana/soranet_pq_ratchet.json` 进行监控
  `sorafs_orchestrator_policy_events_total`、掉电率和 PQ 比率平均值
  演习期间。
- **自动化：** `cargo test -p sorafs_orchestrator pq_ratchet_fire_drill_records_metrics`
  执行相同的转换并验证指标增量是否为
  预期在操作员进行现场演练之前。

## 5. 推出剧本

SNNet-5 传输的推出引入了新的警卫选择和治理
证明和政策回退。下面的剧本将序列编码为
在为最终用户启用多源获取以及降级之前遵循
返回直接模式的路径。

### 5.1 开发人员预检（CI/分期）

1. **在 CI 中重新生成记分板。** 运行 `sorafs_cli fetch`（或 SDK
   等效）针对 `fixtures/sorafs_manifest/ci_sample/` 清单
   候选配置。通过以下方式保留记分板
   `--scoreboard-out=artifacts/sorafs/scoreboard.json` 并断言：
   - `anonymity_status=="met"` 和 `anonymity_pq_ratio` 满足目标
     阶段（`anon-guard-pq`、`anon-majority-pq` 或 `anon-strict-pq`）。
   - 确定性的块收据仍然与承诺的黄金集相匹配
     存储库。
2. **验证清单治理。** 检查 CLI/SDK 摘要并确保
   新出现的 `manifest_governance.council_signatures` 阵列包含
   预计理事会指纹。这确认了网关响应发送了 GAR
   信封，并且 `validate_manifest` 接受了它。
3. **行使合规性覆盖。** 从以下位置加载每个管辖区概况：
   `docs/examples/sorafs_compliance_policy.json` 并断言协调器
   发出正确的策略回退（`compliance_jurisdiction_opt_out` 或
   `compliance_blinded_cid_opt_out`）。记录 no 时导致的 fetch 失败
   提供合规的运输。
4. **模拟降级。** 将`transport_policy`翻转为`direct-only`
   测试配置并重新运行获取以确保协调器
   回退到 Torii/QUIC，而不接触 SoraNet 继电器。保留这个 JSON
   版本控制下的变体，因此可以在
   事件。

### 5.2 操作员推出（生产波次）1. **通过 `iroha_config` 暂存配置。** 发布所使用的确切 JSON
   在 CI 中作为 `actual` 层覆盖。确认 Orchestrator pod/二进制文件
   在启动时记录新的配置哈希。
2. **Prime Guard 缓存。** 通过 `--guard-directory` 刷新中继目录
   并将 Norito 保护缓存保留为 `--guard-cache`。验证缓存是否
   签名（如果配置了 `--guard-cache-key`）并存储在版本控制下
   改变控制。
3. **启用遥测仪表板。** 在提供用户流量之前，请确保
   环境发布 `sorafs.fetch.*`、`sorafs_orchestrator_policy_events_total` 和
   代理指标（使用本地 QUIC 代理时）。警报应与
   `anonymity_brownout_effective` 和合规性回退计数器。
4. **运行实时冒烟测试。** 通过每个
   提供商群组（PQ、经典和直接）并确认大块收据，
   CAR 摘要和理事会签名与 CI 基线相匹配。
5. **沟通激活。** 使用以下内容更新推出跟踪器
   `scoreboard.json` 人工制品、防护缓存指纹以及指向
   显示第一次生产获取的清单治理验证的日志。

### 5.3 降级/回滚过程

当事件、PQ 缺陷或监管要求强制回滚时，请遵循
这个确定性序列：

1. **切换传输策略。** 应用 `transport_policy=direct-only`（并且，如果
   立即停止新的 SoraNet 电路建设。
2. **刷新防护状态。** 删除或归档引用的防护缓存文件
   `--guard-cache` 因此后续运行不会尝试重用固定继电器。
   仅当计划快速重新启用并且缓存保留时才跳过此步骤
   有效。
3. **禁用本地代理。** 如果本地 QUIC 代理处于 `bridge` 模式，
   使用 `proxy_mode="metadata-only"` 重新启动协调器或删除
   `local_proxy` 完全阻止。记录端口释放以便工作站和
   浏览器集成恢复为直接 Torii 访问。
4. **明确的合规性覆盖。** 附加司法管辖区选择退出条目（或
   盲态 CID 条目）到受影响有效负载的合规策略，以便
   自动化和仪表板反映了有意的直接模式操作。
5. **捕获审核证据。** 使用 `--scoreboard-out` 运行更改后获取
   并存储 CLI JSON 摘要（包括 `manifest_governance`）
   事件票。

### 5.4 监管部署清单

|检查站|目的|推荐证据|
|------------|---------|----------------------|
|合规政策出台|确认管辖权剥离与 GAR 备案一致。 |已签名的 `soranet_opt_outs.json` 快照 + Orchestrator 配置差异。 |
|显性治理记录|证明每个网关清单均附有理事会签名。 | `sorafs_cli fetch ... --output /dev/null --summary out.json` 和 `manifest_governance.council_signatures` 已存档。 |
|认证库存|跟踪 `compliance.attestations` 中引用的文档。 |将 PDF/JSON 工件与证明摘要和到期时间一起存储。 |
|降级演习已记录 |确保回滚保持确定性。 |显示仅应用直接策略并清除保护缓存的季度试运行记录。 |
|遥测保留 |为监管机构提供取证数据。 |根据策略保留确认 `sorafs.fetch.*` 和合规回退的仪表板导出或 OTEL 快照。 |

运营商应在每个推出窗口之前检查清单并提供
根据要求向治理或监管机构提供证据包。开发者可以复用
当停电或合规性覆盖时，事后数据包的相同工件
在测试期间被触发。