---
lang: az
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

Bu sahə təlimatı idarəetməni nümayiş etdirməklə sürətli başlanğıcı genişləndirir və
ISO 20022 körpüsü `@iroha/iroha-js` ilə axır. Parçalar eyni şəkildə təkrar istifadə edir
`ToriiClient` ilə göndərilən iş vaxtı köməkçiləri, belə ki, onları birbaşa kopyalaya bilərsiniz
CLI alətləri, CI qoşquları və ya uzunmüddətli xidmətlər.

Əlavə resurslar:

- `javascript/iroha_js/recipes/governance.mjs` — üçün işlək başdan-uca skript
  təkliflər, bülletenlər və şura rotasiyaları.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — təqdim etmək üçün CLI köməkçisi
  pacs.008/pacs.009 faydalı yüklər və sorğunun deterministik statusu.
- `docs/source/finance/settlement_iso_mapping.md` — kanonik ISO sahə xəritəsi.

## Birləşdirilmiş reseptləri işə salmaq

Bu nümunələr `javascript/iroha_js/recipes/`-dəki skriptlərdən asılıdır. Qaç
`npm install && npm run build:native` əvvəlcədən buna görə yaradılan bağlamalar
mövcuddur.

### İdarəetmə köməkçisi

Zəng etməzdən əvvəl aşağıdakı mühit dəyişənlərini konfiqurasiya edin
`recipes/governance.mjs`:

- `TORII_URL` — Torii son nöqtəsi.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — imzalayan hesabı və açar (hex). Açarları a-da saxlayın
  təhlükəsiz gizli mağaza.
- `CHAIN_ID` — əlavə şəbəkə identifikatoru.
- `GOV_SUBMIT=1` — yaradılan əməliyyatları Torii-ə itələyin.
- `GOV_FETCH=1` — təqdim edildikdən sonra təklifləri/kilidləri əldə edin.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` — istifadə edilən isteğe bağlı axtarışlar
  zaman `GOV_FETCH=1`.

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

Hər addım üçün hashlar qeyd olunur və Torii cavabları görünəndə
`GOV_SUBMIT=1` beləliklə, CI işləri təqdim etmə xətalarında tez uğursuz ola bilər.

### ISO körpü köməkçisi

`recipes/iso_bridge.mjs` ya pacs.008, ya da pacs.009 mesajı və sorğular təqdim edir
statusu həll olunana qədər ISO körpüsü. Onu konfiqurasiya edin:

- `TORII_URL` — ISO körpü API-lərini ifşa edən Torii son nöqtəsi.
- `ISO_MESSAGE_KIND` — `pacs.008` (standart) və ya `pacs.009`. Köməkçi istifadə edir
  uyğun nümunə qurucusu (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  öz XML təmin etmədiyiniz zaman.
- `ISO_MESSAGE_SUFFIX` - nümunə faydalı yük identifikatorlarına əlavə edilmiş isteğe bağlı şəkilçi
  təkrarlanan məşqləri unikal saxlayın (defolt olaraq onaltılıqda cari dövr saniyələri).
- `ISO_CONTENT_TYPE` - təqdimatlar üçün `Content-Type` başlığını ləğv edin
  (məsələn, `application/pacs009+xml`); yalnız sorğu keçirdiyiniz zaman nəzərə alınmır
  mövcud mesaj id.
- `ISO_MESSAGE_ID` - təqdimatı tamamilə atlayın və yalnız təqdim olunanları sorğulayın
  `waitForIsoMessageStatus` vasitəsilə identifikator.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` - gözləmə strategiyasını tənzimləyin
  səs-küylü və ya yavaş körpü yerləşdirmələri.
- `ISO_RESOLVE_ON_ACCEPTED=1` — Torii `Accepted`-i qaytaran kimi çıxın,
  tranzaksiya hash hələ də gözlənilən olsa belə (körpünün təmiri zamanı faydalıdır
  mühasibat uçotu gecikdirildikdə).

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

Torii heç vaxt terminal haqqında məlumat vermirsə, hər iki skript `1` status kodu ilə çıxır
keçid, onları CI qapı işləri üçün uyğun edir.

### ISO ləqəb köməkçisi

`recipes/iso_alias.mjs` ISO ləqəb son nöqtələrini hədəfləyir ki, məşqlər əhatə etsin
sifarişli alətlər yazmadan kor-element hashing və ləqəb axtarışları. Bu
zənglər `ToriiClient.evaluateAliasVoprf` plus `resolveAlias` / `resolveAliasByIndex`
və backend, digest, hesab bağlaması, mənbə və deterministik indeksi çap edir
Torii tərəfindən qaytarıldı.

Ətraf mühit dəyişənləri:

- `TORII_URL` — Torii ləqəb köməkçilərini ifşa edən son nöqtə.
- `ISO_VOPRF_INPUT` — hex kodlu kor element (defolt olaraq `deadbeef`).
- `ISO_SKIP_VOPRF=1` — yalnız axtarışları sınaqdan keçirərkən VOPRF çağırışını atlayın.
- `ISO_ALIAS_LABEL` — həll etmək üçün hərfi ad (məsələn, IBAN tipli sətirlər).
- `ISO_ALIAS_INDEX` — onluq və ya `0x`-prefiksli indeks `resolveAliasByIndex`-ə keçdi.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — təhlükəsiz Torii yerləşdirmələri üçün əlavə başlıqlar.

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

Köməkçi Torii-in davranışını əks etdirir: ləqəblər əskik olduqda 404-ləri göstərir
və CI axınlarının körpüyə dözə bilməsi üçün işləmə zamanı dayandırılmış səhvləri yumşaq atlamalar kimi qəbul edir
təmir pəncərələri.

## İdarəetmə iş axınları

### Müqavilə nümunələrini və təkliflərini yoxlayın

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

### Təkliflər və bülletenlər təqdim edin

Ləğv etmək və ya vaxtla bağlı idarəetmə təqdimatlarını – SDK-nı ləğv etmək lazım olduqda `AbortController` istifadə edin
aşağıda göstərilən hər POST köməkçisi üçün əlavə `{ signal }` obyektini qəbul edir.

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

### Şura VRF və qüvvəyə minməsi

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

## ISO 20022 körpü reseptləri

### Pacs.008 / pacs.009 faydalı yükləri yaradın

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

Bütün identifikatorlar (BIC, LEI, IBAN, ISO məbləği) XML-dən əvvəl doğrulanır.
yaradılmışdır. PvP yaymaq üçün `buildPacs008Message`-i `buildPacs009Message` ilə dəyişdirin
yüklərin maliyyələşdirilməsi.

### ISO mesajlarını göndərin və sorğulayın

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

Həm `resolveOnAccepted`, həm də `resolveOnAcceptedWithoutTransaction` etibarlıdır; hər iki bayraqdan istifadə edin
sorğuları təşkil edərkən `Accepted` statuslarını (əməliyyat hashı olmadan) terminal kimi qəbul etmək.

Körpü heç vaxt a hesabat verərsə, köməkçilər `IsoMessageTimeoutError` atırlar
terminal vəziyyəti. Aşağı səviyyəli `submitIsoPacs008` / `submitIsoPacs009` istifadə edin
xüsusi sorğu məntiqini təşkil etmək lazım olduqda zənglər; `getIsoMessageStatus`
tək vuruşlu axtarışı ifşa edir.

### Əlaqədar səthlər

- `torii.getSorafsPorWeeklyReport("2026-W05")` ISO həftəlik PoR paketini gətirir
  yol xəritəsində istinad edilir və xəbərdarlıqlar üçün gözləmə köməkçilərini təkrar istifadə edə bilər.
- `resolveAlias` / `resolveAliasByIndex` ISO körpü ləqəb bağlarını ifşa edir
  uzlaşma alətləri ödənişi verməzdən əvvəl hesabın sahibliyini sübut edə bilər.