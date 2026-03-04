---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-plan
titre : Plan de prise en charge communautaire W2
sidebar_label : Plan W2
description : Admission, approbations et liste de contrôle de preuves pour la cohorte de preview comunitaria.
---

| Article | Détails |
| --- | --- |
| Ola | W2 - Réviseurs communautaires |
| Vente objet | T3 2025 semaine 1 (tentative) |
| Étiquette d'artefact (plané) | `preview-2025-06-15` |
| Problème du tracker | `DOCS-SORA-Preview-W2` |

## Objets

1. Définir les critères d'admission communautaire et les flux de vérification.
2. Obtenir l'approbation de gouvernement pour le roster proposé et l'addendum d'utilisation acceptable.
3. Actualisez l'artefact d'aperçu vérifié par la somme de contrôle et le bundle de télémétrie pour la nouvelle fenêtre.
4. Préparez le proxy Essayez-le et les tableaux de bord avant l'envoi des invitations.

## Desglose de tareas| ID | Tarée | Responsable | Fecha limite | État | Notes |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Rédaction de critères d'admission communautaire (éligibilité, nombre maximum d'emplacements, conditions requises pour CoC) et circulaire d'administration | Responsable Docs/DevRel | 2025-05-15 | Terminé | La politique d'admission fusionne en `DOCS-SORA-Preview-W2` et se répond lors de la réunion du Conseil le 20/05/2025. |
| W2-P2 | Actualiser le modèle de sollicitude avec des questions spécifiques à la communauté (motivation, disponibilité, nécessités de localisation) | Docs-core-01 | 2025-05-18 | Terminé | `docs/examples/docs_preview_request_template.md` inclut désormais la section Community, référencée dans le formulaire d'admission. |
| W2-P3 | Asegurar aprobacion de gobernanza para el plan de admission (vote à la réunion + actes enregistrés) | Liaison gouvernance | 2025-05-22 | Terminé | Vote approuvé à l’unanimité le 20/05/2025 ; actas y roll call enlazados en `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programmer le proxy Try it + capture de télémétrie pour la fenêtre W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Terminé | Ticket de changement `OPS-TRYIT-188` approuvé et exécuté le 2025-06-09 02:00-04:00 UTC ; captures d'écran de Grafana archivées avec le ticket. || W2-P5 | Construire/vérifier une nouvelle balise d'artefact de prévisualisation (`preview-2025-06-15`) et archiver des journaux de descripteur/somme de contrôle/sonde | Portail TL | 2025-06-07 | Terminé | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` publié le 10/06/2025 ; sorties guardados bajo `artifacts/docs_preview/W2/preview-2025-06-15/`. |
| W2-P6 | Liste d'invitations communautaires (<=25 évaluateurs, beaucoup augmentés) avec informations de contact approuvées par le gouvernement | Gestionnaire de communauté | 2025-06-10 | Terminé | Première cohorte de 8 évaluateurs communautaires approuvés ; ID de sollicitation `DOCS-SORA-Preview-REQ-C01...C08` enregistrés dans le tracker. |

## Liste de contrôle des preuves

- [x] Registro de aprobacion de gobernanza (notas de reunion + link de voto) adjunto a `DOCS-SORA-Preview-W2`.
- [x] Modèle de demande actualisé engagé sous `docs/examples/`.
- [x] Descripteur `preview-2025-06-15`, journal de la somme de contrôle, sortie de la sonde, rapport de lien et transcription du proxy Essayez-le gardé sous `artifacts/docs_preview/W2/`.
- [x] Captures d'écran de Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturées pour la fenêtre de contrôle en amont W2.
- [x] Tableau de la liste des invitations avec les identifiants des évaluateurs, les tickets de sollicitude et les horodatages d'approbation complétés avant l'envoi (voir section W2 du tracker).

Maintenir ce plan actualisé ; Le tracker est la référence pour la feuille de route DOCS-SORA et est exactement là où il reste avant de recevoir les invitations W2.