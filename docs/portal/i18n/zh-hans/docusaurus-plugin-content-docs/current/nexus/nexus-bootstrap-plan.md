---
id: nexus-bootstrap-plan
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Nexus bootstrap & observability
description: Operational plan for bringing the core Nexus validator cluster online before layering SoraFS and SoraNet services.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
此页面镜像 `docs/source/soranexus_bootstrap_plan.md`。保持两个副本对齐，直到本地化版本登陆门户。
:::

# Sora Nexus 引导程序和可观测性计划

## 目标
- 使用治理密钥、Torii API 和共识监控建立基础 Sora Nexus 验证器/观察者网络。
- 在启用 SoraFS/SoraNet 搭载部署之前验证核心服务（Torii、共识、持久性）。
- 建立 CI/CD 工作流程和可观察性仪表板/警报，以确保网络健康。

## 先决条件
- HSM 或 Vault 中提供治理密钥材料（理事会多重签名、委员会密钥）。
- 主/辅助区域中的基准基础设施（Kubernetes 集群或裸机节点）。
- 更新了引导程序配置（`configs/nexus/bootstrap/*.toml`），反映了最新的共识参数。

## 网络环境
- 运行两个具有不同网络前缀的 Nexus 环境：
- **Sora Nexus（主网）** – 生产网络前缀 `nexus`，托管规范治理和 SoraFS/SoraNet 搭载服务（链 ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`）。
- **Sora Testus（测试网）** – 暂存网络前缀 `testus`，镜像主网配置以进行集成测试和预发布验证（链 UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`）。
- 为每个环境维护单独的创世文件、治理密钥和基础设施足迹。 Testus 充当升级到 Nexus 之前所有 SoraFS/SoraNet 部署的试验场。
- CI/CD 管道应首先部署到 Testus，执行自动冒烟测试，并在检查通过后需要手动升级到 Nexus。
- 参考配置包位于 `configs/soranexus/nexus/`（主网）和 `configs/soranexus/testus/`（测试网）下，每个包含示例 `config.toml`、`genesis.json` 和 Torii 准入目录。

## 步骤 1 – 配置审查
1. 审核现有文档：
   - `docs/source/nexus/architecture.md`（共识，Torii 布局）。
   - `docs/source/nexus/deployment_checklist.md`（基础要求）。
   - `docs/source/nexus/governance_keys.md`（密钥保管程序）。
2. 验证创世文件 (`configs/nexus/genesis/*.json`) 与当前验证者名单和质押权重是否一致。
3、确认网络参数：
   - 共识委员会规模和法定人数。
   - 区块间隔/最终性阈值。
   - Torii 服务端口和 TLS 证书。

## 步骤 2 – Bootstrap 集群部署
1. 配置验证节点：
   - 使用持久卷部署 `irohad` 实例（验证器）。
   - 确保网络防火墙规则允许节点之间达成共识和 Torii 流量。
2. 在每个使用 TLS 的验证器上启动 Torii 服务 (REST/WebSocket)。
3. 部署观察者节点（只读）以获得额外的弹性。
4. 运行引导脚本（`scripts/nexus_bootstrap.sh`）来分发创世币、启动共识并注册节点。
5. 执行冒烟测试：
   - 通过 Torii (`iroha_cli tx submit`) 提交测试交易。
   - 通过遥测验证区块生产/最终性。
   - 检查验证者/观察者之间的账本复制。

## 步骤 3 – 治理和密钥管理
1. 加载理事会多重签名配置；确认可以提交并批准治理提案。
2. 安全存储共识/委员会密钥；配置带有访问日志记录的自动备份。
3. 设置紧急密钥轮换程序 (`docs/source/nexus/key_rotation.md`) 并验证运行手册。

## 步骤 4 – CI/CD 集成
1.配置管道：
   - 构建并发布验证器/Torii 图像（GitHub Actions 或 GitLab CI）。
   - 自动配置验证（lint genesis、验证签名）。
   - 用于登台和生产集群的部署管道（Helm/Kustomize）。
2. 在 CI 中实施冒烟测试（启动临时集群，运行规范事务套件）。
3. 添加失败部署的回滚脚本和文档 Runbook。

## 步骤 5 – 可观察性和警报
1. 每个区域部署监控堆栈（Prometheus + Grafana + Alertmanager）。
2. 收集核心指标：
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - 通过 Loki/ELK 记录 Torii 和共识服务。
3. 仪表板：
   - 共识健康（区块高度、最终性、对等状态）。
   - Torii API 延迟/错误率。
   - 治理交易和提案状态。
4. 警报：
   - 区块生产停滞（>2 个区块间隔）。
   - 对等计数低于法定人数。
   - Torii 错误率峰值。
   - 治理提案队列积压。

## 步骤 6 – 验证和移交
1. 运行端到端验证：
   - 提交治理提案（例如参数更改）。
   - 通过理事会批准进行处理，以确保治理管道发挥作用。
   - 运行账本状态差异以确保一致性。
2. 记录待命运行手册（事件响应、故障转移、扩展）。
3. 向 SoraFS/SoraNet 团队传达准备情况；确认搭载部署可以指向 Nexus 节点。

## 实施清单
- [ ] 创世/配置审核已完成。
- [ ] 验证者和观察者节点以健康共识部署。
- [ ] 治理密钥已加载，建议已测试。
- [ ] CI/CD 管道正在运行（构建 + 部署 + 冒烟测试）。
- [ ] 可观察性仪表板带有警报。
- [ ] 将文档交付给下游团队。