---
lang: hy
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

Այս դաշտային ուղեցույցը ընդլայնվում է արագ մեկնարկի վրա՝ ցուցադրելով կառավարում և
ISO 20022 կամուրջը հոսում է `@iroha/iroha-js`-ով: Հատվածները կրկին օգտագործվում են նույնը
Գործարկման ժամանակի օգնականներ, որոնք առաքվում են `ToriiClient`-ով, այնպես որ կարող եք դրանք պատճենել անմիջապես
CLI գործիքավորում, CI ամրագոտիներ կամ երկարաժամկետ ծառայություններ:

Լրացուցիչ ռեսուրսներ.

- `javascript/iroha_js/recipes/governance.mjs` - գործարկվող վերջից մինչև վերջ սկրիպտ
  առաջարկներ, քվեաթերթիկներ և ավագանու ռոտացիաներ։
- `javascript/iroha_js/recipes/iso_bridge.mjs` — CLI օգնական՝ ներկայացնելու համար
  pacs.008/pacs.009 payloads and polling deterministic status.
- `docs/source/finance/settlement_iso_mapping.md` — կանոնական ISO դաշտի քարտեզագրում:

## Գործարկում ենք փաթեթավորված բաղադրատոմսերը

Այս օրինակները կախված են `javascript/iroha_js/recipes/`-ի սկրիպտներից: Վազիր
`npm install && npm run build:native` նախապես, այնպես որ առաջացած կապերը լինեն
հասանելի.

### Կառավարման օգնական քայլարշավ

Կազմաձևեք հետևյալ միջավայրի փոփոխականները նախքան կանչելը
`recipes/governance.mjs`:

- `TORII_URL` — Torii վերջնակետ:
- `AUTHORITY` / `PRIVATE_KEY_HEX` — ստորագրողի հաշիվ և բանալի (վեցանկյուն): Պահպանեք բանալիները a
  անվտանգ գաղտնի խանութ.
- `CHAIN_ID` — կամընտիր ցանցի նույնացուցիչ:
- `GOV_SUBMIT=1` — առաջացած գործարքները մղեք դեպի Torii:
- `GOV_FETCH=1` — բեռնեք առաջարկները/կողպեքները ներկայացնելուց հետո:
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — օգտագործված ընտրովի որոնումներ
  երբ `GOV_FETCH=1`.

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

Հեշերը գրանցվում են յուրաքանչյուր քայլի համար, և Torii պատասխանները հայտնվում են, երբ
`GOV_SUBMIT=1`, որպեսզի CI-ի աշխատանքները կարող են արագ ձախողվել ներկայացման սխալների դեպքում:

### ISO կամուրջի օգնական

`recipes/iso_bridge.mjs`-ը ներկայացնում է կամ pacs.008 կամ pacs.009 հաղորդագրություն և հարցումներ
ISO կամուրջը մինչև կարգավիճակի կարգավորումը: Կարգավորեք այն հետևյալով.

- `TORII_URL` — Torii վերջնակետ, որը բացահայտում է ISO կամուրջի API-ները:
- `ISO_MESSAGE_KIND` — `pacs.008` (կանխադրված) կամ `pacs.009`: Օգնականն օգտագործում է
  համապատասխան նմուշի կառուցող (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  երբ դուք չեք տրամադրում ձեր սեփական XML-ը:
- `ISO_MESSAGE_SUFFIX` — կամընտիր վերջածանց, որը կցվում է օգտակար բեռի նմուշի ID-ներին՝
  կրկնվող փորձերը եզակի պահեք (կանխադրված է ընթացիկ դարաշրջանի վայրկյանները վեցանկյունով):
- `ISO_CONTENT_TYPE` — փոխարինել `Content-Type` վերնագիրը ներկայացումների համար
  (օրինակ `application/pacs009+xml`); անտեսվում է, երբ դուք միայն հարցում եք անում
  գոյություն ունեցող հաղորդագրության ID.
- `ISO_MESSAGE_ID` — բաց թողեք ներկայացումը և միայն հարցրեք տրամադրվածը
  նույնացուցիչը `waitForIsoMessageStatus`-ի միջոցով:
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — կարգավորեք սպասման ռազմավարությունը
  աղմկոտ կամ դանդաղ կամուրջների տեղակայում:
- `ISO_RESOLVE_ON_ACCEPTED=1` — դուրս գալ, հենց որ Torii-ը վերադարձնի `Accepted`,
  նույնիսկ եթե գործարքի հեշը դեռ առկախ է (հարմար է կամրջի պահպանման ժամանակ
  երբ հաշվապահական հաշվառման կատարումը հետաձգվում է):

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

Երկու սկրիպտներն էլ դուրս են գալիս `1` կարգավիճակի կոդով, եթե Torii երբեք չի հայտնում տերմինալի մասին
անցում, դարձնելով դրանք հարմար CI դարպասների աշխատանքների համար:

### ISO alias օգնական

`recipes/iso_alias.mjs`-ը թիրախավորում է ISO կեղծանունների վերջնակետերը, որպեսզի փորձերը կարողանան ծածկել
կուրացած տարրերի հեշավորում և այլանունների որոնումներ՝ առանց պատվերով գործիքներ գրելու: Այն
զանգեր `ToriiClient.evaluateAliasVoprf` գումարած `resolveAlias` / `resolveAliasByIndex`
և տպում է հետնամասը, digest-ը, հաշվի կապը, աղբյուրը և դետերմինիստական ինդեքսը
վերադարձված Torii-ի կողմից:

Շրջակա միջավայրի փոփոխականներ.

- `TORII_URL` — Torii վերջնակետը, որը բացահայտում է այլանունների օգնականները:
- `ISO_VOPRF_INPUT` - վեցանկյուն կոդավորված կույր տարր (կանխադրված է `deadbeef`):
- `ISO_SKIP_VOPRF=1` — բաց թողեք VOPRF զանգը միայն որոնումների փորձարկման ժամանակ:
- `ISO_ALIAS_LABEL` — բառացի այլանուններ՝ լուծելու համար (օրինակ՝ IBAN ոճի տողեր):
- `ISO_ALIAS_INDEX` — տասնորդական կամ `0x` նախածանցով ինդեքսը փոխանցվել է `resolveAliasByIndex`-ին:
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — կամընտիր վերնագրեր ապահովված Torii տեղադրումների համար:

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

Օգնականը արտացոլում է Torii-ի պահվածքը. այն հայտնվում է 404 վրկ, երբ կեղծանունները բացակայում են:
և գործարկման ժամանակ անջատված սխալները վերաբերվում են որպես մեղմ բացթողումներ, որպեսզի CI հոսքերը կարողանան հանդուրժել կամուրջը
սպասարկման պատուհաններ.

## Կառավարման աշխատանքային հոսքեր

### Ստուգեք պայմանագրերի օրինակները և առաջարկները

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

### Ներկայացրե՛ք առաջարկներ և քվեաթերթիկներ

Օգտագործեք `AbortController`, երբ ձեզ անհրաժեշտ է չեղարկել կամ ժամանակին սահմանափակված կառավարման ներկայացումները՝ SDK-ն
ընդունում է կամընտիր `{ signal }` օբյեկտ յուրաքանչյուր POST օգնականի համար, որը ներկայացված է ստորև:

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

### Խորհրդի VRF և ուժի մեջ մտնելը

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

## ISO 20022 կամուրջի բաղադրատոմսեր

### Կառուցեք pacs.008 / pacs.009 payloads

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

Բոլոր նույնացուցիչները (BIC, LEI, IBAN, ISO գումարը) վավերացված են մինչև XML-ը
առաջացած. Փոխեք `buildPacs008Message`-ը `buildPacs009Message`-ի հետ՝ PvP արտանետելու համար
ֆինանսավորման օգտակար բեռներ.

### Ներկայացրե՛ք և հարցում կատարե՛ք ISO հաղորդագրություններ

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

Երկուսն էլ՝ `resolveOnAccepted` և `resolveOnAcceptedWithoutTransaction` վավեր են. օգտագործել ցանկացած դրոշ
հարցումները կազմակերպելիս `Accepted` կարգավիճակները (առանց գործարքի հեշի) վերաբերվել որպես տերմինալ:

Օգնողները նետում են `IsoMessageTimeoutError`, եթե կամուրջը երբեք չի հայտնում ա
տերմինալային վիճակ. Օգտագործեք ցածր մակարդակի `submitIsoPacs008` / `submitIsoPacs009`
զանգեր, երբ ձեզ անհրաժեշտ է կազմակերպել մաքսային քվեարկության տրամաբանությունը. `getIsoMessageStatus`
բացահայտում է մեկ կրակոցի որոնում:

### Հարակից մակերեսներ

- `torii.getSorafsPorWeeklyReport("2026-W05")`-ը վերցնում է ISO շաբաթվա PoR փաթեթը
  նշված է ճանապարհային քարտեզում և կարող է կրկին օգտագործել սպասող օգնականները ահազանգերի համար:
- `resolveAlias` / `resolveAliasByIndex` բացահայտում է ISO կամուրջի կեղծանունների կապերը, որպեսզի
  հաշտեցման գործիքները կարող են ապացուցել հաշվի սեփականության իրավունքը՝ նախքան վճարում տրամադրելը: