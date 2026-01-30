---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-tracker.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Tracker des invitations preview

Ce tracker enregistre chaque vague preview du portail docs afin que les owners DOCS-SORA et les relecteurs gouvernance voient quelle cohorte est active, qui a approuve les invitations et quels artefacts restent a traiter. Mettez-le a jour chaque fois que des invitations sont envoyees, revoquees ou reportees pour que la piste d'audit reste dans le depot.

## Statut des vagues

| Vague | Cohorte | Issue de suivi | Approbateur(s) | Statut | Fenetre cible | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers Docs + SDK validant le flux checksum | `DOCS-SORA-Preview-W0` (tracker GitHub/ops) | Lead Docs/DevRel + Portal TL | Termine | Q2 2025 semaines 1-2 | Invitations envoyees 2025-03-25, telemetrie restee verte, resume de sortie publie 2025-04-08. |
| **W1 - Partners** | Operateurs SoraFS, integrateurs Torii sous NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + liaison gouvernance | Termine | Q2 2025 semaine 3 | Invitations 2025-04-12 -> 2025-04-26 avec les huit partners confirmes; evidence capturee dans [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) et resume de sortie dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Communaute** | Waitlist communaute triee (<=25 a la fois) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + community manager | Termine | Q3 2025 semaine 1 (tentatif) | Invitations 2025-06-15 -> 2025-06-29 avec telemetrie verte tout du long; evidence + constats dans [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes beta** | Beta finance/observabilite + partner SDK + advocate ecosysteme | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + liaison gouvernance | Termine | Q1 2026 semaine 8 | Invitations 2026-02-18 -> 2026-02-28; resume + donnees portail generees via la vague `preview-20260218` (voir [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> Note: liez chaque issue du tracker aux tickets de demande preview et archivez-les dans le projet `docs-portal-preview` pour que les approbations restent decouvrables.

## Taches actives (W0)

- Artefacts de preflight rafraichis (execution GitHub Actions `docs-portal-preview` 2025-03-24, descriptor verifie via `scripts/preview_verify.sh` avec le tag `preview-2025-03-24`).
- Baselines telemetrie capturees (`docs.preview.integrity`, snapshot des dashboards `TryItProxyErrors` sauvegarde dans l'issue W0).
- Texte d'outreach fige avec [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) et tag preview `preview-2025-03-24`.
- Demandes d'entree enregistrees pour les cinq premiers maintainers (tickets `DOCS-SORA-Preview-REQ-01` ... `-05`).
- Cinq premieres invitations envoyees 2025-03-25 10:00-10:20 UTC apres sept jours consecutifs de telemetrie verte; accus stockes dans `DOCS-SORA-Preview-W0`.
- Suivi telemetrie + office hours du host (check-ins quotidiens jusqu'au 2025-03-31; log des checkpoints ci-dessous).
- Feedback mi-vague / issues collectees et taguees `docs-preview/w0` (voir [W0 digest](./preview-feedback/w0/summary.md)).
- Resume de vague publie + confirmations de sortie (bundle de sortie date 2025-04-08; voir [W0 digest](./preview-feedback/w0/summary.md)).
- Vague beta W3 suivie; futures vagues planifiees selon revue gouvernance.

## Resume de la vague W1 partners

- Approbations legales et gouvernance. Addendum partners signe 2025-04-05; approbations chargees dans `DOCS-SORA-Preview-W1`.
- Telemetrie + Try it staging. Ticket de changement `OPS-TRYIT-147` execute 2025-04-06 avec snapshots Grafana de `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` archives.
- Preparation artefact + checksum. Bundle `preview-2025-04-12` verifie; logs descriptor/checksum/probe stockes dans `artifacts/docs_preview/W1/preview-2025-04-12/`.
- Roster invitations + envoi. Huit demandes partners (`DOCS-SORA-Preview-REQ-P01...P08`) approuvees; invitations envoyees 2025-04-12 15:00-15:21 UTC avec accus par relecteur.
- Instrumentation feedback. Office hours quotidiennes + checkpoints telemetrie enregistres; voir [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) pour le digest.
- Roster final / log de sortie. [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) enregistre maintenant timestamps d'invitation/ack, evidence telemetrie, exports quiz et pointeurs d'artefacts au 2025-04-26 pour permettre la relecture gouvernance.

## Log des invitations - W0 core maintainers

| ID relecteur | Role | Ticket de demande | Invitation envoyee (UTC) | Sortie attendue (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | Actif | A confirme la verification checksum; focus nav/sidebar. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | Actif | Teste les recettes SDK + quickstarts Norito. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | Actif | Valide la console Try it + flows ISO. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | Actif | Audite les runbooks SoraFS + docs orchestration. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | Actif | Revoit les annexes telemetrie/incidents; responsable couverture Alertmanager. |

Toutes les invitations referencent le meme artefact `docs-portal-preview` (execution 2025-03-24, tag `preview-2025-03-24`) et le log de verification capture dans `DOCS-SORA-Preview-W0`. Toute ajout/pause doit etre consigne dans le tableau ci-dessus et l'issue du tracker avant de passer a la vague suivante.

## Log des checkpoints - W0

| Date (UTC) | Activite | Notes |
| --- | --- | --- |
| 2025-03-26 | Revue telemetrie baseline + office hours | `docs.preview.integrity` + `TryItProxyErrors` sont restes verts; office hours ont confirme la verification checksum terminee. |
| 2025-03-27 | Digest feedback intermediaire publie | Resume capture dans [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md); deux issues nav mineures taguees `docs-preview/w0`, aucun incident. |
| 2025-03-31 | Check telemetrie fin de semaine | Dernieres office hours pre-exit; relecteurs ont confirme les taches restantes, aucune alerte. |
| 2025-04-08 | Resume de sortie + fermeture des invitations | Reviews terminees confirmees, acces temporaire revoque, constats archives dans [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08); tracker mis a jour avant W1. |

## Log des invitations - W1 partners

| ID relecteur | Role | Ticket de demande | Invitation envoyee (UTC) | Sortie attendue (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | Termine | Feedback ops orchestrator livre 2025-04-20; ack sortie 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | Termine | Commentaires rollout logges dans `docs-preview/w1`; ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | Termine | Edits dispute/blacklist enregistres; ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | Termine | Walkthrough Try it auth accepte; ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | Termine | Commentaires RPC/OAuth logges; ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | Termine | Feedback integrite preview merge; ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | Termine | Revue telemetrie/redaction faite; ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | Termine | Commentaires runbook DNS gateway logges; ack 15:24 UTC. |

## Log des checkpoints - W1

| Date (UTC) | Activite | Notes |
| --- | --- | --- |
| 2025-04-12 | Envoi invitations + verification artefacts | Huit partners emails avec descriptor/archive `preview-2025-04-12`; accus stockes dans le tracker. |
| 2025-04-13 | Revue telemetrie baseline | `docs.preview.integrity`, `TryItProxyErrors`, et `DocsPortal/GatewayRefusals` verts; office hours ont confirme la verification checksum terminee. |
| 2025-04-18 | Office hours mi-vague | `docs.preview.integrity` reste vert; deux nits docs tagges `docs-preview/w1` (nav wording + screenshot Try it). |
| 2025-04-22 | Check telemetrie final | Proxy + dashboards sains; aucune nouvelle issue, notee dans le tracker avant sortie. |
| 2025-04-26 | Resume de sortie + fermeture invitations | Tous les partners ont confirme la review, invitations revoquees, evidence archivee dans [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## Recap cohorte beta W3

- Invitations envoyees 2026-02-18 avec verification checksum + accus le meme jour.
- Feedback collecte sous `docs-preview/20260218` avec issue gouvernance `DOCS-SORA-Preview-20260218`; digest + resume genere via `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- Acces revoque 2026-02-28 apres le check telemetrie final; tracker + tables portail mises a jour pour marquer W3 termine.

## Log des invitations - W2 community

| ID relecteur | Role | Ticket de demande | Invitation envoyee (UTC) | Sortie attendue (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | Termine | Ack 16:06 UTC; focus quickstarts SDK; sortie confirmee 2025-06-29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | Termine | Revue gouvernance/SNS terminee; sortie confirmee 2025-06-29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | Termine | Feedback walkthrough Norito logge; ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | Termine | Revue runbooks SoraFS terminee; ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | Termine | Notes accessibilite/UX partagees; ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | Termine | Feedback localisation logge; ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | Termine | Checks docs SDK mobile livres; ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | Termine | Revue annexe observabilite terminee; ack 2025-06-29. |

## Log des checkpoints - W2

| Date (UTC) | Activite | Notes |
| --- | --- | --- |
| 2025-06-15 | Envoi invitations + verification artefacts | Descriptor/archive `preview-2025-06-15` partage avec 8 relecteurs; accus stockes dans tracker. |
| 2025-06-16 | Revue telemetrie baseline | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` verts; logs proxy Try it montrent tokens communaute actifs. |
| 2025-06-18 | Office hours et triage issues | Deux suggestions (`docs-preview/w2 #1` wording tooltip, `#2` sidebar localisation) - toutes deux assignees a Docs. |
| 2025-06-21 | Check telemetrie + fixes docs | Docs a corrige `docs-preview/w2 #1/#2`; dashboards verts, aucun incident. |
| 2025-06-24 | Office hours fin de semaine | Relecteurs ont confirme les retours restants; aucune alerte. |
| 2025-06-29 | Resume de sortie + fermeture invitations | Acks enregistres, acces preview revoque, snapshots + artefacts archives (voir [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | Office hours et triage issues | Deux suggestions documentation loggees sous `docs-preview/w1`; aucun incident ni alerte. |

## Hooks de reporting

- Chaque mercredi, mettre a jour le tableau ci-dessus et l'issue invite active avec une note courte (invitations envoyees, relecteurs actifs, incidents).
- Quand une vague se termine, ajouter le chemin du resume feedback (ex. `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) et le lier depuis `status.md`.
- Si un critere de pause du [preview invite flow](./preview-invite-flow.md) est declenche, ajouter les etapes de remediation ici avant de reprendre les invitations.
