---
id: chunker-profile-authoring
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/chunker_profile_authoring.md`. שמרו על שתי העתקות מסונכרנות עד שמערכת התיעוד הישנה של Sphinx תופסק לחלוטין.
:::

# מדריך כתיבת פרופילי chunker של SoraFS

מדריך זה מסביר כיצד להציע ולפרסם פרופילי chunker חדשים עבור SoraFS.
הוא משלים את RFC הארכיטקטורה (SF-1) ואת רפרנס הרישום (SF-2a)
עם דרישות כתיבה קונקרטיות, שלבי ולידציה ותבניות הצעה.
לדוגמה קנונית ראו
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
ואת לוג ה-dry-run המצורף ב-
`docs/source/sorafs/reports/sf1_determinism.md`.

## סקירה

כל פרופיל שנכנס לרישום חייב:

- לפרסם פרמטרי CDC דטרמיניסטיים והגדרות multihash זהות בין ארכיטקטורות;
- לספק fixtures ניתנים לשחזור (JSON Rust/Go/TS + corpora fuzz + עדי PoR) ש-SDKs downstream יכולים
  לאמת בלי tooling ייעודי;
- לכלול מטאדאטה מוכנה לממשל (namespace, name, semver) לצד הנחיות הגירה וחלונות תאימות; ו-
- לעבור את סט ה-diff הדטרמיניסטי לפני ביקורת המועצה.

עקבו אחר הצ'קליסט להלן כדי להכין הצעה שעומדת בכללים האלה.

## תקציר אמנת הרישום

לפני ניסוח הצעה, ודאו שהיא עומדת באמנת הרישום המיושמת על ידי
`sorafs_manifest::chunker_registry::ensure_charter_compliance()`:

- מזהי פרופיל הם מספרים שלמים חיוביים שעולים בצורה מונוטונית ללא חורים.
- ה-handle הקנוני (`namespace.name@semver`) חייב להופיע ברשימת ה-aliases
- אף alias לא יכול להתנגש עם handle קנוני אחר או להופיע יותר מפעם אחת.
- aliases חייבים להיות לא ריקים ומקוצצים מרווחים.

עוזרי CLI שימושיים:

```bash
# רשימת JSON של כל ה-descriptors הרשומים (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# להוציא מטאדאטה לפרופיל ברירת מחדל מועמד (handle קנוני + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

פקודות אלה שומרות על ההצעות מיושרות עם אמנת הרישום ומספקות מטאדאטה קנונית
הנדרשת בדיוני ממשל.

## מטאדאטה נדרש

| שדה | תיאור | דוגמה (`sorafs.sf1@1.0.0`) |
|-----|--------|----------------------------|
| `namespace` | קיבוץ לוגי של פרופילים קשורים. | `sorafs` |
| `name` | תווית קריאה לבני אדם. | `sf1` |
| `semver` | מחרוזת גרסה סמנטית לסט הפרמטרים. | `1.0.0` |
| `profile_id` | מזהה מספרי מונוטוני שמוקצה כאשר הפרופיל נכנס. שמרו את ה-id הבא אך אל תשתמשו מחדש במספרים קיימים. | `1` |
| `profile.min_size` | אורך chunk מינימלי ב-bytes. | `65536` |
| `profile.target_size` | אורך chunk יעד ב-bytes. | `262144` |
| `profile.max_size` | אורך chunk מקסימלי ב-bytes. | `524288` |
| `profile.break_mask` | מסכה אדפטיבית לשימוש ב-rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | קבוע פולינום gear (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed לגזירת טבלת gear בגודל 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | קוד multihash ל-digests לכל chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest של bundle fixtures קנוני. | `13fa...c482` |
| `fixtures_root` | ספרייה יחסית המכילה fixtures מחודשים. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed לדגימת PoR דטרמיניסטית (`splitmix64`). | `0xfeedbeefcafebabe` (דוגמה) |

המטאדאטה חייבת להופיע גם במסמך ההצעה וגם בתוך fixtures שנוצרו כדי שהרישום, tooling ה-CLI ואוטומציית
הממשל יוכלו לאמת ערכים בלי הצלבות ידניות. בעת ספק, הריצו את CLIs של chunk-store ו-manifest עם
`--json-out=-` כדי להזרים מטאדאטה מחושבת להערות הביקורת.

### נקודות מגע CLI ורישום

- `sorafs_manifest_chunk_store --profile=<handle>` — להריץ שוב מטאדאטה של chunk,
  digest של manifest ובדיקות PoR עם הפרמטרים המוצעים.
- `sorafs_manifest_chunk_store --json-out=-` — להזרים את דוח ה-chunk-store ל-stdout לצורכי
  השוואות אוטומטיות.
- `sorafs_manifest_stub --chunker-profile=<handle>` — לאשר ש-manifests ותכניות CAR
  מטמיעים את ה-handle הקנוני ואת ה-aliases.
- `sorafs_manifest_stub --plan=-` — להזין מחדש את `chunk_fetch_specs` הקודם כדי לבדוק
  offsets/digests אחרי שינוי.

רשמו את פלט הפקודות (digests, שורשי PoR, hashes של manifest) בהצעה כדי שמבקרים יוכלו לשחזרם מילה במילה.

## צ'קליסט דטרמיניזם וולידציה

1. **רגנרציית fixtures**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **הרצת suite של פריטי parity** — `cargo test -p sorafs_chunker` וה-harness diff
   cross-language (`crates/sorafs_chunker/tests/vectors.rs`) חייבים להיות ירוקים עם fixtures חדשים.
3. **הפעלה מחדש של corpora fuzz/back-pressure** — הריצו `cargo fuzz list` ואת harness ה-streaming
   (`fuzz/sorafs_chunker`) מול assets מחודשים.
4. **אימות עדי Proof-of-Retrievability** — הריצו
   `sorafs_manifest_chunk_store --por-sample=<n>` עם הפרופיל המוצע ואשרו שהשורשים תואמים ל-manifest של fixtures.
5. **Dry run ל-CI** — הריצו `ci/check_sorafs_fixtures.sh` מקומית; הסקריפט
   צריך להצליח עם fixtures חדשים ו-`manifest_signatures.json` הקיים.
6. **אישור cross-runtime** — ודאו ש-bindings של Go/TS צורכים את ה-JSON המחודש ומפיקים
   גבולות chunk ו-digests זהים.

תעדו את הפקודות וה-digests שהתקבלו בהצעה כדי ש-Tooling WG יוכל להריץ מחדש ללא ניחושים.

### אישור Manifest / PoR

לאחר רגנרציה של fixtures, הריצו את צינור ה-manifest המלא כדי לוודא שמטאדאטה של CAR והוכחות PoR
נשארות עקביות:

```bash
# לאמת מטאדאטה של chunk + PoR עם הפרופיל החדש
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# ליצור manifest + CAR וללכוד chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# להריץ שוב עם fetch plan שמור (מגן מפני offsets מיושנים)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

החליפו את קובץ הקלט בכל corpus מייצג שבו משתמשים ב-fixtures שלכם
(למשל זרם דטרמיניסטי של 1 GiB) וצרפו את ה-digests שהתקבלו להצעה.

## תבנית הצעה

הצעות מוגשות כ-records Norito מסוג `ChunkerProfileProposalV1` הנבדקות אל תוך
`docs/source/sorafs/proposals/`. תבנית ה-JSON למטה מציגה את הצורה הצפויה
(החליפו לערכים שלכם לפי הצורך):


ספקו דוח Markdown תואם (`determinism_report`) המתעד את פלט הפקודות, digests של chunk וכל
חריגה שנתקלה במהלך הוולידציה.

## זרימת ממשל

1. **שליחת PR עם הצעה + fixtures.** כללו את assets שנוצרו, את הצעת Norito ואת
   העדכונים ל-`chunker_registry_data.rs`.
2. **סקירת Tooling WG.** מבקרים מריצים מחדש את צ'קליסט הוולידציה ומאשרים שההצעה
   תואמת לכללי הרישום (ללא שימוש חוזר ב-id, דטרמיניזם מתקיים).
3. **מעטפת המועצה.** לאחר אישור, חברי המועצה חותמים על digest ההצעה
   (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) ומוסיפים את החתימות למעטפת
   הפרופיל הנשמרת לצד fixtures.
4. **פרסום הרישום.** ה-merge מעדכן את הרישום, docs ו-fixtures. ברירת המחדל של ה-CLI נשארת
   על הפרופיל הקודם עד שהממשל מצהיר שההגירה מוכנה.
   והודיעו למפעילים דרך migration ledger.

## טיפים לכתיבה

- העדיפו גבולות חזקות של שתיים זוגיות כדי למזער התנהגות קצה ב-chunking.
- הימנעו משינוי קוד multihash בלי לתאם עם צרכני manifest ו-gateway; כללו הערת תאימות כשאתם עושים זאת.
- השאירו seeds של טבלת gear קריאות לאדם אך ייחודיות גלובלית כדי להקל על ביקורת.
- אחסנו תוצרי benchmarking (למשל השוואות throughput) תחת
  `docs/source/sorafs/reports/` לעיון עתידי.

לציפיות תפעוליות בזמן rollout ראו את migration ledger
(`docs/source/sorafs/migration_ledger.md`). לכללי תאימות runtime ראו
`docs/source/sorafs/chunker_conformance.md`.
