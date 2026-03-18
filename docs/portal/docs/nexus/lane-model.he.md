---
lang: he
direction: rtl
source: docs/portal/docs/nexus/lane-model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7fedd5b720e5e2842bc88a16be8e3c3b9e8cdd773e3d58390a2281390818b33
source_last_modified: "2025-12-09T16:53:53.145158+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-lane-model
title: מודל lanes של Nexus
description: טקסונומיה לוגית של lanes, גאומטריית תצורה וכללי מיזוג world-state עבור Sora Nexus.
---

# מודל lanes של Nexus וחלוקת WSV

> **סטטוס:** תוצר NX-1 - טקסונומיית lanes, גאומטריית תצורה ופריסת אחסון מוכנות למימוש.  
> **Owners:** Nexus Core WG, Governance WG  
> **הפניה ל-roadmap:** NX-1 ב-`roadmap.md`

דף פורטל זה משקף את התקציר הקנוני `docs/source/nexus_lanes.md` כדי שמפעילי Sora Nexus, בעלי SDK ומבקרים יוכלו לקרוא את הנחיות ה-lanes בלי להיכנס לעץ ה-mono-repo. הארכיטקטורה המיועדת שומרת על world state דטרמיניסטי תוך שהיא מאפשרת ל-data spaces (lanes) בודדים להריץ סטים ציבוריים או פרטיים של מאמתים עם עומסי עבודה מבודדים.

## מושגים

- **Lane:** shard לוגי של ledger Nexus עם סט מאמתים משלו ו-backlog ביצוע. מזוהה על ידי `LaneId` יציב.
- **Data Space:** סל חקיקה שמאגד lane אחת או יותר שמשתפות מדיניות compliance, routing ו-settlement.
- **Lane Manifest:** מטא-דטה בשליטת ממשל שמתארת מאמתים, מדיניות DA, טוקן gas, כללי settlement והרשאות routing.
- **Global Commitment:** proof שמונפקת על ידי lane ומסכמת roots של מצב חדש, נתוני settlement והעברות cross-lane אופציונליות. טבעת NPoS הגלובלית מסדרת commitments.

## טקסונומיית lanes

סוגי lanes מתארים באופן קנוני את הנראות שלהם, משטח הממשל וה-hooks של settlement. גאומטריית התצורה (`LaneConfig`) לוכדת את המאפיינים הללו כך שצמתים, SDKs וכלי tooling יוכלו להסיק את ה-layout בלי לוגיקה ייעודית.

| סוג lane | נראות | חברות מאמתים | חשיפת WSV | ממשל ברירת מחדל | מדיניות settlement | שימוש טיפוסי |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | שכפול מצב מלא | SORA Parliament | `xor_global` | ledger ציבורי בסיסי |
| `public_custom` | public | Permissionless או stake-gated | שכפול מצב מלא | מודול משוקלל לפי stake | `xor_lane_weighted` | אפליקציות ציבוריות עתירות throughput |
| `private_permissioned` | restricted | סט מאמתים קבוע (מאושר ממשל) | Commitments ו-proofs | Federated council | `xor_hosted_custody` | CBDC, עומסי קונסורציום |
| `hybrid_confidential` | restricted | חברות מעורבת; עוטפת ZK proofs | Commitments + חשיפה סלקטיבית | מודול כסף מתכנת | `xor_dual_fund` | כסף מתכנת משמר פרטיות |

כל סוגי ה-lane חייבים להצהיר על:

- Alias של dataspace - קיבוץ קריא לבני אדם שקושר מדיניות compliance.
- Governance handle - מזהה שנפתר דרך `Nexus.governance.modules`.
- Settlement handle - מזהה שנצרך על ידי settlement router כדי לחייב buffers של XOR.
- מטא-דטה טלמטריה אופציונלית (תיאור, איש קשר, תחום עסקי) שמוצגת דרך `/status` ו-dashboards.

## גאומטריית תצורה של lanes (`LaneConfig`)

`LaneConfig` היא גאומטריית runtime שנגזרת מה-catalog המאומת של lanes. היא לא מחליפה manifests של ממשל; במקום זאת היא מספקת מזהי אחסון דטרמיניסטיים ורמזי טלמטריה לכל lane שמוגדרת.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` מחשב מחדש את הגאומטריה כאשר התצורה נטענת (`State::set_nexus`).
- Aliases עוברים סניטיזציה ל-slugs באותיות קטנות; רצפים של תווים לא אלפאנומריים מתכווצים ל-`_`. אם ה-alias יוצר slug ריק חוזרים ל-`lane{id}`.
- `shard_id` נגזר ממפתח metadata בשם `da_shard_id` (ברירת מחדל `lane_id`) ומניע את יומן cursor של shard שנשמר כדי לשמור על DA replay דטרמיניסטי בין restarts/resharding.
- Prefixes של מפתח מבטיחים ש-WSV ישמור טווחי מפתחות נפרדים לכל lane גם כאשר משתמשים באותו backend.
- שמות מקטעי Kura דטרמיניסטיים בין hosts; מבקרים יכולים לאמת ספריות מקטעים ו-manifests בלי tooling ייעודי.
- מקטעי merge (`lane_{id:03}_merge`) מאחסנים את roots האחרונים של merge-hint ואת commitments של מצב גלובלי לאותה lane.

## חלוקת world-state

- world state הלוגי של Nexus הוא איחוד של מרחבי מצב לכל lane. lanes ציבוריות שומרות מצב מלא; lanes פרטיות/confidential מייצאות roots של Merkle/commitment ל-merge ledger.
- אחסון MV מקדים כל מפתח עם prefix של 4 בתים מ-`LaneConfigEntry::key_prefix`, וכך מתקבלות מפתחות כמו `[00 00 00 01] ++ PackedKey`.
- טבלאות משותפות (accounts, assets, triggers, רשומות ממשל) מאחסנות רשומות מקובצות לפי prefix של lane, מה ששומר על range scans דטרמיניסטיים.
- מטא-דטה של merge-ledger משקפת את אותו layout: כל lane כותבת roots של merge-hint ו-roots של מצב גלובלי מצומצם ל-`lane_{id:03}_merge`, כך שניתן לבצע retention או eviction ממוקד כשה-lane יוצאת לפנסיה.
- אינדקסים cross-lane (account aliases, asset registries, governance manifests) מאחסנים prefixes מפורשים של lane כדי לאפשר למפעילים לבצע reconciliation מהיר.
- **מדיניות retention** - lanes ציבוריות שומרות bodies מלאים של בלוקים; lanes עם commitments בלבד יכולות לבצע compact לגופים ישנים אחרי checkpoints כי commitments הם מקור סמכותי. lanes confidential שומרות journals מוצפנים במקטעים ייעודיים כדי לא לחסום workloads אחרים.
- **Tooling** - כלי תחזוקה (`kagami`, פקודות admin ב-CLI) צריכים להפנות ל-namespace עם slug בעת חשיפת metrics, תוויות Prometheus או ארכוב מקטעי Kura.

## Routing ו-APIs

- נקודות קצה של Torii REST/gRPC מקבלות `lane_id` אופציונלי; היעדר מצביע ל-`lane_default`.
- SDKs מציגים בוררי lane וממפים aliases ידידותיים ל-`LaneId` באמצעות catalog ה-lanes.
- כללי routing פועלים על ה-catalog המאומת ויכולים לבחור גם lane וגם dataspace. `LaneConfig` מספק aliases ידידותיים לטלמטריה ב-dashboards ו-logs.

## Settlement ו-fees

- כל lane משלמת fees של XOR לסט המאמתים הגלובלי. lanes יכולות לגבות טוקני gas מקומיים אך חייבות להפקיד equivalents של XOR לצד commitments.
- proofs של settlement כוללים סכום, מטא-דטה המרה והוכחת escrow (לדוגמה, העברה ל-vault הגלובלי של fees).
- settlement router המאוחד (NX-3) מחייב buffers לפי אותו prefix של lane, כך שטלמטריית settlement מסתנכרנת עם גאומטריית האחסון.

## Governance

- lanes מצהירות על מודול הממשל שלהן דרך ה-catalog. `LaneConfigEntry` נושא את ה-alias וה-slug המקוריים כדי להשאיר את הטלמטריה ושבילי הביקורת קריאים.
- ה-registry של Nexus מפיץ manifests חתומים של lanes שכוללים `LaneId`, binding של dataspace, governance handle, settlement handle ו-metadata.
- hooks של runtime-upgrade ממשיכים לאכוף מדיניות ממשל (`gov_upgrade_id` כברירת מחדל) ולרשום diffs דרך telemetry bridge (אירועי `nexus.config.diff`).

## Telemetry ו-status

- `/status` מציג aliases של lane, bindings של dataspace, handles של ממשל ופרופילי settlement, שנגזרו מה-catalog ומה-`LaneConfig`.
- מדדי scheduler (`nexus_scheduler_lane_teu_*`) מציגים aliases/slugs כדי שהמפעילים יוכלו למפות backlog ולחץ TEU במהירות.
- `nexus_lane_configured_total` סופר את מספר רשומות ה-lane הנגזרות ומחושב מחדש כאשר התצורה משתנה. הטלמטריה משדרת diffs חתומים כאשר גאומטריית ה-lanes משתנה.
- gauges של backlog ל-dataspace כוללים מטא-דטה של alias/description כדי לסייע למפעילים לשייך לחץ תור לדומיינים עסקיים.

## תצורה וסוגי Norito

- `LaneCatalog`, `LaneConfig`, ו-`DataSpaceCatalog` נמצאים ב-`iroha_data_model::nexus` ומספקים מבנים תואמי Norito עבור manifests ו-SDKs.
- `LaneConfig` נמצא ב-`iroha_config::parameters::actual::Nexus` ונגזר אוטומטית מה-catalog; הוא לא דורש Norito encoding כי הוא helper פנימי runtime.
- תצורת המשתמש (`iroha_config::parameters::user::Nexus`) ממשיכה לקבל תיאורים דקלרטיביים של lane ו-dataspace; ה-parsing כעת גוזר את הגאומטריה ודוחה aliases לא חוקיים או IDs כפולים של lane.

## עבודה שנותרה

- לשלב עדכוני settlement router (NX-3) עם הגאומטריה החדשה כדי שחיובים וקבלות של buffers XOR יתויגו לפי slug של lane.
- להרחיב tooling admin כדי לרשום column families, לבצע compact ל-lanes שפרשו ולבדוק logs של בלוקים לפי lane תוך שימוש ב-namespace עם slug.
- להשלים את אלגוריתם ה-merge (ordering, pruning, conflict detection) ולצרף fixtures של רגרסיה ל-replay cross-lane.
- להוסיף hooks של compliance עבור whitelists/blacklists ומדיניות כסף מתכנת (נעקב תחת NX-12).

---

*דף זה ימשיך לעקוב אחרי follow-ups של NX-1 כאשר NX-2 עד NX-18 יגיעו. אנא העלו שאלות פתוחות ב-`roadmap.md` או ב-tracker של הממשל כדי שהפורטל יישאר מיושר עם ה-docs הקנוניים.*
