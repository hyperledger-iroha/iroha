---
lang: he
direction: rtl
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ea2984da3aeda91bc196528b345efb7e0acf47cff7f350b9fbba8ada72dd6a5b
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
id: developer-deployment
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Канонический источник
:::

# Заметки по развертыванию

Пайплайн упаковки SoraFS усиливает детерминизм, поэтому переход из CI в продакшен в основном требует операционных guardrails. Используйте этот чеклист при rollout инструментария на реальные gateways и storage providers.

## Предварительная проверка

- **Выравнивание реестра** — убедитесь, что профили chunker и manifests ссылаются на одинаковый кортеж `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`).
- **Политика admission** — проверьте подписанные provider adverts и alias proofs, необходимые для `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook pin registry** — держите `docs/source/sorafs/runbooks/pin_registry_ops.md` под рукой для сценариев восстановления (ротация alias, сбои репликации).

## Конфигурация окружения

- Gateways должны включить endpoint proof streaming (`POST /v1/sorafs/proof/stream`), чтобы CLI мог выпускать телеметрические сводки.
- Настройте политику `sorafs_alias_cache` с помощью значений по умолчанию из `iroha_config` или CLI helper (`sorafs_cli manifest submit --alias-*`).
- Передавайте stream tokens (или учетные данные Torii) через безопасный secret manager.
- Включите телеметрические exporters (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) и отправляйте их в ваш стек Prometheus/OTel.

## Стратегия rollout

1. **Blue/green manifests**
   - Используйте `manifest submit --summary-out` для архивирования ответов каждого rollout.
   - Следите за `torii_sorafs_gateway_refusals_total`, чтобы рано ловить несовместимость возможностей.
2. **Проверка proofs**
   - Считайте сбои в `sorafs_cli proof stream` блокерами развертывания; всплески латентности часто означают throttling провайдера или неверно настроенные tiers.
   - `proof verify` должен входить в smoke test после pin, чтобы удостовериться, что CAR у провайдеров все еще совпадает с digest манифеста.
3. **Dashboards телеметрии**
   - Импортируйте `docs/examples/sorafs_proof_streaming_dashboard.json` в Grafana.
   - Добавьте панели для здоровья pin registry (`docs/source/sorafs/runbooks/pin_registry_ops.md`) и статистики chunk range.
4. **Включение multi-source**
   - Следуйте этапным шагам rollout из `docs/source/sorafs/runbooks/multi_source_rollout.md` при включении orchestrator и архивируйте артефакты scoreboard/телеметрии для аудитов.

## Обработка инцидентов

- Следуйте путям эскалации в `docs/source/sorafs/runbooks/`:
  - `sorafs_gateway_operator_playbook.md` для outage gateway и исчерпания stream-token.
  - `dispute_revocation_runbook.md`, когда возникают споры репликации.
  - `sorafs_node_ops.md` для обслуживания на уровне нод.
  - `multi_source_rollout.md` для override оркестратора, blacklisting peers и поэтапных rollouts.
- Записывайте сбои proofs и аномалии латентности в GovernanceLog через существующие PoR tracker API, чтобы governance могла оценить производительность провайдеров.

## Следующие шаги

- Интегрируйте автоматизацию orchestrator (`sorafs_car::multi_fetch`), когда появится multi-source fetch orchestrator (SF-6b).
- Отслеживайте обновления PDP/PoTR в рамках SF-13/SF-14; CLI и docs будут развиваться, чтобы отображать дедлайны и выбор tiers, когда эти proofs стабилизируются.

Объединив эти заметки по развертыванию с quickstart и CI рецептами, команды смогут перейти от локальных экспериментов к production-grade SoraFS pipelines с повторяемым и наблюдаемым процессом.
