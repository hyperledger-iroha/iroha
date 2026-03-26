---
lang: he
direction: rtl
source: docs/portal/docs/sdks/javascript-governance-iso.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: ממשל ודוגמאות לגשר ISO
תיאור: כונן זרימות עבודה מתקדמות של Torii עם `@iroha/iroha-js`.
slug: /sdks/javascript/governance-iso-examples
---

מדריך שטח זה מרחיב את ההתחלה המהירה על ידי הדגמת ממשל ו
גשר ISO 20022 זורם עם `@iroha/iroha-js`. הקטעים עושים בו שימוש חוזר
עוזרי זמן ריצה הנשלחים עם `ToriiClient`, כך שתוכל להעתיק אותם ישירות לתוך
כלי CLI, רתמות CI או שירותים ארוכי טווח.

משאבים נוספים:

- `javascript/iroha_js/recipes/governance.mjs` - סקריפט מקצה לקצה שניתן להרצה עבור
  הצעות, פתקי הצבעה וסיבובי מועצה.
- `javascript/iroha_js/recipes/iso_bridge.mjs` - עוזר CLI להגשה
  pacs.008/pacs.009 מטענים ומצב דטרמיניסטי של סקרים.
- `docs/source/finance/settlement_iso_mapping.md` — מיפוי שדות ISO קנוני.

## הפעלת המתכונים המצורפים

דוגמאות אלו תלויות בסקריפטים ב-`javascript/iroha_js/recipes/`. רוץ
`npm install && npm run build:native` מראש, כך שהקשרים שנוצרו
זמין.

### הדרכה לעוזר ממשל

הגדר את משתני הסביבה הבאים לפני ההפעלה
`recipes/governance.mjs`:

- `TORII_URL` — נקודת קצה Torii.
- `AUTHORITY` / `PRIVATE_KEY_HEX` - חשבון חותם ומפתח (hex). שמור מפתחות בתוך א
  חנות סודית מאובטחת.
- `CHAIN_ID` - מזהה רשת אופציונלי.
- `GOV_SUBMIT=1` - דחוף את העסקאות שנוצרו ל-Torii.
- `GOV_FETCH=1` - אחזר הצעות/נעילות לאחר ההגשה.
- `GOV_PROPOSAL_ID`, `GOV_REFERENDUM_ID`, `GOV_LOCKS_ID` - נעשה שימוש בחיפושים אופציונליים
  כאשר `GOV_FETCH=1`.

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

גיבוב נרשם עבור כל שלב, ותגובות Torii מופיעות כאשר
`GOV_SUBMIT=1` כך שעבודות CI יכולות להיכשל במהירות על שגיאות הגשה.

### עוזר גשר ISO

`recipes/iso_bridge.mjs` שולח הודעה וסקרים pacs.008 או pacs.009
גשר ה-ISO עד שהמצב ייקבע. הגדר את זה עם:

- `TORII_URL` — נקודת קצה Torii חושפת את ממשקי API של גשר ISO.
- `ISO_MESSAGE_KIND` — `pacs.008` (ברירת מחדל) או `pacs.009`. העוזר משתמש ב-
  בונה מדגם תואם (`buildSamplePacs008Message` / `buildSamplePacs009Message`)
  כאשר אינך מספק XML משלך.
- `ISO_MESSAGE_SUFFIX` - סיומת אופציונלית שצורפה למזהי המטען לדוגמה ל
  שמור על חזרות חוזרות ונשנות ייחודיות (ברירת מחדל היא שניות העידן הנוכחיות בהקסדה).
- `ISO_CONTENT_TYPE` - עוקף את הכותרת `Content-Type` להגשות
  (לדוגמה `application/pacs009+xml`); התעלמו כאשר אתה רק משאל an
  מזהה הודעה קיימת.
- `ISO_MESSAGE_ID` - דלג על ההגשה לגמרי וסקר רק את התוכן שסופק
  מזהה באמצעות `waitForIsoMessageStatus`.
- `ISO_POLL_ATTEMPTS` / `ISO_POLL_INTERVAL_MS` - כוונן את אסטרטגיית ההמתנה עבור
  פריסת גשרים רועשת או איטית.
- `ISO_RESOLVE_ON_ACCEPTED=1` — צא ברגע ש-Torii מחזיר את `Accepted`,
  גם אם ה-hash של העסקה עדיין בהמתנה (שימושי במהלך תחזוקת הגשר
  כאשר ביצוע הפנקס מתעכב).

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

שני הסקריפטים יוצאים עם קוד סטטוס `1` אם Torii אף פעם לא מדווח על מסוף
מעבר, מה שהופך אותם מתאימים לעבודות שער CI.

### עוזר כינוי ISO`recipes/iso_alias.mjs` מכוון לנקודות הקצה הכינוי של ISO כך שהחזרות יכולות לכסות
חיפושי גיבוב של אלמנטים מעוורים וחיפושי כינוי מבלי לכתוב כלי עבודה מותאמים אישית. זה
שיחות `ToriiClient.evaluateAliasVoprf` בתוספת `resolveAlias` / `resolveAliasByIndex`
ומדפיס את הקצה האחורי, התקציר, כריכת החשבון, המקור והאינדקס הדטרמיניסטי
הוחזר על ידי Torii.

משתני סביבה:

- `TORII_URL` — נקודת קצה Torii חושפת את עוזרי הכינוי.
- `ISO_VOPRF_INPUT` - אלמנט מעוור מקודד משושה (ברירת המחדל היא `deadbeef`).
- `ISO_SKIP_VOPRF=1` - דלג על שיחת VOPRF כאשר רק בודקים חיפושים.
- `ISO_ALIAS_LABEL` - כינוי מילולי לפתרון (לדוגמה, מחרוזות בסגנון IBAN).
- `ISO_ALIAS_INDEX` - אינדקס עשרוני או `0x` עם קידומת מועבר ל-`resolveAliasByIndex`.
- `TORII_AUTH_TOKEN` / `TORII_API_TOKEN` - כותרות אופציונליות לפריסות Torii מאובטחות.

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

העוזר משקף את ההתנהגות של Torii: הוא מציג 404s כאשר כינויים חסרים
ומתייחס לשגיאות מושבתות בזמן ריצה כדילוגים רכים כך שזרימות CI יכולות לסבול גשר
חלונות תחזוקה.

## זרימות עבודה של ממשל

### בדוק מקרים והצעות חוזה

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

### הגשת הצעות ופתקי הצבעה

השתמש ב-`AbortController` כאשר אתה צריך לבטל הגשות ניהול מוגבלות בזמן - ה-SDK
מקבל אובייקט `{ signal }` אופציונלי עבור כל עוזר POST המוצג להלן.

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

### VRF וחקיקה של המועצה

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

## מתכוני גשר ISO 20022

### בניית עומסי pacs.008 / pacs.009

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

כל המזהים (BIC, LEI, IBAN, כמות ISO) מאומתים לפני ה-XML
נוצר. החלף את `buildPacs008Message` עבור `buildPacs009Message` כדי לפלוט PvP
מטעני מימון.

### שלח וסקר הודעות ISO

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

גם `resolveOnAccepted` וגם `resolveOnAcceptedWithoutTransaction` תקפים; השתמש בשני הדגלים
להתייחס לסטטוסים של `Accepted` (ללא גיבוב עסקה) כאל מסוף בעת תזמור סקרים.

העוזרים זורקים `IsoMessageTimeoutError` אם הגשר אף פעם לא מדווח על א
מצב טרמינלי. השתמש ב-`submitIsoPacs008` / `submitIsoPacs009` ברמה נמוכה יותר
שיחות כאשר אתה צריך לתזמר היגיון סקרים מותאם אישית; `getIsoMessageStatus`
חושף בדיקת צילום בודדת.

### משטחים קשורים

- `torii.getSorafsPorWeeklyReport("2026-W05")` מביא את חבילת ISO-week PoR
  מוזכר במפת הדרכים ויכול לעשות שימוש חוזר בעוזרי ההמתנה להתראות.
- `resolveAlias` / `resolveAliasByIndex` חושפים כריכות כינויים של גשר ISO כך
  כלי התאמה יכולים להוכיח בעלות על חשבון לפני הוצאת תשלום.