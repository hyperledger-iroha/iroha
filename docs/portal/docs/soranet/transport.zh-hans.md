---
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/transport.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6113bf3d608167f409a9a20b80770bb3ba1d3c050e97492070ee2f9ac706d567
source_last_modified: "2026-01-05T09:28:11.916816+00:00"
translation_last_reviewed: 2026-02-07
id: transport
title: SoraNet transport overview
sidebar_label: Transport Overview
description: Handshake, salt rotation, and capability guidance for the SoraNet anonymity overlay.
translator: machine-google-reviewed
---

:::注意规范来源
:::

SoraNet 是支持 SoraFS 范围获取、Norito RPC 流和未来 Nexus 数据通道的匿名覆盖层。传输计划（路线图项目 **SNNet-1**、**SNNet-1a** 和 **SNNet-1b**）定义了确定性握手、后量子 (PQ) 能力协商和盐轮换计划，以便每个中继、客户端和网关观察相同的安全态势。

## 目标和网络模型

- 在 QUIC v1 上构建三跳电路（入口 → 中间 → 出口），因此滥用者永远不会直接到达 Torii。
- 在 QUIC/TLS 之上分层 Noise XX *混合*握手 (Curve25519 + Kyber768)，以将会话密钥绑定到 TLS 转录本。
- 需要通告 PQ KEM/签名支持、中继角色和协议版本的功能 TLV；润滑未知类型以保持未来扩展的可部署性。
- 每天轮换盲内容盐，并在 30 天内固定保护继电器，以便目录流失无法使客户端匿名。
- 将单元固定在 1024B，注入填充/虚拟单元，并导出确定性遥测数据，以便快速捕获降级尝试。

## 握手管道 (SNNet-1a)

1. **QUIC/TLS 信封** – 客户端通过 QUIC v1 拨号中继，并使用治理 CA 签名的 Ed25519 证书完成 TLS1.3 握手。 TLS 导出器 (`tls-exporter("soranet handshake", 64)`) 播种噪声层，因此转录本是不可分割的。
2. **噪声 XX 混合** – 协议字符串 `Noise_XXhybrid_25519+Kyber768_AESGCM_SHA256`，带有序言 = TLS 导出器。消息流程：

   ```
   -> e, s
   <- e, ee, se, s, pq_ciphertext
   -> ee, se, pq_ciphertext
   ```

   Curve25519 DH 输出和两种 Kyber 封装都混合到最终的对称密钥中。如果未能协商 PQ 材料，握手会彻底中止——不允许仅使用经典的后备方案。

3. **谜题票和代币** – 中继可以在 `ClientHello` 之前要求 Argon2id 工作量证明票。票证是带有长度前缀的帧，携带散列的 Argon2 解决方案并在策略范围内过期：

   ```norito
   struct PowTicketV1 {
       version: u8,
       difficulty: u8,
       expires_at: u64,
       client_nonce: [u8; 32],
       solution: [u8; 32],
   }
   ```

   当来自发行者的 ML-DSA-44 签名根据活动策略和撤销列表进行验证时，前缀为 `SNTK` 的准入令牌会绕过难题。

4. **能力 TLV 交换** – 最终的噪声有效负载传输如下所述的能力 TLV。如果任何强制功能（PQ KEM/签名、角色或版本）丢失或与目录条目不匹配，客户端将中止连接。

5. **转录记录** – 中继记录转录哈希、TLS 指纹和 TLV 内容，以提供降级检测器和合规管道。

## 功能 TLV (SNNet-1c)

功能重用固定的 `typ/length/value` TLV 信封：

```norito
struct CapabilityTLV {
    typ: u16,
    length: u16,
    value: Vec<u8>,
}
```

今天定义的类型：

- `snnet.pqkem` – Kyber 级别（当前推出的为 `kyber768`）。
- `snnet.pqsig` – PQ 签名套件 (`ml-dsa-44`)。
- `snnet.role` – 继电器角色（`entry`、`middle`、`exit`、`gateway`）。
- `snnet.version` – 协议版本标识符。
- `snnet.grease` – 保留范围内的随机填充条目，以确保容忍未来的 TLV。

客户端维护所需 TLV 的允许列表，并在握手失败时忽略或降级它们。中继在其目录微描述符中发布相同的集合，因此验证是确定性的。

## 盐轮换和 CID 致盲 (SNNet-1b)

- 治理发布包含 `(epoch_id, salt, valid_after, valid_until)` 值的 `SaltRotationScheduleV1` 记录。中继和网关从目录发布者获取签名的时间表。
- 客户端在 `valid_after` 应用新的盐，将之前的盐保留 12 小时宽限期，并保留 7 纪元历史记录以容忍延迟更新。
- 规范盲标识符使用：

  ```
  cache_key = BLAKE3("soranet.blinding.canonical.v1" ∥ salt ∥ cid)
  ```

  网关通过 `Sora-Req-Blinded-CID` 接受盲密钥，并在 `Sora-Content-CID` 中回显它。电路/请求致盲 (`CircuitBlindingKey::derive`) 在 `iroha_crypto::soranet::blinding` 中发货。
- 如果继电器错过了一个纪元，它会停止新的电路，直到下载时间表并发出 `SaltRecoveryEventV1`，待命仪表板将其视为寻呼信号。

## 目录数据和保护策略

- 微描述符携带中继身份（Ed25519 + ML-DSA-65）、PQ 密钥、能力 TLV、区域标签、警卫资格和当前公布的盐纪元。
- 客户端将保护设置保留 30 天，并将 `guard_set` 缓存与签名的目录快照一起保留。 CLI 和 SDK 包装器显示缓存指纹，因此可以将推出证据附加到更改评论中。

## 遥测和部署清单

- 生产前导出的指标：
  - `soranet_handshake_success_total{role}`
  - `soranet_handshake_failure_total{reason}`
  - `soranet_handshake_latency_seconds`
  - `soranet_capability_mismatch_total`
  - `soranet_salt_rotation_lag_seconds`
- 警报阈值与盐轮换 SOP SLO 矩阵 (`docs/source/soranet_salt_plan.md#slo--alert-matrix`) 一起存在，并且必须在网络升级之前在 Alertmanager 中进行镜像。
- 警报：5 分钟内故障率 >5%、盐滞后 >15 分钟或生产中观察到的功能不匹配。
- 推出步骤：
  1. 在启用混合握手和 PQ 堆栈的情况下对分段进行中继/客户端互操作性测试。
  2. 演练盐轮换 SOP (`docs/source/soranet_salt_plan.md`) 并将钻孔工件附加到变更记录中。
  3. 在目录中启用能力协商，然后推出到入口中继、中间中继、出口，最后是客户端。
  4. 记录每个阶段的守卫缓存指纹、盐计划和遥测仪表板；将证据包附加到 `status.md`。

遵循此清单可以让运营商、客户和 SDK 团队同步采用 SoraNet 传输，同时满足 SNNet 路线图中捕获的确定性和审核要求。