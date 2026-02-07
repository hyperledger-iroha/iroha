---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Le tracker des convites fait un aperçu

Ce tracker s'enregistre chaque fois sur le portail de prévisualisation des documents pour les propriétaires de DOCS-SORA et les réviseurs de gouvernance qui ont la même coordination que celle-ci, qui ont approuvé les invités et les articles avec précision. Actualisez-o semper que convites forem enviados, revogados ou adiados para que a trilha de auditoria permaneca no repositorio.

## Statut des ondes| Onde | Coorte | Numéro d'accompagnement | Fournisseur(s) | Statut | Janela aussi | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Mainteneurs principaux** | Mainteneurs de Docs + SDK validant ou flux de somme de contrôle | `DOCS-SORA-Preview-W0` (suivi GitHub/ops) | Lead Docs/DevRel + Portail TL | Concluido | T2 2025 semaines 1-2 | Convites envoyés le 25/03/2025, télémétrie ficou verde, résumé de ladite publication le 08/04/2025. |
| **W1 - Partenaires** | Opérateurs SoraFS, intégrateurs Torii sur NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + liaison de gouvernance | Concluido | T2 2025 semaine 3 | Convites 2025-04-12 -> 2025-04-26 avec os oito partenaires confirmés ; Les preuves capturées dans [`preview-feedback/w1/log.md`] (./preview-feedback/w1/log.md) et le résumé de ladite déclaration dans [`preview-feedback/w1/summary.md`] (./preview-feedback/w1/summary.md). |
| **W2 - Communauté** | Liste d'attente comunitaria curada ( 2025-06-29 avec telemetria verde o periodo todo; preuve + achados em [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). || **W3 - Coortes bêta** | Bêta de financement/observabilité + SDK partenaire + défenseur de l'écosystème | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + liaison de gouvernance | Concluido | T1 2026 semaine 8 | Convie le 18/02/2026 -> 28/02/2026 ; résumé + données du portail gerados via onda `preview-20260218` (ver [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Remarque : Vincule chaque fois qu'un tracker est émis pour les tickets de sollicitation de prévisualisation et d'archivage dans le projet `docs-portal-preview` afin que, comme prévu, la découverte continue.

## Tarefas ativas (W0)- Artefatos de preflight actualisés (exécutés GitHub Actions `docs-portal-preview` 2025-03-24, descripteur vérifié via `scripts/preview_verify.sh` avec la balise `preview-2025-03-24`).
- Baselines de télémétrie capturées (`docs.preview.integrity`, instantané des tableaux de bord `TryItProxyErrors` salvo na issue W0).
- Texte de sensibilisation utilisé par [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) avec aperçu des balises `preview-2025-03-24`.
- Sollicitacoes de entrada registradas para os primeiros cinco mainteneurs (billets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinco primeiros convites enviados 2025-03-25 10:00-10:20 UTC après six jours consécutifs de télémétrie verte ; accuse les gardiens em `DOCS-SORA-Preview-W0`.
- Monitoramento de telemetria + heures de bureau d'hébergement (journaux d'enregistrement du 2025-03-31 ; journal des points de contrôle abaixo).
- Commentaires de meio de onda / issues coletadas e tagueadas `docs-preview/w0` (ver [W0 digest](./preview-feedback/w0/summary.md)).
- Résumé de l'onde publiée + confirmations de ladite information (bundle de ladite donnée du 08/04/2025 ; version [W0 digest](./preview-feedback/w0/summary.md)).
- Une version bêta W3 accompagnée ; futuras ondas agendadas conforme revisao de gouvernance.

## Resumo da onda Partenaires W1- Approbations légales et de gouvernance. Addendum des partenaires assassinés le 2025-04-05 ; aprovacoes enviadas para `DOCS-SORA-Preview-W1`.
- Telemetria + Essayez la mise en scène. Ticket de modification `OPS-TRYIT-147` exécuté le 06/04/2025 avec les instantanés Grafana de `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` enregistrés.
- Préparation de l'artefato + somme de contrôle. Bundle `preview-2025-04-12` vérifié ; enregistre les salves de descripteur/somme de contrôle/sonde dans `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Liste des convites + envio. Oito sollicitacoes de partenaires (`DOCS-SORA-Preview-REQ-P01...P08`) approuvés ; convites envoyés 2025-04-12 15:00-15:21 UTC avec des accusations enregistrées par le réviseur.
- Instrument de rétroaction. Agendas des heures de bureau + points de contrôle de télémétrie enregistrés ; voir [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) pour le résumé.
- Liste finale / log de Saida. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) enregistre les horodatages de convite/ack, les preuves de télémétrie, les exportations de quiz et les ponts d'artefatos le 2025-04-26 pour que la gouvernance puisse reproduire une onda.

## Log des convites - Mainteneurs principaux de W0| ID du réviseur | Papier | Ticket de sollicitation | Envoyer une invitation (UTC) | Saïda espérée (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Responsable du portail | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Ativo | Confirmez la vérification de la somme de contrôle ; mis à jour dans la révision de la navigation/barre latérale. |
| sdk-rouille-01 | Responsable du SDK Rust | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Ativo | Test et réception du SDK + démarrages rapides de Norito. |
| sdk-js-01 | Responsable du SDK JS | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Ativo | Console Validando Essayez-le + fluxos ISO. |
| sorafs-ops-01 | SoraFS liaison opérateur | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Ativo | Auditer les runbooks SoraFS + docs de orchestration. |
| observabilité-01 | Observabilité TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Ativo | Réviser les annexes de télémétrie/incidents ; responsable de la couverture d'Alertmanager. |

Tous nos invités se réfèrent à mon artefato `docs-portal-preview` (exécuté le 2025-03-24, tag `preview-2025-03-24`) et au journal de vérification capturé dans `DOCS-SORA-Preview-W0`. Chaque fois qu'une pause/une pause doit être enregistrée tant sur le tableau précis que sur l'émission du tracker avant de passer à l'onde proche.

## Journal des points de contrôle - W0| Données (UTC) | Activité | Notes |
| --- | --- | --- |
| 2025-03-26 | Révision de la télémétrie de base + heures de bureau | `docs.preview.integrity` + `TryItProxyErrors` ficaram verdes; heures de bureau confirmaram verificacao de checksum concluida. |
| 2025-03-27 | Digest de feedback intermédiaire publié | Résumé capturé dans [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); J'ai des problèmes de navigation mineurs tels que `docs-preview/w0`, sans incidents. |
| 2025-03-31 | Contrôle de télémétrie pour la dernière semaine | Horaires de bureau Ultimas avant la sortie ; revisores confirmaram tarefas restantes em andamento, sem alertas. |
| 2025-04-08 | CV de saya + encerramento de convites | Avis complets confirmés, accès temporaire revogado, achados arquivados em [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker actualisé avant la préparation de W1. |

## Log de convites - Partenaires W1| ID du réviseur | Papier | Ticket de sollicitation | Envoyer une invitation (UTC) | Saïda espérée (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | Opérateur SoraFS (UE) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Concluido | Entregou feedback de ops do orchestre 2025-04-20; accusé de réception à 15h05 UTC. |
| sorafs-op-02 | Opérateur SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Concluido | Commentaires sur le déploiement enregistré dans `docs-preview/w1` ; accusé de réception à 15h10 UTC. |
| sorafs-op-03 | Opérateur SoraFS (États-Unis) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Concluido | Edicoes de contest/blacklist registradas ; accusé de réception à 15h12 UTC. |
| torii-int-01 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Concluido | Procédure pas à pas de Try it auth aceito; accusé de réception à 15h14 UTC. |
| torii-int-02 | Intégrateur Torii | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Concluido | Commentaires sur les enregistrements RPC/OAuth ; accusé de réception à 15h16 UTC. |
| sdk-partenaire-01 | Partenaire SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Concluido | Commentaires d'intégration de l'aperçu fusionné ; accusé de réception à 15h18 UTC. |
| sdk-partenaire-02 | Partenaire SDK (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Concluido | Révision de télémétrie/rédaction feita ; accusé de réception à 15h22 UTC. || passerelle-ops-01 | Opérateur de passerelle | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Concluido | Commentaires sur le runbook des passerelles DNS enregistrées ; accusé de réception à 15h24 UTC. |

## Journal des points de contrôle - W1

| Données (UTC) | Activité | Notes |
| --- | --- | --- |
| 2025-04-12 | Envoi des convites + vérification des artefatos | Les partenaires d'Oito reçoivent un e-mail avec le descripteur/archive `preview-2025-04-12` ; accuse les registrados pas de tracker. |
| 2025-04-13 | Révision de la base de référence de la télémétrie | `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` verts ; heures de bureau confirmaram verificacao de checksum concluida. |
| 2025-04-18 | Horaires de bureau de meio de onda | `docs.preview.integrity` vert permanent; dois nits de docs tagueados `docs-preview/w1` (texte de navigation + capture d'écran Essayez-le). |
| 2025-04-22 | Contrôle final de télémétrie | Proxy + tableaux de bord saudaveis ; nenhuma issue nova, enregistré sans tracker avant la dite. |
| 2025-04-26 | CV de saya + encerramento de convites | Tous les partenaires confirment l'examen, invitent les revogados, les preuves sont archivées dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Récapitulatif de la semaine bêta W3- Convites enviados 2026-02-18 com verificacao de checksum + accuses registrados no mesmo dia.
- Commentaires partagés sur `docs-preview/20260218` avec le problème de gouvernance `DOCS-SORA-Preview-20260218` ; résumé + résumé gerados via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acesso revogado 2026-02-28 apos o check final de telemetria ; tracker + tableaux du portail actualisés pour marquer W3 comme conclu.

## Journal des convites - Communauté W2| ID du réviseur | Papier | Ticket de sollicitation | Envoyer une invitation (UTC) | Saïda espérée (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Réviseur communautaire (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Concluido | Accusé de réception à 16h06 UTC ; se concentre sur les démarrages rapides du SDK ; dit confirmé le 2025-06-29. |
| comm-vol-02 | Réviseur communautaire (Gouvernance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Concluido | Révision de gouvernance/SNS feita ; dit confirmé le 2025-06-29. |
| comm-vol-03 | Réviseur communautaire (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Concluido | Les commentaires font la procédure pas à pas Norito enregistrée ; accusé de réception le 29/06/2025. |
| comm-vol-04 | Réviseur communautaire (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Concluido | Révision des runbooks SoraFS feita ; accusé de réception le 29/06/2025. |
| comm-vol-05 | Réviseur communautaire (Accessibilité) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Concluido | Notas de acessibilidade/UX comparilhadas ; accusé de réception le 29/06/2025. |
| comm-vol-06 | Réviseur communautaire (localisation) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Concluido | Commentaires de localisation enregistrés ; accusé de réception le 29/06/2025. || comm-vol-07 | Réviseur communautaire (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Concluido | Vérifie les documents du SDK mobile entregues ; accusé de réception le 29/06/2025. |
| comm-vol-08 | Réviseur communautaire (Observabilité) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Concluido | Révision de l'annexe d'observabilité feita ; accusé de réception le 29/06/2025. |

## Journal des points de contrôle - W2| Données (UTC) | Activité | Notes |
| --- | --- | --- |
| 2025-06-15 | Envoi des convites + vérification des artefatos | Descripteur/archive `preview-2025-06-15` compartimenté avec 8 réviseurs ; accuse les guardados pas de tracker. |
| 2025-06-16 | Révision de la base de référence de la télémétrie | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verts ; les journaux font un proxy Essayez-le mostram tokens comunitarios ativos. |
| 2025-06-18 | Horaires de bureau et tri des problèmes | Deux suggestions (formulation `docs-preview/w2 #1` de l'info-bulle, barre latérale `#2` de localisation) - sont attribuées à Docs. |
| 2025-06-21 | Contrôle de télémétrie + correctifs de documents | Documents résolus `docs-preview/w2 #1/#2` ; tableaux de bord verts, sem incidents. |
| 2025-06-24 | Horaires de bureau pour la dernière semaine | Revisores confirmaram envios restantes; nenhum alerta. |
| 2025-06-29 | CV de saya + encerramento de convites | Accusés de réception enregistrés, accès à l'aperçu revogado, instantanés + artefatos archivés (version [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Horaires de bureau et tri des problèmes | Deux suggestions de documents enregistrées dans le `docs-preview/w1` ; sem incidents nem alertas. |

## Crochets de reporting- Chaque quart d'heure, actualisez une table acima et un numéro de convites ativa com uma nota curta (convites enviados, revisores ativos, incidentses).
- Lorsqu'une onda encerrar, ajoutez le chemin du résumé de feedback (par exemple, `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et lien à partir de `status.md`.
- Selon les critères de pause du [flux d'invitation d'aperçu] (./preview-invite-flow.md) pour l'acionado, ajoutez les étapes de correction ici avant le retour des invités.