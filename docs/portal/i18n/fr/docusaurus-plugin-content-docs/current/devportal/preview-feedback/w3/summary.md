---
id: preview-feedback-w3-summary
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w3/summary.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

| Element | Details |
| --- | --- |
| Vague | W3 - Cohortes beta (finance + ops + partenaire SDK + ecosystem advocate) |
| Fenetre d'invitation | 2026-02-18 -> 2026-02-28 |
| Tag d'artefact | `preview-20260218` |
| Issue tracker | `DOCS-SORA-Preview-W3` |
| Participants | finance-beta-01, observability-ops-02, partner-sdk-03, ecosystem-advocate-04 |

## Points saillants

1. **Pipeline de preuves end-to-end.** `npm run preview:wave -- --wave preview-20260218 --invite-start 2026-02-18 --invite-end 2026-02-28 --report-date 2026-03-01 --notes "Finance/observability beta wave"` genere le resume par vague (`artifacts/docs_portal_preview/preview-20260218-summary.json`), le digest (`preview-20260218-digest.md`) et rafraichit `docs/portal/src/data/previewFeedbackSummary.json` pour que les reviewers de governance puissent s'appuyer sur une seule commande.
2. **Couverture telemetrie + governance.** Les quatre reviewers ont confirme l'acces bloque par checksum, soumis leur feedback et ont ete revoques a temps; le digest reference les issues de feedback (`docs-preview/20260218` set + `DOCS-SORA-Preview-20260218`) ainsi que les runs Grafana collectes pendant la vague.
3. **Mise en avant portail.** La table du portail rafraichie affiche maintenant la vague W3 fermee avec des metriques de latence et de taux de reponse, et la nouvelle page de log ci-dessous reproduit la timeline pour les auditeurs qui ne recuperent pas le log JSON brut.

## Actions

| ID | Description | Responsable | Statut |
| --- | --- | --- | --- |
| W3-A1 | Capturer le digest preview et l'attacher au tracker. | Docs/DevRel lead | Termine 2026-02-28 |
| W3-A2 | Repliquer l'evidence invitation/digest dans le portail + roadmap/status. | Docs/DevRel lead | Termine 2026-02-28 |

## Resume de sortie (2026-02-28)

- Invitations envoyees 2026-02-18 avec accuses enregistres quelques minutes plus tard; acces preview revoque 2026-02-28 apres validation finale de telemetrie.
- Digest + resume stockes sous `artifacts/docs_portal_preview/`, avec le log brut ancre par `artifacts/docs_portal_preview/feedback_log.json` pour la relecture.
- Suivis d'issues enregistres sous `docs-preview/20260218` avec le tracker governance `DOCS-SORA-Preview-20260218`; notes CSP/Try it transmises aux owners observability/finance et liees depuis le digest.
- La ligne du tracker mise a jour en Completed et la table feedback du portail reflete la vague fermee, terminant la tache beta restante de DOCS-SORA.
