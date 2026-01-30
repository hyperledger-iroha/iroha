---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1200f94dd0dec510a4578581abd7b31e313d53eb7697b8f1d300b3e378c51c37
source_last_modified: "2025-11-08T20:16:30.133276+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: chunker-registry-charter
title: אמנת רישום chunker של SoraFS
sidebar_label: אמנת רישום chunker
description: אמנת ממשל להגשות ואישורים של פרופילי chunker.
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/chunker_registry_charter.md`. שמרו על שתי העתקות מסונכרנות עד שמערכת התיעוד הישנה של Sphinx תופסק לחלוטין.
:::

# אמנת ממשל לרישום chunker של SoraFS

> **אושררה:** 2025-10-29 על ידי Sora Parliament Infrastructure Panel (ראו
> `docs/source/sorafs/council_minutes_2025-10-29.md`). כל תיקון מחייב הצבעת ממשל רשמית;
> צוותי היישום חייבים להתייחס למסמך זה כנורמטיבי עד לאישור אמנה חלופית.

אמנה זו מגדירה את התהליך והתפקידים לפיתוח רישום ה-chunker של SoraFS.
היא משלימה את [מדריך כתיבת פרופילי chunker](./chunker-profile-authoring.md) בכך שהיא מתארת כיצד פרופילים חדשים
מוצעים, נבחנים, מאושרים ולבסוף יוצאים משימוש.

## תחולה

האמנה חלה על כל רשומה ב-`sorafs_manifest::chunker_registry` ועל
כל tooling שצורך את הרישום (manifest CLI, provider-advert CLI,
SDKs). היא אוכפת את האינווריאנטים של alias ו-handle שנבדקים על ידי
`chunker_registry::ensure_charter_compliance()`:

- מזהי פרופיל הם מספרים שלמים חיוביים שעולים בצורה מונוטונית.
- ה-handle הקנוני `namespace.name@semver` **חייב** להופיע כערך הראשון
  ב-`profile_aliases`. לאחריו מופיעים alias ישנים.
- מחרוזות alias מקוצצות, ייחודיות ואינן מתנגשות עם handles קנוניים של רשומות אחרות.

## תפקידים

- **Author(s)** – מכינים את ההצעה, מחדשים fixtures ואוספים ראיות לדטרמיניזם.
- **Tooling Working Group (TWG)** – מאמת את ההצעה לפי הצ'קליסטים שפורסמו ומוודא שהאינווריאנטים נשמרים.
- **Governance Council (GC)** – בוחן את דוח ה-TWG, חותם על מעטפת ההצעה ומאשר את לוחות הזמנים לפרסום/דפרקציה.
- **Storage Team** – מתחזק את מימוש הרישום ומפרסם עדכוני תיעוד.

## זרימת מחזור החיים

1. **הגשת הצעה**
   - המחבר מריץ את צ'קליסט הוולידציה ממדריך הכתיבה ויוצר JSON מסוג `ChunkerProfileProposalV1` תחת
     `docs/source/sorafs/proposals/`.
   - יש לכלול פלט CLI מ:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - יש להגיש PR הכולל fixtures, הצעה, דוח דטרמיניזם ועדכוני רישום.

2. **סקירת tooling (TWG)**
   - משחזרים את צ'קליסט הוולידציה (fixtures, fuzz, pipeline של manifest/PoR).
   - מריצים `cargo test -p sorafs_car --chunker-registry` ומוודאים ש-
     `ensure_charter_compliance()` עובר עם הרשומה החדשה.
   - מאמתים שהתנהגות ה-CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) משקפת את ה-aliases וה-handles המעודכנים.
   - מפיקים דוח קצר המסכם ממצאים וסטטוס pass/fail.

3. **אישור מועצה (GC)**
   - בוחנים את דוח ה-TWG ואת מטאדאטה ההצעה.
   - חותמים על digest ההצעה (`blake3("sorafs-chunker-profile-v1" || bytes)`) ומוסיפים חתימות
     למעטפת המועצה הנשמרת לצד fixtures.
   - מתעדים את תוצאת ההצבעה בפרוטוקולי הממשל.

4. **פרסום**
   - ממזגים את ה-PR ומעדכנים:
     - `sorafs_manifest::chunker_registry_data`.
     - תיעוד (`chunker_registry.md`, מדריכי כתיבה/תאימות).
     - Fixtures ודוחות דטרמיניזם.
   - מודיעים למפעילים ולצוותי SDK על הפרופיל החדש ותכנית ה-rollout.

5. **דפרקציה / סיום**
   - הצעות שמחליפות פרופיל קיים חייבות לכלול חלון פרסום כפול (תקופות חסד) ותכנית שדרוג.

6. **שינויים חירומיים**
   - הסרה או hotfix דורשים הצבעת מועצה ברוב קולות.
   - ה-TWG חייב לתעד את צעדי הפחתת הסיכון ולעדכן את יומן התקריות.

## ציפיות tooling

- `sorafs_manifest_chunk_store` ו-`sorafs_manifest_stub` חושפים:
  - `--list-profiles` לבדיקת הרישום.
  - `--promote-profile=<handle>` כדי לייצר את בלוק המטאדאטה הקנוני המשמש לקידום פרופיל.
  - `--json-out=-` כדי להזרים דוחות ל-stdout ולאפשר לוגים של ביקורת שניתנים לשחזור.
- `ensure_charter_compliance()` מופעלת בעת אתחול הבינאריים הרלוונטיים
  (`manifest_chunk_store`, `provider_advert_stub`). בדיקות CI חייבות להיכשל אם
  רשומות חדשות מפרות את האמנה.

## שמירת רשומות

- שמרו את כל דוחות הדטרמיניזם תחת `docs/source/sorafs/reports/`.
- פרוטוקולי המועצה המתייחסים להחלטות chunker נמצאים תחת
  `docs/source/sorafs/migration_ledger.md`.
- עדכנו את `roadmap.md` ו-`status.md` לאחר כל שינוי משמעותי ברישום.

## מקורות

- מדריך כתיבה: [מדריך כתיבת פרופילי chunker](./chunker-profile-authoring.md)
- צ'קליסט תאימות: `docs/source/sorafs/chunker_conformance.md`
- רפרנס רישום: [רישום פרופילי chunker](./chunker-registry.md)
