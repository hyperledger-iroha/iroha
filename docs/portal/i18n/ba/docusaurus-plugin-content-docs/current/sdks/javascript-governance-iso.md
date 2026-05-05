---
slug: /sdks/javascript/governance-iso-examples
lang: ba
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Был ялан етәкселеге тиҙ старт өҫтөндә киңәйә, идара итеү һәм
ISO 20022 күпер ағымы менән I18NI0000000018X. Фрагменттары шул уҡ ҡабаттан ҡулланыла
йүгерә ярҙамсылар, тип судно менән I18NI0000000019X, шулай итеп, һеҙ уларҙы туранан-тура күсерергә мөмкин
CLI инструменталь, CI йүгән, йәки оҙаҡ эшләгән хеҙмәттәр.

Өҫтәмә ресурстар:

- I18NI000000020X — 2012 йыл өсөн йүгерә торған ос-ос сценарийы.
  тәҡдимдәр, бюллетендәр, һәм совет ротациялары.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — тапшырыу өсөн CLI ярҙамсыһы
  pacs.008/pacs.009 файҙалы йөктәр һәм һорау алыу детерминистик статусы.
- `docs/source/finance/settlement_iso_mapping.md` — канонлы ISO ялан картаһы.

## Йүгереп йыйылған рецепттар

Был миҫалдар I18NI000000023X-тағы сценарийҙарға бәйле. Йүгерергә
I18NI0000000024X алдан шулай генерацияланған бәйләүҙәр .
асыҡ.

### Идара итеү ярҙамсыһы проходка

Конфигурациялау түбәндәге мөхит үҙгәртеүсәндәр алдынан саҡырыу .
I18NI000000255Х:

- `TORII_URL` — Torii ос нөктәһе.
- `AUTHORITY` / I18NI000000028X — ҡултамға иҫәп һәм асҡыс (гекс). Асҡыстарҙы һаҡлау өсөн
  йәшерен магазин.
- `CHAIN_ID` — опциональ селтәр идентификаторы.
- I18NI000000030X — генерацияланған транзакцияларҙы Torii-ҡа этәрергә.
- `GOV_FETCH=1` — тапшырылғандан һуң тәҡдимдәр/блоктар алыу.
- I18NI0000000032X, I18NI000000033X, I18NI000000034X — опциональ эҙләүҙәр ҡулланылған
  ҡасан I18NI000000035X.

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

Хэштар һәр аҙым өсөн логин, һәм I18NT0000000002X яуаптар өҫтөндә ҡасан да булһа .
I18NI000000036X шулай CI эш урындары тиҙ уңышһыҙлыҡҡа осрай ала тапшырыу хаталары.

### ISO күпер ярҙамсыһы

I18NI0000000037X йәки pacs.008 йәки pacs.009 хәбәр һәм һорау алыуҙар тапшыра
ISO күпере статус урынлашҡанға тиклем. Уны конфигурациялау:

- I18NI000000038X — ISO күпер API-ларын фашлаусы I18NT00000003Х.
- I18NI000000039X — I18NI000000040X (ғәҙәти) йәки `pacs.009`. Ярҙамсы ҡуллана
  тап килгән өлгө төҙөүсе (I18NI000000042X / I18NI000000043X)
  ҡасан һеҙ үҙегеҙҙең XML тәьмин итмәй.
- I18NI0000000044X — опциональ суффикс ҡушылған өлгө файҙалы йөк идентификаторҙары .
  ҡабат-ҡабат репетицияларҙы үҙенсәлекле тотоу (алтын секундтарға тиклем гекста секундтарға тиклем ғәҙәттәгесә).
- I18NI0000000045X — I18NI0000000046ХХХ-ның тапшырыуҙар өсөн башын күтәрә
  (мәҫәлән, `application/pacs009+xml`); иғтибарға алынмаған, ҡасан һеҙ тик һорау алыу ан
  булған хәбәр id.
- I18NI000000048X — һикереп тапшырыу бөтөнләй һәм тик һорау алыу менән тәьмин итеү
  I18NI000000049X аша идентификатор.
- I18NI000000050X / I18NI000000051X — 1990 йылға көтөү стратегияһын көйләй.
  шау-шыулы йәки яй күпер таратыу.
- I18NI000000052X — I18NT00000000004X 3-сө һанлы 18NI0000000533Х ҡағиҙәһе менән сығыу.
  хатта әгәр ҙә транзакция хеш һаман да көтөп тора (күперҙәрҙе хеҙмәтләндереүҙең ваҡытында ҡулайлы
  ҡасан баш китап ҡылыу тотҡарлана).

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

Ике сценарий ҙа I18NI000000054X статус коды менән сыға, әгәр I18NT000000005X бер ҡасан да терминал тураһында хәбәр итә.
күсеү, уларҙы CI ҡапҡа эштәренә яраҡлы итеү.

### ИСО псевдоним ярҙамсыһы

Torii маҡсатлы ISO псевдонимы ос нөктәләре шулай репетициялар ҡаплай ала
һуҡыр-элемент хеширование һәм псевдоним эҙләүҙәр яҙмай, заказ буйынса инструменттар. Был
шылтыратыуҙары I18NI0000000056X плюс `resolveAlias` / I18NI000000058X .
һәм бэкэнд, үҙләштереү, иҫәп бәйләү, сығанаҡ һәм детерминистик индексы баҫтыра
Torii ҡайтарып ҡайтара.

Тирә-яҡ мөхит үҙгәртеүселәре:

- I18NI000000059X — Torii тамамлаусы псевдоним ярҙамсыларын фашлау.
- `ISO_VOPRF_INPUT` — алты кодлы һуҡыр элемент (`deadbeef` тиклем ғәҙәттәгесә).
- I18NI000000062X — VOPRF шылтыратыуын үткәреп ебәргәндә тик һынау ғына.
- `ISO_ALIAS_LABEL` — туранан-тура псевдоним хәл итеү өсөн (мәҫәлән, IBAN стилендәге ҡылдар).
- I18NI000000064X — унлыҡ йәки I18NI000000065X-префиксированный индекс `resolveAliasByIndex` XX.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — Torii-ны һаҡлау өсөн өҫтәмә башлыҡтар.

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

Ярҙамсы көҙгө I18NT0000000009X’s тәртибе: ул 404-се урында тора, ҡасан псевдоним юҡ
һәм эшкәртеү ваҡыты-инвалид хаталар кеүек йомшаҡ скиптар, шулай итеп, CI ағымдары күпер түҙә ала
хеҙмәтләндереүҙең тәҙрәләре.

## Идара итеү эш ағымы

### Контракт инстанцияларын һәм тәҡдимдәрен тикшерергә

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

### Тәҡдимдәр һәм бюллетендәр

Ҡулланыу I18NI0000000069X, ҡасан һеҙгә кәрәк, йәки ваҡыт менән бәйле идара итеү тапшырыуҙарын юҡҡа сығарыу-SDK .
ҡабул итә опциональ `{ signal }` объекты өсөн һәр POST ярҙамсыһы түбәндә күрһәтелгән.

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

### Совет VRF һәм ҡабул итеү

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

## ISO 20022 күпер рецептары

### Төҙөү pacs.008 / pacs.009 файҙалы йөкләмәләр

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

Бөтә идентификаторҙар (БИК, LEI, IBAN, ISO суммаһы) XML тиклем раҫланған.
генерацияланған. I18NI0000071X өсөн I18NI000000072X өсөн PvP сығарыу өсөн Swap .
финанслау файҙалы йөктәр.

### ISO хәбәрҙәрен тапшырыу һәм һорау алыу

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

`resolveOnAccepted` һәм `resolveOnAcceptedWithoutTransaction` икеһе лә ғәмәлдә; йәки флаг ҡулланыу
`Accepted` статустарын дауалау өсөн (транзакция хешыһыҙ) һорау алыуҙарҙы ойоштороуҙа терминал булараҡ.

Ярҙамсылар ташлай I18NI0000000076X, әгәр күпер бер ҡасан да хәбәр
терминаль хәле. Ҡулланыу түбән кимәлдә I18NI000000077X / I18NI000000078Х.
шылтыратыуҙар ҡасан һеҙгә кәрәк, тип оркестрлаштырыу өсөн заказ буйынса һорау алыу логикаһы; `getIsoMessageStatus`.
бер тапҡыр атыуҙы фашлай.

### Бәйләнешле ер өҫтө

- I18NI000000080X ISO-аҙна PoR өйөмөн килтерә
  юл картаһында һылтанма һәм иҫкәртмәләр өсөн көтөү ярҙамсыларын ҡабаттан ҡуллана ала.
- I18NI000000081X / I18NI000000082X ISO күпер псевдонимдарын фашлай.
  яраштырыу ҡоралдары түләүҙе биргәнсе иҫәп милекселеген иҫбатлай ала.