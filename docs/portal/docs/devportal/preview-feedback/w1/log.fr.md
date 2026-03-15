---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 51971e1dc4e763ac7017f76c7239eef943bc21151e49e827988b61972fa58245
source_last_modified: "2025-12-19T22:36:07.161570+00:00"
translation_last_reviewed: 2026-01-01
---

---
id: preview-feedback-w1-log
title: Journal feedback et telemetrie W1
sidebar_label: Journal W1
description: Roster agrege, checkpoints de telemetrie et notes reviewers pour la premiere vague preview partenaires.
---

Ce journal conserve le roster des invitations, les checkpoints de telemetrie et le feedback reviewers pour le
**preview partenaires W1** qui accompagne les taches d'acceptation dans
[`preview-feedback/w1/plan.md`](./plan.md) et l'entree du tracker de vague dans
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Mettez-le a jour quand une invitation est envoyee,
qu'un snapshot de telemetrie est enregistre, ou qu'un item de feedback est trie afin que les reviewers governance puissent
rejouer les preuves sans courir apres des tickets externes.

## Roster de cohorte

| Partner ID | Ticket de demande | NDA recu | Invite envoyee (UTC) | Ack/premier login (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| partner-w1-01 | `DOCS-SORA-Preview-REQ-P01` | OK 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | Termine 2025-04-26 | sorafs-op-01; concentre sur les preuves de parite de docs orchestrator. |
| partner-w1-02 | `DOCS-SORA-Preview-REQ-P02` | OK 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | Termine 2025-04-26 | sorafs-op-02; a valide les cross-links Norito/telemetrie. |
| partner-w1-03 | `DOCS-SORA-Preview-REQ-P03` | OK 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Termine 2025-04-26 | sorafs-op-03; a execute des drills de failover multi-source. |
| partner-w1-04 | `DOCS-SORA-Preview-REQ-P04` | OK 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Termine 2025-04-26 | torii-int-01; revue du cookbook Torii `/v1/pipeline` + Try it. |
| partner-w1-05 | `DOCS-SORA-Preview-REQ-P05` | OK 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Termine 2025-04-26 | torii-int-02; a accompagne la mise a jour de capture Try it (docs-preview/w1 #2). |
| partner-w1-06 | `DOCS-SORA-Preview-REQ-P06` | OK 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Termine 2025-04-26 | sdk-partner-01; feedback cookbooks JS/Swift + sanity checks ISO bridge. |
| partner-w1-07 | `DOCS-SORA-Preview-REQ-P07` | OK 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Termine 2025-04-26 | sdk-partner-02; compliance valide 2025-04-11, focalise sur notes Connect/telemetrie. |
| partner-w1-08 | `DOCS-SORA-Preview-REQ-P08` | OK 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Termine 2025-04-26 | gateway-ops-01; audit du guide ops gateway + flux proxy Try it anonymise. |

Renseignez **Invite envoyee** et **Ack** des que l'email sortant est emis.
Ancrez les heures au planning UTC defini dans le plan W1.

## Checkpoints telemetrie

| Horodatage (UTC) | Dashboards / probes | Responsable | Resultat | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tout vert | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcript `npm run manage:tryit-proxy -- --stage preview-w1` | Ops | Staged | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Dashboards ci-dessus + `probe:portal` | Docs/DevRel + Ops | Snapshot pre-invite, aucune regression | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Dashboards ci-dessus + diff de latence proxy Try it | Docs/DevRel lead | Checkpoint milieu valide (0 alertes; latence Try it p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Dashboards ci-dessus + probe de sortie | Docs/DevRel + Governance liaison | Snapshot de sortie, zero alertes restantes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Les echantillons quotidiens d'office hours (2025-04-13 -> 2025-04-25) sont regroupes en exports NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` avec les noms de fichier
`docs-preview-integrity-<date>.json` et les captures correspondantes.

## Log feedback et issues

Utilisez ce tableau pour resumer les constats des reviewers. Liez chaque entree au ticket GitHub/discuss
ainsi qu'au formulaire structure capture via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Reference | Severite | Responsable | Statut | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Low | Docs-core-02 | Resolu 2025-04-18 | Clarification du wording de nav Try it + ancre sidebar (`docs/source/sorafs/tryit.md` mis a jour avec le nouveau label). |
| `docs-preview/w1 #2` | Low | Docs-core-03 | Resolu 2025-04-19 | Capture Try it + legende rafraichies selon la demande; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Info | Docs/DevRel lead | Ferme | Les commentaires restants etaient uniquement Q&A; captures dans chaque formulaire partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Suivi knowledge check et surveys

1. Enregistrer les scores de quiz (cible >=90%) pour chaque reviewer; joindre le CSV exporte a cote des artefacts d'invitation.
2. Collecter les reponses qualitatives du survey capturees via le template de feedback et les copier sous
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifier des appels de remediation pour toute personne sous le seuil et les consigner ici.

Les huit reviewers ont obtenu >=94% au knowledge check (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Aucun appel de remediation
n'a ete necessaire; les exports de survey pour chaque partenaire sont sous
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventaire des artefacts

- Bundle preview descriptor/checksum: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Resume probe + link-check: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de changement du proxy Try it: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exports telemetrie: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Bundle telemetrie daily office hours: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exports feedback + survey: placer des dossiers par reviewer sous
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV knowledge check et resume: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garder l'inventaire synchronise avec l'issue tracker. Joindre des hashes lors de la copie d'artefacts vers
le ticket governance afin que les auditeurs puissent verifier les fichiers sans acces shell.
