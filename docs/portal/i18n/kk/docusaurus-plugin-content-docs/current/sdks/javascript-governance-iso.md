---
slug: /sdks/javascript/governance-iso-examples
lang: kk
direction: ltr
source: docs/portal/docs/sdks/javascript-governance-iso.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Governance & ISO bridge examples
description: Drive advanced Torii workflows with `@iroha/iroha-js`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Бұл өріс нұсқаулығы басқаруды көрсету арқылы жылдам бастауға кеңейтіледі және
ISO 20022 көпірі `@iroha/iroha-js` арқылы өтеді. Үзінділер бірдей қайта пайдаланылады
`ToriiClient` арқылы жеткізілетін орындау уақыты көмекшілері, сондықтан оларды тікелей көшіруге болады
CLI құралдары, CI қондырғылары немесе ұзақ жұмыс істейтін қызметтер.

Қосымша ресурстар:

- `javascript/iroha_js/recipes/governance.mjs` — орындалатын соңына дейін сценарий
  ұсыныстар, бюллетеньдер және кеңес ротациялары.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — жіберуге арналған CLI көмекшісі
  pacs.008/pacs.009 пайдалы жүктемелер және сұраудың детерминирленген күйі.
- `docs/source/finance/settlement_iso_mapping.md` — ISO өрісінің канондық картасы.

## Жинақталған рецепттерді іске қосу

Бұл мысалдар `javascript/iroha_js/recipes/` ішіндегі сценарийлерге байланысты. Жүгіру
`npm install && npm run build:native` алдын ала, сондықтан жасалған байланыстырулар болады
қолжетімді.

### Басқару бойынша көмекші шолу

Шақыру алдында келесі орта айнымалы мәндерін конфигурациялаңыз
`recipes/governance.mjs`:

- `TORII_URL` — Torii соңғы нүкте.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — қол қоюшы тіркелгісі және кілт (он алтылық). Кілттерді а ішінде сақтаңыз
  қауіпсіз құпия дүкен.
- `CHAIN_ID` — қосымша желі идентификаторы.
- `GOV_SUBMIT=1` — жасалған транзакцияларды Torii дейін итеріңіз.
- `GOV_FETCH=1` — жіберілгеннен кейін ұсыныстарды/құлыптарды алу.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — пайдаланылған қосымша іздеулер
  `GOV_FETCH=1` кезінде.

```bash
npm run build:native

# Hashes only (safe for CI smoke runs).
TORII_URL=https://torii.testnet.sora \
node javascript/iroha_js/recipes/governance.mjs

# Submit + fetch using live credentials.
TORII_URL=https://torii.testnet.sora \
AUTHORITY=soraカタカナ... \
PRIVATE_KEY_HEX="$(cat ~/.iroha/keys/alice.key)" \
CHAIN_ID=7f2c...-prod \
GOV_SUBMIT=1 GOV_FETCH=1 \
GOV_PROPOSAL_ID=calc.v1 \
node javascript/iroha_js/recipes/governance.mjs
```

Әр қадам үшін хэштер журналға жазылады және Torii жауаптары пайда болады:
`GOV_SUBMIT=1`, сондықтан CI тапсырмалары жіберу қателерінде тез істен шығуы мүмкін.

### ISO көпір көмекшісі

`recipes/iso_bridge.mjs` pacs.008 немесе pacs.009 хабарын және сауалнаманы жібереді.
күй реттелгенше ISO көпірін басыңыз. Оны конфигурациялаңыз:

- `TORII_URL` — Torii соңғы нүктесі ISO көпірінің API интерфейстерін ашады.
- `ISO_MESSAGE_KIND` — `pacs.008` (әдепкі) немесе `pacs.009`. Көмекші пайдаланады
  сәйкес үлгі құрастырушы (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  сіз өзіңіздің XML-ді бермеген кезде.
- `ISO_MESSAGE_SUFFIX` — үлгі пайдалы жүктеме идентификаторларына қосылған қосымша жұрнақ
  қайталанатын жаттығуларды бірегей етіп сақтаңыз (әдепкі бойынша ағымдағы дәуір секундына он алтылықпен).
- `ISO_CONTENT_TYPE` — жіберулер үшін `Content-Type` тақырыбын қайта анықтау
  (мысалы, `application/pacs009+xml`); тек сауалнама жүргізгенде еленбейді
  бар хабар идентификаторы.
- `ISO_MESSAGE_ID` — жіберуді мүлдем өткізіп жіберіңіз және тек жеткізілгенді сұраңыз
  `waitForIsoMessageStatus` арқылы идентификатор.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — күту стратегиясын реттеңіз
  шулы немесе баяу көпірді орналастыру.
- `ISO_RESOLVE_ON_ACCEPTED=1` — Torii қайтарған кезде бірден шығу `Accepted`,
  транзакция хэші әлі күтілуде болса да (көпірге техникалық қызмет көрсету кезінде ыңғайлы
  бухгалтерлік міндеттеме кешіктірілгенде).

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

Екі сценарий де `1` күй кодымен шығады, егер Torii ешқашан терминал туралы есеп бермесе
өту, оларды CI қақпасы жұмыстарына қолайлы етеді.

### ISO бүркеншік ат көмекшісі

`recipes/iso_alias.mjs` ISO бүркеншік атының соңғы нүктелеріне бағытталған, осылайша жаттығулар қамтылуы мүмкін.
соқыр элементтерді хэштеу және арнайы құралдарды жазбай бүркеншік аттарды іздеу. Ол
қоңыраулар `ToriiClient.evaluateAliasVoprf` плюс `resolveAlias` / `resolveAliasByIndex`
және серверді, дайджестті, тіркелгіні байланыстыруды, бастапқы және детерминирленген индексті басып шығарады
Torii арқылы қайтарылды.

Қоршаған ортаның айнымалылары:

- `TORII_URL` — Torii соңғы нүкте бүркеншік ат көмекшілерін көрсетеді.
- `ISO_VOPRF_INPUT` — алтылық кодталған соқыр элемент (әдепкі бойынша `deadbeef`).
- `ISO_SKIP_VOPRF=1` — тек іздеулерді тексеру кезінде VOPRF қоңырауын өткізіп жіберіңіз.
- `ISO_ALIAS_LABEL` — шешуге арналған әріптік бүркеншік ат (мысалы, IBAN стиліндегі жолдар).
- `ISO_ALIAS_INDEX` — ондық немесе `0x`-префиксті индекс `resolveAliasByIndex`-қа өтті.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — қорғалған Torii орналастыруларына арналған қосымша тақырыптар.

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

Көмекші Torii әрекетін көрсетеді: бүркеншік аттар болмаған кезде ол 404-ті көрсетеді
және CI ағындары көпірге төзе алатындықтан, орындау уақытында өшірілген қателерді жұмсақ өткізіп жіберу ретінде қарастырады.
техникалық қызмет көрсету терезелері.

## Басқару жұмыс процестері

### Келісім-шарттар мен ұсыныстарды тексеріңіз

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

### Ұсыныстар мен бюллетеньдерді жіберу

`AbortController` нұсқасын бас тарту немесе уақытпен байланысты басқару жіберілімдерін — SDK пайдаланыңыз.
төменде көрсетілген әрбір POST көмекшісі үшін қосымша `{ signal }` нысанын қабылдайды.

```ts
const authority = "soraカタカナ...";
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

const zkOwner = "soraカタカナ..."; // canonical Katakana i105 account id for ZK public inputs
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

### Кеңес VRF және күшіне енуі

```ts
const validatorPk = Buffer.alloc(48, 0xdd);
const validatorProof = Buffer.alloc(96, 0xee);

const current = await torii.getGovernanceCouncilCurrent();
console.log(`epoch=${current.epoch} members=${current.members.length}`);

const derived = await torii.governanceDeriveCouncilVrf({
  committeeSize: 2,
  candidates: [
    {
      accountId: "soraカタカナ...",
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

## ISO 20022 көпір рецептері

### Пак.008 / пакет.009 пайдалы жүктемені құрастырыңыз

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
  creditorAccount: { otherId: "soraカタカナ..." },
  purposeCode: "SECU",
  supplementaryData: { account_id: "soraカタカナ...", leg: "delivery" },
});
```

Барлық идентификаторлар (BIC, LEI, IBAN, ISO сомасы) XML жасалғанға дейін тексеріледі.
құрылған. PvP шығару үшін `buildPacs008Message`-ті `buildPacs009Message`-ке ауыстырыңыз
пайдалы жүктемелерді қаржыландыру.

### ISO хабарламаларын жіберіңіз және сұраңыз

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

`resolveOnAccepted` және `resolveOnAcceptedWithoutTransaction` екеуі де жарамды; кез келген жалаушаны пайдаланыңыз
сауалнамаларды ұйымдастыру кезінде `Accepted` күйлерін (транзакция хэшсіз) терминал ретінде қарастыру.

Көмекшілер көпір ешқашан а хабар бермесе, `IsoMessageTimeoutError` лақтырады
терминалдық күй. Төменгі деңгейлі `submitIsoPacs008` / `submitIsoPacs009` пайдаланыңыз
теңшелетін сұрау логикасын реттеу қажет болғанда шақырады; `getIsoMessageStatus`
бір реттік іздеуді көрсетеді.

### Байланысты беттер

- `torii.getSorafsPorWeeklyReport("2026-W05")` ISO апталық PoR жинағын алады
  жол картасында сілтеме жасалған және ескертулер үшін күту көмекшілерін қайта пайдалана алады.
- `resolveAlias` / `resolveAliasByIndex` ISO көпірінің бүркеншік атын байланыстырады, осылайша
  салыстыру құралдары төлемді бермес бұрын шоттың иелігін дәлелдей алады.