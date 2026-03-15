---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-log
titre : Journal feedback et télémétrie W1
sidebar_label : Journal W1
description : Roster agrégé, points de contrôle de télémétrie et notes reviewers pour la première vague aperçu partenaires.
---

Ce journal conserve le roster des invitations, les points de contrôle de télémétrie et le feedback reviewers pour le
**preview partenaires W1** qui accompagne les taches d'acceptation dans
[`preview-feedback/w1/plan.md`](./plan.md) et l'entrée du tracker de vague dans
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Commencez-le a jour quand une invitation est envoyee,
qu'un instantané de télémétrie est enregistré, ou qu'un élément de feedback est trié afin que les réviseurs puissent
rejouer les preuves sans courir après des tickets externes.

## Liste des cohortes| ID du partenaire | Billet de demande | Récupération NDA | Inviter un envoyé (UTC) | Accusé de réception/première connexion (UTC) | Statuts | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| partenaire-w1-01 | `DOCS-SORA-Preview-REQ-P01` | D'accord 03/04/2025 | 2025-04-12 15:00 | 2025-04-12 15:11 | Terminer 2025-04-26 | sorafs-op-01 ; concentrez-vous sur les preuves de parité de docs orchestrator. |
| partenaire-w1-02 | `DOCS-SORA-Preview-REQ-P02` | D'accord 03/04/2025 | 2025-04-12 15:03 | 2025-04-12 15:15 | Terminer 2025-04-26 | sorafs-op-02 ; a valider les cross-links Norito/telemetrie. |
| partenaire-w1-03 | `DOCS-SORA-Preview-REQ-P03` | D'accord 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Terminer 2025-04-26 | sorafs-op-03 ; a exécuter des exercices de basculement multi-source. |
| partenaire-w1-04 | `DOCS-SORA-Preview-REQ-P04` | D'accord 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Terminer 2025-04-26 | torii-int-01 ; revue du livre de recettes Torii `/v1/pipeline` + Essayez-le. |
| partenaire-w1-05 | `DOCS-SORA-Preview-REQ-P05` | D'accord 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Terminer 2025-04-26 | torii-int-02 ; a accompagne la mise à jour de capture Essayez-le (docs-preview/w1 #2). |
| partenaire-w1-06 | `DOCS-SORA-Preview-REQ-P06` | D'accord 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Terminer 2025-04-26 | sdk-partenaire-01 ; commentaires sur les livres de recettes JS/Swift + vérifications d'intégrité pont ISO. || partenaire-w1-07 | `DOCS-SORA-Preview-REQ-P07` | D'accord 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Terminer 2025-04-26 | sdk-partenaire-02 ; conformité valide 2025-04-11, focalisée sur notes Connect/telemetrie. |
| partenaire-w1-08 | `DOCS-SORA-Preview-REQ-P08` | D'accord 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Terminer 2025-04-26 | passerelle-ops-01 ; audit du guide ops gateway + flux proxy Essayez-le de manière anonyme. |

Renseignez **Invite envoyee** et **Ack** des que l'email sortant est émis.
Ancrez les heures au planning UTC définies dans le plan W1.

## Télémétrie des points de contrôle

| Horodatage (UTC) | Tableaux de bord / sondes | Responsable | Résultat | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tout vert | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcription `npm run manage:tryit-proxy -- --stage preview-w1` | Opérations | Mise en scène | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Tableaux de bord ci-dessus + `probe:portal` | Docs/DevRel + Ops | Pré-invitation d'instantané, aucune régression | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Tableaux de bord ci-dessus + diff de latence proxy Essayez-le | Responsable Docs/DevRel | Checkpoint milieu valide (0 alertes; latence Essayez-le p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Tableaux de bord ci-dessus + sonde de sortie | Docs/DevRel + liaison gouvernance | Snapshot de sortie, zéro alertes restantes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Les echantillons quotidiens d'office hours (2025-04-13 -> 2025-04-25) sont regroupés en exports NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` avec les noms de fichier
`docs-preview-integrity-<date>.json` et les captures correspondantes.

## Consigner les commentaires et les problèmes

Utilisez ce tableau pour reprendre les constats des reviewers. Liez chaque entrée au ticket GitHub/discuss
ainsi qu'au capture de la structure du formulaire via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Référence | Sévère | Responsable | Statuts | Remarques |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Faible | Docs-core-02 | Résolution 2025-04-18 | Clarification du wording de nav Try it + ancre sidebar (`docs/source/sorafs/tryit.md` mis a jour avec le nouveau label). |
| `docs-preview/w1 #2` | Faible | Docs-core-03 | Résolution 2025-04-19 | Capture Try it + légende rafraichies selon la demande ; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informations | Responsable Docs/DevRel | Ferme | Les commentaires restants étaient uniquement Q&A; captures dans chaque formulaire partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Suivi contrôle des connaissances et enquêtes

1. Enregistrer les scores de quiz (cible >=90%) pour chaque évaluateur ; joindre le CSV exporte à la cote des artefacts d'invitation.
2. Collecter les réponses qualitatives du sondage capturées via le template de feedback et les copieur sous
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifier des appels de remédiation pour toute personne sous le seuil et les consignataires ici.Les huit évaluateurs ont obtenu >=94% au contrôle des connaissances (CSV :
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Aucun appel de remédiation
n'a été nécessaire; les exportations de sondage pour chaque partenaire sont sous
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventaire des objets

- Descripteur/somme de contrôle de l'aperçu du bundle : `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Reprise de la sonde + vérification du lien : `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de changement du proxy Essayez-le : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exports télémétrie : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Forfait télémétrie horaires quotidiens de bureau : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Retour d'expérience exports + enquête : placer des dossiers par reviewer sous
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Vérification des connaissances et CV CSV : `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garder l'inventaire synchronisé avec l'issue tracker. Joindre des hachages lors de la copie d'artefacts vers
le ticket gouvernance afin que les auditeurs puissent vérifier les fichiers sans accès au shell.