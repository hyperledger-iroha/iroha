---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: "SoraFS Migration Roadmap"
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 改编自 [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md)。

# SoraFS 迁移路线图 (SF-1)

本文档实施了中捕获的迁移指南
`docs/source/sorafs_architecture_rfc.md`。它将 SF-1 可交付成果扩展为
执行就绪的里程碑、门控标准和所有者清单，以便存储、
工件托管到 SoraFS 支持的出版物。

路线图是有意确定性的：每个里程碑都指定了所需的内容
工件、命令调用和证明步骤，以便下游管道
产生相同的输出，治理保留可审计的跟踪。

## 里程碑概述

|里程碑|窗口|主要目标|必须发货 |业主 |
|------------|--------|----------------|------------|--------|
| **M1 – 确定性执行** |第 7-12 周 |当管道采用期望标志时，强制执行签名的装置和阶段别名证明。 |每晚固定装置验证、理事会签署的清单、别名注册表暂存条目。 |存储、治理、SDK |

里程碑状态在 `docs/source/sorafs/migration_ledger.md` 中跟踪。全部
对此路线图的更改必须更新分类帐以保持治理和发布
工程同步。

## 工作流

### 2. 确定性固定采用

|步骤|里程碑|描述 |所有者 |输出|
|------|------------|-------------|---------|--------|
|固定排练| M0 |每周进行一次演练，将本地块摘要与 `fixtures/sorafs_chunker` 进行比较。在 `docs/source/sorafs/reports/` 下发布报告。 |存储提供商| `determinism-<date>.md` 带有通过/失败矩阵。 |
|强制签名 | M1 |如果签名或清单发生偏差，`ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` 会失败。开发优先权需要 PR 附带的治理豁免。 |工具工作组 | CI 日志、豁免票证链接（如果适用）。 |
|期望旗帜 | M1 |管道调用 `sorafs_manifest_stub` 并明确期望引脚输出： |文档 CI |更新了引用期望标志的脚本（请参阅下面的命令块）。 |
|注册表优先固定| M2| `sorafs pin propose` 和 `sorafs pin approve` 包装清单提交； CLI 默认为 `--require-registry`。 |治理行动|注册表 CLI 审核日志、失败提案的遥测。 |
|可观测性平价 | M3 |当块库存与注册表清单不一致时，Prometheus/Grafana 仪表板会发出警报；警报连接到待命的操作人员。 |可观察性|仪表板链接、警报规则 ID、GameDay 结果。 |

#### 规范发布命令

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

将摘要、大小和 CID 值替换为记录在中的预期引用
工件的迁移分类帐条目。

### 3. 别名转换和通信

|步骤|里程碑|描述 |所有者 |输出|
|------|------------|-------------|---------|--------|
|暂存中的别名证明 | M1 |在 Pin 注册表暂存环境中注册别名声明，并将 Merkle 证明附加到清单 (`--alias`)。 |治理，文档 |证明包存储在带有别名的清单 + 账本注释旁边。 |
|证据执行 | M2|网关拒绝没有新 `Sora-Proof` 标头的清单； CI 获得 `sorafs alias verify` 步来获取证明。 |网络|网关配置补丁+ CI 输出捕获验证成功。 |

### 4.沟通与审计

- **账本规则：**每次状态变化（夹具漂移、注册表提交、
  别名激活）必须附加注明日期的注释
  `docs/source/sorafs/migration_ledger.md`。
- **治理会议纪要：** 理事会会议批准 PIN 注册更改或
  别名策略必须引用此路线图和分类账。
- **外部通讯：** DevRel 在每个里程碑发布状态更新（博客 +
  变更日志摘录）强调确定性保证和别名时间表。

## 依赖性和风险

|依赖|影响 |缓解措施 |
|------------|--------|------------|
| Pin 注册合同可用性 |阻止 M2 引脚优先推出。 |在 M2 之前进行阶段合约并进行重播测试；保持包络回退直至无回归。 |
|理事会签名密钥 |舱单信封和登记处批准所必需的。 |签字仪式记录在`docs/source/sorafs/signing_ceremony.md`中；旋转带有重叠和分类帐注释的密钥。 |
| SDK 发布节奏 |客户必须在 M3 之前遵守别名证明。 |将 SDK 发布窗口与里程碑门保持一致；将迁移清单添加到发布模板。 |

剩余风险和缓解措施反映在 `docs/source/sorafs_architecture_rfc.md` 中
并在调整时应相互参考。

## 退出标准清单

|里程碑|标准|
|------------|----------|
| M1 | - 每晚固定工作连续七天呈绿色。 <br /> - 在 CI 中验证暂存别名证明。 <br /> - 治理批准预期标志政策。 |

## 变革管理

1. 通过 PR 更新此文件提出调整**和**
   `docs/source/sorafs/migration_ledger.md`。
2. PR 描述中支持治理会议纪要和 CI 证据的链接。
3. 合并时，通知存储 + DevRel 邮件列表以及摘要和预期
   操作员的动作。

遵循此过程可确保 SoraFS 部署保持确定性，
参与 Nexus 发布的团队之间可审核且透明。