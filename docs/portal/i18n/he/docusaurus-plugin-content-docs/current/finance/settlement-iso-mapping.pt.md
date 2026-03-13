---
lang: he
direction: rtl
source: docs/portal/docs/finance/settlement-iso-mapping.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: יישוב-איזו-מיפוי
כותרת: התיישבות ↔ ISO 20022 מיפוי שדות
sidebar_label: Settlement ↔ ISO 20022
תיאור: מיפוי קנוני בין זרימות היישוב Iroha לבין גשר ISO 20022.
---

:::הערה מקור קנוני
:::

## יישוב ↔ ISO 20022 מיפוי שדות

הערה זו לוכדת את המיפוי הקנוני בין הוראות ההתנחלות Iroha
(`DvpIsi`, `PvpIsi`, תזרימי בטחונות ריפו) והודעות ISO 20022 שהופעלו
ליד הגשר. זה משקף את פיגום המסר המיושם ב
`crates/ivm/src/iso20022.rs` ומשמש כאסמכתא בעת הפקת או
אימות מטענים של Norito.

### מדיניות נתוני התייחסות (מזהים ואימות)

מדיניות זו אוספת את העדפות המזהה, כללי האימות ונתוני ההפניה
חובות שעל גשר Norito ↔ ISO 20022 לאכוף לפני פליטת הודעות.

**נקודות עיגון בתוך הודעת ISO:**
- **מזהי מכשיר** → `delivery_leg.asset_definition_id` ↔ `SctiesLeg/FinInstrmId`
  (או שדה המכשיר המקביל).
- **צדדים / סוכנים** → `DlvrgSttlmPties/Pty` ו-`RcvgSttlmPties/Pty` עבור `sese.*`,
  או מבני הסוכן ב-`pacs.009`.
- **חשבונות** → `…/Acct` אלמנטים לשמירה/חשבונות מזומנים; מראה את ספר החשבונות
  `AccountId` ב-`SupplementaryData`.
- **מזהים קנייניים** → `…/OthrId` עם `Tp/Prtry` ומשתקפים ב
  `SupplementaryData`. לעולם אל תחליף מזהים מוסדרים במזהים קנייניים.

#### העדפת מזהה לפי משפחת הודעות

##### `sese.023` / `.024` / `.025` (הסדר ניירות ערך)

- **מכשיר (`FinInstrmId`)**
  - מועדף: **ISIN** תחת `…/ISIN`. זהו המזהה הקנוני של CSDs / T2S.[^anna]
  - נפילות:
    - **CUSIP** או NSIN אחר תחת `…/OthrId/Id` עם `Tp/Cd` מוגדר מהחיצוני ISO
      רשימת קודים (לדוגמה, `CUSP`); כלול את המנפיק ב-`Issr` כאשר הוחלט.[^iso_mdr]
    - **Norito מזהה נכס** כקנייני: `…/OthrId/Id`, `Tp/Prtry="NORITO_ASSET_ID"`, ו
      רשום את אותו הערך ב-`SupplementaryData`.
  - מתארים אופציונליים: **CFI** (`ClssfctnTp`) ו-**FISN** כאשר הם נתמכים כדי להקל
    פיוס.[^iso_cfi][^iso_fisn]
- **מסיבות (`DlvrgSttlmPties`, `RcvgSttlmPties`)**
  - מועדף: **BIC** (`AnyBIC/BICFI`, ISO 9362).[^swift_bic]
  - Fallback: **LEI** שבו גרסת ההודעה חושפת שדה LEI ייעודי; אם
    נעדר, נושא מזהים קנייניים עם תוויות `Prtry` ברורות וכולל BIC במטא נתונים.[^iso_cr]
- **מקום יישוב/מקום** → **MIC** למקום ו-**BIC** עבור CSD.[^iso_mic]

##### `colr.010` / `.011` / `.012` ו-`colr.007` (ניהול בטחונות)

- פעל לפי אותם כללי המכשיר כמו `sese.*` (מועדף ISIN).
- הצדדים משתמשים ב-**BIC** כברירת מחדל; **LEI** מקובל היכן שהסכימה חושפת אותו.[^swift_bic]
- סכומי מזומן חייבים להשתמש בקודי מטבע **ISO 4217** עם יחידות משניות נכונות.[^iso_4217]

##### `pacs.009` / `camt.054` (מימון והצהרות PvP)- **סוכנים (`InstgAgt`, `InstdAgt`, סוכני חייבים/נושים)** → **BIC** עם אופציונלי
  LEI היכן שמותר.[^swift_bic]
- **חשבונות**
  - בין-בנקאי: זהה לפי **BIC** והפניות בחשבון פנימי.
  - הצהרות מול לקוחות (`camt.054`): כוללות **IBAN** כאשר קיימות ומאמתות אותו
    (אורך, חוקי המדינה, mod-97 checksum).[^swift_iban]
- **מטבע** → **ISO 4217** קוד בן 3 אותיות, יש לכבד את עיגול יחידות מינוריות.[^iso_4217]
- **Torii בליעה** ← שלח רגליים למימון PvP דרך `POST /v2/iso20022/pacs009`; את הגשר
  דורש `Purp=SECU` וכעת אוכף מעברי חצייה BIC כאשר נתוני התייחסות מוגדרים.

#### כללי אימות (חלים לפני הפליטה)

| מזהה | כלל אימות | הערות |
|------------|----------------|-------|
| **ISIN** | Regex `^[A-Z]{2}[A-Z0-9]{9}[0-9]$` ו-Luhn (mod-10) ספרת ביקורת לפי ISO 6166 נספח C | דחה לפני פליטת גשר; מעדיפים העשרה במעלה הזרם.[^anna_luhn] |
| **CUSIP** | Regex `^[A-Z0-9]{9}$` ו-modulus-10 עם 2 שקלול (תווים ממפות לספרות) | רק כאשר ISIN אינו זמין; מפה דרך מעבר חצייה של ANNA/CUSIP פעם אחת.[^cusip] |
| **LEI** | Regex `^[A-Z0-9]{18}[0-9]{2}$` ו-mod-97 ספרת ביקורת (ISO 17442) | אימות מול קבצי דלתא יומיים של GLEIF לפני הקבלה.[^gleif] |
| **BIC** | Regex `^[A-Z]{4}[A-Z]{2}[A-Z0-9]{2}([A-Z0-9]{3})?$` | קוד סניף אופציונלי (שלושת התווים האחרונים). אשר סטטוס פעיל בקבצי RA.[^swift_bic] |
| **MIC** | שמור מקובץ RA ISO 10383; ודא שהמקומות פעילים (ללא דגל סיום `!`) | סגל MICs שהוצא משימוש לפני פליטה.[^iso_mic] |
| **IBAN** | אורך ספציפי למדינה, אותיות גדולות אלפאנומריות, mod-97 = 1 | השתמש ברישום המתוחזק על ידי SWIFT; דחה IBANs בלתי חוקיים מבחינה מבנית.[^swift_iban] |
| **זיהויי חשבון/צד קנייני** | `Max35Text` (UTF-8, ≤35 תווים) עם רווח לבן חתוך | חל על שדות `GenericAccountIdentification1.Id` ו-`PartyIdentification135.Othr/Id`. דחה ערכים שעולים על 35 תווים כך שמטעני הגשר תואמים סכימות ISO. |
| **מזהי חשבון פרוקסי** | `Max2048Text` לא ריק תחת `…/Prxy/Id` עם קודי סוג אופציונליים ב-`…/Prxy/Tp/{Cd,Prtry}` | מאוחסן לצד ה-IBAN הראשי; אימות עדיין דורש IBANs תוך קבלת נקודות הפעלה (עם קודי סוג אופציונליים) לשיקוף מסילות PvP. |
| **CFI** | קוד בן שישה תווים, אותיות רישיות באמצעות טקסונומיה ISO 10962 | העשרה אופציונלית; ודא שתווים תואמים למחלקת כלי.[^iso_cfi] |
| **FISN** | עד 35 תווים, רישיות אלפאנומריות בתוספת סימני פיסוק מוגבלים | אופציונלי; חתוך/נרמל לפי הנחיית ISO 18774.[^iso_fisn] |
| **מטבע** | ISO 4217 קוד בן 3 אותיות, קנה המידה נקבע לפי יחידות משניות | הסכומים חייבים לעגל לעשרונים המותרים; לאכוף בצד Norito.[^iso_4217] |

#### חובות מעבר חציה ותחזוקת נתונים- שמור על **ISIN ↔ Norito מזהה נכס** ו-**CUSIP ↔ ISIN** מעברי חציה. עדכון מדי לילה מ
  עדכוני ANNA/DSB וגירסאות שולטים בתמונות המשמשות את CI.[^anna_crosswalk]
- רענן את מיפויי **BIC ↔ LEI** מקובצי יחסי הציבור של GLEIF כדי שהגשר יוכל
  פולט את שניהם כאשר נדרש.[^bic_lei]
- אחסן **הגדרות MIC** לצד המטא-נתונים של הגשר כך שאימות המקום יהיה
  דטרמיניסטית גם כאשר קבצי RA משתנים באמצע היום.[^iso_mic]
- רישום מקור הנתונים (חותמת זמן + מקור) במטא נתונים של גשר לביקורת. תמשיכו את
  מזהה תמונת מצב לצד הוראות שנפלטו.
- הגדר את `iso_bridge.reference_data.cache_dir` כדי לשמור עותק של כל מערך נתונים נטען
  לצד מטא נתונים של מקור (גרסה, מקור, חותמת זמן, סכום בדיקה). זה מאפשר למבקרים
  ומפעילים להבדיל בין הזנות היסטוריות גם לאחר סיבוב של תמונות במעלה הזרם.
- צילומי ISO של מעבר חציה נבלעים על ידי `iroha_core::iso_bridge::reference_data` באמצעות
  בלוק התצורה `iso_bridge.reference_data` (נתיבים + מרווח רענון). מדדים
  `iso_reference_status`, `iso_reference_age_seconds`, `iso_reference_records`, ו
  `iso_reference_refresh_interval_secs` לחשוף את בריאות זמן הריצה להתראה. ה-Torii
  גשר דוחה הגשות `pacs.008` שה-BIC של הסוכן שלהן נעדר מהתצורה המוגדרת
  מעבר חציה, מציף שגיאות `InvalidIdentifier` דטרמיניסטיות כאשר צד נגדי
  לא ידוע.【crates/iroha_torii/src/iso20022_bridge.rs#L1078】
- כריכות IBAN ו-ISO 4217 נאכפות באותה שכבה: pacs.008/pacs.009 זורם עכשיו
  פולט שגיאות `InvalidIdentifier` כאשר IBANs לחייב/נושה חסרים כינויים מוגדרים או כאשר
  מטבע ההסדר חסר ב-`currency_assets`, מונע גשר פגום
  הוראות מהגעה לפנקס החשבונות. אימות IBAN חל גם ספציפי למדינה
  אורכים וספרות בדיקה מספריות לפני מעבר ה-ISO 7064 mod-97 ולכן לא חוקי מבחינה מבנית
  הערכים נדחים מוקדם.【crates/iroha_torii/src/iso20022_bridge.rs#L775】【crates/iroha_torii/src/iso20022_bridge.rs#L827】【crates/ivm/src/iso2020#L775】12.520
- עוזרי ההתנחלות CLI יורשים את אותם מעקות שמירה: עוברים
  `--iso-reference-crosswalk <path>` לצד `--delivery-instrument-id` כדי לקבל את ה-DvP
  תצוגה מקדימה אמת מזהי מכשירים לפני פליטת תמונת ה-XML `sese.023`.【crates/iroha_cli/src/main.rs#L3752】
- מוך `cargo xtask iso-bridge-lint` (ומעטפת ה-CI `ci/check_iso_reference_data.sh`)
  תמונות ומתקני חצייה. הפקודה מקבלת `--isin`, `--bic-lei`, `--mic`, ו
  `--fixtures` מסמן ונופל בחזרה למערכי הנתונים לדוגמה ב-`fixtures/iso_bridge/` בעת הפעלה
  ללא ארגומנטים.【xtask/src/main.rs#L146】【ci/check_iso_reference_data.sh#L1】
- העוזר IVM מכניס כעת מעטפות XML אמיתיות של ISO 20022 (head.001 + `DataPDU` + `Document`)
  ומאמת את כותרת היישום העסקי באמצעות סכימת `head.001` כך `BizMsgIdr`,
  סוכני `MsgDefIdr`, `CreDt` ו-BIC/ClrSysMmbId נשמרים באופן דטרמיניסטי; XMLDSig/XAdES
  בלוקים נשארים דילוג בכוונה. 

#### שיקולי רגולציה ומבנה שוק- **הסדר T+1**: שוקי המניות בארה"ב/קנדה עברו ל-T+1 ב-2024; התאם את Norito
  תזמון והתראות SLA בהתאם.[^sec_t1][^csa_t1]
- **קנסות CSDR**: כללי משמעת בהסדר אוכפים קנסות במזומן; להבטיח Norito
  metadata לוכדים הפניות לעונשים לצורך התאמה.[^csdr]
- **פיילוטים להתנחלויות באותו יום**: הרגולטור של הודו משלב בהדרגה את הסדר T0/T+0; לשמור
  יומני הגשר מתעדכנים ככל שהטייסים מתרחבים.[^india_t0]
- **רכישות / החזקות בטחונות**: עקוב אחר עדכוני ESMA על לוחות זמנים לרכישה והחזקות אופציונליות
  כך שהמסירה המותנית (`HldInd`) תואמת את ההנחיות העדכניות ביותר.[^csdr]

[^anna]: ANNA ISIN Guidelines, December 2023. https://anna-web.org/wp-content/uploads/2024/01/ISIN-Guidelines-Version-22-Dec-2023.pdf
[^iso_mdr]: ISO 20022 external code list (CUSIP `CUSP`) and MDR Part 2. https://www.iso20022.org/milestone/22048/download
[^iso_cfi]: ISO 10962 (CFI) taxonomy. https://www.iso.org/standard/81140.html
[^iso_fisn]: ISO 18774 (FISN) format guidance. https://www.iso.org/standard/66153.html
[^swift_bic]: SWIFT business identifier code (ISO 9362) guidance. https://www.swift.com/standards/data-standards/bic-business-identifier-code
[^iso_cr]: ISO 20022 change request introducing LEI options for party identification. https://www.iso20022.org/milestone/16116/download
[^iso_mic]: ISO 10383 Market Identifier Code maintenance agency. https://www.iso20022.org/market-identifier-codes
[^iso_4217]: ISO 4217 currency and minor-units table (SIX). https://www.six-group.com/en/products-services/financial-information/market-reference-data/data-standards.html
[^swift_iban]: IBAN registry and validation rules. https://www.swift.com/swift-resource/22851/download
[^anna_luhn]: ISIN checksum algorithm (Annex C). https://www.anna-dsb.com/isin/
[^cusip]: CUSIP format and checksum rules. https://www.iso20022.org/milestone/22048/download
[^gleif]: GLEIF LEI structure and validation details. https://www.gleif.org/en/organizational-identity/introducing-the-legal-entity-identifier-lei/iso-17442-the-lei-code-structure
[^anna_crosswalk]: ISIN cross-reference (ANNA DSB) feeds for derivatives and debt instruments. https://www.anna-dsb.com/isin/
[^bic_lei]: GLEIF BIC-to-LEI relationship files. https://www.gleif.org/en/lei-data/lei-mapping/download-bic-to-lei-relationship-files
[^sec_t1]: SEC release on US T+1 transition (2023). https://www.sec.gov/newsroom/press-releases/2023-29
[^csa_t1]: CSA amendments for Canadian institutional trade matching (T+1). https://www.osc.ca/en/securities-law/instruments-rules-policies/2/24-101/csa-notice-amendments-national-instrument-24-101-institutional-trade-matching-and-settlement-and
[^csdr]: ESMA CSDR settlement discipline / penalty mechanism updates. https://www.esma.europa.eu/sites/default/files/2024-11/ESMA74-2119945925-2059_Final_Report_on_Technical_Advice_on_CSDR_Penalty_Mechanism.pdf
[^india_t0]: SEBI circular on same-day settlement pilot. https://www.reuters.com/sustainability/boards-policy-regulation/india-markets-regulator-extends-deadline-same-day-settlement-plan-brokers-2025-04-29/

### משלוח מול תשלום → `sese.023`| שדה DvP | נתיב ISO 20022 | הערות |
|--------------------------------------------------------|----------------------------------------|------|
| `settlement_id` | `TxId` | מזהה מחזור חיים יציב |
| `delivery_leg.asset_definition_id` (אבטחה) | `SctiesLeg/FinInstrmId` | מזהה קנוני (ISIN, CUSIP, ...) |
| `delivery_leg.quantity` | `SctiesLeg/Qty` | מחרוזת עשרונית; מכבד דיוק נכס |
| `payment_leg.asset_definition_id` (מטבע) | `CashLeg/Ccy` | קוד מטבע ISO |
| `payment_leg.quantity` | `CashLeg/Amt` | מחרוזת עשרונית; מעוגל לפי מפרט נומרי |
| `delivery_leg.from` (מוכר / צד מסירה) | `DlvrgSttlmPties/Pty/Bic` | BIC של מסירת משתתף *(מזהה קנוני של חשבון מיוצא כעת במטא נתונים)* |
| `delivery_leg.from` מזהה חשבון | `DlvrgSttlmPties/Acct` | צורה חופשית; מטא נתונים Norito נושאים מזהה חשבון מדויק |
| `delivery_leg.to` (קונה / צד מקבל) | `RcvgSttlmPties/Pty/Bic` | BIC של משתתף מקבל |
| `delivery_leg.to` מזהה חשבון | `RcvgSttlmPties/Acct` | צורה חופשית; תואם מזהה חשבון מקבל |
| `plan.order` | `Plan/ExecutionOrder` | Enum: `DELIVERY_THEN_PAYMENT` או `PAYMENT_THEN_DELIVERY` |
| `plan.atomicity` | `Plan/Atomicity` | Enum: `ALL_OR_NOTHING`, `COMMIT_FIRST_LEG`, `COMMIT_SECOND_LEG` |
| **מטרת ההודעה** | `SttlmTpAndAddtlParams/SctiesMvmntTp` | `DELI` (לספק) או `RECE` (לקבל); מראות אשר רגל הצד המגיש מבצע. |
|                                                        | `SttlmTpAndAddtlParams/Pmt` | `APMT` (נגד תשלום) או `FREE` (ללא תשלום). |
| `delivery_leg.metadata`, `payment_leg.metadata` | `SctiesLeg/Metadata`, `CashLeg/Metadata` | אופציונלי Norito JSON מקודד כ-UTF‑8 |

> **סמכויות יישוב** - הגשר משקף את נהלי השוק על ידי העתקת קודי תנאי יישוב (`SttlmTxCond`), אינדיקטורים להסדר חלקי (`PrtlSttlmInd`), ומסמכים אופציונליים אחרים מ-Norito מטא-נתונים ל-I0017060X כאשר הם קיימים. אכוף את הספירות שפורסמו ברשימות הקוד החיצוני של ISO כך ש-CSD היעד יזהה את הערכים.

### תשלום מול תשלום מימון → `pacs.009`

רגלי המזומן במזומן המממנות הוראת PvP מונפקות כאשראי FI-to-FI
העברות. הגשר מציין את התשלומים הללו כך שמערכות במורד הזרם מזהות
הם מממנים הסדר ניירות ערך.| תחום מימון PvP | נתיב ISO 20022 | הערות |
|------------------------------------------------|------------------------------------------------|------|
| `primary_leg.quantity` / {amount, currency} | `IntrBkSttlmAmt` + `IntrBkSttlmCcy` | סכום/מטבע שחויב מהיזם. |
| מזהי סוכני צד שכנגד | `InstgAgt`, `InstdAgt` | BIC/LEI של סוכני שולח וקבלה. |
| מטרת התיישבות | `CdtTrfTxInf/PmtTpInf/CtgyPurp/Cd` | מוגדר ל-`SECU` עבור מימון PvP הקשור לניירות ערך. |
| מטא נתונים Norito (מזהי חשבון, נתוני FX) | `CdtTrfTxInf/SplmtryData` | נושא AccountId מלא, חותמות זמן של FX, רמזים לתוכנית ביצוע. |
| מזהה הוראות / קישור מחזור חיים | `CdtTrfTxInf/PmtId/InstrId`, `CdtTrfTxInf/RmtInf` | מתאים ל-Norito `settlement_id` כך שרגל המזומנים מתיישבת עם צד ניירות הערך. |

גשר ה-ISO של JavaScript SDK מתיישר עם הדרישה הזו על ידי ברירת המחדל של
מטרת קטגוריית `pacs.009` ל-`SECU`; המתקשרים עשויים לעקוף אותו עם אחר
קוד ISO חוקי בעת פליטת העברות אשראי שאינן ניירות ערך, אך לא חוקי
ערכים נדחים מראש.

אם תשתית דורשת אישור ניירות ערך מפורש, הגשר
ממשיך לפלוט `sese.025`, אבל האישור הזה משקף את רגל ניירות הערך
סטטוס (לדוגמה, `ConfSts = ACCP`) במקום "מטרת ה-PvP".

### אישור תשלום מול תשלום → `sese.025`

| שדה PvP | נתיב ISO 20022 | הערות |
|------------------------------------------------|------------------------|-------|
| `settlement_id` | `TxId` | מזהה מחזור חיים יציב |
| `primary_leg.asset_definition_id` | `SttlmCcy` | קוד מטבע עבור הרגל הראשית |
| `primary_leg.quantity` | `SttlmAmt` | סכום שנמסר על ידי יוזם |
| `counter_leg.asset_definition_id` | `AddtlInf` (עומס JSON) | קוד מטבע נגדי מוטבע במידע משלים |
| `counter_leg.quantity` | `SttlmQty` | סכום נגדי |
| `plan.order` | `Plan/ExecutionOrder` | אותו הקובץ מוגדר כמו DvP |
| `plan.atomicity` | `Plan/Atomicity` | אותו הקובץ מוגדר כמו DvP |
| מצב `plan.atomicity` (`ConfSts`) | `ConfSts` | `ACCP` בהתאמה; גשר פולט קודי כשל בעת דחייה |
| מזהי צד שכנגד | `AddtlInf` JSON | הגשר הנוכחי מביא בסידרה טופלים מלאים של AccountId/BIC במטא נתונים |

### החלפת בטחונות חוזרת → `colr.007`| שדה ריפו / הקשר | נתיב ISO 20022 | הערות |
|------------------------------------------------|--------------------------------|------|
| `agreement_id` (`RepoIsi` / `ReverseRepoIsi`) | `OblgtnId` | מזהה חוזה ריפו |
| מזהה Tx להחלפת בטחונות | `TxId` | נוצר לפי החלפה |
| כמות בטחונות מקורית | `Substitution/OriginalAmt` | התאמות משועבדות לפני החלפה |
| מטבע בטחונות מקורי | `Substitution/OriginalCcy` | קוד מטבע |
| כמות בטחונות תחליפי | `Substitution/SubstituteAmt` | סכום החלפה |
| מטבע בטחונות חלופי | `Substitution/SubstituteCcy` | קוד מטבע |
| תאריך תוקף (לוח זמנים לשולי ממשל) | `Substitution/EffectiveDt` | תאריך ISO (YYYY-MM-DD) |
| סיווג תספורת | `Substitution/Type` | נכון לעכשיו `FULL` או `PARTIAL` מבוסס על מדיניות ממשל |
| סיבת ממשל / הערת תספורת | `Substitution/ReasonCd` | אופציונלי, נושא רציונל ממשל |

### מימון והצהרות

| הקשר Iroha | הודעת ISO 20022 | מיפוי מיקום |
|--------------------------------|------------------------|----------------|
| הצתת רגל ריפו מזומן / התנתקות | `pacs.009` | `IntrBkSttlmAmt`, `IntrBkSttlmCcy`, `IntrBkSttlmDt`, `InstgAgt`, `InstdAgt` מאוכלס מרגלי DvP/PvP |
| הצהרות לאחר ההתנחלות | `camt.054` | תנועות רגל תשלום שנרשמו תחת `Ntfctn/Ntry[*]`; גשר מכניס מטא נתונים של ספר חשבונות/חשבון ב-`SplmtryData` |

### הערות שימוש* כל הסכומים מסודרים באמצעות העוזרים המספריים Norito (`NumericSpec`)
  כדי להבטיח התאמה לקנה מידה בין הגדרות נכסים.
* ערכי `TxId` הם `Max35Text` - אכוף אורך UTF‑8 ≤35 תווים לפני
  ייצוא להודעות ISO 20022.
* BIC חייב להיות 8 או 11 תווים אלפאנומריים גדולים (ISO9362); לדחות
  מטא נתונים Norito שנכשלו בבדיקה זו לפני פליטת תשלומים או פשרה
  אישורים.
* מזהי חשבון (AccountId / ChainId) מיוצאים אל משלים
  מטא נתונים כך שהמשתתפים המקבלים יוכלו להתיישב מול ספרי החשבונות המקומיים שלהם.
* `SupplementaryData` חייב להיות JSON קנוני (UTF‑8, מפתחות ממוינים, מקורי JSON
  בורח). עוזרי SDK אוכפים זאת כך חתימות, גיבוב טלמטריה ו-ISO
  ארכיוני מטען נשארים דטרמיניסטיים בכל בנייה מחדש.
* סכומי המטבע עוקבים אחר ספרות השבר של ISO4217 (לדוגמה ל-JPY יש 0
  עשרונים, בדולר ארה"ב יש 2); הגשר מהדק Norito דיוק מספרי בהתאם.
* עוזרי ההתנחלויות של CLI (`iroha app settlement ... --atomicity ...`) פולטים כעת
  הוראות Norito שתוכניות הביצוע שלהן ממפות 1:1 ל-`Plan/ExecutionOrder` ו
  `Plan/Atomicity` למעלה.
* עוזר ISO (`ivm::iso20022`) מאמת את השדות המפורטים למעלה ודוחה
  הודעות שבהן רגלי DvP/PvP מפרות מפרט נומרי או הדדיות של צד שכנגד.

### עוזרי בונה SDK

- SDK JavaScript חושף כעת את `buildPacs008Message` /
  `buildPacs009Message` (ראה `javascript/iroha_js/src/isoBridge.js`) אז הלקוח
  אוטומציה יכולה להמיר מטא נתונים מובנים של יישוב (BIC/LEI, IBANs,
  קודי מטרה, שדות Norito משלימים) לתוך חבילות דטרמיניסטיות XML
  מבלי ליישם מחדש את כללי המיפוי ממדריך זה.
- שני העוזרים דורשים `creationDateTime` מפורש (ISO-8601 עם אזור זמן)
  אז אופרטורים חייבים להשחיל חותמת זמן דטרמיניסטית מזרימת העבודה שלהם במקום זאת
  לאפשר ל-SDK כברירת מחדל לזמן שעון קיר.
- `recipes/iso_bridge_builder.mjs` מדגים כיצד לחבר את העוזרים האלה
  CLI שממזג משתני סביבה או קובצי תצורה של JSON, מדפיס את ה
  נוצר XML, ואפשר לשלוח אותו ל-Torii (`ISO_SUBMIT=1`), תוך שימוש חוזר
  אותו קצב המתנה כמו מתכון גשר ISO.


### הפניות

- דוגמאות ליישוב LuxCSD / Clearstream ISO 20022 המציגות את `SttlmTpAndAddtlParams/SctiesMvmntTp` (`DELI`/`RECE`) ו-`Pmt` (`APMT`/`FREE`).<sup>[1](https://www.luxcsd.com/resource/blob/3434074/6f8add4708407a4701055be4dd04846b/c23005-eis-examples-cbf-data.pdf)</sup>
- מפרטי Clearstream DCP המכסים את מוקדי ההסדר (`SttlmTxCond`, `PrtlSttlmInd`).<sup>[2](https://www.clearstream.com/clearstream-en/res-library/market-coverage/instruction-specifications-swift-iso-20022-dcp-mode-ceu-spain-2357008)</sup>
- הנחיות SWIFT PMPG הממליצות על `pacs.009` עם `CtgyPurp/Cd = SECU` למימון PvP הקשור לניירות ערך.<sup>[3](https://www.swift.com/swift-resource/251897/download)</sup>
- דוחות הגדרות הודעות ISO 20022 עבור אילוצי אורך מזהה (BIC, Max35Text).<sup>[4](https://www.iso20022.org/sites/default/files/2020-12/ISO20022_MDRPart2_ChangeOrVerifyAccountIdentification_2020_2021_v1_ForSEGReview.pdf)</sup>
- הנחיות ANNA DSB לגבי פורמט ISIN וכללי סכום בדיקה.<sup>[5](https://www.anna-dsb.com/isin/)</sup>

### טיפים לשימוש- הדבק תמיד את הקטע הרלוונטי Norito או פקודת CLI כדי ש-LLM יוכל לבדוק
  שמות שדות מדויקים וסולם מספרי.
- בקש ציטוטים (`provide clause references`) כדי לשמור על עקבות נייר עבור
  ציות ובדיקת מבקר.
- ללכוד את סיכום התשובה ב-`docs/source/finance/settlement_iso_mapping.md`
  (או נספחים מקושרים) כך שמהנדסים עתידיים לא יצטרכו לחזור על השאילתה.

## ספרי הזמנת אירועים (ISO 20022 ↔ Norito Bridge)

### תרחיש א' - החלפת בטחונות (ריפו / משכון)

**משתתפים:** נותן/נוטל בטחונות (ו/או סוכנים), אפוטרופוס(ים), CSD/T2S  
**תזמון:** לכל ניתוק שוק ומחזורי T2S יום/לילה; תזמר את שתי הרגליים כך שיסיימו באותו חלון יישוב.

#### כוריאוגרפיית מסרים
1. `colr.010` בקשה להחלפת בטחונות ← נותן/נוטל בטחונות או סוכן.  
2. `colr.011` תגובת החלפת בטחונות ← קבל/דחה (סיבת דחייה אופציונלית).  
3. `colr.012` אישור החלפת בטחונות → מאשר הסכם החלפה.  
4. הוראות `sese.023` (שתי רגליים):  
   - החזר בטחונות מקוריים (`SctiesMvmntTp=DELI`, `Pmt=FREE`, `SctiesTxTp=COLO`).  
   - לספק בטחונות חלופיים (`SctiesMvmntTp=RECE`, `Pmt=FREE`, `SctiesTxTp=COLI`).  
   קשר את הזוג (ראה למטה).  
5. עצות סטטוס `sese.024` (התקבלו, תואמו, בהמתנה, נכשלים, נדחו).  
6. אישורי `sese.025` לאחר ההזמנה.  
7. דלתא מזומן אופציונלית (עמלות/תספורת) → `pacs.009` העברת אשראי FI-to-FI עם `CtgyPurp/Cd = SECU`; סטטוס דרך `pacs.002`, מחזיר דרך `pacs.004`.

#### אישורים/סטטוסים נדרשים
- רמת תחבורה: שערים עשויים לפלוט `admi.007` או דחויים לפני עיבוד עסקי.  
- מחזור חיים של יישוב: `sese.024` (סטטוסי עיבוד + קודי סיבה), `sese.025` (סופי).  
- צד מזומן: `pacs.002` (`PDNG`, `ACSC`, `RJCT` וכו'), `pacs.004` להחזרות.

#### תנאי / שחרור שדות
- `SctiesSttlmTxInstr/Lnkgs` (`WITH`/`BEFO`/`AFTE`) כדי לשרשר את שתי ההוראות.  
- `SttlmParams/HldInd` להחזיק עד עמידה בקריטריונים; שחרור באמצעות `sese.030` (סטטוס `sese.031`).  
- `SttlmParams/PrtlSttlmInd` לשליטה בהתיישבות חלקית (`NPAR`, `PART`, `PARC`, `PARQ`).  
- `SttlmParams/SttlmTxCond/Cd` לתנאים ספציפיים לשוק (`NOMC` וכו').  
- כללי T2S אופציונלי משלוח ניירות ערך (CoSD) כאשר הם נתמכים.

#### הפניות
- SWIFT ניהול בטחונות MDR (`colr.010/011/012`).  
- מדריכי שימוש ב-CSD/T2S (למשל, DNB, ECB Insights) לקישור וסטטוסים.  
- תרגול יישוב SMPG, מדריכי Clearstream DCP, סדנאות ASX ISO.

### תרחיש B — פריצת חלון FX (כשל במימון PvP)

**משתתפים:** צדדים נגדיים וסוכני מזומנים, אפוטרופוס ניירות ערך, CSD/T2S  
**תזמון:** חלונות FX PvP (CLS/דו-צדדיים) וחתיכי CSD; לשמור את רגלי ניירות הערך בהמתנה עד לאישור מזומן.#### כוריאוגרפיית מסרים
1. העברת אשראי `pacs.009` FI-to-FI לכל מטבע עם `CtgyPurp/Cd = SECU`; סטטוס דרך `pacs.002`; אחזור/ביטול באמצעות `camt.056`/`camt.029`; אם כבר הוסדר, `pacs.004` החזר.  
2. הוראות `sese.023` DvP עם `HldInd=true` כך שחלק ניירות הערך ממתין לאישור מזומן.  
3. הודעות `sese.024` מחזור חיים (התקבלו/מתואמות/בהמתנה).  
4. אם שתי הרגליים `pacs.009` מגיעות ל-`ACSC` לפני שפג תוקפו של החלון ← שחרר עם `sese.030` ← `sese.031` (מצב מוד) ← `sese.025` (אישור).  
5. אם חלון המט"ח נפרץ ← בטל/חזר מזומן (`camt.056/029` או `pacs.004`) ובטל ניירות ערך (`sese.020` + `sese.027`, או `sese.027`, או Norito אם כבר אושר לכל ביטול שוק).

#### אישורים/סטטוסים נדרשים
- מזומן: `pacs.002` (`PDNG`, `ACSC`, `RJCT`), `pacs.004` להחזרות.  
- ניירות ערך: `sese.024` (סיבות ממתינות/כושלות כמו `NORE`, `ADEA`), `sese.025`.  
- הובלה: `admi.007` / דחיית שער לפני עיבוד עסקי.

#### תנאי / שחרור שדות
- `SttlmParams/HldInd` + `sese.030` שחרור/ביטול על הצלחה/כישלון.  
- `Lnkgs` לקשירת הוראות ניירות ערך לרגל המזומנים.  
- כלל T2S CoSD אם משתמשים במשלוח מותנה.  
- `PrtlSttlmInd` למניעת חלקים לא מכוונים.  
- ב-`pacs.009`, `CtgyPurp/Cd = SECU` מסמן מימון הקשור לניירות ערך.

#### הפניות
- הנחיות PMPG / CBPR+ לתשלומים בתהליכי ניירות ערך.  
- נוהלי יישוב SMPG, תובנות T2S על קישורים/החזקות.  
- מדריכי Clearstream DCP, תיעוד ECMS להודעות תחזוקה.

### pacs.004 הערות מיפוי חוזר

- אביזרי החזרה מנרמלים כעת את `ChrgBr` (`DEBT`/`CRED`/`SHAR`/`SLEV`) וסיבות החזרה קנייניות שנחשפו כ-`CRED`/`SHAR`/`SLEV`) וסיבות החזרה קנייניות חשופות כ-Norito והחזרה לצרכן. קודי מפעיל מבלי לנתח מחדש את מעטפת ה-XML.
- בלוקי חתימה של AppHdr בתוך מעטפות `DataPDU` נותרות התעלמות בעת בליעה; ביקורת צריכה להסתמך על מקור הערוץ ולא על שדות XMLDSIG משובצים.

### רשימת בדיקה תפעולית לגשר
- אכוף את הכוריאוגרפיה שלמעלה (בטחונות: `colr.010/011/012 → sese.023/024/025`; הפרת FX: `pacs.009 (+pacs.002) → sese.023 held → release/cancel`).  
- התייחס למצבי `sese.024`/`sese.025` ולתוצאות `pacs.002` כאותות שער; `ACSC` מפעיל שחרור, `RJCT` כוחות להירגע.  
- קידוד משלוח מותנה באמצעות `HldInd`, `Lnkgs`, `PrtlSttlmInd`, `SttlmTxCond` וכללי CoSD אופציונליים.  
- השתמש ב-`SupplementaryData` כדי לתאם מזהים חיצוניים (למשל, UETR עבור `pacs.009`) כאשר נדרש.  
- קבע את תזמון ההחזקה/התנתקות לפי לוח שנה/חתכים בשוק; בעיה `sese.030`/`camt.056` לפני מועדי ביטול, חזרה להחזרות בעת הצורך.

### עומסי ISO 20022 לדוגמה (מוערים)

#### זוג החלפת בטחונות (`sese.023`) עם הצמדת הוראות

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>SUBST-2025-04-001-A</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>FREE</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
      <sese:SttlmTxCond>
        <sese:Cd>NOMC</sese:Cd>
      </sese:SttlmTxCond>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>SUBST-2025-04-001-B</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Original collateral FoP back to giver -->
    <sese:FctvSttlmDt>2025-04-03</sese:FctvSttlmDt>
    <sese:SctiesMvmntDtls>
      <sese:SctiesId>
        <sese:ISIN>XS1234567890</sese:ISIN>
      </sese:SctiesId>
      <sese:Qty>
        <sese:QtyChc>
          <sese:Unit>1000</sese:Unit>
        </sese:QtyChc>
      </sese:Qty>
    </sese:SctiesMvmntDtls>
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```שלח את ההוראה המקושרת `SUBST-2025-04-001-B` (קבלת בטחונות חלופיים ב-FOP) עם `SctiesMvmntTp=RECE`, `Pmt=FREE`, וההצמדה `WITH` מפנה אל `SUBST-2025-04-001-A`. שחרר את שתי הרגליים עם `sese.030` תואם לאחר אישור ההחלפה.

#### חלק ניירות ערך בהמתנה בהמתנה לאישור FX (`sese.023` + `sese.030`)

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.023.001.11">
  <sese:SctiesSttlmTxInstr>
    <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
    <sese:SttlmTpAndAddtlParams>
      <sese:SctiesMvmntTp>DELI</sese:SctiesMvmntTp>
      <sese:Pmt>APMT</sese:Pmt>
    </sese:SttlmTpAndAddtlParams>
    <sese:SttlmParams>
      <sese:HldInd>true</sese:HldInd>
      <sese:PrtlSttlmInd>NPAR</sese:PrtlSttlmInd>
    </sese:SttlmParams>
    <sese:Lnkgs>
      <sese:Lnkg>
        <sese:Tp>
          <sese:Cd>WITH</sese:Cd>
        </sese:Tp>
        <sese:Ref>
          <sese:Prtry>PACS009-USD-CLS01</sese:Prtry>
        </sese:Ref>
      </sese:Lnkg>
    </sese:Lnkgs>
    <!-- Remaining settlement details omitted for brevity -->
  </sese:SctiesSttlmTxInstr>
</sese:Document>
```

שחרר ברגע ששתי רגלי `pacs.009` מגיעות ל-`ACSC`:

```xml
<sese:Document xmlns:sese="urn:iso:std:iso:20022:tech:xsd:sese.030.001.04">
  <sese:SctiesSttlmCondModReq>
    <sese:ReqDtls>
      <sese:TxId>DVP-2025-05-CLS01</sese:TxId>
      <sese:ChngTp>
        <sese:Cd>RELE</sese:Cd>
      </sese:ChngTp>
    </sese:ReqDtls>
  </sese:SctiesSttlmCondModReq>
</sese:Document>
```

`sese.031` מאשר את שחרור ההחזקה, ואחריו `sese.025` לאחר הזמנה של חלקת ניירות הערך.

#### רגל מימון PvP (`pacs.009` למטרת ניירות ערך)

```xml
<pacs:Document xmlns:pacs="urn:iso:std:iso:20022:tech:xsd:pacs.009.001.08">
  <pacs:FinInstnCdtTrf>
    <pacs:GrpHdr>
      <pacs:MsgId>PACS009-USD-CLS01</pacs:MsgId>
      <pacs:IntrBkSttlmDt>2025-05-07</pacs:IntrBkSttlmDt>
    </pacs:GrpHdr>
    <pacs:CdtTrfTxInf>
      <pacs:PmtId>
        <pacs:InstrId>DVP-2025-05-CLS01-USD</pacs:InstrId>
        <pacs:EndToEndId>SETTLEMENT-CLS01</pacs:EndToEndId>
      </pacs:PmtId>
      <pacs:PmtTpInf>
        <pacs:CtgyPurp>
          <pacs:Cd>SECU</pacs:Cd>
        </pacs:CtgyPurp>
      </pacs:PmtTpInf>
      <pacs:IntrBkSttlmAmt Ccy="USD">5000000.00</pacs:IntrBkSttlmAmt>
      <pacs:InstgAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKUS33XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstgAgt>
      <pacs:InstdAgt>
        <pacs:FinInstnId>
          <pacs:BICFI>BANKGB22XXX</pacs:BICFI>
        </pacs:FinInstnId>
      </pacs:InstdAgt>
      <pacs:SplmtryData>
        <pacs:Envlp>
          <nor:NoritoBridge xmlns:nor="urn:norito:settlement">
            <nor:SettlementId>DVP-2025-05-CLS01</nor:SettlementId>
            <nor:Atomicity>ALL_OR_NOTHING</nor:Atomicity>
          </nor:NoritoBridge>
        </pacs:Envlp>
      </pacs:SplmtryData>
    </pacs:CdtTrfTxInf>
  </pacs:FinInstnCdtTrf>
</pacs:Document>
```

`pacs.002` עוקב אחר מצב התשלום (`ACSC` = אושר, `RJCT` = דחייה). אם החלון נפרץ, חזור באמצעות `camt.056`/`camt.029` או שלח `pacs.004` כדי להחזיר כספים שסולקו.