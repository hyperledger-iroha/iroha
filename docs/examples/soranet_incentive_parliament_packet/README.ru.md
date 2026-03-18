---
lang: ru
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

# Пакет парламента стимулов Relay SoraNet

Этот bundle содержит artefacts, требуемые парламентом Sora для утверждения автоматических выплат relay (SNNet-7):

- `reward_config.json` - конфигурация движка вознаграждений, сериализуемая Norito, готовая к загрузке через `iroha app sorafs incentives service init`. `budget_approval_id` совпадает с hash, указанным в minutes governance.
- `shadow_daemon.json` - карта бенефициаров и bonds, используемая replay harness (`shadow-run`) и production daemon.
- `economic_analysis.md` - summary справедливости для shadow симуляции 2025-10 -> 2025-11.
- `rollback_plan.md` - операционный playbook для отключения автоматических выплат.
- Supporting artefacts: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Integrity Checks

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

Сравните digests со значениями, записанными в minutes парламента. Проверьте подпись shadow-run, как описано в
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Updating the Packet

1. Обновляйте `reward_config.json` при изменении весов вознаграждений, базовой выплаты или hash одобрения.
2. Перезапустите shadow симуляцию на 60 дней, обновите `economic_analysis.md` новыми выводами и закоммитьте JSON + detached подпись.
3. Представьте обновленный bundle парламенту вместе с exports дашбордов Observatory при запросе продления.
