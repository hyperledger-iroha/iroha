---
id: reserve-ledger-digest
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


מדיניות Reserve+Rent (פריט מפת דרכים **SFM-6**) מספקת כעת את עזרי ה-CLI `sorafs reserve`
ואת המתרגם `scripts/telemetry/reserve_ledger_digest.py` כך שריצות אוצר יפיקו העברות
rent/reserve דטרמיניסטיות. עמוד זה משקף את הזרימה המוגדרת ב-
`docs/source/sorafs_reserve_rent_plan.md` ומסביר כיצד לחבר את הזנת ההעברות החדשה אל
Grafana + Alertmanager כדי שמבקרי כלכלה וממשל יוכלו לבחון כל מחזור חיוב.

## זרימה מקצה לקצה

1. **הצעת מחיר + הקרנת ספר**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

   sorafs reserve ledger \
     --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
     --provider-account soraカタカナ... \
     --treasury-account soraカタカナ... \
     --reserve-account soraカタカナ... \
     --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
     --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   עוזר ה-ledger מצרף בלוק `ledger_projection` (rent due, reserve shortfall,
   top-up delta, וערכי underwriting בוליאניים) לצד ISIs מסוג `Transfer` של Norito
   הדרושים להעברת XOR בין חשבונות האוצר והרזרבה.

2. **הפקת הדיג׳סט + פלט Prometheus/NDJSON**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   עוזר הדיג׳סט מנרמל סכומי micro-XOR ל-XOR, רושם אם ההקרנה עומדת ב-underwriting,
   ומפיק את מדדי זרם ההעברות `sorafs_reserve_ledger_transfer_xor` ו-
   `sorafs_reserve_ledger_instruction_total`. כאשר צריך לעבד מספר ledgers (למשל אצווה
   של ספקים), חזרו על זוגות `--ledger`/`--label` והעוזר יכתוב קובץ NDJSON/Prometheus
   יחיד שמכיל כל דיג׳סט כדי שה-dashboards יקלטו את כל המחזור ללא glue ייעודי. קובץ
   `--out-prom` מכוון ל-textfile collector של node-exporter - הניחו את קובץ ה-`.prom`
   בתיקיה המנוטרת של ה-exporter או העלו אותו לדלי הטלמטריה שנצרך על ידי job הדשבורד
   של Reserve - בעוד ש-`--ndjson-out` מזין את אותם payloads לצינורות הנתונים.

3. **פרסום artefacts + ראיות**
   - אחסנו digests תחת `artifacts/sorafs_reserve/ledger/<provider>/` וקשרו את סיכום
     ה-Markdown מתוך דוח הכלכלה השבועי.
   - צרפו את ה-JSON digest ל-burn-down של ה-rent (כדי שמבקרים יוכלו לשחזר את החישוב)
     והכניסו את ה-checksum לחבילת הראיות של הממשל.
   - אם הדיג׳סט מסמן top-up או הפרת underwriting, הפנו למזהי ההתראה
     (`SoraFSReserveLedgerTopUpRequired`, `SoraFSReserveLedgerUnderwritingBreach`) וציינו
     אילו ISIs של העברה יושמו.

## מדדים → dashboards → התראות

| מדד מקור | פאנל Grafana | התראה / חיבור מדיניות | הערות |
|----------|--------------|------------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | “DA Rent Distribution (XOR/hour)” ב-`dashboards/grafana/sorafs_capacity_health.json` | הזינו את digest האוצר השבועי; קפיצות בזרימת הרזרבה מתגלגלות ל-`SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`). |
| `torii_da_rent_gib_months_total` | “Capacity Usage (GiB-months)” (אותו דשבורד) | הצמידו לדיג׳סט ה-ledger כדי להוכיח שהאחסון המחויב תואם להעברות ה-XOR. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + כרטיסי סטטוס ב-`dashboards/grafana/sorafs_reserve_economics.json` | ההתראה `SoraFSReserveLedgerTopUpRequired` מופעלת כאשר `requires_top_up=1`; ההתראה `SoraFSReserveLedgerUnderwritingBreach` מופעלת כאשר `meets_underwriting=0`. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | “Transfers by Kind”, “Latest Transfer Breakdown” וכרטיסי הכיסוי ב-`dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` ו-`SoraFSReserveLedgerTopUpTransferMissing` מתריעים כאשר זרם ההעברות חסר או מאופס למרות שנדרש rent/top-up; כרטיסי הכיסוי נופלים ל-0% באותם מקרים. |

עם סיום מחזור rent, רעננו את צילומי Prometheus/NDJSON, ודאו שלוחות Grafana מזהים
את ה-`label` החדש, וצרפו צילומי מסך + מזהי Alertmanager לחבילת הממשל של ה-rent.
כך מוכח שההקרנה ב-CLI, הטלמטריה וה-artefacts של הממשל נובעים מאותו **זרם העברות**
ומשאיר את לוחות הכלכלה של ה-roadmap מסונכרנים עם אוטומציית Reserve+Rent. כרטיסי
הכיסוי צריכים להציג 100% (או 1.0) וההתראות החדשות אמורות להיעלם לאחר שהעברות rent
ו-top-up של הרזרבה קיימות בתוך הדיג׳סט.
