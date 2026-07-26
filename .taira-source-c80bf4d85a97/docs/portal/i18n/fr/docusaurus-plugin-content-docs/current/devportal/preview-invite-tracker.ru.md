---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Aperçu de l'installation du trek

Ce fournisseur de logiciels de recherche a accès à un portail de prévisualisation des documents, qui comprend les documents DOCS-SORA et les rapports sur la gouvernance et les activités. bien sûr, il s'agit d'une création complète et de nombreux objets d'art qui sont en train de travailler. Veuillez l'indiquer lors de l'ouverture, en ouvrant ou en effectuant une opération, pour que l'auditoire soit placé dans le dépôt.

## Statut du vol| Volna | Kogorta | Numéro trekera | Approbateur(s) | Statut | Целевое окно | Première |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Mainteneurs Docs + SDK, validation du flux de somme de contrôle | `DOCS-SORA-Preview-W0` (suivi GitHub/ops) | Lead Docs/DevRel + Portail TL | Завершено | T2 2025 pas 1-2 | L'opération s'est déroulée le 25/03/2025, la télémétrie s'est déroulée le 08/04/2025 et le résumé a été publié. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii selon NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + liaison gouvernance | Завершено | T2 2025 pas 3 | Mise en service du 12/04/2025 au 26/04/2025, tous vos partenaires seront informés ; preuves dans [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) et résumé dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Communauté** | Liste d'attente de la communauté des conservateurs ( 2025-06-29, телеметрия зеленая весь период; preuves + résultats в [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes bêta** | Finance/observabilité bêta + partenaire SDK + défenseur de l'écosystème | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + liaison gouvernance | Завершено | T1 2026 pas le 8 | Mise en service du 18/02/2026 -> 28/02/2026 ; résumé + données du portail через волну `preview-20260218` (см. [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |> Exemple : Choisissez le voyageur de numéros avec les demandes de prévisualisation des billets et archivez-les dans le projet `docs-portal-preview` pour obtenir les approbations доступными.

## Listes actives (W0)

- Éléments de contrôle en amont publiés (disponibles sur GitHub Actions `docs-portal-preview` 2025-03-24, descripteur prouvé par `scripts/preview_verify.sh` avec `preview-2025-03-24`).
- Paramètres télémétriques de base (`docs.preview.integrity`, instantané `TryItProxyErrors` affiché dans le numéro W0).
- Le texte de sensibilisation concerne [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) avec la balise d'aperçu `preview-2025-03-24`.
- Записаны admission запросы для первых пяти mainteneurs (billets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Первые пять приглашений отправлены 2025-03-25 10:00-10:20 UTC после семи дней зеленой телеметрии; remerciements сохранены в `DOCS-SORA-Preview-W0`.
- Surveillance télémétrique + heures de bureau pour l'hôtel (enregistrements effectués le 31/03/2025 ; journal du point de contrôle ici).
- Собраны commentaires / problèmes à mi-parcours et отмечены `docs-preview/w0` (avec [W0 digest](./preview-feedback/w0/summary.md)).
- Résumé publié волны + подтверждения выхода (sortie bundle 2025-04-08 ; см. [W0 digest](./preview-feedback/w0/summary.md)).
- Sortie de la vague bêta W3 ; Le bureau planifiera après l'examen de la gouvernance.

## Резюме волны Partenaires W1- Approbations juridiques et de gouvernance. Addendum partenaires подписан 2025-04-05 ; approbations загружены в `DOCS-SORA-Preview-W1`.
- Télémétrie + Essayez-le par mise en scène. Changer le ticket `OPS-TRYIT-147` depuis le 06/04/2025 avec les instantanés Grafana `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals`.
- Artefact + préparation de la somme de contrôle. Bundle `preview-2025-04-12` testé ; le descripteur/somme de contrôle/sonde des journaux est utilisé dans `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Liste d'invitations + répartition. Все восемь demandes de partenaires (`DOCS-SORA-Preview-REQ-P01...P08`) одобрены ; приглашения отправлены 2025-04-12 15:00-15:21 UTC, remerciements зафиксированы по ревьюеру.
- Instrumentation de rétroaction. Ежедневные heures de bureau + points de contrôle télémétriques ; см. [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md).
- Liste finale/journal de sortie. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) Vous aurez besoin d'inviter/reconnaître les horodatages, les preuves de télémétrie, les exportations de quiz et les pointeurs sur les articles du 2025-04-26, dont la gouvernance peut vous aider.

## Journal d'invitation - Mainteneurs principaux de W0| ID du réviseur | Rôle | Demander un billet | Invitation envoyée (UTC) | Sortie prévue (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Responsable du portail | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Actif | Подтвердил la somme de contrôle de vérification ; Focus sur la revue de navigation/barre latérale. |
| sdk-rouille-01 | Responsable du SDK Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Actif | Testez les recettes du SDK + les démarrages rapides Norito. |
| sdk-js-01 | Responsable du SDK JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Actif | Valider Essayez-le console + flux ISO. |
| sorafs-ops-01 | SoraFS liaison opérateur | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Actif | Auditer les runbooks SoraFS + les documents d'orchestration. |
| observabilité-01 | Observabilité TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Actif | Ревьюит les annexes de télémétrie/incident ; отвечает за Alertmanager couverture. |

Vous avez installé la recherche sur l'artefact `docs-portal-preview` (exécuté le 24/03/2025, balise `preview-2025-03-24`) et les preuves de transcription dans `DOCS-SORA-Preview-W0`. Les travaux/pause de lubrifiant doivent être corrigés et dans votre tableau, et dans le numéro de voyage avant le début de votre vol.

## Journal des points de contrôle - W0| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-03-26 | Examen de la télémétrie de base + heures de bureau | `docs.preview.integrity` + `TryItProxyErrors` оставались зелеными; les heures de bureau подтвердили завершение vérification de la somme de contrôle. |
| 2025-03-27 | Publication d'un résumé des commentaires à mi-parcours | Résumé сохранен в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); Il y a des problèmes mineurs de navigation liés à `docs-preview/w0`, mais il n'y a pas d'incidents. |
| 2025-03-31 | Vérification ponctuelle de télémétrie de la dernière semaine | Après les heures de bureau avant votre arrivée ; Les notifications des alertes ne sont pas disponibles. |
| 2025-04-08 | Résumé de sortie + fermetures d'invitations | Подтверждены завершенные обзоры, временный доступ отозван, conclusions archivированы в [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker обновлен перед W1. |

## Journal d'invitation - Partenaires W1| ID du réviseur | Rôle | Demander un billet | Invitation envoyée (UTC) | Sortie prévue (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Opérateur SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Terminé | Commentaires sur les opérations d'Orchestrator livrés le 20/04/2025 ; accusé de réception à 15h05 UTC. |
| sorafs-op-02 | Opérateur SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Terminé | Commentaires de déploiement enregistrés dans `docs-preview/w1` ; accusé de réception à 15h10 UTC. |
| sorafs-op-03 | Opérateur SoraFS (États-Unis) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Terminé | Modifications de litige/liste noire déposées ; accusé de réception à 15h12 UTC. |
| torii-int-01 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Terminé | Essayez-le, procédure pas à pas d'authentification acceptée ; accusé de réception à 15h14 UTC. |
| torii-int-02 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Terminé | Commentaires de documents RPC/OAuth enregistrés ; accusé de réception à 15h16 UTC. |
| sdk-partenaire-01 | Partenaire SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Terminé | Aperçu des commentaires sur l'intégrité fusionnés ; accusé de réception à 15h18 UTC. |
| sdk-partenaire-02 | Partenaire SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Terminé | Examen de la télémétrie/expurgation effectué ; accusé de réception à 15h22 UTC. || passerelle-ops-01 | Opérateur de passerelle | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Terminé | Commentaires du runbook DNS de la passerelle déposés ; accusé de réception à 15h24 UTC. |

## Journal des points de contrôle - W1

| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-04-12 | Envoi d'invitations + vérification des artefacts | Les huit partenaires ont envoyé un e-mail avec le descripteur/archive `preview-2025-04-12` ; accusés de réception stockés dans le tracker. |
| 2025-04-13 | Examen de base de la télémétrie | Tableaux de bord `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` verts ; les heures de bureau ont confirmé la vérification de la somme de contrôle terminée. |
| 2025-04-18 | Horaires de bureau en moyenne vague | `docs.preview.integrity` est resté vert ; deux lentes de documentation enregistrées sous `docs-preview/w1` (libellé de navigation + capture d'écran Essayez-le). |
| 2025-04-22 | Vérification ponctuelle finale de télémétrie | Proxy + tableaux de bord sains ; aucun nouveau problème soulevé, noté dans le tracker avant la sortie. |
| 2025-04-26 | Résumé de sortie + fermetures d'invitations | Tous les partenaires ont confirmé l'achèvement de l'examen, les invitations révoquées et les preuves archivées dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Récapitulatif de la cohorte bêta W3

- Invitations envoyées le 18/02/2026 avec vérification de la somme de contrôle + accusés de réception enregistrés le jour même.
- Commentaires recueillis sous `docs-preview/20260218` avec le problème de gouvernance `DOCS-SORA-Preview-20260218` ; digest + résumé généré via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Accès révoqué le 28/02/2026 après le contrôle télémétrique final ; tracker + tables de portail mises à jour pour afficher W3 comme terminé.## Journal des invitations - Communauté W2| ID du réviseur | Rôle | Demander un billet | Invitation envoyée (UTC) | Sortie prévue (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Réviseur communautaire (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Terminé | Accusé de réception à 16h06 UTC ; se concentrer sur les démarrages rapides du SDK ; sortie confirmée le 29/06/2025. |
| comm-vol-02 | Réviseur communautaire (Gouvernance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Terminé | Examen de la gouvernance/SNS effectué ; sortie confirmée le 29/06/2025. |
| comm-vol-03 | Réviseur communautaire (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Terminé | Commentaires pas à pas Norito enregistrés ; accusé de réception le 29/06/2025. |
| comm-vol-04 | Réviseur communautaire (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Terminé | Examen du runbook SoraFS effectué ; accusé de réception le 29/06/2025. |
| comm-vol-05 | Réviseur communautaire (Accessibilité) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Terminé | Notes d’accessibilité/UX partagées ; accusé de réception le 29/06/2025. |
| comm-vol-06 | Réviseur communautaire (localisation) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Terminé | Commentaires sur la localisation enregistrés ; accusé de réception le 29/06/2025. |
| comm-vol-07 | Réviseur communautaire (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Terminé | Vérifications de documents du SDK mobile fournies ; accusé de réception le 29/06/2025. || comm-vol-08 | Réviseur communautaire (Observabilité) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Terminé | Examen de l'annexe sur l'observabilité effectué ; accusé de réception le 29/06/2025. |

## Journal des points de contrôle - W2

| Date (UTC) | Activité | Remarques |
| --- | --- | --- |
| 2025-06-15 | Envoi d'invitations + vérification des artefacts | Descripteur/archive `preview-2025-06-15` partagé avec 8 évaluateurs de la communauté ; accusés de réception stockés dans le tracker. |
| 2025-06-16 | Examen de base de la télémétrie | Tableaux de bord `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verts ; Essayez-le, les journaux de proxy affichent les jetons de communauté actifs. |
| 2025-06-18 | Horaires de bureau et tri des problèmes | Collecte de deux suggestions (formulation de l'info-bulle `docs-preview/w2 #1`, barre latérale de localisation `#2`), toutes deux acheminées vers Docs. |
| 2025-06-21 | Vérification de télémétrie + correctifs de documentation | Documents adressés à `docs-preview/w2 #1/#2` ; tableaux de bord toujours verts, aucun incident. |
| 2025-06-24 | Horaires de bureau de la dernière semaine | Les évaluateurs ont confirmé les commentaires restants ; pas d'alerte feu. |
| 2025-06-29 | Résumé de sortie + fermetures d'invitations | Accusés de réception enregistrés, accès à l'aperçu révoqué, instantanés de télémétrie + artefacts archivés (voir [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horaires de bureau et tri des problèmes | Deux suggestions de documentation enregistrées sous `docs-preview/w1` ; aucun incident ni alerte n’a été déclenché. |

## Crochets de rapport- Chaque mercredi, mettez à jour le tableau de suivi ci-dessus ainsi que le problème d'invitation actif avec une courte note d'état (invitations envoyées, réviseurs actifs, incidents).
- Lorsqu'une vague se ferme, ajoutez le chemin du résumé des commentaires (par exemple, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et liez-le à partir de `status.md`.
- Si des critères de pause du déclencheur [flux d'invitations en aperçu](./preview-invite-flow.md), ajoutez les étapes de correction ici avant de reprendre les invitations.