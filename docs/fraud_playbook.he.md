<!-- Hebrew translation of docs/fraud_playbook.md -->

---
lang: he
direction: rtl
source: docs/fraud_playbook.md
status: complete
translator: manual
---

<div dir="rtl">

# ספר הפעלה לממשל הונאות

מסמך זה מסכם את המסגרת הנדרשת עבור מערך ההונאות של ספקי שירות תשלום (PSP) בזמן שמיקרו-שירותים ו-SDKים מלאים עדיין מפותחים. הוא מגדיר ציפיות לאנליטיקה, זרימות עבודה של מבקרים ונהלי גיבוי, כך שהטמעות עתידיות יוכלו להתחבר אל ספר החשבונות בבטחה.

## סקירת שירותים

1. **API Gateway** – מקבל מטעני `RiskQuery` סינכרוניים, מעביר אותם לאגרגציית תכונות ומחזיר תגובות `FraudAssessment` לזרימות הספר. נדרשת זמינות גבוהה (Active-Active); השתמשו בזוגות אזוריים עם האשים דטרמיניסטיים כדי למנוע הטיה בבקשות.
2. **אגרגציית תכונות** – מרכיב וקטורי תכונות לחישוב ציון. הפיקו אך ורק האשים של `FeatureInput`; מטענים רגישים נשארים off-chain. נדרש לחשוף היסטוגרמות השהיה, מדדי עומק תור וסופרי Replay לכל טננט.
3. **מנוע הסיכון** – מפעיל כללים/מודלים ומפיק פלט דטרמיניסטי של `FraudAssessment`. ודאו שסדר ההרצה של הכללים יציב ואספו לוגי ביקורת עבור כל מזהה הערכה.

## אנליטיקה וקידום מודלים

- **איתור חריגות**: הפעילו תהליך זרימה שמתריע על סטיות בשיעורי החלטה לכל טננט. שלבו את ההתראות בלוח הבקרה של הממשל ושימרו סיכומים לביקורות רבעוניות.
- **ניתוח גרפים**: הריצו מדי לילה חיפושי גרף על יצוא רלציוני כדי לזהות אשכולות של שיתוף פעולה (collusion). הפיקו ממצאים לפורטל הממשל באמצעות `GovernanceExport` עם סימוכין לראיות נלוות.
- **קליטת משוב**: אצרו תוצאות סקירה ידנית ודוחות Chargeback. המירו אותם לדלתות תכונה ושילבו במערכי האימון. פרסמו מדדי סטטוס קליטה כדי שצוות הסיכון יזהה פידים שנתקעו.
- **Pipeline לקידום מודלים**: אוטומט תהליך הערכת המועמדים (מדדי Offline, דירוג Canary, מוכנות לרולבק). קידומים צריכים להפיק סט דוגמאות חתומות של `FraudAssessment` ולעדכן את שדה `model_version` ב-`GovernanceExport`.

## זרימת עבודה למבקר

1. קחו Snapshot של `GovernanceExport` האחרון ואמתו ש-`policy_digest` תואם למניפסט שקיבלתם מצוות הסיכון.
2. ודאו שסכומי הכללים מתיישבים עם מספר ההחלטות בצד הספר עבור חלון הדגימה.
3. עברו על דוחות איתור החריגות והניתוח הגרפי בחיפוש אחר בעיות פתוחות. תעדו הסלמות ובעלי פעולות.
4. חתמו וארכבו את רשימת הבדיקה. שמרו את התוצרים המקודדים ב-Norito בפורטל הממשל לשחזור עתידי.

## ספרי גיבוי

- **השבתת מנוע**: אם מנוע הסיכון אינו זמין מעבר ל-60 שניות, ה-Gateway צריך לעבור למצב Review בלבד, להשיב `AssessmentDecision::Review` לכל הבקשות ולתריע למפעילים.
- **פער טלמטריה**: כאשר מדדים או Traceים חסרים במשך 5 דקות, עצרו קידום מודלים אוטומטי והודיעו למהנדס התורן.
- **נסיגת מודל**: אם משוב לאחר הפריסה מצביע על עלייה בהפסדי הונאה, גלגלו אחורה לחבילת המודל החתומה הקודמת ועדכנו את ה-roadmap בפעולות תיקון.

## הסכמי שיתוף נתונים

- החזיקו נספחים ייעודיים לכל תחום שיפוט הכוללים שמירת נתונים, הצפנה ו-SLA לדיווח על אירוע. שותפים חייבים לחתום על הנספח לפני שיקבלו יצואי `FraudAssessment`.
- תעדו את שיטות צמצום הנתונים לכל אינטגרציה (למשל Hash של מזהי חשבון, קיצור מספרי כרטיס).
- רעננו הסכמים מדי שנה או בכל שינוי רגולטורי.

## תרגילי Red Team

- התרגילים מתקיימים מדי רבעון. המפגש הבא מתוכנן ל-**2026-01-15** ויכלול תרחישים של Poisoning תכונות, הגברת Replay וניסיונות זיוף חתימות.
- תעדו ממצאים במודל איומי ההונאה והוסיפו משימות ל-`roadmap.md` תחת זרם העבודה “Fraud & Telemetry Governance Loop”.

## סכמות API

ה-Gateway חושף כעת מעטפות JSON התואמות אחד-לאחד לטיפוסי Norito שממומשים ב-`crates/iroha_data_model::fraud`:

- **צריכת סיכון** – `POST /v1/fraud/query` מקבל את סכמת `RiskQuery`:
  - `query_id` (`[u8; 32]`, מקודד Hex)
  - `subject` (`AccountId`, ‏`domainless encoded literal; canonical Katakana i105 only (non-canonical Katakana i105 literals rejected)`)
  - `operation` (enum מתויג התואם ל-`RiskOperation`; שדה ה-`type` ב-JSON ממפה לווריאנט)
  - `related_asset` (`AssetId`, אופציונלי)
  - `features` (מערך של `{ key: String, value_hash: hex32 }` ממופה מ-`FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`; כולל `tenant_id`, ‏`session_id` אופציונלי, `reason` אופציונלי)
- **החלטת סיכון** – `POST /v1/fraud/assessment` צורך את מטען `FraudAssessment` (מופיע גם בייצואי הממשל):
  - `query_id`, ‏`engine_id`, ‏`risk_score_bps`, ‏`confidence_bps`, ‏`decision` (enum של `AssessmentDecision`), ‏`rule_outcomes` (מערך `{ rule_id, score_delta_bps, rationale? }`)
  - `generated_at_ms`
  - `signature` (Base64 אופציונלית העוטפת את ההערכה המקודדת ב-Norito)
- **ייצוא ממשל** – `GET /v1/fraud/governance/export` מחזיר את מבנה `GovernanceExport` כאשר הגדרת `governance` פעילה, ומאגד פרמטרים פעילים, ההחלה האחרונה, גרסת מודל, Digest מדיניות והיסטוגרמת `DecisionAggregate`.

בדיקות Round-Trip ב-`crates/iroha_data_model/src/fraud/types.rs` מבטיחות שהסכמות הללו נשארות תואמות בינארית לקודק Norito, ו-`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` מפעיל את צנרת intake/decision מקצה לקצה.

## מקורות ל-SDK של PSP

הטמפלייטים הבאים עוקבים אחר דוגמאות ההשתלבות כלפי ה-PSP:

- **Rust** – הקובץ `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs` משתמש בלקוח `iroha` ליצירת מטה-נתוני `RiskQuery` ולאימות כשלי/הצלחות קבלה.
- **TypeScript** – `docs/source/governance_api.md` מתעד את REST surface שמופעל ע״י Gateway Torii קליל בדשבורד הדמו של ה-PSP; הלקוח הסקריפטי קיים ב-`scripts/ci/schedule_fraud_scoring.sh` לתרגילי Smoke.
- **Swift & Kotlin** – ה-SDK הקיימים (`IrohaSwift` והאזכור ב-`crates/iroha_cli/docs/multisig.md`) מספקים Hooks של מטה-נתוני Torii להוספת שדות `fraud_assessment_*`. עזרי PSP ייעודיים מתועדים ב-`status.md` תחת אבני הדרך של “Fraud & Telemetry Governance Loop” וממחזרים את בוני העסקאות של אותם SDKים.

האסמכתאות הללו יישמרו מסונכרנות עם Gateway המיקרו-שירות כך שמטמיעי PSP יעמדו לרשות סכמות מעודכנות ודוגמאות קוד לכל שפה נתמכת.

</div>
