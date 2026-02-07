---
lang: my
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

'@site/src/components/SampleDownload' မှ SampleDownload ကို တင်သွင်းပါ။

ဤစာရွက်သည် JS5 လမ်းပြမြေပုံတွင် ခေါ်သော အဆင့်မြင့်လုပ်ငန်းအသွားအလာနှစ်ခုကို စုစည်းထားသည်။
အကြောင်းအရာများ- အစမှအဆုံး အုပ်ချုပ်မှုအထောက်အပံများ (အဆိုပြုချက်များ၊ မဲများ၊ ကောင်စီလျှပ်တစ်ပြက်ပုံများ)
နှင့် ISO 20022 တံတားလမ်းညွှန်ချက် (pacs.008/pacs.009)။ နမူနာတစ်ခုစီသည် အလုပ်လုပ်သည်။
ထုတ်ဝေထားသော `@iroha/iroha-js` ပက်ကေ့ဂျ်ကိုဆန့်ကျင်ပြီး အတိုအထွာများကို အလင်းပြန်ပေးပါ။
`docs/source/sdk/js/governance_iso_examples.md`။

## အုပ်ချုပ်မှုအကူအညီနမူနာ

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/javascript/governance.mjs"
  ဖိုင်အမည် = "governance.mjs"
  description="ဤစာရွက်တွင် ကိုးကားထားသော လုပ်ဆောင်နိုင်သော အုပ်ချုပ်မှုအကူအညီကို ဒေါင်းလုဒ်လုပ်ပါ။"
/>

### ကြိုတင်လိုအပ်ချက်များ

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
export AUTHORITY="ih58..."
export PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)"
export CHAIN_ID="00000000-0000-0000-0000-000000000000"
# optional lookups for GOV_FETCH
export GOV_PROPOSAL_ID="calc.v1"
export GOV_REFERENDUM_ID="calc-referendum"
export GOV_LOCKS_ID="calc-locks"
```

I18NT000000012X ကို Torii သို့ လက်မှတ်ရေးထိုးထားသော လွှဲပြောင်းမှုများ တင်သွင်းရန် နှင့်
တင်ပြပြီးနောက် ရရှိလာသော အုပ်ချုပ်မှုအခြေအနေကို စစ်ဆေးရန် `GOV_FETCH=1`။

### ဥပမာ ဇာတ်ညွှန်း

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
const AUTHORITY = process.env.AUTHORITY ?? "ih58...";
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

### လည်ပတ်ပြီး စောင့်ကြည့်ပါ။

- hash များကိုသာထုတ်ပေးရန် `node governance.mjs` ကိုလုပ်ဆောင်ပါ။ `GOV_SUBMIT=1` သို့ ထည့်ပါ။
  တိုက်ရိုက်အုပ်ချုပ်မှုအခြေအနေသို့ဝင်ရောက်ရန် Torii နှင့် `GOV_FETCH=1` တွင် ငွေပေးငွေယူမှုများကို ပို့စ်တင်ပါ
  (`getGovernanceProposal*`၊ `getGovernanceReferendum`၊ `getGovernanceLocks` နှင့်
  `getGovernanceCouncilCurrent`)။
- CI မှတ်တမ်းများတွင် အဆုံးအဖြတ်ရှိသော hashe များကို ဖမ်းယူပါ။ အဆင့်တိုင်းတွင် လက်မှတ်ထိုးထားသော byte ကို ပရင့်ထုတ်သည်။
  ရွေးချယ်နိုင်သော ဇာတိအကူအညီကို ရနိုင်သောအခါတွင် အရှည်နှင့် ပြန်လည်တွက်ချက်ထားသော hash။
- စာရင်းစစ်များသည် ခြေရာခံနိုင်စေရန် အုပ်ချုပ်မှုပြန်လည်သုံးသပ်ခြင်းထုပ်ပိုးမှုများတွင် ကွန်ဆိုးအထွက်ကို ပူးတွဲပါ။
  အဆိုပြုချက် / ပြည်လုံးကျွတ်ဆန္ဒခံယူပွဲ ID များကို ပြန်လည်ထုတ်လုပ်နိုင်သော CLI အထောက်အထားများ။

## ISO တံတားနမူနာ

<နမူနာဒေါင်းလုဒ်လုပ်ပါ။
  href="/sdk-recipes/javascript/iso-bridge.mjs"
  ဖိုင်အမည် = "iso-bridge.mjs"
  description="ဤစာရွက်တွင်ဖော်ပြထားသော runnable ISO 20022 အကူအညီကို ဒေါင်းလုဒ်လုပ်ပါ။"
/>

### ကြိုတင်လိုအပ်ချက်များ

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
# optional overrides for the wait strategy
export ISO_POLL_ATTEMPTS=24
export ISO_POLL_INTERVAL_MS=1500
# use ISO_MESSAGE_KIND=pacs.009 for PvP submissions
export ISO_MESSAGE_KIND=pacs.008
```

တင်သွင်းမှုကို ကျော်ပြီး သိရှိသူကိုသာ ကောက်ယူလိုသောအခါတွင် `ISO_MESSAGE_ID` ကို သတ်မှတ်ပါ
အမှတ်အသား။ တံတားအမှတ်အသားကို အမြန်ဆုံးထွက်ရန် `ISO_RESOLVE_ON_ACCEPTED=1` ကိုသုံးပါ။
ငွေပေးငွေယူ hash သည် အပြီးသတ်မပြီးသေးသော်လည်း `Accepted` ၏ entry တစ်ခု။

### ဥပမာ ဇာတ်ညွှန်း

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

### လည်ပတ်ပြီး စောင့်ကြည့်ပါ။

- နမူနာ payload တင်သွင်းရန် `node iso-bridge.mjs` ကို လုပ်ဆောင်ပါ။ သတ်မှတ်
  PvP စီးဆင်းမှုကို လေ့ကျင့်ရန် `ISO_MESSAGE_KIND=pacs.009` သို့မဟုတ် `ISO_MESSAGE_ID` သို့
  ပြန်မတင်ဘဲ လက်ရှိတင်ပြမှုကို စစ်တမ်းကောက်ယူပါ။
- အကူအညီပေးသူက `wait.onPoll` မှတစ်ဆင့် စစ်တမ်းတစ်ခုစီတိုင်းကို လွယ်ကူစွာ မှတ်သားထားပြီး၊
  CI မှတ်တမ်းများတွင် လက်ခံမှုအချိန်ဇယားများကို ဖမ်းယူပါ။
- နောက်ဆုံးအခြေအနေ + ငွေပေးငွေယူ hash ကို ISO ပေါင်းကူး runbooks သို့ တွဲ၍ စာရင်းစစ်သူများ၊
  pacs.008/pacs.009 ပေးပို့မှုများကို ပြန်လည်ထုတ်လုပ်နိုင်သော payloads အဖြစ် ခြေရာခံနိုင်သည်
  JS5 လမ်းပြမြေပုံမှ လိုအပ်သည်

## အော့ဖ်လိုင်းထောက်ပံ့ကြေးများနှင့် လွှဲပြောင်းမှုများ

`@iroha/iroha-js` တွင် ကိုးကားဖော်ပြထားသော တူညီသောထောက်ပံ့ကြေး/လွှဲပြောင်းကူညီသူများကို ပို့ဆောင်ပေးသည်
အော့ဖ်လိုင်း လမ်းပြမြေပုံ အတန်းများ။ ခိုင်မာမှုမူဝါဒများကို စစ်ဆေးရန် ၎င်းတို့ကို အသုံးပြုပါ (အမှတ်အသားသော့၊ Play
တိကျမှု၊ မက်တာဒေတာကို ခွဲခြမ်းစိတ်ဖြာခြင်းမရှိဘဲ စီမံထားသော)၊

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

Torii က စီမံထားသော ထောက်ပံ့ကြေးကို အစီရင်ခံသောအခါ စစ်ဆေးရေးမှူး အများသူငှာသော့ကို ထင်ရှားစွာပြပါ၊
schema၊ optional version၊ TTL နှင့် digest အောက်တွင် တိုက်ရိုက်ထုတ်လွှင့်သည်။
`integrity_metadata.provisioned`၊ လိုအပ်သည်များကိုပူးတွဲရန်အသေးအဖွဲဖြစ်စေသည်။
မက်တာဒေတာသည် OA10.3 အထောက်အထားထုပ်များ။

## နောက်တစ်ဆင့်

- `javascript/iroha_js/recipes/governance.mjs` နှင့် စူးစမ်းလေ့လာပါ။
  ချဲ့ထွင်ထားသော ဥပမာများအတွက် `javascript/iroha_js/recipes/iso_bridge.mjs` (sig ပေါင်းများစွာ
  မဲများ၊ ကောင်စီ VRF ဆင်းသက်လာမှု၊ ထပ်စမ်းကြည့်ပါ မူဝါဒများ)။
- Norito-side documentation in ကို ပြန်သုံးသပ်ပါ။
  `docs/source/sdk/js/governance_iso_examples.md` နှင့်
  Canonical အကွက်အတွက် `docs/source/finance/settlement_iso_mapping.md`
  ဤအကူအညီပေးသူများမှကိုးကားထားသောမြေပုံများ။
- လုပ်ဆောင်နေသောမှတ်တမ်းများကိုဖမ်းယူပြီး ၎င်းတို့အား ကျေနပ်စေရန် အုပ်ချုပ်မှု/ ISO အတည်ပြုချက်များတွင် ပူးတွဲပါရှိသည်။
  JS5 သည် `roadmap.md` တွင် ကိုးကားထားသော "စာရွက်စာတမ်း + ထုတ်ဝေခြင်း" လိုအပ်ချက်။