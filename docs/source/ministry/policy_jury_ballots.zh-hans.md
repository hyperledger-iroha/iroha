---
lang: zh-hans
direction: ltr
source: docs/source/ministry/policy_jury_ballots.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff3faabda5f1c277f545b7edbbc93f3b58dee65cec943cfd464a026b2984a146
source_last_modified: "2025-12-29T18:16:35.979378+00:00"
translation_last_reviewed: 2026-02-07
title: Policy Jury Sortition & Ballots
translator: machine-google-reviewed
---

路线图项目 **MINFO-5 — 政策陪审团投票工具包** 需要便携式格式
用于确定性陪审员选择以及密封提交 → 揭示选票。  的
`iroha_data_model::ministry::jury` 模块现在提供三个 Norito 有效负载，
涵盖整个投票流程：

1. **`PolicyJurySortitionV1`** – 记录抽奖元数据（提案 ID、
   回合 ID、身份证明快照摘要、随机性信标），
   委员会规模、选定的陪审员以及用于自动确定的候补名单
   故障转移。  每个主插槽可能包含 `PolicyJuryFailoverPlan`
   指向候补名单排名，在其宽限后应升级到
   经期流逝。  该结构是有意确定性的，因此审计员
   可以重放抽签并从同一 POP 重新生成清单
   快照+信标。
2. **`PolicyJuryBallotCommitV1`** – 投票前写下的密封承诺
   被揭露。  它存储回合/提案/陪审员标识符、
   Blake2b-256 陪审员 ID + 投票选择 + 随机数元组摘要，捕获
   时间戳和选票模式（`plaintext` 或 `zk-envelope`
   `zk-ballot` 功能已激活）。  `PolicyJuryBallotCommitV1::verify_reveal`
   确保存储的摘要与显示有效负载匹配。
3. **`PolicyJuryBallotRevealV1`** – 包含以下内容的公共揭示对象
   投票选择、提交时使用的随机数以及可选的 ZK 证明 URI。
   Reveals 需要至少 16 字节的随机数，以便治理可以处理
   即使陪审员通过不安全的渠道运作，承诺也具有约束力。

`PolicyJurySortitionV1::validate` 帮助程序强制执行委员会规模调整，
重复检测（评审员不得同时出现在委员会和评审委员会中）
等候名单）、有序的等候名单排名以及有效的故障转移参考。  选票
当提案或回合 ID 时，验证例程会引发 `PolicyJuryBallotError`
漂移，当陪审员试图用不正确的随机数来揭示时，或者当
`zk-envelope`承诺未能在其承诺中提供匹配的证明参考
揭示。

### 与客户整合

- 治理工具应保留抽签清单并将其包含在
  策略数据包，以便观察者可以重新计算 POP 快照摘要并
  确认随机性信标加上候选集会导致相同的结果
  陪审员分配。
- 陪审员客户在之后立即记录 `PolicyJuryBallotCommitV1`
  为他们的投票生成随机数。  导出的承诺字节可以是
  作为 base64 值提交给 Torii 或直接嵌入到 Norito 中
  事件。
- 在揭晓阶段，陪审员发出 `PolicyJuryBallotRevealV1`。  运营商
  之前将有效负载馈送到 `PolicyJuryBallotCommitV1::verify_reveal`
  接受投票，确保信息不被交换或篡改。
- 当启用 `zk-ballot` 功能时，陪审员可以附加确定性
  证明 URI（例如，`sorafs://proofs/pj-2026-02/juror-5`），以便下游
  审计员可以检索引用的零知识见证包
  承诺。所有三个结构都派生出 `Encode`、`Decode` 和 `IntoSchema`，这意味着它们
可用于 ISI 流、CLI 工具、SDK 和治理 REST API。
请参阅 `crates/iroha_data_model/src/ministry/jury.rs` 以了解规范的 Rust
定义和辅助方法。

### CLI 对抽签清单的支持

路线图项目 **MINFO-5** 还呼吁使用可重复的工具，以便治理可以
在每次公投数据包发布之前发送可验证的政策陪审团名册。
工作区现在公开 `cargo xtask ministry-jury sortition` 命令：

```bash
cargo xtask ministry-jury sortition \
  --roster docs/examples/ministry/policy_jury_roster_example.json \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --beacon 22b1e48d47123f5c9e3f0cc0c8e34aa3c5f9c49a2cbb70559d3cb0ddc1a6ef01 \
  --committee-size 3 \
  --waitlist-size 2 \
  --drawn-at 2026-01-15T09:00:00Z \
  --waitlist-ttl-hours 72 \
  --out artifacts/ministry/policy_jury_sortition.json
```

- `--roster` 接受确定性 PoP 名册（JSON 示例：
  `docs/examples/ministry/policy_jury_roster_example.json`）。  每个条目
  声明 `juror_id`、`pop_identity`、重量和可选
  `grace_period_secs`。  不符合条件的条目将被自动过滤。
- `--beacon` 注入治理中捕获的 32 字节随机信标
  分钟。  CLI 将信标直接连接到 ChaCha20 RNG，以便审核员
  可以逐字节重放绘制。
- `--committee-size`、`--waitlist-size` 和 `--waitlist-ttl-hours` 控制
  就座陪审员数量、故障转移缓冲区以及应用的到期时间戳
  到候补名单条目。  当某个插槽存在故障转移等级时，该命令
  记录指向匹配的候补名单排名的 `PolicyJuryFailoverPlan`。
- `--drawn-at` 记录抽签的挂钟时间戳；工具
  将其转换为清单中的 Unix 毫秒。

生成的清单是经过充分验证的 `PolicyJurySortitionV1` 有效负载。
大型部署通常将输出保存在 `artifacts/ministry/` 下，因此
可以与审查小组一起直接捆绑到公投包中
总结。  说明性输出可在
`docs/examples/ministry/policy_jury_sortition_example.json`，以便 SDK 团队可以
运行其 Norito 解码器，而无需在本地重播整个绘制过程。

### 投票提交/显示助手

陪审员客户也需要用于提交 → 揭示流程的确定性工具。
相同的 `cargo xtask ministry-jury` 命令现在公开以下帮助程序：

```bash
cargo xtask ministry-jury ballot commit \
  --proposal AC-2026-042 \
  --round PJ-2026-02 \
  --juror citizen:ada \
  --choice approve \
  --nonce-hex aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899 \
  --committed-at 2026-02-01T13:00:00Z \
  --out artifacts/ministry/policy_jury_commit_ada.json \
  --reveal-out artifacts/ministry/policy_jury_reveal_ada.json

cargo xtask ministry-jury ballot verify \
  --commit artifacts/ministry/policy_jury_commit_ada.json \
  --reveal artifacts/ministry/policy_jury_reveal_ada.json
```

- `ballot commit` 发出 `PolicyJuryBallotCommitV1` JSON 负载。  当
  `--out` 被省略，该命令将承诺打印到标准输出。  如果
  `--reveal-out` 提供的工具还写入匹配的
  `PolicyJuryBallotRevealV1`，重用提供的随机数并应用
  可选 `--revealed-at` 时间戳（默认为 `--committed-at` 或
  当前时间）。
- `--nonce-hex` 接受任何长度≥16 字节的偶数十六进制字符串。  当省略时
  helper 使用 `OsRng` 生成 32 字节随机数，从而可以轻松编写脚本
  无需自定义随机性管道的陪审员工作流程。
- `--choice` 不区分大小写，接受 `approve`、`reject` 或 `abstain`。

`ballot verify` 通过交叉检查承诺/揭示对
`PolicyJuryBallotCommitV1::verify_reveal`，保证轮次id，
提案 ID、陪审员 ID、随机数和投票选择在揭晓之前全部对齐
考入Torii。  验证时助手以非零状态退出
失败，从而可以安全地连接到 CI 或当地陪审员门户。