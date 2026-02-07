---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/priority-snapshot-2025-03.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: priority-snapshot-2025-03
כותרת: Snapshot de prioridades — מרץ 2025 (בטא)
תיאור: Espejo del snapshot de steering de Nexus 2025-03; תלוי ב-ACKs לפני הפרסום הציבורי.
---

> Fuente canonica: `docs/source/sorafs/priority_snapshot_2025-03.md`
>
> אסטרטגיה: **ביטא / esperando ACKs de steering** (רשתות, אחסון, לידים ב-Docs).

## קורות חיים

תמונת מצב של מרזו מנטיין לאס התחלה של מסמכים/רשת תוכן alineadas
con las pistas de entrega de SoraFS (SF-3, SF-6b, SF-9). Una vez que todos los
מוביל אישור אל תמונת מצב en el canal de steering de Nexus, elimina la nota
"ביתא" דה אריבה.

### Hilos de enfoque

1. **תמונת מצב מעגלית de prioridades** - הודאות חוזרות y
   registrarlos en las minutas del Council del 2025-03-05.
2. **Cierre del kickoff de Gateway/DNS** - ensayar el nuevo kit de facilitacion
   (Seccion 6 del runbook) antes del workshop 2025-03-03.
3. **Migracion de runbooks de operadores** - הפורטל `Runbook Index` esta live;
   גלה את כתובת האתר של תצוגה מקדימה של בטא ביטול ההרשמה לכניסה לבודקים.
4. **Hilos de entrega de SoraFS** - alinear el trabajo restante de SF-3/6b/9 con
   תוכנית/מפת דרכים:
   - Worker de ingesta PoR + נקודת קצה de estado en `sorafs-node`.
   - Pulido de bindings CLI/SDK עם שילובים של מתזמר Rust/JS/Swift.
   - Cableado de runtime del coordinador PoR y eventos de GovernanceLog.

עיין בארכיון פואנטה עבור הטאבלה המלאה, רשימת ההפצה
y las entradas de log.