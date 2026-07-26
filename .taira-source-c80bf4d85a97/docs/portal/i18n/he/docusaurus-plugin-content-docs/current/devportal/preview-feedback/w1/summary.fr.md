---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: preview-feedback-w1-summary
כותרת: משוב על קורות חיים et sortie W1
sidebar_label: קורות חיים W1
תיאור: Constats, actions et preuves de sortie pour la vague de preview partenaires/integrators Torii.
---

| אלמנט | פרטים |
| --- | --- |
| מעורפל | W1 - Partenaires et integrates Torii |
| Fenetre d'invitation | 2025-04-12 -> 2025-04-26 |
| Tag d'Artefact | `preview-2025-04-12` |
| מעקב אחר נושאים | `DOCS-SORA-Preview-W1` |
| משתתפים | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## נקודות שייטים

1. **סכום בדיקת זרימת עבודה** - כל הסוקרים מאמתים את התיאור/ארכיון באמצעות `scripts/preview_verify.sh`; les logs ont ete stockes avec les accuses d'invitation.
2. **Telemetrie** - לוחות מחוונים של `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` sont restes verts pendant toute la vague; תקרית aucun ni page d'alerte.
3. **מסמכי משוב (`docs-preview/w1`)** - Deux nits mineurs ont ete signales:
   - `docs-preview/w1 #1`: מבהיר את הניסוח של הניווט בסעיף נסה את זה (resolu).
   - `docs-preview/w1 #2`: mettre a jour la capture נסה את זה (resolu).
4. **Parite runbook** - Les operateurs SoraFS ont confirme que les nouveaux cross-links entre `orchestrator-ops` et `multi-source-rollout` on traite leurs points W0.

## פעולות

| תעודה מזהה | תיאור | אחראי | סטטוט |
| --- | --- | --- | --- |
| W1-A1 | Mettre a jour la formulation de navigation נסה את זה selon `docs-preview/w1 #1`. | Docs-core-02 | טרמין (2025-04-18). |
| W1-A2 | Rafraichir la capture נסה את זה selon `docs-preview/w1 #2`. | Docs-core-03 | טרמין (2025-04-19). |
| W1-A3 | Resumer les constats partenaires et la preuve telemetrie dans map/status. | Docs/DevRel lead | Termine (voir tracker + status.md). |

## קורות חיים (2025-04-26)

- הבודקים של Les Huit ontseg la fin pendant les finals hours office, purge les artefacts locaux et leurs acces on ete revoques.
- La telemetrie est restee verte jusqu'a la sortie; פינוקס מצרף `DOCS-SORA-Preview-W1`.
- Le log d'invitations a ete mis a jour avec les accuses de sortie; ה-tracker ו-Marque W1 comme termine and ajoute les checkpoints.
- Bundle de preuve (מתאר, יומן בדיקה, פלט בדיקה, תמליל של פרוקסי נסה את זה, צילומי מסך של טלמטריה, תקציר משוב) ארכיון sous `artifacts/docs_preview/W1/`.

## Prochaines etapes

- הכנת תוכנית כניסת האיחוד האירופי W2 (ממשל אישור + התאמות של תבנית דרישה).
- Rafraichir le tag d'artefact preview pour la vague W2 and relancer the script de preflight une fois les data finalisees.
- Porter les constats applications de W1 dans map/status pour que la vague communautaire ait les dernieres indications.