---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w0/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w0-summary
כותרת: Digest des retours mi-parcours W0
sidebar_label: Retours W0 (mi-parcours)
תיאור: Points de Control, Constats and Actions de mi-parcours pour la vague de preview des maintenanceers core.
---

| אלמנט | פרטים |
| --- | --- |
| מעורפל | W0 - ליבת תחזוקה |
| Date du digest | 27-03-2025 |
| Fenetre de review | 2025-03-25 -> 2025-04-08 |
| משתתפים | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observability-01 |
| Tag d'Artefact | `preview-2025-03-24` |

## נקודות שייטים

1. **זרימת בדיקה ** - Tous les reviewers ont confirme que `scripts/preview_verify.sh`
   a reussi contre le couple descriptor/partage ארכיון. Aucun עוקף מנואל דרישות.
2. **Retours de Navigation** - Deux problemes mineurs d'ordre du sidebar ont ete signales
   (`docs-preview/w0 #1-#2`). Les deux sont routes vers Docs/DevRel et ne bloquent pas la
   מעורפל.
3. **Parite des runbooks SoraFS** - sorafs-ops-01 a demande des liens croises plus clairs
   entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. Issue de suivi ouverte;
   בוגד אוונג W1.
4. **Revue de telemetrie** - observability-01 a confirme que `docs.preview.integrity`,
   `TryItProxyErrors` et les logs du proxy Try-it sont restes au vert; aucune alerte n'a
   ete declenchee.

## פעולות

| תעודת זהות | תיאור | אחראי | סטטוט |
| --- | --- | --- | --- |
| W0-A1 | Reordonner les entrees du sidebar du devportal pour mettre en avant les docs pour reviewers (`preview-invite-*` regropes). | Docs-core-01 | Termine - רשימת סרגל הצד לתחזוקה les docs reviewers de facon contigue (`docs/portal/sidebars.js`). |
| W0-A2 | Ajouter un lien croise explicite entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Termine - chaque runbook pointe desormais vers l'autre pour que les operateurs voient les deux guides pendant les rollouts. |
| W0-A3 | חלק תמונות מצב של טלמטריה + חבילת בקשות למעקב אחר ממשל. | צפיות-01 | Termine - צרף צרור `DOCS-SORA-Preview-W0`. |

## קורות חיים (2025-04-08)

- מבקרי Les cinq ontiche la fin, purge les builds locaux et quitte la fenetre de
  תצוגה מקדימה; les revocations d'acces sont enregistrees dans `DOCS-SORA-Preview-W0`.
- Aucun incident ni alerte pendant la vague; les dashboards de telemetrie sont restes verts
  תליון toute la periode.
- Les actions de navigation + liens croises (W0-A1/A2) sont implementees et refletees dans
  les docs ci-dessus; la preuve telemetrie (W0-A3) est attachee au tracker.
- ארכיון Bundle de preuve: צילומי מסך של טלמטריה, האשמת ההזמנה וכו'
  sont lies depuis l'issue du tracker.

## Prochaines etapes

- מיישם את הפעולות W0 avant d'ouvrir W1.
- Obtenir l'approbation legale et un slot de staging pour le proxy, puis suivre les etapes de
  preflight de la vague partenaires detaillees dans le [זרימת הזמנת תצוגה מקדימה](../../preview-invite-flow.md).

_Ce digest lie depuis le [תצוגה מקדימה הזמנת tracker](../../preview-invite-tracker.md) pour
garder le roadmap DOCS-SORA tracable._