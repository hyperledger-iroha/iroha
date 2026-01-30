---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w1-summary
title: Resume feedback et sortie W1
sidebar_label: Resume W1
description: Constats, actions et preuves de sortie pour la vague de preview partenaires/integrateurs Torii.
---

| Element | Details |
| --- | --- |
| Vague | W1 - Partenaires et integrateurs Torii |
| Fenetre d'invitation | 2025-04-12 -> 2025-04-26 |
| Tag d'artefact | `preview-2025-04-12` |
| Issue tracker | `DOCS-SORA-Preview-W1` |
| Participants | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Points saillants

1. **Workflow checksum** - Tous les reviewers ont verifie le descriptor/archive via `scripts/preview_verify.sh`; les logs ont ete stockes avec les accuses d'invitation.
2. **Telemetrie** - Les dashboards `docs.preview.integrity`, `TryItProxyErrors` et `DocsPortal/GatewayRefusals` sont restes verts pendant toute la vague; aucun incident ni page d'alerte.
3. **Feedback docs (`docs-preview/w1`)** - Deux nits mineurs ont ete signales:
   - `docs-preview/w1 #1`: clarifier la formulation de navigation dans la section Try it (resolu).
   - `docs-preview/w1 #2`: mettre a jour la capture Try it (resolu).
4. **Parite runbook** - Les operateurs SoraFS ont confirme que les nouveaux cross-links entre `orchestrator-ops` et `multi-source-rollout` ont traite leurs points W0.

## Actions

| ID | Description | Responsable | Statut |
| --- | --- | --- | --- |
| W1-A1 | Mettre a jour la formulation de navigation Try it selon `docs-preview/w1 #1`. | Docs-core-02 | Termine (2025-04-18). |
| W1-A2 | Rafraichir la capture Try it selon `docs-preview/w1 #2`. | Docs-core-03 | Termine (2025-04-19). |
| W1-A3 | Resumer les constats partenaires et la preuve telemetrie dans roadmap/status. | Docs/DevRel lead | Termine (voir tracker + status.md). |

## Resume de sortie (2025-04-26)

- Les huit reviewers ont confirme la fin pendant les office hours finales, purge les artefacts locaux et leurs acces ont ete revoques.
- La telemetrie est restee verte jusqu'a la sortie; snapshots finaux attaches a `DOCS-SORA-Preview-W1`.
- Le log d'invitations a ete mis a jour avec les accuses de sortie; le tracker a marque W1 comme termine et ajoute les checkpoints.
- Bundle de preuve (descriptor, checksum log, probe output, transcript du proxy Try it, screenshots de telemetrie, feedback digest) archive sous `artifacts/docs_preview/W1/`.

## Prochaines etapes

- Preparer le plan d'intake communautaire W2 (approbation governance + ajustements du template de demande).
- Rafraichir le tag d'artefact preview pour la vague W2 et relancer le script de preflight une fois les dates finalisees.
- Porter les constats applicables de W1 dans roadmap/status pour que la vague communautaire ait les dernieres indications.
