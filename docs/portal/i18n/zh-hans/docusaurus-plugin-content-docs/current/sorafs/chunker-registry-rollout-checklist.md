---
id: chunker-registry-rollout-checklist
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Chunker Registry Rollout Checklist
sidebar_label: Chunker Rollout Checklist
description: Step-by-step rollout plan for chunker registry updates.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::注意规范来源
:::

# SoraFS 注册表部署清单

该清单记录了推广新分块配置文件所需的步骤或
治理后从审核到生产的提供商准入捆绑包
宪章已获得批准。

> **范围：** 适用于所有修改的版本
> `sorafs_manifest::chunker_registry`、提供者入学信封，或
> 规范夹具捆绑包 (`fixtures/sorafs_chunker/*`)。

## 1. 飞行前验证

1. 重新生成装置并验证确定性：
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. 确认确定性哈希值
   `docs/source/sorafs/reports/sf1_determinism.md`（或相关配置文件
   报告）与重新生成的工件相匹配。
3. 确保 `sorafs_manifest::chunker_registry` 编译为
   `ensure_charter_compliance()` 通过运行：
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 更新提案档案：
   - `docs/source/sorafs/proposals/<profile>.json`
   - `docs/source/sorafs/council_minutes_*.md` 下的理事会会议记录条目
   - 决定论报告

## 2. 治理签核

1. 向 Sora 提交工具工作组报告和提案摘要
   议会基础设施小组。
2. 将批准详细信息记录在
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`。
3. 将议会签名的信封与赛程表一起公布：
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. 验证信封可通过治理获取帮助程序访问：
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. 分阶段部署

请参阅 [暂存清单手册](./staging-manifest-playbook) 了解
这些步骤的详细演练。

1. 部署 Torii，并启用 `torii.sorafs` 发现和准入
   强制执行已开启 (`enforce_admission = true`)。
2. 将批准的提供者准入信封推送至暂存登记处
   `torii.sorafs.discovery.admission.envelopes_dir` 引用的目录。
3. 验证提供商广告是否通过发现 API 传播：
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. 使用治理标头练习清单/计划端点：
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. 确认遥测仪表板 (`torii_sorafs_*`) 和警报规则报告
   新的配置文件没有错误。

## 4. 生产部署

1. 对生产 Torii 节点重复暂存步骤。
2. 宣布激活窗口（日期/时间、宽限期、回滚计划）
   运营商和SDK渠道。
3. 合并包含以下内容的发布 PR：
   - 更新了固定装置和信封
   - 文档变更（章程参考、决定论报告）
   - 路线图/状态刷新
4. 标记版本并存档已签名的工件以查找出处。

## 5. 推出后审核

1. 捕获最终指标（发现计数、获取成功率、错误
   直方图）推出后 24 小时。
2. 使用简短摘要和确定性报告的链接更新 `status.md`。
3. 将任何后续任务（例如，其他配置文件创作指南）归档到
   `roadmap.md`。