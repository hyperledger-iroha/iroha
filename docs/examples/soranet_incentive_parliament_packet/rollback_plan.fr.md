---
lang: fr
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

# Plan de rollback des incentives relay

Utilisez ce playbook pour desactiver les paiements automatiques de relay si governance
demande un halt ou si les guardrails de telemetrie se declenchent.

1. **Geler l'automatisation.** Arretez le daemon d'incentives sur chaque host orchestrator
   (`systemctl stop soranet-incentives.service` ou l'equivalent container) et confirmez que le processus ne tourne plus.
2. **Drenner les instructions en attente.** Executez
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   pour garantir qu'il n'y a pas d'instructions de payout en attente. Archivez les payloads Norito resultants pour audit.
3. **Revoquer l'approbation governance.** Editez `reward_config.json`, mettez
   `"budget_approval_id": null`, puis redeployez la configuration via
   `iroha app sorafs incentives service init` (ou `update-config` si un daemon long-terme tourne). Le moteur de payout echoue maintenant en ferme avec
   `MissingBudgetApprovalId`, donc le daemon refuse de minter des payouts tant qu'un nouveau hash d'approbation n'est pas restaure. Enregistrez le commit git et le SHA-256 de la config modifiee dans le log d'incident.
4. **Notifier le Parlement Sora.** Joignez le ledger de payouts draine, le rapport shadow-run et un resume court de l'incident. Les minutes du Parlement doivent noter le hash de la configuration revoquee et l'heure d'arret du daemon.
5. **Validation rollback.** Gardez le daemon desactive jusqu'a:
   - les alertes de telemetrie (`soranet_incentives_rules.yml`) soient vertes pendant >=24 h,
   - le rapport de reconciliation de tresorerie montre zero transferts manquants, et
   - le Parlement approuve un nouveau hash de budget.

Une fois que governance re-emet un hash d'approbation budget, mettez a jour `reward_config.json`
avec le nouveau digest, relancez la commande `shadow-run` sur la telemetrie la plus recente,
et redemarrez le daemon d'incentives.
