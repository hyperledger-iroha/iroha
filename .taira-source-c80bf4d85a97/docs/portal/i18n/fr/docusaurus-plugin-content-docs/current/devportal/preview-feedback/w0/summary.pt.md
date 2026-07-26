---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w0-summary
titre : Resumo de feedback do meio do W0
sidebar_label : Commentaires W0 (meio)
description : Points de contrôle, achados et acoes de meio de onda para a onda de preview de mantenedores core.
---

| Article | Détails |
| --- | --- |
| Onde | W0 - Noyau Mantenedores |
| Données du CV | 2025-03-27 |
| Janela de révision | 2025-03-25 -> 2025-04-08 |
| Participants | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilité-01 |
| Étiquette d'artefato | `preview-2025-03-24` |

## Destaques

1. **Flux de somme de contrôle** - Tous les réviseurs confirment que `scripts/preview_verify.sh`
   vous avez réussi contre le descripteur/archive partagé. Nenhum override manuel foi
   nécessaire.
2. **Feedback de navigation** - Deux problèmes mineurs de commande dans le forum de la barre latérale
   enregistrés (`docs-preview/w0 #1-#2`). Ambos foram encaminhados para Docs/DevRel et nao
   bloque une onda.
3. **Parité des runbooks SoraFS** - sorafs-ops-01 en cliquant sur les liens croisés mais plus clairs entre
   `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. Numéro d'accompagnement ouvert;
   tratar avant W1.
4. **Révision de télémétrie** - observabilité-01 confirme que `docs.preview.integrity`,
   `TryItProxyErrors` Les journaux du système d'exploitation font un proxy Try-it ficaram verdes ; nenhum alerta disparou.

## Ingrédients de cacao| ID | Description | Responsavel | Statut |
| --- | --- | --- | --- |
| W0-A1 | Réorganisez les entrées de la barre latérale du portail de développement pour que les documents soient focalisés sur les réviseurs (groupe `preview-invite-*`). | Docs-core-01 | Concluido - la barre latérale affiche maintenant la liste des documents des réviseurs de forme continue (`docs/portal/sidebars.js`). |
| W0-A2 | Lien supplémentaire croisé explicitement entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Concluido - chaque runbook vient d'être disponible pour l'outre pour que les opérateurs souhaitent utiliser les guides lors des déploiements. |
| W0-A3 | Partagez des instantanés de télémétrie + un ensemble de requêtes avec un tracker de gouvernance. | Observabilité-01 | Concluido - bundle anexado ao `DOCS-SORA-Preview-W0`. |

## Résumé de l'encerclement (2025-04-08)

- Tous les cinq critiques confirment la conclusion, limparam builds local et sairam da janela
  l'aperçu ; en tant que revogacoes de acesso ficaram enregistrés sous `DOCS-SORA-Preview-W0`.
- Nenhum incidente ou alerta ocorreu durante a onda ; les tableaux de bord de télémétrie ficaram
  verts pendant toute la période.
- As acoes de navegacao + links cruzados (W0-A1/A2) sont mis en œuvre et reflètent nos documents
  acima; La preuve de télémétrie (W0-A3) est examinée par le tracker.
- Bundle de preuves archivées : captures d'écran de télémétrie, confirmations de convite et este
  résumé estao linkados aucun problème avec le tracker.

## Passons à proximité- Implémenter les éléments de cacao du W0 avant d'ouvrir W1.
- Obtenir l'approbation légale d'un emplacement de mise en scène pour le mandataire, après avoir suivi les étapes de contrôle en amont
  l'onde des colis décrit le flux d'invitation d'aperçu (../../preview-invite-flow.md).

_Ce résumé est lié à partir du [preview invitation tracker](../../preview-invite-tracker.md) pour
suivre la feuille de route DOCS-SORA rastreavel._