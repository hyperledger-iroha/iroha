---
lang: fr
direction: ltr
source: docs/examples/docs_preview_feedback_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b1191d3475126df594d0f9f81d91f8ebbcd002c74a1f4d4176f2f42a59ca885
source_last_modified: "2025-11-19T07:56:11.635822+00:00"
translation_last_reviewed: 2026-01-01
---

# Digest de feedback de preview du portail docs (Modele)

Utilisez ce modele pour resumer une vague de preview pour la governance, les revues de
release ou `status.md`. Copiez le Markdown dans le ticket de suivi, remplacez les
placeholders par des donnees reelles et joignez le resume JSON exporte via
`npm run --prefix docs/portal preview:log -- --summary --summary-json`. Le helper
`preview:digest` (`npm run --prefix docs/portal preview:digest -- --wave <label>`) genere
la section de metriques ci-dessous pour que vous n'ayez qu'a remplir les lignes
highlights/actions/artefacts.

```markdown
## Digest de feedback de la vague preview-<tag> (YYYY-MM-DD)
- Fenetre d'invitation: <start -> end>
- Relecteurs invites: <count> (ouverts: <count>)
- Soumissions de feedback: <count>
- Issues ouverts: <count>
- Dernier timestamp d'evenement: <ISO8601 from summary.json>

| Categorie | Details | Owner / Suivi |
| --- | --- | --- |
| Highlights | <ex., "ISO builder walkthrough landed well"> | <owner + date limite> |
| Findings bloquants | <liste d'IDs d'issue ou liens du tracker> | <owner> |
| Elements mineurs de polissage | <regrouper edits cosmetiques ou copy> | <owner> |
| Anomalies de telemetrie | <lien vers snapshot de dashboard / log de probe> | <owner> |

## Actions
1. <Action + lien + ETA>
2. <Deuxieme action optionnelle>

## Artefacts
- Feedback log: `artifacts/docs_portal_preview/feedback_log.json` (`sha256:<digest>`)
- Resume de vague: `artifacts/docs_portal_preview/preview-<tag>-summary.json`
- Snapshot de dashboard: `<lien ou chemin>`

```

Gardez chaque digest avec le ticket de suivi des invitations afin que les relecteurs et la
governance puissent rejouer la trace de preuves sans fouiller les logs CI.
