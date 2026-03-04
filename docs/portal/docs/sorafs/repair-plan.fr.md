---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/repair-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 302b74b4022656e57c2b876a8f15bf5301a593030a18ad1b93780061e5d783ef
source_last_modified: "2026-01-21T19:17:13.232211+00:00"
translation_last_reviewed: 2026-02-07
id: repair-plan
title: SoraFS Repair Automation & Auditor API
sidebar_label: Repair Automation
description: Governance policy, escalation lifecycle, and API expectations for SoraFS repair automation.
translator: machine-google-reviewed
---

:::note Source canonique
Miroirs `docs/source/sorafs_repair_plan.md`. Gardez les deux versions synchronisées jusqu'à ce que l'ensemble Sphinx soit retiré.
:::

## Cycle de vie des décisions de gouvernance
1. Les réparations escaladées créent un brouillon de proposition de barre oblique et ouvrent la fenêtre de litige.
2. Les électeurs de gouvernance soumettent des votes d'approbation/rejet pendant la fenêtre de contestation.
3. À `escalated_at_unix + dispute_window_secs`, la décision est calculée de manière déterministe : nombre minimum de votants, les approbations dépassent les rejets et le taux d'approbation atteint le seuil de quorum.
4. Les décisions approuvées ouvrent une fenêtre d'appel ; les appels enregistrés avant `approved_at_unix + appeal_window_secs` marquent la décision comme faisant l'objet d'un appel.
5. Des plafonds de pénalités s'appliquent à toutes les propositions ; les soumissions dépassant le plafond sont rejetées.

## Politique d'escalade de la gouvernance
La stratégie d'escalade provient de `governance.sorafs_repair_escalation` dans `iroha_config` et est appliquée pour chaque proposition de barre oblique de réparation.

| Paramètre | Par défaut | Signification |
|---------|---------|---------|
| `quorum_bps` | 6667 | Taux d’approbation minimum (points de base) parmi les votes comptés. |
| `minimum_voters` | 3 | Nombre minimum d'électeurs distincts requis pour prendre une décision. |
| `dispute_window_secs` | 86400 | Temps après l'escalade avant la finalisation des votes (secondes). |
| `appeal_window_secs` | 604800 | Délai après l'approbation pendant lequel les appels sont acceptés (secondes). |
| `max_penalty_nano` | 1 000 000 000 | Pénalité barre oblique maximale autorisée pour les escalades de réparations (nano-XOR). |

- Les propositions générées par le planificateur sont plafonnées à `max_penalty_nano` ; les soumissions de l’auditeur dépassant le plafond sont rejetées.
- Les enregistrements de vote sont stockés dans `repair_state.to` avec un ordre déterministe (tri `voter_id`) afin que tous les nœuds obtiennent le même horodatage de décision et le même résultat.