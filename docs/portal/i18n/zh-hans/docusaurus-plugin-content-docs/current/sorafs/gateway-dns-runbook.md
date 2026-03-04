---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS 网关和 DNS 启动运行手册

此门户副本反映了规范运行手册
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)。
它捕获了去中心化 DNS 和网关的操作护栏
工作流，以便网络、运营和文档主管可以排练
2025 年 3 月启动之前的自动化堆栈。

## 范围和可交付成果

- 通过演练确定性来绑定 DNS (SF-4) 和网关 (SF-5) 里程碑
  主机派生、解析器目录发布、TLS/GAR 自动化和证据
  捕获。
- 保留启动输入（议程、邀请、出席跟踪器、GAR 遥测）
  快照）与最新的所有者分配同步。
- 为治理审查者生成可审计的工件包：解析器
  目录发行说明、网关探测日志、一致性工具输出以及
  Docs/DevRel 摘要。

## 角色和职责

|工作流程 |职责|所需文物|
|------------|--------------------------------|--------------------|
|网络 TL（DNS 堆栈）|维护确定性主机计划、运行 RAD 目录版本、发布解析器遥测输入。 | `artifacts/soradns_directory/<ts>/`，`docs/source/soradns/deterministic_hosts.md` 的差异，RAD 元数据。 |
|运营自动化主管（网关）|执行 TLS/ECH/GAR 自动化演练，运行 `sorafs-gateway-probe`，更新 PagerDuty 挂钩。 | `artifacts/sorafs_gateway_probe/<ts>/`，探测 JSON，`ops/drill-log.md` 条目。 |
| QA 协会和工具工作组 |运行 `ci/check_sorafs_gateway_conformance.sh`、整理装置、存档 Norito 自我认证包。 | `artifacts/sorafs_gateway_conformance/<ts>/`、`artifacts/sorafs_gateway_attest/<ts>/`。 |
|文档/开发版本 |记录会议记录、更新设计预读和附录，并在此门户中发布证据摘要。 |更新了 `docs/source/sorafs_gateway_dns_design_*.md` 文件和部署说明。 |

## 输入和先决条件

- 确定性主机规范 (`docs/source/soradns/deterministic_hosts.md`) 和
  解析器证明脚手架 (`docs/source/soradns/resolver_attestation_directory.md`)。
- 网关文物：操作员手册、TLS/ECH 自动化助手、
  直接模式指导以及 `docs/source/sorafs_gateway_*` 下的自我认证工作流程。
- 工具：`cargo xtask soradns-directory-release`，
  `cargo xtask sorafs-gateway-probe`, `scripts/telemetry/run_soradns_transparency_tail.sh`,
  `scripts/sorafs_gateway_self_cert.sh` 和 CI 助手
  （`ci/check_sorafs_gateway_conformance.sh`、`ci/check_sorafs_gateway_probe.sh`）。
- 秘密：GAR 发布密钥、DNS/TLS ACME 凭证、PagerDuty 路由密钥、
  Torii 用于解析器获取的身份验证令牌。

## 飞行前检查清单

1. 通过更新确认与会者和议程
   `docs/source/sorafs_gateway_dns_design_attendance.md` 并循环
   当前议程（`docs/source/sorafs_gateway_dns_design_agenda.md`）。
2. 舞台文物根源，例如
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` 和
   `artifacts/soradns_directory/<YYYYMMDD>/`。
3. 刷新固定装置（GAR 清单、RAD 证明、网关一致性包）和
   确保 `git submodule` 状态与最新的排练标签匹配。
4. 验证机密（Ed25519 发布密钥、ACME 帐户文件、PagerDuty 令牌）是否
   显示并匹配保管库校验和。
5. 事先对遥测目标（Pushgateway 端点、GAR Grafana 板）进行冒烟测试
   到钻头。

## 自动化排练步骤

### 确定性主机映射和 RAD 目录发布

1. 针对建议的清单运行确定性主机派生助手
   设置并确认没有漂移
   `docs/source/soradns/deterministic_hosts.md`。
2. 生成解析器目录包：

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3.在里面记录打印的目录ID、SHA-256和输出路径
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` 和开球
   分钟。

### DNS 遥测捕获

- 尾部解析器透明度日志使用≥10分钟
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`。
- 导出 Pushgateway 指标并在运行时存档 NDJSON 快照
  身份证号目录。

### 网关自动化演练

1. 执行 TLS/ECH 探测：

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. 运行一致性工具 (`ci/check_sorafs_gateway_conformance.sh`) 并
   自认证助手 (`scripts/sorafs_gateway_self_cert.sh`) 刷新
   Norito 证明捆绑包。
3. 捕获 PagerDuty/Webhook 事件以证明自动化路径始终有效
   结束。

### 证据包装

- 使用时间戳、参与者和探测哈希值更新 `ops/drill-log.md`。
- 将工件存储在运行 ID 目录下并发布执行摘要
  在 Docs/DevRel 会议纪要中。
- 在启动审核之前链接治理票证中的证据包。

## 会议引导和证据移交

- **主持人时间表：**  
  - T‑24h — 项目管理在 `#nexus-steering` 中发布提醒 + 议程/出勤快照。  
  - T-2h — Networking TL 刷新 GAR 遥测快照并在 `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` 中记录增量。  
  - T‑15m — Ops Automation 验证探针准备情况并将活动运行 ID 写入 `artifacts/sorafs_gateway_dns/current`。  
  - 在通话期间 - 主持人分享此操作手册并指派一名实时抄写员； Docs/DevRel 捕获内联操作项。
- **分钟模板：** 复制骨架
  `docs/source/sorafs_gateway_dns_design_minutes.md`（也反映在门户中
  包）并为每个会话提交一个填充实例。包括与会者名册、
  决策、行动项目、证据哈希和突出风险。
- **证据上传：** 从排练中压缩 `runbook_bundle/` 目录，
  附上渲染的会议记录 PDF，在会议记录 + 议程中记录 SHA-256 哈希值，
  然后在上传土地后 ping 治理审核者别名
  `s3://sora-governance/sorafs/gateway_dns/<date>/`。

## 证据快照（2025 年 3 月启动）

路线图和治理中引用的最新排练/现场制品
分钟住在 `s3://sora-governance/sorafs/gateway_dns/` 桶下。哈希值
下面镜像了规范清单 (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)。

- **试运行 — 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`)**
  - 捆绑包 tarball：`b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - 会议纪要 PDF：`cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **现场研讨会 — 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _（待上传：`gateway_dns_minutes_20250303.pdf` — Docs/DevRel 将在渲染的 PDF 放入捆绑包后附加 SHA-256。）_

## 相关材料

- [网关操作手册](./operations-playbook.md)
- [SoraFS 可观测性计划](./observability-plan.md)
- [去中心化 DNS 和网关跟踪器](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)