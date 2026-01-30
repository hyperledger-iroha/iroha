---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/nexus/nexus-default-lane-quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b6a200e5dd9846bc568dbfee1727b378d2ef77dbd4ca61e3b56176fba0f6fdde
source_last_modified: "2025-11-14T04:43:20.384579+00:00"
translation_last_reviewed: 2026-01-30
---

:::note מקור קנוני
העמוד הזה משקף את `docs/source/quickstart/default_lane.md`. שמרו על שתי הגרסאות מסונכרנות עד שסבב הלוקליזציה יגיע לפורטל.
:::

# התחלה מהירה ל-default lane (NX-5)

> **הקשר Roadmap:** NX-5 - שילוב ה-default public lane. סביבת הריצה חושפת כעת fallback `nexus.routing_policy.default_lane` כך שנקודות הקצה REST/gRPC של Torii וכל SDK יכולים להשמיט בבטחה `lane_id` כאשר התנועה שייכת ל-public lane הקנוני. המדריך הזה מוביל מפעילים דרך הגדרת הקטלוג, אימות ה-fallback ב-`/status`, ובדיקת התנהגות הלקוח מקצה לקצה.

## דרישות מקדימות

- build של Sora/Nexus ל-`irohad` (הרצה עם `irohad --sora --config ...`).
- גישה לריפו הקונפיגורציה כדי לערוך את מקטעי `nexus.*`.
- `iroha_cli` מוגדר כדי לדבר עם ה-cluster היעד.
- `curl`/`jq` (או חלופה) כדי לבדוק את payload `/status` של Torii.

## 1. תיאור קטלוג ה-lane וה-dataspace

הכריזו על lanes ו-dataspaces שצריכים להתקיים ברשת. הקטע הבא (מקוצר מתוך `defaults/nexus/config.toml`) רושם שלושה lanes ציבוריים יחד עם aliases תואמים ל-dataspace:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

כל `index` חייב להיות ייחודי ורציף. מזהי dataspace הם ערכי 64-ביט; הדוגמאות למעלה משתמשות באותם ערכים מספריים כמו אינדקסי lane לצורך בהירות.

## 2. הגדרת ברירות מחדל לניתוב ועקיפות אופציונליות

המקטע `nexus.routing_policy` שולט ב-fallback lane ומאפשר עקיפת ניתוב עבור הוראות ספציפיות או prefix-ים של חשבון. אם שום כלל לא תואם, ה-scheduler מנתב את העסקה ל-`default_lane` ול-`default_dataspace` שהוגדרו. לוגיקת ה-router נמצאת ב-`crates/iroha_core/src/queue/router.rs` ומחילה את המדיניות באופן שקוף על משטחי Torii REST/gRPC.

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```

כאשר מוסיפים lanes חדשים בהמשך, עדכנו קודם את הקטלוג ואז הרחיבו את כללי הניתוב. ה-fallback lane צריך להמשיך להצביע על ה-public lane שמחזיק את רוב תעבורת המשתמשים כדי ש-SDKs ישנים יישארו תואמים.

## 3. אתחול node עם המדיניות מופעלת

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

ה-node רושם בלוג את מדיניות הניתוב הנגזרת בזמן האתחול. כל שגיאת ולידציה (אינדקסים חסרים, aliases כפולים, מזהי dataspace לא חוקיים) מוצגת לפני תחילת gossip.

## 4. אימות מצב ה-governance של lane

לאחר שה-node עולה, השתמשו ב-CLI helper כדי לוודא שה-default lane sealed (manifest נטען) ומוכן לתעבורה. תצוגת הסיכום מדפיסה שורה אחת לכל lane:

```bash
iroha_cli app nexus lane-report --summary
```

Example output:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```

אם ה-default lane מציג `sealed`, עקבו אחרי runbook ה-governance של lanes לפני מתן תעבורה חיצונית. הדגל `--fail-on-sealed` שימושי ל-CI.

## 5. בדיקת payload-ים של סטטוס Torii

תשובת `/status` חושפת גם את מדיניות הניתוב וגם snapshot של ה-scheduler לכל lane. השתמשו ב-`curl`/`jq` כדי לאשר את ברירות המחדל שהוגדרו ולבדוק שה-fallback lane מפיק טלמטריה:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Sample output:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

כדי לבדוק מונים חיים של ה-scheduler עבור lane `0`:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

זה מאשר ש-snapshot ה-TEU, מטא-דאטה של alias ודגלי manifest תואמים את הקונפיגורציה. אותו payload משמש את לוחות Grafana עבור dashboard ה-lane-ingest.

## 6. אימות ברירות המחדל בצד הלקוח

- **Rust/CLI.** `iroha_cli` ו-crate הלקוח של Rust משמיטים את השדה `lane_id` כאשר לא מעבירים `--lane-id` / `LaneSelector`. לכן ה-queue router נופל ל-`default_lane`. השתמשו בדגלים מפורשים `--lane-id`/`--dataspace-id` רק כאשר מכוונים ל-lane שאינו ברירת מחדל.
- **JS/Swift/Android.** הגרסאות האחרונות של ה-SDK מתייחסות ל-`laneId`/`lane_id` כאופציונליות ונופלות לערך שמפורסם ב-`/status`. שמרו על מדיניות הניתוב מסונכרנת בין staging ל-production כדי שאפליקציות מובייל לא יזדקקו לשינויים דחופים.
- **Pipeline/SSE tests.** מסנני אירועי טרנזקציות מקבלים predicates של `tx_lane_id == <u32>` (ראו `docs/source/pipeline.md`). הירשמו ל-`/v1/pipeline/events/transactions` עם המסנן כדי להוכיח שכתיבות שנשלחו ללא lane מפורש מגיעות תחת ה-fallback lane id.

## 7. תצפיתיות וחיבורי governance

- `/status` מפרסם גם `nexus_lane_governance_sealed_total` ו-`nexus_lane_governance_sealed_aliases` כדי ש-Alertmanager יוכל להתריע כאשר lane מאבד manifest. שמרו על ההתראות פעילות גם ב-devnets.
- מפת הטלמטריה של ה-scheduler ולוח ה-governance של lanes (`dashboards/grafana/nexus_lanes.json`) מצפים לשדות alias/slug מהקטלוג. אם אתם משנים alias, תגיות מחדש את תיקיות Kura המתאימות כדי שהמבקרים ישמרו על מסלולים דטרמיניסטיים (מעקב תחת NX-1).
- אישורי הפרלמנט עבור default lanes צריכים לכלול תוכנית rollback. רשמו את hash ה-manifest והעדויות ל-governance לצד ה-quickstart הזה ב-runbook האופרטורי שלכם כדי שסבבים עתידיים לא ינחשו את המצב הנדרש.

