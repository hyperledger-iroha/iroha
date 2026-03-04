---
lang: he
direction: rtl
source: docs/source/soradns_ir_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e9ad416067db38bc5a8f43c654b0094cf9ce8872dd4b919108b71afe22cfcc3e
source_last_modified: "2025-11-14T06:05:41.880329+00:00"
translation_last_reviewed: 2026-01-21
---

<div dir="rtl">

<!-- תרגום עברי ל-docs/source/soradns_ir_playbook.md -->

# ספר תגובה לאירועים עבור Resolver ושער SoraDNS

נוהל זה מכסה אירועים שבהם חבילות ה‑resolver סוטות ממדיניות הרג'יסטרי,
הוכחות Signed Tree Head מתיישנות, או שערים מגישים תוכן שכבר אינו תואם
למניפסט GAR. הוא קושר בין ה‑transparency tailer, ה‑CLI לדוח Signed Tree Head,
ולולאות הסלמת ממשל המוזכרות בפריט DG-5 במפת הדרכים.

## זיהוי

1. **התראות Prometheus** – `dashboards/alerts/soradns_transparency_rules.yml`
   מעלה את האזהרה `SoradnsProofAgeExceeded` ואת ההתראות הקריטיות
   `SoradnsCidDriftDetected`. כאשר אחת מהן נדלקת, תפסו את רשומת
   Alertmanager והוסיפו אותה לכרטיס האירוע.
2. **סקירת דוח שקיפות** – הריצו את ה‑CLI של הדוח השבועי עם
   `cargo run -p soradns-resolver --bin soradns_transparency_report -- \
   --log /var/log/soradns/resolver.transparency.log \
   --output artifacts/soradns/transparency_report.md \
   --sth-output artifacts/soradns/signed_tree_heads.json \
   --recent-limit 40`. טבלת "Recent Events" תבליט אזורים שעברו reorg או
   סטו; צרפו את שני הארטיפקטים לכרטיס.
3. **טלמטריית שער** – `docs/source/sorafs_gateway_dns_owner_runbook.md`
   מתאר את בדיקות הטלמטריה של GAR. לכדו את הפלט האחרון של
   `sorafs_gateway-probe` אם האירוע הגיע משער ה‑HTTP.

## הכלה

1. **הקפאת ה‑namehash המושפע** – הגישו את פעולת הרשם המתאימה
   (`RegisterOfflineVerdictRevocation` או הקפאת GAR) לפי
   `docs/source/sns/governance_playbook.md`.
2. **פרסום מחדש של Signed Tree Heads** – העלו את ארטיפקט ה‑JSON שנוצר
   על ידי ה‑CLI של הדוח ל‑bucket ראיות הממשל ושתפו אותו עם מאמתים כדי שיוכלו
   לאשר את ערכי `policy_hash_hex` החדשים.
3. **Rollback לשער** – פעלו לפי נוהל ה‑GAR rollback ב‑
   `docs/source/sorafs_gateway_dns_owner_runbook.md`, וודאו ש‑transparency tailer
   מורץ מחדש לאחר מכן כדי לאשר תאימות.

## דיווח

1. הגישו הערת שקיפות הכוללת:
   - דוח ה‑Markdown וחבילת JSON Signed Tree Head תחת
     `artifacts/soradns/<stamp>/`.
   - צילומי Grafana המציגים בריאות התראות ומגמות גיל הוכחות.
   - פעולת הממשל שבוצעה (הקפאה, advisory, או הפשרה).
2. עדכנו את תקציר השקיפות השבועי המתואר ב‑
   `docs/source/reports/soradns_transparency.md` עם סיכום האירוע ובעלי אחריות
   לתיקון.
3. תעדו את התרגיל או האירוע ב‑`ops/drill-log.md` כאשר מתאים.

## רשימת בדיקה

- [ ] לאשר התראות Prometheus ולתעד צילומי מסך/קישורי לוג.
- [ ] להריץ `run_soradns_transparency_tail.sh` לרענון ארטיפקטי המטריקות.
- [ ] להריץ `soradns_transparency_report` ליצירת חבילות Markdown + JSON.
- [ ] להקפיא/להחזיר rollback לפי הצורך ולתעד מזהי כרטיסי הממשל.
- [ ] לעדכן את הדוח השבועי ואת תקציר השקיפות עם הערות האירוע.

</div>
