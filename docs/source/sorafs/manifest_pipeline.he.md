---
lang: he
direction: rtl
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2026-01-30
---

# Chunking של SoraFS → Pipeline של מניפסטים

מסמך זה מתאר את הצעדים המינימליים הדרושים כדי להמיר payload של בתים למניפסט מקודד Norito
המתאים ל‑pinning ברישום SoraFS.

1. **Chunking דטרמיניסטי של ה‑payload**
   - השתמשו ב‑`sorafs_car::CarBuildPlan::single_file` (משתמש פנימית ב‑chunker SF-1)
     כדי להפיק היסטים, אורכים ודיגסטים BLAKE3 של chunks.
   - התוכנית חושפת את דיגסט ה‑payload ואת מטא‑נתוני ה‑chunks שהכלים downstream יכולים
     לעשות בהם שימוש מחדש להרכבת CAR ולתזמון Proof-of-Replication.
   - לחלופין, הפרוטוטייפ `sorafs_car::ChunkStore` קולט בתים ומקליט מטא‑נתוני chunks
     דטרמיניסטיים לבניית CAR בהמשך. ה‑store מפיק כעת עץ דגימה PoR בגודל 64 KiB / 4 KiB
     (מתויג דומיין ומיושר ל‑chunks) כדי שמתזמנים יוכלו לבקש הוכחות Merkle בלי לקרוא
     מחדש את ה‑payload.
    השתמשו ב‑`--por-proof=<chunk>:<segment>:<leaf>` כדי להפיק עד JSON עבור עלה שנדגם וב‑
    `--por-json-out` כדי לכתוב snapshot של דיגסט השורש לאימות מאוחר יותר. שלבו
    `--por-proof` עם `--por-proof-out=path` כדי לשמור את העד, והשתמשו ב‑
    `--por-proof-verify=path` כדי לוודא שהוכחה קיימת תואמת ל‑`por_root_hex` שחושב עבור
    ה‑payload הנוכחי. עבור עלים מרובים, `--por-sample=<count>` (עם `--por-sample-seed` ו‑
    `--por-sample-out` אופציונליים) מפיק דגימות דטרמיניסטיות ומסמן `por_samples_truncated=true`
    כאשר הבקשה חורגת ממספר העלים הזמינים.
   - שמרו את היסטי/אורכי/דיגסטי ה‑chunks אם בכוונתכם לבנות הוכחות bundle (מניפסטים של CAR,
     לוחות זמנים של PoR).
   - עיינו ב‑[`sorafs/chunker_registry.md`](chunker_registry.md) עבור רשומות הרישום הקנוניות
     והכוונת משא ומתן.

2. **עטיפת מניפסט**
   - הזינו את מטא‑נתוני ה‑chunking, CID שורש, התחייבויות CAR, מדיניות pin, claims של alias
     וחתימות ממשל אל `sorafs_manifest::ManifestBuilder`.
   - קראו ל‑`ManifestV1::encode` כדי לקבל את בתים Norito ול‑`ManifestV1::digest` כדי לקבל
     את הדיגסט הקנוני שנרשם ב‑Pin Registry.

3. **פרסום**
   - הגישו את דיגסט המניפסט דרך ממשל (חתימת מועצה, הוכחות alias) ובצעו pin לבתים של
     המניפסט ב‑SoraFS באמצעות ה‑pipeline הדטרמיניסטי.
   - ודאו שקובץ ה‑CAR (והאינדקס האופציונלי שלו) שמופיע במניפסט נשמר באותו סט pins של SoraFS.

### Quickstart של CLI

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

הפקודה מדפיסה דיגסטים של chunks ופרטי מניפסט; כאשר `--manifest-out` ו/או `--car-out`
מסופקים, היא כותבת payload של Norito וארכיון CARv2 תואם־מפרט (pragma + header + בלוקים
של CARv1 + אינדקס Multihash) לדיסק. אם מעבירים נתיב ספרייה, הכלי סורק אותה רקורסיבית
(בסדר לקסיקוגרפי), מבצע chunking לכל קובץ ומפיק עץ dag-cbor עם שורש ספרייה שה‑CID שלו
מופיע כשורש הן במניפסט והן ב‑CAR. דוח ה‑JSON כולל את דיגסט ה‑CAR המחושב, הדיגסט המלא של
הארכיון, הגודל, ה‑CID הגולמי והשורש (עם הפרדת codec dag-cbor), יחד עם רשומות alias/metadata
של המניפסט. השתמשו ב‑`--root-cid`/`--dag-codec` כדי *לאמת* את השורש או ה‑codec המחושב בזמן
ריצות CI, ב‑`--car-digest` כדי לאכוף hash של ה‑payload, ב‑`--car-cid` כדי לאכוף מזהה CAR
גולמי מחושב מראש (CIDv1, codec `raw`, multihash BLAKE3), וב‑`--json-out` כדי לשמור את ה‑JSON
המודפס לצד ארטיפקטי המניפסט/ CAR עבור אוטומציה downstream.

כאשר `--manifest-signatures-out` מסופק (יחד עם לפחות דגל אחד `--council-signature*`), הכלי
גם כותב מעטפת `manifest_signatures.json` הכוללת את דיגסט ה‑BLAKE3 של המניפסט, דיגסט SHA3-256
המצטבר של תוכנית ה‑chunks (היסטים, אורכים ודיגסטים BLAKE3 של chunks) ואת חתימות המועצה.
המעטפת מתעדת כעת את פרופיל ה‑chunker בצורה הקנונית `namespace.name@semver`; מעטפות ישנות
מסוג `namespace-name` ממשיכות לאמת לצורך תאימות. אוטומציה downstream יכולה לפרסם את המעטפת
ביומני ממשל או להפיץ אותה עם ארטיפקטי המניפסט ו‑CAR. כאשר מתקבלת מעטפת מחותם חיצוני, הוסיפו
`--manifest-signatures-in=<path>` כדי שה‑CLI יאשר את הדיגסטים ויוודא שכל חתימת Ed25519 תואמת
לדיגסט המניפסט המחושב מחדש.

כאשר רשומים מספר פרופילי chunker, ניתן לבחור אחד באופן מפורש באמצעות `--chunker-profile-id=<id>`.
הדגל ממופה למזהים מספריים ב‑[`chunker_registry`](chunker_registry.md) ומבטיח שגם מעבר ה‑chunking
וגם המניפסט שהופק מפנים לאותו `(namespace, name, semver)` tuple. העדיפו את צורת ה‑handle הקנונית
באוטומציה (`--chunker-profile=sorafs.sf1@1.0.0`), כדי להימנע מקיבוע IDs מספריים. הריצו
`sorafs_manifest_chunk_store --list-profiles` כדי לראות את רשומות הרישום הנוכחיות (הפלט משקף את
הרשימה שמסופקת על ידי `sorafs_manifest_chunk_store`), או השתמשו ב‑`--promote-profile=<handle>`
כדי לייצא את ה‑handle הקנוני ומטא‑נתוני alias בעת הכנת עדכון רישום.

מבקרים יכולים לבקש את עץ Proof-of-Retrievability המלא באמצעות `--por-json-out=path`, שמסריאל
דיגסטים של chunk/segment/leaf לצורך אימות דגימה. עדים בודדים ניתן לייצא עם
`--por-proof=<chunk>:<segment>:<leaf>` (ולאמת עם `--por-proof-verify=path`), בעוד
`--por-sample=<count>` מפיק דגימות דטרמיניסטיות וללא כפילויות עבור בדיקות נקודתיות.

כל דגל שכותב JSON (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`, וכו') מקבל גם `-`
כנתיב, מה שמאפשר להזרים את ה‑payload ישירות ל‑stdout ללא יצירת קבצים זמניים.

השתמשו ב‑`--chunk-fetch-plan-out=path` כדי לשמור את מפרט fetch המסודר של chunks (אינדקס chunk,
היסט ה‑payload, אורך, דיגסט BLAKE3) שמלווה את תוכנית המניפסט. לקוחות multi-source יכולים להזין
את ה‑JSON שנוצר ישירות לאורקסטרטור fetch של SoraFS בלי לקרוא מחדש את ה‑payload המקורי. דוח ה‑JSON
שמודפס על ידי ה‑CLI כולל גם את המערך תחת `chunk_fetch_specs`. גם מקטע `chunking` וגם אובייקט
`manifest` חושפים `profile_aliases` לצד ה‑handle הקנוני `profile` כדי ש‑SDKs יוכלו להגר מהצורה
המורשת `namespace-name` בלי לאבד תאימות.

בעת הרצה מחדש של ה‑stub (למשל ב‑CI או בצינור release) אפשר להעביר `--plan=chunk_fetch_specs.json`
או `--plan=-` כדי לייבא את המפרט שנוצר קודם. ה‑CLI מאמת שכל אינדקס, היסט, אורך ודיגסט BLAKE3 של
כל chunk עדיין תואמים לתוכנית CAR שגובשה מחדש לפני המשך ה‑ingestion, מה שמגן מפני תוכניות ישנות
או משובשות.

### Smoke-test של אורקסטרציה מקומית

ה‑crate `sorafs_car` מספק כעת את `sorafs-fetch`, CLI למפתחים שצורכת את מערך
`chunk_fetch_specs` ומדמה שליפה multi-provider מקבצים מקומיים. הצביעו על ה‑JSON שמופק
על ידי `--chunk-fetch-plan-out`, ספקו נתיב אחד או יותר ל‑payloads של ספקים (ואפשר להוסיף `#N`
להגדלת המקביליות), והוא יאמת chunks, ירכיב מחדש את ה‑payload, וידפיס דוח JSON המסכם ספירות
הצלחה/כישלון לפי ספק וקבלות לכל chunk:

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

השתמשו בזרימה זו כדי לאמת את התנהגות האורקסטרטור או להשוות payloads של ספקים לפני חיבור
תעבורות רשת אמיתיות לצומת SoraFS.

כאשר יש צורך לפנות ל‑Torii gateway חי במקום קבצים מקומיים, החליפו את הדגלים `--provider=/path`
באפשרויות החדשות המכוונות ל‑HTTP:

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

ה‑CLI מאמת את ה‑stream token, אוכף יישור של chunker/profile, ורושם את מטא‑נתוני ה‑gateway לצד
הקבלות הרגילות של ספקים כדי שמפעילים יוכלו לארכב את הדוח כראיית rollout (ראו handbook הפריסה
לתהליך blue/green המלא).

אם תעבירו `--provider-advert=name=/path/to/advert.to`, ה‑CLI כעת מפענח את מעטפת Norito,
מאמת חתימת Ed25519 ודורש שהספק יפרסם את יכולת `chunk_range_fetch`. הדבר שומר על סימולציית
fetch רב‑מקורות מיושרת עם מדיניות הקבלה של הממשל ומונע שימוש מקרי בספקים מורשתיים שאינם
יכולים לעמוד בבקשות chunk בטווחים.

הסיומת `#N` מגדילה את מגבלת המקביליות של הספק, בעוד `@W` קובע את משקל התזמון (ברירת מחדל 1
כשמושמט). כאשר adverts או descriptors של gateway מסופקים, ה‑CLI כעת מעריך את scoreboard של
האורקסטרטור לפני התחלת fetch: ספקים כשירים יורשים משקלים מודעים לטלמטריה וה‑JSON snapshot נשמר
ב‑`--scoreboard-out=<path>` כאשר מסופק. ספקים שנכשלו בבדיקות יכולת או בדד־ליינים של הממשל נופלים
אוטומטית עם אזהרה כדי שהריצות יישארו מיושרות למדיניות הקבלה. ראו
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` עבור זוג קלט/פלט לדוגמה.

העבירו `--expect-payload-digest=<hex>` ו/או `--expect-payload-len=<bytes>` כדי לאשר שה‑payload
המורכב תואם את ציפיות המניפסט לפני כתיבת הפלט — שימושי ל‑CI smoke-tests שרוצים לוודא שהאורקסטרטור
לא השמיט או סידר מחדש chunks בשקט.

אם כבר יש לכם דוח JSON שנוצר על ידי `sorafs-manifest-stub`, העבירו אותו ישירות באמצעות
`--manifest-report=docs.report.json`. ה‑CLI של fetch ישתמש מחדש בשדות המוטמעים
`chunk_fetch_specs`, `payload_digest_hex`, ו‑`payload_len`, כך שאין צורך לנהל קובצי תוכנית או
אימות נפרדים.

דוח ה‑fetch חושף גם טלמטריה מצטברת לעזרה במעקב: `chunk_retry_total`, `chunk_retry_rate`,
`chunk_attempt_total`, `chunk_attempt_average`, `provider_success_total`, `provider_failure_total`,
`provider_failure_rate` ו‑`provider_disabled_total` לוכדים את הבריאות הכללית של סשן fetch ומתאימים
לדשבורדים של Grafana/Loki או לאסרטים ב‑CI. השתמשו ב‑`--provider-metrics-out` כדי לכתוב רק את מערך
`provider_reports` אם הכלי downstream צריך רק סטטיסטיקות ברמת ספק.

### צעדים הבאים

- תעדו מטא‑נתוני CAR לצד דיגסטי מניפסט ביומני הממשל כדי שהצופים יוכלו לאמת את תכולת CAR בלי
  להוריד את ה‑payload מחדש.
- שלבו את זרימת פרסום המניפסט וה‑CAR ב‑CI כדי שכל build של docs/artefacts יפיק באופן אוטומטי
  מניפסט, יקבל חתימות ויבצע pin ל‑payloads המתקבלים.
