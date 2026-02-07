---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-plan
titre : Plano de admission comunitario W2
sidebar_label : Plano W2
description : Admission, autorisations et liste de contrôle des preuves pour la coordination de l'aperçu communautaire.
---

| Article | Détails |
| --- | --- |
| Onde | W2 - Réviseurs communautaires |
| Janela aussi | T3 2025 semaine 1 (tentative) |
| Étiquette d'artefato (avion) ​​| `preview-2025-06-15` |
| Problème de suivi | `DOCS-SORA-Preview-W2` |

## Objets

1. Définir les critères d'admission communautaire et le flux de travail de vérification.
2. Obtenir l'approbation de la gouvernance pour la liste proposée et l'addendum d'utilisation de l'acitavel.
3. Actualisez l'art de l'aperçu vérifié par la somme de contrôle et le bundle de télémétrie pour une nouvelle année.
4. Préparer le proxy Essayez-le et les tableaux de bord avant d'envoyer les invités.

## Départ des tares| ID | Taréfa | Responsavel | Prazo | Statut | Notes |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Réviser les critères d'admission communautaire (éligibilité, nombre maximum d'emplacements, conditions requises pour le CoC) et la circulaire de gouvernance | Responsable Docs/DevRel | 2025-05-15 | Concluido | La politique d'admission a été fusionnée avec le `DOCS-SORA-Preview-W2` et adoptée lors de la réunion du Conseil le 20/05/2025. |
| W2-P2 | Actualiser un modèle de sollicitation avec des questions communautaires (motivation, disponibilité, nécessités de localisation) | Docs-core-01 | 2025-05-18 | Concluido | `docs/examples/docs_preview_request_template.md` vient d'inclure une communauté secao, référencée dans le formulaire d'admission. |
| W2-P3 | Garantir l'approbation de la gouvernance pour le plan d'admission (vote à la réunion + atas registradas) | Liaison gouvernance | 2025-05-22 | Concluido | Vote approuvé à l’unanimité le 20/05/2025 ; atas et roll call linkados em `DOCS-SORA-Preview-W2`. |
| W2-P4 | Programmer la mise en scène du proxy Essayez-le + capture de télémétrie pour Janela W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | Concluido | Billet de change `OPS-TRYIT-188` approuvé et exécuté le 2025-06-09 02:00-04:00 UTC ; captures d'écran Grafana archivées avec le ticket. |
| W2-P5 | Construire/vérifier une nouvelle balise de l'article d'aperçu (`preview-2025-06-15`) et archiver les journaux de descripteur/somme de contrôle/sonde | Portail TL | 2025-06-07 | Concluido | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` publié le 2025-06-10 ; sorties armazenados em `artifacts/docs_preview/W2/preview-2025-06-15/`. || W2-P6 | Monter la liste des convites communautaires (<=25 évaluateurs, beaucoup augmentés) avec les contacts approuvés par le gouvernement | Gestionnaire de communauté | 2025-06-10 | Concluido | Première cohorte de 8 évaluateurs communautaires approuvés ; ID de requisicao `DOCS-SORA-Preview-REQ-C01...C08` enregistrés sans tracker. |

## Liste de contrôle des preuves

- [x] Registro de aprovacao de gouvernance (notas de reuniao + link de voto) anexado a `DOCS-SORA-Preview-W2`.
- [x] Modèle de sollicitacao atualizado commis sob `docs/examples/`.
- [x] Descripteur `preview-2025-06-15`, journal de somme de contrôle, sortie de sonde, rapport de lien et transcription du proxy Essayez-le armazenados em `artifacts/docs_preview/W2/`.
- [x] Captures d'écran Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) capturées pour le vol en amont de Janela W2.
- [x] Tableau de la liste des invités avec les identifiants des évaluateurs, les tickets de sollicitation et les horodatages d'approbation préalables à l'envoi (voir secao W2 sans tracker).

Mantenha este plano actualizado; Le tracker ou la référence pour que la feuille de route DOCS-SORA voit exactement ce qui précède avant d'envoyer des invitations à W2.