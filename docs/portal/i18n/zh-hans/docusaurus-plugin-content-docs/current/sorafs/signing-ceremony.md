---
id: signing-ceremony
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Signing Ceremony Replacement
description: How the Sora Parliament approves and distributes SoraFS chunker fixtures (SF-1b).
sidebar_label: Signing Ceremony
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> 路线图：**SF-1b — Sora 议会固定装置批准。**

用于 SoraFS chunker 装置的手动签名仪式已停用。全部
批准现在通过 **Sora 议会**，这是一个基于抽签的 DAO，
管辖 Nexus。议会成员通过 XOR 债券获得公民身份，轮换
小组，并进行链上投票以批准、拒绝或回滚固定装置
发布。本指南解释了流程和开发人员工具。

## 议会概况

- **公民身份** — 运营商绑定所需的 XOR 以注册为公民，并且
  获得抽签资格。
- **小组** — 职责划分给轮换小组（基础设施、
  适度、财政部……）。基础设施面板拥有 SoraFS 夹具
  批准。
- **排序和轮换** — 面板座位按照指定的节奏重新绘制
  议会宪法因此没有任何一个团体垄断批准。

## 夹具审批流程

1. **提交提案**
   - 工具工作组上传候选 `manifest_blake3.json` 捆绑包以及
     通过 `sorafs.fixtureProposal` 与链上注册表进行固定差异。
   - 该提案记录了 BLAKE3 摘要、语义版本和变更说明。
2. **审核与投票**
   - 基础设施小组通过议会任务接收任务
     队列。
   - 小组成员检查 CI 制品、运行奇偶校验测试和铸件加权
     链上投票。
3. **最终确定**
   - 一旦达到法定人数，运行时就会发出一个批准事件，其中包括
     规范清单摘要和 Merkle 对夹具有效负载的承诺。
   - 该事件被镜像到 SoraFS 注册表中，以便客户端可以获取
     最新议会批准的清单。
4. **分配**
   - CLI 助手 (`cargo xtask sorafs-fetch-fixture`) 提取已批准的清单
     来自 Nexus RPC。存储库的 JSON/TS/Go 常量通过以下方式保持同步
     重新运行 `export_vectors` 并根据链上验证摘要
     记录。

## 开发人员工作流程

- 重新生成装置：

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- 使用议会获取助手下载批准的信封，验证
  签名，并刷新本地赛程。点 `--signatures` 处
  议会出版的信封；帮助者解析随附的清单，
  重新计算 BLAKE3 摘要，并强制执行规范
  `sorafs.sf1@1.0.0` 配置文件。

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

如果清单位于不同的 URL，则传递 `--manifest`。未签名的信封
除非为本地烟雾运行设置 `--allow-unsigned`，否则将被拒绝。

- 通过暂存网关验证清单时，目标为 Torii 而不是
  本地有效负载：

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- 本地 CI 不再需要 `signer.json` 名册。
  `ci/check_sorafs_fixtures.sh` 将回购状态与最新的进行比较
  链上承诺，当它们出现分歧时就会失败。

## 治理说明

- 议会宪法规定法定人数、轮换和升级——否
  需要板条箱级别的配置。
- 紧急回滚通过议会仲裁小组处理。的
  基础设施小组提交了一份参考先前清单的恢复提案
  摘要，一旦获得批准就会取代版本。
- 历史批准仍可在 SoraFS 法医注册表中获取
  重播。

## 常见问题解答

- **`signer.json`去哪儿了？**  
  它被删除了。所有签名者归属都存在于链上； `manifest_signatures.json`
  存储库中只有一个必须与最新版本相匹配的开发人员固定装置
  批准事件。

- **我们还需要本地 Ed25519 签名吗？**  
  不会。议会的批准作为链上文物存储。存在当地固定装置
  为了可重复性，但根据议会摘要进行了验证。

- **团队如何监控审批？**  
  订阅 `ParliamentFixtureApproved` 事件或通过以下方式查询注册表
  Nexus 用于检索当前清单摘要和小组点名的 RPC。