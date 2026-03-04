---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Tracker des invitations de prévisualisation

Ce tracker enregistre chaque aperçu du portail de documents pour que les propriétaires de DOCS-SORA et les réviseurs d'administration veillent à ce que leur cohorte soit active, qui demande les invitations et les artefacts qui sont en attente. Actualisez-vous chaque fois que vous êtes envieux, révoquez ou difieran invitaciones para que le rastro de auditoria quide dentro del repositorio.

## État d'Olas| Ola | Cohorte | Numéro suivant | Fournisseur(s) | État | Vente objet | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Les responsables de Docs + SDK valident le flux de somme de contrôle | `DOCS-SORA-Preview-W0` (suivi GitHub/ops) | Lead Docs/DevRel + Portail TL | Terminé | T2 2025 semaines 1-2 | Invitations envoyées le 25/03/2025, télémétrie en cours, reprise de la sortie publiée le 08/04/2025. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii sous NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + enlace de gouvernement | Terminé | T2 2025 semaine 3 | Invitaciones 2025-04-12 -> 2025-04-26 avec les autres partenaires confirmés ; preuves capturées en [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) et le résumé de la sortie en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Communauté** | Liste d'espérance communautaire curée ( 2025-06-29 avec télémétrie verte toute la période ; preuve + hallazgos capturés en [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). || **W3 - Cohortes bêta** | Financements bêta/observabilité + SDK partenaire + défenseur de l'écosystème | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + enlace de gouvernement | Terminé | T1 2026 semaine 8 | Invitations 2026-02-18 -> 2026-02-28 ; CV + données du portail généré via ola `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Remarque : enlaza chaque émission du tracker avec les tickets de sollicitude de prévisualisation et archivage sous le projet `docs-portal-preview` pour que les autorisations soient siendo descubribles.

## Tareas activas (W0)- Artefacts de contrôle en amont actualisés (exécution de GitHub Actions `docs-portal-preview` 2025-03-24, descripteur vérifié via `scripts/preview_verify.sh` à l'aide de la balise `preview-2025-03-24`).
- Baselines de télémétrie capturées (`docs.preview.integrity`, instantané des tableaux de bord `TryItProxyErrors` gardés dans le numéro W0).
- Texte de sensibilisation bloqué à l'aide de [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) avec aperçu de la balise `preview-2025-03-24`.
- Sollicitudes de ingreso registradas para los primeros cinco mainteneurs (billets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Premières cinq invitations envoyées le 2025-03-25 10:00-10:20 UTC après chaque journée consécutive de télémétrie verte ; accuse les gardiens en `DOCS-SORA-Preview-W0`.
- Surveillance de télémétrie + heures de bureau de l'hôte (journaux d'enregistrement jusqu'au 31/03/2025 ; journal des points de contrôle en bas).
- Commentaires de mitad de ola / issues recopilados y etiquetados `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Resumen de ola publicado + confirmaciones de salida de invitaciones (bundle de salida fechado 2025-04-08 ; ver [W0 digest](./preview-feedback/w0/summary.md)).
- Ola bêta W3 suivi; futurs olas programadas segun revision de gobernanza.

## Resumen de ola Partenaires W1- Aprobaciónes legales y de gobernanza. Addendum des partenaires confirmé le 2025-04-05 ; arobaciones subidas a `DOCS-SORA-Preview-W1`.
- Telemetria + Essayez la mise en scène. Ticket de changement `OPS-TRYIT-147` émis le 2025-04-06 avec les instantanés Grafana de `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` archivés.
- Préparation de l'artefact + somme de contrôle. Bundle `preview-2025-04-12` vérifié ; logs de descriptor/checksum/probe gardés en `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Liste d'invitations + envoi. Ocho sollicite les partenaires (`DOCS-SORA-Preview-REQ-P01...P08`) aprobadas ; invitations envoyées le 2025-04-12 15:00-15:21 UTC avec des accusations enregistrées par le réviseur.
- Instrumentation de feedback. Agendas des heures de bureau + points de contrôle de télémétrie enregistrés ; voir [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) pour le résumé.
- Liste finale / log de salida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) enregistrez maintenant les horodatages d'invitation/ack, les preuves de télémétrie, les exportations de quiz et les points d'artefacts le 2025-04-26 pour que l'administration puisse reproduire la même chose.

## Journal des invitations - Mainteneurs principaux de W0| ID du réviseur | Rôle | Ticket de sollicitude | Invitation envoyée (UTC) | Sortie attendue (UTC) | État | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Responsable du portail | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Actif | Confirmation de la vérification de la somme de contrôle ; mis en évidence dans la révision de nav/sidebar. |
| sdk-rouille-01 | Responsable du SDK Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Actif | J'ai essayé de recevoir le SDK + les démarrages rapides de Norito. |
| sdk-js-01 | Responsable du SDK JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Actif | Validando consola Essayez-le + flux ISO. |
| sorafs-ops-01 | SoraFS liaison opérateur | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Actif | Audit des runbooks de SoraFS + documents d'inventaire. |
| observabilité-01 | Observabilité TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Actif | Réviser les annexes de télémétrie/incidents ; responsable de la couverture de Alertmanager. |

Toutes les invitations font référence au même artefact `docs-portal-preview` (éjection du 2025-03-24, balise `preview-2025-03-24`) et au registre de vérification capturé en `DOCS-SORA-Preview-W0`. N'importe quelle hauteur/pause doit être enregistrée tant sur la table antérieure que sur l'émission du tracker avant de procéder à la suivante.

## Journal des points de contrôle - W0| Fécha (UTC) | Activité | Notes |
| --- | --- | --- |
| 2025-03-26 | Révision de base de télémétrie + heures de bureau | `docs.preview.integrity` + `TryItProxyErrors` se mantuvieron verdes; Les heures de bureau confirment que tous les réviseurs complètent la vérification de la somme de contrôle. |
| 2025-03-27 | Digest de feedback intermédiaire publié | Résumé capturé en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); deux problèmes mineurs de navigation enregistrés comme `docs-preview/w0`, sans incidents signalés. |
| 2025-03-31 | Chèque de télémétrie de la dernière semaine | Horaires de bureau Ultimas avant la sortie ; les revisores confirmaron tareas restantes en curso, sin alertas. |
| 2025-04-08 | CV + certificats d'invitation | Avis completadas confirméadas, acceso temporal revocado, hallazgos archivados en [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker actualisé avant la préparation de W1. |

## Journal des invitations - Partenaires W1| ID du réviseur | Rôle | Ticket de sollicitude | Invitation envoyée (UTC) | Sortie attendue (UTC) | État | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Opérateur SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Terminé | Entrego feedback des opérations de l’orchestre 2025-04-20 ; Arrivée à 15h05 UTC. |
| sorafs-op-02 | Opérateur SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Terminé | Registre des commentaires sur le déploiement en `docs-preview/w1` ; accusé de réception à 15h10 UTC. |
| sorafs-op-03 | Opérateur SoraFS (États-Unis) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Terminé | Éditions de litige/liste noire enregistrées ; accusé de réception à 15h12 UTC. |
| torii-int-01 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Terminé | Procédure pas à pas de Try it auth aceptado ; accusé de réception à 15h14 UTC. |
| torii-int-02 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Terminé | Commentaires sur les enregistrements RPC/OAuth ; accusé de réception à 15h16 UTC. |
| sdk-partenaire-01 | Partenaire SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Terminé | Commentaires sur l'intégrité de l'aperçu fusionné ; accusé de réception à 15h18 UTC. |
| sdk-partenaire-02 | Partenaire SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Terminé | Révision de télémétrie/rédaction hecha; accusé de réception à 15h22 UTC. || passerelle-ops-01 | Opérateur de passerelle | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Terminé | Commentaires sur le runbook des passerelles DNS enregistrées ; accusé de réception à 15h24 UTC. |

## Journal des points de contrôle - W1

| Fécha (UTC) | Activité | Notes |
| --- | --- | --- |
| 2025-04-12 | Envoi d'invitations + vérification des artefacts | Les partenaires d'Ocho reçoivent un e-mail avec le descripteur/archive `preview-2025-04-12` ; accuse les registrados en el tracker. |
| 2025-04-13 | Révision de la ligne de base de la télémétrie | `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` en vert ; confirmation des heures de bureau, vérification de la somme de contrôle complète. |
| 2025-04-18 | Heures de bureau de mitad de ola | `docs.preview.integrity` se mantuvo verde; dos nits de docs registrados como `docs-preview/w1` (texte de navigation + capture d'écran de Try it). |
| 2025-04-22 | Chèque final de télémétrie | Proxy + tableaux de bord saludables ; sin issues nuevas, annotado en el tracker antes de salida. |
| 2025-04-26 | CV + certificats d'invitation | Tous les partenaires confirment la révision, les invitations révoquées, les preuves archivées en [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Récapitulatif de la cohorte bêta W3- Invitaciones enviadas 2026-02-18 con verificación de checksum + accuses registrados el mismo dia.
- Commentaires copiés sous `docs-preview/20260218` avec le problème de gouvernement `DOCS-SORA-Preview-20260218` ; digest + reprise des générations via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Accès annulé le 2026-02-28 après le chèque final de télémétrie ; tracker + tableaux du portail actualisés pour marquer W3 comme complet.

## Journal des invitations - Communauté W2| ID du réviseur | Rôle | Ticket de sollicitude | Invitation envoyée (UTC) | Sortie attendue (UTC) | État | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Réviseur communautaire (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Terminé | Accusé de réception à 16h06 UTC ; mis à jour les démarrages rapides du SDK ; sortie confirmée le 2025-06-29. |
| comm-vol-02 | Réviseur communautaire (Gouvernance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Terminé | Révision de gobernanza/SNS hecha; sortie confirmée le 2025-06-29. |
| comm-vol-03 | Réviseur communautaire (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Terminé | Commentaires sur la procédure pas à pas Norito enregistrée ; accusé de réception le 29/06/2025. |
| comm-vol-04 | Réviseur communautaire (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Terminé | Révision des runbooks SoraFS hecha ; accusé de réception le 29/06/2025. |
| comm-vol-05 | Réviseur communautaire (Accessibilité) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Terminé | Notes d'accessibilité/UX compartimentées ; accusé de réception le 29/06/2025. |
| comm-vol-06 | Réviseur communautaire (localisation) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Terminé | Commentaires de localisation enregistrés ; accusé de réception le 29/06/2025. || comm-vol-07 | Réviseur communautaire (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Terminé | Vérifie les documents du SDK mobile stockés ; accusé de réception le 29/06/2025. |
| comm-vol-08 | Réviseur communautaire (Observabilité) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Terminé | Révision de l'annexe de l'observabilité hecha ; accusé de réception le 29/06/2025. |

## Journal des points de contrôle - W2| Fécha (UTC) | Activité | Notes |
| --- | --- | --- |
| 2025-06-15 | Envoi d'invitations + vérification des artefacts | Descripteur/archive `preview-2025-06-15` compartimenté avec 8 réviseurs ; accuse les guardados en tracker. |
| 2025-06-16 | Révision de la ligne de base de la télémétrie | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` en vert ; logs del proxy Essayez-le avec les jetons communautaires actifs. |
| 2025-06-18 | Horaires de bureau et tri des problèmes | Deux suggestions (formulation `docs-preview/w2 #1` de l'info-bulle, barre latérale `#2` de localisation) - sont attribuées à Docs. |
| 2025-06-21 | Chèque de télémétrie + correctifs de documents | Docs résolution `docs-preview/w2 #1/#2` ; tableaux de bord verts, sans incidents. |
| 2025-06-24 | Horaires de bureau de la dernière semaine | Revisores confirmaron envios pendientes ; aucune alerte n'est dispararone. |
| 2025-06-29 | CV + certificats d'invitation | Accusés de réception enregistrés, accès à l'aperçu retiré, instantanés + artefacts archivés (version [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horaires de bureau et tri des problèmes | Deux suggestions de documentation enregistrées sous `docs-preview/w1` ; sin incidents ni alertas. |

## Crochets de rapport- Chaque fois, actualisez le tableau du tracker et l'émission des invitations activées avec une note courte de l'état (invitations envoyées, réviseurs actifs, incidents).
- Lorsque vous avez une lettre, ajoutez l'itinéraire du résumé des commentaires (par exemple, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et lancez-le à partir de `status.md`.
- Si vous activez les critères de pause de [flux d'invitation d'aperçu] (./preview-invite-flow.md), ajoutez les étapes de correction ici avant de recevoir de nouvelles invitations.