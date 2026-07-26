---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-plan
titre : Plan d'admission communautaire W2
sidebar_label : Plan W2
description : Intake, approbations et checklist de preuve pour la cohorte aperçu communautaire.
---

| Élément | Détails |
| --- | --- |
| Vague | W2 - Réviseurs communautaires |
| Fenêtre cible | T3 2025 semaine 1 (tentatif) |
| Tag d'artefact (planifier) ​​| `preview-2025-06-15` |
| Suivi des problèmes | `DOCS-SORA-Preview-W2` |

## Objectifs

1. Définir les critères d'admission communautaire et le flux de travail de vérification.
2. Obtenir l'approbation de la gouvernance pour le roster proposé et l'addendum d'usage acceptable.
3. Rafraichir l'artefact preview verifie par checksum et le bundle de télémétrie pour la nouvelle fenêtre.
4. Préparer le proxy Try it et les tableaux de bord avant l'envoi des invitations.

## Découpage des taches| ID | Taché | Responsable | Écheance | Statuts | Remarques |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Rediger les critères d'admission communautaire (éligibilité, max slots, exigences CoC) et les diffuseurs de gouvernance | Responsable Docs/DevRel | 2025-05-15 | Terminer | La politique d'admission a été fusionnée dans `DOCS-SORA-Preview-W2` et endossée lors de la réunion du conseil 2025-05-20. |
| W2-P2 | Mettre à jour le modèle de demande avec des questions communautaires (motivation, disponibilité, besoins de localisation) | Docs-core-01 | 2025-05-18 | Terminer | `docs/examples/docs_preview_request_template.md` inclut maintenant la section Communauté, référencée dans le formulaire d'admission. |
| W2-P3 | Obtenir l'approbation de gouvernance pour le plan d'admission (vote en réunion + minutes enregistrées) | Liaison gouvernance | 2025-05-22 | Terminer | Votez pour l'adoption à l'unanimité le 2025-05-20; minutes + appel nominal se situe dans `DOCS-SORA-Preview-W2`. |
| W2-P4 | Planifier le staging du proxy Try it + capture télémétrie pour la fenêtre W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Terminer | Ticket de changement `OPS-TRYIT-188` approuver et exécuter 2025-06-09 02:00-04:00 UTC ; captures d'écran Archives Grafana avec le ticket. || W2-P5 | Construire/vérifier le nouveau tag d'artefact preview (`preview-2025-06-15`) et archiver descriptor/checksum/probe logs | Portail TL | 2025-06-07 | Terminer | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` exécute le 10/06/2025 ; sorties stocks sous `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Assembler le roster d'invitations communautaires (<=25 évaluateurs, beaucoup d'échelons) avec les contacts approuvés par gouvernance | Gestionnaire de communauté | 2025-06-10 | Terminer | Première cohorte de 8 évaluateurs communautaires approuvés; IDs de requête `DOCS-SORA-Preview-REQ-C01...C08` logs dans le tracker. |

## Checklist de preuve

- [x] Enregistrement d'approbation gouvernance (notes de réunion + lien de vote) joindre un `DOCS-SORA-Preview-W2`.
- [x] Modèle de demande mis à jour comité sous `docs/examples/`.
- [x] Descripteur `preview-2025-06-15`, somme de contrôle du journal, sortie de sonde, rapport de lien et proxy de transcription Essayez-le en stock sous `artifacts/docs_preview/W2/`.
- [x] Captures d'écran Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) captures pour la fenêtre de contrôle en amont W2.
- [x] Tableau roster d'invitations avec IDs reviewers, tickets de demande et timestamps d'approbation remplis avant l'envoi (voir section W2 du tracker).

Garder ce plan un jour ; le tracker la référence pour que le roadmap DOCS-SORA voie exactement ce qu'il reste avant l'envoi des invitations W2.