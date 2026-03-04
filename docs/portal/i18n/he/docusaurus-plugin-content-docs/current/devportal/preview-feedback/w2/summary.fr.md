---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w2/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w2-summary
כותרת: משוב על קורות חיים וחוק W2
sidebar_label: קורות חיים W2
תיאור: Digest en direct pour la vague de preview communautaire (W2).
---

| אלמנט | פרטים |
| --- | --- |
| מעורפל | W2 - סוקרים קהילתיים |
| Fenetre d'invitation | 2025-06-15 -> 2025-06-29 |
| Tag d'Artefact | `preview-2025-06-15` |
| גשש נושאים | `DOCS-SORA-Preview-W2` |
| משתתפים | comm-vol-01...comm-vol-08 |

## נקודות שייטים

1. **ממשל וכלים** - La politique d'intake communautaire approuvee a l'unanimite le 2025-05-20; le template de demande mis a jour avec champs motivation/fuseau horaire est dans `docs/examples/docs_preview_request_template.md`.
2. **Preflight et preuves** - Le changement du proxy נסה את זה `OPS-TRYIT-188` לבצע le 2025-06-09, לוחות מחוונים Grafana לכידות, et les outputs descriptor/checksum/probe de `preview-2025-06-15` `artifacts/docs_preview/W2/`.
3. **Vague d'invitations** - Huit reviewers communautaires invites le 2025-06-15, avec accuses enregistres dans la table d'invitation du tracker; tous ont termine la verification checksum avant ניווט.
4. **משוב** - `docs-preview/w2 #1` (ניסוח de tooltip) et `#2` (ordre de sidebar de localisation) ont ete saisis le 2025-06-18 et resolus d'ici 2025-06-204/05-core; תליון תקרית aucun la vague.

## פעולות

| תעודת זהות | תיאור | אחראי | סטטוט |
| --- | --- | --- | --- |
| W2-A1 | Traiter `docs-preview/w2 #1` (ניסוח de tooltip). | Docs-core-04 | טרמין 2025-06-21 |
| W2-A2 | Traiter `docs-preview/w2 #2` (סרגל צד של לוקליזציה). | Docs-core-05 | טרמין 2025-06-21 |
| W2-A3 | Archiver les preuves de sortie + mettre a jour map/סטטוס. | Docs/DevRel lead | טרמין 2025-06-29 |

## קורות חיים (2025-06-29)

- Les huit reviewers communautaires ont confirme la fin et l'acces preview a ete revoque; מאשים נרשמים ב-le log d'invitation du tracker.
- Les snapshots finaux de telemetrie (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) sont restes verts; יומנים ותמלילים של proxy נסה את זה מצרף `DOCS-SORA-Preview-W2`.
- Bundle de preuves (תיאור, יומן בדיקה, פלט בדיקה, דוח קישור, צילומי מסך Grafana, accuses d'invitation) ארכיון sous `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Le log de checkpoints W2 du tracker a ete mis a jour jusqu'a la sortie, garantissant un resignable auditable avant le demarrage de la planification W3.