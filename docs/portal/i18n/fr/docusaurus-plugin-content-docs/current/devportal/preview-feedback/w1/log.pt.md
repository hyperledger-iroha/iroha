---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-log
titre : Journal de rétroaction et de télémétrie W1
sidebar_label : journal W1
description : Liste regroupée, points de contrôle de télémétrie et notes des réviseurs pour la première onda de prévisualisation des colis.
---

Ce journal contient la liste des invités, les points de contrôle de télémétrie et les commentaires des évaluateurs pour
**aperçu des colis W1** qui accompagnent les tarefas de aceitacao em
[`preview-feedback/w1/plan.md`](./plan.md) et l'entrée du tracker de l'onda em
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Actualisez quand vous êtes convié à l'enviado,
un instantané de télémétrie à enregistrer ou un élément de feedback à trier pour les réviseurs de gouvernance possam
reproduire comme preuves sem perseguir tickets externos.

## Liste de la coorte| ID du partenaire | Ticket de sollicitation | NDA reçu | Envoyer une invitation (UTC) | Accusé de réception/première connexion (UTC) | Statut | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| partenaire-w1-01 | `DOCS-SORA-Preview-REQ-P01` | D'accord 03/04/2025 | 2025-04-12 15:00 | 2025-04-12 15:11 | Concluido 2025-04-26 | sorafs-op-01 ; mis en évidence la parité des documents de l'orchestrateur. |
| partenaire-w1-02 | `DOCS-SORA-Preview-REQ-P02` | D'accord 03/04/2025 | 2025-04-12 15:03 | 2025-04-12 15:15 | Concluido 2025-04-26 | sorafs-op-02 ; validou réticulations Norito/telemetria. |
| partenaire-w1-03 | `DOCS-SORA-Preview-REQ-P03` | D'accord 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | Concluido 2025-04-26 | sorafs-op-03 ; exécuter des exercices de basculement multi-sources. |
| partenaire-w1-04 | `DOCS-SORA-Preview-REQ-P04` | D'accord 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | Concluido 2025-04-26 | torii-int-01 ; révision du livre de recettes Torii `/v1/pipeline` + Essayez-le. |
| partenaire-w1-05 | `DOCS-SORA-Preview-REQ-P05` | D'accord 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | Concluido 2025-04-26 | torii-int-02 ; accompagner une capture d'écran actualisée Essayez-le (docs-preview/w1 #2). |
| partenaire-w1-06 | `DOCS-SORA-Preview-REQ-P06` | D'accord 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | Concluido 2025-04-26 | sdk-partenaire-01 ; feedback des livres de recettes JS/Swift + contrôles d'intégrité du pont ISO. || partenaire-w1-07 | `DOCS-SORA-Preview-REQ-P07` | D'accord 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | Concluido 2025-04-26 | sdk-partenaire-02 ; conformité approuvée le 11/04/2025, indiquée dans les notes de Connect/télémétrie. |
| partenaire-w1-08 | `DOCS-SORA-Preview-REQ-P08` | D'accord 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | Concluido 2025-04-26 | passerelle-ops-01 ; audit ou guia ops do gateway + fluxo anonimo do proxy Essayez-le. |

Prévoyez les horodatages de **Convite envoyé** et **Ack** comme l'e-mail dit pour l'envoi.
Encore les horaires dans le chronogramme UTC définis sur le plan W1.

## Points de contrôle de télémétrie

| Horodatage (UTC) | Tableaux de bord / sondes | Responsavel | Résultat | Artefato |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | Tout vert | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcription de `npm run manage:tryit-proxy -- --stage preview-w1` | Opérations | Mise en scène | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Tableaux de bord acima + `probe:portal` | Docs/DevRel + Ops | Pré-invitation d'instantané, sans régression | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Tableaux de bord acima + diff de latencia do proxy Essayez-le | Responsable Docs/DevRel | Checkpoint de meio approuvé (0 alertes; latence Essayez-le p95=410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Tableaux de bord acima + sonde de Saida | Docs/DevRel + liaison gouvernance | Instantané de Saida, zéro alerte pendante | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |En ce qui concerne les horaires de bureau (2025-04-13 -> 2025-04-25), estao agrupadas como exporte NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` avec noms d'archives
`docs-preview-integrity-<date>.json` et captures d'écran correspondantes.

## Journal des commentaires et des problèmes

Utilisez ce tableau pour reprendre les achats envoyés par les évaluateurs. Vincule chaque entrée sur le ticket GitHub/discuss
mais le formulaire estruturado capturé via
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Référence | Sévérité | Responsavel | Statut | Notes |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Faible | Docs-core-02 | Résolu 2025-04-18 | Afficher le texte de navigation de Try it + ancora de sidebar (`docs/source/sorafs/tryit.md` actualisé avec une nouvelle étiquette). |
| `docs-preview/w1 #2` | Faible | Docs-core-03 | Résolu 2025-04-19 | Capture d'écran Do Try it + legenda atualizados conforme pedido ; artefato `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informations | Responsable Docs/DevRel | Fechado | Commentaires restants pour quelques questions-réponses ; capturados no formulario de feedback de cada parceiro sob `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Accompagnement de contrôle de connaissances et d'enquêtes

1. Enregistrez-vous comme notes du quiz (méta >=90 %) pour chaque réviseur ; annexe ou CSV exportée vers les deux artefatos de convite.
2. Répondez aux réponses qualitatives de l'enquête capturée sans modèle de commentaires et envoyez-les
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Programme de remédiation pour que je puisse limiter et enregistrer ici.Tous les évaluateurs marquent >=94 % aucune vérification des connaissances (CSV :
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Nenhuma chamada de remédiation
foi nécessaire; exports de Survey para cada parceiro vivem em
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventaire des objets d'art

- Descripteur/somme de contrôle de l'aperçu du bundle : `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Résumé de la sonde + vérification du lien : `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de mudanca do proxy Essayez-le : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exportations de télémétrie : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Bundle journal de télémétrie des heures de bureau: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportations de feedback + enquête : colocar pastas por reviewer em
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- CSV et CV de vérification des connaissances : `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Mantenha o inventario synchronisé com o issue do tracker. Anexe hashs ao copier artefatos para o ticket de gouvernance
pour que les auditeurs vérifient les archives sans accès à Shell.