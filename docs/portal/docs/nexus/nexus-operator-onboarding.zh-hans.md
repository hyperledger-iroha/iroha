---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c958f1044ce6ae35dde36629b55aa3880c3926e60349cb06e80efdd8a3f9211c
source_last_modified: "2025-12-31T15:58:47.310713+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operator-onboarding
title: Sora Nexus data-space operator onboarding
description: Mirror of `docs/source/sora_nexus_operator_onboarding.md`, tracking the end-to-end release checklist for Nexus operators.
translator: machine-google-reviewed
---

:::注意规范来源
此页面镜像 `docs/source/sora_nexus_operator_onboarding.md`。保持两个副本对齐，直到本地化版本到达门户。
:::

# Sora Nexus 数据空间操作员入门

本指南介绍了 Sora Nexus 数据空间操作员在发布版本后必须遵循的端到端流程。它通过描述如何在使节点上线之前将下载的捆绑包/映像、清单和配置模板与全局通道期望保持一致，对双轨运行手册 (`docs/source/release_dual_track_runbook.md`) 和工件选择说明 (`docs/source/release_artifact_selection.md`) 进行了补充。

## 受众和先决条件
- 您已获得 Nexus 计划批准并收到您的数据空间分配（通道索引、数据空间 ID/别名和路由策略要求）。
- 您可以访问发布工程发布的签名发布工件（tarball、图像、清单、签名、公钥）。
- 您已为验证者/观察者角色生成或收到生产密钥材料（Ed25519 节点身份；BLS 共识密钥 + 验证者的 PoP；加上任何机密功能切换）。
- 您可以访问现有的 Sora Nexus 对等点来引导您的节点。

## 步骤 1 — 确认发布配置文件
1. 确定为您提供的网络别名或链 ID。
2. 签出此存储库时运行 `scripts/select_release_profile.py --network <alias>`（或 `--chain-id <id>`）。帮助程序查阅 `release/network_profiles.toml` 并打印要部署的配置文件。对于 Sora Nexus，响应必须是 `iroha3`。对于任何其他值，请停止并联系发布工程。
3. 记下发布公告引用的版本标签（例如 `iroha3-v3.2.0`）；您将使用它来获取文物和清单。

## 步骤 2 — 检索并验证人工制品
1. 下载 `iroha3` 捆绑包 (`<profile>-<version>-<os>.tar.zst`) 及其配套文件（`.sha256`、可选的 `.sig/.pub`、`<profile>-<version>-manifest.json` 和 `<profile>-<version>-image.json`（如果部署容器））。
2. 开箱前验证完整性：
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub \
       -signature iroha3-<version>-linux.tar.zst.sig \
       iroha3-<version>-linux.tar.zst
   ```
   如果您使用硬件支持的 KMS，请将 `openssl` 替换为组织批准的验证程序。
3. 检查 tarball 内的 `PROFILE.toml` 和 JSON 清单以确认：
   - `profile = "iroha3"`
   - `version`、`commit` 和 `built_at` 字段与发布公告相符。
   - 操作系统/架构与您的部署目标相匹配。
4. 如果使用容器镜像，请对`<profile>-<version>-<os>-image.tar`重复哈希/签名验证，并确认`<profile>-<version>-image.json`中记录的镜像ID。

## 步骤 3 — 从模板进行阶段配置
1. 解压捆绑包并将 `config/` 复制到节点将读取其配置的位置。
2. 将 `config/` 下的文件视为模板：
   - 将 `public_key`/`private_key` 替换为您的生产 Ed25519 密钥。如果节点将从 HSM 获取私钥，则从磁盘中删除私钥；更新配置以指向 HSM 连接器。
   - 调整 `trusted_peers`、`network.address` 和 `torii.address`，以便它们反映您可访问的接口以及分配给您的引导对等点。
   - 使用面向操作员的 Torii 端点（包括 TLS 配置，如果适用）以及您为操作工具提供的凭据更新 `client.toml`。
3. 保留捆绑包中提供的链 ID，除非治理明确指示，否则全局通道需要单个规范链标识符。
4. 计划启动带有 Sora 配置文件标志的节点：`irohad --sora --config <path>`。当该标志不存在时，配置加载器将拒绝 SoraFS 或多通道设置。

## 步骤 4 — 调整数据空间元数据和路由
1. 编辑 `config/config.toml`，使 `[nexus]` 部分与 Nexus 委员会提供的数据空间目录相匹配：
   - `lane_count` 必须等于当前时期启用的通道总数。
   - `[[nexus.lane_catalog]]` 和 `[[nexus.dataspace_catalog]]` 中的每个条目必须包含唯一的 `index`/`id` 和商定的别名。不要删除现有的全局条目；如果理事会分配了额外的数据空间，请添加您委托的别名。
   - 确保每个数据空间条目包括 `fault_tolerance (f)`；车道接力委员会的规模为 `3f+1`。
2. 更新 `[[nexus.routing_policy.rules]]` 以获取为您提供的策略。默认模板将治理指令路由到通道 `1`，将合约部署路由到通道 `2`；附加或修改规则，以便将发往您的数据空间的流量转发到正确的通道和别名。在更改规则顺序之前与发布工程人员协调。
3. 查看 `[nexus.da]`、`[nexus.da.audit]` 和 `[nexus.da.recovery]` 阈值。运营商应保持理事会批准的值；仅在更新的政策获得批准后才进行调整。
4. 在操作跟踪器中记录最终配置。双轨发布操作手册需要将有效的 `config.toml`（已编辑机密）附加到入职票据。

## 步骤 5 — 飞行前验证
1. 在加入网络之前运行内置配置验证器：
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   这会打印已解析的配置，如果目录/路由条目不一致或者创世和配置不一致，则会提前失败。
2. 如果部署容器，请在使用 `docker load -i <profile>-<version>-<os>-image.tar` 加载映像后在映像中运行相同的命令（请记住包含 `--sora`）。
3. 检查日志中有关占位符通道/数据空间标识符的警告。如果出现任何情况，请重新访问步骤 4 - 生产部署不得依赖于模板附带的占位符 ID。
4. 执行本地冒烟程序（例如，使用 `iroha_cli` 提交 `FindNetworkStatus` 查询，确认遥测端点公开 `nexus_lane_state_total`，并验证流密钥是否按要求轮换或导入）。

## 步骤 6 — 切换和移交
1. 将经过验证的 `manifest.json` 和签名工件存储在发布票据中，以便审核员可以重现您的检查。
2. 通知Nexus Operations该节点已准备好引入；包括：
   - 节点身份（对等 ID、主机名、Torii 端点）。
   - 有效的车道/数据空间目录和路由策略值。
   - 您验证的二进制文件/图像的哈希值。
3. 与 `@nexus-core` 协调最终的同伴准入（八卦种子和通道分配）。在获得批准之前请勿加入网络； Sora Nexus 强制执行确定性车道占用并需要更新的入场清单。
4. 节点上线后，使用您引入的任何替代更新 Runbook，并记下发布标签，以便下一次迭代可以从此基线开始。

## 参考清单
- [ ] 发布配置文件已验证为 `iroha3`。
- [ ] 捆绑包/图像哈希值和签名已验证。
- [ ] 密钥、对等地址和 Torii 端点已更新为生产值。
- [ ] Nexus 通道/数据空间目录和路由策略匹配委员会分配。
- [ ] 配置验证器 (`irohad --sora --config … --trace-config`) 通过，没有警告。
- [ ] 清单/签名存档在入职通知单和通知的操作人员中。

有关 Nexus 迁移阶段和遥测预期的更广泛背景，请查看 [Nexus 转换说明](./nexus-transition-notes)。