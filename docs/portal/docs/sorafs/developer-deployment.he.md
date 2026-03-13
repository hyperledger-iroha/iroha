---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f370cbc81b9e8f340a24d561d94f49cf154413781f7194505a712758cd4c85aa
source_last_modified: "2025-11-10T05:16:44.297906+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: developer-deployment
title: הערות פריסה של SoraFS
sidebar_label: הערות פריסה
description: צ'קליסט לקידום pipeline SoraFS מ-CI לפרודקשן.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/developer/deployment.md`. שמרו על שתי הגרסאות מסונכרנות עד שהמסמכים הישנים ייצאו משימוש.
:::

# הערות פריסה

זרימת האריזה של SoraFS מחזקת דטרמיניזם, ולכן המעבר מ-CI לפרודקשן דורש בעיקר guardrails תפעוליים. השתמשו בצ'קליסט הזה בעת rollout של הכלים ל-gateways ולספקי אחסון אמיתיים.

## בדיקות מקדימות

- **יישור רישום** — ודאו שפרופילי chunker וה-manifests מפנים לאותה טופס `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **מדיניות admission** — עברו על adverts חתומים של ספקים ועל alias proofs הנדרשים ל-`manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook של pin registry** — החזיקו את `docs/source/sorafs/runbooks/pin_registry_ops.md` לשימוש בסצנריו התאוששות (סיבוב alias, כשלי רפליקציה).

## קונפיגורציית סביבה

- Gateways חייבים להפעיל את endpoint של proof streaming (`POST /v2/sorafs/proof/stream`) כדי שה-CLI יוכל לפלוט סיכומי טלמטריה.
- הגדירו מדיניות `sorafs_alias_cache` באמצעות ברירות המחדל ב-`iroha_config` או בעזרת helper של ה-CLI (`sorafs_cli manifest submit --alias-*`).
- ספקו stream tokens (או אישורי Torii) באמצעות מנהל סודות מאובטח.
- הפעילו exporters של טלמטריה (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) ושלחו אותם ל-stack של Prometheus/OTel.

## אסטרטגיית rollout

1. **Manifests של blue/green**
   - השתמשו ב-`manifest submit --summary-out` כדי לארכב תגובות לכל rollout.
   - שימו לב ל-`torii_sorafs_gateway_refusals_total` כדי לזהות mismatches של יכולות מוקדם.
2. **אימות proofs**
   - התייחסו לכשלים ב-`sorafs_cli proof stream` כאל חוסמי פריסה; קפיצות לטנטיות מצביעות לעיתים על throttling של ספק או tiers לא מוגדרים נכון.
   - `proof verify` צריך להיות חלק מ-smoke test שלאחר pin כדי לוודא שה-CAR שמאוחסן אצל הספקים עדיין תואם ל-digest של manifest.
3. **Dashboards של טלמטריה**
   - ייבאו את `docs/examples/sorafs_proof_streaming_dashboard.json` ל-Grafana.
   - הוסיפו פאנלים לבריאות pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) ולסטטיסטיקות chunk range.
4. **הפעלה רב-מקורית**
   - עקבו אחרי צעדי rollout מדורג ב-`docs/source/sorafs/runbooks/multi_source_rollout.md` בעת הפעלת האורקסטרטור, וארכבו ארטיפקטים של scoreboard/טלמטריה לצורכי ביקורת.

## טיפול בתקריות

- עקבו אחרי מסלולי ההסלמה ב-`docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` עבור תקלות gateway ומיצוי stream-token.
  - `dispute_revocation_runbook.md` כשמתרחשות מחלוקות רפליקציה.
  - `sorafs_node_ops.md` עבור תחזוקה ברמת הצומת.
  - `multi_source_rollout.md` עבור overrides של האורקסטרטור, blacklisting של peers ו-rollouts מדורגים.
- תעדו כשלי proofs ואנומליות לטנטיות ב-GovernanceLog דרך ממשקי PoR tracker הקיימים כדי שהממשל יוכל להעריך את ביצועי הספקים.

## צעדים הבאים

- שלבו אוטומציה של האורקסטרטור (`sorafs_car::multi_fetch`) כאשר orchestrator של multi-source fetch (SF-6b) נוחת.
- עקבו אחר שדרוגי PDP/PoTR תחת SF-13/SF-14; ה-CLI והמסמכים יתפתחו כדי לחשוף deadlines ובחירת tiers כאשר ה-proofs הללו יתייצבו.

בשילוב הערות הפריסה האלה עם ה-quickstart ועם מתכוני ה-CI, הצוותים יכולים לעבור מניסויים מקומיים ל-pipelines של SoraFS בפרודקשן באמצעות תהליך חוזר וניתן לתצפית.
