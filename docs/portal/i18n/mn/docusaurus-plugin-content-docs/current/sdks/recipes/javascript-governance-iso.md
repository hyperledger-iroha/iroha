---
slug: /sdks/recipes/javascript-governance-iso
lang: mn
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript governance & ISO recipe
description: Run the governance helpers and ISO 20022 bridge flows shipped with @iroha/iroha-js, including runnable CLI samples.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-г '@site/src/components/SampleDownload'-аас импортлох;

Энэхүү жор нь JS5 замын зураглалд дурдсан хоёр дэвшилтэт ажлын урсгалыг нэгтгэдэг
зүйлс: төгсгөлийн засаглалын туслахууд (санал, саналын хуудас, зөвлөлийн хормын хувилбар)
болон ISO 20022 гүүрний танилцуулга (pacs.008/pacs.009). Дээж бүр ажилладаг
нийтлэгдсэн `@iroha/iroha-js` багцын эсрэг бөгөөд хэсэгчилсэн хэсгүүдийг тусгадаг
`docs/source/sdk/js/governance_iso_examples.md`.

## Засаглалын туслагчийн жишээ

<Жишээ татаж авах
  href="/sdk-recipes/javascript/governance.mjs"
  файлын нэр = "governance.mjs"
  description="Энэ жор дээр дурдсан удирдаж болох засаглалын туслахыг татаж авна уу."
/>

### Урьдчилсан нөхцөл

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
export AUTHORITY="soraカタカナ..."
export PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)"
export CHAIN_ID="00000000-0000-0000-0000-000000000000"
# optional lookups for GOV_FETCH
export GOV_PROPOSAL_ID="calc.v1"
export GOV_REFERENDUM_ID="calc-referendum"
export GOV_LOCKS_ID="calc-locks"
```

Гарын үсэг зурсан гүйлгээг Torii руу илгээхийн тулд `GOV_SUBMIT=1`-г тохируулж,
`GOV_FETCH=1` өргөн мэдүүлсний дараа үүссэн засаглалын төлөвийг шалгах.

### Жишээ скрипт

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
const AUTHORITY = process.env.AUTHORITY ?? "soraカタカナ...";
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

### Ажиллаж, хянах

- Зөвхөн хэш үүсгэхийн тулд `node governance.mjs`-г ажиллуул. `GOV_SUBMIT=1` дээр нэмнэ үү
  шууд засаглалын төлөвийг бүртгэхийн тулд гүйлгээг Torii болон `GOV_FETCH=1` руу илгээнэ үү.
  (`getGovernanceProposal*`, `getGovernanceReferendum`, `getGovernanceLocks`, ба
  `getGovernanceCouncilCurrent`).
- CI логууд дахь детерминистик хэшүүдийг авах; алхам бүр гарын үсэг зурсан байтыг хэвлэдэг
  нэмэлт эх туслах боломжтой үед урт болон дахин тооцоолсон хэш.
- Консолын гаралтыг засаглалын хяналтын багцад хавсаргаснаар аудиторууд мөшгих боломжтой
  санал / бүх нийтийн санал асуулгын ID-г хуулбарлах боломжтой CLI нотлох баримтууд руу буцаана.

## ISO гүүрний дээж

<Жишээ татаж авах
  href="/sdk-recipes/javascript/iso-bridge.mjs"
  файлын нэр = "iso-bridge.mjs"
  description="Энэ жоронд дурдсан ажиллах боломжтой ISO 20022 туслах програмыг татаж авна уу."
/>

### Урьдчилсан нөхцөл

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
# optional overrides for the wait strategy
export ISO_POLL_ATTEMPTS=24
export ISO_POLL_INTERVAL_MS=1500
# use ISO_MESSAGE_KIND=pacs.009 for PvP submissions
export ISO_MESSAGE_KIND=pacs.008
```

Илгээмжийг алгасахдаа `ISO_MESSAGE_ID`-г тохируулж, зөвхөн мэдэгдэж буй санал асуулга явуулна уу.
танигч. Гүүр тэмдэглэгдсэн даруйд гарахын тулд `ISO_RESOLVE_ON_ACCEPTED=1` ашиглана уу
гүйлгээний хэш хараахан дуусаагүй байсан ч гэсэн `Accepted` оруулга.

### Жишээ скрипт

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

### Ажиллаж, хянах

- Дээжийн ачааллыг оруулахын тулд `node iso-bridge.mjs`-г ажиллуул. Тохируулах
  PvP урсгалыг ашиглахын тулд `ISO_MESSAGE_KIND=pacs.009` эсвэл `ISO_MESSAGE_ID`
  Одоо байгаа материалыг дахин нийтлэхгүйгээр санал асуулга явуулах.
- Туслах нь санал асуулгын оролдлого бүрийг `wait.onPoll`-ээр бүртгэдэг бөгөөд ингэснээр үүнийг хийхэд хялбар болгодог.
  CI бүртгэлд хүлээн авах хугацааг бичих.
- Эцсийн статус + гүйлгээний хэшийг ISO bridge runbook-д хавсаргаснаар аудиторууд
  pacs.008/pacs.009 хүргэлтийг дахин давтагдах боломжтой ачааллыг хянах боломжтой.
  JS5 замын газрын зургийн үр дүнд шаардлагатай.

## Офлайн тэтгэмж, шилжүүлэг

`@iroha/iroha-js`-д дурдсан ижил тэтгэмж/шилжүүлэх туслахуудыг илгээдэг.
офлайн замын зургийн мөр. Бүрэн бүтэн байдлын бодлогыг шалгахын тулд тэдгээрийг ашиглана уу (тэмдэглэгээний түлхүүр, Play
бүрэн бүтэн байдал, HMS Safety Detect, Provisioned) нь түүхий мета өгөгдлийг задлахгүйгээр:

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

Torii өгөгдсөн тэтгэмжийн талаар мэдээлэх үед байцаагчийн нийтийн түлхүүр, манифест
схем, нэмэлт хувилбар, TTL, болон дижест дор амьдардаг
`integrity_metadata.provisioned`, шаардлагатай хавсаргах нь өчүүхэн болгодог
OA10.3 нотолгооны багцын мета өгөгдөл.

## Дараагийн алхамууд

- `javascript/iroha_js/recipes/governance.mjs` болон
  Өргөтгөсөн жишээнүүдийн хувьд `javascript/iroha_js/recipes/iso_bridge.mjs` (олон сиг
  саналын хуудас, зөвлөлийн VRF үүсмэл, дахин оролдох бодлого).
- Norito талын баримт бичгийг шалгана уу
  `docs/source/sdk/js/governance_iso_examples.md` ба
  Каноник талбарт зориулсан `docs/source/finance/settlement_iso_mapping.md`
  эдгээр туслахуудын иш татсан зураглал.
- Ашиглалтын бүртгэлийг авч, тэдгээрийг засаглалын / ISO зөвшөөрөлд хавсаргана
  JS5 "баримт бичиг + нийтлэх" шаардлагыг `roadmap.md`-д иш татсан.