---
lang: he
direction: rtl
source: docs/portal/docs/nexus/settlement-faq.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-settlement-faq
כותרת: שאלות נפוצות יישוב
תיאור: תגובות pour les operateurs couvrant le routage התנחלות, la conversion XOR, la telemetrie et les preuves d'audit.
---

Cette page reprend la FAQ interne de settlement (`docs/source/nexus_settlement_faq.md`) pour que les lecteurs du portail puissent consulter les memes indications sans fouiller le mono-repo. אחרת, הערה מפורשת של נתב ההתיישבות תכונה לתשלומים, בדיקות מדדי מעקב והערות SDK מאפשרות אינטגרציה של מטענים Norito.

## נקודות cles

1. **Mappage des lanes** - chaque dataspace declare un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` או `xor_dual_fund`). Consultez le dernier catalog des lanes dans `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversion deterministe** - הנתב ממיר את ההתנחלויות ב-XOR דרך les sources de liquidite approuvees par la governance. Les lanes privees prefinancent des מאגרים XOR; les haircuts ne s'appliquent que lorsque les buffers הנגזרות הורס דה לה פוליטיקה.
3. **Telemetrie** - surveillez `nexus_settlement_latency_seconds`, les compteurs de conversion et les jauges de haircut. לוחות המחוונים האלה נראים ב-`dashboards/grafana/nexus_settlement.json` ו-les התראות ב-`dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuves** - ארכיון הגדרות, יומני נתב, יצוא של טלמטריה ו-rapports de reconciliation pour les audits.
5. **Responsabilites SDK** - Chaque SDK doit exposer des helpers de settlement, des IDs de lane et des encodeurs de payloads Norito pour rester aligne avec le router.

## Flux d'exemple

| סוג דה ליין | Preuves אספן | Ce que cela prouve |
|-----------|------------------------|----------------|
| Privee `xor_hosted_custody` | לוג דו נתב + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Les buffers CBDC debitent un XOR deterministe and les haircuts restent dans la politique. |
| Publique `xor_global` | התחבר לנתב + הפניה DEX/TWAP + מדדי אחזור/המרה | Le chemin de liquidite partage a fixe le prix du transfert sur le TWAP publie avec אפס תספורת. |
| Hybride `xor_dual_fund` | Log du Router montrant la repartition public vs shielded + compteurs de telemetrie | Le mix shielded/public a respect les ratios de governance ו-rescribe le תספורת אפליקציית צ'אק ג'מבה. |

## Besoin de plus de details ?

- שאלות נפוצות מלאות: `docs/source/nexus_settlement_faq.md`
- נתב Spec du Settlement: `docs/source/settlement_router.md`
- Playbook de politique CBDC: `docs/source/cbdc_lane_playbook.md`
- פעולות פנקס הפעלה: [פעולות Nexus](./nexus-operations)