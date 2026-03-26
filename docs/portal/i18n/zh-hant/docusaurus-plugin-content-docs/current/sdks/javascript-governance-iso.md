---
slug: /sdks/javascript/governance-iso-examples
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

本現場指南通過展示治理和
ISO 20022 橋接流程為 `@iroha/iroha-js`。片段重複使用相同的內容
`ToriiClient` 附帶的運行時助手，因此您可以將它們直接複製到
CLI 工具、CI 工具或長期運行的服務。

其他資源：

- `javascript/iroha_js/recipes/governance.mjs` — 可運行的端到端腳本
  提案、投票和理事會輪換。
- `javascript/iroha_js/recipes/iso_bridge.mjs` — 用於提交的 CLI 幫助程序
  pacs.008/pacs.009 有效負載和輪詢確定性狀態。
- `docs/source/finance/settlement_iso_mapping.md` — 規範 ISO 字段映射。

## 運行捆綁的食譜

這些示例取決於 `javascript/iroha_js/recipes/` 中的腳本。運行
事先 `npm install && npm run build:native` 所以生成的綁定是
可用。

### 治理助手演練

調用前配置以下環境變量
`recipes/governance.mjs`：

- `TORII_URL` — Torii 端點。
- `AUTHORITY` / `PRIVATE_KEY_HEX` — 簽名者帳戶和密鑰（十六進制）。將鑰匙存放在
  安全的秘密商店。
- `CHAIN_ID` — 可選網絡標識符。
- `GOV_SUBMIT=1` — 將生成的交易推送到 Torii。
- `GOV_FETCH=1` — 提交後獲取提案/鎖定。
- `GOV_PROPOSAL_ID`、`GOV_REFERENDUM_ID`、`GOV_LOCKS_ID` — 使用的可選查找
  當 `GOV_FETCH=1` 時。

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=i105... \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

每個步驟都會記錄哈希值，並且在以下情況下會顯示 Torii 響應：
`GOV_SUBMIT=1`，因此 CI 作業可能會因提交錯誤而快速失敗。

### ISO 橋接助手

`recipes/iso_bridge.mjs` 提交 pacs.008 或 pacs.009 消息並進行輪詢
ISO 橋接，直到狀態穩定。配置它：

- `TORII_URL` — Torii 端點公開 ISO 橋 API。
- `ISO_MESSAGE_KIND` — `pacs.008`（默認）或 `pacs.009`。助手使用
  匹配樣本構建器（`buildSamplePacs008Message` / `buildSamplePacs009Message`）
  當您不提供自己的 XML 時。
- `ISO_MESSAGE_SUFFIX` — 附加到示例有效負載 ID 的可選後綴
  保持重複排練的唯一性（默認為十六進制的當前紀元秒）。
- `ISO_CONTENT_TYPE` — 覆蓋提交的 `Content-Type` 標頭
  （例如 `application/pacs009+xml`）；當您只輪詢時被忽略
  現有消息 ID。
- `ISO_MESSAGE_ID` — 完全跳過提交，僅輪詢提供的
  通過 `waitForIsoMessageStatus` 進行識別。
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — 調整等待策略
  嘈雜或緩慢的網橋部署。
- `ISO_RESOLVE_ON_ACCEPTED=1` — 一旦 Torii 返回 `Accepted` 就退出，
  即使交易哈希仍然處於待處理狀態（在橋維護期間很方便）
  當分類帳提交被延遲時）。

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

如果 Torii 從未報告終端，則兩個腳本都會退出並顯示狀態代碼 `1`
過渡，使它們適合 CI 門工作。

### ISO 別名助手

`recipes/iso_alias.mjs` 以 ISO 別名端點為目標，以便排練可以覆蓋
盲元素散列和別名查找，無需編寫定制工具。它
調用 `ToriiClient.evaluateAliasVoprf` 加 `resolveAlias` / `resolveAliasByIndex`
並打印後端、摘要、帳戶綁定、源和確定性索引
由 Torii 返回。

環境變量：

- `TORII_URL` — Torii 端點公開別名助手。
- `ISO_VOPRF_INPUT` — 十六進制編碼的盲元素（默認為 `deadbeef`）。
- `ISO_SKIP_VOPRF=1` — 僅測試查找時跳過 VOPRF 調用。
- `ISO_ALIAS_LABEL` — 要解析的文字別名（例如 IBAN 樣式字符串）。
- `ISO_ALIAS_INDEX` — 十進製或傳遞給 `resolveAliasByIndex` 的 `0x` 前綴索引。
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — 用於安全 Torii 部署的可選標頭。

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

幫助器反映了 Torii 的行為：當別名丟失時，它會顯示 404
並將運行時禁用的錯誤視為軟跳過，以便 CI 流程可以容忍橋接
維護窗口。

## 治理工作流程

### 檢查合同實例和提案

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

### 提交提案和選票

當您需要取消或有時限的治理提交時，請使用 `AbortController` - SDK
如下所示，每個 POST 幫助程序接受一個可選的 `{ signal }` 對象。

```ts
const authority = "i105...";
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

const zkOwner = "i105..."; // canonical i105 account id for ZK public inputs
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

### 理事會 VRF 和頒布

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "i105...",
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

## ISO 20022 橋樑配方

### 構建 pacs.008 / pacs.009 有效負載

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
  creditorAccount: { otherId: "i105..." },
  purposeCode: "SECU",
  supplementaryData: { account_id: "i105...", leg: "delivery" },
});
```

所有標識符（BIC、LEI、IBAN、ISO 金額）在 XML 之前均經過驗證
生成的。將 `buildPacs008Message` 交換為 `buildPacs009Message` 以發出 PvP
資金有效負載。

### 提交並輪詢 ISO 消息

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

`resolveOnAccepted` 和 `resolveOnAcceptedWithoutTransaction` 均有效；使用任一標誌
在編排輪詢時將 `Accepted` 狀態（沒有事務哈希）視為終端。

如果橋從未報告過，助手會拋出 `IsoMessageTimeoutError`
終端狀態。使用較低級別的 `submitIsoPacs008` / `submitIsoPacs009`
當您需要編排自定義輪詢邏輯時調用； `getIsoMessageStatus`
公開單次查找。

### 相關表面

- `torii.getSorafsPorWeeklyReport("2026-W05")` 獲取 ISO 週 PoR 包
  在路線圖中引用，並且可以重用警報的等待助手。
- `resolveAlias` / `resolveAliasByIndex` 公開 ISO 橋別名綁定，以便
  對賬工具可以在付款前證明賬戶所有權。