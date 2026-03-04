---
lang: ur
direction: rtl
source: docs/portal/docs/sdks/javascript-governance-iso.he.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
عنوان: گورننس اور آئی ایس او برج کی مثالیں
تفصیل: `@iroha/iroha-js` کے ساتھ ایڈوانسڈ Torii ایکس ورک فلوز ڈرائیو کریں۔
سلگ:/ایس ڈی کے/جاوا اسکرپٹ/گورننس-آئی ایس او مثال
---

یہ فیلڈ گائیڈ گورننس کا مظاہرہ کرکے کوئیک اسٹارٹ پر پھیلتا ہے
ISO & nbsp ؛ 20022 برج `@iroha/iroha-js` کے ساتھ بہتا ہے۔ ٹکڑوں کو دوبارہ استعمال کیا جاتا ہے
رن ٹائم مددگار جو `ToriiClient` کے ساتھ جہاز بھیجتے ہیں ، تاکہ آپ انہیں براہ راست کاپی کرسکیں
سی ایل آئی ٹولنگ ، سی آئی ہارنس ، یا طویل عرصے سے چلنے والی خدمات۔

اضافی وسائل:

-`javascript/iroha_js/recipes/governance.mjs`-رننابل اختتام سے آخر میں اسکرپٹ کے لئے
  تجاویز ، بیلٹ اور کونسل کی گردشیں۔
- `javascript/iroha_js/recipes/iso_bridge.mjs` - جمع کرانے کے لئے CLI مددگار
  PACS.008/PACS.009 پے لوڈز اور پولنگ ڈٹرمینسٹک حیثیت۔
- `docs/source/finance/settlement_iso_mapping.md` - کیننیکل آئی ایس او فیلڈ میپنگ۔

## بنڈل ترکیبیں چلاتے ہیں

ان مثالوں کا انحصار `javascript/iroha_js/recipes/` میں اسکرپٹ پر ہوتا ہے۔ چلائیں
`npm install && npm run build:native` پہلے ہی پیدا کیا گیا ہے
دستیاب ہے۔

### گورننس ہیلپر واک تھرو

درخواست کرنے سے پہلے مندرجہ ذیل ماحولیاتی متغیرات کو تشکیل دیں
`recipes/governance.mjs`:

- `TORII_URL` - Torii اختتامی نقطہ۔
- `AUTHORITY` / `PRIVATE_KEY_HEX` - اکاؤنٹ سائنر اور کلید (ہیکس)۔ چابیاں ایک میں رکھیں
  محفوظ خفیہ اسٹور۔
- `CHAIN_ID` - اختیاری نیٹ ورک شناخت کنندہ۔
- `GOV_SUBMIT=1` - پیدا شدہ لین دین کو Torii پر دبائیں۔
- `GOV_FETCH=1` - جمع کرانے کے بعد تجاویز/تالے بازیافت کریں۔
- `GOV_PROPOSAL_ID` ، `GOV_REFERENDUM_ID` ، `GOV_LOCKS_ID` - اختیاری تلاش استعمال کی گئی
  جب `GOV_FETCH=1`۔

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

ہیش ہر مرحلے کے لئے لاگ ان ہوتے ہیں ، اور جب Torii ردعمل سامنے آتے ہیں
`GOV_SUBMIT=1` تاکہ CI ملازمتیں جمع کرانے کی غلطیوں پر تیزی سے ناکام ہوسکیں۔

### ISO برج مددگار

`recipes/iso_bridge.mjs` یا تو PACS.008 یا PACS.009 پیغام اور پولز پیش کرتا ہے
آئی ایس او پل جب تک کہ حیثیت طے نہیں ہوتی ہے۔ اس کے ساتھ تشکیل دیں:

- `TORII_URL` - Torii اختتامی نقطہ ISO برج APIs کو بے نقاب کررہا ہے۔
- `ISO_MESSAGE_KIND` - `pacs.008` (پہلے سے طے شدہ) یا `pacs.009`۔ مددگار استعمال کرتا ہے
  مماثل نمونہ بلڈر (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  جب آپ اپنا XML فراہم نہیں کرتے ہیں۔
- `ISO_MESSAGE_SUFFIX` - نمونہ پے لوڈ IDs میں شامل اختیاری لاحقہ
  بار بار ریہرسلوں کو منفرد رکھیں (ہیکس میں موجودہ دور کے سیکنڈوں سے پہلے سے طے شدہ)۔
- `ISO_CONTENT_TYPE` - گذارشات کے لئے `Content-Type` ہیڈر کو اوور رائڈ کریں
  (مثال کے طور پر `application/pacs009+xml`) ؛ جب آپ صرف ایک پولنگ کرتے ہیں تو نظرانداز کیا جاتا ہے
  موجودہ پیغام ID۔
- `ISO_MESSAGE_ID` - جمع کرانے کو مکمل طور پر چھوڑیں اور صرف سپلائی شدہ رائے شماری کریں
  `waitForIsoMessageStatus` کے ذریعے شناخت کنندہ۔
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` - انتظار کی حکمت عملی کے لئے ٹیون کریں
  شور یا سست پل کی تعیناتی۔
- `ISO_RESOLVE_ON_ACCEPTED=1` - جیسے ہی Torii واپس آنے سے `Accepted` ،
  یہاں تک کہ اگر ٹرانزیکشن ہیش ابھی بھی زیر التوا ہے (پل کی بحالی کے دوران آسان)
  جب لیجر کمٹ میں تاخیر ہوتی ہے)۔

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

دونوں اسکرپٹس اسٹیٹس کوڈ `1` کے ساتھ باہر نکلیں اگر Torii کبھی ٹرمینل کی اطلاع نہیں دیتا ہے
منتقلی ، انہیں سی آئی گیٹ کی نوکریوں کے ل suitable موزوں بنا۔

### ISO عرف مددگار`recipes/iso_alias.mjs` اہداف ISO عرف کے اختتامی نکات کو تاکہ ریہرسلوں کا احاطہ کیا جاسکے
بلائنڈ عنصر ہیشنگ اور عرف کی تلاش کے بغیر بیسپوک ٹولنگ لکھے بغیر۔ یہ
کالز `ToriiClient.evaluateAliasVoprf` پلس `resolveAlias` / `resolveAliasByIndex`
اور پسدید ، ڈائجسٹ ، اکاؤنٹ بائنڈنگ ، ماخذ ، اور عین مطابق اشاریہ پرنٹ کرتا ہے
Torii کے ذریعہ واپس آیا۔

ماحولیاتی متغیرات:

- `TORII_URL` - Torii اختتامی نقطہ عرف مددگاروں کو بے نقاب کررہا ہے۔
- `ISO_VOPRF_INPUT`- ہیکس انکوڈڈ بلائنڈڈ عنصر (`deadbeef` سے پہلے سے طے شدہ)۔
- `ISO_SKIP_VOPRF=1` - جب صرف جانچ پڑتال کرنے کی جانچ پڑتال کرتے ہو تو Voprf کال کو چھوڑ دیں۔
- `ISO_ALIAS_LABEL`- حل کرنے کے لئے لفظی عرف (جیسے ، ابن طرز کے تار)۔
- `ISO_ALIAS_INDEX`- اعشاریہ یا `0x`-prefixed انڈیکس `resolveAliasByIndex` میں منتقل ہوا۔
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` - محفوظ Torii تعیناتیوں کے لئے اختیاری ہیڈر۔

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

مددگار آئینہ Torii کا طرز عمل: جب 404s کی سطحیں آتی ہیں جب عرفی لاپتہ ہوتے ہیں
اور رن ٹائم ڈس ایبلڈ غلطیوں کا علاج کرتا ہے جیسا کہ نرم اسکیپس ہے تاکہ سی آئی فلو پل کو برداشت کرسکے
بحالی ونڈوز

## گورننس ورک فلوز

### معاہدے کے واقعات اور تجاویز کا معائنہ کریں

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

### تجاویز اور بیلٹ جمع کروائیں

جب آپ کو منسوخ کرنے کی ضرورت ہو یا وقت پر پابند گورننس گذارشات-SDK
ذیل میں دکھائے گئے ہر پوسٹ مددگار کے لئے ایک اختیاری `{ signal }` آبجیکٹ کو قبول کرتا ہے۔

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

### کونسل وی آر ایف اور نفاذ

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

## iso & nbsp ؛ 20022 پل کی ترکیبیں

### PACS.008 / PACS.009 پے لوڈ بنائیں

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

XML ہونے سے پہلے تمام شناخت کنندگان (BIC ، LEI ، IBAN ، ISO رقم) کی توثیق کی جاتی ہے
پیدا ہوا `buildPacs009Message` برائے PVP خارج کرنے کے لئے `buildPacs008Message` کو تبدیل کریں
فنڈنگ ​​پے لوڈ۔

### جمع کروائیں اور آئی ایس او پیغامات

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

دونوں `resolveOnAccepted` اور `resolveOnAcceptedWithoutTransaction` درست ہیں۔ یا تو جھنڈا استعمال کریں
آرکسٹریٹنگ پولز جب ٹرمینل کے طور پر `Accepted` اسٹیٹس (ٹرانزیکشن ہیش کے بغیر) کے ساتھ سلوک کرنے کے لئے۔

اگر پل کبھی بھی اطلاع نہیں دیتا ہے تو مددگار `IsoMessageTimeoutError` پھینک دیتے ہیں
ٹرمینل ریاست۔ نچلے درجے کے `submitIsoPacs008` / `submitIsoPacs009` استعمال کریں
کالز جب آپ کو کسٹم پولنگ منطق کو آرکیسٹریٹ کرنے کی ضرورت ہوتی ہے۔ `getIsoMessageStatus`
ایک شاٹ تلاش کو بے نقاب کرتا ہے۔

### متعلقہ سطحیں

- `torii.getSorafsPorWeeklyReport("2026-W05")` ISO-ہفتہ POR بنڈل لاتا ہے
  روڈ میپ میں حوالہ دیا گیا ہے اور انتباہات کے لئے انتظار کرنے والے مددگاروں کا دوبارہ استعمال کرسکتا ہے۔
- `resolveAlias` / `resolveAliasByIndex` ISO برج عرف بائنڈنگز کو بے نقاب کریں
  مفاہمت کے اوزار ادائیگی جاری کرنے سے پہلے اکاؤنٹ کی ملکیت ثابت کرسکتے ہیں۔