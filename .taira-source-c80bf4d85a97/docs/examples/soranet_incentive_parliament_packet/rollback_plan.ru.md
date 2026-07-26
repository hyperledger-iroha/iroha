---
lang: ru
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/rollback_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1add138352dabf2433c36c9abac24085af8c7e53dca8bc579d73b37680e470cf
source_last_modified: "2025-11-22T04:59:33.125102+00:00"
translation_last_reviewed: 2026-01-01
---

# План rollback для стимулов Relay

Используйте этот playbook, чтобы отключить автоматические выплаты relay, если governance
запрашивает остановку или если срабатывают guardrails телеметрии.

1. **Заморозить автоматизацию.** Остановите incentives daemon на каждом orchestrator хосте
   (`systemctl stop soranet-incentives.service` или эквивалентный контейнерный деплой) и подтвердите, что процесс больше не работает.
2. **Слить ожидающие инструкции.** Запустите
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   чтобы убедиться, что нет незавершенных payout инструкций. Архивируйте полученные Norito payloads для аудита.
3. **Отозвать governance approval.** Отредактируйте `reward_config.json`, установите
   `"budget_approval_id": null`, и заново разверните конфигурацию через
   `iroha app sorafs incentives service init` (или `update-config` при long-lived daemon). Payout engine теперь fail-closed с
   `MissingBudgetApprovalId`, поэтому daemon отказывается mint payouts, пока не будет восстановлен новый approval hash. Запишите git commit и SHA-256
   измененного конфига в журнал инцидента.
4. **Уведомить Sora Parliament.** Приложите drained payout ledger, shadow-run report и краткое описание инцидента. В minutes парламента должны быть указаны hash отозванной конфигурации и время остановки daemon.
5. **Проверка rollback.** Держите daemon выключенным, пока:
   - телеметрийные алерты (`soranet_incentives_rules.yml`) зеленые >=24 h,
   - отчет treasury reconciliation показывает нулевое число пропущенных переводов, и
   - парламент утверждает новый budget hash.

Когда governance заново выпустит budget approval hash, обновите `reward_config.json`
новым digest, перезапустите `shadow-run` на последней телеметрии,
и перезапустите incentives daemon.
