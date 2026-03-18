---
slug: /sdks/javascript/governance-iso-examples
lang: am
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ይህ የመስክ መመሪያ አስተዳደርን በማሳየት ፈጣን ጅምር ላይ ያሰፋዋል።
ISO 20022 ድልድይ ከ`@iroha/iroha-js` ጋር ይፈስሳል። ቁርጥራጮቹ እንደገና ተመሳሳይ ናቸው።
በ`ToriiClient` የሚላኩ የሩጫ ጊዜ ረዳቶች፣ ስለዚህ በቀጥታ ወደ ውስጥ መቅዳት ይችላሉ።
የ CLI መሣሪያ፣ CI ቋጠሮዎች፣ ወይም ለረጅም ጊዜ የሚሰሩ አገልግሎቶች።

ተጨማሪ ግብዓቶች፡-

- `javascript/iroha_js/recipes/governance.mjs` - ከጫፍ እስከ ጫፍ የሚሄድ ስክሪፕት
  የውሳኔ ሃሳቦች፣ የምርጫ ካርዶች እና የምክር ቤት ሽክርክሮች።
- `javascript/iroha_js/recipes/iso_bridge.mjs` - ለማስገባት CLI አጋዥ
  pacs.008/pacs.009 የሚጫኑ ጭነቶች እና የድምጽ መስጫ ሁኔታ ሁኔታ።
- `docs/source/finance/settlement_iso_mapping.md` - ቀኖናዊ ISO የመስክ ካርታ።

## የተጣመሩ የምግብ አዘገጃጀቶችን በማስኬድ ላይ

እነዚህ ምሳሌዎች በ `javascript/iroha_js/recipes/` ውስጥ ባሉ ስክሪፕቶች ላይ ይወሰናሉ። ሩጡ
`npm install && npm run build:native` አስቀድሞ ስለዚህ የተፈጠሩት ማሰሪያዎች ናቸው።
ይገኛል ።

### የአስተዳደር አጋዥ አካሄድ

ከመጥራትዎ በፊት የሚከተሉትን የአካባቢ ተለዋዋጮች ያዋቅሩ
`recipes/governance.mjs`፡

- `TORII_URL` - I18NT0000000X የመጨረሻ ነጥብ።
- `AUTHORITY` / I18NI0000028X - የፈራሚ መለያ እና ቁልፍ (ሄክስ)። ቁልፎችን በ ሀ
  ደህንነቱ የተጠበቀ ሚስጥራዊ መደብር።
- `CHAIN_ID` - አማራጭ የአውታረ መረብ መለያ።
- `GOV_SUBMIT=1` - የተፈጠሩትን ግብይቶች ወደ Torii ይግፉ።
- `GOV_FETCH=1` - ከቀረቡ በኋላ የውሳኔ ሃሳቦችን/መቆለፊያዎችን አምጡ።
- `GOV_PROPOSAL_ID`፣ `GOV_REFERENDUM_ID`፣ `GOV_LOCKS_ID` — አማራጭ ፍለጋዎች ጥቅም ላይ ይውላሉ
  መቼ `GOV_FETCH=1`.

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

Hashes ለእያንዳንዱ እርምጃ ገብተዋል፣ እና የTorii ምላሾች ሲታዩ ይታያሉ።
`GOV_SUBMIT=1` ስለዚህ CI ስራዎች የማስረከቢያ ስህተቶች ላይ በፍጥነት ሊወድቁ ይችላሉ.

### የ ISO ድልድይ አጋዥ

`recipes/iso_bridge.mjs` ወይ pacs.008 ወይም pacs.009 መልእክት እና ምርጫዎችን ያቀርባል
ሁኔታው እስኪረጋጋ ድረስ የ ISO ድልድይ. አዋቅር በ፡

- `TORII_URL` — Torii የ ISO ድልድይ ኤፒአይዎችን የሚያጋልጥ የመጨረሻ ነጥብ።
- `ISO_MESSAGE_KIND` — `pacs.008` (ነባሪ) ወይም `pacs.009`። ረዳቱ ይጠቀማል
  ተዛማጅ ናሙና ገንቢ (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  የራስዎን ኤክስኤምኤል በማይሰጡበት ጊዜ.
- `ISO_MESSAGE_SUFFIX` - አማራጭ ቅጥያ ከናሙና የመጫኛ መታወቂያዎች ጋር ተያይዟል
  ተደጋጋሚ ልምምዶችን ልዩ ያድርጉ (ነባሪዎች አሁን ላለው የኢፖክ ሰከንድ በሄክስ)።
- `ISO_CONTENT_TYPE` - ለመቅረቡ የ I18NI0000046X ራስጌን ይሽሩ
  (ለምሳሌ `application/pacs009+xml`); አስተያየት ሲሰጡ ችላ ተብለዋል።
  ነባር የመልእክት መታወቂያ
- `ISO_MESSAGE_ID` - በአጠቃላይ ማስረከብን ይዝለሉ እና የቀረበውን አስተያየት ይስጡ
  መለያ በI18NI0000049X።
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` - የጥበቃ ስልቱን ማስተካከል ለ
  ጫጫታ ወይም ዘገምተኛ ድልድይ ማሰማራት.
- `ISO_RESOLVE_ON_ACCEPTED=1` - Torii `Accepted` እንደተመለሰ ውጣ፣
  ምንም እንኳን የግብይቱ ሃሽ አሁንም በመጠባበቅ ላይ ቢሆንም (በድልድይ ጥገና ወቅት ምቹ
  የሂሳብ ደብተር ሲዘገይ).

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

Torii መቼም ተርሚናል ካላሳወቀ ሁለቱም ስክሪፕቶች በሁኔታ ኮድ `1` ይወጣሉ
ሽግግር, ለ CI በር ስራዎች ተስማሚ ያደርጋቸዋል.

### ISO ተለዋጭ ስም አጋዥ

`recipes/iso_alias.mjs` ልምምዶች መሸፈን እንዲችሉ የ ISO ተለዋጭ ስም የመጨረሻ ነጥቦችን ያነጣጥራል።
ዓይነ ስውር-ኤለመንት ሀሺንግ እና ቅጽል ምልልሶች የቃል መሣሪያን ሳይጽፉ። እሱ
`ToriiClient.evaluateAliasVoprf` እና `resolveAlias` / `resolveAliasByIndex` ይደውላል
እና የጀርባውን፣ የመፍጨት ሂደቱን፣ የመለያ ማሰሪያውን፣ ምንጩን እና የመወሰን መረጃን ያትማል
በ Torii ተመልሷል።

የአካባቢ ተለዋዋጮች፡-

- `TORII_URL` — Torii ቅጽል ረዳቶችን የሚያጋልጥ የመጨረሻ ነጥብ።
- `ISO_VOPRF_INPUT` — ሄክስ-የተመሰጠረ ዓይነ ስውር ኤለመንት (የ`deadbeef` ነባሪዎች)።
- `ISO_SKIP_VOPRF=1` — ፍለጋዎችን ሲሞክሩ የVOPRF ጥሪን ይዝለሉ።
- `ISO_ALIAS_LABEL` - ለመፍታት ቀጥተኛ ተለዋጭ ስም (ለምሳሌ፣ IBAN-style strings)።
- `ISO_ALIAS_INDEX` — አስርዮሽ ወይም I18NI0000065X-ቅድመ-ቅጥያ ኢንዴክስ ወደ `resolveAliasByIndex` አልፏል።
- `TORII_AUTH_TOKEN` / I18NI0000068X - ለደህንነታቸው የተጠበቁ Torii ማሰማራቶች አማራጭ ራስጌዎች።

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

ረዳቱ የI18NT0000009X ባህሪን ያንጸባርቃል፡ ተለዋጭ ስሞች ሲጠፉ 404s ላይ ይገለጣል
እና የ CI ፍሰቶች ድልድይ መቋቋም እንዲችሉ የሩጫ ጊዜ-አካል ጉዳተኞች ስህተቶችን ለስላሳ መዝለሎች ይመለከታል
የጥገና መስኮቶች.

## የአስተዳደር የስራ ሂደቶች

### የኮንትራት ሁኔታዎችን እና ሀሳቦችን ይፈትሹ

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

### ፕሮፖዛል እና ድምጽ ይሰጡ

በጊዜ የተገደበ የአስተዳደር ማቅረቢያዎችን መሰረዝ ሲፈልጉ `AbortController` ይጠቀሙ - ኤስዲኬ
ከታች ለሚታየው ለእያንዳንዱ የPOST አጋዥ አማራጭ I18NI0000070X ነገር ይቀበላል።

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

const zkOwner = "i105..."; // canonical I105 account id for ZK public inputs
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

### ካውንስል VRF እና አፈጻጸም

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

## ISO 20022 ድልድይ አዘገጃጀት

### ገንቡ pacs.008 / pacs.009 የሚጫኑ ጭነቶች

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

ሁሉም ለዪዎች (BIC፣ LEI፣ IBAN፣ ISO መጠን) ኤክስኤምኤል ከመሆኑ በፊት የተረጋገጡ ናቸው።
የተፈጠረ. PvP ለመልቀቅ `buildPacs008Message` ለ`buildPacs009Message` ቀይር
የገንዘብ ጭነቶች.

### የ ISO መልዕክቶችን ያስገቡ እና ድምጽ ይስጡ

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

ሁለቱም `resolveOnAccepted` እና I18NI0000074X ልክ ናቸው; ወይ ባንዲራ ይጠቀሙ
ምርጫዎችን ሲያቀናብሩ `Accepted` ሁኔታዎችን (ያለ የግብይት ሃሽ) እንደ ተርሚናል ለማከም።

ድልድዩ መቼም ሪፖርት ካላደረገ ረዳቶቹ I18NI0000076X ይጥላሉ
ተርሚናል ሁኔታ. ዝቅተኛ-ደረጃ `submitIsoPacs008`/I18NI0000078X ይጠቀሙ
ብጁ የምርጫ አመክንዮ ማቀናበር ሲፈልጉ ይደውላል; `getIsoMessageStatus`
ነጠላ-ምት ፍለጋን ያጋልጣል።

### ተዛማጅ ገጽታዎች

- `torii.getSorafsPorWeeklyReport("2026-W05")` የ ISO-ሳምንት PoR ጥቅልን ያመጣል
  በፍኖተ ካርታው ውስጥ ተጠቅሷል እና የጥበቃ አጋዥዎችን ለማንቂያዎች እንደገና መጠቀም ይችላል።
- `resolveAlias` / `resolveAliasByIndex` የ ISO ድልድይ ቅጽል ማያያዣዎችን ያጋልጣል
  የማስታረቂያ መሳሪያዎች ክፍያ ከመሰጠቱ በፊት የመለያ ባለቤትነትን ሊያረጋግጡ ይችላሉ.