---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Aperçu du tracker des invitations

Ce tracker enregistre chaque vague aperçu du portail docs afin que les propriétaires DOCS-SORA et les relecteurs gouvernance voient quelle cohorte est active, qui a approuve les invitations et quels artefacts restent à traiter. Mettez-le à jour chaque fois que des invitations sont envoyées, révoquées ou rapportées pour que la piste d'audit reste dans le dépôt.

## Statut des vagues| Vague | Cohorte | Numéro de suivi | Approbateur(s) | Statuts | Fenêtre cible | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Mainteneurs Docs + SDK validant la somme de contrôle du flux | `DOCS-SORA-Preview-W0` (outil de suivi GitHub/ops) | Lead Docs/DevRel + Portail TL | Terminer | T2 2025 semaines 1-2 | Invitations envoyées 2025-03-25, télémétrie restante verte, résumé de sortie publique 2025-04-08. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii sous NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + liaison gouvernance | Terminer | T2 2025 semaine 3 | Invitations 2025-04-12 -> 2025-04-26 avec les huit partenaires confirmés ; preuves capturées dans [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) et résumé de sortie dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Communauté** | Liste d'attente communauté triée ( 2025-06-29 avec télémétrie verte tout du long; preuves + constats dans [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes bêta** | Bêta finance/observabilité + SDK partenaire + écosystème défenseur | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + liaison gouvernance | Terminer | T1 2026 semaine 8 | Invitations 18/02/2026 -> 28/02/2026 ; CV + donnees portail generees via la vague `preview-20260218` (voir [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> Note : liez chaque issue du tracker aux tickets de demande preview et archivez-les dans le projet `docs-portal-preview` pour que les approbations restent découvrables.

## Taches actifs (W0)

- Artefacts de preflight rafraichis (exécution GitHub Actions `docs-portal-preview` 2025-03-24, descripteur vérifié via `scripts/preview_verify.sh` avec le tag `preview-2025-03-24`).
- Baselines télémétrie capturées (`docs.preview.integrity`, snapshot des tableaux de bord `TryItProxyErrors` sauvegardés dans l'issue W0).
- Texte de sensibilisation fige avec [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) et tag preview `preview-2025-03-24`.
- Demandes d'entrée enregistrée pour les cinq premiers mainteneurs (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinq premières invitations envoyées 2025-03-25 10:00-10:20 UTC après sept jours consécutifs de télémétrie verte; accuse stockes dans `DOCS-SORA-Preview-W0`.
- Suivi télémétrie + horaires de bureau de l'hôte (check-ins quotidiens jusqu'au 2025-03-31 ; journal des points de contrôle ci-dessous).
- Feedback mi-vague / issues collectées et taguées `docs-preview/w0` (voir [W0 digest](./preview-feedback/w0/summary.md)).
- Resume de vague publication + confirmations de sortie (bundle de sortie date 2025-04-08; voir [W0 digest](./preview-feedback/w0/summary.md)).
- Vague bêta W3 suivie ; futures vagues planifiées selon revue gouvernance.

## Resume de la vague partenaires W1- Approbations juridiques et gouvernance. Les partenaires de l'addendum signent le 05/04/2025 ; approbations chargées dans `DOCS-SORA-Preview-W1`.
- Télémétrie + Essayez la mise en scène. Ticket de changement `OPS-TRYIT-147` exécuté le 2025-04-06 avec snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` archives.
- Artefact de préparation + somme de contrôle. Vérification du bundle `preview-2025-04-12` ; descripteur de journaux/somme de contrôle/stocks de sonde dans `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Roster d'invitations + envoi. Huit demandes partenaires (`DOCS-SORA-Preview-REQ-P01...P08`) approuvees; invitations envoyés 2025-04-12 15:00-15:21 UTC avec accusé par récepteur.
- Retour d'information sur les instruments. Horaires de bureau quotidiens + points de contrôle télémétries enregistrés ; voir [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) pour le résumé.
- Roster final / log de sortie. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) enregistre maintenant les timestamps d'invitation/ack, la télémétrie des preuves, les exports quiz et les pointeurs d'artefacts au 2025-04-26 pour permettre la gouvernance de la relecture.

## Log des invitations - Mainteneurs principaux de W0| Récepteur d'identification | Rôle | Billet de demande | Envoyé sur invitation (UTC) | Sortie attendue (UTC) | Statuts | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Responsable du portail | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Actif | Une somme de contrôle de confirmation de la vérification ; focus nav/barre latérale. |
| sdk-rouille-01 | Responsable du SDK Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Actif | Testez les recettes SDK + quickstarts Norito. |
| sdk-js-01 | Responsable du SDK JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Actif | Valide la console Essayez-le + flux ISO. |
| sorafs-ops-01 | SoraFS liaison opérateur | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Actif | Auditer les runbooks SoraFS + orchestration de la documentation. |
| observabilité-01 | Observabilité TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Actif | Revoit les annexes télémétrie/incidents ; responsable couverture Alertmanager. |

Toutes les invitations référencent le meme artefact `docs-portal-preview` (exécution 2025-03-24, tag `preview-2025-03-24`) et le log de vérification capture dans `DOCS-SORA-Preview-W0`. Tout ajout/pause doit être consigné dans le tableau ci-dessus et l'issue du tracker avant de passer à la vague suivante.

## Journal des points de contrôle - W0| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-03-26 | Revue télémétrie baseline + horaires de bureau | `docs.preview.integrity` + `TryItProxyErrors` sont restes verts ; les heures de bureau ont confirmé la somme de contrôle de vérification terminée. |
| 2025-03-27 | Digest feedback intermédiaire public | Reprendre la capture dans [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); deux issues nav mineures taguees `docs-preview/w0`, aucun incident. |
| 2025-03-31 | Vérifiez la télémétrie fin de semaine | Dernières heures de bureau avant la sortie ; les rélecteurs ont confirmé les taches restantes, aucune alerte. |
| 2025-04-08 | Reprise de sortie + fermeture des invitations | Reviews terminées confirmées, accès temporaire revoque, constats archives dans [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker mis à jour avant W1. |

## Log des invitations - Partenaires W1| Récepteur d'identification | Rôle | Billet de demande | Envoyé sur invitation (UTC) | Sortie attendue (UTC) | Statuts | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Opérateur SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Terminer | Orchestrateur des opérations de rétroaction livre 2025-04-20 ; accusé de réception à 15h05 UTC. |
| sorafs-op-02 | Opérateur SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Terminer | Commentaires rollout logges dans `docs-preview/w1`; accusé de réception à 15h10 UTC. |
| sorafs-op-03 | Opérateur SoraFS (États-Unis) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Terminer | Editer les enregistrements de litiges/listes noires ; accusé de réception à 15h12 UTC. |
| torii-int-01 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Terminer | Procédure pas à pas Essayez-le auth accept ; accusé de réception à 15h14 UTC. |
| torii-int-02 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Terminer | Journaux RPC/OAuth ; accusé de réception à 15h16 UTC. |
| sdk-partenaire-01 | Partenaire SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Terminer | Fusion d'aperçu intégré des commentaires ; accusé de réception à 15h18 UTC. |
| sdk-partenaire-02 | Partenaire SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Terminer | Revue télémétrie/rédaction faite; accusé de réception à 15h22 UTC. || passerelle-ops-01 | Opérateur de passerelle | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Terminer | Journaux de la passerelle DNS du runbook ; accusé de réception à 15h24 UTC. |

## Journal des points de contrôle - W1

| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-04-12 | Envoi invitations + artefacts de vérification | Huit partenaires emails avec descripteur/archive `preview-2025-04-12`; accuse stockes dans le tracker. |
| 2025-04-13 | Base de référence de la Revue télémétrie | `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` verts ; les heures de bureau ont confirmé la somme de contrôle de vérification terminée. |
| 2025-04-18 | Horaires de bureau mi-vagues | `docs.preview.integrity` reste vert; deux lentes docs taggent `docs-preview/w1` (texte de navigation + capture d'écran Essayez-le). |
| 2025-04-22 | Vérifier la télémétrie finale | Proxy + tableaux de bord sains ; Aucune nouvelle issue, notée dans le tracker avant sortie. |
| 2025-04-26 | Reprise de sortie + fermeture invitations | Tous les partenaires ont confirmé la revue, invitations révoquées, preuves archivées dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Récapitulatif de la cohorte bêta W3

- Invitations envoyés 2026-02-18 avec checksum de vérification + accuser le meme jour.
- Feedback collecté sous `docs-preview/20260218` avec issue gouvernance `DOCS-SORA-Preview-20260218` ; digérer + reprendre le genre via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Accès revoque le 28/02/2026 après le contrôle télémétrique final ; tracker + tables portail mises à jour pour marquer W3 termine.## Log des invitations - Communauté W2| Récepteur d'identification | Rôle | Billet de demande | Envoyé sur invitation (UTC) | Sortie attendue (UTC) | Statuts | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Réviseur communautaire (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Terminer | Accusé de réception à 16h06 UTC ; Focus sur le SDK de démarrage rapide ; sortie confirmée 2025-06-29. |
| comm-vol-02 | Réviseur communautaire (Gouvernance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Terminer | Revue gouvernance/SNS terminée; sortie confirmée 2025-06-29. |
| comm-vol-03 | Réviseur communautaire (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Terminer | Procédure pas à pas de commentaires Journal Norito ; accusé de réception le 29/06/2025. |
| comm-vol-04 | Réviseur communautaire (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Terminer | Revue runbooks SoraFS terminée ; accusé de réception le 29/06/2025. |
| comm-vol-05 | Réviseur communautaire (Accessibilité) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Terminer | Notes d'accessibilité/UX partages ; accusé de réception le 29/06/2025. |
| comm-vol-06 | Réviseur communautaire (localisation) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Terminer | Journal de localisation des commentaires ; accusé de réception le 29/06/2025. |
| comm-vol-07 | Réviseur communautaire (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Terminer | Vérifie les documents SDK livres mobiles ; accusé de réception le 29/06/2025. || comm-vol-08 | Réviseur communautaire (Observabilité) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Terminer | Revue annexe observabilité terminée; accusé de réception le 29/06/2025. |

## Journal des points de contrôle - W2

| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-06-15 | Envoi invitations + artefacts de vérification | Descripteur/archive `preview-2025-06-15` partage avec 8 rélecteurs; accuser stockes dans tracker. |
| 2025-06-16 | Base de référence de la Revue télémétrie | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verts ; logs proxy Essayez-le montrer les jetons de la communauté active. |
| 2025-06-18 | Horaires de bureau et problèmes de triage | Deux suggestions (info-bulle de formulation `docs-preview/w2 #1`, localisation de la barre latérale `#2`) - toutes deux responsables de Docs. |
| 2025-06-21 | Vérifier la télémétrie + la documentation des correctifs | Docs un corrige `docs-preview/w2 #1/#2` ; tableaux de bord verts, aucun incident. |
| 2025-06-24 | Horaires de bureau fin de semaine | Les rélecteurs ont confirmé les retours restants ; aucune alerte. |
| 2025-06-29 | Reprise de sortie + fermeture invitations | Accusés de réception enregistrés, accès aperçu revoque, instantanés + archives d'artefacts (voir [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horaires de bureau et problèmes de triage | Deux suggestions de documentation loggées sous `docs-preview/w1`; aucun incident ni alerte. |

## Crochets de reporting- Chaque mercredi, mettre à jour le tableau ci-dessus et l'émission d'invitations actives avec une note courte (invitations envoyées, rélecteurs actifs, incidents).
- Lorsqu'une vague se termine, ajoutez le chemin du CV feedback (ex. `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et le lier depuis `status.md`.
- Si un critère de pause du [preview invitation flow](./preview-invite-flow.md) est declenché, ajoutez les étapes de remédiation ici avant de reprendre les invitations.