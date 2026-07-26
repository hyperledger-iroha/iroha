---
lang: ru
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-11-04T16:28:48.303168+00:00"
translation_last_reviewed: 2026-01-01
---

# Плейбук реакции на brownout / downgrade

1. **Обнаружение**
   - Срабатывает алерт `soranet_privacy_circuit_events_total{kind="downgrade"}` или приходит brownout webhook от governance.
   - Подтвердите через `kubectl logs soranet-relay` или systemd journal в течение 5 мин.

2. **Стабилизация**
   - Заморозьте guard rotation (`relay guard-rotation disable --ttl 30m`).
   - Включите direct-only override для затронутых клиентов
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Зафиксируйте текущий hash config compliance (`sha256sum compliance.toml`).

3. **Диагностика**
   - Соберите последний directory snapshot и пакет метрик relay:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - Отметьте глубину очереди PoW, счетчики throttling и всплески категорий GAR.
   - Определите, вызвано ли событие дефицитом PQ, override compliance или отказом relay.

4. **Эскалация**
   - Уведомьте governance bridge (`#soranet-incident`) с кратким описанием и hash пакета.
   - Откройте инцидентный тикет со ссылкой на алерт, включая timestamps и шаги mitigation.

5. **Восстановление**
   - После устранения причины включите rotation
     (`relay guard-rotation enable`) и отмените direct-only overrides.
   - Наблюдайте KPI 30 минут; убедитесь, что новые brownout не появляются.

6. **Постмортем**
   - Отправьте отчет об инциденте в течение 48 часов по шаблону governance.
   - Обновите runbooks при обнаружении нового режима отказа.
