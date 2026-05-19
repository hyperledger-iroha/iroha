<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/source/universal_accounts_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 09a308ecbf07f0293add7f35cf4f1a50b5e6d3630b8b37a8f0f45a7cf82d3924
source_last_modified: "2026-03-30T18:22:55.987822+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# מדריך חשבון אוניברסלי

מדריך זה מזקק את דרישות ההשקה של UAID (זיהוי חשבון אוניברסלי).
מפת הדרכים Nexus ואורזת אותם בהדרכה ממוקדת מפעיל + SDK.
זה מכסה גזירת UAID, בדיקת תיק/מניפסט, תבניות רגולטור,
והראיות שחייבות ללוות כל מניפסט של ספריית חלל של אפליקציה של iroha
publish` run (roadmap reference: `roadmap.md:2209`).

## 1. התייחסות מהירה ל-UAID- UAIDs הם `uaid:<hex>` מילוליים כאשר `<hex>` הוא Blake2b-256 digest שלו
  LSB מוגדר ל-`1`. הטיפוס הקנוני חי בו
  `crates/iroha_data_model/src/nexus/manifest.rs::UniversalAccountId`.
- רשומות חשבון (`Account` ו-`AccountDetails`) נושאות כעת `uaid` אופציונלי
  כדי שיישומים יוכלו ללמוד את המזהה ללא hashing מותאם אישית.
- מדיניות מזהה של פונקציה נסתרת יכולה לאגד תשומות מנורמלות שרירותיות
  (מספרי טלפון, מיילים, מספרי חשבונות, מחרוזות שותפים) למזהי `opaque:`
  תחת מרחב שמות של UAID. החלקים בשרשרת הם `IdentifierPolicy`,
  `IdentifierClaimRecord`, ואינדקס `opaque_id -> uaid`.
- Space Directory שומרת על מפת `World::uaid_dataspaces` הקושרת כל UAID
  לחשבונות מרחב הנתונים שאליהם מתייחסים מניפסטים פעילים. Torii עושה שימוש חוזר בזה
  מפה עבור `/portfolio` ו-`/uaids/*` ממשקי API.
- `POST /v1/accounts/onboard` מפרסם מניפסט ברירת מחדל של ספריית שטח עבור
  מרחב הנתונים הגלובלי כאשר אף אחד לא קיים, כך שה-UAID מאוגד באופן מיידי.
  רשויות ההטמעה חייבות להחזיק ב-`CanPublishSpaceDirectoryManifest{dataspace=0}`.
- כל ערכות ה-SDK חושפות עוזרים לקנוניזציה של מילות UAID (למשל,
  `UaidLiteral` ב-Android SDK). העוזרים מקבלים עיכובים גולמיים של 64 הקס
  (LSB=1) או `uaid:<hex>` ליטרלים ושימוש חוזר באותם Norito codec
  תקציר לא יכול להיסחף על פני שפות.

## 1.1 מדיניות מזהה מוסתר

UAIDs הם כעת העוגן לשכבת זהות שנייה:- `IdentifierPolicyId` גלובלי (`<kind>#<business_rule>`) מגדיר את
  מרחב שמות, מטא-נתונים של מחויבות ציבורית, מפתח אימות פותר וה-
  מצב נורמליזציה של קלט קנוני (`Exact`, `LowercaseTrimmed`,
  `PhoneE164`, `EmailAddress`, או `AccountNumber`).
- תביעה מחייבת מזהה `opaque:` נגזר אחד בדיוק ל-UAID אחד ואחד
  canonical `AccountId` תחת מדיניות זו, אך הרשת מקבלת רק את
  תביעה כאשר היא מלווה בחתימה `IdentifierResolutionReceipt`.
- הרזולוציה נשארת זרימה של `resolve -> transfer`. Torii פותר את האטום
  לטפל ולהחזיר את `AccountId` הקנוני; העברות עדיין מכוונות את
  חשבון קנוני, לא מילולי `uaid:` או `opaque:` ישירות.
- מדיניות יכולה כעת לפרסם פרמטרי הצפנת קלט BFV באמצעות
  `PolicyCommitment.public_parameters`. כאשר הם קיימים, Torii מפרסם אותם ב-
  `GET /v1/identifier-policies`, ולקוחות יכולים להגיש קלט עטוף BFV
  במקום טקסט רגיל. מדיניות מתוכנתת עוטפת את פרמטרי BFV ב-a
  חבילת `BfvProgrammedPublicParameters` קנונית המפרסמת גם את
  ציבורי `ram_fhe_profile`; עומסי BFV גולמיים מדור קודם משודרגים לשם כך
  צרור קנוני כאשר ההתחייבות נבנית מחדש.
- מסלולי המזהה עוברים דרך אותו Torii אסימון גישה ומגבלת תעריף
  בדיקות כנקודות קצה אחרות הפונות לאפליקציה. הם לא מעקף סביב הרגיל
  מדיניות API.

## 1.2 טרמינולוגיה

פיצול השמות הוא מכוון:- `ram_lfe` היא ההפשטה החיצונית של פונקציה נסתרת. זה מכסה את הפוליסה
  רישום, התחייבויות, מטא נתונים ציבוריים, קבלות ביצוע, וכן
  מצב אימות.
- `BFV` היא ערכת ההצפנה ההוממורפית Brakerski/Fan-Vercauteren המשמשת על ידי
  חלק מהקצה האחורי של `ram_lfe` להערכת קלט מוצפן.
- `ram_fhe_profile` הוא מטא נתונים ספציפיים ל-BFV, לא שם שני עבור כולו
  תכונה. הוא מתאר את מכונת הביצוע המתוכנתת של BFV שארנקים ו
  על המאמתים למקד כאשר מדיניות משתמשת ב-backend המתוכנת.

במונחים קונקרטיים:

- `RamLfeProgramPolicy` ו-`RamLfeExecutionReceipt` הם סוגי שכבת LFE.
- `BfvParameters`, `BfvCiphertext`, `BfvProgrammedPublicParameters`, וכן
  `BfvRamProgramProfile` הם סוגי שכבת FHE.
- `HiddenRamFheProgram` ו-`HiddenRamFheInstruction` הם שמות פנימיים עבור
  תוכנית ה-BFV הנסתרת שמבוצעת על ידי ה-backend המתוכנת. הם נשארים על
  צד FHE כי הם מתארים את מנגנון הביצוע המוצפן במקום
  המדיניות החיצונית או הפשטת הקבלה.

## 1.3 זהות חשבון לעומת כינויים

השקת חשבון אוניברסלית אינה משנה את מודל הזהות הקנוני של החשבון:- `AccountId` נשאר נושא החשבון הקנוני ללא דומיין.
- ערכי `AccountAlias` הם כריכות SNS נפרדות על הנושא הזה. א
  כינוי מוסמך לתחום כגון `merchant@banka.paynet` וכינוי בסיס נתונים מרחבי נתונים
  כגון `merchant@paynet` יכולים שניהם לפתור לאותו `AccountId` הקנוני.
- רישום חשבון קנוני הוא תמיד `Account::new(AccountId)` /
  `NewAccount::new(AccountId)`; אין תחום מוסמך או תחום מממש
  נתיב הרישום.
- בעלות על דומיין, הרשאות כינוי והתנהגויות אחרות בהיקף של דומיין בזמן אמת
  במצב ובממשקי ה-API שלהם ולא על זהות החשבון עצמו.
- חיפוש החשבון הציבורי עוקב אחר הפיצול הזה: שאילתות כינוי נשארות ציבוריות, בעוד
  זהות חשבון קנוני נשארת `AccountId` טהורה.

כלל יישום עבור אופרטורים, SDKs ובדיקות: התחל מהקנוני
`AccountId`, ולאחר מכן הוסף חכירות כינוי, הרשאות מרחב נתונים/דומיין וכל
מדינה בבעלות דומיין בנפרד. אל תסנתז חשבון מזויף שמקורו בכינויים
או לצפות לכל שדה של דומיין מקושר ברשומות החשבון רק בגלל כינוי או
המסלול נושא קטע תחום.

Current Torii routes:

| Route | Purpose |
|-------|---------|
| `GET /v1/ram-lfe/program-policies` | Lists active and inactive RAM-LFE program policies plus their public execution metadata, including optional BFV `input_encryption` parameters and the programmed-backend `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | Accepts `{ encrypted_input }` only and returns the stateless `RamLfeExecutionReceipt`, `{ output_ciphertext, output_hash, receipt_hash }`, and no plaintext output. The current Torii runtime issues receipts for the programmed BFV backend. |
| `POST /v1/ram-lfe/receipts/verify` | Statelessly validates a `RamLfeExecutionReceipt` against the published on-chain program policy and optionally checks that a caller-supplied encrypted `output_hex` matches the receipt `output_hash`. |
| `GET /v1/identifier-policies` | Lists active and inactive hidden-function policy namespaces plus their public metadata, including optional BFV `input_encryption` parameters, the required `normalization` mode for encrypted client-side input, and `ram_fhe_profile` for programmed BFV policies. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | Accepts `{ policy_id, encrypted_input, output_opening }`. The BFV `encrypted_input` must already be normalized according to the published policy mode. The endpoint derives the `opaque:` handle from the verified external `RamLfeOutputOpening` and returns a signed receipt that `ClaimIdentifier` can submit on-chain. |
| `POST /v1/identifiers/resolve` | Accepts `{ policy_id, encrypted_input, output_opening }`. The endpoint re-evaluates the encrypted input, verifies the external output opening, derives the `opaque:` handle from the opened output hash, and returns a nested `{ payload, attestation }` receipt when an active claim exists. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | Looks up the persisted `IdentifierClaimRecord` bound to a deterministic receipt hash so operators and SDKs can audit claim ownership or diagnose replay / mismatch failures without scanning the full identifier index. |

Torii's in-process execution runtime is configured under
`torii.ram_lfe.programs[*]`, keyed by `program_id`. The identifier routes now
reuse that same RAM-LFE runtime instead of a separate `identifier_resolver`
config surface.

Current SDK support:

- `normalizeIdentifierInput(value, normalization)` matches the Rust
  canonicalizers for `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, and `account_number`.
- `ToriiClient.listIdentifierPolicies()` lists policy metadata, including BFV
  input-encryption metadata when the policy publishes it, plus a decoded
  BFV parameter object via `input_encryption_public_parameters_decoded`.
  Programmed policies also expose the decoded `ram_fhe_profile`. That field is
  intentionally BFV-scoped: it lets wallets verify the expected register
  count, lane count, canonicalization mode, and minimum ciphertext modulus for
  the programmed FHE backend before encrypting client-side input.
- `getIdentifierBfvPublicParameters(policy)` and
  `buildIdentifierRequestForPolicy(policy, { encryptedInput | input,
  encrypt: true, outputOpening })` help JS callers consume published BFV
  metadata and build policy-aware encrypted request bodies without
  reimplementing policy-id and normalization rules.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` and
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true,
  outputOpening })` now let JS wallets construct the full BFV Norito
  ciphertext envelope locally from published policy parameters instead of
  shipping prebuilt ciphertext hex.
- `ToriiClient.resolveIdentifier({ policyId, encryptedInput, outputOpening })`
  resolves a hidden identifier and returns the signed nested
  `{ payload, attestation }` receipt.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId,
  encryptedInput, outputOpening })` issues the signed receipt needed by
  `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` verifies the returned
  receipt against the policy resolver key on the client side, and
  `ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` fetches the
  persisted claim record for later audit/debug flows.
- `IrohaSwift.ToriiClient` now exposes `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:encryptedInputHex:outputOpening:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:encryptedInputHex:outputOpening:)`,
  and `getIdentifierClaimByReceiptHash(_)`, plus
  `ToriiIdentifierNormalization` for the same phone/email/account-number
  canonicalization modes.
- `ToriiIdentifierLookupRequest` and encrypted request helpers provide the
  typed Swift request surface for resolve and claim-receipt calls, and Swift
  policies can now derive the BFV ciphertext locally via `encryptInput(...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` validates that
  the top-level receipt fields match the signed payload and verifies the
  resolver signature client-side before submission.
- `HttpClientTransport` in the Android SDK now exposes
  `listIdentifierPolicies()`, encrypted-only `resolveIdentifier(...)`,
  encrypted-only `issueIdentifierClaimReceipt(...)`, and
  `getIdentifierClaimByReceiptHash(...)`,
  plus `IdentifierNormalization` for the same canonicalization rules.
- `IdentifierResolveRequest` and encrypted request helpers provide the typed
  Android request surface, while `IdentifierPolicySummary.encryptInput(...)`
  derives the BFV ciphertext envelope locally from published policy
  parameters.
  `IdentifierResolutionReceipt.verifySignature(policy)` verifies the returned
  resolver signature client-side.

Current instruction set:

- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (receipt-bound; raw `opaque_id` claims are rejected)
- `RevokeIdentifier`

Three backends now exist in `iroha_crypto::ram_lfe`:

- the historical commitment-bound `HKDF-SHA3-512` PRF, and
- a BFV-backed secret affine evaluator that consumes BFV-encrypted identifier
  slots directly. When `iroha_crypto` is built with the default
  `bfv-accel` feature, BFV ring multiplication uses an exact deterministic
  CRT-NTT backend internally; disabling that feature falls back to the
  scalar schoolbook path with identical outputs, and
- a BFV-backed secret programmed evaluator that derives an instruction-driven
  RAM-style execution trace over encrypted registers and ciphertext memory
  lanes before deriving the opaque identifier and receipt hash. The programmed
  backend now requires a stronger BFV modulus floor than the affine path, and
  its public parameters are published in a canonical bundle that includes the
  RAM-FHE execution profile consumed by wallets and verifiers.

Here BFV means the Brakerski/Fan-Vercauteren FHE scheme implemented in
`crates/iroha_crypto/src/fhe_bfv.rs`. It is the encrypted-execution mechanism
used by the affine and programmed backends, not the name of the outer hidden
function abstraction.

Torii uses the backend published by the policy commitment. For the first
release, RAM-LFE and hidden-identifier routes are encrypted-only: Torii does
not accept plaintext inputs, does not hold BFV secret keys, and does not
decrypt input or output ciphertexts. Identifier claim and resolve requests must
include an externally signed `RamLfeOutputOpening`; the `opaque:` identifier is
derived from the verified opened-output hash, not from Torii-side plaintext or
from the ciphertext hash alone.

## 2. גזירת ואימות UAIDs

ישנן שלוש דרכים נתמכות להשיג UAID:

1. **קרא אותו מדגמי המדינה או SDK.** כל `Account`/`AccountDetails`
   מטען שנשאל באמצעות Torii מאוכלס כעת בשדה `uaid` כאשר
   המשתתף הצטרף לחשבונות אוניברסליים.
2. **שאילתה ברישום UAID.** Torii חושף
   `GET /v1/space-directory/uaids/{uaid}` שמחזירה את כריכות מרחב הנתונים
   ומטא-נתונים מניפסטים שהמארח של ספריית החלל נמשך (ראה
   `docs/space-directory.md` §3 עבור דוגמאות מטען).
3. **הפק את זה באופן דטרמיניסטי.** בעת אתחול של UAIDs חדשים במצב לא מקוון, hash
   המשתתף הקנוני משחזר את Blake2b-256 ותחיל את התוצאה עם הקידומת
   `uaid:`. הקטע למטה משקף את העוזר שתועד בו
   `docs/space-directory.md` §3.3:

   ```python
   import hashlib
   seed = b"participant@example"  # canonical address/domain seed
   digest = bytearray(hashlib.blake2b(seed, digest_size=32).digest())
   digest[-1] |= 1
   print(f"uaid:{digest.hex()}")
   ```אחסן תמיד את המילולי באותיות קטנות ונרמל את הרווח הלבן לפני הגיבוב.
עוזרי CLI כגון `iroha app space-directory manifest scaffold` והאנדרואיד
מנתח `UaidLiteral` מיישם את אותם כללי חיתוך כדי שביקורות ניהול יכולות
צלב ערכים ללא סקריפטים אד-הוק.

## 3. בדיקת אחזקות ומניפסטים של UAID

אגרגטור התיק הדטרמיניסטי ב-`iroha_core::nexus::portfolio`
מציג כל זוג נכס/מרחב נתונים שמתייחס ל-UAID. אופרטורים ו-SDKs
יכול לצרוך את הנתונים דרך המשטחים הבאים:

| משטח | שימוש |
|--------|-------|
| `GET /v1/accounts/{uaid}/portfolio` | מחזירה מרחב נתונים ← נכס ← סיכומי יתרה; מתואר ב-`docs/source/torii/portfolio_api.md`. |
| `GET /v1/space-directory/uaids/{uaid}` | מפרט מזהי מרחבי נתונים + מילולי חשבון הקשורים ל-UAID. |
| `GET /v1/space-directory/uaids/{uaid}/manifests` | מספק את ההיסטוריה המלאה של `AssetPermissionManifest` עבור ביקורת. |
| `iroha app space-directory bindings fetch --uaid <literal>` | קיצור דרך CLI שעוטף את נקודת הקצה של bindings וכותב אופציונלי את ה-JSON לדיסק (`--json-out`). |
| `iroha app space-directory manifest fetch --uaid <literal> --json-out <path>` | מביא את חבילת המניפסט JSON עבור חבילות ראיות. |

הפעלת CLI לדוגמה (כתובת אתר Torii מוגדרת באמצעות `torii_api_url` ב-`iroha.json`):

```bash
iroha app space-directory bindings fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/bindings.json

iroha app space-directory manifest fetch \
  --uaid uaid:86e8ee39a3908460a0f4ee257bb25f340cd5b5de72735e9adefe07d5ef4bb0df \
  --json-out artifacts/uaid86/manifests.json
```

אחסן את צילומי ה-JSON לצד ה-hash המניפסט המשמש במהלך ביקורות; את
Space Directory Watcher בונה מחדש את המפה `uaid_dataspaces` בכל פעם שמתגלה
להפעיל, לפוג או לבטל, כך שתצלומים אלו הם הדרך המהירה ביותר להוכיח
אילו כריכות היו פעילות בתקופה נתונה.## 4. יכולת פרסום מתבטאת עם ראיות

השתמש בזרימת ה-CLI שלהלן בכל פעם שמתפרסמת קצבה חדשה. כל שלב חייב
נחתה בחבילת הראיות שנרשמה לצורך אישור ממשל.

1. **קודד את JSON המניפסט** כדי שהבודקים יראו את ה-hash הדטרמיניסטי לפני כן
   הגשה:

   ```bash
   iroha app space-directory manifest encode \
     --json fixtures/space_directory/capability/eu_regulator_audit.manifest.json \
     --out artifacts/eu_regulator_audit.manifest.to \
     --hash-out artifacts/eu_regulator_audit.manifest.hash
   ```

2. **פרסם את הקצבה** באמצעות מטען Norito (`--manifest`) או
   תיאור ה-JSON (`--manifest-json`). רשום את הקבלה Torii/CLI פלוס
   ה-hash של הוראות `PublishSpaceDirectoryManifest`:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --reason "ESMA wave 2 onboarding"
   ```

3. **Capture SpaceDirectoryEvent ראיות.** הירשם ל
   `SpaceDirectoryEvent::ManifestActivated` וכוללים את מטען האירוע ב
   החבילה כדי שהמבקרים יוכלו לאשר מתי השינוי הגיע.

4. **צור חבילת ביקורת** הקושרת את המניפסט לפרופיל מרחב הנתונים שלו ו
   ווי טלמטריה:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest artifacts/eu_regulator_audit.manifest.to \
     --profile fixtures/space_directory/profile/cbdc_lane_profile.json \
     --out-dir artifacts/eu_regulator_audit_bundle
   ```

5. **אמת כריכות באמצעות Torii** (`bindings fetch` ו-`manifests fetch`) ו
   אחסן את קבצי ה-JSON האלה עם ה-hash + חבילה למעלה.

רשימת הוכחות:

- [ ] חשיש מניפסט (`*.manifest.hash`) חתום על ידי מאשר השינוי.
- [ ] קבלה CLI/Torii עבור קריאת הפרסום (stdout או `--json-out` artefact).
- [ ] `SpaceDirectoryEvent` הוכחת מטען הפעלה.
- [ ] בדוק את ספריית החבילה עם פרופיל מרחב הנתונים, ווים ועותק מניפסט.
- [ ] כריכות + תמונות מניפסט שנלקחו מ-Torii לאחר ההפעלה.זה משקף את הדרישות ב-`docs/space-directory.md` §3.2 תוך מתן SDK
הבעלים של דף בודד להצביע עליו במהלך ביקורות מהדורה.

## 5. תבניות רגולטור/מניפסט אזורי

השתמשו במתקני ה-repo כנקודות התחלה כאשר מתבטא יכולת יצירה
עבור רגולטורים או מפקחים אזוריים. הם מדגימים כיצד לאפשר / לשלול היקף
כללים והסבירו את הערות המדיניות שהבודקים מצפים להם.

| מתקן | מטרה | הבהרה |
|--------|--------|----------------|
| `fixtures/space_directory/capability/eu_regulator_audit.manifest.json` | עדכון ביקורת ESMA/ESRB. | קצבאות לקריאה בלבד עבור `compliance.audit::{stream_reports, request_snapshot}` עם הכחשת זכיות בהעברות קמעונאיות כדי לשמור על UAIDs של הרגולטורים פסיביים. |
| `fixtures/space_directory/capability/jp_regulator_supervision.manifest.json` | נתיב פיקוח JFSA. | מוסיף קצבה מוגבלת של `cbdc.supervision.issue_stop_order` (חלון לכל יום + `max_amount`) ודחייה מפורשת ב-`force_liquidation` כדי לאכוף פקדים כפולים. |

בעת שיבוט מתקנים אלה, עדכן:

1. מזהי `uaid` ו-`dataspace` שיתאימו למשתתף ולנתיב שאתה מאפשר.
2. חלונות `activation_epoch`/`expiry_epoch` המבוססים על לוח הזמנים של הממשל.
3. שדות `notes` עם הפניות למדיניות של הרגולטור (מאמר MiCA, JFSA
   מעגלי וכו').
4. חלונות קצבה (`PerSlot`, `PerMinute`, `PerDay`) ואופציונלי
   `max_amount` מכסים כך ש-SDK אוכפים את אותן הגבלות כמו המארח.

## 6. הערות הגירה לצרכני SDKשילובי SDK קיימים שהתייחסו למזהי חשבון לכל דומיין חייבים לעבור אליהם
המשטחים הממוקדים ב-UAID שתוארו לעיל. השתמש ברשימת הבדיקה הזו במהלך שדרוגים:

  מזהי חשבון. עבור Rust/JS/Swift/Android זה אומר שדרוג לגרסה העדכנית ביותר
  ארגזי סביבת עבודה או כריכות Norito מתחדשות.
- **שיחות API:** החלף שאילתות פורטפוליו בהיקף דומיין בשאילתות
  `GET /v1/accounts/{uaid}/portfolio` ונקודות הקצה של המניפסט/הקשרים.
  `GET /v1/accounts/{uaid}/portfolio` מקבל שאילתת `asset_id` אופציונלית
  פרמטר כאשר ארנקים צריכים רק מופע נכס בודד. לקוח עוזרים כאלה
  כמו `ToriiClient.getUaidPortfolio` (JS) והאנדרואיד
  `SpaceDirectoryClient` כבר עוטפים את המסלולים האלה; מעדיף אותם על פני מותאמים אישית
  קוד HTTP.
- **מטמון וטלמטריה:** רשומות מטמון לפי UAID + מרחב נתונים במקום גולמי
  מזהי חשבון, ופולטות טלמטריה המציגות את ה-UAID מילולית, כך שהפעולות יוכלו
  ליישר יומנים עם עדויות של ספריית החלל.
- **טיפול בשגיאות:** נקודות קצה חדשות מחזירות את שגיאות הניתוח המחמירות של UAID
  מתועד ב-`docs/source/torii/portfolio_api.md`; לשטח את הקודים האלה
  מילה במילה כדי שצוותי תמיכה יוכלו לבדוק בעיות ללא שלבי תיקון.
- **בדיקה:** חבר את המתקנים שהוזכרו לעיל (בתוספת מניפסטים של UAID משלך)
  לתוך חבילות בדיקה של SDK כדי להוכיח Norito הלוך ושוב הערכות מניפסט
  להתאים למימוש המארח.

## 7. הפניות- `docs/space-directory.md` - ספר הפעלה למפעיל עם פירוט עמוק יותר של מחזור החיים.
- `docs/source/torii/portfolio_api.md` - סכימת REST עבור תיק UAID ו
  נקודות קצה ברורות.
- `crates/iroha_cli/src/space_directory.rs` - יישום CLI המוזכר ב
  המדריך הזה.
- `fixtures/space_directory/capability/*.manifest.json` - רגולטור, קמעונאות ו
  תבניות מניפסט של CBDC מוכנות לשיבוט.
