---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/operations.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-operations
title: Runbook по операциям Nexus
description: Практичный полевой обзор рабочего процесса оператора Nexus, отражающий `docs/source/nexus_operations.md`.
---

Используйте эту страницу как быстрый справочник к `docs/source/nexus_operations.md`. Она концентрирует операционный чек-лист, точки управления изменениями и требования к телеметрии, которым должны следовать операторы Nexus.

## Чек-лист жизненного цикла

| Этап | Действия | Доказательства |
|-------|--------|----------|
| Предстарт | Проверить хэши/подписи релиза, подтвердить `profile = "iroha3"` и подготовить шаблоны конфигурации. | Вывод `scripts/select_release_profile.py`, журнал checksum, подписанный bundle манифестов. |
| Выравнивание каталога | Обновить каталог `[nexus]`, политику маршрутизации и пороги DA по манифесту совета, затем сохранить `--trace-config`. | Вывод `irohad --sora --config ... --trace-config`, сохраненный с тикетом onboarding. |
| Smoke & cutover | Запустить `irohad --sora --config ... --trace-config`, выполнить CLI smoke (`FindNetworkStatus`), проверить экспорт телеметрии и запросить admission. | Лог smoke-test + подтверждение Alertmanager. |
| Штатный режим | Следить за dashboards/alerts, ротировать ключи по графику governance и синхронизировать configs/runbooks при изменении манифестов. | Протоколы квартального обзора, скриншоты дашбордов, ID тикетов ротации. |

Подробный onboarding (замена ключей, шаблоны маршрутизации, шаги профиля релиза) остается в `docs/source/sora_nexus_operator_onboarding.md`.

## Управление изменениями

1. **Обновления релиза** - следите за объявлениями в `status.md`/`roadmap.md`; прикладывайте чек-лист onboarding к каждому PR релиза.
2. **Изменения манифестов lane** - проверяйте подписанные bundles из Space Directory и архивируйте их в `docs/source/project_tracker/nexus_config_deltas/`.
3. **Дельты конфигурации** - каждое изменение `config/config.toml` требует тикет с ссылкой на lane/data-space. Сохраняйте редактированную копию эффективной конфигурации при join/upgrade узлов.
4. **Тренировки rollback** - ежеквартально репетируйте stop/restore/smoke; фиксируйте результаты в `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Согласования compliance** - private/CBDC lanes должны получить одобрение compliance перед изменением политики DA или knobs редактирования телеметрии (см. `docs/source/cbdc_lane_playbook.md`).

## Телеметрия и SLOs

- Дашборды: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, а также SDK-специфичные виды (например, `android_operator_console.json`).
- Алерты: `dashboards/alerts/nexus_audit_rules.yml` и правила Torii/Norito transport (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Метрики для мониторинга:
  - `nexus_lane_height{lane_id}` - алерт при отсутствии прогресса три слота.
  - `nexus_da_backlog_chunks{lane_id}` - алерт выше порогов для lane (по умолчанию 64 public / 8 private).
  - `nexus_settlement_latency_seconds{lane_id}` - алерт когда P99 превышает 900 ms (public) или 1200 ms (private).
  - `torii_request_failures_total{scheme="norito_rpc"}` - алерт если 5-минутная доля ошибок >2%.
  - `telemetry_redaction_override_total` - Sev 2 немедленно; убедитесь, что overrides имеют тикеты compliance.
- Выполняйте чек-лист телеметрии из [Nexus telemetry remediation plan](./nexus-telemetry-remediation) минимум ежеквартально и прикладывайте заполненную форму к заметкам операционного обзора.

## Матрица инцидентов

| Severity | Определение | Ответ |
|----------|------------|----------|
| Sev 1 | Нарушение изоляции data-space, остановка settlement >15 минут или порча голосования governance. | Пейджинг Nexus Primary + Release Engineering + Compliance, заморозить admission, собрать артефакты, выпустить коммуникации <=60 минут, RCA <=5 рабочих дней. |
| Sev 2 | Нарушение SLA backlog lane, слепая зона телеметрии >30 минут, провал rollout манифеста. | Пейджинг Nexus Primary + SRE, смягчение <=4 часа, оформить follow-ups в течение 2 рабочих дней. |
| Sev 3 | Не блокирующий дрейф (docs, alerts). | Зафиксировать в tracker и запланировать исправление в спринте. |

Тикеты инцидентов должны фиксировать затронутые IDs lane/data-space, хэши манифестов, таймлайн, поддерживающие метрики/логи, и follow-up задачи/ответственных.

## Архив доказательств

- Храните bundles/manifestes/экспорты телеметрии в `artifacts/nexus/<lane>/<date>/`.
- Сохраняйте редактированные configs + вывод `--trace-config` для каждого релиза.
- Прикладывайте протоколы совета + подписанные решения при внесении изменений конфигурации или манифеста.
- Храните еженедельные snapshots Prometheus по метрикам Nexus в течение 12 месяцев.
- Фиксируйте правки runbook в `docs/source/project_tracker/nexus_config_deltas/README.md`, чтобы аудиторы знали, когда менялись обязанности.

## Связанные материалы

- Обзор: [Nexus overview](./nexus-overview)
- Спецификация: [Nexus spec](./nexus-spec)
- Геометрия lane: [Nexus lane model](./nexus-lane-model)
- Переход и routing shims: [Nexus transition notes](./nexus-transition-notes)
- Onboarding операторов: [Sora Nexus operator onboarding](./nexus-operator-onboarding)
- Ремедиация телеметрии: [Nexus telemetry remediation plan](./nexus-telemetry-remediation)
