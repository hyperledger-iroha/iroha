---
lang: zh-hant
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-governance-iso.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4eb134e064949015cee74d2913a10a82f8be74e919cb3c6b87795a326c5cfd91
source_last_modified: "2026-01-22T16:26:46.512893+00:00"
translation_last_reviewed: 2026-02-07
title: JavaScript governance & ISO recipe
description: Run the governance helpers and ISO 20022 bridge flows shipped with @iroha/iroha-js, including runnable CLI samples.
slug: /sdks/recipes/javascript-governance-iso
translator: machine-google-reviewed
---

從“@site/src/components/SampleDownload”導入 SampleDownload；

本配方捆綁了 JS5 路線圖中提到的兩個高級工作流程
項目：端到端治理助手（提案、選票、理事會快照）
以及 ISO 20022 橋樑演練 (pacs.008/pacs.009)。每個樣本運行
針對已發布的 `@iroha/iroha-js` 包並鏡像其中的片段
`docs/source/sdk/js/governance_iso_examples.md`。

## 治理助手示例

<樣本下載
  href="/sdk-recipes/javascript/governance.mjs"
  文件名=“治理.mjs”
  描述=“下載本節中引用的可運行的治理助手。”
/>

### 先決條件

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
export AUTHORITY="<i105-account-id>"
export PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)"
export CHAIN_ID="00000000-0000-0000-0000-000000000000"
# optional lookups for GOV_FETCH
export GOV_PROPOSAL_ID="calc.v1"
export GOV_REFERENDUM_ID="calc-referendum"
export GOV_LOCKS_ID="calc-locks"
```

設置 `GOV_SUBMIT=1` 將簽名交易提交到 Torii 並
`GOV_FETCH=1` 提交後檢查最終的治理狀態。

### 示例腳本

```ts title="governance.mjs"
#!/usr/bin/env node

import { Buffer } from "node:buffer";
import {
  ToriiClient,
  buildProposeDeployContractTransaction,
  buildCastPlainBallotTransaction,
  buildEnactReferendumTransaction,
  buildFinalizeReferendumTransaction,
  buildPersistCouncilForEpochTransaction,
  hashSignedTransaction,
} from "@iroha/iroha-js";

const TORII_URL = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const CHAIN_ID = process.env.CHAIN_ID ?? "00000000-0000-0000-0000-000000000000";
const AUTHORITY = process.env.AUTHORITY ?? "<i105-account-id>";
const PRIVATE_KEY = process.env.PRIVATE_KEY_HEX
  ? Buffer.from(process.env.PRIVATE_KEY_HEX, "hex")
  : Buffer.alloc(32, 0x11);
const SHOULD_SUBMIT = process.env.GOV_SUBMIT === "1";
const SHOULD_FETCH = process.env.GOV_FETCH === "1";
const GOV_PROPOSAL_ID = process.env.GOV_PROPOSAL_ID;
const GOV_REFERENDUM_ID = process.env.GOV_REFERENDUM_ID;
const GOV_LOCKS_ID = process.env.GOV_LOCKS_ID ?? GOV_REFERENDUM_ID;

const SAMPLE_NAMESPACE = "apps";
const SAMPLE_CONTRACT_ID = "calc.v1";
const SAMPLE_REFERENDUM_ID = "calc-referendum";
const SAMPLE_REFERENDUM_HASH = Buffer.alloc(32, 0xaa);
const SAMPLE_PROPOSAL_HASH = Buffer.alloc(32, 0xbb);

function logTransaction(label, tx) {
  const hashHex = tx.hash.toString("hex");
  console.log(`\n[${label}]`);
  console.log("  hash:", hashHex);
  try {
    const recomputed = hashSignedTransaction(tx.signedTransaction);
    console.log("  matches recomputed hash:", recomputed === hashHex);
  } catch (error) {
    console.warn("  hash recompute skipped:", error?.message ?? error);
  }
  console.log("  signed bytes:", tx.signedTransaction.length);
}

async function maybeSubmit(client, label, tx) {
  if (!SHOULD_SUBMIT) return;
  try {
    const response = await client.submitTransaction(tx.signedTransaction);
    console.log(`  submitted ${label}:`, response ?? "<empty>");
  } catch (error) {
    console.warn(`  submission failed for ${label}:`, error?.message ?? error);
  }
}

async function inspectGovernance(client) {
  console.log("\nInspecting governance state...");
  if (GOV_PROPOSAL_ID) {
    const proposal = await client.getGovernanceProposalTyped(GOV_PROPOSAL_ID);
    console.log("  proposal", proposal?.proposal_id, proposal?.status);
  }
  if (GOV_REFERENDUM_ID) {
    const referendum = await client.getGovernanceReferendum(GOV_REFERENDUM_ID);
    console.log("  referendum", referendum?.referendum_id, referendum?.status);
  }
  if (GOV_LOCKS_ID) {
    const locks = await client.getGovernanceLocks(GOV_LOCKS_ID);
    console.log("  locks", locks?.locks?.length ?? 0);
  }
  const council = await client.getGovernanceCouncilCurrent();
  console.log("  council epoch", council?.epoch, "members", council?.members?.length ?? 0);
}

async function main() {
  const client = SHOULD_SUBMIT || SHOULD_FETCH ? new ToriiClient(TORII_URL) : null;
  const transactions = [
    {
      label: "ProposeDeployContract",
      build: () =>
        buildProposeDeployContractTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          proposal: {
            namespace: SAMPLE_NAMESPACE,
            contractId: SAMPLE_CONTRACT_ID,
            codeHash: Buffer.alloc(32, 0xcd),
            abiHash: Buffer.alloc(32, 0xef),
            abiVersion: "1",
            window: { lower: 100, upper: 200 },
            votingMode: "Plain",
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "CastPlainBallot",
      build: () =>
        buildCastPlainBallotTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          ballot: {
            referendumId: SAMPLE_REFERENDUM_ID,
            owner: AUTHORITY,
            amount: "2500",
            durationBlocks: 7_200,
            direction: "aye",
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "EnactReferendum",
      build: () =>
        buildEnactReferendumTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          enactment: {
            referendumId: SAMPLE_REFERENDUM_HASH,
            preimageHash: Buffer.alloc(32, 0xee),
            window: { lower: 300, upper: 360 },
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "FinalizeReferendum",
      build: () =>
        buildFinalizeReferendumTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          finalization: {
            referendumId: SAMPLE_REFERENDUM_ID,
            proposalId: SAMPLE_PROPOSAL_HASH,
          },
          privateKey: PRIVATE_KEY,
        }),
    },
    {
      label: "PersistCouncilForEpoch",
      build: () =>
        buildPersistCouncilForEpochTransaction({
          chainId: CHAIN_ID,
          authority: AUTHORITY,
          record: {
            epoch: 42,
            members: [AUTHORITY],
            candidatesCount: 10,
            derivedBy: "Vrf",
          },
          privateKey: PRIVATE_KEY,
        }),
    },
  ];

  for (const entry of transactions) {
    const tx = entry.build();
    logTransaction(entry.label, tx);
    if (client) {
      // eslint-disable-next-line no-await-in-loop
      await maybeSubmit(client, entry.label, tx);
    }
  }

  if (client && SHOULD_FETCH) {
    await inspectGovernance(client);
  }

  if (!SHOULD_SUBMIT) {
    console.log("\nSet GOV_SUBMIT=1 to push the signed bytes to Torii.");
  }
}

main().catch((error) => {
  console.error("governance recipe failed:", error);
  process.exitCode = 1;
});
```

### 運行和監控

- 執行 `node governance.mjs` 僅生成哈希值。添加 `GOV_SUBMIT=1` 到
  將交易發佈到 Torii 和 `GOV_FETCH=1` 以記錄實時治理狀態
  （`getGovernanceProposal*`、`getGovernanceReferendum`、`getGovernanceLocks` 和
  `getGovernanceCouncilCurrent`）。
- 捕獲 CI 日誌中的確定性哈希值；每一步都會打印有符號的字節
  當可選的本機助手可用時，長度加上重新計算的哈希值。
- 將控制台輸出附加到治理審查數據包中，以便審計員可以跟踪
  提案/公投 ID 返回可複制的 CLI 證據。

## ISO 橋示例

<樣本下載
  href="/sdk-recipes/javascript/iso-bridge.mjs"
  文件名=“iso-bridge.mjs”
  description="下載本節中引用的可運行的 ISO 20022 幫助程序。"
/>

### 先決條件

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
# optional overrides for the wait strategy
export ISO_POLL_ATTEMPTS=24
export ISO_POLL_INTERVAL_MS=1500
# use ISO_MESSAGE_KIND=pacs.009 for PvP submissions
export ISO_MESSAGE_KIND=pacs.008
```

當您想要跳過提交並僅輪詢已知的情況時，請設置 `ISO_MESSAGE_ID`
標識符。使用 `ISO_RESOLVE_ON_ACCEPTED=1` 橋標記後立即退出
條目 `Accepted`，即使交易哈希尚未最終確定。

### 示例腳本

```ts title="iso-bridge.mjs"
#!/usr/bin/env node

import { ToriiClient } from "@iroha/iroha-js";

const TORII_URL = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const MAX_POLL_ATTEMPTS = Number(process.env.ISO_POLL_ATTEMPTS ?? 12);
const POLL_INTERVAL_MS = Number(process.env.ISO_POLL_INTERVAL_MS ?? 2_000);
const MESSAGE_KIND = (process.env.ISO_MESSAGE_KIND ?? "pacs.008").toLowerCase();
const EXISTING_MESSAGE_ID = process.env.ISO_MESSAGE_ID?.trim();
const RESOLVE_ON_ACCEPTED = process.env.ISO_RESOLVE_ON_ACCEPTED === "1";

if (!["pacs.008", "pacs.009"].includes(MESSAGE_KIND)) {
  console.error("ISO_MESSAGE_KIND must be 'pacs.008' or 'pacs.009'");
  process.exit(1);
}

const SAMPLE_PACS008_XML = `<?xml version="1.0" encoding="UTF-8"?>\n` +
  `<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.008.001.10">\n` +
  "  <FIToFICstmrCdtTrf>\n" +
  "    <GrpHdr>\n" +
  `      <MsgId>example-${Date.now()}</MsgId>\n` +
  "      <CreDtTm>2026-01-26T12:00:00Z</CreDtTm>\n" +
  "    </GrpHdr>\n" +
  "    <CdtTrfTxInf>\n" +
  "      <PmtId>\n" +
  "        <InstrId>example-instruction</InstrId>\n" +
  "        <EndToEndId>example-e2e</EndToEndId>\n" +
  "        <TxId>example-transaction</TxId>\n" +
  "      </PmtId>\n" +
  "      <IntrBkSttlmAmt Ccy=\"EUR\">10.00</IntrBkSttlmAmt>\n" +
  "    </CdtTrfTxInf>\n" +
  "  </FIToFICstmrCdtTrf>\n" +
  "</Document>`;

const SAMPLE_PACS009_XML = `<?xml version="1.0" encoding="UTF-8"?>\n` +
  `<Document xmlns="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.10">\n` +
  "  <FICdtTrf>\n" +
  "    <GrpHdr>\n" +
  `      <BizMsgIdr>example-${Date.now()}</BizMsgIdr>\n` +
  "      <MsgDefIdr>pacs.009.001.10</MsgDefIdr>\n" +
  "      <CreDtTm>2026-01-26T12:00:00Z</CreDtTm>\n" +
  "    </GrpHdr>\n" +
  "    <CdtTrfTxInf>\n" +
  "      <PmtId>\n" +
  "        <InstrId>example-instruction</InstrId>\n" +
  "        <TxId>example-tx</TxId>\n" +
  "      </PmtId>\n" +
  "      <IntrBkSttlmAmt Ccy=\"USD\">1000.00</IntrBkSttlmAmt>\n" +
  "      <IntrBkSttlmDt>2026-01-27</IntrBkSttlmDt>\n" +
  "      <InstgAgt>DEUTDEFF</InstgAgt>\n" +
  "      <InstdAgt>DEUTDEFF</InstdAgt>\n" +
  "      <Purp>SECU</Purp>\n" +
  "    </CdtTrfTxInf>\n" +
  "  </FICdtTrf>\n" +
  "</Document>`;

async function main() {
  const client = new ToriiClient(TORII_URL);
  const wait = {
    maxAttempts: MAX_POLL_ATTEMPTS,
    pollIntervalMs: POLL_INTERVAL_MS,
    resolveOnAcceptedWithoutTransaction: RESOLVE_ON_ACCEPTED,
    onPoll: ({ attempt, status }) => {
      if (!status) {
        console.log(`Attempt ${attempt}: status unavailable (retrying)`);
        return;
      }
      const hash = status.transaction_hash ?? "<pending>";
      console.log(`Attempt ${attempt}: ${status.status ?? "unknown"} tx=${hash}`);
    },
  };

  let result;
  if (EXISTING_MESSAGE_ID) {
    console.log(`Waiting for ISO message ${EXISTING_MESSAGE_ID}...`);
    result = await client.waitForIsoMessageStatus(EXISTING_MESSAGE_ID, wait);
  } else {
    const payload = MESSAGE_KIND === "pacs.009" ? SAMPLE_PACS009_XML : SAMPLE_PACS008_XML;
    console.log(`Submitting ${MESSAGE_KIND} payload to ${TORII_URL}`);
    result =
      MESSAGE_KIND === "pacs.009"
        ? await client.submitIsoPacs009AndWait(payload, { wait })
        : await client.submitIsoPacs008AndWait(payload, { wait });
  }

  const hash = result.transaction_hash ?? "<pending>";
  console.log(
    `Final status for ${result.message_id}: ${result.status ?? "unknown"} tx=${hash}`,
  );
}

main().catch((error) => {
  console.error("ISO bridge recipe failed:", error);
  process.exitCode = 1;
});
```

### 運行和監控

- 執行 `node iso-bridge.mjs` 以提交示例有效負載。套裝
  `ISO_MESSAGE_KIND=pacs.009` 執行 PvP 流程或 `ISO_MESSAGE_ID` 執行 PvP 流程
  輪詢現有提交內容而不重新發布它。
- 幫助程序通過 `wait.onPoll` 記錄每次輪詢嘗試，從而輕鬆
  捕獲 CI 日誌中的接受時間表。
- 將最終狀態 + 交易哈希值附加到 ISO 橋操作手冊，以便審核員
  可以追踪 pacs.008/pacs.009 傳送回可重現的有效負載，如
  JS5 路線圖可交付成果所要求的。

## 線下津貼和轉賬

`@iroha/iroha-js` 運送與
離線路線圖行。使用它們來檢查完整性策略（標記密鑰、播放
完整性、HMS 安全檢測、已配置），無需解析原始元數據：

```bash
# List recent allowances and log their integrity policies
TORII_URL=https://torii.nexus.example \
node -e '
  import { ToriiClient } from "@iroha/iroha-js";
  const client = new ToriiClient(process.env.TORII_URL);
  const page = await client.listOfflineAllowances({ limit: 5 });
  for (const entry of page.items) {
    const policy = entry.integrity_metadata?.policy ?? "<unknown>";
    console.log(entry.controller_display, policy);
  }
'

# Summarize transfers + Provisioned manifest metadata
TORII_URL=https://torii.nexus.example \
node -e '
  import { ToriiClient } from "@iroha/iroha-js";
  const client = new ToriiClient(process.env.TORII_URL);
  const page = await client.listOfflineTransfers({ limit: 5 });
  for (const entry of page.items) {
    const manifest = entry.integrity_metadata?.provisioned?.manifest_schema ?? "<none>";
    console.log(entry.bundle_id_hex, entry.integrity_metadata?.policy ?? "<unknown>", manifest);
  }
'
```

當 Torii 報告預配置津貼時，檢查員公鑰、清單
架構、可選版本、TTL 和摘要如下
`integrity_metadata.provisioned`，因此可以輕鬆附加所需的
OA10.3 證據數據包的元數據。

## 後續步驟

- 探索 `javascript/iroha_js/recipes/governance.mjs` 和
  `javascript/iroha_js/recipes/iso_bridge.mjs` 用於擴展示例（多重簽名
  選票、理事會 VRF 推導、重試政策）。
- 查看 Norito 側文檔
  `docs/source/sdk/js/governance_iso_examples.md` 和
  `docs/source/finance/settlement_iso_mapping.md` 用於規範字段
  這些助手引用的映射。
- 捕獲運行日誌並將其附加到治理/ISO 批准中，以滿足
  `roadmap.md` 中引用的 JS5“文檔+發布”要求。