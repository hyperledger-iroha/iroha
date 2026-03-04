---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-log
titre : W1 فيڈبیک اور ٹیلیمیٹری لاگ
sidebar_label : W1 ici
la description: پہلی پارٹنر preview wave کے لئے مجموعی roster, ٹیلیمیٹری checkpoints, اور reviewers نوٹس۔
---

Voici **Aperçu de la version W1** et liste d'invitations, points de contrôle et commentaires des évaluateurs.
et [`preview-feedback/w1/plan.md`](./plan.md) pour les tâches d'acceptation et le suivi des vagues
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md) کے ساتھ وابستہ ہیں۔ جب دعوت ارسال ہو،
ٹیلیمیٹری snapshot ریکارڈ ہو، یا feedback item triage ہو تو اسے اپ ڈیٹ کریں تاکہ gouvernance reviewers
بغیر بیرونی ٹکٹس کے پیچھے گئے ثبوت replay کر سکیں۔

## Liste des cohortes| ID du partenaire | Demander un billet | NDA | Invitation envoyée (UTC) | Accusé de réception/première connexion (UTC) | Statut | Remarques |
| --- | --- | --- | --- | --- | --- | --- |
| partenaire-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ مکمل 2025-04-26 | sorafs-op-01 ; Orchestrator documente les preuves de parité پر فوکس۔ |
| partenaire-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ مکمل 2025-04-26 | sorafs-op-02 ; Norito/liaison croisée télémétrie |
| partenaire-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04/04/2025 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ مکمل 2025-04-26 | sorafs-op-03 ; exercices de basculement multi-sources |
| partenaire-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04/04/2025 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ مکمل 2025-04-26 | torii-int-01 ; Torii `/v1/pipeline` + Essayez-le, critique du livre de recettes۔ |
| partenaire-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ مکمل 2025-04-26 | torii-int-02 ; Essayez-le, mise à jour de capture d'écran ici (docs-preview/w1 #2). |
| partenaire-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ مکمل 2025-04-26 | sdk-partenaire-01 ; Commentaires sur les livres de recettes JS/Swift + vérifications de l'intégrité du pont ISO۔ |
| partenaire-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ مکمل 2025-04-26 | sdk-partenaire-02 ; conformité 2025-04-11 کو clear، Notes de connexion/télémétrie پر فوکس۔ || partenaire-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ مکمل 2025-04-26 | passerelle-ops-01 ; Audit du guide des opérations de passerelle + flux anonyme Essayez-le proxy۔ |

**Invitation envoyée** et **Ack** et horodatages et réponse à votre courrier électronique sortant.
Les horaires W1 sont actuellement disponibles selon le calendrier UTC.

## Points de contrôle de télémétrie

| Horodatage (UTC) | Tableaux de bord / sondes | Propriétaire | Résultat | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ Tout vert | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcription `npm run manage:tryit-proxy -- --stage preview-w1` | Opérations | ✅ Mise en scène | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Tableaux de bord اوپر + `probe:portal` | Docs/DevRel + Ops | ✅ Instantané de pré-invitation, pas de régressions | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Tableaux de bord اوپر + Essayez-le diff de latence proxy | Responsable Docs/DevRel | ✅ Vérification à mi-parcours réussie (0 alerte ; Essayez-le, latence p95 = 410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Tableaux de bord اوپر + sonde de sortie | Docs/DevRel + liaison gouvernance | ✅ Quittez l'instantané, aucune alerte en attente | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

Échantillons d'heures de bureau (2025-04-13 -> 2025-04-25) Exportations NDJSON + PNG par ici
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` میں موجود ہیں، فائل نام
`docs-preview-integrity-<date>.json` et captures d'écran ci-dessous

## Commentaires sur le journal des problèmesVoici les conclusions des examinateurs et les résultats des critiques. ہر entrée sur GitHub/discuss
billet اور اس forme structurée سے جوڑیں جو
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) کے ذریعے ھرا گیا۔

| Référence | Gravité | Propriétaire | Statut | Remarques |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Faible | Docs-core-02 | ✅ Résolu le 18/04/2025 | Essayez-le avec le libellé de navigation + l'ancre de la barre latérale (`docs/source/sorafs/tryit.md` avec l'étiquette pour la barre latérale). |
| `docs-preview/w1 #2` | Faible | Docs-core-03 | ✅ Résolu le 19/04/2025 | Essayez-le capture d'écran + critique de légende کی درخواست پر اپ ڈیٹ؛ artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informations | Responsable Docs/DevRel | 🟢 Fermé | باقی تبصرے صرف Q&A تھے؛ Utilisez le formulaire de commentaires pour votre demande `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Vérification des connaissances et suivi des enquêtes

1. Les résultats du quiz du réviseur ریکارڈ کریں (cible >=90 %) ; CSV exporté et inviter des artefacts à joindre
2. formulaire de commentaires et réponses à l'enquête qualitative ci-dessous
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/` miroir miroir
3. seuil سے نیچے والوں کے لئے planning des appels de résolution کریں اور انہیں اس فائل میں log کریں۔

Les évaluateurs ont vérifié leurs connaissances > = 94 % (CSV :
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). Appels de remédiation
L'enquête sur les exportations est la suivante :
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Inventaire des artefacts- Aperçu du descripteur/somme de contrôle : `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Récapitulatif sonde + vérification de liaison : `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Essayez-le journal des modifications du proxy : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exports télémétrie : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Forfait télémétrie journalier aux heures de bureau : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportations de commentaires + d'enquêtes : dossiers spécifiques aux évaluateurs
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` en cours
- Vérification des connaissances CSV et résumé : `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Problème d'inventaire et de suivi et synchronisation des données Il s'agit d'artefacts et d'un ticket de gouvernance et d'un hachage attaché.
Les auditeurs vérifient l'accès au shell et vérifient les paramètres.