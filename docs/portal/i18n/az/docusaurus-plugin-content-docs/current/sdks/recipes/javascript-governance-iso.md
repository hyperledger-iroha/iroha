---
slug: /sdks/recipes/javascript-governance-iso
lang: az
direction: ltr
source: docs/portal/docs/sdks/recipes/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: JavaScript governance & ISO recipe
description: Run the governance helpers and ISO 20022 bridge flows shipped with @iroha/iroha-js, including runnable CLI samples.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

SampleDownload-ı '@site/src/components/SampleDownload'dan idxal edin;

Bu resept JS5 yol xəritəsində göstərilən iki qabaqcıl iş axınını birləşdirir
maddələr: başdan-başa idarəetmə köməkçiləri (təkliflər, bülletenlər, şuranın şəkilləri)
və ISO 20022 körpüsü (pacs.008/pacs.009). Hər bir nümunə işləyir
nəşr edilmiş `@iroha/iroha-js` paketinə qarşı və parçaları əks etdirir
`docs/source/sdk/js/governance_iso_examples.md`.

## İdarəetmə köməkçisi nümunəsi

<Nümunə Yüklə
  href="/sdk-recipes/javascript/governance.mjs"
  fayl adı = "governance.mjs"
  description="Bu reseptdə istinad edilən icra edilə bilən idarəetmə köməkçisini yükləyin."
/>

### İlkin şərtlər

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

İmzalanmış əməliyyatları Torii-ə təqdim etmək üçün `GOV_SUBMIT=1` seçin və
`GOV_FETCH=1` təqdim edildikdən sonra yaranan idarəetmə vəziyyətini yoxlamaq üçün.

### Nümunə skript

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

### Çalışın və nəzarət edin

- Yalnız hash yaratmaq üçün `node governance.mjs`-i yerinə yetirin. `GOV_SUBMIT=1` əlavə edin
  canlı idarəetmə vəziyyətini qeyd etmək üçün əməliyyatları Torii və `GOV_FETCH=1`-ə göndərin
  (`getGovernanceProposal*`, `getGovernanceReferendum`, `getGovernanceLocks` və
  `getGovernanceCouncilCurrent`).
- CI qeydlərində deterministik hashləri çəkin; hər addım imzalanmış baytı çap edir
  uzunluq və əlavə yerli köməkçi mövcud olduqda yenidən hesablanmış hash.
- Auditorların izləyə bilməsi üçün konsol çıxışını idarəetmənin nəzərdən keçirilməsi paketlərinə əlavə edin
  Təklif/referendum identifikatorları təkrarlana bilən CLI sübutlarına qaytarılır.

## ISO körpü nümunəsi

<Nümunə Yüklə
  href="/sdk-recipes/javascript/iso-bridge.mjs"
  fayl adı = "iso-bridge.mjs"
  description="Bu reseptdə istinad edilən işlək ISO 20022 köməkçisini endirin."
/>

### İlkin şərtlər

```bash
npm install @iroha/iroha-js
export TORII_URL="https://torii.nexus.example"
# optional overrides for the wait strategy
export ISO_POLL_ATTEMPTS=24
export ISO_POLL_INTERVAL_MS=1500
# use ISO_MESSAGE_KIND=pacs.009 for PvP submissions
export ISO_MESSAGE_KIND=pacs.008
```

Təqdimatı atlamaq və yalnız məlum olanı sorğulamaq istədiyiniz zaman `ISO_MESSAGE_ID` təyin edin
identifikator. Körpü işarələyən kimi çıxmaq üçün `ISO_RESOLVE_ON_ACCEPTED=1` istifadə edin
tranzaksiya hash hələ yekunlaşdırılmamış olsa belə, `Accepted` girişi.

### Nümunə skript

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

### Çalışın və nəzarət edin

- Nümunə yükü təqdim etmək üçün `node iso-bridge.mjs`-i yerinə yetirin. Set
  PvP axınını həyata keçirmək üçün `ISO_MESSAGE_KIND=pacs.009` və ya `ISO_MESSAGE_ID`
  mövcud təqdimatı yenidən yerləşdirmədən sorğu keçirin.
- Köməkçi `wait.onPoll` vasitəsilə hər sorğu cəhdini qeyd edir və bunu asanlaşdırır
  CI qeydlərində qəbul müddətlərini qeyd edin.
- Auditorlar üçün son statusu + əməliyyat hashını ISO körpü runbook-larına əlavə edin
  pacs.008/pacs.009 çatdırılmalarını təkrar istehsal olunan faydalı yüklərə qədər izləyə bilər.
  JS5 yol xəritəsinin çatdırılması tələb olunur.

## Oflayn müavinətlər və köçürmələr

`@iroha/iroha-js` sənəddə istinad edilən eyni müavinət/köçürmə köməkçilərini göndərir.
oflayn yol xəritəsi sətirləri. Dürüstlük siyasətlərini yoxlamaq üçün onlardan istifadə edin (marker açarı, Play
Bütövlük, HMS Safety Detect, Provisioned) xam metadatanı təhlil etmədən:

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

Torii Təmin edilmiş müavinət haqqında məlumat verdikdə, müfəttiş açıq açarı açıqlayır
şema, isteğe bağlı versiya, TTL və həzm altında yaşayır
`integrity_metadata.provisioned`, lazım olanı əlavə etməyi mənasız edir
OA10.3 sübut paketlərinə metadata.

## Növbəti addımlar

- `javascript/iroha_js/recipes/governance.mjs` və kəşf edin
  Genişləndirilmiş nümunələr üçün `javascript/iroha_js/recipes/iso_bridge.mjs` (çox işarəli
  seçki bülletenləri, şuranın VRF-dən əldə edilməsi, təkrar cəhd siyasətləri).
- Norito tərəfindəki sənədləri nəzərdən keçirin
  `docs/source/sdk/js/governance_iso_examples.md` və
  Kanonik sahə üçün `docs/source/finance/settlement_iso_mapping.md`
  bu köməkçilərin istinad etdiyi xəritələr.
- İş qeydlərini götürün və onları təmin etmək üçün idarəetmə / ISO təsdiqlərinə əlavə edin
  `roadmap.md`-də istinad edilən JS5 “sənədləşdirmə + nəşriyyat” tələbi.