---
lang: zh-hans
direction: ltr
source: docs/source/ministry/emergency_canon_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: afd5db8a761f8cc56dcd8f67f047f06b45c3211738fedb6dfff326d84b4e8a68
source_last_modified: "2026-01-22T14:45:02.098548+00:00"
translation_last_reviewed: 2026-02-07
title: Emergency Canon & TTL Policy
summary: Reference implementation notes for roadmap item MINFO-6a covering denylist tiers, TTL enforcement, and governance evidence requirements.
translator: machine-google-reviewed
---

# 紧急佳能和 TTL 政策 (MINFO-6a)

路线图参考：**MINFO-6a — 紧急规范和 TTL 政策**。

本文档定义了现在在 Torii 和 CLI 中提供的拒绝列表分层规则、TTL 实施和治理义务。运营商在发布新条目或调用紧急规则之前必须遵守这些规则。

## 等级定义

|等级 |默认 TTL |审查窗口|要求|
|------|-------------|----------------|----------------|
|标准| 180 天 (`torii.sorafs_gateway.denylist.standard_ttl`) |不适用 |必须提供 `issued_at`。 `expires_at` 可以省略； Torii 默认为 `issued_at + standard_ttl` 并拒绝更长的窗口。 |
|紧急| 30 天 (`torii.sorafs_gateway.denylist.emergency_ttl`) | 7 天 (`torii.sorafs_gateway.denylist.emergency_review_window`) |需要引用预先批准的规范的非空 `emergency_canon` 标签（例如 `csam-hotline`）。 `issued_at` + `expires_at` 必须在 30 天的窗口内提交，并且审查证据必须引用自动生成的截止日期 (`issued_at + review_window`)。 |
|永久|无有效期 |不适用 |保留给绝大多数治理决策。参赛作品必须引用非空 `governance_reference`（投票 ID、宣言哈希等）。 `expires_at` 被拒绝。 |

默认值仍然可以通过 `torii.sorafs_gateway.denylist.*` 进行配置，`iroha_cli` 镜像限制以在 Torii 重新加载文件之前捕获无效条目。

## 工作流程

1. **准备元数据：** 在每个 JSON 条目 (`docs/source/sorafs_gateway_denylist_sample.json`) 中包含 `policy_tier`、`issued_at`、`expires_at`（如果适用）和 `emergency_canon`/`governance_reference`。
2. **本地验证：** 运行 `iroha app sorafs gateway lint-denylist --path <denylist.json>`，以便 CLI 在提交或固定文件之前强制执行特定于层的 TTL 和必填字段。
3. **发布证据：** 将条目中引用的规范 ID 或治理参考附加到 GAR 案例包（议程包、公投记录等），以便审计员可以追踪决策。
4. **查看紧急条目：** 紧急规则会在 30 天内自动过期。运营商必须在 7 天的时间内完成事后审查，并将结果记录在部门跟踪器/SoraFS 证据存储中。
5. **重新加载 Torii：** 验证后，通过 `torii.sorafs_gateway.denylist.path` 部署拒绝列表路径并重新启动/重新加载 Torii；运行时在允许条目之前强制执行相同的限制。

## 工具和参考

- 运行时策略实施位于 `sorafs::gateway::denylist` (`crates/iroha_torii/src/sorafs/gateway/denylist.rs`) 中，加载程序现在在解析 `torii.sorafs_gateway.denylist.*` 输入时应用层元数据。
- CLI 验证镜像 `GatewayDenylistRecord::validate` (`crates/iroha_cli/src/commands/sorafs.rs`) 内的运行时语义。当 TTL 超过配置的窗口或缺少强制性规范/管理参考时，linter 会失败。
- 配置旋钮在 `torii.sorafs_gateway.denylist` (`crates/iroha_config/src/parameters/{defaults,actual.rs,user.rs}`) 下定义，因此如果治理批准不同的界限，操作员可以调整 TTL/审查截止日期。
- 公共样本拒绝列表 (`docs/source/sorafs_gateway_denylist_sample.json`) 现在说明了所有三个层，应用作新条目的规范模板。这些护栏通过编纂紧急规范列表、防止无限制的 TTL 以及强制永久区块的明确治理证据来满足路线图项目 **MINFO-6a** 的要求。

## 注册自动化和证据导出

紧急规范批准必须产生确定性的注册表快照和
Torii 加载拒绝列表之前的 diff 包。下的工具
`xtask/src/sorafs.rs` 加上 CI 线束 `ci/check_sorafs_gateway_denylist.sh`
覆盖整个工作流程。

### 规范包生成

1. 在工作中暂存原始条目（通常是由治理审查的文件）
   目录。
2. 通过以下方式规范化并密封 JSON：
   ```bash
   cargo xtask sorafs-gateway denylist pack \
     --input path/to/denylist.json \
     --out artifacts/ministry/denylist_registry/$(date +%Y%m%dT%H%M%SZ) \
     --label ministry-emergency \
     --force
   ```
   该命令发出一个签名友好的 `.json` 捆绑包，即 Norito `.to`
   信封，以及治理审核者期望的 Merkle-root 文本文件。
   将目录存储在 `artifacts/ministry/denylist_registry/` 下（或您的
   选择证据桶）所以 `scripts/ministry/transparency_release.py` 可以
   稍后使用 `--artifact denylist_bundle=<path>` 获取。
3. 在推送之前，将生成的 `checksums.sha256` 与捆绑包放在一起
   至 SoraFS/GAR。 CI 的 `ci/check_sorafs_gateway_denylist.sh` 执行相同的操作
   `pack` 帮助程序针对样本拒绝名单，以保证工具正常运行
   每次发布。

### 差异 + 审核包

1. 使用以下命令将新包与之前的生产快照进行比较
   xtask 差异助手：
   ```bash
   cargo xtask sorafs-gateway denylist diff \
     --old artifacts/ministry/denylist_registry/2026-05-01/denylist_old.json \
     --new artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --report-json artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
   JSON 报告列出了所有添加/删除并反映了证据
   `MinistryDenylistChangeV1` 消耗的结构（由
   `docs/source/sorafs_gateway_self_cert.md` 和合规计划）。
2. 将 `denylist_diff.json` 附加到每个佳能请求（它证明有多少
   条目被触及，哪一层发生了变化，以及哪些证据哈希映射到了
   规范束）。
3. 当自动生成差异（CI 或发布管道）时，导出
   `denylist_diff.json` 路径通过 `--artifact denylist_diff=<path>` 所以
   透明度清单将其与经过净化的指标一起记录。相同的 CI
   帮助程序接受运行 CLI 摘要步骤的 `--evidence-out <path>` 并
   将生成的 JSON 复制到请求的位置以供以后发布。

### 发布和透明度1. 将 pack + diff 工件放入季度透明度目录中
   （`artifacts/ministry/transparency/<YYYY-Q>/denylist/`）。透明度
   然后发布助手可以包含它们：
   ```bash
   scripts/ministry/transparency_release.py \
     --quarter 2026-Q3 \
     --output-dir artifacts/ministry/transparency/2026-Q3 \
     --sanitized artifacts/ministry/transparency/2026-Q3/sanitized_metrics.json \
     --dp-report artifacts/ministry/transparency/2026-Q3/dp_report.json \
     --artifact denylist_bundle=artifacts/ministry/denylist_registry/2026-05-14/denylist_new.json \
     --artifact denylist_diff=artifacts/ministry/denylist_registry/2026-05-14/denylist_diff.json
   ```
2. 在季度报告中引用生成的bundle/diff
   (`docs/source/ministry/reports/<YYYY-Q>.md`) 并将相同的路径附加到
   GAR 投票包，以便审计员可以重播证据线索，而无需访问
   内部 CI。 `ci/check_sorafs_gateway_denylist.sh --evidence-out \
   现在工件/部委/denylist_registry//denylist_evidence.json`
   执行 pack/diff/evidence 试运行（调用 `iroha_cli app sorafs gateway
   证据`在引擎盖下），这样自动化就可以将摘要与
   规范束。
3. 发布后，通过以下方式锚定治理有效负载
   `cargo xtask ministry-transparency anchor`（由自动调用
   `transparency_release.py`（当提供 `--governance-dir` 时），因此
   拒绝列表注册表摘要与透明度出现在同一 DAG 树中
   释放。

遵循此流程将关闭“注册自动化和证据导出”
`roadmap.md:450` 中提出的间隙，并确保每个紧急规范
决策带有可重现的工件、JSON 差异和透明度日志
条目。

### TTL 和佳能证据助手

生成捆绑包/差异对后，运行 CLI 证据帮助程序来捕获
治理要求的 TTL 摘要和紧急审查截止日期：

```bash
iroha app sorafs gateway evidence \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --out artifacts/ministry/denylist_registry/2026-05-14/denylist_evidence.json \
  --label csam-canon-2026-05
```

该命令对源 JSON 进行哈希处理，验证每个条目，并发出一个紧凑的
摘要包含：

- 每个 `kind` 和每个保单层（最早/最新）的条目总数
  观察到的时间戳。
- `emergency_reviews[]` 列表，枚举每个紧急规范及其
  描述符、有效过期时间、最大允许 TTL 以及计算得出的
  `review_due_by` 截止日期。

将 `denylist_evidence.json` 与打包的捆绑包/差异一起附上，以便审核员可以
无需重新运行 CLI 即可确认 TTL 合规性。已经产生的 CI 职位
捆绑包可以调用助手并发布证据工件（例如通过
调用 `ci/check_sorafs_gateway_denylist.sh --evidence-out <path>`)，确保
每个佳能请求都会有一致的摘要。

### Merkle 注册证据

MINFO-6 中引入的 Merkle 注册表要求运营商发布
根证明和每个条目的证明以及 TTL 摘要。跑步后立即
证据助手，捕获 Merkle 文物：

```bash
iroha app sorafs gateway merkle snapshot \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_snapshot.to

iroha app sorafs gateway merkle proof \
  --denylist artifacts/ministry/denylist_registry/2026-05-14/denylist.json \
  --index 12 \
  --json-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.json \
  --norito-out artifacts/ministry/denylist_registry/2026-05-14/denylist_merkle_proof_entry_12.to
```快照 JSON 记录了 BLAKE3 Merkle 根、叶子计数和每个
描述符/哈希对，以便 GAR 投票可以引用经过哈希处理的确切树。
提供 `--norito-out` 将 `.to` 工件与 JSON 一起存储，让
网关直接通过 Norito 获取注册表项，无需抓取
标准输出。 `merkle proof` 发出方向位和同级哈希值
从零开始的条目索引，可以轻松地为每个条目附加包含证明
GAR 备忘录中引用的紧急规范 — 可选的 Norito 副本保留了证据
准备好在账本上分发。接下来存储 JSON 和 Norito 工件
TTL 摘要和 diff 捆绑包，以便透明发布和治理
锚点引用相同的根。