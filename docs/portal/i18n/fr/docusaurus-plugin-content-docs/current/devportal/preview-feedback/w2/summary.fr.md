---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-summary
titre : CV feedback et statut W2
sidebar_label : Reprendre W2
description : Digest en direct pour la vague de prévisualisation communautaire (W2).
---

| Élément | Détails |
| --- | --- |
| Vague | W2 - Réviseurs communautaires |
| Fenêtre d'invitation | 2025-06-15 -> 2025-06-29 |
| Étiquette d'artefact | `preview-2025-06-15` |
| Suivi des problèmes | `DOCS-SORA-Preview-W2` |
| Participants | comm-vol-01...comm-vol-08 |

## Points saillants

1. **Gouvernance et outillage** - La politique d'admission communautaire approuvée à l'unanimité le 2025-05-20; le modèle de demande mis à jour avec champs motivation/fuseau horaire est dans `docs/examples/docs_preview_request_template.md`.
2. **Preflight et preuves** - Le changement du proxy Try it `OPS-TRYIT-188` exécute le 2025-06-09, les tableaux de bord Grafana captures, et les sorties descriptor/checksum/probe de `preview-2025-06-15` archives sous `artifacts/docs_preview/W2/`.
3. **Vague d'invitations** - Huit reviewers communautaires invite le 2025-06-15, avec accusés enregistrés dans la table d'invitation du tracker; tous ont terminé la somme de contrôle de vérification avant la navigation.
4. **Feedback** - `docs-preview/w2 #1` (wording de tooltip) et `#2` (ordre de sidebar de localisation) ont été saisis le 2025-06-18 et résolu d'ici 2025-06-21 (Docs-core-04/05) ; aucun incident pendant la vague.

## Actions| ID | Descriptif | Responsable | Statuts |
| --- | --- | --- | --- |
| W2-A1 | Traiter `docs-preview/w2 #1` (formulation de l'info-bulle). | Docs-core-04 | Terminer 2025-06-21 |
| W2-A2 | Traiter `docs-preview/w2 #2` (sidebar de localisation). | Docs-core-05 | Terminer 2025-06-21 |
| W2-A3 | Archiver les preuves de sortie + mettre à jour roadmap/status. | Responsable Docs/DevRel | Terminer 2025-06-29 |

## Reprise de sortie (2025-06-29)

- Les huit critiques communautaires ont confirmé la fin et l'accès en avant-première à l'été revoque ; accuse d'enregistrer dans le log d'invitation du tracker.
- Les instantanés finaux de télémétrie (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) sont restes verts ; logs et transcripts du proxy Essayez-le en pièce jointe un `DOCS-SORA-Preview-W2`.
- Bundle de preuves (descripteur, journal de somme de contrôle, sortie de sonde, rapport de lien, captures d'écran Grafana, accusés d'invitation) archive sous `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Le log de checkpoints W2 du tracker a ete mis a jour jusqu'a la sortie, garantissant un enregistrement auditable avant le demarrage de la planification W3.