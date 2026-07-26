---
id: puzzle-service-operations
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Puzzle Service Operations Guide
sidebar_label: Puzzle Service Ops
description: Operating the `soranet-puzzle-service` daemon for Argon2/ML-DSA admission tickets.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

# Puzzle 服务操作指南

`soranet-puzzle-service` 守护程序 (`tools/soranet-puzzle-service/`) 问题
Argon2 支持的入场券反映了中继的 `pow.puzzle.*` 政策
配置后，代表边缘中继代理 ML-DSA 准入令牌。
它公开了五个 HTTP 端点：

- `GET /healthz` – 活性探针。
- `GET /v1/puzzle/config` – 返回拉取的有效 PoW/谜题参数
  来自继电器 JSON（`handshake.descriptor_commit_hex`、`pow.*`）。
- `POST /v1/puzzle/mint` – 铸造一张 Argon2 票证；可选的 JSON 正文
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  请求更短的 TTL（限制到策略窗口），将票证绑定到
  成绩单哈希，并返回中继签名票+签名指纹
  配置签名密钥时。
- `GET /v1/token/config` – 当 `pow.token.enabled = true` 时，返回活动的
  准入令牌策略（发行者指纹、TTL/时钟偏差边界、中继 ID、
  和合并的撤销集）。
- `POST /v1/token/mint` – 铸造一个与所提供的绑定的 ML-DSA 准入令牌
  恢复哈希；请求正文接受 `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`。

该服务生成的票证在
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
集成测试，还在容量 DoS 期间执行中继节流
场景。【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## 配置代币发行

设置 `pow.token.*` 下的中继 JSON 字段（请参阅
以 `tools/soranet-relay/deploy/config/relay.entry.json` 为例）启用
ML-DSA 代币。至少提供发行者公钥和可选的
撤销清单：

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

拼图服务重用这些值并自动重新加载 Norito
运行时的 JSON 撤销文件。使用 `soranet-admission-token` CLI
(`cargo run -p soranet-relay --bin soranet_admission_token`) 铸造和检验
脱机令牌，将 `token_id_hex` 条目附加到吊销文件中，并进行审核
将更新推送到生产之前的现有凭据。

通过 CLI 标志将发行者密钥传递给拼图服务：

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

当机密由带外管理时，`--token-secret-hex` 也可用
工具管道。吊销文件监视程序使 `/v1/token/config` 保持最新状态；
使用 `soranet-admission-token revoke` 命令协调更新以避免滞后
撤销状态。

在中继 JSON 中设置 `pow.signed_ticket_public_key_hex` 以通告 ML-DSA-44 公共
用于验证签名 PoW 票据的密钥； `/v1/puzzle/config` 与该键及其 BLAKE3 相呼应
指纹 (`signed_ticket_public_key_fingerprint_hex`)，以便客户端可以固定验证器。
签名票证根据中继 ID 和转录本绑定进行验证，并共享相同的内容
撤销存储；当签名票证验证者处于有效状态时，原始 74 字节 PoW 票证仍然有效
配置。通过 `--signed-ticket-secret-hex` 传递签名者机密或
`--signed-ticket-secret-path` 启动拼图服务时；启动拒绝不匹配
密钥对（如果密钥未根据 `pow.signed_ticket_public_key_hex` 进行验证）。
`POST /v1/puzzle/mint` 接受 `"signed": true`（和可选的 `"transcript_hash_hex"`）
返回 Norito 编码的签名票证以及原始票证字节；回应包括
`signed_ticket_b64` 和 `signed_ticket_fingerprint_hex` 帮助跟踪重放指纹。
如果未配置签名者密钥，则 `signed = true` 的请求将被拒绝。

## 密钥轮换手册

1. **收集新的描述符提交。** 治理发布中继
   目录包中的描述符提交。将十六进制字符串复制到
   `handshake.descriptor_commit_hex` 继电器内部 JSON 配置共享
   与拼图服务。
2. **查看拼图策略范围。** 确认更新
   `pow.puzzle.{memory_kib,time_cost,lanes}` 值与版本一致
   计划。运营商应保持 Argon2 配置的确定性
   继电器（最小 4MiB 内存，1≤通道≤16）。
3. **分阶段重启。** 治理后重新加载 systemd 单元或容器
   宣布轮换切换。该服务不支持热重载；一个
   需要重新启动才能获取新的描述符提交。
4. **验证。** 通过 `POST /v1/puzzle/mint` 出票并确认
   返回的 `difficulty` 和 `expires_at` 与新策略匹配。浸泡报告
   (`docs/source/soranet/reports/pow_resilience.md`) 捕获预期延迟
   界限供参考。启用令牌后，获取 `/v1/token/config` 到
   确保公布的发行者指纹和撤销计数匹配
   预期值。

## 紧急禁用程序

1. 在共享继电器配置中设置 `pow.puzzle.enabled = false`。保留
   `pow.required = true` 如果 hashcash 后备票证必须保持强制性。
2. 可选择强制 `pow.emergency` 条目拒绝过时的描述符，同时
   Argon2 门已离线。
3. 重新启动中继和拼图服务以应用更改。
4. 监控`soranet_handshake_pow_difficulty`，确保难度降至
   预期的 hashcash 值，并验证 `/v1/puzzle/config` 报告
   `puzzle = null`。

## 监控和警报

- **延迟 SLO：** 跟踪 `soranet_handshake_latency_seconds` 并保留 P95
  低于 300 毫秒。浸泡测试偏移为防护装置提供校准数据
  节流阀。【docs/source/soranet/reports/pow_resilience.md:1】
- **配额压力：** 使用 `soranet_guard_capacity_report.py` 和中继指标
  调整 `pow.quotas` 冷却时间 (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **拼图对齐：** `soranet_handshake_pow_difficulty` 应匹配
  `/v1/puzzle/config` 返回的难度。分歧表明中继失效
  配置或重启失败。
- **令牌准备情况：** 如果 `/v1/token/config` 降至 `enabled = false`，则发出警报
  意外地或者如果 `revocation_source` 报告过时的时间戳。运营商
  每当令牌被使用时，应通过 CLI 轮换 Norito 吊销文件
  已退休以保持此端点的准确性。
- **服务运行状况：** 以通常的活跃节奏和警报探测 `/healthz`
  如果 `/v1/puzzle/mint` 返回 HTTP 500 响应（表示 Argon2 参数
  不匹配或 RNG 失败）。通过 HTTP 4xx/5xx 出现令牌铸造错误
  `/v1/token/mint` 上的回复；将重复失败视为寻呼条件。

## 合规性和审计日志记录

继电器发出结构化 `handshake` 事件，其中包括节流原因和
冷却时间。确保中描述的合规管道
`docs/source/soranet/relay_audit_pipeline.md` 摄取这些日志所以令人困惑
政策变化仍可审计。当拼图门启用时，存档
铸造的票据样本和 Norito 配置快照随推出
未来审计的票据。在维护窗口之前铸造的入场代币
应使用其 `token_id_hex` 值进行跟踪并插入到
一旦过期或被撤销，撤销文件。