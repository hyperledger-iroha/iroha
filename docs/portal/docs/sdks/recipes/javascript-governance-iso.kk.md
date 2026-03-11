---
lang: kk
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

SampleDownload файлын '@site/src/components/SampleDownload' ішінен импорттау;

Бұл рецепт JS5 жол картасында айтылған екі жетілдірілген жұмыс үрдісін біріктіреді
элементтер: басқарудың түпкілікті көмекшілері (ұсыныстар, бюллетеньдер, кеңес суреттері)
және ISO 20022 көпірі бойынша шолу (pacs.008/pacs.009). Әрбір үлгі іске қосылады
жарияланған `@iroha/iroha-js` пакетіне қарсы және үзінділерді көрсетеді
`docs/source/sdk/js/governance_iso_examples.md`.

## Басқару көмекшісі үлгісі

<Үлгі жүктеп алу
  href="/sdk-recipes/javascript/governance.mjs"
  файл атауы = "governance.mjs"
  description="Осы рецептте сілтеме жасалған орындалатын басқару көмекшісін жүктеп алыңыз."
/>

### Алғышарттар

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

Қол қойылған транзакцияларды Torii мекенжайына жіберу үшін `GOV_SUBMIT=1` параметрін орнатыңыз және
`GOV_FETCH=1` жібергеннен кейін алынған басқару күйін тексеру үшін.

### Сценарий үлгісі

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

### Іске қосу және бақылау

- Тек хэштерді жасау үшін `node governance.mjs` орындаңыз. `GOV_SUBMIT=1` қосыңыз
  тікелей басқару күйін тіркеу үшін транзакцияларды Torii және `GOV_FETCH=1` мекенжайларына орналастырыңыз
  (`getGovernanceProposal*`, `getGovernanceReferendum`, `getGovernanceLocks`, және
  `getGovernanceCouncilCurrent`).
- CI журналдарында детерминирленген хэштерді түсіру; әрбір қадам қол қойылған байтты басып шығарады
  Қосымша жергілікті көмекші қол жетімді болғанда ұзындық пен қайта есептелген хэш.
- Аудиторлар қадағалай алатындай консоль шығысын басқаруды шолу пакеттеріне тіркеңіз
  ұсыныс/референдум идентификаторлары қайталанатын CLI дәлелдеріне оралады.

## ISO көпірі үлгісі

<Үлгі жүктеп алу
  href="/sdk-recipes/javascript/iso-bridge.mjs"
  файл атауы = "iso-bridge.mjs"
  description="Осы рецептте сілтеме жасалған орындалатын ISO 20022 көмекшісін жүктеп алыңыз."
/>

### Алғышарттар

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
# optional overrides for the wait strategy
export ISO_POLL_ATTEMPTS=24
export ISO_POLL_INTERVAL_MS=1500
# use ISO_MESSAGE_KIND=pacs.009 for PvP submissions
export ISO_MESSAGE_KIND=pacs.008
```

Жіберуді өткізіп жібергіңіз келсе және тек белгілі сұрау салғыңыз келгенде `ISO_MESSAGE_ID` орнатыңыз.
идентификатор. Көпір белгіленген бойда шығу үшін `ISO_RESOLVE_ON_ACCEPTED=1` пайдаланыңыз
`Accepted` жазбасы, тіпті транзакция хэші әлі аяқталмаған болса да.

### Сценарий үлгісі

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

### Іске қосу және бақылау

- Үлгі пайдалы жүктемені жіберу үшін `node iso-bridge.mjs` орындаңыз. Орнату
  PvP ағынын орындау үшін `ISO_MESSAGE_KIND=pacs.009` немесе `ISO_MESSAGE_ID`
  бұрыннан бар жіберуді қайта жарияламай сұрау.
- Көмекші `wait.onPoll` арқылы әрбір сауалнама әрекетін тіркейді, бұл
  CI журналдарында қабылдау уақыт шкаласын түсіріңіз.
- Аудиторлар үшін соңғы мәртебені + транзакция хэшін ISO bridge runbook файлдарына тіркеңіз
  pacs.008/pacs.009 жеткізілімдерін қайталанатын пайдалы жүктемелерге дейін бақылай алады, мысалы
  JS5 жол картасының жеткізілімдері талап етеді.

## Офлайн төлемдер мен аударымдар

`@iroha/iroha-js` құжатта сілтеме жасалған бірдей жәрдемақы/трансфер көмекшілерін жібереді.
офлайн жол картасы жолдары. Оларды тұтастық саясаттарын тексеру үшін пайдаланыңыз (маркер кілті, Ойнату
Integrity, HMS Safety Detect, Provisioned) шикі метадеректерді талдаусыз:

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

Torii Қамтамасыз етілген жеңілдік туралы есеп бергенде, инспектордың ашық кілті, манифест
схема, қосымша нұсқа, TTL және дайджест астында
`integrity_metadata.provisioned`, бұл қажеттіні тіркеуді жеңілдетеді
OA10.3 дәлелдеме пакеттеріне метадеректер.

## Келесі қадамдар

- `javascript/iroha_js/recipes/governance.mjs` және
  Кеңейтілген мысалдар үшін `javascript/iroha_js/recipes/iso_bridge.mjs` (көп сиг
  бюллетеньдер, кеңестің VRF шығаруы, қайталау саясаттары).
- Norito жағындағы құжаттаманы қараңыз
  `docs/source/sdk/js/governance_iso_examples.md` және
  Канондық өріс үшін `docs/source/finance/settlement_iso_mapping.md`
  осы көмекшілер сілтеме жасаған салыстырулар.
- Орындау журналдарын жазып алыңыз және оларды қанағаттандыру үшін басқару / ISO мақұлдауларына тіркеңіз
  `roadmap.md` құжатында сілтеме жасалған JS5 "құжаттама + жариялау" талабы.