---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c3f62f39df7554547b720a12b13bd818e8876848fc59f4100f787d73362503fe
source_last_modified: "2025-11-08T20:22:29.776274+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: chunker-registry-rollout-checklist
title: צ'קליסט rollout לרישום chunker של SoraFS
sidebar_label: צ'קליסט rollout chunker
description: תכנית rollout שלב-אחר-שלב לעדכוני רישום chunker.
---

:::note מקור קנוני
משקף את `docs/source/sorafs/chunker_registry_rollout_checklist.md`. שמרו על שתי העתקות מסונכרנות עד שמערכת התיעוד הישנה של Sphinx תופסק לחלוטין.
:::

# צ'קליסט rollout לרישום SoraFS

צ'קליסט זה מרכז את השלבים הנדרשים לקידום פרופיל chunker חדש או bundle של admission
לספקים משלב review לפרודקשן לאחר שהאמנה הממשלית אושרה.

> **תחולה:** חל על כל ריליס שמשנה את
> `sorafs_manifest::chunker_registry`, envelopes של admission לספקים, או bundles
> של fixtures קנוניים (`fixtures/sorafs_chunker/*`).

## 1. ולידציית pre-flight

1. צרו מחדש fixtures ואמתו דטרמיניזם:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. ודאו שה-hashes של הדטרמיניזם ב-
   `docs/source/sorafs/reports/sf1_determinism.md` (או הדו"ח הרלוונטי לפרופיל)
   תואמים לארטיפקטים שנוצרו מחדש.
3. ודאו ש-`sorafs_manifest::chunker_registry` עובר קומפילציה עם
   `ensure_charter_compliance()` באמצעות:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. עדכנו את תיק ההצעה:
   - `docs/source/sorafs/proposals/<profile>.json`
   - רשומת minutes של המועצה תחת `docs/source/sorafs/council_minutes_*.md`
   - דו"ח דטרמיניזם

## 2. אישור ממשל

1. הציגו את דו"ח Tooling Working Group ואת digest ההצעה בפני
   Sora Parliament Infrastructure Panel.
2. תעדו את פרטי האישור ב-
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. פרסמו את ה-envelope החתום על ידי הפרלמנט לצד ה-fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. ודאו שה-envelope נגיש דרך helper של governance fetch:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. rollout ב-staging

ראו את [playbook של staging manifest](./staging-manifest-playbook) לתיאור מפורט.

1. פרסו Torii עם discovery `torii.sorafs` מופעל ועם enforcement של admission
   דולק (`enforce_admission = true`).
2. דחפו את envelopes של admission שאושרו לספריית הרישום של staging המופנית
   על ידי `torii.sorafs.discovery.admission.envelopes_dir`.
3. ודאו ש-provider adverts מתפשטים דרך API ה-discovery:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. הפעילו את endpoints של manifest/plan עם headers של governance:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. ודאו שדשבורדי הטלמטריה (`torii_sorafs_*`) וחוקי ההתראה מדווחים את הפרופיל החדש
   ללא שגיאות.

## 4. rollout בפרודקשן

1. חזרו על שלבי ה-staging מול צמתי Torii בפרודקשן.
2. הכריזו על חלון ההפעלה (תאריך/שעה, תקופת חסד, תכנית rollback) לערוצי
   המפעילים וה-SDK.
3. מיזגו את PR הריליס שכולל:
   - Fixtures ו-envelope מעודכנים
   - שינויי תיעוד (הפניות לאמנה, דו"ח דטרמיניזם)
   - רענון roadmap/status
4. תייגו את הריליס וארכבו את הארטיפקטים החתומים לצורך provenance.

## 5. Audit לאחר rollout

1. אספו מדדים סופיים (ספירות discovery, שיעור הצלחת fetch, histograms של שגיאות)
   24 שעות אחרי rollout.
2. עדכנו את `status.md` בסיכום קצר ובקישור לדו"ח הדטרמיניזם.
3. פתחו משימות המשך (למשל, guidance נוספת לכתיבת פרופילים) ב-`roadmap.md`.
