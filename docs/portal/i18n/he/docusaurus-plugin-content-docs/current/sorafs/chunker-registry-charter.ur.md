---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry-charter.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry-charter
כותרת: SoraFS chunker registry charter
sidebar_label: אמנת הרישום של Chunker
description: Chunker profile submissions اور approvals کے لیے governance charter۔
---

:::note مستند ماخذ
:::

# אמנת ניהול הרישום של SoraFS chunker

> **אושרר:** 2025-10-29 על ידי פאנל תשתיות פרלמנט סורה (ראה
> `docs/source/sorafs/council_minutes_2025-10-29.md`). کسی بھی ترمیم کے لیے باضابطہ governance ووٹ درکار ہے؛
> יישום אמנת תקינה נורמטיבית תקינה

یہ charter SoraFS chunker registry کو evolve کرنے کے لیے process اور roles define کرتی ہے۔
20 [מדריך ליצירת פרופילים של צ'ונקר](./chunker-profile-authoring.md)
profiles کیسے propose، review، ratify اور بالآخر deprecate ہوتے ہیں۔

## היקף

یہ charter `sorafs_manifest::chunker_registry` کی ہر entry پر لاگو ہے اور
כלי עזר או רישום רישום (CLI מניפסט, ספק-מודעה CLI,
ערכות SDK). یہ alias اور handle invariants enforce کرتی ہے جنہیں
`chunker_registry::ensure_charter_compliance()` תכנים:

- Profile IDs مثبت integers ہوتے ہیں جو monotonic طور پر بڑھتے ہیں۔
- Canonical handle `namespace.name@semver` **لازم** ہے کہ `profile_aliases` میں پہلی
- Alias ​​strings trim کی جاتی ہیں، unique ہوتی ہیں، اور دوسری entries کے canonical handles سے collide نہیں کرتیں۔

## תפקידים

- **Author(s)** – proposal تیار کرتے ہیں، fixtures regenerate کرتے ہیں، اور determinism evidence جمع کرتے ہیں۔
- **Tooling Working Group (TWG)** – published checklists کے ذریعے proposal validate کرتا ہے اور registry invariants برقرار رکھتا ہے۔
- **Governance Council (GC)** – TWG report کا review کرتا ہے، proposal envelope پر sign کرتا ہے، اور publication/deprecation timelines approve کرتا ہے۔
- **Storage Team** – registry implementation maintain کرتا ہے اور documentation updates publish کرتا ہے۔

## זרימת עבודה במחזור החיים

1. **הגשת הצעה**
   - Author authoring guide کی validation checklist چلاتا ہے اور
     `docs/source/sorafs/proposals/` کے تحت `ChunkerProfileProposalV1` JSON بناتا ہے۔
   - תקלות פלט CLI:
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Fixtures، proposal، determinism report، اور registry updates پر مشتمل PR submit کریں۔

2. **סקירת כלי עבודה (TWG)**
   - Validation checklist دوبارہ چلائیں (fixtures, fuzz, manifest/PoR pipeline).
   - `cargo test -p sorafs_car --chunker-registry` גרסה חדשה
     `ensure_charter_compliance()` نئی entry کے ساتھ پاس ہو۔
   - התנהגות CLI (`--list-profiles`, `--promote-profile`, סטרימינג
     `--json-out=-`) کی توثیق کریں کہ یہ updated aliases اور handles دکھاتا ہے۔
   - Findings اور pass/fail status پر مشتمل مختصر رپورٹ تیار کریں۔

3. **אישור המועצה (GC)**
   - TWG report اور proposal metadata کا review کریں۔
   - Proposal digest (`blake3("sorafs-chunker-profile-v1" || bytes)`) پر sign کریں اور
     signatures کو council envelope میں شامل کریں جو fixtures کے ساتھ رکھا جاتا ہے۔
   - Vote outcome کو governance minutes میں ریکارڈ کریں۔4. **פרסום**
   - מיזוג יחסי ציבור:
     - `sorafs_manifest::chunker_registry_data`.
     - תיעוד (`chunker_registry.md`, מדריכי כתיבה/התאמה).
     - מתקנים או דוחות דטרמיניזם.
   - Operators اور SDK teams کو نئے profile اور planned rollout کے بارے میں اطلاع دیں۔

5. **הוצאה משימוש / שקיעה**
   - جو proposals موجودہ profile کو replace کرتی ہیں ان میں dual-publish window
     (grace periods) اور upgrade plan شامل ہونا چاہیے۔
     اور migration ledger کو update کریں۔

6. **שינויי חירום**
   - Removal یا hotfixes کے لیے council vote اور اکثریتی approval درکار ہے۔
   - TWG کو risk mitigation steps document کرنے اور incident log update کرنے ہوں گے۔

## ציפיות כלי עבודה

- `sorafs_manifest_chunk_store` או `sorafs_manifest_stub` חשיפת תמונות:
  - Registry inspection کے لیے `--list-profiles`.
  - Profile promote کرتے وقت canonical metadata block بنانے کے لیے `--promote-profile=<handle>`.
  - Reports کو stdout پر stream کرنے کے لیے `--json-out=-`، تاکہ reproducible review logs ممکن ہوں۔
- `ensure_charter_compliance()` relevant binaries کے startup پر چلایا جاتا ہے
  (`manifest_chunk_store`, `provider_advert_stub`). CI tests کو fail ہونا چاہیے اگر
  نئی entries charter کی خلاف ورزی کریں۔

## שמירת תיעוד

- تمام determinism reports کو `docs/source/sorafs/reports/` میں محفوظ کریں۔
- Chunker فیصلوں کا حوالہ دینے والی council minutes
  `docs/source/sorafs/migration_ledger.md` میں موجود ہیں۔
- ہر بڑے registry change کے بعد `roadmap.md` اور `status.md` اپڈیٹ کریں۔

## הפניות

- מדריך כתיבה: [מדריך ליצירת פרופילים של Chunker](./chunker-profile-authoring.md)
- רשימת תאימות: `docs/source/sorafs/chunker_conformance.md`
- הפניה לרישום: [Chunker Profile Registry](./chunker-registry.md)