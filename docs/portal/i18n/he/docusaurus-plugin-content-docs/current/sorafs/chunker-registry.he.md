---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0018cfedc54c119aff35a9acde558ba96dde6142641e9cc48e6d00c8f896c227
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: chunker-registry
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/chunker_registry.md`. שמרו על שתי העתקות מסונכרנות עד שמערכת התיעוד הישנה של Sphinx תופסק לחלוטין.
:::

## רישום פרופילי chunker של SoraFS (SF-2a)

מחסנית SoraFS מנהלת משא ומתן על התנהגות chunking באמצעות רישום קטן עם namespace.
כל פרופיל מקצה פרמטרי CDC דטרמיניסטיים, מטאדאטה semver וה-digest/multicodec הצפוי המשמש ב-manifests ובארכיוני CAR.

מחברי פרופילים צריכים לעיין ב-
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
כדי לקבל את המטאדאטה הנדרש, צ'קליסט הוולידציה ותבנית ההצעה לפני שליחת רשומות חדשות.
לאחר שהממשל מאשר שינוי, עקבו אחר
[צ'קליסט rollout לרישום](./chunker-registry-rollout-checklist.md) ו-
[playbook manifest ב-staging](./staging-manifest-playbook) כדי לקדם את
ה-fixtures ל-staging ולפרודקשן.

### פרופילים

| Namespace | שם | SemVer | מזהה פרופיל | מינימום (bytes) | יעד (bytes) | מקסימום (bytes) | מסכת שבירה | Multihash | Aliases | הערות |
|-----------|----|--------|-------------|-----------------|------------|-----------------|------------|-----------|--------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | פרופיל קנוני המשמש ב-fixtures של SF-1 |

הרישום חי בקוד בתור `sorafs_manifest::chunker_registry` (מוסדר על ידי [`chunker_registry_charter.md`](./chunker-registry-charter.md)). כל רשומה
מתוארת כ-`ChunkerProfileDescriptor` עם:

* `namespace` – קיבוץ לוגי של פרופילים קשורים (למשל `sorafs`).
* `name` – תווית קריאה לבני אדם (`sf1`, `sf1-fast`, …).
* `semver` – מחרוזת גרסה סמנטית עבור סט הפרמטרים.
* `profile` – ה-`ChunkProfile` בפועל (min/target/max/mask).
* `multihash_code` – ה-multihash המשמש ליצירת digests של chunk (`0x1f`
  עבור ברירת המחדל של SoraFS).

ה-manifest מסריאל פרופילים באמצעות `ChunkingProfileV1`. המבנה רושם את מטאדאטת הרישום
(namespace, name, semver) לצד פרמטרי ה-CDC הגולמיים ורשימת ה-aliases למעלה. צרכנים
צריכים קודם לנסות lookup ברישום לפי `profile_id` ולחזור לפרמטרים inline כאשר מופיעים
IDs לא ידועים; רשימת ה-aliases מבטיחה שלקוחות HTTP יוכלו להמשיך לשלוח handles ישנים
ב-`Accept-Chunker` בלי לנחש. כללי ה-charter של הרישום מחייבים שה-handle הקנוני
(`namespace.name@semver`) יהיה הערך הראשון ב-`profile_aliases`, ולאחריו כל alias ישן.

כדי לבדוק את הרישום מכלי tooling, הריצו את CLI helper:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

כל flags ה-CLI שכותבים JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) מקבלים את `-` כנתיב, מה שמזרים את המטען ל-stdout במקום ליצור קובץ.
כך קל לבצע piping של הנתונים לכלי tooling תוך שמירה על התנהגות ברירת המחדל של הדפסת הדו"ח הראשי.

### מטריצת תאימות ותכנית rollout


הטבלה למטה מתארת את סטטוס התמיכה הנוכחי עבור `sorafs.sf1@1.0.0` ברכיבים המרכזיים.
"Bridge" מתייחס למסלול תאימות CARv1 + SHA-256 שדורש משא ומתן מפורש של הלקוח
(`Accept-Chunker` + `Accept-Digest`).

| רכיב | סטטוס | הערות |
|------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ נתמך | מאמת handle קנוני + alias, מזרים דו"חות דרך `--json-out=-`, ומכיל אכיפת charter באמצעות `ensure_charter_compliance()`. |
| `sorafs_fetch` (developer orchestrator) | ✅ נתמך | קורא `chunk_fetch_specs`, מבין payloads של יכולת `range`, ומרכיב פלט CARv2. |
| SDK fixtures (Rust/Go/TS) | ✅ נתמך | נוצרו מחדש דרך `export_vectors`; ה-handle הקנוני מופיע ראשון בכל רשימת alias וחתום על ידי envelopes של המועצה. |
| משא ומתן פרופילים ב-gateway Torii | ✅ נתמך | מיישם את הדקדוק המלא של `Accept-Chunker`, כולל headers `Content-Chunker`, וחושף Bridge CARv1 רק בבקשות downgrade מפורשות. |

Rollout טלמטריה:

- **טלמטריית fetch של chunks** — CLI של Iroha `sorafs toolkit pack` מפיק digests של chunk, מטאדאטת CAR ושורשי PoR לצריכת dashboards.
- **Provider adverts** — payloads של adverts כוללים מטאדאטת יכולות ו-aliases; אמתו כיסוי דרך `/v2/sorafs/providers` (למשל קיום יכולת `range`).
- **ניטור gateway** — מפעילים צריכים לדווח על צמדי `Content-Chunker`/`Content-Digest` כדי לזהות downgrades בלתי צפויים; צפוי ששימוש ב-bridge ירד לאפס לפני הדפרקציה.

מדיניות דפרקציה: לאחר שאושר פרופיל יורש, תזמנו חלון פרסום כפול (מתועד בהצעה) לפני סימון

כדי לבדוק עד witness PoR ספציפי, ספקו אינדקסים של chunk/segment/leaf ובמידת הצורך שמרו את ההוכחה לדיסק:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

אפשר לבחור פרופיל לפי id מספרי (`--profile-id=1`) או לפי handle ברישום
(`--profile=sorafs.sf1@1.0.0`); פורמט ה-handle נוח לסקריפטים שמעבירים
namespace/name/semver ישירות ממטאדאטת ממשל.

השתמשו ב-`--promote-profile=<handle>` כדי להפיק בלוק JSON של מטאדאטה (כולל כל ה-aliases
הרשומים) שניתן להדביק ל-`chunker_registry_data.rs` כאשר מקדמים פרופיל ברירת מחדל חדש:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

הדו"ח הראשי (וקובץ ההוכחה האופציונלי) כולל את ה-digest השורשי, הבתים של leaf שנדגם
(מקודד ב-hex), ואת sibling digests של segment/chunk כך שמאמתים יוכלו לבצע rehash
לשכבות 64 KiB/4 KiB מול הערך `por_root_hex`.

כדי לאמת הוכחה קיימת מול payload, העבירו את הנתיב דרך
`--por-proof-verify` (ה-CLI מוסיף `"por_proof_verified": true` כשה-witness תואם לשורש המחושב):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

לדגימה אצוותית, השתמשו ב-`--por-sample=<count>` ואפשר לספק seed/נתיב פלט אופציונלי.
ה-CLI מבטיח סדר דטרמיניסטי (`splitmix64` seeded) ויחתוך אוטומטית כאשר הבקשה
חוצה את מספר העלים הזמין:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

manifest stub משקף את אותם נתונים, מה שנוח לסקריפטים של בחירת `--chunker-profile-id`
ב-pipelines. שני ה-CLIs של chunk store מקבלים גם את פורמט ה-handle הקנוני
(`--profile=sorafs.sf1@1.0.0`) כדי שסקריפטי build יימנעו מהטמעת IDs מספריים קשיחים:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

שדה `handle` (`namespace.name@semver`) תואם למה שה-CLIs מקבלים דרך
`--profile=…`, כך שאפשר להעתיק אותו ישירות לאוטומציה.

### תיאום chunkers

Gateways ולקוחות מכריזים על פרופילים נתמכים דרך provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (משתמע דרך הרישום)
    capabilities: [...]
}
```

תזמון chunks רב-מקוריים מוכרז באמצעות יכולת `range`. ה-CLI מקבל אותה עם
`--capability=range[:streams]`, כאשר הסיומת המספרית האופציונלית מקודדת את
הקונקרנציה המועדפת של range-fetch לספק (לדוגמה, `--capability=range:64` מודיע על
תקציב 64 streams). כאשר היא חסרה, צרכנים חוזרים להינט הכללי `max_streams`
שמפורסם במקום אחר ב-advert.

בעת בקשת נתוני CAR, לקוחות צריכים לשלוח header `Accept-Chunker` שמונה tuples
`(namespace, name, semver)` לפי סדר עדיפות:

```

Gateways בוחרים פרופיל נתמך הדדית (ברירת מחדל `sorafs.sf1@1.0.0`) ומשקפים את ההחלטה
דרך header התגובה `Content-Chunker`. Manifests מטמיעים את הפרופיל הנבחר כך שנודים
downstream יכולים לאמת את פריסת ה-chunks בלי להסתמך על מו"מ HTTP.

### תאימות CAR

אנחנו שומרים נתיב ייצוא CARv1+SHA-2:

* **נתיב ראשי** – CARv2, BLAKE3 payload digest (`0x1f` multihash),
  `MultihashIndexSorted`, פרופיל chunk כפי שתועד לעיל.
  רשאים לחשוף וריאנט זה כאשר הלקוח משמיט `Accept-Chunker` או מבקש
  `Accept-Digest: sha2-256`.

נוספים לתאימות אך אינם אמורים להחליף את ה-digest הקנוני.

### תאימות

* הפרופיל `sorafs.sf1@1.0.0` ממופה ל-fixtures הציבוריים תחת
  `fixtures/sorafs_chunker` ולקורפוסים הרשומים תחת
  `fuzz/sorafs_chunker`. פריטי parity מקצה-לקצה נבדקים ב-Rust, Go ו-Node באמצעות הבדיקות שסופקו.
* `chunker_registry::lookup_by_profile` מאשר שפרמטרי ה-descriptor תואמים ל-`ChunkProfile::DEFAULT` כדי למנוע סטיות מקריות.
* Manifests שמיוצרים על ידי `iroha app sorafs toolkit pack` ו-`sorafs_manifest_stub` כוללים את מטאדאטת הרישום.
