---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-summary
titre : Reprendre feedback et sortie W1
sidebar_label : Reprendre W1
description : Constats, actions et preuves de sortie pour la vague de prévisualisation partenaires/intégrateurs Torii.
---

| Élément | Détails |
| --- | --- |
| Vague | W1 - Partenaires et intégrateurs Torii |
| Fenêtre d'invitation | 2025-04-12 -> 2025-04-26 |
| Étiquette d'artefact | `preview-2025-04-12` |
| Suivi des problèmes | `DOCS-SORA-Preview-W1` |
| Participants | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Points saillants

1. **Workflow checksum** - Tous les réviseurs ont vérifié le descripteur/archive via `scripts/preview_verify.sh` ; les logs ont été stockés avec les accusés d'invitation.
2. **Télémétrie** - Les tableaux de bord `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` sont restes verts pendant toute la vague; aucun incident ni page d'alerte.
3. **Feedback docs (`docs-preview/w1`)** - Deux lentes mineures ont été signalées :
   - `docs-preview/w1 #1` : clarifier la formulation de navigation dans la section Try it (resolu).
   - `docs-preview/w1 #2` : mettre a jour la capture Essayez-le (resolu).
4. **Parite runbook** - Les opérateurs SoraFS ont confirmé que les nouveaux cross-links entre `orchestrator-ops` et `multi-source-rollout` ont traité leurs points W0.

## Actions| ID | Descriptif | Responsable | Statuts |
| --- | --- | --- | --- |
| W1-A1 | Mettre à jour la formulation de navigation Try it selon `docs-preview/w1 #1`. | Docs-core-02 | Terminer (2025-04-18). |
| W1-A2 | Rafraichir la capture Essayez-le selon `docs-preview/w1 #2`. | Docs-core-03 | Terminer (2025-04-19). |
| W1-A3 | Resumer les constats partenaires et la preuve télémétrie dans roadmap/status. | Responsable Docs/DevRel | Termine (voir tracker + status.md). |

## Reprise de sortie (2025-04-26)

- Les huit évaluateurs ont confirmé la fin pendant les heures de bureau finales, purgent les artefacts locaux et leurs accès ont été révoqués.
- La télémétrie est restée verte jusqu'à la sortie; les instantanés finaux attachent un `DOCS-SORA-Preview-W1`.
- Le journal d'invitations à ete mis a jour avec les accusés de sortie ; le tracker a marque W1 comme termine et ajoute les points de contrôle.
- Bundle de preuve (descripteur, journal de somme de contrôle, sortie de sonde, transcript du proxy Try it, captures d'écran de télémétrie, résumé de feedback) archive sous `artifacts/docs_preview/W1/`.

## Prochaines étapes

- Préparateur du plan d'admission communautaire W2 (approbation gouvernance + ajustements du template de demande).
- Rafraichir le tag d'artefact preview pour la vague W2 et relancer le script de preflight une fois les dates finalisées.
- Porter les constats applicables de W1 dans roadmap/status pour que la vague communautaire ait les dernières indications.