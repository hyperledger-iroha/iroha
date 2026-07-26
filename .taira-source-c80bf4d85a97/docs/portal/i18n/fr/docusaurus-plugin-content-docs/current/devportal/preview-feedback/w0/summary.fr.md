---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w0-summary
titre : Digest des retours mi-parcours W0
sidebar_label : Retours W0 (mi-parcours)
description : Points de contrôle, constats et actions de mi-parcours pour la vague de prévisualisation des mainteneurs core.
---

| Élément | Détails |
| --- | --- |
| Vague | W0 - Noyau des responsables |
| Date du résumé | 2025-03-27 |
| Fenêtre de revue | 2025-03-25 -> 2025-04-08 |
| Participants | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilité-01 |
| Étiquette d'artefact | `preview-2025-03-24` |

## Points saillants

1. **Workflow de checksum** - Tous les reviewers ont confirmé que `scripts/preview_verify.sh`
   a reussi contre le couple descripteur/archive partage. Aucune dérogation manuelle requise.
2. **Retours de navigation** - Deux problèmes mineurs d'ordre du sidebar ont été signalés
   (`docs-preview/w0 #1-#2`). Les deux sont routes vers Docs/DevRel et ne bloquent pas la
   vague.
3. **Parite des runbooks SoraFS** - sorafs-ops-01 a demande des liens croisés plus clairs
   entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. Numéro de suivi ouvert;
   à traiter avant W1.
4. **Revue de télémétrie** - observabilité-01 a confirmé que `docs.preview.integrity`,
   `TryItProxyErrors` et les logs du proxy Try-it sont restés au vert; aucune alerte n'a
   ete declenchee.

## Actions| ID | Descriptif | Responsable | Statuts |
| --- | --- | --- | --- |
| W0-A1 | Réordonner les entrées du sidebar du devportal pour mettre en avant les docs pour reviewers (groupes `preview-invite-*`). | Docs-core-01 | Termine - la barre latérale liste maintenant les docs reviewers de facon contigue (`docs/portal/sidebars.js`). |
| W0-A2 | Ajouter un lien croise explicite entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Termine - chaque runbook pointe désormais vers l'autre pour que les opérateurs voient les deux guides pendant les déploiements. |
| W0-A3 | Partager des instantanés de télémétrie + bundle de requêtes avec le tracker de gouvernance. | Observabilité-01 | Termine - le bundle attache un `DOCS-SORA-Preview-W0`. |

## Reprise de sortie (2025-04-08)

- Les cinq reviewers ont confirmé la fin, purge les builds locaux et quitte la fenêtre de
  aperçu ; les révocations d'accès sont enregistrées dans `DOCS-SORA-Preview-W0`.
- Aucun incident ni alerte pendant la vague ; les tableaux de bord de télémétrie sont restes verts
  pendentif toute la période.
- Les actions de navigation + liens croisés (W0-A1/A2) sont mises en œuvre et reflétées dans
  les documents ci-dessus; la preuve télémétrique (W0-A3) est attachée au tracker.
- Bundle de preuve archive : captures d'écran de télémétrie, accuses d'invitation et ce digest
  sont couchés depuis l'issue du tracker.

## Prochaines étapes- Implémenter les actions W0 avant d'ouvrir W1.
- Obtenir l'approbation légale et un slot de staging pour le proxy, puis suivre les étapes de
  preflight de la vague partenaires détaillée dans le [preview invite flow](../../preview-invite-flow.md).

_Ce digest est lie depuis le [preview invitation tracker](../../preview-invite-tracker.md) pour
garder le roadmap DOCS-SORA traçable._