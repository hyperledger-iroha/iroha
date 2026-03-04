---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-summary
titre : Résumé des commentaires et état W2
sidebar_label : CV W2
description : Résumé en vivo pour la prévisualisation communautaire (W2).
---

| Article | Détails |
| --- | --- |
| Ola | W2 - Réviseurs communautaires |
| Ventana d'invitation | 2025-06-15 -> 2025-06-29 |
| Étiquette d'artefact | `preview-2025-06-15` |
| Problème du tracker | `DOCS-SORA-Preview-W2` |
| Participants | comm-vol-01 ... comm-vol-08 |

## Descados

1. **Gouvernement et outillage** - La politique d'admission communautaire a été approuvée à l'unanimité le 2025-05-20 ; le modèle de sollicitude actualisé avec des champs de motivation/zone horaire vive en `docs/examples/docs_preview_request_template.md`.
2. **Preflight preflight** - Le changement du proxy Try it `OPS-TRYIT-188` est exécuté le 2025-06-09, les tableaux de bord de Grafana capturés et les sorties du descripteur/somme de contrôle/sonde de `preview-2025-06-15` archivados bas `artifacts/docs_preview/W2/`.
3. **Ola des invitations** - D'autres évaluateurs communautaires ont été invités le 2025-06-15, avec des remerciements enregistrés dans le tableau des invitations du tracker ; toutes les vérifications de la somme de contrôle doivent être effectuées avant la navigation.
4. **Commentaires** - `docs-preview/w2 #1` (texte de l'info-bulle) et `#2` (ordre de localisation de la barre latérale) sont enregistrés le 2025-06-18 et résolus pour le 2025-06-21 (Docs-core-04/05) ; aucun incident hubo pendant la ola.

## Actions| ID | Description | Responsable | État |
| --- | --- | --- | --- |
| W2-A1 | Atender `docs-preview/w2 #1` (formulation de l'info-bulle). | Docs-core-04 | Terminé 2025-06-21 |
| W2-A2 | Atender `docs-preview/w2 #2` (barre latérale de localisation). | Docs-core-05 | Terminé 2025-06-21 |
| W2-A3 | Archivage des preuves de sortie + mise à jour de la feuille de route/du statut. | Responsable Docs/DevRel | Terminé 2025-06-29 |

## Résumé de la sortie (2025-06-29)

- Les autres critiques communautaires confirment la finalisation et récupèrent l'accès à l'aperçu ; remerciements enregistrés dans le journal des invitaciones del tracker.
- Les instantanés finaux de télémétrie (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) se mantuvieron verdes ; journaux et transcriptions du proxy Essayez-le en complément du `DOCS-SORA-Preview-W2`.
- Bundle de preuves (descripteur, journal de somme de contrôle, sortie de sonde, rapport de lien, captures d'écran de Grafana, accusés de réception d'invitation) archivé sous `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Le journal des points de contrôle W2 du tracker est actualisé jusqu'au cercle, garantissant que la feuille de route conserve un registre vérifiable avant de lancer la planification de W3.