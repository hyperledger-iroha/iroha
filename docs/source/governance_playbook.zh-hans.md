---
lang: zh-hans
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T14:35:37.551676+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 治理手册

这本手册记录了维持 Sora Network 的日常仪式
治理委员会一致。它汇集了来自权威的参考文献
存储库，以便各个仪式可以保持简洁，而操作员始终
为更广泛的流程提供单一入口点。

## 理事会仪式

- **装置治理** – 请参阅[Sora 议会装置批准](sorafs/signing_ceremony.md)
  对于议会基础设施小组现在的链上审批流程
  查看 SoraFS 分块器更新时如下。
- **计票结果发布** – 请参阅
  [治理投票统计](governance_vote_tally.md) 用于分步 CLI
  工作流程和报告模板。

## 操作手册

- **API 集成** – [治理 API 参考](governance_api.md) 列出了
  理事会服务公开的 REST/gRPC 表面，包括身份验证
  要求和分页规则。
- **遥测仪表板** – Grafana JSON 定义
  `docs/source/grafana_*` 定义了“治理约束”和“调度程序”
  TEU”板。每次发布后将 JSON 导出到 Grafana 以保持一致
  与规范布局。

## 数据可用性监督

### 保留类

批准 DA 清单的议会小组必须参考强制保留
投票前的政策。下表反映了通过强制执行的默认值
`torii.da_ingest.replication_policy` 因此审阅者无需
寻找源TOML。【docs/source/da/replication_policy.md:1】

|治理标签|斑点类|热保持|保冷|所需副本 |存储类|
|----------------|------------|---------------|----------------|--------------------|----------------------------|
| `da.taikai.live` | `taikai_segment` | 24小时 | 14 天 | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6小时| 7 天 | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12 小时 | 180天| 3 | `cold` |
| `da.default` | _所有其他类别_ | 6小时| 30 天 | 3 | `warm` |

基础设施小组应附上来自
`docs/examples/da_manifest_review_template.md` 每张选票因此清单
摘要、保留标签和 Norito 文物在治理中保持关联
记录。

### 签名清单审计跟踪

在选票进入议程之前，议会工作人员必须证明清单
正在审查的字节与议会信封和 SoraFS 文物相符。使用
收集证据的现有工具：1. 从 Torii (`iroha app da get-blob --storage-ticket <hex>`
   或等效的 SDK 帮助程序），因此每个人都对到达的相同字节进行哈希处理
   网关。
2. 使用签名的信封运行清单存根验证程序：
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   这会重新计算 BLAKE3 清单摘要，验证
   `chunk_digest_sha3_256`，并检查嵌入的每个 Ed25519 签名
   `manifest_signatures.json`。参见 `docs/source/sorafs/manifest_pipeline.md`
   有关其他 CLI 选项。
3. 将摘要、`chunk_digest_sha3_256`、配置文件句柄和签名者列表复制到
   审核模板。注意：如果验证者报告“配置文件不匹配”或
   缺少签名，停止投票并要求提供更正的信封。
4. 存储验证器输出（或来自的 CI 工件）
   `ci/check_sorafs_fixtures.sh`) 与 Norito `.to` 有效负载一起，以便审核员
   无需访问内部网关即可重播证据。

由此产生的审计包应该让议会重新创建每个哈希值和签名
即使在清单从热存储中转出后也要进行检查。

### 审查清单

1. 取出议会批准的舱单信封（参见
   `docs/source/sorafs/signing_ceremony.md`）并记录 BLAKE3 摘要。
2. 验证清单的 `RetentionPolicy` 块与表中的标记匹配
   上面； Torii 将拒绝不匹配，但理事会必须捕获
   审计员的证据。【docs/source/da/replication_policy.md:32】
3. 确认提交的 Norito 有效负载引用相同的保留标签
   以及出现在入场券中的 blob 类。
4. 附上策略检查证明（CLI 输出，`torii.da_ingest.replication_policy`
   dump 或 CI artefact）到审核数据包，以便 SRE 可以重播决策。
5. 当提案取决于时，记录计划的补贴水龙头或租金调整
   `docs/source/sorafs_reserve_rent_plan.md`。

### 升级矩阵

|请求类型 |拥有面板|附上证据 |截止日期和遥测|参考文献 |
|--------------|--------------|--------------------|------------------------|------------|
|补贴/租金调整|基础设施+财政|填充 DA 数据包、`reserve_rentd` 的租金增量、更新的储备金预测 CSV、理事会投票记录 |在提交财务更新之前注意租金影响；包括滚动 30 天缓冲遥测，以便财务部门可以在下一个结算窗口内进行调节 | `docs/source/sorafs_reserve_rent_plan.md`、`docs/examples/da_manifest_review_template.md` |
|审核删除/合规行动 |适度+合规|合规票证 (`ComplianceUpdateV1`)、证明令牌、签名清单摘要、上诉状态 |遵循网关合规性 SLA（24 小时内确认，完全删除≤72 小时）。附上显示该操作的 `TransparencyReportV1` 摘录。 | `docs/source/sorafs_gateway_compliance_plan.md`、`docs/source/sorafs_moderation_panel_plan.md` |
|紧急冻结/回滚|议会调解小组|事先批准包、新冻结令、回滚清单摘要、事件日志 |立即发布冻结通知，并在下一个治理时段内安排回滚公投；包括缓冲区饱和+ DA 复制遥测来证明紧急情况的合理性。 | `docs/source/sorafs/signing_ceremony.md`、`docs/source/sorafs_moderation_panel_plan.md` |在对入场券进行分类时使用该表，以便每个小组都收到准确的
执行任务所需的文物。

### 报告可交付成果

每个 DA-10 决策都必须附带以下工件（将它们附加到
投票中引用的治理 DAG 条目）：

- 完整的 Markdown 数据包来自
  `docs/examples/da_manifest_review_template.md`（现在包括签名和
  升级部分）。
- 已签名的 Norito 清单 (`.to`) 以及 `manifest_signatures.json` 信封
  或证明提取摘要的 CI 验证程序日志。
- 由该操作触发的任何透明度更新：
  - `TransparencyReportV1` 删除或合规驱动冻结的增量。
  - 租金/储备账本增量或 `ReserveSummaryV1` 补贴快照。
- 审查期间收集的遥测快照的链接（复制深度、
  缓冲余量、审核积压），以便观察者可以交叉检查条件
  事后。

## 审核和升级

合规后将关闭网关、收回补贴或冻结 DA
`docs/source/sorafs_gateway_compliance_plan.md` 中描述的管道和
`docs/source/sorafs_moderation_panel_plan.md` 中的上诉工具。面板应：

1. 记录原始合规票证（`ComplianceUpdateV1` 或
   `ModerationAppealV1`）并附上相关的证明令牌。【docs/source/sorafs_gateway_compliance_plan.md:20】
2. 确认请求是否调用审核上诉路径（公民小组
   投票）或议会紧急冻结；两个流程都必须引用清单
   新模板中捕获的摘要和保留标签。【docs/source/sorafs_moderation_panel_plan.md:1】
3. 列举升级期限（上诉提交/披露窗口、紧急情况
   冻结期限）并说明哪个理事会或小组拥有后续行动。
4. 捕获用于的遥测快照（缓冲区余量、审核积压）
   证明该行动的合理性，以便下游审核可以将决策与实际情况相匹配
   状态。

合规和审核小组必须同步其每周透明度报告
与路由器运营商结算，因此下架和补贴影响相同
清单集。

## 报告模板

所有 DA-10 评论现在都需要签名的 Markdown 数据包。复制
`docs/examples/da_manifest_review_template.md`，填充清单元数据，
保留验证表和小组投票摘要，然后固定已完成的内容
文档（加上引用的 Norito/JSON 工件）到治理 DAG 条目。
专家组应在治理会议纪要中链接该数据包，以便将来删除或
补贴续订可以引用原始清单摘要，而无需重新运行
整个仪式。

## 事件和撤销工作流程

紧急行动现在发生在链上。当需要发布夹具时
回滚，提交治理票并提出议会恢复提案
指向先前批准的清单摘要。基础设施小组
处理投票，一旦最终确定，Nexus 运行时就会发布回滚
下游客户端消费的事件。不需要本地 JSON 工件。

## 保持剧本最新- 每当新的面向治理的操作手册登陆时更新此文件
  存储库。
- 在这里交叉链接新的仪式，以便理事会索引仍然可以被发现。
- 如果引用的文档发生移动（例如，新的 SDK 路径），请更新链接
  作为同一拉取请求的一部分，以避免过时的指针。