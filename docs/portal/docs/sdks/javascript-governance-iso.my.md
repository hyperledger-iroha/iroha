---
lang: my
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

ဤနယ်ပယ်လမ်းညွှန်သည် အုပ်ချုပ်ရေးနှင့် သရုပ်ပြခြင်းဖြင့် အမြန်စတင်ခြင်းတွင် ချဲ့ထွင်သည်။
ISO 20022 တံတားသည် `@iroha/iroha-js` ဖြင့် စီးဆင်းသည်။ အတိုအထွာများသည် အလားတူ ပြန်လည်အသုံးပြုသည်။
`ToriiClient` ဖြင့်ပေးပို့သော runtime helpers များဖြစ်သောကြောင့် ၎င်းတို့ကို တိုက်ရိုက်ကူးယူနိုင်ပါသည်။
CLI ကိရိယာတန်ဆာပလာများ၊ CI ကြိုးများ သို့မဟုတ် ကာလကြာရှည် ဝန်ဆောင်မှုများ။

နောက်ထပ်အရင်းအမြစ်များ-

- `javascript/iroha_js/recipes/governance.mjs` — အတွက် အဆုံးမှအဆုံးအထိ လုပ်ဆောင်နိုင်သော ဇာတ်ညွှန်း
  အဆိုများ၊ မဲများနှင့် ကောင်စီအလှည့်ကျများ။
- `javascript/iroha_js/recipes/iso_bridge.mjs` — တင်ပြရန်အတွက် CLI အကူအညီပေးသူ
  pacs.008/pacs.009 ပေးချေမှုများနှင့် မဲရုံများ၏ အဆုံးအဖြတ်အခြေအနေ။
- `docs/source/finance/settlement_iso_mapping.md` — canonical ISO အကွက်ပုံဖော်ခြင်း။

## ထုပ်ပိုးထားသော ချက်ပြုတ်နည်းများကို လုပ်ဆောင်ခြင်း။

ဤဥပမာများသည် `javascript/iroha_js/recipes/` ရှိ script များပေါ်တွင်မူတည်သည်။ ပြေး
`npm install && npm run build:native` သည် ကြိုတင်ထုတ်လုပ်ထားသော binding များဖြစ်သည်။
ရရှိနိုင်

### အုပ်ချုပ်မှုအထောက် အကူပြု လမ်းညွှန်ချက်

မခေါ်မီ အောက်ပါပတ်ဝန်းကျင် ကိန်းရှင်များကို ပြင်ဆင်ပါ။
`recipes/governance.mjs`-

- `TORII_URL` — Torii အဆုံးမှတ်။
- `AUTHORITY` / `PRIVATE_KEY_HEX` — လက်မှတ်ထိုးအကောင့်နှင့် သော့ (hex)။ သော့တစ်ချောင်းကို သိမ်းထားပါ။
  လုံခြုံသောလျှို့ဝှက်စတိုးဆိုင်။
- `CHAIN_ID` — ရွေးချယ်နိုင်သော ကွန်ရက်သတ်မှတ်စနစ်။
- `GOV_SUBMIT=1` — ထုတ်ပေးထားသော ငွေလွှဲမှုများကို Torii သို့ တွန်းပါ။
- `GOV_FETCH=1` — တင်ပြပြီးနောက် အဆိုပြုချက်များ/သော့ခလောက်များကို ရယူပါ။
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — ရွေးချယ်နိုင်သော ရှာဖွေမှုများကို အသုံးပြုထားသည်
  `GOV_FETCH=1` တုန်းက။

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=ih58... \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

အဆင့်တိုင်းအတွက် Hashes များကို မှတ်သားထားပြီး Torii တုံ့ပြန်မှုများကို ပေါ်လာသည့်အခါ
`GOV_SUBMIT=1` ထို့ကြောင့် CI အလုပ်များသည် တင်ပြမှုအမှားများတွင် လျင်မြန်စွာ ကျရှုံးနိုင်ပါသည်။

### ISO တံတားအကူ

`recipes/iso_bridge.mjs` သည် pacs.008 သို့မဟုတ် pacs.009 မက်ဆေ့ဂျ်နှင့် စစ်တမ်းများကို တင်သွင်းသည်
အခြေအနေကို ပြေလည်သွားသည်အထိ ISO တံတား။ ၎င်းကို စီစဉ်သတ်မှတ်ပါ-

- `TORII_URL` — Torii ISO တံတား API များကို ဖော်ထုတ်ပြသသည့် အဆုံးမှတ်။
- `ISO_MESSAGE_KIND` — `pacs.008` (မူရင်း) သို့မဟုတ် `pacs.009`။ အကူအညီပေးသူက အဆိုပါကို အသုံးပြု
  ကိုက်ညီသော နမူနာတည်ဆောက်သူ (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  သင်သည်သင်၏ကိုယ်ပိုင် XML ကိုမထောက်ပံ့ပေးသောအခါ။
- `ISO_MESSAGE_SUFFIX` — နမူနာ payload IDs တွင် ထည့်သွင်းထားသော ရွေးချယ်နိုင်သော နောက်ဆက်တွဲ
  ထပ်ခါတလဲလဲ အစမ်းလေ့ကျင့်မှုများကို ထူးထူးခြားခြား ထားရှိပါ (hex ရှိ လက်ရှိခေတ် စက္ကန့်များအတွက် ပုံသေများ)။
- `ISO_CONTENT_TYPE` — တင်ပြချက်များအတွက် `Content-Type` ခေါင်းစီးကို အစားထိုးပါ။
  (ဥပမာ `application/pacs009+xml`); သင်တစ်ဦးကို မဲဆွယ်သည့်အခါတွင် လျစ်လျူရှုထားသည်။
  ရှိပြီးသား မက်ဆေ့ခ်ျအိုင်ဒီ။
- `ISO_MESSAGE_ID` — တင်သွင်းမှုကို လုံး၀ ကျော်သွားကာ ပံ့ပိုးပေးထားသည့်ကိုသာ စစ်တမ်းကောက်ပါ
  `waitForIsoMessageStatus` မှတဆင့် identifier
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — စောင့်ဆိုင်းနည်းဗျူဟာကို ချိန်ညှိပါ
  ဆူညံသော သို့မဟုတ် နှေးကွေးသော တံတားများ ထားရှိမှု။
- `ISO_RESOLVE_ON_ACCEPTED=1` — Torii `Accepted` ပြန်တက်လာပြီးတာနဲ့ ထွက်ပါ
  ငွေပေးငွေယူ hash ကို ဆိုင်းငံ့ထားသော်လည်း (တံတားပြုပြင်ထိန်းသိမ်းမှုအတွင်း အဆင်ပြေသည်။
  လယ်ဂျာ ကတိကဝတ် နောက်ကျနေချိန်)။

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

Torii သည် terminal ကိုဘယ်တော့မှအစီရင်ခံခြင်းမရှိပါက၊ script နှစ်ခုလုံးသည် status code `1` ဖြင့်ထွက်သည်
အကူးအပြောင်းကြောင့် ၎င်းတို့ကို CI ဂိတ်အလုပ်များအတွက် သင့်လျော်စေသည်။

### ISO alias အကူအညီပေးသူ

`recipes/iso_alias.mjs` သည် ISO alias အဆုံးမှတ်များကို ပစ်မှတ်ထားသောကြောင့် လေ့ကျင့်မှုများသည် အကျုံးဝင်သည်
စိတ်ကြိုက်တူးလ်ကို မရေးဘဲ မျက်စိကွယ်နေသော ဒြပ်စင်ကို ဟက်ခြင်း နှင့် နံပတ်ရှာရှာဖွေခြင်း ။ အဲဒါ
`ToriiClient.evaluateAliasVoprf` နှင့် `resolveAlias` / `resolveAliasByIndex`
နောက်ကွယ်မှ၊ အချေအတင်၊ အကောင့်ချိတ်ဆက်မှု၊ အရင်းအမြစ်နှင့် သတ်မှတ်အညွှန်းကိန်းတို့ကို ပရင့်ထုတ်သည်။
Torii ဖြင့် ပြန်ပေးခဲ့သည်။

ပတ်ဝန်းကျင် ပြောင်းလဲမှုများ-

- `TORII_URL` — Torii alias helpers များကို ဖော်ထုတ်ပြသသည့် အဆုံးမှတ်။
- `ISO_VOPRF_INPUT` — hex-encoded blinded element (`deadbeef` သို့ ပုံသေများ)။
- `ISO_SKIP_VOPRF=1` — စမ်းသပ်ရှာဖွေမှုများသာရှိသည့်အခါ VOPRF ခေါ်ဆိုမှုကို ကျော်လိုက်ပါ။
- `ISO_ALIAS_LABEL` — ဖြေရှင်းရန် ပကတိအမည်များ (ဥပမာ၊ IBAN ပုံစံစာကြောင်းများ)။
- `ISO_ALIAS_INDEX` — ဒဿမ သို့မဟုတ် `0x`-ရှေ့ဆက်အညွှန်းကို `resolveAliasByIndex` သို့ ကျော်သွားသည်။
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — လုံခြုံသော Torii ဖြန့်ကျက်မှုအတွက် ရွေးချယ်နိုင်သော ခေါင်းစီးများ။

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

အကူအညီပေးသူက Torii ၏အပြုအမူကို ထင်ဟပ်စေသည်- ၎င်းသည် 404s များ ပျောက်နေသောအခါတွင် ၎င်းသည် 404s ပေါ်လာသည်
နှင့် runtime-disabled error များကို soft skips များအဖြစ် သဘောထားပြီး CI စီးဆင်းမှုများကို တံတားခံနိုင်သည်
ပြုပြင်ထိန်းသိမ်းမှုပြတင်းပေါက်များ။

## အုပ်ချုပ်မှုလုပ်ငန်းခွင်

### စာချုပ်စာတမ်းများနှင့် အဆိုပြုချက်များကို စစ်ဆေးပါ။

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

### အဆိုများနှင့် မဲစာရင်းများ တင်သွင်းပါ။

သင်ပယ်ဖျက်ရန် သို့မဟုတ် အချိန်ကန့်သတ်ထားသော အုပ်ချုပ်မှုတင်ပြချက်များ—SDK လိုအပ်သည့်အခါ `AbortController` ကိုသုံးပါ။
အောက်တွင်ဖော်ပြထားသော POST အကူအညီပေးသူတိုင်းအတွက် ရွေးချယ်နိုင်သော `{ signal }` အရာဝတ္ထုတစ်ခုကို လက်ခံသည်။

```ts
const authority = "ih58...";
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

const zkOwner = "ih58..."; // canonical IH58 account id for ZK public inputs
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

### ကောင်စီ VRF နှင့် အတည်ပြုပြဌာန်းခြင်း။

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "ih58...",
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

## ISO 20022 တံတားချက်ပြုတ်နည်းများ

### pacs.008 / pacs.009 payloads ကိုတည်ဆောက်ပါ။

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
  creditorAccount: { otherId: "ih58..." },
  purposeCode: "SECU",
  supplementaryData: { account_id: "ih58...", leg: "delivery" },
});
```

XML မတိုင်မီ အထောက်အထားများ (BIC၊ LEI၊ IBAN၊ ISO ပမာဏ) အားလုံးကို အတည်ပြုပြီးဖြစ်သည်
ထုတ်ပေးသည်။ PvP ကိုထုတ်လွှတ်ရန် `buildPacs009Message` အတွက် `buildPacs008Message` ကို လဲလှယ်ပါ
ထောက်ပံ့ငွေပေးချေမှုများ။

### ISO မက်ဆေ့ဂျ်များကို တင်သွင်းပြီး စစ်တမ်းကောက်ယူပါ။

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

`resolveOnAccepted` နှင့် `resolveOnAcceptedWithoutTransaction` နှစ်ခုစလုံးသည် တရားဝင်သည်; အလံတစ်ခုခုကိုသုံးပါ။
စစ်တမ်းများကို စည်းရုံးသည့်အခါ `Accepted` (ငွေပေးငွေယူ hash မပါဘဲ) အဆင့်များကို terminal အဖြစ် ဆက်ဆံရန်။

တံတားက ဘယ်တော့မှ သတင်းမပို့ရင် အကူအညီပေးသူတွေက `IsoMessageTimeoutError` ကို ပစ်ချတယ်။
terminal အခြေအနေ။ အောက်အဆင့် `submitIsoPacs008` / `submitIsoPacs009` ကိုသုံးပါ
စိတ်ကြိုက်မဲရုံ ယုတ္တိဗေဒကို စီမံဆောင်ရွက်ရန် လိုအပ်သည့်အခါ ဖုန်းခေါ်ဆိုပါ။ `getIsoMessageStatus`
တစ်ချက်တည်းရှာဖွေမှုကို ဖော်ထုတ်သည်။

### ဆက်စပ်လို့ ရပါသေးတယ်။

- `torii.getSorafsPorWeeklyReport("2026-W05")` သည် ISO-week PoR အတွဲကို ရယူသည်။
  လမ်းပြမြေပုံတွင် ကိုးကားပြီး သတိပေးချက်များအတွက် စောင့်ဆိုင်းနေသော အကူအညီများကို ပြန်သုံးနိုင်သည်။
- `resolveAlias` / `resolveAliasByIndex` သည် ISO Bridge alias bindings များကို ဖော်ထုတ်ရန်၊
  ပြန်လည်သင့်မြတ်ရေး ကိရိယာများသည် ငွေပေးချေမှုတစ်ခု မထုတ်ပြန်မီ အကောင့်ပိုင်ဆိုင်မှုကို သက်သေပြနိုင်သည်။