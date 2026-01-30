---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/summary.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w2-summary
title: Resume feedback et statut W2
sidebar_label: Resume W2
description: Digest en direct pour la vague de preview communautaire (W2).
---

| Element | Details |
| --- | --- |
| Vague | W2 - Reviewers communautaires |
| Fenetre d'invitation | 2025-06-15 -> 2025-06-29 |
| Tag d'artefact | `preview-2025-06-15` |
| Issue tracker | `DOCS-SORA-Preview-W2` |
| Participants | comm-vol-01...comm-vol-08 |

## Points saillants

1. **Governance et tooling** - La politique d'intake communautaire approuvee a l'unanimite le 2025-05-20; le template de demande mis a jour avec champs motivation/fuseau horaire est dans `docs/examples/docs_preview_request_template.md`.
2. **Preflight et preuves** - Le changement du proxy Try it `OPS-TRYIT-188` execute le 2025-06-09, dashboards Grafana captures, et les outputs descriptor/checksum/probe de `preview-2025-06-15` archives sous `artifacts/docs_preview/W2/`.
3. **Vague d'invitations** - Huit reviewers communautaires invites le 2025-06-15, avec accuses enregistres dans la table d'invitation du tracker; tous ont termine la verification checksum avant navigation.
4. **Feedback** - `docs-preview/w2 #1` (wording de tooltip) et `#2` (ordre de sidebar de localisation) ont ete saisis le 2025-06-18 et resolus d'ici 2025-06-21 (Docs-core-04/05); aucun incident pendant la vague.

## Actions

| ID | Description | Responsable | Statut |
| --- | --- | --- | --- |
| W2-A1 | Traiter `docs-preview/w2 #1` (wording de tooltip). | Docs-core-04 | Termine 2025-06-21 |
| W2-A2 | Traiter `docs-preview/w2 #2` (sidebar de localisation). | Docs-core-05 | Termine 2025-06-21 |
| W2-A3 | Archiver les preuves de sortie + mettre a jour roadmap/status. | Docs/DevRel lead | Termine 2025-06-29 |

## Resume de sortie (2025-06-29)

- Les huit reviewers communautaires ont confirme la fin et l'acces preview a ete revoque; accuses enregistres dans le log d'invitation du tracker.
- Les snapshots finaux de telemetrie (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) sont restes verts; logs et transcripts du proxy Try it attaches a `DOCS-SORA-Preview-W2`.
- Bundle de preuves (descriptor, checksum log, probe output, link report, screenshots Grafana, accuses d'invitation) archive sous `artifacts/docs_preview/W2/preview-2025-06-15/`.
- Le log de checkpoints W2 du tracker a ete mis a jour jusqu'a la sortie, garantissant un enregistrement auditable avant le demarrage de la planification W3.
