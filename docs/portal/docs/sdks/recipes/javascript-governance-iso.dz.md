---
lang: dz
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

filight Sample load འདི་ '@site/src/ཆ་ཤས་/ཆ་ཤས་/དཔེ་ཚད་ཕབ་ལེན་';

འདི་གི་ཐབས་ཤེས་འདི་གིས་ JS5 ལམ་སྟོན་ནང་ལུ་ ཡར་ཐོན་ཅན་གྱི་ལཱ་གི་རྒྱུན་རིམ་གཉིས་འདི་ བསྡམས་ཡོདཔ་ཨིན།
རྣམ་གྲངས་: མཇུག་ལས་མཇུག་ཚུན་ཚོད་ གཞུང་སྐྱོང་རོགས་སྐྱོར་ (གྲོས་འཆར་དང་ ཚོགས་རྒྱན་ ཚོགས་སྡེའི་པར་བཏབ་ནི།)
དང་ ISO 20022 ཟམ་གྱི་ལམ་ཐིག་ (pacs.008/pacs.009). དཔེ་ཚད་རེ་རེ་བཞིན་ གཡོག་བཀོལ།
དཔར་བསྐྲུན་འབད་ཡོད་པའི་ `@iroha/iroha-js` ཐུམ་སྒྲིལ་དང་ ༢༠༡༦ ལུ་ ཤོག་འཛིན་ཚུ་ མེ་ལོང་ནང་ བཏོནམ་ཨིན།
`docs/source/sdk/js/governance_iso_examples.md`.

## གཞུང་སྐྱོང་རོགས་སྐྱོར་དཔེ་ཚད།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལན་/ཇ་བ་སི་ཀིརིཔརིཀ་ཊི་/གཱོན་ནེནསི་.ཨེམ་ཇེ་སི།"
  filen="governance.mjs"
  secont="ཐབས་ཤེས་འདི་ནང་ རྒྱབ་རྟེན་འབད་མི་ གཞུང་སྐྱོང་གྲོགས་རམ་པ་ཕབ་ལེན་འབད།"
།/>།

### སྔོན་འགྲོའི་ཆ་རྐྱེན།

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
export AUTHORITY="i105..."
export PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)"
export CHAIN_ID="00000000-0000-0000-0000-000000000000"
# optional lookups for GOV_FETCH
export GOV_PROPOSAL_ID="calc.v1"
export GOV_REFERENDUM_ID="calc-referendum"
export GOV_LOCKS_ID="calc-locks"
```

I18NI000000012X མིང་རྟགས་བཀོད་པའི་ཚོང་འབྲེལ་ཚུ་ Torii དང་ ལུ་བཙུགས་ནིའི་དོན་ལུ་ གཞི་སྒྲིག་འབད།
I18NI0000000013X གྲུབ་འབྲས་གཞུང་སྐྱོང་མངའ་སྡེ་འདི་ བཙུགས་པའི་ཤུལ་ལས་ བརྟག་དཔྱད་འབད་ནིའི་དོན་ལུ་ཨིན།

### དཔེར་ཡིག་།

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
const AUTHORITY = process.env.AUTHORITY ?? "i105...";
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

### གཡོག་བཀོལ།

- ཧེསི་རྐྱངམ་ཅིག་བཟོ་ནི་གི་དོན་ལུ་ `node governance.mjs` ལག་ལེན་འཐབ་དགོ། I18NI0000015X ལུ་ཁ་སྐོང་འབད།
  ཚོང་འབྲེལ་ཚུ་ I18NT000000003X དང་ I18NI000000016X ལུ་བཙུགས་ནིའི་དོན་ལུ་ གཞུང་སྐྱོང་མངའ་སྡེ་ཚུ་ ལོག་བཏང་དགོ།
  (I18NI0000000017X, `getGovernanceReferendum`, `getGovernanceLocks`, དང་།
  `getGovernanceCouncilCurrent`).
- སི་ཨའི་དྲན་ཐོ་ནང་ གཏན་འབེབས་ཀྱི་ ཧ་ཤེ་ཚུ་ བསྡུ་ལེན་འབད་ནི། གོ་རིམ་རེ་རེ་གིས་ མིང་རྟགས་བཀོད་ཡོད་པའི་བཱའིཊི་འདི་དཔར་བསྐྲུན་འབདཝ་ཨིན།
  རིང་ཚད་འདི་གདམ་ཁ་ཅན་གྱི་ས་གནས་ཀྱི་རོགས་རམ་འཐོབ་ཚུགས་པའི་སྐབས་ བསྐྱར་རྩིས་འབད་ཡོད་པའི་ཧ་ཤི་འདི་ཁ་སྐོང་འབད།
- རྩིས་ཞིབ་པ་ཚུ་གིས་ འཚོལ་ཞིབ་འབད་ཚུགས་ནིའི་དོན་ལུ་ གཞུང་སྐྱོང་བསྐྱར་ཞིབ་འབད་ནིའི་དོན་ལུ་ ཀོན་སོལ་ཨའུཊི་པུཊི་འདི་ མཐུད་དགོ།
  གྲོས་འཆར་ / འོས་བསྡུའི་ངོ་རྟགས་ཚུ་ བསྐྱར་བཟོ་འབད་ཚུགས་པའི་ CLI སྒྲུབ་བྱེད་ལུ་ ལོག་འོང་།

## ISO ཟམ་པའི་དཔེ་ཚད།

<དཔེ་ཚད་ཕབ་ལེན་འབད།
  href="/sdk-ལེན་འཐབ།/ཇ་བ་སི་ཀིརིཔཀྲི་/ཨའི་སོ་-ཟམ་.ཨེམ་ཇེ་སི།"
  filen name="iso-bridge.mjs"
  secont="བཟའ་ཐབས་འདི་ནང་ གཡོག་བཀོལ་བཏུབ་པའི་ ISO 20022 གྲོགས་རམ་པ་ ཕབ་ལེན་འབད།"
།/>།

### སྔོན་འགྲོའི་ཆ་རྐྱེན།

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
# optional overrides for the wait strategy
export ISO_POLL_ATTEMPTS=24
export ISO_POLL_INTERVAL_MS=1500
# use ISO_MESSAGE_KIND=pacs.009 for PvP submissions
export ISO_MESSAGE_KIND=pacs.008
```

ཁྱོད་ཀྱིས་ ཞུ་ཡིག་བཙུགས་མ་བཏུབ་པའི་སྐབས་ `ISO_MESSAGE_ID` འདི་ གཞི་སྒྲིག་འབད་ཞིནམ་ལས་ ཤེས་མི་ཅིག་ འོས་བསྡུ།
ངོས་འཛིན་འབད་མི། ཟམ་གྱི་སྐུགས་ཐོབ་པའི་བསྒང་ལས་ ཕྱིར་ཐོན་འབད་ནི་ལུ་ I18NI0000022X ལག་ལེན་འཐབ།
ཐོ་བཀོད་ `Accepted` ཚོང་འབྲེལ་ཧེཤ་འདི་ད་ལྟོ་མཇུག་བསྡུ་མ་ཚུགས་རུང་ཡང་།

### དཔེར་ཡིག་།

I18NF0000008X

### གཡོག་བཀོལ།

- དཔེ་ཚད་པེ་ལོཌི་ཅིག་བཙུགས་ནིའི་དོན་ལུ་ `node iso-bridge.mjs` ལག་ལེན་འཐབ། སྡེ༌ཚན
  `ISO_MESSAGE_KIND=pacs.009` གིས་ PvP རྒྱུན་འགྲུལ་ཡང་ན་ `ISO_MESSAGE_ID` ལུ་ལག་ལེན་འཐབ་ནི།
  འོས་ཤོག་འོགམ་འདི་ ལོག་མ་བཙུགས་པར་ ད་ལྟོ་ཡོད་པའི་ ཞུ་ཡིག་ཅིག།
- གྲོགས་རམ་པ་འདི་གིས་ `wait.onPoll` བརྒྱུད་དེ་ འོས་བསྡུའི་དཔའ་བཅམ་མི་ཚུ་ག་ར་ འཇམ་ཏོང་ཏོ་བཟོཝ་ཨིན།
  CI དྲན་ཐོ་ཚུ་ནང་ ངོས་ལེན་དུས་ཚོད་གྲལ་ཐིག་ཚུ་ འཛིན་བཟུང་འབད།
- མཐའ་མཇུག་གི་གནས་རིམ་ + ཚོང་འབྲེལ་ཧེཤ་ ཨའི་ཨེསི་ཨོ་ཟམ་རན་དེབ་ཚུ་ལུ་དེ་སྦེ་རྩིས་ཞིབ་པ་ཚུ་མཉམ་སྦྲགས་འབད།
  can trace.008/pacs.009 བསྐྱར་བཟོ་ཐུབ་པའི་གླ་ཆ་ལུ་ལོག་སྤྲོད་ནི།
  JS5 roadpe གིས་ དགོས་མཁོ་ཡོདཔ།

## ཟུར་ཐོ ་ དང་ སྤོ་བཤུད།

`@iroha/iroha-js` ནང་ གཞི་བསྟུན་འབད་ཡོད་པའི་ འཐུས་དང་ བརྗེ་སོར་གྱི་གྲོགས་རམ་པ་ཚུ་ གཅིག་མཚུངས་སྦེ་ བཏངམ་ཨིན།
ofline save dain གྱལ་ཚུ། དེ་ཚུ་ ཆིག་སྒྲིལ་སྲིད་བྱུས་ཚུ་ བརྟག་དཔྱད་འབད་ནི་ལུ་ ལག་ལེན་འཐབ།
ཆིག་སྒྲིལ་དང་ ཨེཆ་ཨེམ་ཨེསི་ཉེན་སྲུང་ཤེས་རྟོགས་ དགོངས་ཞུ་འབད་ཡོདཔ་)

I18NF0000009X

Torii གིས་ དགོངས་ཞུ་འབད་མི་ ཞིབ་དཔྱད་ཀྱི་འཐུས་ཅིག་ མི་མང་ལྡེ་མིག་ལུ་ སྙན་ཞུ་འབད་བའི་སྐབས་ གསལ་སྟོན་འབདཝ་ཨིན།
ལས་འཆར་དང་ གདམ་ཁའི་ཐོན་རིམ་ ཊི་ཊི་ཨེལ་ དེ་ལས་ བཞུ་བཅོས་ཚུ་ འོག་ལུ་སྡོདཔ་ཨིན།
`integrity_metadata.provisioned`, དགོས་མཁོ་འདི་མཉམ་སྦྲགས་འབད་ནི་ལུ་ གལ་གནད་ཅན་བཟོཝ་ཨིན།
མེ་ཊ་ཌེ་ཊ་ ལས་ OA10.3 སྒྲུབ་བྱེད་ཐུམ་སྒྲིལ་ཚུ།

## ཤུལ་མམ་གྱི་གོམ་པ།

- འཚོལ།- I18NI000000030X དང་།
  རྒྱ་བསྐྱེད་འབད་ཡོད་པའི་དཔེ་ཚུ་གི་དོན་ལུ་ `javascript/iroha_js/recipes/iso_bridge.mjs` (multi-sig
  ཚོགས་རྒྱན་བཙུགས་ནི་དང་ ཚོགས་སྡེ་ཝི་ཨར་ཨེཕ་ བསྐྱར་བཟོའི་སྲིད་བྱུས།)
- སྤྱི་ལོ་༢༠༡༨ ལུ་ Norito-ཟུར་ཡིག་ཆ་ཚུ་བསྐྱར་ཞིབ་འབད།
  `docs/source/sdk/js/governance_iso_examples.md` དང་།
  ཀེ་ནོ་ནིག་ས་སྒོ་གི་དོན་ལུ་ `docs/source/finance/settlement_iso_mapping.md`
  གྲོགས་རམ་འདི་ཚུ་གིས་ གཞི་བསྟུན་འབད་མི་སབ་ཁྲ་ཚུ།
- དྲན་ཐོ་ཚུ་ བཟུང་ཞིནམ་ལས་ གཞུང་སྐྱོང་ལུ་ མཉམ་སྦྲགས་འབད་ནི།
  JS5 “ཡིག་ཆ་ + དཔེ་སྐྲུན་” དགོས་མཁོ་ `roadmap.md` ནང་ གཞི་བསྟུན་འབད་ཡོདཔ།