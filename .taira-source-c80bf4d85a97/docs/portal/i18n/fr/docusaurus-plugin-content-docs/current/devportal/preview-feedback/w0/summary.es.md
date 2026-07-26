---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w0-summary
titre : Résumé des commentaires de la mitad de W0
sidebar_label : Commentaires W0 (mitad)
description: Points de contrôle, hallazgos et actions de mitad de ola para la ola de preview de mantenedores core.
---

| Article | Détails |
| --- | --- |
| Ola | W0 - Noyau Mantenedores |
| Fecha del CV | 2025-03-27 |
| Ventana de révision | 2025-03-25 -> 2025-04-08 |
| Participants | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilité-01 |
| Étiquette d'artefact | `preview-2025-03-24` |

## Descados

1. **Flujo de checksum** - Tous les réviseurs confirment que `scripts/preview_verify.sh`
   tuvo exito contra el par descriptor/archivo compartimento. Non requis
   remplace les manuels.
2. **Feedback de navigation** - Vous enregistrez deux problèmes mineurs dans l'ordre de la barre latérale
   (`docs-preview/w0 #1-#2`). Ambos est attribué à Docs/DevRel et ne bloque pas le
   ola.
3. **Parité des runbooks de SoraFS** - sorafs-ops-01 pidio enlace cruzados mas claros
   entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. Se ouvrir un
   issue de seguimiento; il sera présent avant la W1.
4. **Révision de télémétrie** - observabilité-01 confirme que `docs.preview.integrity`,
   `TryItProxyErrors` et les journaux du proxy Essayez-le en vert ; non
   alertes disparates.

## Actions| ID | Description | Responsable | État |
| --- | --- | --- | --- |
| W0-A1 | Réorganisez les entrées de la barre latérale du portail de développement pour que les documents soient mis en évidence et les réviseurs (`preview-invite-*` agrégés). | Docs-core-01 | Complété - la barre latérale répertorie maintenant les documents des réviseurs de forme continue (`docs/portal/sidebars.js`). |
| W0-A2 | Ajouter le lien croisé explicitement entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Terminé - chaque runbook est maintenant affiché à l'autre pour que les opérateurs puissent apporter des conseils lors des déploiements. |
| W0-A3 | Partagez des instantanés de télémétrie + des paquets de requêtes avec le tracker de gouvernance. | Observabilité-01 | Complété - paquet complémentaire au `DOCS-SORA-Preview-W0`. |

## Résumé de carrière (2025-04-08)

- Les cinq réviseurs confirment la finalisation, limpiaron builds locales et salieron de la
  fenêtre d'aperçu ; les révocations d'accès quedaron enregistrées en `DOCS-SORA-Preview-W0`.
- Aucun incident hubo ni alerte pendant la journée ; les tableaux de bord de télémétrie se mantuvieron
  en vert toute la période.
- Les actions de navigation + liens croisés (W0-A1/A2) sont mises en œuvre et réfléchies
  les documents de arriba ; la preuve de télémétrie (W0-A3) est ajoutée au tracker.
- Paquet de preuves archivées : captures d'écran de télémétrie, accusés d'invitation et este
  reprendre estan enlazados depuis le numéro du tracker.

## Suivant pasos- Implémenter les éléments d'action de W0 avant d'ouvrir W1.
- Obtenir une approbation légale et un emplacement de staging pour le proxy, puis suivre les étapes de
  contrôle en amont de l'ola des partenaires détaillé dans le [flux d'invitation d'aperçu] (../../preview-invite-flow.md).

_Ce résumé est affiché à partir du [traqueur d'invitation d'aperçu](../../preview-invite-tracker.md) pour
maintenir la feuille de route DOCS-SORA tratable._