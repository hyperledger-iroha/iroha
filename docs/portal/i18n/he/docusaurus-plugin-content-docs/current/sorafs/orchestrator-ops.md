---
id: orchestrator-ops
lang: he
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-ops.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: ראנבוק תפעול לאורקסטרטור של SoraFS
sidebar_label: ראנבוק אורקסטרטור
description: מדריך תפעולי צעד-אחר-צעד לפריסה, ניטור וגלגול אחורה של אורקסטרטור רב-מקורות.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/runbooks/sorafs_orchestrator_ops.md`. שמרו על סנכרון שתי הגרסאות עד שהסט הישן של תיעוד Sphinx יעבור מיגרציה מלאה.
:::

ראנבוק זה מנחה את צוותי ה-SRE בהכנה, בפריסה ובתפעול של אורקסטרטור fetch רב-מקורות. הוא משלים את מדריך המפתחים עם נהלים המותאמים לפריסות ייצור, כולל הפעלה מדורגת וחסימת peers.

> **ראו גם:** [ראנבוק ה-rollout רב-המקורות](./multi-source-rollout.md) מתמקד בגלי rollout ברמת הצי ובהדחת ספקים במצבי חירום. השתמשו בו לתיאום ממשל / staging בזמן שאתם משתמשים במסמך זה לתפעול היומיומי של האורקסטרטור.

## 1. רשימת בדיקה לפני ההפעלה

1. **איסוף קלטים מספקים**
   - פרסומי ספקים אחרונים (`ProviderAdvertV1`) ותמונת טלמטריה עבור הצי היעד.
   - תוכנית payload (`plan.json`) שמקורה במניפסט הנבחן.
2. **בניית scoreboard דטרמיניסטי**

   ```bash
   sorafs_fetch \
     --plan fixtures/plan.json \
     --telemetry-json fixtures/telemetry.json \
     --provider alpha=fixtures/provider-alpha.bin \
     --provider beta=fixtures/provider-beta.bin \
     --provider gamma=fixtures/provider-gamma.bin \
     --scoreboard-out artifacts/scoreboard.json \
     --json-out artifacts/session.summary.json
   ```

   - ודאו ש-`artifacts/scoreboard.json` כולל כל ספק ייצור כ-`eligible`.
   - ארכבו את ה-JSON המסכם לצד ה-scoreboard; מבקרים מסתמכים על מוני נסיונות חוזרים של chunks בעת אישור בקשת השינוי.
3. **Dry-run עם fixtures** — הריצו את אותה פקודה מול ה-fixtures הציבוריים שב-`docs/examples/sorafs_ci_sample/` כדי לוודא שהבינארי של האורקסטרטור תואם לגרסה הצפויה לפני שנוגעים ב-payloads של הייצור.

## 2. נוהל rollout מדורג

1. **שלב קנרית (≤2 ספקים)**
   - בנו מחדש את ה-scoreboard והריצו עם `--max-peers=2` כדי להגביל את האורקסטרטור לתת-קבוצה קטנה.
   - ניטור:
     - `sorafs_orchestrator_active_fetches`
     - `sorafs_orchestrator_fetch_failures_total{reason!="retry"}`
     - `sorafs_orchestrator_retries_total`
   - המשיכו כאשר שיעורי הנסיונות החוזרים נשארים מתחת ל-1% עבור fetch מלא של המניפסט ואין ספק שצובר תקלות.
2. **שלב רמפה (50% ספקים)**
   - הגדילו את `--max-peers` והריצו שוב עם תמונת טלמטריה עדכנית.
   - שימרו כל ריצה באמצעות `--provider-metrics-out` ו-`--chunk-receipts-out`. שמרו את הארטיפקטים למשך ≥7 ימים.
3. **Rollout מלא**
   - הסירו את `--max-peers` (או הגדירו אותו למספר המלא של הספקים הזכאים).
   - הפעילו מצב אורקסטרטור בפריסות הלקוחות: הפיצו את ה-scoreboard המתמיד ואת קובץ ה-JSON של ההגדרות דרך מערכת ניהול התצורה שלכם.
   - עדכנו לוחות מחוונים כך שיציגו `sorafs_orchestrator_fetch_duration_ms` p95/p99 והיסטוגרמות נסיונות חוזרים לפי אזור.

## 3. חסימה וחיזוק peers

השתמשו בעקיפות מדיניות הניקוד של ה-CLI כדי לטפל בספקים בעייתיים בלי להמתין לעדכוני ממשל.

```bash
sorafs_fetch \
  --plan fixtures/plan.json \
  --telemetry-json fixtures/telemetry.json \
  --provider alpha=fixtures/provider-alpha.bin \
  --provider beta=fixtures/provider-beta.bin \
  --provider gamma=fixtures/provider-gamma.bin \
  --deny-provider=beta \
  --boost-provider=gamma=5 \
  --json-out artifacts/override.summary.json
```

- `--deny-provider` מסיר את הכינוי המוגדר מהשיקול בסשן הנוכחי.
- `--boost-provider=<alias>=<weight>` מעלה את משקל הספק במתזמן. הערכים מצטברים למשקל ה-scoreboard המנורמל וחלים רק על הריצה המקומית.
- תעדו את העקיפות בכרטיס האירוע וצרפו את פלטי ה-JSON כדי שהצוות האחראי יוכל להתאים את המצב לאחר שהתיקון הבסיסי יושלם.

לשינויים קבועים, עדכנו את הטלמטריה המקורית (סמנו את העבריין כ-penalised) או רעננו את ההודעה עם תקציבי זרימה מעודכנים לפני ניקוי עקיפות ה-CLI.

## 4. אבחון תקלות

כאשר fetch נכשל:

1. אספו את הארטיפקטים הבאים לפני הרצה מחדש:
   - `scoreboard.json`
   - `session.summary.json`
   - `chunk_receipts.json`
   - `provider_metrics.json`
2. בדקו את `session.summary.json` עבור מחרוזת השגיאה הקריאה:
   - `no providers were supplied` → ודאו את נתיבי הספקים והפרסומים.
   - `retry budget exhausted ...` → הגדילו את `--retry-budget` או הסירו peers לא יציבים.
   - `no compatible providers available ...` → בצעו ביקורת למטא-דאטה של יכולות הטווח של הספק הבעייתי.
3. הצליבו את שם הספק עם `sorafs_orchestrator_provider_failures_total` ופתחו כרטיס מעקב אם המדד קופץ.
4. שחזרו את ה-fetch במצב אופליין עם `--scoreboard-json` והטלמטריה שנלכדה כדי לשחזר את הכשל בצורה דטרמיניסטית.

## 5. Rollback

כדי להחזיר rollout של האורקסטרטור:

1. הפיצו תצורה שמגדירה `--max-peers=1` (משביתה למעשה את התזמון רב-המקורות) או החזירו את הלקוחות למסלול fetch מדור קודם של מקור יחיד.
2. הסירו כל עקיפה של `--boost-provider` כדי שה-scoreboard יחזור למשקל ניטרלי.
3. המשיכו לאסוף את מדדי האורקסטרטור למשך לפחות יום אחד כדי לוודא שלא נותרו פעולות fetch פעילות.

שמירה על לכידה ממושמעת של ארטיפקטים ו-rollout מדורג מבטיחה שהאורקסטרטור רב-המקורות יופעל בבטחה על פני ציי ספקים הטרוגניים תוך שמירה על דרישות ניטור וביקורת.
