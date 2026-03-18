---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-log
titre : Journal de feedback et télémétrie W1
sidebar_label : journal W1
description : Liste composée, points de contrôle de télémétrie et notes des réviseurs pour la première fois de l'aperçu des partenaires.
---

Ce journal conserve la liste des invitations, les points de contrôle de télémétrie et les commentaires des évaluateurs pour le
**aperçu des partenaires W1** qui accompagnent les tâches d'acceptation en
[`preview-feedback/w1/plan.md`](./plan.md) et l'entrée du tracker de la feuille en
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Actualizalo cuando se envie une invitación,
enregistrer un instantané de télémétrie ou trier un élément de feedback pour que les réviseurs d'État puissent le reproduire
la preuve sans persécuter les billets externes.

## Liste des cohortes| ID du partenaire | Ticket de sollicitude | NDA reçu | Invitation envoyée (UTC) | Connexion avec accusé de réception/amorce (UTC) | État | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| partenaire-w1-01 | `DOCS-SORA-Preview-REQ-P01` | D'accord 03/04/2025 | 2025-04-12 15:00 | 2025-04-12 15:11 | Terminé 2025-04-26 | sorafs-op-01 ; mis en évidence la preuve de parité des documents de l'orchestrateur. |
| partenaire-w1-02 | `DOCS-SORA-Preview-REQ-P02` | D'accord 03/04/2025 | 2025-04-12 15:03 | 2025-04-12 15:15 | Terminé 2025-04-26 | sorafs-op-02 ; valido cross-links de Norito/télémétrie. |
| partenaire-w1-03 | `DOCS-SORA-Preview-REQ-P03` | D'accord 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Terminé 2025-04-26 | sorafs-op-03 ; exécutez des exercices de basculement multi-sources. |
| partenaire-w1-04 | `DOCS-SORA-Preview-REQ-P04` | D'accord 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Terminé 2025-04-26 | torii-int-01 ; révision du livre de recettes de Torii `/v1/pipeline` + Essayez-le. |
| partenaire-w1-05 | `DOCS-SORA-Preview-REQ-P05` | D'accord 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Terminé 2025-04-26 | torii-int-02 ; accompagne la mise à jour de la capture d'écran de Try it (docs-preview/w1 #2). |
| partenaire-w1-06 | `DOCS-SORA-Preview-REQ-P06` | D'accord 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Terminé 2025-04-26 | sdk-partenaire-01 ; feedback des livres de recettes JS/Swift + contrôles d'intégrité du puente ISO. || partenaire-w1-07 | `DOCS-SORA-Preview-REQ-P07` | D'accord 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Terminé 2025-04-26 | sdk-partenaire-02 ; conformité approuvée le 2025-04-11, mentionnée dans les notes de Connect/télémétrie. |
| partenaire-w1-08 | `DOCS-SORA-Preview-REQ-P08` | D'accord 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Terminé 2025-04-26 | passerelle-ops-01 ; auditer le guide des opérations de la passerelle + flux anonyme du proxy Essayez-le. |

Complétez les horodatages de **Invitation envoyée** et **Ack** après avoir émis l'e-mail saillant.
Ancla los tiempos al calendrier UTC définis sur le plan W1.

## Points de contrôle de télémétrie

| Horodatage (UTC) | Tableaux de bord / sondes | Responsable | Résultat | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tout en vert | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcription de `npm run manage:tryit-proxy -- --stage preview-w1` | Opérations | Préparé | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Tableaux de bord de arriba + `probe:portal` | Docs/DevRel + Ops | Pré-invitation d'instantané, sans régression | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Tableaux de bord d'arriba + diff de latence du proxy Essayez-le | Responsable Docs/DevRel | Chèque de mitad de ola ok (0 alertes; latence Essayez-le p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Tableaux de bord + sonde de sortie | Docs/DevRel + liaison gouvernance | Instantané de sortie, cero alertas pendientes | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Les horaires de bureau (2025-04-13 -> 2025-04-25) sont agrégés avec les exportations NDJSON + PNG bas
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` avec numéros d'archives
`docs-preview-integrity-<date>.json` et les captures d'écran correspondantes.

## Journal des commentaires et des problèmes

Utilisez ce tableau pour reprendre les messages envoyés par les évaluateurs. Ajouter chaque entrée au ticket de GitHub/discuss
mas el formulario structuré capturado via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Référence | Sévérité | Responsable | État | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Faible | Docs-core-02 | Résultatau 2025-04-18 | Voir le libellé de la navigation de Try it + l'onglet de la barre latérale (`docs/source/sorafs/tryit.md` actualisé avec une nouvelle étiquette). |
| `docs-preview/w1 #2` | Faible | Docs-core-03 | Résultatau 2025-04-19 | Voir la capture d'écran actuelle de Try it + légende segun pedido del reviewer ; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informations | Responsable Docs/DevRel | Cerrado | Les commentaires restants seront en solo Q&A ; capturés dans le formulaire de commentaires de chaque partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Suite du contrôle des connaissances et des enquêtes1. Enregistrez les points du quiz (objet > = 90 %) pour chaque réviseur ; ajouter le fichier CSV exporté avec les objets d'invitation.
2. Récupérez les réponses qualitatives de l'enquête capturée avec le modèle de feedback et réfléchissez-y bas
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Agenda des appels de remédiation pour toute question de débarras de l'ombre et inscription dans ce fichier.

Les autres évaluateurs marquent >=94 % dans le contrôle des connaissances (CSV :
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Il n'est pas nécessaire d'appeler des appels de remédiation ;
les exportations de sondage pour chaque partenaire viven bajo
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventaire des artefacts

- Bundle de descripteur/somme de contrôle de l'aperçu : `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Reprise de sonde + vérification de lien : `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Journal de changement du proxy Essayez-le : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportations de télémétrie : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Bundle journal de télémétrie des heures de bureau: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportations de feedback + enquêtes : colocar carpetas por reviewer bajo
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV et reprise du contrôle des connaissances : `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Maintenir l'inventaire synchronisé avec le numéro du tracker. Adjunta hashs al copier artefactos al ticket de gobernanza
pour que les auditeurs vérifient les archives sans accès au shell.