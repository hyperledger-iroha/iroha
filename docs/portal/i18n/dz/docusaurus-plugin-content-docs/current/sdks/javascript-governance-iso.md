---
slug: /sdks/javascript/governance-iso-examples
lang: dz
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

འ་ནི་ས་ཁོངས་ལམ་སྟོན་འདི་ གཞུང་སྐྱོང་དང་ མགྱོགས་དྲགས་འགོ་བཙུགས་མི་ལུ་ རྒྱ་སྐྱེད་འབདཝ་ཨིན།
ISO 20022 ཟམ་གྱི་རྒྱུན་འབབ་ `@iroha/iroha-js`. ཕྲ་རིང་ཚུ་གིས་ དེ་བཟུམ་ལོག་ལག་ལེན་འཐབ།
གྲུ་དེ་ `ToriiClient` ཡོད་པའི་གྲུ་དེ་ རན་ཊའིམ་ གཡོག་བཀོལ་ནི་ དེ་འབདཝ་ལས་ ཁྱོད་ཀྱིས་ ཐད་ཀར་དུ་ འདྲ་བཤུས་རྐྱབ་ཚུགས།
CLI ལག་ཆས་དང་ CI harness ཡང་ན་ ཡུན་རིང་རྒྱུག་པའི་ཞབས་ཏོག་ཚུ་ཨིན།

ཐོན་ཁུངས་ཁ་སྐོང་:

- I18NI000000020X — དོན་ལུ་ གཡོག་བཀོལ་ཚུགས་པའི་ མཇུག་ལས་མཇུག་ཚུན་ཚོད་ ཡིག་ཚུགས་བཀོད།
  གྲོས་འཆར་དང་ཚོགས་རྒྱན་ དེ་ལས་ ཚོགས་སྡེའི་འཁོར་སྐྱོད་ཚུ།
- I18NI000000021X — བཙུགས་ནིའི་དོན་ལུ་ CLI གྲོགས་རམ་པ།
  pacs.008/pacs.009 གླ་ཆ་དང་ འོས་བསྡུའི་གཏན་འབེབས་གནས་རིམ།
- `docs/source/finance/settlement_iso_mapping.md` — ཀེ་ནོ་ནིག་ཨའི་ཨེསི་ཨོ་ས་སྒོ་སབ་ཁྲ་བཟོ་ནི།

## བརྩེགས་པའི་བཟའ་ཐབས་བརྒྱུགས་པ།

དཔེ་འདི་ཚུ་ `javascript/iroha_js/recipes/` ནང་ལུ་ཡོད་པའི་ཡིག་གཟུགས་ཚུ་ལུ་རག་ལསཔ་ཨིན། རྒྱུག༌ནི
I18NI000000024X འདི་ཧེ་མ་ལས་བཏོན་ཡོདཔ་ལས་ བཟོ་བཏོན་འབད་ཡོད་པའི་བཱའིན་ཌིང་ཚུ་ཨིན།
ཡོད༌པ།

### གཞུང་སྐྱོང་རོགས་སྐྱོར་འབད་མི།

འབོད་བརྡ་མ་འབད་བའི་ཧེ་མ་ འོག་གི་མཐའ་འཁོར་འགྱུར་ཅན་ཚུ་རིམ་སྒྲིག་འབད།
`recipes/governance.mjs`:

- `TORII_URL` — Torii མཇུག་བསྡུ།
- I18NI000000027X / `PRIVATE_KEY_HEX` — མཚན་རྟགས་བཀོད་མི་རྩིས་ཐོ་དང་ལྡེ་མིག་ (hex)། ལྡེ་མིག་ཚུ་ ༡ ལུ་བཞག་དགོ།
  གསང་བའི་ཚོང་ཁང་ཉེན་སྲུང་།
- `CHAIN_ID` — གདམ་ཁའི་ཡོངས་འབྲེལ་ངོས་འཛིན་འབད་མི།
- I18NI000000030X — བཟོ་བཏོན་འབད་ཡོད་པའི་ཚོང་འབྲེལ་ཚུ་ Torii ལུ་ཨེབ་གཏང་།
- `GOV_FETCH=1` — བཙུགས་པའི་ཤུལ་ལས་ གྲོས་འཆར་/ལྡེ་མིག་ཚུ་ལེན་དགོ།
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, I18NI000000034X — གདམ་ཁའི་འཚོལ་ཞིབ་ཚུ་ལག་ལེན་འཐབ་ཡོདཔ།
  ག་དུས་ `GOV_FETCH=1`.

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=<i105-account-id> \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

གོ་རིམ་རེ་རེ་གི་དོན་ལུ་ ཧ་ཤི་ཚུ་ ནང་བསྐྱོད་འབད་ཡོདཔ་དང་ I18NT0000002X ལན་ཚུ་ ག་དེམ་ཅིག་སྦེ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
`GOV_SUBMIT=1` དེ་འབདཝ་ལས་ CI ལཱ་ཚུ་ ཕུལ་བའི་འཛོལ་བ་ཚུ་ལུ་ མགྱོགས་དྲགས་སྦེ་ འཐུས་ཤོར་འགྱོ་འོང་།

### ISO ཟམ་པ་རོགས་སྐྱོར།

I18NI000000037X གང་རུང་གིས་ pacs.008 ཡང་ན་ pacs.009 འཕྲིན་དོན་དང་འོས་བསྡུ།
གནས་ཚད་འདི་གཞི་སྒྲིག་མ་འབད་ཚུན་ཚོད་ ཨའི་ཨེསི་ཨོ་ཟམ་འདི་ཨིན། འདི་དང་གཅིག་ཁར་རིམ་སྒྲིག་འབད།

- I18NI000000038X — ISO ཟམ་པའི་ཨེ་པི་ཨའི་ཚུ་ཕྱིར་བཏོན་འབད་བའི་ Torii མཇུག་བསྡུ།
- `ISO_MESSAGE_KIND` — I18NI0000004X (སྔོན་སྒྲིག་) ཡང་ན་ I18NI00000041. གྲོགས་རམ་པ་དེ་གིས་ ལག་ལེན་འཐབ་ཨིན།
  མཐུན་སྒྲིག་དཔེ་ཚད་བཟོ་མི་ (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  ཁྱོད་རའི་ XML བཀྲམ་སྤེལ་མ་འབད་བའི་སྐབས།
- `ISO_MESSAGE_SUFFIX` — དཔེ་ཚད་པེ་ལོཌི་ཨའི་ཌི་ཚུ་ལུ་ གདམ་ཁའི་རྗེས་འཇུག་ མཉམ་སྦྲགས་འབད་ཡོདཔ་ཨིན།
  བསྐྱར་ལོག་སྦྱོང་བརྡར་ཚུ་ གཞན་དང་མ་འདྲཝ་སྦེ་བཞག། (ད་ལྟོའི་དུས་སྐབས་ཀྱི་སྐར་ཆ་ཚུ་ལུ་ ཧེགསི་ནང་ སྔོན་སྒྲིག་འབད།)
- `ISO_CONTENT_TYPE` — བཙུགས་ནིའི་དོན་ལུ་ `Content-Type` མགོ་ཡིག་འདི་ བཀག་ཆ་འབད་ཡོདཔ།
  (དཔེར་ན་ I18NI0000004X); ཁྱོད་ཀྱིས་ འོས་བསྡུའི་ནང་རྐྱངམ་ཅིག་ འོས་འདེམས་འབད་བའི་སྐབས་ སྣང་མེད་བཞག་ཡོདཔ།
  ད་ལྟོ་ཡོད་པའི་འཕྲིན་དོན་ཨའི་ཌི།
- `ISO_MESSAGE_ID` — གཡོ་སྒྱུ་ཕུལ་ཏེ་ འོས་འདེམས་རྐྱངམ་ཅིག་ བཀྲམ་སྤེལ་འབདཝ་ཨིན།
  ངོས་འཛིན་འབད་མི་བརྒྱུད་དེ་ `waitForIsoMessageStatus`.
- I18NI000000050X / `ISO_POLL_INTERVAL_MS` — བསྒུག་ཐབས་ཀྱི་ཐབས་ཤེས་འདི་ ༢༠༡༧
  སྒྲ་སྐད་ཡང་ན་ ཟམ་བཀྲམ་སྤེལ་ཚུ།
- `ISO_RESOLVE_ON_ACCEPTED=1` — Torii སླར་ལོག་འབད་བའི་སྐབས་ I18NI0000000053X,
  གལ་ཏེ་ བརྗེ་སོར་གྱི་ཧ་ཤི་འདི་ད་ལྟོ་ཡང་མཇུག་བསྡུ་སྟེ་ཡོད་རུང་ (ཟམ་བདག་འཛིན་འཐབ་པའི་སྐབས་ལགཔ་ལགཔ་གིས་)།
  རྩིས་ཁྲ་འདི་ ཕྱིར་འགྱངས་འབད་ཡོད་པའི་སྐབས་)།

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

གལ་སྲིད་ I18NT000000005X གིས་ ཊར་མི་ནཱལ་འདི་ ནམ་ཡང་སྙན་ཞུ་མ་འབད་བ་ཅིན་ གནས་ཚད་ཨང་རྟགས་ I18NI000000054X དང་ཅིག་ཁར་ ཕྱིར་འཐོན་འབདཝ་ཨིན།
འགྱུར་བ་འདི་ CI གི་སྒོ་རའི་དོན་ལུ་འོས་འབབ་ཅན་བཟོཝ་ཨིན།

### ISO མིང་གཞན་རོགས་སྐྱོར།

I18NI0000005X གིས་ ISO alias མཐའ་མཚམས་ལུ་དམིགས་གཏད་བསྐྱེདཔ་ལས་ བསྐྱར་སྦྱོང་གིས་ ཁྱབ་ཚུགས།
མིག་མེད་བཟོ་བའི་ཧ་ཤིང་དང་ མིང་གཞན་བལྟ་སྟངས་ཚུ་ བེ་སི་པོཀ་ལག་ཆས་ཚུ་ མ་བྲིས་པས། འདི
འབོད་བརྡ་ `ToriiClient.evaluateAliasVoprf` དང་ `resolveAlias` / `resolveAliasByIndex` ཟེར་སླབ་ཨིན།
དེ་ལས་ རྒྱབ་ཐག་དང་ བཞུ་བཅོས་ རྩིས་ཐོ་ མཐུད་བྱེད་འབྱུང་ཁུངས་ དེ་ལས་ གཏན་འབེབས་ཟུར་ཐོ་ཚུ་ དཔར་བསྐྲུན་འབདཝ་ཨིན།
Torii གིས་ལོག་འོང་ཡོདཔ།

ཁོར་ཡུག་འགྱུར་ལྡོག་ཅན།

- I18NI000000059X — Torii མཐའ་མའི་གྲོགས་རམ་པ་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།
- I18NI0000000060X — ཧེགསི་ཨེན་ཀོཌ་མེད་པའི་ཆ་ཤས་ (I18NI000000061X ལུ་སྔོན་སྒྲིག་)།
- I18NI000000062X — བརྟག་ཞིབ་ཚུ་རྐྱངམ་ཅིག་འབད་བའི་སྐབས་ VOPRF འབོད་བརྡ་འདི་ གོམ་འགྱོཝ་ཨིན།
- I18NI000000063X — ཐག་གཅོད་འབད་ནི་ལུ་ ངོ་མ་མིང་གཞན་ (དཔེར་ན་ IBAN-style ཡིག་རྒྱུན་ཚུ་)།
- I18NI000000064X — ཚག་ཡང་ན་ `0x`-སྔོན་སྒྲིག་ཟུར་ཐོ་ I18NI000000066X ལུ་སྤྲོད་ཡོདཔ་ཨིན།
- I18NI0000000067X / `TORII_API_TOKEN` — བདེ་སྲུང་དོན་ལུ་ གདམ་ཁ་ཅན་གྱི་མགོ་ཡིག་ I18NT0000008X བཀྲམ་སྤེལ་ཚུ།

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

གྲོགས་རམ་པ་འདི་གིས་ Torii’s གི་སྤྱོད་ལམ་འདི་ མེ་ལོང་: འདི་གིས་ མིང་གཞན་མེད་པའི་སྐབས་ 404s ཕྱིར་ཐོན་འབདཝ་ཨིན།
དང་ རན་ཊའིམ་ལྕོགས་མེད་ཀྱི་འཛོལ་བ་ཚུ་ མཉེན་པའི་གོམ་པ་སྦེ་ བརྩི་འཇོག་འབདཝ་ལས་ སི་ཨའི་ རྒྱུན་འབབ་ཀྱིས་ ཟམ་བཟོད་བསྲན་འབད་ཚུགས།
བདག་འཛིན་སྒོ་སྒྲིག་ཚུ།

## གཞུང་སྐྱོང་གི་ལས་རིམ།

### གན་འཛིན་གྱི་གནས་སྟངས་དང་གྲོས་འཆར།

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
  console.log(`${entry.contract_address} :: ${entry.code_hash_hex}`);
}

const controller = new AbortController();
const proposal = await torii.getGovernanceProposal("proposal-001", {
  signal: controller.signal,
});
console.log(proposal?.kind, proposal?.status);
```

### གྲོས་འཆར་དང་ ཚོགས་རྒྱན་ཚུ་ ཐོ་བཀོད་འབད།

ཁྱོད་ཀྱིས་ དུས་ཚོད་བཀག་ཆ་འབད་མི་ གཞུང་སྐྱོང་ཕུལ་མི་ཚུ་ ཆ་མེད་གཏང་དགོཔ་ད་ `AbortController` ལག་ལེན་འཐབ།
འོག་ལུ་སྟོན་ཡོད་མི་ པི་ཨོ་ཨེསི་ཊི་གྲོགས་རམ་རེ་རེ་གི་དོན་ལུ་ གདམ་ཁ་ཅན་གྱི་ `{ signal }` དངོས་པོ་ཅིག་ དང་ལེན་འབདཝ་ཨིན།

```ts
const authority = "<i105-account-id>";
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

const zkOwner = "<i105-account-id>"; // canonical I105 account id for ZK public inputs
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

### ལྷན་ཚོགས་ཝི་ཨར་ཨེཕ་དང་བརྩོན།

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "<i105-account-id>",
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

## ISO 20022 ཟམ་པའི་ཐབས་ལམ།

###

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
  creditorAccount: { otherId: "<i105-account-id>" },
  purposeCode: "SECU",
  supplementaryData: { account_id: "<i105-account-id>", leg: "delivery" },
});
```

ངོས་འཛིན་འབད་མི་ཆ་མཉམ་ (BIC, LEI, IBAN, ISO གྱངས་ཁ་) ཨེགསི་ཨེམ་ཨེལ་གྱི་ཧེ་མ་ བདེན་དཔྱད་འབད་ཡོདཔ་ཨིན།
བཟོ་བཏོན་འབད་ཡོདཔ། PvP བཏོན་གཏང་ནིའི་དོན་ལུ་ I18NI0000000000000071X གི་དོན་ལུ་ བརྗེ་སོར་འབད།
མ་དངུལ་སྤྲོད་ལེན་ཚུ།

### འོས་འཚམས་དང་ འོས་འཚམས་ཀྱི་འཕྲིན་ཡིག་བཙུགས་པ།

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

I18NI000000073X དང་ `resolveOnAcceptedWithoutTransaction` གཉིས་ཆ་ར་ ནུས་ཅན་ཨིན། དར་དར་གཉིས་ཆ་ར་ལག་ལེན་འཐབ།
`Accepted` གནས་རིམ་ཚུ་ འོས་བསྡུའི་སྒྲིག་བཀོད་འབད་བའི་སྐབས་ ཊར་མི་ནཱལ་སྦེ་ སྨན་བཅོས་འབད་ནི།

ཟམ་པ་དེ་གིས་ སྙན་ཞུ་མ་འབད་བ་ཅིན་ གྲོགས་རམ་འབད་མི་ཚུ་གིས་ I18NI000000076X བཀོ་བཞག་ཡོདཔ་ཨིན།
ཊར་མི་ནཱལ་གནས་སྟངས། དམའ་རིམ་གྱི་ `submitIsoPacs008` / I18NI000000078X ལག་ལེན་འཐབ།
ཁྱོད་ཀྱིས་ སྲོལ་སྒྲིག་འོས་བསྡུའི་ཚད་མ་འདི་ རིམ་སྒྲིག་འབད་དགོ་པའི་སྐབས་ལུ་ འབོད་བརྡ་འབདཝ་ཨིན། `getIsoMessageStatus`
པར་གཅིག་ལྟ་ཞིབ་ཅིག་གསལ་སྟོན་འབདཝ་ཨིན།

### འབྲེལ ་པའི་ངོས་འཛིན།

- I18NI000000080X ཨའི་ཨེསི་ཨོ་-བདུན་ཕྲག་ པོ་ཨར་ བང་སྒྲིག།
  ལམ་སྟོན་ནང་ལུ་གཞི་བསྟུན་འབད་དེ་ དྲན་སྐུལ་གྱི་དོན་ལུ་ བསྒུག་སྡོད་མི་གྲོགས་རམ་པ་ཚུ་ ལོག་ལག་ལེན་འཐབ་བཏུབ།
- I18NI000000081X / `resolveAliasByIndex` ISO ཟམ་པའི་མིང་ཚིག་ཚུ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
  མཐུན་སྒྲིག་ལག་ཆས་ཚུ་གིས་ ཏི་རུ་མ་སྤྲོད་པའི་ཧེ་མ་ རྩིས་ཁྲའི་བདག་དབང་འདི་ བདེན་ཁུངས་བཀལ་ཚུགས།