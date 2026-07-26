<!-- Hebrew translation of docs/source/project_tracker/2026_q1_clarification_prompts.md -->

---
lang: he
direction: rtl
source: docs/source/project_tracker/2026_q1_clarification_prompts.md
status: complete
translator: manual
---

<div dir="rtl">

# פרומפטים להבהרה — רבעון 1 ‎2026

המסמך מרכז פרומפטים מוכנים ל-LLM עבור פריטי מפת הדרכים הפתוחים ברבעון 1 ‎2026. העתקו את הקטע הרלוונטי לשרשור התיאום, החליפו את הסוגריים המשולשים וצרפו דיפים או לוגים לפני הפניה ל-‎@mtakemiya‎.

## Kaigi Privacy שלב 3 — שכבת Relay ומוקדי ממשל

*(הושלם: רשימות מורשות, דיווחי בריאות, טלמטריה וכלי failover הוטמעו; אין צורך בפרומפט נוסף.)*

## NPoS Sumeragi — Gates לקבלת Restart ו-Randomness

````markdown
אנחנו נערכים לסגור את **Sumeragi Phase A, סעיפים A3/A4** (חיות restart ושילוב randomness VRF) וצריכים הבהרה לגבי Gates הקבלה.

**צילום מצב נוכחי**
- חיבור pacemaker מבוסס EMA קיים; כיסוי restart חלקי (`crates/iroha_core/src/sumeragi/mod.rs`, `crates/iroha_core/tests/sumeragi_restart.rs`).
- plumbing של commit/reveal ל-VRF מוכן אך חסרות מדיניות/תיעוד.
- טיוטות harness כאוס מתועדות ב-`docs/source/sumeragi_pacemaker.md`.

**שאלות עבורך**
1. אילו תרחישי restart חייבים להיכלל לפני שנוכל להכריז על Milestone A3 כהושלם (cold boot, חילופי מנהיג, נפילת ולידטורים סימולטנית וכו')?
2. מהם קריטריוני הקבלה המדויקים ל-VRF (מקור אנטרופיה, אורך חלון commit, טלמטריה לביקורת) עבור Milestone A4?
3. האם harness כאוס/ביצועים צריך לכלול ספי throughput/latency מינימליים, או שבשלב זה מספיק קריטריון איכותי?

נעדכן את הסקריפטים ואת runbook בהתאם להכוונה.
````

## מסלול שחרור כפול Iroha 2/3 — החלטות Build ו-Packaging

````markdown
אנו מגדירים את **matrix הבנייה הכפולה עבור Iroha 2 ו-Iroha 3** וזקוקים להכרעות לפני בניית ה-CI והאוטומציה.

**צילום מצב נוכחי**
- פריטי מפת דרכים 1364–1375 מתארים את המשימות; אין עדיין pipeline משותף.
- מניפסטי השחרור מניחים משפחת ארטיפקט יחידה.
- מדריכי המפעיל (`docs/source/release_procedure.org`) מכסים רק את ה-flow הישן.

**שאלות עבורך**
1. איך עלינו לקרוא ולסמן את הארטיפקטים כדי ש-Sora Nexus תבחר ב-Iroha 3, בעוד ששאר הרשתות יקבלו כברירת מחדל את Iroha 2?
2. האם יש knobs שצריך לחשוף דרך `iroha_config` כדי שמפעילים יוודאו שהתקינו את הגרסה הנכונה?
3. אילו יעדי חתימה והפצה נחשבים סמכותיים לכל משפחת build (רג'יסטרי של קונטיינרים, דליי חבילות, מניפסטי checksum)?

לאחר קבלת תשובות נוכל לסגור את תוכנית ה-CI ולעדכן את תיעוד המפעילים.
````

</div>
