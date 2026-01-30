---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 351bcc23313b6a65e70d4c33e2f9f2032a871da4f906ad8db909a004ff774a40
source_last_modified: "2025-11-14T04:43:21.229863+00:00"
translation_last_reviewed: 2026-01-30
---

import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note מקור קנוני
עמוד זה משקף את `docs/source/sns/address_display_guidelines.md` ומשמש כעת כהעתק
הפורטל הקנוני. קובץ המקור נשאר לצורך PRs תרגום.
:::

ארנקים, סיירים ודוגמאות SDK חייבים להתיחס לכתובות חשבון כאל מטענים בלתי
משתנים. דוגמת הארנק הקמעונאי של Android ב-
`examples/android/retail-wallet` מדגימה כעת את דפוס ה-UX הנדרש:

- **שתי מטרות העתקה.** ספקו שני כפתורי העתקה מפורשים: IH58 (מועדף)
  והצורה הדחוסה עבור Sora בלבד (`sora...`, אפשרות שנייה). IH58 תמיד בטוחה לשיתוף חיצוני
  ומזינה את מטען ה-QR. הצורה הדחוסה חייבת לכלול אזהרה מוטמעת משום שהיא עובדת
  רק באפליקציות מודעות Sora. דוגמת Android מחברת את שני כפתורי Material ואת
  ה-tooltips שלהם ב-
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, והדמו
  iOS SwiftUI משקף את אותו UX דרך `AddressPreviewCard` בתוך
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **מונוספייס, טקסט נבחר.** הציגו את שתי המחרוזות בגופן מונוספייס ועם
  `textIsSelectable="true"` כדי שמשתמשים יוכלו לבדוק ערכים בלי להפעיל IME.
  הימנעו משדות עריכה: IME יכול לשכתב kana או להחדיר נקודות קוד ברוחב אפס.
- **רמזים לדומיין ברירת מחדל משתמע.** כאשר הסלקטור מצביע על הדומיין המשתמע
  `default`, הציגו כיתוב שמזכיר למפעילים שאין צורך בסיומת. סיירים צריכים גם
  להדגיש את תווית הדומיין הקנונית כאשר הסלקטור מקודד digest.
- **מטעני QR של IH58.** קודי QR חייבים לקודד את מחרוזת IH58. אם יצירת ה-QR
  נכשלת, הציגו שגיאה מפורשת במקום תמונה ריקה.
- **הודעות לוח גזירים.** לאחר העתקת הצורה הדחוסה, שלחו toast או snackbar שמזכירים
  למשתמשים שהיא Sora-only ורגישה לשיבוש IME.

שמירה על כללים אלה מונעת פגיעות Unicode/IME ומספקת את קריטריוני הקבלה של
מפת הדרכים ADDR-6 עבור UX של ארנקים/סיירים.

## צילומי מסך להפניה

השתמשו במקורות הבאים במהלך סקירות לוקליזציה כדי לוודא שתוויות כפתורים,
ה-tooltips והאזהרות נשארים מיושרים בין פלטפורמות:

- הפניה ל-Android: `/img/sns/address_copy_android.svg`

  ![הפניה להעתקה כפולה ב-Android](/img/sns/address_copy_android.svg)

- הפניה ל-iOS: `/img/sns/address_copy_ios.svg`

  ![הפניה להעתקה כפולה ב-iOS](/img/sns/address_copy_ios.svg)

## עוזרי SDK

כל SDK חושף עוזר נוחות שמחזיר את הצורות IH58 והדחוסה יחד עם מחרוזת האזהרה כדי
ששכבות UI ישארו עקביות:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` מחזיר את מחרוזת האזהרה הדחוסה
  ומוסיף אותה ל-`warnings` כאשר קוראים מספקים literal `sora...`, כדי שסיירים/
  לוחות ארנקים יוכלו להציג את אזהרת Sora-only במהלך זרימות הדבקה/אימות במקום
  רק כאשר הם מייצרים את הצורה הדחוסה בעצמם.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

השתמשו בעוזרים אלה במקום ליישם מחדש את לוגיקת ה-encode בשכבות UI. עוזר
JavaScript חושף גם payload `selector` ב-`domainSummary` (`tag`, `digest_hex`,
`registry_id`, `label`) כדי ש-UIs יוכלו לציין האם סלקטור הוא Local-12 או מגובה
רישום בלי לפרק מחדש את ה-payload הגולמי.

## דמו אינסטרומנטציה לסיירים

<ExplorerAddressCard />

סיירים צריכים לשקף את עבודת הטלמטריה והנגישות של הארנק:

- יישמו `data-copy-mode="ih58|compressed|qr"` על כפתורי ההעתקה כדי שהחזיתות
  יוכלו לשגר מוני שימוש לצד מדד Torii `torii_address_format_total`. רכיב הדמו
  למעלה שולח אירוע `iroha:address-copy` עם `{mode,timestamp}` - חברו זאת לצינור
  האנליטיקה/טלמטריה שלכם (למשל, שליחה ל-Segment או לאוסף מבוסס NORITO) כדי
  שלוחות מחוונים יוכלו לקשר שימוש בפורמט כתובת בצד השרת עם מצבי ההעתקה של
  הלקוח. שיקפו גם את מוני הדומיין של Torii (`torii_address_domain_total{domain_kind}`)
  באותו feed כדי שבדיקות פרישה של Local-12 יוכלו לייצא הוכחת 30 יום
  `domain_kind="local12"` ישירות מלוח `address_ingest` ב-Grafana.
- קשרו כל פקד לרמזי `aria-label`/`aria-describedby` ייחודיים שמסבירים האם literal
  בטוח לשיתוף (IH58) או Sora-only (דחוס). כללו את כיתוב הדומיין המשתמע
  בתיאור כך שטכנולוגיות מסייעות יציגו את אותו הקשר חזותי.
- חשפו אזור חי (למשל, `<output aria-live="polite">...</output>`) שמכריז על
  תוצאות העתקה ואזהרות, בהתאם להתנהגות VoiceOver/TalkBack שכבר מחוברת בדוגמאות
  Swift/Android.

אינסטרומנטציה זו מספקת את ADDR-6b בכך שהיא מוכיחה שמפעילים יכולים לעקוב גם אחרי
קליטת Torii וגם אחרי מצבי ההעתקה בצד הלקוח לפני שמבטלים סלקטורים מקומיים.

## ערכת מעבר Local -> Global

השתמשו ב-[Local -> Global toolkit](local-to-global-toolkit.md) כדי לאוטומט
ביקורת והמרה של סלקטורים Local מורשתיים. העוזר מפיק גם דוח ביקורת JSON וגם את
רשימת ה-IH58/דחוס שהוסבה שמפעילים מצרפים לכרטיסי readiness, בעוד ה-runbook
המצורף מקשר ללוחות Grafana ולכללי Alertmanager שמגדרים את ה-cutover במצב strict.

## הפניה מהירה לפריסת בינארי (ADDR-1a)

כאשר SDKs מציגים כלי כתובות מתקדמים (inspectors, רמזי אימות, בוני manifest),
כוונו מפתחים לפורמט ה-wire הקנוני שמתועד ב-`docs/account_structure.md`. הפריסה
תמיד היא `header · selector · controller`, כאשר ביטי ה-header הם:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) היום; ערכים שאינם אפס שמורים וחייבים לגרום
  ל-`AccountAddressError::InvalidHeaderVersion`.
- `addr_class` מבדיל בין בקרים יחידים (`0`) לבין multisig (`1`).
- `norm_version = 1` מקודד את כללי הסלקטור Norm v1. גרסאות עתידיות ישתמשו
  מחדש באותו שדה בן 2 ביטים.
- `ext_flag` הוא תמיד `0`; ביטים פעילים מצביעים על הרחבות payload לא נתמכות.

הסלקטור מגיע מיד אחרי ה-header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

ממשקי UI ו-SDKs צריכים להיות מוכנים להציג את סוג הסלקטור:

- `0x00` = דומיין ברירת מחדל משתמע (ללא payload).
- `0x01` = digest מקומי (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = רשומת רישום גלובלית (`registry_id:u32` big-endian).

דוגמאות hex קנוניות שכלי ארנק יכולים לקשר או לשלב ב-docs/tests:

| סוג סלקטור | Hex קנוני |
|---------------|---------------|
| ברירת מחדל משתמעת | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| digest מקומי (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| רישום גלובלי (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

ראו `docs/source/references/address_norm_v1.md` עבור טבלת selector/state מלאה
ו-`docs/account_structure.md` עבור תרשים הבתים המלא.

## אכיפת צורות קנוניות

מפעילים שממירים קידודים Local מורשתיים ל-IH58 קנוני או למחרוזות דחוסות צריכים
לעקוב אחרי תהליך CLI שמתועד תחת ADDR-5:

1. `iroha tools address inspect` מפיק כעת תקציר JSON מובנה עם IH58, דחוס ו-payloads hex
   קנוניים. התקציר כולל גם אובייקט `domain` עם שדות `kind`/`warning` ומחזיר כל
   דומיין שסופק דרך השדה `input_domain`. כאשר `kind` הוא `local12`, ה-CLI מדפיס
   אזהרה ל-stderr והתקציר משקף את אותה הנחיה כך שצינורות CI ו-SDKs יוכלו להציג
   אותה. העבירו `--append-domain` בכל פעם שתרצו להשמיע את הקידוד המומר בתור
   `<ih58>@<domain>`.
2. SDKs יכולים להציג את אותה אזהרה/תקציר דרך עוזר JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  העוזר משמר את קידומת IH58 שזוהתה מה-literal אלא אם מספקים במפורש
  `networkPrefix`, ולכן תקצירים לרשתות שאינן ברירת מחדל לא נבנים מחדש בשקט עם
  הקידומת ברירת המחדל.

3. המירו את ה-payload הקנוני בעזרת השדות `ih58.value` או `compressed` מהתקציר
   (או בקשו קידוד אחר דרך `--format`). מחרוזות אלו כבר בטוחות לשיתוף חיצוני.
4. עדכנו manifests, registries ומסמכים מול לקוח עם הצורה הקנונית והודיעו
   לשותפים שסלקטורים Local יידחו לאחר השלמת ה-cutover.
5. עבור מערכי נתונים גדולים, הריצו
   `iroha tools address audit --input addresses.txt --network-prefix 753`. הפקודה
   קוראת literals מופרדים בשורות (תגובות שמתחילות ב-`#` נזנחות, ו-`--input -` או
   ללא דגל משתמש ב-STDIN), מפיקה דוח JSON עם תקצירים קנוניים/IH58/דחוסים לכל
   ערך, וסופרת שגיאות parse ואזהרות דומיין Local. השתמשו ב-`--allow-errors`
   כאשר אתם מבקרים dumps מורשתיים שמכילים שורות זבל, ונעלו אוטומציה עם
   `--fail-on-warning` כאשר המפעילים מוכנים לחסום סלקטורים Local ב-CI.
6. כאשר צריך שכתוב שורה-לשורה, השתמשו
  עבור גיליונות remediation של סלקטורים Local, השתמשו
  כדי לייצא CSV `input,status,format,...` שמדגיש קידודים קנוניים, אזהרות ושגיאות
  parse במעבר אחד. העוזר מדלג על שורות שאינן Local כברירת מחדל, ממיר כל ערך
  שנותר לקידוד המבוקש (IH58/דחוס/hex/JSON), ומשמר את הדומיין המקורי כאשר
  `--append-domain` מוגדר. שלבו עם `--allow-errors` כדי להמשיך בסריקה גם כאשר
  dump מכיל literals פגומים.
7. אוטומציית CI/lint יכולה להריץ `ci/check_address_normalize.sh`, שמחלצת את
   הסלקטורים Local מתוך `fixtures/account/address_vectors.json`, ממירה אותם דרך
   `iroha tools address normalize`, ומריצה שוב
   `iroha tools address audit --fail-on-warning` כדי להוכיח ששחרורים כבר לא מפיקים
   digests Local.

`torii_address_local8_total{endpoint}` יחד עם
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, ולוח Grafana
`dashboards/grafana/address_ingest.json` מספקים את אות האכיפה: כאשר לוחות
הייצור מציגים אפס הגשות Local תקינות ואפס התנגשויות Local-12 במשך 30 ימים
רצופים, Torii יהפוך את שער Local-8 לכשל קשיח ב-mainnet, ולאחר מכן Local-12
כאשר לדומיינים גלובליים יש רשומות registry תואמות. ראו בפלט ה-CLI את ההודעה
למפעילים על הקפאה זו - אותה מחרוזת אזהרה משמשת ב-tooltips של SDK ובאוטומציה
כדי לשמור על התאמה לקריטריוני היציאה של מפת הדרכים. Torii משתמש כעת כברירת
כאשר מאבחנים רגרסיות. המשיכו לשקף `torii_address_domain_total{domain_kind}` ב-
Grafana (`dashboards/grafana/address_ingest.json`) כדי שחבילת הראיות ADDR-7
תוכל להוכיח ש-`domain_kind="local12"` נשאר אפס בחלון הנדרש של 30 יום לפני ש-
mainnet מבטל את הסלקטורים הישנים. חבילת Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) מוסיפה שלושה guardrails:

- `AddressLocal8Resurgence` מתריעה כאשר הקשר מדווח על עליה חדשה של Local-8.
  עצרו rollouts של מצב strict, מצאו את פני השטח של ה-SDK בלוח המחוונים, ואם
  שחזרו את ברירת המחדל (`true`).
- `AddressLocal12Collision` פועלת כאשר שני תוויות Local-12 מתגלגלות לאותו digest.
  עצרו קידומי manifest, הריצו את ערכת Local -> Global כדי לבקר את מיפוי ה-digests,
  ותאמו עם ממשל Nexus לפני הנפקת רשומת registry מחדש או הפעלת rollouts במורד
  הזרם.
- `AddressInvalidRatioSlo` מזהירה כאשר יחס ה-invalid בצי כולו (למעט דחיות
  Local-8/strict-mode) חוצה את SLO של 0.1% למשך עשר דקות. השתמשו ב-
  `torii_address_invalid_total` כדי לאתר את ההקשר/הסיבה ולהתאם עם צוות ה-SDK
  לפני הפעלה מחדש של מצב strict.

### קטע הערת שחרור (ארנק ומסייר)

כללו את ה-bullet הבא בהערות השחרור של הארנק/מסייר בעת cutover:

> **כתובות:** נוסף העוזר `iroha tools address normalize --only-local --append-domain`
> וחובר ל-CI (`ci/check_address_normalize.sh`) כך שצינורות ארנק/מסייר יוכלו להמיר
> סלקטורים Local מורשתיים לצורות IH58/דחוסות קנוניות לפני ש-Local-8/Local-12
> נחסמים ב-mainnet. עדכנו כל export מותאם להריץ את הפקודה ולצרף את הרשימה
> המנורמלת לחבילת הראיות של השחרור.
