---
lang: zh-hans
direction: ltr
source: docs/examples/finance/repo_governance_packet_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cd018a94197722adfbb9d54bf02f1c486147078174ba4c81f32e9d93b8c3f6d5
source_last_modified: "2026-01-22T16:26:46.473419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# 回购协议治理包模板（路线图 F1）

准备路线图项所需的工件包时使用此模板
F1（存储库生命周期文档和工具）。目标是向审稿人提供
列出每个输入、哈希值和证据包的单个 Markdown 文件，以便
治理委员会可以重播提案中引用的字节。

> 将模板复制到您自己的证据目录中（例如
> `artifacts/finance/repo/2026-03-15/packet.md`)，替换占位符，并且
> 将其提交/上传到下面引用的散列工件旁边。

## 1. 元数据

|领域|价值|
|--------|--------|
|协议/变更标识符 | `<repo-yyMMdd-XX>` |
|准备时间/日期 | `<desk lead> – 2026-03-15T10:00Z` |
|评论者 | `<dual-control reviewer(s)>` |
|更改类型 | `Initiation / Haircut update / Substitution matrix change / Margin policy` |
|托管人 | `<custodian id(s)>` |
|链接提案/公投 | `<governance ticket id or GAR link>` |
|证据目录| ``artifacts/finance/repo/<slug>/`` |

## 2. 指令有效负载

记录各服务台通过以下方式签署的分阶段 Norito 指令
`iroha app repo ... --output`。每个条目应包含发出的哈希值
文件以及投票后将提交的操作的简短描述
通过。

|行动|文件| SHA-256 |笔记|
|--------|------|---------|--------|
|发起 | `instructions/initiate.json` | `<sha256>` |包含经服务台+交易对手批准的现金/抵押品。 |
|追加保证金通知 | `instructions/margin_call.json` | `<sha256>` |捕获触发呼叫的节奏 + 参与者 ID。 |
|放松 | `instructions/unwind.json` | `<sha256>` |一旦条件满足，反向腿的证明。 |

```bash
# Example hash helper (repeat per instruction file)
sha256sum artifacts/finance/repo/<slug>/instructions/initiate.json \
  | tee artifacts/finance/repo/<slug>/hashes/initiate.sha256
```

## 2.1 托管人确认（仅限三方）

每当存储库使用 `--custodian` 时，请完成此部分。治理包
必须包含每个托管人的签名确认以及哈希值
`docs/source/finance/repo_ops.md` §2.8 中引用的文件。

|托管人 |文件| SHA-256 |笔记|
|------------|------|---------|--------|
| `<ih58...>` | `custodian_ack_<custodian>.md` | `<sha256>` |签署的 SLA 涵盖托管窗口、路由账户和钻探联系人。 |

> 将确认信息存储在其他证据旁边 (`artifacts/finance/repo/<slug>/`)
> 所以 `scripts/repo_evidence_manifest.py` 将文件记录在同一棵树中
> 分阶段说明和配置片段。参见
> `docs/examples/finance/repo_custodian_ack_template.md` 表示可立即灌装
> 与治理证据合约相匹配的模板。

## 3. 配置片段

粘贴将登陆集群的 `[settlement.repo]` TOML 块（包括
`collateral_substitution_matrix`）。将哈希存储在代码片段旁边，以便
审计员可以确认回购预订时处于活动状态的运行时策略
获得批准。

```toml
[settlement.repo]
eligible_collateral = ["bond#wonderland", "note#wonderland"]
default_margin_percent = "0.025"

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["bill#wonderland"]
```

`SHA-256 (config snippet): <sha256>`

### 3.1 批准后配置快照

公投或治理投票完成后，`[settlement.repo]`
更改已推出，从每个对等方捕获 `/v1/configuration` 快照，以便
审计员可以证明批准的政策在整个集群中有效（参见
`docs/source/finance/repo_ops.md` §2.9 证据工作流程）。

```bash
mkdir -p artifacts/finance/repo/<slug>/config/peers
curl -fsSL https://peer01.example/v1/configuration \
  | jq '.' \
  > artifacts/finance/repo/<slug>/config/peers/peer01.json
```

|同行/来源|文件| SHA-256 |区块高度 |笔记|
|----------------|------|---------|----------------|--------|
| `peer01` | `config/peers/peer01.json` | `<sha256>` | `<block-height>` |配置推出后立即捕获的快照。 |
| `peer02` | `config/peers/peer02.json` | `<sha256>` | `<block-height>` |确认 `[settlement.repo]` 与暂存的 TOML 匹配。 |

将摘要与对等 ID 一起记录在 `hashes.txt`（或等效的
摘要），以便审阅者可以跟踪哪些节点吸收了更改。快照
位于 TOML 片段旁边的 `config/peers/` 下，并将被拾取
由 `scripts/repo_evidence_manifest.py` 自动生成。

## 4. 确定性测试工件

附上最新的输出：

- `cargo test -p iroha_core -- repo_deterministic_lifecycle_proof_matches_fixture`
- `cargo test --package integration_tests --test repo`

记录 CI 生成的日志包或 JUnit XML 的文件路径 + 哈希值
系统。

|文物 |文件| SHA-256 |笔记|
|----------|------|---------|--------|
|生命周期证明日志 | `tests/repo_lifecycle.log` | `<sha256>` |使用 `--nocapture` 输出捕获。 |
|集成测试日志| `tests/repo_integration.log` | `<sha256>` |包括替代+边际节奏覆盖。 |

## 5. 生命周期证明快照

每个数据包必须包含从以下位置导出的确定性生命周期快照
`repo_deterministic_lifecycle_proof_matches_fixture`。运行线束
启用导出旋钮，以便审阅者可以区分 JSON 框架并摘要
`crates/iroha_core/tests/fixtures/` 中跟踪的夹具（参见
`docs/source/finance/repo_ops.md` §2.7)。

```bash
REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

或者使用固定的助手重新生成灯具并将它们复制到您的
一步收集证据：

```bash
scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
  --bundle-dir artifacts/finance/repo/<slug>
```

|文物 |文件| SHA-256 |笔记|
|----------|------|---------|--------|
|快照 JSON | `repo_proof_snapshot.json` | `<sha256>` |由证明线束发出的规范生命周期框架。 |
|摘要文件 | `repo_proof_digest.txt` | `<sha256>` |大写十六进制摘要镜像自 `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`；即使未更改也要附加。 |

## 6. 证据清单

生成整个证据目录的清单，以便审核员可以验证
散列而不解压存档。助手反映了所描述的工作流程
在 `docs/source/finance/repo_ops.md` §3.2 中。

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/<slug> \
  --agreement-id <repo-identifier> \
  --output artifacts/finance/repo/<slug>/manifest.json
```

|文物|文件| SHA-256 |笔记|
|----------|------|---------|--------|
|证据清单 | `manifest.json` | `<sha256>` |将校验和包含在治理票/公投注释中。 |

## 7. 遥测和事件快照

导出相关的 `AccountEvent::Repo(*)` 条目和任何仪表板或 CSV
`docs/source/finance/repo_ops.md` 中引用的导出。记录文件 +
这里有哈希值，以便审阅者可以直接跳到证据。

|出口|文件| SHA-256 |笔记|
|--------|------|---------|--------|
|回购事件 JSON | `evidence/repo_events.ndjson` | `<sha256>` |原始 Torii 事件流已过滤到桌面帐户。 |
|遥测 CSV | `evidence/repo_margin_dashboard.csv` | `<sha256>` |使用回购保证金面板从 Grafana 导出。 |

## 8. 批准和签名

- **双控签名者：** `<names + timestamps>`
- **GAR / 分钟摘要：** `<sha256>` 签名的 GAR PDF 或分钟上传。
- **存储位置：** `governance://finance/repo/<slug>/packet/`

## 9. 清单

完成后标记每个项目。

- [ ] 指令有效负载已暂存、散列并附加。
- [ ] 记录配置片段哈希值。
- [ ] 捕获+散列的确定性测试日志。
- [ ] 生命周期快照 + 导出摘要。
- [ ] 生成证据清单并记录哈希值。
- [ ] 事件/遥测导出捕获+散列。
- [ ] 双控制确认已存档。
- [ ] GAR/分钟上传；上面记录了摘要。

与每个数据包一起维护此模板可以保留治理 DAG
确定性，并为审计人员提供回购生命周期的便携式清单
决定。