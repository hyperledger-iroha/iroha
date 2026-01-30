---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/devportal/preview-feedback/w2/plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8c7054cbbdbf6579cdaf36d73d9c74081f60d539842b6a91862af25c8bf1e26f
source_last_modified: "2025-11-14T04:43:19.936889+00:00"
translation_last_reviewed: 2026-01-30
---

| Element | Details |
| --- | --- |
| Vague | W2 - Reviewers communautaires |
| Fenetre cible | Q3 2025 semaine 1 (tentatif) |
| Tag d'artefact (planifie) | `preview-2025-06-15` |
| Issue tracker | `DOCS-SORA-Preview-W2` |

## Objectifs

1. Definir les criteres d'intake communautaire et le workflow de vetting.
2. Obtenir l'approbation governance pour le roster propose et l'addendum d'usage acceptable.
3. Rafraichir l'artefact preview verifie par checksum et le bundle de telemetrie pour la nouvelle fenetre.
4. Preparer le proxy Try it et les dashboards avant l'envoi des invitations.

## Decoupage des taches

| ID | Tache | Responsable | Echeance | Statut | Notes |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Rediger les criteres d'intake communautaire (eligibilite, max slots, exigences CoC) et les diffuser a governance | Docs/DevRel lead | 2025-05-15 | Termine | La politique d'intake a ete mergee dans `DOCS-SORA-Preview-W2` et endossee lors de la reunion du conseil 2025-05-20. |
| W2-P2 | Mettre a jour le template de demande avec des questions communautaires (motivation, disponibilite, besoins de localisation) | Docs-core-01 | 2025-05-18 | Termine | `docs/examples/docs_preview_request_template.md` inclut maintenant la section Community, referencee dans le formulaire intake. |
| W2-P3 | Obtenir l'approbation governance pour le plan intake (vote en reunion + minutes enregistrees) | Governance liaison | 2025-05-22 | Termine | Vote adopte a l'unanimite le 2025-05-20; minutes + roll call lies dans `DOCS-SORA-Preview-W2`. |
| W2-P4 | Planifier le staging du proxy Try it + capture telemetrie pour la fenetre W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Termine | Ticket de changement `OPS-TRYIT-188` approuve et execute 2025-06-09 02:00-04:00 UTC; screenshots Grafana archives avec le ticket. |
| W2-P5 | Construire/verifier le nouveau tag d'artefact preview (`preview-2025-06-15`) et archiver descriptor/checksum/probe logs | Portal TL | 2025-06-07 | Termine | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` execute 2025-06-10; outputs stockes sous `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Assembler le roster d'invitations communautaires (<=25 reviewers, lots echelonn es) avec les contacts approuves par governance | Community manager | 2025-06-10 | Termine | Premiere cohorte de 8 reviewers communautaires approuvee; IDs de requete `DOCS-SORA-Preview-REQ-C01...C08` logges dans le tracker. |

## Checklist de preuve

- [x] Enregistrement d'approbation governance (notes de reunion + lien de vote) attache a `DOCS-SORA-Preview-W2`.
- [x] Template de demande mis a jour commite sous `docs/examples/`.
- [x] Descriptor `preview-2025-06-15`, log checksum, probe output, link report et transcript proxy Try it stockes sous `artifacts/docs_preview/W2/`.
- [x] Screenshots Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) captures pour la fenetre preflight W2.
- [x] Tableau roster d'invitations avec IDs reviewers, tickets de demande et timestamps d'approbation remplis avant l'envoi (voir section W2 du tracker).

Garder ce plan a jour; le tracker le reference pour que le roadmap DOCS-SORA voie exactement ce qu'il reste avant l'envoi des invitations W2.
