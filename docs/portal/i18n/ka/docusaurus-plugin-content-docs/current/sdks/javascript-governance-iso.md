---
slug: /sdks/javascript/governance-iso-examples
lang: ka
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

ეს საველე სახელმძღვანელო აფართოებს სწრაფ დაწყებას მმართველობის დემონსტრირებით და
ISO 20022 ხიდი მიედინება `@iroha/iroha-js`-ით. ფრაგმენტები ხელახლა გამოიყენება იგივე
გაშვების დამხმარეები, რომლებიც იგზავნება `ToriiClient`-ით, ასე რომ თქვენ შეგიძლიათ დააკოპიროთ ისინი პირდაპირ
CLI ხელსაწყოები, CI აღკაზმულობა ან გრძელვადიანი სერვისები.

დამატებითი რესურსები:

- `javascript/iroha_js/recipes/governance.mjs` — გაშვებადი სკრიპტი ბოლოდან ბოლომდე
  წინადადებები, ბიულეტენები და საბჭოს როტაცია.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — CLI დამხმარე გაგზავნისთვის
  pacs.008/pacs.009 payloads და polling deterministic status.
- `docs/source/finance/settlement_iso_mapping.md` — კანონიკური ISO ველის რუქა.

## შეფუთული რეცეპტების გაშვება

ეს მაგალითები დამოკიდებულია `javascript/iroha_js/recipes/`-ის სკრიპტებზე. გაიქეცი
`npm install && npm run build:native` წინასწარ, ასე რომ გენერირებული საკინძები არის
ხელმისაწვდომი.

### მმართველობის დამხმარე გზამკვლევი

გამოძახებამდე დააკონფიგურირეთ შემდეგი გარემო ცვლადები
`recipes/governance.mjs`:

- `TORII_URL` — Torii საბოლოო წერტილი.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — ხელმომწერის ანგარიში და გასაღები (ჰექს.). შეინახეთ გასაღებები ა
  უსაფრთხო საიდუმლო მაღაზია.
- `CHAIN_ID` — არჩევითი ქსელის იდენტიფიკატორი.
- `GOV_SUBMIT=1` — დააყენეთ გენერირებული ტრანზაქციები Torii-მდე.
- `GOV_FETCH=1` — მიიღეთ წინადადებები/დაბლოკვები წარდგენის შემდეგ.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — გამოყენებული არჩევითი ძიება
  როცა `GOV_FETCH=1`.

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

ჰეშები იწერება ყოველი ნაბიჯისთვის და Torii პასუხები გამოჩნდება, როდესაც
`GOV_SUBMIT=1` ასე რომ, CI სამუშაოები შეიძლება სწრაფად ჩავარდეს წარდგენის შეცდომებზე.

### ISO ხიდის დამხმარე

`recipes/iso_bridge.mjs` აგზავნის ან pacs.008 ან pacs.009 შეტყობინებას და გამოკითხვებს
ISO ხიდი სტატუსის მოწესრიგებამდე. კონფიგურაცია:

- `TORII_URL` — Torii საბოლოო წერტილი, რომელიც ავლენს ISO ხიდის API-ებს.
- `ISO_MESSAGE_KIND` — `pacs.008` (ნაგულისხმევი) ან `pacs.009`. დამხმარე იყენებს
  შესატყვისი ნიმუშის შემქმნელი (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  როდესაც არ აწვდით საკუთარ XML-ს.
- `ISO_MESSAGE_SUFFIX` — სურვილისამებრ სუფიქსი, რომელიც დართულია სანიმუშო დატვირთვის ID-ებზე
  შეინახეთ განმეორებითი რეპეტიციები უნიკალური (ნაგულისხმევი ეპოქის წამებში ექვსკუთხედში).
- `ISO_CONTENT_TYPE` — უგულებელყოთ `Content-Type` სათაური წარდგენისთვის
  (მაგალითად `application/pacs009+xml`); იგნორირებულია, როცა მხოლოდ გამოკითხვაზე აკეთებთ
  არსებული შეტყობინების ID.
- `ISO_MESSAGE_ID` — საერთოდ გამოტოვეთ გაგზავნა და გამოკითხეთ მხოლოდ მოწოდებული
  იდენტიფიკატორი `waitForIsoMessageStatus`-ის საშუალებით.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — დაარეგულირეთ ლოდინის სტრატეგია
  ხმაურიანი ან ნელი ხიდის განლაგება.
- `ISO_RESOLVE_ON_ACCEPTED=1` — გადით, როგორც კი Torii დააბრუნებს `Accepted`,
  მაშინაც კი, თუ ტრანზაქციის ჰეში ჯერ კიდევ მოლოდინშია (ხელსაყრელია ხიდის მოვლის დროს
  როდესაც წიგნის ჩადენა დაგვიანებულია).

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

ორივე სკრიპტი გამოდის სტატუსის კოდით `1`, თუ Torii არასოდეს იტყობინება ტერმინალზე
გარდამავალი, რაც მათ შესაფერისს ხდის CI კარიბჭის სამუშაოებისთვის.

### ISO მეტსახელის დამხმარე

`recipes/iso_alias.mjs` მიზნად ისახავს ISO მეტსახელის საბოლოო წერტილებს, რათა რეპეტიციები დაფაროს
დაბრმავებული ელემენტების ჰეშირება და ფსევდონიმების ძიება შეკვეთილი ხელსაწყოების დაწერის გარეშე. ის
ზარები `ToriiClient.evaluateAliasVoprf` პლუს `resolveAlias` / `resolveAliasByIndex`
და ბეჭდავს backend, დაიჯესტი, ანგარიშის სავალდებულო, წყარო და დეტერმინისტული ინდექსი
დააბრუნა Torii.

გარემოს ცვლადები:

- `TORII_URL` — Torii საბოლოო წერტილი, რომელიც ავლენს მეტსახელის დამხმარეებს.
- `ISO_VOPRF_INPUT` — თექვსმეტობით დაშიფრული დაბრმავებული ელემენტი (ნაგულისხმევად არის `deadbeef`).
- `ISO_SKIP_VOPRF=1` — გამოტოვეთ VOPRF ზარი მხოლოდ ძიების ტესტირებისას.
- `ISO_ALIAS_LABEL` — პირდაპირი მეტსახელი ამოსახსნელად (მაგ., IBAN-ის სტილის სტრიქონები).
- `ISO_ALIAS_INDEX` — ათობითი ან `0x` პრეფიქსის ინდექსი გადავიდა `resolveAliasByIndex`-ზე.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — არჩევითი სათაურები დაცული Torii განლაგებისთვის.

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

დამხმარე ასახავს Torii-ის ქცევას: ის ჩნდება 404s-ზე, როცა მეტსახელები აკლია
და განიხილავს შეცდომებს, რომლებიც გამორთულია მუშაობის დროს
მოვლის ფანჯრები.

## მმართველობის სამუშაო ნაკადები

### შეამოწმეთ კონტრაქტის შემთხვევები და წინადადებები

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

### წარადგინეთ წინადადებები და ბიულეტენები

გამოიყენეთ `AbortController`, როდესაც გჭირდებათ გააუქმოთ ან დროში შეზღუდული მმართველობითი წარდგენები — SDK
იღებს არასავალდებულო `{ signal }` ობიექტს ყველა POST დამხმარესთვის, რომელიც ნაჩვენებია ქვემოთ.

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

### საბჭოს VRF და ამოქმედება

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

## ISO 20022 ხიდის რეცეპტები

### Build pacs.008 / pacs.009 payloads

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

ყველა იდენტიფიკატორი (BIC, LEI, IBAN, ISO თანხა) დამოწმებულია XML-მდე
გენერირებული. შეცვალეთ `buildPacs008Message` `buildPacs009Message`-ით PvP გამოსაშვებად
დაფინანსების ტვირთამწეობა.

### გაგზავნეთ და გამოკითხეთ ISO შეტყობინებები

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

ორივე `resolveOnAccepted` და `resolveOnAcceptedWithoutTransaction` მოქმედებს; გამოიყენეთ რომელიმე დროშა
`Accepted` სტატუსების (ტრანზაქციის ჰეშის გარეშე) განხილვა, როგორც ტერმინალი გამოკითხვების ორკესტრირებისას.

დამხმარეები აგდებენ `IsoMessageTimeoutError`-ს, თუ ხიდი არასოდეს იტყობინება ა
ტერმინალური მდგომარეობა. გამოიყენეთ ქვედა დონის `submitIsoPacs008` / `submitIsoPacs009`
ზარები, როდესაც გჭირდებათ პერსონალური გამოკითხვის ლოგიკის ორკესტრირება; `getIsoMessageStatus`
ავლენს ერთი დარტყმის ძიებას.

### დაკავშირებული ზედაპირები

- `torii.getSorafsPorWeeklyReport("2026-W05")` იღებს ISO-კვირის PoR პაკეტს
  მითითებულია საგზაო რუკაში და შეუძლია ხელახლა გამოიყენოს ლოდინის დამხმარეები გაფრთხილებისთვის.
- `resolveAlias` / `resolveAliasByIndex` გამოაშკარავებს ISO ხიდის მეტსახელის შეკვრას, ასე რომ
  შერიგების ინსტრუმენტებს შეუძლიათ დაამტკიცონ ანგარიშის საკუთრება გადახდის გაცემამდე.