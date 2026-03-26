---
lang: uz
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

Ushbu soha qo'llanmasi boshqaruvni namoyish qilish orqali tezkor boshlashni kengaytiradi va
ISO 20022 koʻprigi `@iroha/iroha-js` bilan oqadi. Snippetlar xuddi shunday qayta ishlatiladi
`ToriiClient` bilan yuboriladigan ish vaqti yordamchilari, shuning uchun ularni to'g'ridan-to'g'ri nusxalashingiz mumkin
CLI asboblari, CI jabduqlari yoki uzoq muddatli xizmatlar.

Qo'shimcha manbalar:

- `javascript/iroha_js/recipes/governance.mjs` - ishga tushiriladigan uchdan-end skript
  takliflar, saylov byulletenlari va kengash rotatsiyalari.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — yuborish uchun CLI yordamchisi
  pacs.008/pacs.009 foydali yuklar va so'rovning deterministik holati.
- `docs/source/finance/settlement_iso_mapping.md` - kanonik ISO maydon xaritasi.

## Birlashtirilgan retseptlarni ishga tushirish

Bu misollar `javascript/iroha_js/recipes/` dagi skriptlarga bog'liq. Yugurish
Oldindan `npm install && npm run build:native`, shuning uchun yaratilgan bog'lanishlar
mavjud.

### Boshqaruv boʻyicha yordamchi maʼlumot

Chaqirishdan oldin quyidagi muhit o'zgaruvchilarini sozlang
`recipes/governance.mjs`:

- `TORII_URL` — Torii oxirgi nuqtasi.
- `AUTHORITY` / `PRIVATE_KEY_HEX` - imzolovchi hisobi va kalit (oltilik). Kalitlarni a ichida saqlang
  xavfsiz maxfiy do'kon.
- `CHAIN_ID` — ixtiyoriy tarmoq identifikatori.
- `GOV_SUBMIT=1` - yaratilgan tranzaksiyalarni Torii ga suring.
- `GOV_FETCH=1` - taqdim etilgandan keyin takliflar/qulflarni olish.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` - ixtiyoriy qidiruvlar ishlatiladi
  qachon `GOV_FETCH=1`.

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

Har bir qadam uchun xeshlar qayd qilinadi va Torii javoblari paydo bo'lganda
`GOV_SUBMIT=1`, shuning uchun CI ishlari yuborish xatolarida tezda muvaffaqiyatsiz bo'lishi mumkin.

### ISO ko'prik yordamchisi

`recipes/iso_bridge.mjs` pacs.008 yoki pacs.009 xabar va so‘rovnomalarni yuboradi
holat o'zgarmaguncha ISO ko'prigi. Uni quyidagi bilan sozlang:

- `TORII_URL` - Torii so'nggi nuqta ISO ko'prigi API-larini ochib beradi.
- `ISO_MESSAGE_KIND` — `pacs.008` (standart) yoki `pacs.009`. Yordamchi dan foydalanadi
  mos keluvchi namuna yaratuvchisi (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  o'zingizning XML-ni taqdim qilmasangiz.
- `ISO_MESSAGE_SUFFIX` - namunaviy yuk identifikatorlariga qo'shilgan ixtiyoriy qo'shimcha
  takroriy takroriy mashqlarni noyob saqlang (birinchi marta joriy davr soniyalari uchun birlamchi).
- `ISO_CONTENT_TYPE` - yuborish uchun `Content-Type` sarlavhasini bekor qilish
  (masalan, `application/pacs009+xml`); faqat so'rov o'tkazganingizda e'tiborga olinmaydi
  mavjud xabar identifikatori.
- `ISO_MESSAGE_ID` - topshirishni butunlay o'tkazib yuboring va faqat taqdim etilgan so'rovni o'tkazing
  `waitForIsoMessageStatus` orqali identifikator.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` - kutish strategiyasini sozlang
  shovqinli yoki sekin ko'prikni joylashtirish.
- `ISO_RESOLVE_ON_ACCEPTED=1` - Torii `Accepted` qaytishi bilanoq chiqish,
  tranzaksiya xeshi hali kutilayotgan bo'lsa ham (ko'prikga texnik xizmat ko'rsatish paytida qulay
  daftarni topshirish kechiktirilganda).

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

Ikkala skript ham `1` holat kodi bilan chiqadi, agar Torii hech qachon terminal haqida xabar bermasa
o'tish, ularni CI darvoza ishlari uchun mos qilish.

### ISO taxallus yordamchisi

`recipes/iso_alias.mjs` ISO taxallusning so'nggi nuqtalarini maqsad qilib qo'yadi, shuning uchun mashg'ulotlar qamrab olinishi mumkin
maxsus asboblarni yozmasdan ko'r-ko'rona elementlarni xeshlash va taxalluslarni qidirish. Bu
qo'ng'iroq qiladi `ToriiClient.evaluateAliasVoprf` plus `resolveAlias` / `resolveAliasByIndex`
va backend, dayjest, hisob ulanishi, manba va deterministik indeksni chop etadi
Torii tomonidan qaytarildi.

Atrof-muhit o'zgaruvchilari:

- `TORII_URL` — Torii so'nggi nuqta yordamchi nomli yordamchilarni ochib beradi.
- `ISO_VOPRF_INPUT` - olti burchakli kodlangan ko'r element (birlamchi `deadbeef`).
- `ISO_SKIP_VOPRF=1` — faqat qidiruvlarni sinab ko'rayotganda VOPRF chaqiruvini o'tkazib yuboring.
- `ISO_ALIAS_LABEL` — hal qilish uchun literal taxallus (masalan, IBAN uslubidagi satrlar).
- `ISO_ALIAS_INDEX` - o'nlik yoki `0x`-prefiksli indeks `resolveAliasByIndex` ga o'tkazildi.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` - xavfsiz Torii joylashtirishlari uchun ixtiyoriy sarlavhalar.

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

Yordamchi Torii ning xatti-harakatlarini aks ettiradi: taxalluslar yo'q bo'lganda u 404 ni ko'rsatadi.
va ish vaqti o'chirilgan xatolarni yumshoq o'tkazib yuborish sifatida ko'rib chiqadi, shuning uchun CI oqimlari ko'priklarga bardosh bera oladi
parvarishlash oynalari.

## Boshqaruv ish oqimlari

### Shartnoma misollari va takliflarini tekshiring

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

### Takliflar va byulletenlarni yuboring

Bekor qilish yoki vaqt bilan bogʻliq boʻlgan boshqaruv yuborishlari kerak boʻlganda `AbortController` dan foydalaning — SDK
quyida ko'rsatilgan har bir POST yordamchisi uchun ixtiyoriy `{ signal }` ob'ektini qabul qiladi.

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

### VRF Kengashi va qabul qilish

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

## ISO 20022 ko'prik retseptlari

### Pacs.008 / pacs.009 foydali yuklarni yarating

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

Barcha identifikatorlar (BIC, LEI, IBAN, ISO miqdori) XML dan oldin tekshiriladi.
yaratilgan. PvP chiqarish uchun `buildPacs008Message`ni `buildPacs009Message` bilan almashtiring
foydali yuklarni moliyalashtirish.

### ISO xabarlarini yuboring va so'rov qiling

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

`resolveOnAccepted` va `resolveOnAcceptedWithoutTransaction` ham amal qiladi; ikkala bayroqdan foydalaning
so'rovlarni tashkil qilishda `Accepted` holatini (tranzaksiya xeshsiz) terminal sifatida ko'rib chiqish.

Ko'prik hech qachon a xabar bermasa, yordamchilar `IsoMessageTimeoutError` tashlaydi
terminal holati. Past darajadagi `submitIsoPacs008` / `submitIsoPacs009` dan foydalaning
maxsus so'rov mantig'ini tartibga solish kerak bo'lganda qo'ng'iroqlar; `getIsoMessageStatus`
bir martalik qidiruvni ochib beradi.

### Tegishli yuzalar

- `torii.getSorafsPorWeeklyReport("2026-W05")` ISO haftalik PoR to'plamini oladi
  yo'l xaritasida ko'rsatilgan va ogohlantirishlar uchun kutish yordamchilaridan qayta foydalanishi mumkin.
- `resolveAlias` / `resolveAliasByIndex` ISO ko'prigi taxallusli bog'lanishlarni ochib beradi
  solishtirish vositalari to'lovni amalga oshirishdan oldin hisob egaligini isbotlashi mumkin.