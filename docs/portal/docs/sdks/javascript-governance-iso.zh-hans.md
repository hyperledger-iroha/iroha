---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 885f89984341598dfb4423efc8bcbf139bbf749e37d398e6c23f34399414bfe3
source_last_modified: "2026-01-22T16:26:46.511976+00:00"
translation_last_reviewed: 2026-02-07
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
slug: /sdks/javascript/governance-iso-examples
translator: machine-google-reviewed
---

本现场指南通过展示治理和
ISO 20022 桥接流程为 `@iroha/iroha-js`。片段重复使用相同的内容
`ToriiClient` 附带的运行时助手，因此您可以将它们直接复制到
CLI 工具、CI 工具或长期运行的服务。

其他资源：

- `javascript/iroha_js/recipes/governance.mjs` — 可运行的端到端脚本
  提案、投票和理事会轮换。
- `javascript/iroha_js/recipes/iso_bridge.mjs` — 用于提交的 CLI 帮助程序
  pacs.008/pacs.009 有效负载和轮询确定性状态。
- `docs/source/finance/settlement_iso_mapping.md` — 规范 ISO 字段映射。

## 运行捆绑的食谱

这些示例取决于 `javascript/iroha_js/recipes/` 中的脚本。运行
事先 `npm install && npm run build:native` 所以生成的绑定是
可用。

### 治理助手演练

调用前配置以下环境变量
`recipes/governance.mjs`：

- `TORII_URL` — Torii 端点。
- `AUTHORITY` / `PRIVATE_KEY_HEX` — 签名者帐户和密钥（十六进制）。将钥匙存放在
  安全的秘密商店。
- `CHAIN_ID` — 可选网络标识符。
- `GOV_SUBMIT=1` — 将生成的交易推送到 Torii。
- `GOV_FETCH=1` — 提交后获取提案/锁定。
- `GOV_PROPOSAL_ID`、`GOV_REFERENDUM_ID`、`GOV_LOCKS_ID` — 使用的可选查找
  当 `GOV_FETCH=1` 时。

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=soraカタカナ... \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

每个步骤都会记录哈希值，并且在以下情况下会显示 Torii 响应：
`GOV_SUBMIT=1`，因此 CI 作业可能会因提交错误而快速失败。

### ISO 桥接助手

`recipes/iso_bridge.mjs` 提交 pacs.008 或 pacs.009 消息并进行轮询
ISO 桥接，直到状态稳定。配置它：

- `TORII_URL` — Torii 端点公开 ISO 桥 API。
- `ISO_MESSAGE_KIND` — `pacs.008`（默认）或 `pacs.009`。助手使用
  匹配样本构建器（`buildSamplePacs008Message` / `buildSamplePacs009Message`）
  当您不提供自己的 XML 时。
- `ISO_MESSAGE_SUFFIX` — 附加到示例有效负载 ID 的可选后缀
  保持重复排练的唯一性（默认为十六进制的当前纪元秒）。
- `ISO_CONTENT_TYPE` — 覆盖 `Content-Type` 提交标头
  （例如 `application/pacs009+xml`）；当您只轮询时被忽略
  现有消息 ID。
- `ISO_MESSAGE_ID` — 完全跳过提交，仅轮询提供的
  通过 `waitForIsoMessageStatus` 进行识别。
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — 调整等待策略
  嘈杂或缓慢的网桥部署。
- `ISO_RESOLVE_ON_ACCEPTED=1` — 一旦 Torii 返回 `Accepted` 就退出，
  即使交易哈希仍然处于待处理状态（在桥维护期间很方便）
  当分类帐提交被延迟时）。

```bash
# Submit a pacs.009 message and wait for completion.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_KIND=pacs.009 \
ISO_POLL_ATTEMPTS=20 \
ISO_POLL_INTERVAL_MS=1500 \
node javascript/iroha_js/recipes/iso_bridge.mjs

# Poll an existing message id without re-submitting XML.
TORII_URL=https://torii.testnet.sora \
ISO_MESSAGE_ID=iso-demo-1 \
node javascript/iroha_js/recipes/iso_bridge.mjs
```

如果 Torii 从未报告终端，则两个脚本都会退出并显示状态代码 `1`
过渡，使它们适合 CI 门工作。

### ISO 别名助手

`recipes/iso_alias.mjs` 以 ISO 别名端点为目标，以便排练可以覆盖
盲元素散列和别名查找，无需编写定制工具。它
调用 `ToriiClient.evaluateAliasVoprf` 加 `resolveAlias` / `resolveAliasByIndex`
并打印后端、摘要、帐户绑定、源和确定性索引
由 Torii 返回。

环境变量：

- `TORII_URL` — Torii 端点公开别名助手。
- `ISO_VOPRF_INPUT` — 十六进制编码的盲元素（默认为 `deadbeef`）。
- `ISO_SKIP_VOPRF=1` — 仅测试查找时跳过 VOPRF 调用。
- `ISO_ALIAS_LABEL` — 要解析的文字别名（例如 IBAN 样式字符串）。
- `ISO_ALIAS_INDEX` — 十进制或传递给 `resolveAliasByIndex` 的 `0x` 前缀索引。
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — 用于安全 Torii 部署的可选标头。

```bash
# Evaluate a blinded element and resolve an alias literal + deterministic index.
TORII_URL=https://torii.testnet.sora \
ISO_VOPRF_INPUT=deadbeefcafebabe \
ISO_ALIAS_LABEL="GB82 WEST 1234 5698 7654 32" \
ISO_ALIAS_INDEX=0 \
node javascript/iroha_js/recipes/iso_alias.mjs

# Only perform literal resolution.
TORII_URL=https://torii.testnet.sora \
ISO_SKIP_VOPRF=1 \
ISO_ALIAS_LABEL="iso:demo:alpha" \
node javascript/iroha_js/recipes/iso_alias.mjs
```

帮助器反映了 Torii 的行为：当别名丢失时，它会显示 404
并将运行时禁用的错误视为软跳过，以便 CI 流程可以容忍桥接
维护窗口。

## 治理工作流程

### 检查合同实例和提案

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const instances = await torii.listGovernanceInstances("apps", {
  contains: "ledger",
  hashPrefix: "deadbeef",
  order: "hash_desc",
  limit: 5,
});
for (const entry of instances.instances) {
  console.log(`${entry.contract_id} :: ${entry.code_hash_hex}`);
}

const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### 提交提案和选票

当您需要取消或有时限的治理提交时，请使用 `AbortController` - SDK
如下所示，每个 POST 帮助程序接受一个可选的 `{ signal }` 对象。

```ts
const authority = "soraカタカナ...";
const privateKey = Buffer.alloc(32, 0xaa);

// All governance writes accept optional `{ signal }` options for cancellation.
const writeController = new AbortController();
const deployDraft = await torii.governanceProposeDeployContract({
  namespace: "apps",
  contractId: "calc.v1",
  codeHash: "hash:7B38...#ABCD",
  abiHash: Buffer.alloc(32, 0xbb),
  abiVersion: "1",
  window: { lower: 12_345, upper: 12_500 },
  mode: "Plain",
}, { signal: writeController.signal });
console.log("draft instructions", deployDraft.tx_instructions.length);

const ballot = await torii.governanceSubmitPlainBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  referendumId: "ref-plain",
  owner: authority,
  amount: "5000",
  durationBlocks: 7_200,
  direction: "Aye",
}, { signal: writeController.signal });
if (!ballot.accepted) {
  console.warn("ballot rejected", ballot.reason);
}

const zkOwner = "soraカタカナ..."; // canonical Katakana i105 account id for ZK public inputs
await torii.governanceSubmitZkBallot({
  authority,
  chainId: "00000000-0000-0000-0000-000000000000",
  electionId: "ref-zk",
  proof: Buffer.alloc(96, 0xcd),
  public: {
    owner: zkOwner,
    amount: "5000",
    duration_blocks: 7_200,
    direction: "Aye",
  },
}, { signal: writeController.signal });
```

### 理事会 VRF 和颁布

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "soraカタカナ...",
      variant: "Normal",
      pk: validatorPk,
      proof: validatorProof,
    },
  ],
}, { signal: writeController.signal });
await torii.governancePersistCouncil({
  committeeSize: derived.members.length,
  candidates: derived.members.map((member) => ({
    accountId: member.account_id,
    variant: "Normal",
    pk: validatorPk,
    proof: validatorProof,
  })),
  authority,
  privateKey,
}, { signal: writeController.signal });

const finalizeDraft = await torii.governanceFinalizeReferendumTyped({
  referendumId: "ref-mainnet-001",
  proposalId: "0123abcd...beef",
}, { signal: writeController.signal });
console.log("finalize tx count", finalizeDraft.tx_instructions.length);

const enactDraft = await torii.governanceEnactProposalTyped({
  proposalId: "abcd0123...cafe",
  window: { lower: 10, upper: 25 },
}, { signal: writeController.signal });
console.log("enact tx count", enactDraft.tx_instructions.length);
```

## ISO 20022 桥梁配方

### 构建 pacs.008 / pacs.009 有效负载

```ts
import { buildPacs008Message } from "@iroha/iroha-js";

const settlement = buildPacs008Message({
  messageId: "iso-demo-1",
  instructionId: "instr-1",
  settlementDate: "2026-02-10",
  amount: { currency: "EUR", value: "25.00" },
  instigatingAgent: { bic: "DEUTDEFF", lei: "529900ODI3047E2LIV03" },
  instructedAgent: { bic: "COBADEFF" },
  debtorAccount: { iban: "DE89370400440532013000" },
  creditorAccount: { otherId: "soraカタカナ..." },
  purposeCode: "SECU",
  supplementaryData: { account_id: "soraカタカナ...", leg: "delivery" },
});
```

所有标识符（BIC、LEI、IBAN、ISO 金额）在 XML 之前均经过验证
生成的。将 `buildPacs008Message` 交换为 `buildPacs009Message` 以发出 PvP
资金有效负载。

### 提交并轮询 ISO 消息

```ts
import { ToriiClient } from "@iroha/iroha-js";

const torii = new ToriiClient(process.env.TORII_URL ?? "https://torii.nexus.example");

const status = await torii.submitIsoPacs008AndWait(settlement, {
  wait: {
    maxAttempts: Number(process.env.ISO_POLL_ATTEMPTS ?? 20),
    pollIntervalMs: Number(process.env.ISO_POLL_INTERVAL_MS ?? 3_000),
    resolveOnAccepted: process.env.ISO_RESOLVE_ON_ACCEPTED === "1",
    onPoll: ({ attempt, status: snapshot }) => {
      console.log(`[attempt ${attempt}] status=${snapshot?.status ?? "pending"}`);
    },
  },
});
console.log(status.message_id, status.status, status.transaction_hash);

await torii.waitForIsoMessageStatus(process.env.ISO_MESSAGE_ID!, {
  maxAttempts: 10,
  pollIntervalMs: 2_000,
});

// Build XML on the fly from structured fields (skips the sample payloads).
await torii.submitIsoMessage(
  {
    instructionId: "pvpfund-1",
    amount: { currency: "USD", value: "1250.50" },
    instigatingAgent: { bic: "BOFAUS3N" },
    instructedAgent: { bic: "DEUTDEFF" },
  },
  {
    kind: "pacs.009",
    wait: { maxAttempts: 5, pollIntervalMs: 1_500 },
  },
);
```

`resolveOnAccepted` 和 `resolveOnAcceptedWithoutTransaction` 均有效；使用任一标志
在编排轮询时将 `Accepted` 状态（没有事务哈希）视为终端。

如果桥从未报告过，助手会抛出 `IsoMessageTimeoutError`
终端状态。使用较低级别的 `submitIsoPacs008` / `submitIsoPacs009`
当您需要编排自定义轮询逻辑时调用； `getIsoMessageStatus`
公开单次查找。

### 相关表面

- `torii.getSorafsPorWeeklyReport("2026-W05")` 获取 ISO 周 PoR 包
  在路线图中引用，并且可以重用警报的等待助手。
- `resolveAlias` / `resolveAliasByIndex` 公开 ISO 桥别名绑定，以便
  对账工具可以在付款前证明账户所有权。