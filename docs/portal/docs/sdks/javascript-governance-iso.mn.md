---
lang: mn
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

Энэхүү хээрийн гарын авлага нь засаглал болон
ISO 20022 гүүр нь `@iroha/iroha-js`-тай урсдаг. Хэсэгчилсэн хэсгүүдийг дахин ашигладаг
`ToriiClient`-тэй хамт ирдэг ажиллах цагийн туслахууд тул та тэдгээрийг шууд хуулах боломжтой.
CLI багаж хэрэгсэл, CI бэхэлгээ, эсвэл урт хугацааны үйлчилгээ.

Нэмэлт нөөц:

- `javascript/iroha_js/recipes/governance.mjs` — ажиллуулах боломжтой төгсгөлийн скрипт
  санал, саналын хуудас, зөвлөлийн эргэлт.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — Илгээх CLI туслах
  pacs.008/pacs.009 ачаалал ба санал асуулгын детерминистик статус.
- `docs/source/finance/settlement_iso_mapping.md` — ISO талбарын каноник зураглал.

## Багцалсан жоруудыг ажиллуулж байна

Эдгээр жишээнүүд нь `javascript/iroha_js/recipes/` дээрх скриптүүдээс хамаарна. Гүй
`npm install && npm run build:native` өмнө нь үүсгэсэн холбоосууд нь байна
боломжтой.

### Засаглалын туслах заавар

Дуудлага хийхээсээ өмнө дараах орчны хувьсагчдыг тохируулна уу
`recipes/governance.mjs`:

- `TORII_URL` — Torii төгсгөлийн цэг.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — гарын үсэг зурсан данс ба түлхүүр (hex). Түлхүүрийг а-д хадгал
  аюулгүй нууц дэлгүүр.
- `CHAIN_ID` — нэмэлт сүлжээ танигч.
- `GOV_SUBMIT=1` — үүсгэсэн гүйлгээг Torii рүү түлхэнэ.
- `GOV_FETCH=1` — илгээсний дараа санал/түгжээг дуудах.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — нэмэлт хайлтуудыг ашигласан
  `GOV_FETCH=1` үед.

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

Хэшийг алхам бүрт бүртгэж, Torii хариу гарч ирэхэд
`GOV_SUBMIT=1` тиймээс CI ажлууд нь илгээх алдаан дээр хурдан бүтэлгүйтдэг.

### ISO гүүрний туслах

`recipes/iso_bridge.mjs` pacs.008 эсвэл pacs.009 мессеж болон санал асуулга илгээдэг.
статус тогтох хүртэл ISO гүүр. Үүнийг тохируулна уу:

- `TORII_URL` — Torii төгсгөлийн цэг нь ISO гүүр API-г илчлэх.
- `ISO_MESSAGE_KIND` — `pacs.008` (анхдагч) эсвэл `pacs.009`. Туслах нь ашигладаг
  тохирох загвар бүтээгч (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  Хэрэв та өөрийн XML-г нийлүүлээгүй үед.
- `ISO_MESSAGE_SUFFIX` - түүвэр ачааллын ID-д хавсаргасан нэмэлт дагавар
  давтан давталтыг өвөрмөц байлгах (одоогийн эрин секундын өгөгдмөл нь зургаан өнцөгт).
- `ISO_CONTENT_TYPE` — илгээлтийн `Content-Type` толгой хэсгийг дарж бичих
  (жишээ нь `application/pacs009+xml`); та зөвхөн санал асуулга явуулах үед үл тоомсорлодог
  одоо байгаа мессежийн ID.
- `ISO_MESSAGE_ID` - илгээхийг бүрмөсөн алгасаж, зөвхөн нийлүүлсэнээс санал авна.
  `waitForIsoMessageStatus`-ээр дамжуулан танигч.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` - хүлээх стратегийг тохируулна уу.
  чимээ шуугиантай эсвэл удаан гүүр байрлуулах.
- `ISO_RESOLVE_ON_ACCEPTED=1` — Torii `Accepted`-г буцаамагц гарах,
  гүйлгээний хэш хүлээгдэж байгаа байсан ч (гүүрийн засвар үйлчилгээний үед тохиромжтой
  бүртгэлийн үүрэг хойшлогдсон үед).

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

Хэрэв Torii терминалыг хэзээ ч мэдээлдэггүй бол хоёр скрипт хоёулаа `1` статус кодоор гарна.
шилжилт, тэдгээрийг CI хаалганы ажилд тохиромжтой болгох.

### ISO нэрийн туслах

`recipes/iso_alias.mjs` нь ISO нэрийн төгсгөлийн цэгүүдийг чиглүүлдэг тул сургуулилтууд хамрагдах боломжтой
Захиалгат хэрэгслийг бичихгүйгээр сохор элементийн хэш болон бусад нэр хайх. Энэ
дууддаг `ToriiClient.evaluateAliasVoprf` plus `resolveAlias` / `resolveAliasByIndex`
мөн backend, digest, account binding, source, deterministic индексийг хэвлэдэг
Torii буцаасан.

Хүрээлэн буй орчны хувьсагчид:

- `TORII_URL` — Torii төгсгөлийн цэг нь бусад нэрийн туслахуудыг илчлэх.
- `ISO_VOPRF_INPUT` — зургаан өнцөгт кодлогдсон сохор элемент (анхдагчаар `deadbeef`).
- `ISO_SKIP_VOPRF=1` — зөвхөн хайлт хийх үед VOPRF дуудлагыг алгасах.
- `ISO_ALIAS_LABEL` — шийдвэрлэх энгийн нэр (жишээ нь, IBAN маягийн мөрүүд).
- `ISO_ALIAS_INDEX` — аравтын бутархай буюу `0x` угтвартай индексийг `resolveAliasByIndex` руу шилжүүлсэн.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — хамгаалалттай Torii байршуулалтад зориулсан нэмэлт толгой.

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

Туслагч нь Torii-ийн зан төлөвийг тусгадаг: нэр байхгүй үед 404-ийг харуулдаг.
ба ажиллах үеийн идэвхгүй алдааг зөөлөн алгасах гэж үздэг тул CI урсгал нь гүүрийг тэсвэрлэх чадвартай.
засвар үйлчилгээний цонх.

## Засаглалын ажлын урсгал

### Гэрээний тохиолдлууд болон саналыг шалгах

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

### Санал болон саналын хуудас ирүүлнэ үү

SDK-г цуцлах эсвэл хугацаатай засаглалын илгээлтийг цуцлах шаардлагатай үед `AbortController` ашиглана уу.
доор үзүүлсэн POST туслагч бүрийн нэмэлт `{ signal }` объектыг хүлээн авдаг.

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

### Зөвлөлийн VRF ба хууль тогтоомж

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

## ISO 20022 гүүрний жор

### Пак.008 / багц.009 ачааг бүтээх

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

Бүх танигч (BIC, LEI, IBAN, ISO хэмжээ) нь XML-г оруулахаас өмнө баталгаажсан.
үүсгэсэн. PvP гаргахын тулд `buildPacs008Message`-г `buildPacs009Message`-ээр соль
ачааллыг санхүүжүүлэх.

### ISO мессеж илгээх, санал асуулга явуулах

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

`resolveOnAccepted` болон `resolveOnAcceptedWithoutTransaction` хоёулаа хүчинтэй; аль нэг туг ашиглана уу
санал асуулга зохион байгуулахдаа `Accepted` статусыг (гүйлгээний хэшгүй) терминал гэж үзэх.

Туслагч нар гүүр хэзээ ч мэдээлэхгүй бол `IsoMessageTimeoutError` шидэх a
терминал төлөв. Доод түвшний `submitIsoPacs008` / `submitIsoPacs009` ашиглах
санал асуулгын логикийг зохицуулах шаардлагатай үед дуудлага хийх; `getIsoMessageStatus`
нэг удаагийн хайлтыг харуулж байна.

### Холбогдох гадаргуу

- `torii.getSorafsPorWeeklyReport("2026-W05")` нь ISO долоо хоногийн PoR багцыг татаж авдаг
  Замын зурагт дурдсан бөгөөд сэрэмжлүүлэг өгөхөд хүлээх туслахуудыг дахин ашиглах боломжтой.
- `resolveAlias` / `resolveAliasByIndex` ISO гүүрний бусад нэрийн холболтыг ил гаргах
  Төлбөр хийхээс өмнө тооцооны хэрэгсэл нь данс эзэмшиж байгааг нотлох боломжтой.