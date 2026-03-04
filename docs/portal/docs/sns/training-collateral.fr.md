---
lang: fr
direction: ltr
source: docs/portal/docs/sns/training-collateral.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e992cea8d0c835b30bd9e91860f6b6f87bed79a2c25bd6d0544639685834f80c
source_last_modified: "2025-11-10T17:37:35.921927+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: training-collateral
title: Supports de formation SNS
description: Programme, workflow de localisation et capture des preuves d’annexes exigées par SN-8.
---

> Reflète `docs/source/sns/training_collateral.md`. Utilisez cette page pour briefer les équipes registre, DNS, guardians et finance avant chaque lancement de suffixe.

## 1. Instantané du programme

| Parcours | Objectifs | Pré‑lectures |
|-------|------------|-----------|
| Ops registraires | Soumettre les manifests, surveiller les dashboards KPI, escalader les erreurs. | `sns/onboarding-kit`, `sns/kpi-dashboard`. |
| DNS & gateway | Appliquer les squelettes de résolveur, répéter gels/rollback. | `sorafs/gateway-dns-runbook`, direct-mode policy samples. |
| Guardians & conseil | Exécuter les litiges, mettre à jour les addenda de gouvernance, consigner les annexes. | `sns/governance-playbook`, steward scorecards. |
| Finance & analytics | Capturer les métriques ARPU/bulk, publier les lots d’annexes. | `finance/settlement-iso-mapping`, KPI dashboard JSON. |

### Flux des modules

1. **M1 — Orientation KPI (30 min) :** parcourir les filtres de suffixe, exports et compteurs de gel. Livrable : instantanés PDF/CSV avec digest SHA-256.
2. **M2 — Cycle de vie du manifest (45 min) :** construire et valider les manifests de registre, générer des squelettes de résolveur via `scripts/sns_zonefile_skeleton.py`. Livrable : diff git montrant le squelette + preuve GAR.
3. **M3 — Exercices de litige (40 min) :** simuler gel + appel guardian, capturer les logs CLI sous `artifacts/sns/training/<suffix>/<cycle>/logs/`.
4. **M4 — Capture d’annexes (25 min) :** exporter le JSON du dashboard et exécuter :

   ```bash
   cargo xtask sns-annex      --suffix <suffix>      --cycle <cycle>      --dashboard artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --dashboard-artifact artifacts/sns/regulatory/<suffix>/<cycle>/sns_suffix_analytics.json      --output docs/source/sns/reports/<suffix>/<cycle>.md      --regulatory-entry docs/source/sns/regulatory/<memo>.md      --portal-entry docs/portal/docs/sns/regulatory/<memo-id>.md
   ```

   Livrable : Markdown d’annexe mis à jour + mémo réglementaire + blocs portail.

## 2. Workflow de localisation

- Langues : `ar`, `es`, `fr`, `ja`, `pt`, `ru`, `ur`.
- Chaque traduction vit à côté du fichier source (`docs/source/sns/training_collateral.<lang>.md`). Mettez à jour `status` + `translation_last_reviewed` après rafraîchissement.
- Les assets par langue vont sous `artifacts/sns/training/<suffix>/<lang>/<cycle>/` (slides/, workbooks/, recordings/, logs/).
- Exécutez `python3 scripts/sync_docs_i18n.py --lang <code>` après édition de la source anglaise pour que les traducteurs voient le nouveau hash.

### Checklist de livraison

1. Mettre à jour le stub de traduction (`status: complete`) une fois localisé.
2. Exporter les slides en PDF et les déposer dans le répertoire `slides/` par langue.
3. Enregistrer un walkthrough KPI ≤10 min ; le lier depuis le stub de langue.
4. Créer un ticket de gouvernance tagué `sns-training` contenant les digests des slides/workbook, les liens d’enregistrement et les preuves d’annexe.

## 3. Assets de formation

- Plan des slides : `docs/examples/sns_training_template.md`.
- Modèle de workbook : `docs/examples/sns_training_workbook.md` (un par participant).
- Invitations + rappels : `docs/examples/sns_training_invite_email.md`.
- Formulaire d’évaluation : `docs/examples/sns_training_eval_template.md` (réponses archivées sous `artifacts/sns/training/<suffix>/<cycle>/feedback/`).

## 4. Calendrier & métriques

| Cycle | Fenêtre | Métriques | Notes |
|-------|--------|---------|-------|
| 2026‑03 | Après revue KPI | Participation %, digest d’annexe consigné | `.sora` + `.nexus` cohorts |
| 2026‑06 | Avant GA `.dao` | Préparation finance ≥90 % | Inclure la mise à jour de politique |
| 2026‑09 | Expansion | Exercice de litige <20 min, SLA d’annexes ≤2 jours | Aligner avec les incitations SN-7 |

Capturer les retours anonymes dans `docs/source/sns/reports/sns_training_feedback.md` afin d’améliorer la localisation et les labs pour les cohortes suivantes.

