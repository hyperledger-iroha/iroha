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
  כינוי מוסמך לתחום כגון `merchant@banka.sbp` וכינוי בסיס נתונים מרחבי נתונים
  כגון `merchant@sbp` יכולים שניהם לפתור לאותו `AccountId` הקנוני.
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

מסלולי Torii נוכחיים:| מסלול | מטרה |
|-------|--------|
| `GET /v1/ram-lfe/program-policies` | מפרט מדיניות תוכנית RAM-LFE פעילה ולא פעילה בתוספת מטא-נתוני הביצוע הציבוריים שלהן, כולל פרמטרים אופציונליים של BFV `input_encryption` וה-backend המתוכנת `ram_fhe_profile`. |
| `POST /v1/ram-lfe/programs/{program_id}/execute` | מקבל בדיוק אחד מ-`{ input_hex }` או `{ encrypted_input }` ומחזיר את `RamLfeExecutionReceipt` חסר המדינה בתוספת `{ output_hex, output_hash, receipt_hash }` עבור התוכנית הנבחרת. זמן הריצה הנוכחי של Torii מנפיק קבלות עבור ה-BFV האחורי המתוכנת. |
| `POST /v1/ram-lfe/receipts/verify` | מאמת ללא מדינה `RamLfeExecutionReceipt` כנגד מדיניות התוכנית ברשת המפורסמת ובודק באופן אופציונלי ש-`output_hex` שסופק על ידי המתקשר תואם את הקבלה `output_hash`. |
| `GET /v1/identifier-policies` | מפרט מרחבי שמות פעילים ולא פעילים של פונקציות נסתרות בתוספת המטא-נתונים הציבוריים שלהם, כולל פרמטרים אופציונליים של BFV `input_encryption`, מצב `normalization` הנדרש עבור קלט מוצפן בצד הלקוח ו-`ram_fhe_profile` עבור מדיניות BFV מתוכנתת. |
| `POST /v1/accounts/{account_id}/identifiers/claim-receipt` | מקבל בדיוק אחד מ-`{ input }` או `{ encrypted_input }`. טקסט פשוט `input` הוא מנורמל בצד השרת; BFV `encrypted_input` כבר חייב להיות מנורמל בהתאם למצב המדיניות שפורסם. לאחר מכן, נקודת הקצה גוזרת את הידית `opaque:` ומחזירה קבלה חתומה ש-`ClaimIdentifier` יכולה להגיש על השרשרת, כולל גם את ה-`signature_payload_hex` הגולמי וגם את ה-`signature_payload` המנתח. || `POST /v1/identifiers/resolve` | מקבל בדיוק אחד מ-`{ input }` או `{ encrypted_input }`. טקסט רגיל `input` מנורמל בצד השרת; BFV `encrypted_input` כבר חייב להיות מנורמל בהתאם למצב המדיניות שפורסם. נקודת הקצה פותרת את המזהה ל-`{ opaque_id, receipt_hash, uaid, account_id, signature }` כאשר קיימת תביעה פעילה, וגם מחזירה את המטען החתום הקנוני כ-`{ signature_payload_hex, signature_payload }`. |
| `GET /v1/identifiers/receipts/{receipt_hash}` | מחפש את ה-`IdentifierClaimRecord` המתמשך הקשור ל-hash קבלה דטרמיניסטית, כך שמפעילים ו-SDKs יכולים לבדוק בעלות על תביעות או לאבחן כשלים בהפעלה חוזרת/אי התאמה מבלי לסרוק את אינדקס המזהה המלא. |

זמן הריצה בתהליך ביצוע של Torii מוגדר תחת
`torii.ram_lfe.programs[*]`, מובנה על ידי `program_id`. נתיבי המזהה עכשיו
שימוש חוזר באותו זמן ריצה RAM-LFE במקום `identifier_resolver` נפרד
משטח תצורה.

תמיכת SDK נוכחית:- `normalizeIdentifierInput(value, normalization)` תואם ל-Rust
  קנונילייזרים עבור `exact`, `lowercase_trimmed`, `phone_e164`,
  `email_address`, ו-`account_number`.
- `ToriiClient.listIdentifierPolicies()` מפרט מטא נתונים של מדיניות, כולל BFV
  מטא-נתונים של הצפנת קלט כאשר המדיניות מפרסמת אותם, בתוספת פיענוח
  אובייקט פרמטר BFV דרך `input_encryption_public_parameters_decoded`.
  מדיניות מתוכנתת חושפת גם את `ram_fhe_profile` המפוענח. השדה הזה הוא
  בכוונה BFV-scope: זה מאפשר לארנקים לאמת את הרישום הצפוי
  ספירה, ספירת נתיבים, מצב קנוניזציה ומודול טקסט צופן מינימלי עבור
  קצה ה-FHE המתוכנת לפני הצפנת קלט בצד הלקוח.
- `getIdentifierBfvPublicParameters(policy)` ו
  `buildIdentifierRequestForPolicy(policy, { input | encryptedInput })` עזרה
  מתקשרי JS צורכים מטא נתונים של BFV שפורסמו ובונים בקשות מודעת למדיניות
  גופים מבלי ליישם מחדש את כללי המדיניות והנורמליזציה.
- `encryptIdentifierInputForPolicy(policy, input, { seedHex? })` ו
  `buildIdentifierRequestForPolicy(policy, { input, encrypt: true })` עכשיו אפשר
  ארנקי JS בונים את מעטפת הטקסט המלאה של BFV Norito באופן מקומי מ
  פרמטרי מדיניות שפורסמו במקום משלוח hex טקסט מוצפן בנוי מראש.
- `ToriiClient.resolveIdentifier({ policyId, input | encryptedInput })`
  פותר מזהה נסתר ומחזיר את מטען הקבלה החתום,
  כולל `receipt_hash`, `signature_payload_hex`, ו
  `signature_payload`.
- `ToriiClient.issueIdentifierClaimReceipt(accountId, { policyId, input |
  encryptedInput })` issues the signed receipt needed by `ClaimIdentifier`.
- `verifyIdentifierResolutionReceipt(receipt, policy)` מאמת את המוחזר
  קבלה כנגד מפתח פותר המדיניות בצד הלקוח, וכן`ToriiClient.getIdentifierClaimByReceiptHash(receiptHash)` מביא את
  רשומת תביעה מתמשכת עבור זרימות ביקורת/ניפוי באגים מאוחרות יותר.
- `IrohaSwift.ToriiClient` חושף כעת את `listIdentifierPolicies()`,
  `resolveIdentifier(policyId:input:encryptedInputHex:)`,
  `issueIdentifierClaimReceipt(accountId:policyId:input:encryptedInputHex:)`,
  ו-`getIdentifierClaimByReceiptHash(_)`, פלוס
  `ToriiIdentifierNormalization` עבור אותו מספר טלפון/אימייל/חשבון
  מצבי קנוניזציה.
- `ToriiIdentifierLookupRequest` וה-
  `ToriiIdentifierPolicySummary.plaintextRequest(...)` /
  עוזרי `.encryptedRequest(...)` מספקים את משטח הבקשה של Swift המוקלד עבור
  לפתור שיחות קבלה ולתבוע, ומדיניות Swift יכולה כעת לגזור את ה-BFV
  טקסט צופן באופן מקומי באמצעות `encryptInput(...)` / `encryptedRequest(input:...)`.
- `ToriiIdentifierResolutionReceipt.verifySignature(using:)` מאמת את זה
  שדות הקבלה ברמה העליונה תואמים את המטען החתום ומאמת את
  חתימת פותר בצד הלקוח לפני ההגשה.
- `HttpClientTransport` ב-Android SDK נחשף כעת
  `listIdentifierPolicies()`, `resolveIdentifier(policyId, input,
  encryptedInputHex)`, `issueIdentifierClaimReceipt(accountId, policyId,
  input, encryptedInputHex)`, and `getIdentifierClaimByReceiptHash(...)`,
  בתוספת `IdentifierNormalization` עבור אותם כללי קנוניזציה.
- `IdentifierResolveRequest` וה-
  `IdentifierPolicySummary.plaintextRequest(...)` /
  עוזרי `.encryptedRequest(...)` מספקים את משטח בקשת האנדרואיד המוקלד,
  בעוד `IdentifierPolicySummary.encryptInput(...)` /
  `.encryptedRequestFromInput(...)` גוזרים את מעטפת ה-BFV צופן
  באופן מקומי מפרמטרי מדיניות שפורסמו.
  `IdentifierResolutionReceipt.verifySignature(policy)` מאמת את המוחזר
  חתימת פותר בצד הלקוח.

ערכת הוראות נוכחית:- `RegisterIdentifierPolicy`
- `ActivateIdentifierPolicy`
- `ClaimIdentifier` (כרוכה בקבלה; תביעות גולמיות של `opaque_id` נדחות)
- `RevokeIdentifier`

שלושה ענפים אחוריים קיימים כעת ב-`iroha_crypto::ram_lfe`:

- המחויבות ההיסטורית מחויבת `HKDF-SHA3-512` PRF, ו
- מעריך סודי מגובה BFV שצורך מזהה מוצפן BFV
  חריצים ישירות. כאשר `iroha_crypto` בנוי עם ברירת המחדל
  תכונה `bfv-accel`, כפל טבעת BFV משתמש בדטרמיניסטית מדויקת
  CRT-NTT backend פנימי; השבתת תכונה זו נופלת בחזרה ל-
  נתיב ספרי בית ספר סקלרי עם תפוקות זהות, ו
- מעריך סודי מתוכנת בגיבוי BFV שמפיק הוראה מונעת
  מעקב ביצוע בסגנון RAM על אוגרים מוצפנים וזיכרון טקסט צופן
  נתיבים לפני גזירת המזהה האטום וה-hash הקבלה. המתוכנת
  הקצה האחורי דורש כעת רצפת מודול BFV חזקה יותר מהנתיב האפיפי, ו
  הפרמטרים הציבוריים שלו מתפרסמים בצרור קנוני הכולל את
  פרופיל ביצוע RAM-FHE הנצרך על ידי ארנקים ומאמתים.

כאן BFV פירושו ערכת FHE Brakerski/Fan-Vercauteren המיושמת ב
`crates/iroha_crypto/src/fhe_bfv.rs`. זהו מנגנון הביצוע המוצפן
בשימוש על ידי הקצה האפיפי והמתוכנת, לא את השם של החיצוני המוסתר
הפשטת פונקציות.Torii משתמש ב-backend שפורסם על ידי התחייבות המדיניות. כאשר ה-BFV backend
פעיל, בקשות טקסט רגיל מנורמלות ואז מוצפנות בצד השרת לפני כן
הערכה. בקשות BFV `encrypted_input` עבור הקצה האפיפי נבדקות
ישירות וחייב כבר להיות מנורמל בצד הלקוח; הקצה האחורי המתוכנת
מקנוניזציה של קלט מוצפן בחזרה אל ה-BFV הדטרמיניסטי של הפותר
מעטפה לפני הפעלת תוכנית ה-RAM הסודית כך ש-hashs של קבלות נשארות
יציב על פני טקסטים צפנים שווים מבחינה סמנטית.

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
   digest = hashlib.blake2b(seed, digest_size=32).hexdigest()
   print(f"uaid:{digest}")
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