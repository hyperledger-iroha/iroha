---
lang: he
direction: rtl
source: docs/source/mochi/troubleshooting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee2fc3d436de10f6a05a0b73cf3f5a0e07bc1ef1c421203cfcbd09d8d0e039b
source_last_modified: "2025-11-20T04:33:21.759984+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/mochi/troubleshooting.md -->

# מדריך פתרון תקלות ל‑MOCHI

השתמשו בנוהל זה כאשר אשכולות MOCHI מקומיים מסרבים לעלות, נתקעים בלולאת
אתחולים, או מפסיקים להזרים עדכוני בלוקים/אירועים/סטטוס. הוא מרחיב את סעיף
המפה "Documentation & rollout" בכך שהוא מתרגם את התנהגויות ה‑supervisor ב‑
`mochi-core` לצעדי התאוששות מעשיים.

## 1. רשימת בדיקות למגיב ראשון

1. לכדו את שורש הנתונים שבו MOCHI משתמש. ברירת המחדל היא
   `$TMPDIR/mochi/<profile-slug>`; נתיבים מותאמים מופיעים בכותרת ה‑UI וגם דרך
   `cargo run -p mochi-ui-egui -- --data-root ...`.
2. הריצו `./ci/check_mochi.sh` משורש ה‑workspace. הדבר מאמת את הקרייטים core,
   UI ו‑integration לפני שמתחילים לשנות תצורות.
3. רשמו את ה‑preset (`single-peer` או `four-peer-bft`). הטופולוגיה שנוצרה קובעת
   כמה תיקיות/לוגים של peers אתם אמורים לראות תחת שורש הנתונים.

## 2. איסוף לוגים וראיות טלמטריה

`NetworkPaths::ensure` (ראו `mochi/mochi-core/src/config.rs`) יוצר פריסה יציבה:

```
<data_root>/<profile>/
  peers/<alias>/...
  logs/<alias>.log
  genesis/
  snapshots/
```

פעלו כך לפני שינוי כלשהו:

- השתמשו בלשונית **Logs** או פתחו ישירות `logs/<alias>.log` כדי ללכוד את 200
  השורות האחרונות לכל peer. ה‑supervisor מזרים stdout/stderr/system דרך
  `PeerLogStream`, ולכן קבצים אלו תואמים לפלט ה‑UI.
- ייצאו snapshot דרך **Maintenance → Export snapshot** (או קריאה ל‑
  `Supervisor::export_snapshot`). ה‑snapshot מאגד אחסון, תצורות ולוגים תחת
  `snapshots/<timestamp>-<label>/`.
- אם הבעיה קשורה ל‑stream widgets, העתיקו את מחווני הבריאות של
  `ManagedBlockStream`, `ManagedEventStream`, ו‑`ManagedStatusStream` מה‑Dashboard.
  ה‑UI מציג את ניסיון החיבור האחרון ואת סיבת השגיאה; צלמו מסך לרישום האירוע.

## 3. פתרון בעיות בהפעלת peers

רוב כשלי ההפעלה מתחלקים לשלושה סוגים:

### בינאריים חסרים או דריסות שגויות

`SupervisorBuilder` מריץ דרך shell את `irohad`, `kagami`, ו‑(בעתיד) `iroha_cli`.
אם ה‑UI מדווח "failed to spawn process" או "permission denied", הפנו את MOCHI
לבינאריים ידועים כתקינים:

```bash
cargo run -p mochi-ui-egui -- \
  --irohad /path/to/irohad \
  --kagami /path/to/kagami \
  --iroha-cli /path/to/iroha_cli
```

ניתן להגדיר `MOCHI_IROHAD`, `MOCHI_KAGAMI`, ו‑`MOCHI_IROHA_CLI` כדי להימנע מהקלדה
חוזרת. בעת ניפוי חבילות, השוו את `BundleConfig` ב‑`mochi/mochi-ui-egui/src/config/`
אל מול הנתיבים ב‑`target/mochi-bundle`.

### התנגשויות פורטים

`PortAllocator` בודק את ממשק loopback לפני כתיבת תצורות. אם אתם רואים
`failed to allocate Torii port` או `failed to allocate P2P port`, תהליך אחר כבר
מאזין בטווח ברירת המחדל (8080/1337). הפעילו את MOCHI מחדש עם בסיסים מפורשים:

```bash
cargo run -p mochi-ui-egui -- --torii-start 12000 --p2p-start 19000
```

ה‑builder מפזר פורטים עוקבים מהבסיסים, אז שמרו טווח בגודל ה‑preset שלכם
(`peer_count` peers → `peer_count` פורטים לכל תעבורה).

### השחתת genesis ואחסון

אם Kagami יוצא לפני פליטת מניפסט, peers יקרסו מיד. בדקו את
`genesis/*.json`/`.toml` בתוך שורש הנתונים. הריצו שוב עם
`--kagami /path/to/kagami` או הפנו את דיאלוג **Settings** לבינארי הנכון.
במקרה של השחתת אחסון, השתמשו בכפתור **Wipe & re-genesis** ב‑Maintenance
(מוסבר בהמשך) במקום למחוק תיקיות ידנית; הוא יוצר מחדש את תיקיות ה‑peer
ושורשי ה‑snapshot לפני אתחול התהליכים.

### כיוונון אתחולים אוטומטיים

`[supervisor.restart]` ב‑`config/local.toml` (או דגלי CLI
`--restart-mode`, `--restart-max`, `--restart-backoff-ms`) שולט בתדירות ניסיונות
האתחול. הגדירו `mode = "never"` כאשר אתם רוצים שה‑UI יציג את הכשל הראשון מיד,
או קצרו `max_restarts`/`backoff_ms` כדי להדק את חלון הניסיונות עבור משימות CI
שצריכות להיכשל מהר.

## 4. איפוס peers בבטחה

1. עצרו את ה‑peers המושפעים מה‑Dashboard או צאו מה‑UI. ה‑supervisor מסרב
   למחוק אחסון בזמן ש‑peer רץ (`PeerHandle::wipe_storage` מחזיר `PeerStillRunning`).
2. עברו אל **Maintenance → Wipe & re-genesis**. MOCHI יבצע:
   - מחיקת `peers/<alias>/storage`;
   - הרצת Kagami מחדש כדי לבנות configs/genesis תחת `genesis/`; ו‑
   - אתחול peers מחדש עם דריסות CLI/סביבה שנשמרו.
3. אם חייבים לעשות זאת ידנית:
   ```bash
   cargo run -p mochi-ui-egui -- --data-root /tmp/mochi --profile four-peer-bft --help
   # שימו לב לשורש בפלט, ואז:
   rm -rf /tmp/mochi/four-peer-bft
   ```
   לאחר מכן, אתחלו מחדש את MOCHI כדי ש‑`NetworkPaths::ensure` ייצור מחדש את העץ.

תמיד ארכבו את תיקיית `snapshots/<timestamp>` לפני מחיקה, אפילו בפיתוח מקומי —
החבילות הללו לוכדות את הלוגים וה‑configs המדויקים של `irohad` הדרושים לשחזור באגים.

### 4.1 שחזור מ‑snapshots

כאשר ניסוי משחית אחסון או צריך לשחזר מצב ידוע‑טוב, השתמשו בכפתור
**Restore snapshot** בדיאלוג Maintenance (או קראו ל‑`Supervisor::restore_snapshot`)
במקום להעתיק תיקיות ידנית. ספקו נתיב מוחלט לחבילה או את שם התיקייה המחוטא תחת
`snaphots/`. ה‑supervisor יבצע:

1. עצירת peers רצים;
2. אימות ש‑`metadata.json` של ה‑snapshot תואם ל‑`chain_id` הנוכחי ולמספר ה‑peers;
3. העתקת `peers/<alias>/{storage,snapshot,config.toml,latest.log}` בחזרה אל הפרופיל
   הפעיל; ו‑
4. שחזור `genesis/genesis.json` לפני אתחול peers אם הם רצו קודם.

אם ה‑snapshot נוצר עבור preset אחר או מזהה שרשרת אחר, הקריאה תחזיר
`SupervisorError::Config` כך שתוכלו להשיג חבילה תואמת במקום לערבב ארטיפקטים
בשקט. שמרו לפחות snapshot רענן אחד לכל preset כדי להאיץ תרגילי התאוששות.

## 5. תיקון זרמי בלוק/אירוע/סטטוס

- **הזרם נתקע אך peers בריאים.** בדקו את לוחות **Events**/**Blocks** עבור
  פסי סטטוס אדומים. לחצו “Stop” ואז “Start” כדי לאלץ את ה‑managed stream
  לבצע subscribe מחדש; ה‑supervisor רושם כל ניסיון התחברות מחדש (עם alias של
  peer ושגיאה) כך שניתן לאשר את שלבי ה‑backoff.
- **שכבת הסטטוס לא מעודכנת.** `ManagedStatusStream` מבצע polling ל‑`/status`
  כל שתי שניות ומסמן נתונים כ‑stale אחרי `STATUS_POLL_INTERVAL *
  STATUS_STALE_MULTIPLIER` (ברירת מחדל שש שניות). אם התג נשאר אדום, ודאו
  ש‑`torii_status_url` בקונפיג של ה‑peer נכון ושה‑gateway או ה‑VPN לא חוסמים
  חיבורי loopback.
- **כשלים בפענוח אירועים.** ה‑UI מדפיס את שלב הפענוח (raw bytes,
  `BlockSummary`, או Norito decode) ואת ה‑hash של העסקה הבעייתית. ייצאו את
  האירוע דרך כפתור ה‑clipboard כדי לשחזר את הפענוח בבדיקות
  (`mochi-core` חושף קונסטרוקטורים עזר תחת
  `mochi/mochi-core/src/torii.rs`).

כאשר זרמים קורסים שוב ושוב, עדכנו את הבעיה עם alias ה‑peer המדויק ומחרוזת
השגיאה (`ToriiErrorKind`) כדי שאבני הדרך הטלמטריות במפת הדרכים יישארו קשורות
לראיות קונקרטיות.

</div>
