---
lang: ar
direction: rtl
source: docs/portal/docs/sdks/javascript-governance-iso.ja.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: أمثلة على الحوكمة وجسر ISO
الوصف: قم بقيادة مسارات عمل Torii المتقدمة باستخدام `@iroha/iroha-js`.
سبيكة: /sdks/javascript/governance-iso-examples
---

يتوسع هذا الدليل الميداني في البداية السريعة من خلال إظهار الحوكمة و
يتدفق جسر ISO&nbsp;20022 مع `@iroha/iroha-js`. المقتطفات تعيد استخدام نفس الشيء
مساعدات وقت التشغيل التي تأتي مع `ToriiClient`، بحيث يمكنك نسخها مباشرة إلى
أدوات CLI، أو أدوات CI، أو الخدمات طويلة الأمد.

موارد إضافية:

- `javascript/iroha_js/recipes/governance.mjs` — برنامج نصي شامل قابل للتشغيل لـ
  المقترحات والاقتراع وتناوب المجالس.
- `javascript/iroha_js/recipes/iso_bridge.mjs` — مساعد CLI للإرسال
  pacs.008/pacs.009 الحمولات والحالة الحتمية للاقتراع.
- `docs/source/finance/settlement_iso_mapping.md` - رسم خرائط مجال ISO الأساسي.

## تشغيل الوصفات المجمعة

تعتمد هذه الأمثلة على البرامج النصية الموجودة في `javascript/iroha_js/recipes/`. تشغيل
`npm install && npm run build:native` مسبقًا لذا فإن الارتباطات التي تم إنشاؤها هي
متاح.

### الإرشادات التفصيلية لمساعد الحوكمة

قم بتكوين متغيرات البيئة التالية قبل الاستدعاء
`recipes/governance.mjs`:- `TORII_URL` — نقطة النهاية Torii.
- `AUTHORITY` / `PRIVATE_KEY_HEX` — حساب المُوقع والمفتاح (ست عشري). احتفظ بالمفاتيح في أ
  مخزن سري آمن.
- `CHAIN_ID` - معرف الشبكة الاختياري.
- `GOV_SUBMIT=1` - ادفع المعاملات التي تم إنشاؤها إلى Torii.
- `GOV_FETCH=1` — جلب المقترحات/الأقفال بعد الإرسال.
- `GOV_PROPOSAL_ID`، `GOV_REFERENDUM_ID`، `GOV_LOCKS_ID` — عمليات البحث الاختيارية المستخدمة
  عندما `GOV_FETCH=1`.

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

يتم تسجيل التجزئة لكل خطوة، وتظهر استجابات Torii عندما
`GOV_SUBMIT=1` لذلك يمكن أن تفشل مهام CI بسرعة عند حدوث أخطاء في الإرسال.

### مساعد جسر ISO

يقوم `recipes/iso_bridge.mjs` بإرسال رسالة واستطلاعات رأي pacs.008 أو pacs.009
جسر ISO حتى تستقر الحالة. قم بتكوينه باستخدام:- `TORII_URL` — نقطة النهاية Torii التي تعرض واجهات برمجة تطبيقات جسر ISO.
- `ISO_MESSAGE_KIND` — `pacs.008` (افتراضي) أو `pacs.009`. يستخدم المساعد
  منشئ العينات المطابق (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  عندما لا تقوم بتوفير XML الخاص بك.
- `ISO_MESSAGE_SUFFIX` — لاحقة اختيارية ملحقة بمعرفات الحمولة الصافية النموذجية
  احتفظ بالتدريبات المتكررة فريدة من نوعها (الإعدادات الافتراضية للثواني الحالية بالنظام السداسي).
- `ISO_CONTENT_TYPE` - تجاوز رأس `Content-Type` لعمليات الإرسال
  (على سبيل المثال `application/pacs009+xml`)؛ يتم تجاهله عند إجراء استطلاع للرأي فقط
  معرف الرسالة الموجودة
- `ISO_MESSAGE_ID` - تخطي الإرسال تمامًا واستقصاء ما تم توفيره فقط
  المعرف عبر `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` — ضبط استراتيجية الانتظار لـ
  عمليات نشر الجسر الصاخبة أو البطيئة.
- `ISO_RESOLVE_ON_ACCEPTED=1` — الخروج بمجرد إرجاع Torii إلى `Accepted`،
  حتى لو كانت تجزئة المعاملة لا تزال معلقة (مفيدة أثناء صيانة الجسر
  عندما يتأخر التزام دفتر الأستاذ).

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

يتم إنهاء كلا البرنامجين النصيين برمز الحالة `1` إذا لم يبلغ Torii عن أي محطة طرفية مطلقًا
الانتقال، مما يجعلها مناسبة لوظائف بوابة CI.

### مساعد الاسم المستعار ISOيستهدف `recipes/iso_alias.mjs` نقاط نهاية الاسم المستعار ISO حتى يمكن تغطية التدريبات
تجزئة العناصر العمياء وعمليات البحث عن الأسماء المستعارة دون كتابة أدوات مخصصة. ذلك
يستدعي `ToriiClient.evaluateAliasVoprf` بالإضافة إلى `resolveAlias` / `resolveAliasByIndex`
ويطبع الواجهة الخلفية والملخص وربط الحساب والمصدر والفهرس الحتمي
تم إرجاعها بواسطة Torii.

متغيرات البيئة:

- `TORII_URL` — نقطة النهاية Torii تكشف عن الأسماء المستعارة للمساعدين.
- `ISO_VOPRF_INPUT` - عنصر أعمى بتشفير سداسي عشري (الإعداد الافتراضي هو `deadbeef`).
- `ISO_SKIP_VOPRF=1` - تخطي استدعاء VOPRF عند اختبار عمليات البحث فقط.
- `ISO_ALIAS_LABEL` - الاسم المستعار الحرفي المطلوب حله (على سبيل المثال، سلاسل بنمط IBAN).
- `ISO_ALIAS_INDEX` - تم تمرير الفهرس العشري أو البادئة `0x` إلى `resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` — رؤوس اختيارية لعمليات نشر Torii الآمنة.

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

يعكس المساعد سلوك Torii: فهو يظهر 404s عندما تكون الأسماء المستعارة مفقودة
ويتعامل مع أخطاء تعطيل وقت التشغيل على أنها تخطيات بسيطة حتى تتمكن تدفقات CI من تحمل الجسر
نوافذ الصيانة.

## سير عمل الحوكمة

### فحص مثيلات العقد والمقترحات

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

### تقديم المقترحات وصناديق الاقتراع

استخدم `AbortController` عندما تحتاج إلى الإلغاء أو عمليات إرسال الإدارة المقيدة بفترة زمنية — SDK
يقبل كائن `{ signal }` اختياري لكل مساعد POST الموضح أدناه.

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

### مجلس VRF وسنه

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

## وصفات الجسر ISO&nbsp;20022### بناء الحمولات pacs.008 / pacs.009

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

يتم التحقق من صحة جميع المعرفات (BIC، وLEI، وIBAN، ومبلغ ISO) قبل التحقق من XML
ولدت. قم بتبديل `buildPacs008Message` بـ `buildPacs009Message` لإصدار حماية الأصناف النباتية
حمولات التمويل.

### إرسال واستطلاع رسائل ISO

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

كل من `resolveOnAccepted` و`resolveOnAcceptedWithoutTransaction` صالحان؛ استخدم أيًا من العلمين
للتعامل مع حالات `Accepted` (بدون تجزئة المعاملة) كمحطة طرفية عند تنسيق الاستقصاءات.

يقوم المساعدون بإلقاء `IsoMessageTimeoutError` إذا لم يبلغ الجسر أبدًا عن
الحالة النهائية. استخدم المستوى الأدنى `submitIsoPacs008` / `submitIsoPacs009`
المكالمات عندما تحتاج إلى تنسيق منطق الاقتراع المخصص؛ `getIsoMessageStatus`
يكشف عن بحث طلقة واحدة.

### الأسطح ذات الصلة

- `torii.getSorafsPorWeeklyReport("2026-W05")` يجلب حزمة ISO-week PoR
  المشار إليها في خريطة الطريق ويمكن إعادة استخدام مساعدي الانتظار للتنبيهات.
- يعرض `resolveAlias` / `resolveAliasByIndex` روابط الاسم المستعار لجسر ISO لذلك
  يمكن لأدوات التسوية إثبات ملكية الحساب قبل إصدار الدفع.