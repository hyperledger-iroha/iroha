---
lang: he
direction: rtl
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sorafs/orchestrator-config.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e97f85aaef5b3b2b6ef0d15c43b03c3782a2181abadca09a97527ede80d4f346
source_last_modified: "2025-11-14T04:43:21.972013+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/developer/orchestrator.md`. שמרו על סנכרון שתי הגרסאות עד שהסט הישן של התיעוד יוסר.
:::

# מדריך אורקסטרטור fetch רב-מקורות

אורקסטרטור ה-fetch רב-המקורות של SoraFS מניע הורדות דטרמיניסטיות ומקבילות מתוך
סט הספקים שפורסם ב-adverts הנתמכים על ידי הממשל. מדריך זה מסביר כיצד להגדיר את
האורקסטרטור, אילו אותות כשל לצפות במהלך rollouts, ואילו זרמי טלמטריה חושפים
אינדיקטורים לבריאות.

## 1. סקירת תצורה

האורקסטרטור ממזג שלושה מקורות תצורה:

| מקור | מטרה | הערות |
|------|------|-------|
| `OrchestratorConfig.scoreboard` | מנרמל משקלי ספקים, מאמת טריות טלמטריה, ושומר את ה-scoreboard JSON לשם ביקורות. | נתמך ב-`crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | מיישם מגבלות זמן ריצה (תקציבי retry, גבולות מקביליות, מתגי אימות). | ממופה אל `FetchOptions` ב-`crates/sorafs_car::multi_fetch`. |
| פרמטרים של CLI / SDK | מגבילים את מספר ה-peers, מצרפים אזורי טלמטריה, וחושפים מדיניות deny/boost. | `sorafs_cli fetch` חושף את הדגלים ישירות; ה-SDKs מעבירים אותם דרך `OrchestratorConfig`. |

עוזרי JSON ב-`crates/sorafs_orchestrator::bindings` מסדרים את כל התצורה ל-Norito
JSON, מה שהופך אותה לניידת בין bindings של SDK לאוטומציה.

### 1.1 דוגמת תצורת JSON

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

שמרו את הקובץ באמצעות שכבות `iroha_config` הרגילות (`defaults/`, user, actual)
כדי שפריסות דטרמיניסטיות יורשו את אותם גבולות בכל הצמתים. לפרופיל fallback
במצב direct-only המתיישר עם rollout של SNNet-5a, ראו
`docs/examples/sorafs_direct_mode_policy.json` ואת ההנחיות הנלוות ב-
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 Overrides של תאימות

SNNet-9 משלבת תאימות מונחית-ממשל באורקסטרטור. אובייקט `compliance` חדש בתצורת
Norito JSON מתעד את ה-carve-outs שמאלצים את pipeline ה-fetch למצב direct-only:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` מצהיר על קודי ISO‑3166 alpha‑2 שבהם מופעל מופע
  האורקסטרטור. הקודים מנורמלים לאותיות גדולות בעת הפרסינג.
- `jurisdiction_opt_outs` משקף את רישום הממשל. כאשר כל תחום שיפוט של המפעיל
  מופיע ברשימה, האורקסטרטור אוכף `transport_policy=direct-only` ומפליט את
  סיבת ה-fallback `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` מפרט digests של manifest (blinded CIDs בקידוד hex
  באותיות גדולות). התאמות מכריחות גם תזמון direct-only וחושפות את fallback
  `compliance_blinded_cid_opt_out` בטלמטריה.
- `audit_contacts` מתעד URI שהממשל מצפה מהמפעילים לפרסם ב-playbooks של GAR.
- `attestations` לוכד חבילות תאימות חתומות שמגבות את המדיניות. כל רשומה מגדירה
  `jurisdiction` אופציונלי (ISO‑3166 alpha‑2), `document_uri`, `digest_hex`
  קנוני באורך 64 תווים, חותמת זמן `issued_at_ms` ו-`expires_at_ms` אופציונלי.
  הארטיפקטים האלו מזינים את צ׳ק-ליסט הביקורת של האורקסטרטור כדי שכלי ממשל
  יקשרו את ה-overrides לניירת החתומה.

ספקו את בלוק התאימות דרך שכבות התצורה הרגילות כך שהמפעילים יקבלו overrides
דטרמיניסטיים. האורקסטרטור מיישם תאימות _לאחר_ רמזי write-mode: גם אם SDK
מבקש `upload-pq-only`, opt-outs לפי תחום שיפוט או manifest עדיין מכריחים
direct-only ונכשלים מהר כאשר אין ספקים תואמים.

קטלוגי opt-out קנוניים נמצאים תחת
`governance/compliance/soranet_opt_outs.json`; מועצת הממשל מפרסמת עדכונים דרך
releases מתוייגים. דוגמה מלאה לתצורה (כולל attestations) זמינה ב-
`docs/examples/sorafs_compliance_policy.json`, והתהליך התפעולי מתועד ב-
[playbook תאימות GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 כפתורי CLI ו-SDK

| דגל / שדה | השפעה |
|-----------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | מגביל כמה ספקים עוברים את מסנן ה-scoreboard. הגדירו `None` כדי להשתמש בכל הספקים הכשירים. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | מגביל retries לכל chunk. חריגה מהגבול מעלה `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | מזריק snapshots של latency/failure לבונה ה-scoreboard. טלמטריה ישנה מעבר ל-`telemetry_grace_secs` מסמנת ספקים כלא כשירים. |
| `--scoreboard-out` | שומר את ה-scoreboard המחושב (ספקים כשירים + לא כשירים) לצורך בדיקה לאחר הריצה. |
| `--scoreboard-now` | עוקף את חותמת הזמן של ה-scoreboard (שניות Unix) כדי לשמור על דטרמיניזם של fixtures. |
| `--deny-provider` / hook של מדיניות score | מוציא ספקים מתזמון בצורה דטרמיניסטית ללא מחיקת adverts. שימושי להרחיקה מהירה. |
| `--boost-provider=name:delta` | מתאים קרדיטי round-robin משוקללים לספק תוך שמירה על משקלי ממשל. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | מתייג מדדים ולוגים מובנים כדי שדשבורדים יוכלו לסנן לפי גאוגרפיה או גל rollout. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | ברירת המחדל היא `soranet-first` כעת שהאורקסטרטור הרב-מקורות בסיסי. השתמשו ב-`direct-only` בעת downgrades או הנחיות תאימות, ושמרו `soranet-strict` לפיילוטים PQ-only; overrides של תאימות עדיין מהווים תקרת קשיחה. |

SoraNet-first הוא כעת ברירת המחדל למשלוח, ו-rollbacks חייבים לציין את ה-SNNet
blocker הרלוונטי. לאחר ש-SNNet-4/5/5a/5b/6a/7/8/12/13 יסיימו, הממשל יקשיח את
העמדה הנדרשת (לכיוון `soranet-strict`); עד אז, רק overrides שמונעים מאירועים
צריכים להעדיף `direct-only`, וחובה לרשום אותם בלוג rollout.

כל הדגלים לעיל מקבלים תחביר `--` הן ב-`sorafs_cli fetch` והן בבינארי
`sorafs_fetch`. ה-SDKs חושפים את אותם options דרך builders טיפוסיים.

### 1.4 ניהול Guard Cache

ה-CLI מחבר כעת את בוחר ה-guards של SoraNet כדי שהמפעילים יוכלו לנעול relays
כניסה באופן דטרמיניסטי לקראת rollout מלא של SNNet-5. שלושה דגלים חדשים שולטים
בזרימה:

| דגל | מטרה |
|-----|------|
| `--guard-directory <PATH>` | מצביע לקובץ JSON שמתאר את הקונצנזוס העדכני של relays (תת-סט למטה). העברת directory מרעננת את cache ה-guards לפני fetch. |
| `--guard-cache <PATH>` | משמר `GuardSet` בקידוד Norito. ריצות עוקבות משתמשות ב-cache גם ללא directory חדש. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | Overrides אופציונליים למספר guards כניסה (ברירת מחדל 3) ולחלון השימור (ברירת מחדל 30 ימים). |
| `--guard-cache-key <HEX>` | מפתח אופציונלי באורך 32 בתים לתגית Blake3 MAC לקבצי guard cache כדי לאמת אותם לפני שימוש חוזר. |

מטעני directory של guards משתמשים בסכימה קומפקטית:

דגל `--guard-directory` מצפה כעת למטען `GuardDirectorySnapshotV2` בקידוד Norito.
ה-snapshot הבינארי כולל:

- `version` — גרסת הסכימה (כרגע `2`).
- `directory_hash`, `published_at_unix`, `valid_after_unix`, `valid_until_unix` —
  מטאדאטה של קונצנזוס שחייבת להתאים לכל תעודה מוטמעת.
- `validation_phase` — שער מדיניות תעודות (`1` = לאפשר חתימת Ed25519 יחידה,
  `2` = להעדיף חתימות כפולות, `3` = לדרוש חתימות כפולות).
- `issuers` — מנפיקי ממשל עם `fingerprint`, `ed25519_public`, `mldsa65_public`.
  ה-fingerprint מחושב כ-
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — רשימת חבילות SRCv2 (פלט `RelayCertificateBundleV2::to_cbor()`). כל
  חבילה נושאת descriptor של relay, דגלי יכולת, מדיניות ML-KEM וחתימות כפולות
  Ed25519/ML-DSA-65.

ה-CLI מאמת כל bundle מול מפתחות issuer המוצהרים לפני מיזוג ה-directory עם
guard cache. סקיצות JSON ישנות אינן מתקבלות; נדרשות snapshots SRCv2.

הפעילו את ה-CLI עם `--guard-directory` כדי למזג את הקונצנזוס העדכני עם cache
קיים. הבוחר משמר guards שנעולים ועדיין בתוך חלון השימור וכשירים ב-directory;
relays חדשים מחליפים רשומות שפג תוקפן. לאחר fetch מוצלח ה-cache המעודכן נכתב
חזרה לנתיב שסופק ב-`--guard-cache`, כדי לשמור על דטרמיניזם בהפעלות הבאות. SDKs
יכולים לשחזר את אותו התנהגות ע״י קריאה ל-
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` והעברת
`GuardSet` ל-`SorafsGatewayFetchOptions`.

`ml_kem_public_hex` מאפשר לבוחר להעדיף guards עם יכולת PQ במהלך rollout SNNet-5.
מתגי שלב (`anon-guard-pq`, `anon-majority-pq`, `anon-strict-pq`) מדרדרים כעת
relays קלאסיים באופן אוטומטי: כאשר guard PQ זמין, הבוחר מסיר pins קלאסיים עודפים
כך שסשנים הבאים יעדיפו handshakes היברידיים. סיכומי CLI/SDK חושפים את המיקס
באמצעות `anonymity_status`/`anonymity_reason`, `anonymity_effective_policy`,
`anonymity_pq_selected`, `anonymity_classical_selected`, `anonymity_pq_ratio`,
`anonymity_classical_ratio`, ושדות נוספים של מועמדים/חוסר/דלתא באספקה, כך ש-
brownouts ו-fallbacks קלאסיים גלויים במפורש.

Directories של guards יכולים כעת לשלב bundle מלא של SRCv2 דרך
`certificate_base64`. האורקסטרטור מפענח כל bundle, מאמת מחדש חתימות
Ed25519/ML-DSA ושומר את התעודה המנותחת לצד cache ה-guards. כאשר תעודה קיימת היא
הופכת למקור הקנוני למפתחות PQ, העדפות handshake ומשקול; תעודות שפג תוקפן
מושלכות והבוחר חוזר לשדות ה-descriptor הישנים. תעודות זורמות לניהול מחזור חיי
circuit ונחשפות דרך `telemetry::sorafs.guard` ו-`telemetry::sorafs.circuit`, עם
רישום חלון התוקף, חבילות handshake והאם נצפו חתימות כפולות לכל guard.

השתמשו ב-CLI helpers כדי לשמור snapshots מסונכרנים עם המפרסמים:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` מוריד ומאמת את snapshot SRCv2 לפני כתיבה לדיסק, בעוד `verify` מריץ מחדש
את צינור האימות עבור ארטיפקטים מצוותים אחרים, ומפיק תקציר JSON שמשקף את פלט
בוחר ה-guards ב-CLI/SDK.

### 1.5 מנהל מחזור חיי Circuit

כאשר גם relay directory וגם guard cache מסופקים, האורקסטרטור מפעיל מנהל מחזור
חיי circuits כדי לבנות מראש ולחדש מעגלי SoraNet לפני כל fetch. התצורה נמצאת ב-
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) דרך שני שדות:

- `relay_directory`: נושא את snapshot ה-directory של SNNet-3 כדי לבחור middle/exit
  hops באופן דטרמיניסטי.
- `circuit_manager`: תצורה אופציונלית (מופעלת כברירת מחדל) השולטת ב-TTL של
  circuit.

Norito JSON מקבל כעת בלוק `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

ה-SDKs מעבירים את נתוני ה-directory דרך
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`), וה-CLI מחבר אותו אוטומטית כאשר
`--guard-directory` מסופק (`crates/iroha_cli/src/commands/sorafs.rs:365`).

המנהל מחדש circuits כאשר מטאדאטה של guard משתנה (endpoint, מפתח PQ או timestamp
מקובע) או כאשר ה-TTL פג. העזר `refresh_circuits` שנקרא לפני כל fetch
(`crates/sorafs_orchestrator/src/lib.rs:1346`) פולט לוגים של `CircuitEvent` כדי
לאפשר למפעילים לעקוב אחר החלטות מחזור החיים. ה-soak test
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) מדגים לטנסיה יציבה לאורך שלוש
רוטציות guard; ראו את הדוח ב-
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 Proxy QUIC מקומי

האורקסטרטור יכול לבחור להפעיל proxy QUIC מקומי כך שתוספי דפדפן ומתאמי SDK לא
יצטרכו לנהל תעודות או מפתחות guard cache. ה-proxy נקשר לכתובת loopback, מסיים
חיבורי QUIC ומחזיר manifest של Norito המתאר את התעודה ואת מפתח guard cache
האופציונלי ללקוח. אירועי transport שנפלטים מה-proxy נספרים דרך
`sorafs_orchestrator_transport_events_total`.

אפשרו את ה-proxy דרך בלוק `local_proxy` החדש ב-JSON של האורקסטרטור:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```

- `bind_addr` קובע היכן ה-proxy מאזין (השתמשו בפורט `0` כדי לבקש פורט זמני).
- `telemetry_label` מתפשט למדדים כדי שדשבורדים יבדילו בין proxy לסשנים של fetch.
- `guard_cache_key_hex` (אופציונלי) מאפשר ל-proxy לחשוף את אותו cache של guards
  עם מפתח שה-CLI/SDKs משתמשים בו, כדי לשמור על סנכרון הרחבות הדפדפן.
- `emit_browser_manifest` מחליף אם ה-handshake מחזיר manifest שהרחבות יכולות
  לשמור ולאמת.
- `proxy_mode` בוחר האם ה-proxy מגשר תעבורה מקומית (`bridge`) או רק מפיק
  מטאדאטה כך שה-SDKs יפתחו circuits של SoraNet בעצמם (`metadata-only`). ברירת
  המחדל `bridge`; בחרו `metadata-only` כאשר תחנה צריכה לחשוף את ה-manifest ללא
  העברת streams.
- `prewarm_circuits`, `max_streams_per_circuit`, `circuit_ttl_hint_secs` חושפים
  רמזים נוספים לדפדפן כדי להקצות תקציב ל-streams מקבילים ולהבין את אגרסיביות
  השימוש החוזר ב-circuits.
- `car_bridge` (אופציונלי) מצביע ל-cache מקומי של CAR archive. שדה `extension`
  קובע את הסיומת כשמטרת ה-stream משמיטה `*.car`; הגדירו `allow_zst = true` כדי
  לשרת payloads של `*.car.zst` ישירות.
- `kaigi_bridge` (אופציונלי) חושף מסלולי Kaigi ל-proxy. שדה `room_policy` מכריז
  אם ה-bridge פועל ב-`public` או `authenticated` כדי שדפדפנים יבחרו מראש את
  labels של GAR.
- `sorafs_cli fetch` חושף `--local-proxy-mode=bridge|metadata-only` ו-
  `--local-proxy-norito-spool=PATH`, כך שאפשר לשנות מצב ריצה או לבחור spools
  חלופיים ללא שינוי מדיניות JSON.
- `downgrade_remediation` (אופציונלי) מגדיר hook של downgrade אוטומטי. כאשר הוא
  פעיל, האורקסטרטור עוקב אחרי טלמטריית relays ל-bursts של downgrade, ולאחר חציית
  `threshold` בתוך `window_secs` הוא כופה על ה-proxy לעבור ל-`target_mode` (ברירת
  מחדל `metadata-only`). אחרי שה-downgrades נפסקים, ה-proxy חוזר ל-`resume_mode`
  לאחר `cooldown_secs`. השתמשו במערך `modes` כדי להגביל את הטריגר לתפקידי relay
  ספציפיים (ברירת מחדל entry relays).

כאשר ה-proxy רץ במצב bridge הוא מספק שני שירותי אפליקציה:

- **`norito`** — יעד ה-stream של הלקוח נפתר יחסית ל-`norito_bridge.spool_dir`.
  היעדים עוברים sanitization (ללא traversal וללא נתיבים מוחלטים), וכאשר הקובץ
  חסר סיומת, הסיומת המוגדרת מיושמת לפני שידור ה-payload לדפדפן.
- **`car`** — יעדי ה-stream נפתרים בתוך `car_bridge.cache_dir`, יורשים את הסיומת
  ברירת המחדל שהוגדרה ודוחים payloads דחוסים אלא אם `allow_zst` מופעל. bridges
  מוצלחים מחזירים `STREAM_ACK_OK` לפני העברת הבייטים כדי שהלקוחות יוכלו לבצע
  pipeline של אימות.

בשני המקרים ה-proxy מספק HMAC של cache-tag (כאשר guard cache key היה נוכח במהלך
handshake) ורושם קודי סיבה של טלמטריה `norito_*` / `car_*` כדי שדשבורדים יזהו
הצלחות, קבצים חסרים וכשלי sanitization במהירות.

`Orchestrator::local_proxy().await` חושף את ה-handle הפועל כך שאפשר לקרוא את PEM
של התעודה, לקבל את manifest הדפדפן או לבקש כיבוי מסודר בעת סיום האפליקציה.

כאשר ה-proxy מופעל, הוא מגיש כעת רשומות **manifest v2**. מעבר לתעודה קיימת
ולמפתח guard cache, v2 מוסיפה:

- `alpn` (`"sorafs-proxy/1"`) ומערך `capabilities` כדי לאשר את פרוטוקול ה-stream.
- `session_id` לכל handshake ובלוק `cache_tagging` כדי לגזור affinities של guards
  לכל סשן ותגיות HMAC.
- רמזי circuit ו-guard selection (`circuit`, `guard_selection`, `route_hints`) כדי
  שהאינטגרציות בדפדפן יציגו UI עשיר יותר לפני פתיחת streams.
- `telemetry_v2` עם knobs של דגימה ופרטיות למדידה מקומית.
- כל `STREAM_ACK_OK` כולל `cache_tag_hex`. הלקוחות משקפים את הערך בכותרת
  `x-sorafs-cache-tag` בעת בקשות HTTP או TCP כדי לשמור על הצפנת בחירות guard
  במנוחה.

השדות האלו תואמים לאחור — לקוחות ישנים יכולים להתעלם מהמפתחות החדשים ולהמשיך
להסתמך על תת-הקבוצה של v1.

## 2. סמנטיקת כשל

האורקסטרטור אוכף בדיקות יכולת ותקציב מחמירות לפני מעבר בייט אחד. הכשלים
מתחלקים לשלוש קטגוריות:

1. **כשלים של כשירות (pre-flight).** ספקים ללא יכולת טווח, adverts שפג תוקפן,
   או טלמטריה מיושנת נרשמים בארטיפקט ה-scoreboard ומושמטים מהתזמון. סיכומי CLI
   ממלאים את המערך `ineligible_providers` עם הסיבות כדי שמפעילים יבדקו drift
   של governance בלי לגרד לוגים.
2. **התשה בזמן ריצה.** כל ספק עוקב אחרי כשלי רצף. ברגע ש-`provider_failure_threshold`
   מושג, הספק מסומן `disabled` לשארית הסשן. אם כל הספקים עוברים ל-`disabled`,
   האורקסטרטור מחזיר `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **עצירות דטרמיניסטיות.** מגבלות קשיחות מופיעות כשגיאות מובנות:
   - `MultiSourceError::NoCompatibleProviders` — ה-manifest דורש span או alignment
     שהספקים שנותרו לא מסוגלים לעמוד בו.
   - `MultiSourceError::ExhaustedRetries` — תקציב retries לכל chunk נוצל.
   - `MultiSourceError::ObserverFailed` — observers downstream (hooks של streaming)
     דחו chunk מאומת.

כל שגיאה מכילה את אינדקס ה-chunk הבעייתי ואת סיבת הכשל הסופית של הספק (כאשר
זמינה). ראו בכך blockers לשחרור — retries עם אותו קלט ישחזרו את הכשל עד שה-
advert, הטלמטריה או בריאות הספק ישתנו.

### 2.1 שמירת scoreboard

כאשר `persist_path` מוגדר, האורקסטרטור כותב את ה-scoreboard הסופי לאחר כל ריצה.
מסמך ה-JSON כולל:

- `eligibility` (`eligible` או `ineligible::<reason>`).
- `weight` (משקל מנורמל שיועד לריצה זו).
- מטאדאטה של `provider` (מזהה, endpoints, תקציב מקביליות).

ארכבו snapshots של scoreboard לצד ארטיפקטים של release כדי שהחלטות blacklist
ו-rollout יישארו ניתנות לביקורת.

## 3. טלמטריה ודיבוג

### 3.1 מדדי Prometheus

האורקסטרטור מפיק את המדדים הבאים דרך `iroha_telemetry`:

| מדד | Labels | תיאור |
|-----|--------|-------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`, `region` | Gauge של fetch פעילים. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`, `region` | היסטוגרמה שמקליטה את זמן ה-fetch מקצה לקצה. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`, `region`, `reason` | מונה כשלים סופיים (ריטריי מותש, ללא ספקים, כשל observer). |
| `sorafs_orchestrator_retries_total` | `manifest_id`, `provider`, `reason` | מונה נסיונות retry לכל ספק. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`, `provider`, `reason` | מונה כשלים של ספק ברמת סשן שמובילים להשבתה. |
| `sorafs_orchestrator_policy_events_total` | `region`, `stage`, `outcome`, `reason` | ספירת החלטות מדיניות אנונימיות (עמידה מול brownout) לפי שלב rollout וסיבת fallback. |
| `sorafs_orchestrator_pq_ratio` | `region`, `stage` | היסטוגרמה של חלק יחסי של relays PQ בתוך הסט הנבחר של SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`, `stage` | היסטוגרמה של יחס היצע PQ ב-snapshot של scoreboard. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`, `stage` | היסטוגרמה של deficit במדיניות (פער בין יעד לבין שיעור PQ בפועל). |
| `sorafs_orchestrator_classical_ratio` | `region`, `stage` | היסטוגרמה של שיעור relays קלאסיים בכל סשן. |
| `sorafs_orchestrator_classical_selected` | `region`, `stage` | היסטוגרמה של מספר relays קלאסיים שנבחרו לכל סשן. |

שלבו את המדדים בדשבורדי staging לפני הפעלת knobs בפרודקשן. הפריסה המומלצת
משקפת את תוכנית observability של SF-6:

1. **Fetches פעילים** — התרעה אם המדד עולה בלי completions תואמים.
2. **Retry ratio** — מזהיר כאשר מוני `retry` חוצים baselines היסטוריים.
3. **כשלים בספקים** — מפעיל pager כשכל ספק חוצה `session_failure > 0` בתוך 15 דקות.

### 3.2 יעדי לוג מובנים

האורקסטרטור מפרסם אירועים מובנים ליעדים דטרמיניסטיים:

- `telemetry::sorafs.fetch.lifecycle` — סמני מחזור חיים `start` ו-`complete` עם
  מספר chunk, retries ומשך כולל.
- `telemetry::sorafs.fetch.retry` — אירועי retry (`provider`, `reason`, `attempts`).
- `telemetry::sorafs.fetch.provider_failure` — ספקים שהושבתו עקב שגיאות חוזרות.
- `telemetry::sorafs.fetch.error` — כשלים סופיים עם `reason` ומטאדאטה אופציונלית של הספק.

נתבו את הזרמים הללו לצינור הלוגים Norito הקיים כך שלתגובת תקריות תהיה מקור אמת
אחד. אירועי מחזור חיים חושפים את תמהיל PQ/קלאסי דרך `anonymity_effective_policy`,
`anonymity_pq_ratio`, `anonymity_classical_ratio` והמונים הנלווים, כך שאפשר לבנות
דשבורדים בלי scraping של מדדים. במהלך rollouts של GA, קבעו רמת log ל-`info` עבור
אירועי מחזור חיים/รีtry והסתמכו על `warn` עבור כשלים סופיים.

### 3.3 סיכומי JSON

גם `sorafs_cli fetch` וגם SDK Rust מחזירים סיכום מובנה הכולל:

- `provider_reports` עם ספירות הצלחה/כשל והאם ספק הושבת.
- `chunk_receipts` המפרטים איזה ספק סיפק כל chunk.
- מערכים `retry_stats` ו-`ineligible_providers`.

ארכבו את קובץ הסיכום בעת דיבוג ספקים בעייתיים — ה-receipts מתאימים ישירות
למטאדאטה הלוגית לעיל.

## 4. צ׳ק-ליסט תפעולי

1. **ביצוע staging של תצורה ב-CI.** הריצו `sorafs_fetch` עם התצורה היעדית,
   העבירו `--scoreboard-out` כדי ללכוד את תצוגת הזכאות, והשו אותה ל-release
   הקודם. כל ספק לא כשיר שלא צפיתם בו עוצר את ההתקדמות.
2. **אימות טלמטריה.** ודאו שהפריסה מוציאה מדדים `sorafs.fetch.*` ולוגים מובנים
   לפני הפעלת fetch רב-מקורות למשתמשים. היעדר מדדים לרוב מצביע על כך שהאורקסטרטור
   לא נקרא.
3. **תיעוד overrides.** בעת `--deny-provider` או `--boost-provider` חירומיים,
   רשמו את ה-JSON (או קריאת CLI) ב-changelog. Rollbacks חייבים להסיר את override
   ולצלם snapshot חדש של scoreboard.
4. **הרצת smoke tests מחדש.** לאחר שינוי budgets של retry או caps של ספקים,
   בצעו fetch מחדש של fixture קנוני (`fixtures/sorafs_manifest/ci_sample/`) וודאו
   ש-receipts של chunks נשארים דטרמיניסטיים.

ביצוע הצעדים הללו שומר על התנהגות אורקסטרטור ניתנת לשחזור בגלי rollout ומספק
את הטלמטריה הנדרשת לתגובה לאירועים.

### 4.1 Overrides למדיניות

מפעילים יכולים לנעול את שלב ה-transport/anonimity הפעיל בלי לערוך את תצורת הבסיס
על ידי הגדרת `policy_override.transport_policy` ו-`policy_override.anonymity_policy`
ב-JSON של `orchestrator` (או אספקת `--transport-policy-override=` /
`--anonymity-policy-override=` ל-`sorafs_cli fetch`). כאשר override קיים,
האורקסטרטור מדלג על fallback brownout הרגיל: אם רמת ה-PQ המבוקשת לא ניתנת
להשגה, ה-fetch נכשל עם `no providers` במקום להדרדר בשקט. החזרה להתנהגות ברירת
המחדל היא פשוט ניקוי שדות ה-override.
