---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/multi-source-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a510a005b4b14df18915a1166620f78e8a99fe9042de80bb916d7c3f38d32d9
source_last_modified: "2026-07-26T10:32:32.940432+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: multi-source-rollout
title: ראנבוק rollout רב-מקורות וחסימת ספקים
sidebar_label: ראנבוק rollout רב-מקורות
description: רשימת בדיקה תפעולית לרולאאוט רב-מקורות בשלבים וחסימת ספקים במקרי חירום.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/runbooks/multi_source_rollout.md`. שמרו על סנכרון שתי הגרסאות עד שהסט הישן של התיעוד יוסר.
:::

## מטרה

ראנבוק זה מנחה את צוותי ה-SRE והמהנדסים בתורנות בשני תהליכים קריטיים:

1. לפרוס את האורקסטרטור רב-המקורות בגלים מבוקרים.
2. להכניס לרשימת חסימה או להוריד עדיפות של ספקים בעייתיים בלי לערער סשנים קיימים.

הוא מניח שמערך האורקסטרציה שסופק תחת SF-6 כבר פרוס (`sorafs_orchestrator`, API טווחי הצ'אנקים של ה-gateway, exporters של טלמטריה).

> **ראו גם:** [ראנבוק תפעול לאורקסטרטור](./orchestrator-ops.md) מפרט נהלים לכל ריצה (לכידת scoreboard, טוגלים לרולאאוט מדורג, ו-rollback). השתמשו בשתי ההפניות יחד בעת שינויים חיים.

## 1. בדיקות טרום-הפעלה

1. **לאשר קלטי ממשל.**
   - כל ספק מועמד חייב לפרסם מעטפות `ProviderAdvertV1` עם payloads של יכולות טווח ותקציבי זרם. בדקו דרך `/v1/sorafs/providers` והשוו לשדות היכולת הצפויים.
   - snapshots של טלמטריה המספקים שיעורי השהיה/כשל צריכים להיות בני פחות מ-15 דקות לפני כל הרצת canary.
2. **להכין את הקונפיגורציה.**
   - Store the canonical client-side orchestrator JSON at `/etc/iroha/sorafs/orchestrator.json`. This file is consumed explicitly by client/orchestrator workloads; it is not an `iroha_config` or Torii namespace. Update it with rollout-specific limits (`max_providers`, retry budgets), deploy the same reviewed file to staging/production, and pass it to every CLI fetch as `--orchestrator-config=/etc/iroha/sorafs/orchestrator.json`.
3. **להריץ fixtures קנוניים.**
   - מלאו משתני סביבה של manifest/token והריצו fetch דטרמיניסטי:

     ```bash
     sorafs_cli fetch \
       --plan fixtures/sorafs_manifest/ci_sample/payload.plan.json \
       --orchestrator-config=/etc/iroha/sorafs/orchestrator.json \
       --manifest-id "$CANARY_MANIFEST_ID" \
       --provider name=alpha,provider-id="$PROVIDER_ALPHA_ID",gateway-key="$PROVIDER_ALPHA_GATEWAY_KEY",base-url=https://gw-alpha.example,stream-token="$PROVIDER_ALPHA_TOKEN" \
       --provider name=beta,provider-id="$PROVIDER_BETA_ID",gateway-key="$PROVIDER_BETA_GATEWAY_KEY",base-url=https://gw-beta.example,stream-token="$PROVIDER_BETA_TOKEN" \
       --provider name=gamma,provider-id="$PROVIDER_GAMMA_ID",gateway-key="$PROVIDER_GAMMA_GATEWAY_KEY",base-url=https://gw-gamma.example,stream-token="$PROVIDER_GAMMA_TOKEN" \
       --max-peers=3 \
       --retry-budget=4 \
       --scoreboard-out artifacts/canary.scoreboard.json \
       --json-out artifacts/canary.fetch.json
     ```

     משתני הסביבה צריכים לכלול את digest של payload המניפסט (hex) ואת טוקני ה-stream המקודדים ב-base64 לכל ספק המשתתף ב-canary.
   - בצעו diff של `artifacts/canary.scoreboard.json` מול הריליס הקודם. כל ספק חדש שאינו כשיר או שינוי משקל >10% דורש סקירה.
4. **לוודא שהטלמטריה מחוברת.**
   - פתחו את ייצוא Grafana ב-`docs/examples/sorafs_fetch_dashboard.json`. ודאו שמדדי `sorafs_orchestrator_*` מופיעים ב-staging לפני שממשיכים.

## 2. חסימת ספקים במקרי חירום

פעלו לפי הנוהל הזה כאשר ספק מגיש צ'אנקים פגומים, חווה timeouts מתמשכים, או נכשל בבדיקות ציות.

1. **לאסוף ראיות.**
   - ייצאו את סיכום ה-fetch האחרון (פלט `--json-out`). רשמו אינדקסים של צ'אנקים שנכשלו, aliases של ספקים וחוסר התאמה ב-digest.
   - שמרו קטעי לוג רלוונטיים מתוך ה-targets `telemetry::sorafs.fetch.*`.
2. **להחיל override מיידי.**
   - סמנו את הספק כ-penalized בסנאפשוט הטלמטריה שמופץ לאורקסטרטור (הגדירו `penalty=true` או הגבילו את `token_health` ל-`0`). בניית ה-scoreboard הבאה תחריג את הספק אוטומטית.
   - לבדיקות smoke אד-הוק, העבירו `--deny-provider gw-alpha` ל-`sorafs_cli fetch` כדי להפעיל את מסלול הכשל בלי להמתין להפצת הטלמטריה.
   - פרסו מחדש את חבילת הטלמטריה/הקונפיגורציה המעודכנת בסביבה המושפעת (staging → canary → production). תעדו את השינוי בלוג האירוע.
3. **לאמת את ה-override.**
   - הריצו שוב את ה-fetch של ה-fixture הקנוני. אשרו שה-scoreboard מסמן את הספק כלא כשיר עם הסיבה `policy_denied`.
   - בדקו את `sorafs_orchestrator_provider_failures_total` כדי לוודא שהמונה הפסיק לעלות עבור הספק שנחסם.
4. **להסלים חסימות ממושכות.**
   - אם הספק יישאר חסום ליותר מ-24 h, פתחו טיקט ממשל לצורך רוטציה או השעיה של ה-advert שלו. עד שההצבעה עוברת, שמרו על deny list ורעננו snapshots של טלמטריה כדי שהספק לא יחזור ל-scoreboard.
5. **פרוטוקול rollback.**
   - כדי להחזיר את הספק, הסירו אותו מה-deny list, פרסו מחדש, וקחו snapshot חדש של scoreboard. צרפו את השינוי לפוסט-מורטם של האירוע.

## 3. תוכנית rollout מדורגת

| שלב | היקף | אותות נדרשים | קריטריוני Go/No-Go |
|-----|------|--------------|--------------------|
| **Lab** | קלאסטר אינטגרציה ייעודי | fetch ידני ב-CLI מול payloads של fixtures | כל הצ'אנקים מצליחים, מוני כשל ספק נשארים על 0, יחס retries < 5%. |
| **Staging** | Staging מלא של control-plane | דשבורד Grafana מחובר; כללי התראה במצב warning-only | `sorafs_orchestrator_active_fetches` חוזר לאפס אחרי כל ריצת בדיקה; אין התראות `warn/critical`. |
| **Canary** | ≤10% מתעבורת הייצור | ה-pager מושתק אך הטלמטריה מנוטרת בזמן אמת | יחס retries < 10%, כשלי ספקים מוגבלים ל-peers רועשים ידועים, היסטוגרמת latency תואמת את קו הבסיס של staging ±20%. |
| **General Availability** | rollout של 100% | כללי pager פעילים | אפס שגיאות `NoHealthyProviders` במשך 24 h, יחס retries יציב, פאנלי SLA ירוקים בדשבורד. |

לכל שלב:

1. עדכנו את JSON האורקסטרטור עם `max_providers` ו-budgets של retries המתוכננים.
2. הריצו `sorafs_cli fetch` או את סוויטת בדיקות האינטגרציה של ה-SDK מול ה-fixture הקנוני ומניפסט מייצג מהסביבה.
3. תפסו את artefacts של scoreboard + summary וצרפו אותם לרשומת הריליס.
4. סקרו את דשבורדי הטלמטריה עם המהנדס בתורנות לפני מעבר לשלב הבא.

## 4. תצפיתיות ו-hooks לאירועים

- **מדדים:** ודאו ש-Alertmanager מנטר `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` ו-`sorafs_orchestrator_retries_total`. זינוק פתאומי בדרך כלל אומר שספק מדרדר תחת עומס.
- **לוגים:** נתבו את targets `telemetry::sorafs.fetch.*` לאגרגטור הלוגים המשותף. צרו חיפושים שמורים עבור `event=complete status=failed` כדי להאיץ טריאז'.
- **Scoreboards:** שמרו כל artefact של scoreboard באחסון ארוך טווח. ה-JSON משמש גם כשרשרת ראיות לביקורות ציות ול-rollbacks מדורגים.
- **דשבורדים:** שכפלו את דשבורד Grafana הקנוני (`docs/examples/sorafs_fetch_dashboard.json`) לתיקיית הפרודקשן עם כללי ההתראה מ-`docs/examples/sorafs_fetch_alerts.yaml`.

## 5. תקשורת ותיעוד

- רשמו כל שינוי deny/boost ב-changelog התפעולי עם חותמת זמן, מפעיל, סיבה ואירוע קשור.
- עדכנו את צוותי ה-SDK כאשר משקלי ספקים או budgets של retries משתנים כדי ליישר ציפיות בצד הלקוח.
- לאחר השלמת GA, עדכנו את `status.md` בסיכום ה-rollout וארכבו את הפניה לראנבוק הזה בהערות השחרור.
