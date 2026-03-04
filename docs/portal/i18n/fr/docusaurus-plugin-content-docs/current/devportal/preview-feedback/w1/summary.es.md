---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-summary
titre : Resumen de feedback y cierre W1
sidebar_label : CV W1
description : Hallazgos, actions et preuves de cierre pour l'ola de prévisualisation des partenaires et intégrateurs Torii.
---

| Article | Détails |
| --- | --- |
| Ola | W1 - Partenaires et intégrateurs de Torii |
| Ventana d'invitation | 2025-04-12 -> 2025-04-26 |
| Étiquette d'artefact | `preview-2025-04-12` |
| Problème du tracker | `DOCS-SORA-Preview-W1` |
| Participants | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Descados

1. **Flujo de checksum** - Tous les réviseurs vérifient le descripteur/l'archive via `scripts/preview_verify.sh` ; les journaux sont gardés conjointement avec les accusés d'invitation.
2. **Télémétrie** - Les tableaux de bord `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` sont affichés en vert pendant toute la journée ; aucun incident hubo ni pages d'alerte.
3. **Feedback de docs (`docs-preview/w1`)** - Se registraron dos nits menores :
   - `docs-preview/w1 #1` : clarar wording de navegacion dans la section Try it (résultat).
   - `docs-preview/w1 #2` : actualiser la capture d'écran de Try it (résultat).
4. **Parité des runbooks** - Les opérateurs de SoraFS confirment que les nouveaux liens croisés entre `orchestrator-ops` et `multi-source-rollout` résolvent vos préoccupations de W0.

## Actions| ID | Description | Responsable | État |
| --- | --- | --- | --- |
| W1-A1 | Actualiser le libellé de la navigation de Try it segun `docs-preview/w1 #1`. | Docs-core-02 | Terminé (2025-04-18). |
| W1-A2 | Actualiser la capture d'écran de Try it segun `docs-preview/w1 #2`. | Docs-core-03 | Terminé (2025-04-19). |
| W1-A3 | Résumer les hallazgos des partenaires et les preuves de télémétrie sur la feuille de route/le statut. | Responsable Docs/DevRel | Complété (version tracker + status.md). |

## Résumé de carrière (2025-04-26)

- Les autres critiques confirment la finalisation pendant les heures de bureau finales, limpiaron artefactos locales et se revoco su acceso.
- La télémétrie se mantuvo en verde hasta el cierre; instantanés finaux adjuntos a `DOCS-SORA-Preview-W1`.
- Le journal des invitations est mis à jour avec des arguments de sortie ; Le tracker Marco W1 est terminé et il a ajouté les points de contrôle.
- Paquet de preuves (descripteur, journal de somme de contrôle, sortie de sonde, transcription du proxy Essayez-le, captures d'écran de télémétrie, résumé de commentaires) archivé sous `artifacts/docs_preview/W1/`.

## Suivant pasos

- Préparer le plan d'admission communautaire W2 (aprobacion de gobernanza + ajustes de template de solicitud).
- Refrescar el tag de artefacto de preview para la ola W2 et rejecutar el script de preflight lorsque se finalicen fechas.
- Volcar hallazgos aplicables de W1 en roadmap/status para que la ola comunitaria tenga la guia mas recente.