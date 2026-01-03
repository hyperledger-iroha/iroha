---
lang: ru
direction: ltr
source: docs/source/nexus_operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 413953b2ca41875bb960be56995aa03dbaa58af4a30f927c24d1e8815c7da472
source_last_modified: "2025-11-08T16:26:57.335679+00:00"
translation_last_reviewed: 2026-01-01
---

# Ранбук операций Nexus (NX-14)

**Ссылка на roadmap:** NX-14 — документация Nexus и ранбуки операторов
**Статус:** Черновик 2026-03-24 — согласован с `docs/source/nexus_overview.md` и
онбординг-флоу в `docs/source/sora_nexus_operator_onboarding.md`.
**Аудитория:** сетевые операторы, SRE/on-call инженеры, координаторы governance.

Этот ранбук суммирует операционный жизненный цикл узлов Sora Nexus (Iroha 3).
Он не заменяет глубокую спецификацию (`docs/source/nexus.md`) или lane-специфичные
гайды (например, `docs/source/cbdc_lane_playbook.md`), но собирает конкретные
чеклисты, хуки телеметрии и требования к доказательствам, которые должны быть
выполнены до допуска или апгрейда узла.

## 1. Операционный жизненный цикл

| Этап | Чеклист | Доказательства |
|------|---------|---------------|
| **Pre-flight** | Проверить хэши/подписи артефактов, подтвердить `profile = "iroha3"`, подготовить шаблоны конфигурации. | Вывод `scripts/select_release_profile.py`, лог checksum, подписанный manifest bundle. |
| **Catalog alignment** | Обновить каталог lane + dataspace в `[nexus]`, политику маршрутизации и DA-пороги в соответствии с manifest, выпущенным советом. | Вывод `irohad --sora --config ... --trace-config`, сохраненный вместе с тикетом. |
| **Smoke & cutover** | Запустить `irohad --sora --config ... --trace-config`, выполнить CLI smoke-тест (например, `FindNetworkStatus`), проверить telemetry endpoints, затем запросить admission. | Лог smoke-теста + подтверждение тишины в Alertmanager. |
| **Steady state** | Мониторить дашборды/алерты, ротировать ключи по cadence governance, держать configs + runbooks в синке с ревизиями manifest. | Протоколы квартальных ревью, привязанные скриншоты дашбордов и IDs тикетов ротации. |

Подробные инструкции онбординга (включая замену ключей, примеры routing policy
и проверку release profile) находятся в
`docs/source/sora_nexus_operator_onboarding.md`. Ссылайтесь на этот документ при
изменении форматов артефактов или скриптов.

## 2. Управление изменениями и governance hooks

1. **Обновления релизов**
   - Следить за объявлениями в `status.md` и `roadmap.md`.
   - Каждый PR релиза должен включать заполненный чеклист из
     `docs/source/sora_nexus_operator_onboarding.md`.
2. **Изменения lane manifest**
   - Governance публикует подписанные manifest bundles через Space Directory.
   - Операторы проверяют подписи, обновляют записи каталога и архивируют
     manifests в `docs/source/project_tracker/nexus_config_deltas/`.
3. **Дельты конфигурации**
   - Любые изменения в `config/config.toml` требуют тикета с ссылкой на lane ID
     и alias dataspace.
   - Держать редактированную копию эффективного конфига в тикете при присоединении
     или апгрейде узла.
4. **Rollback drills**
   - Проводить квартальные rollback-репетиции (остановить узел, восстановить
     прошлый bundle, переиграть config, снова выполнить smoke). Результаты
     фиксировать в `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Согласования compliance**
   - Приватные/CBDC lanes должны получить compliance sign-off перед изменением
     DA policy или knobs редактирования телеметрии. См.
     `docs/source/cbdc_lane_playbook.md#governance-hand-offs`.

## 3. Телеметрия и покрытие SLO

Дашборды и правила алертов версионируются в `dashboards/` и описаны в
`docs/source/nexus_telemetry_remediation_plan.md`. Операторы ДОЛЖНЫ:

- Подписать PagerDuty/on-call цели на `dashboards/alerts/nexus_audit_rules.yml`
  и правила здоровья lanes в `dashboards/alerts/torii_norito_rpc_rules.yml`
  (покрывает транспорт Torii/Norito).
- Опубликовать следующие Grafana boards в операционный портал:
  - `nexus_lanes.json` (высота lanes, backlog, паритет DA).
  - `nexus_settlement.json` (латентность settlement, deltas казначейства).
  - `android_operator_console.json` / SDK дашборды, когда lane зависит от
    мобильной телеметрии.
- Держать OTEL exporters в соответствии с `docs/source/torii/norito_rpc_telemetry.md`
  при включенном Torii binary transport.
- Проводить telemetry remediation checklist не реже квартала (Section 5 в
  `docs/source/nexus_telemetry_remediation_plan.md`) и прикладывать заполненную
  форму к протоколам ops review.

### Ключевые метрики

| Метрика | Описание | Порог алерта |
|---------|----------|--------------|
| `nexus_lane_height{lane_id}` | Высота головы по lane; фиксирует застой валидаторов. | Алерт, если нет роста 3 последовательных слота. |
| `nexus_da_backlog_chunks{lane_id}` | Непроцессed DA chunks по lane. | Алерт выше лимита (default: 64 public, 8 private). |
| `nexus_settlement_latency_seconds{lane_id}` | Время между commit lane и глобальным settlement. | Алерт >900 мс P99 (public) или >1200 мс (private). |
| `torii_request_failures_total{scheme="norito_rpc"}` | Счетчик ошибок Norito RPC. | Алерт, если 5-минутный error ratio >2%. |
| `telemetry_redaction_override_total` | Overrides для редактирования телеметрии. | Немедленный алерт (Sev 2) и требование compliance тикета. |

## 4. Реагирование на инциденты

| Severity | Определение | Обязательные действия |
|----------|-------------|----------------------|
| **Sev 1** | Нарушение изоляции data-space, остановка settlement >15 мин, или коррупция голосования governance. | Пейдж Nexus Primary + Release Engineering + Compliance. Заморозить admission lanes, собрать метрики/логи, опубликовать коммуникацию об инциденте за 60 мин, оформить RCA за <=5 рабочих дней. |
| **Sev 2** | Backlog lane сверх SLA, слепая зона телеметрии >30 мин, провал rollout manifest. | Пейдж Nexus Primary + SRE, смягчение за 4 ч, оформить follow-up issues за 2 рабочих дня. |
| **Sev 3** | Некритичные регрессии (дрифт docs, ложный алерт). | Зафиксировать в трекере, запланировать исправление в спринте. |

Инцидентные тикеты должны включать:

1. Затронутые lane/data-space IDs и хэши manifest.
2. Timeline (UTC) с детекцией, mitigation, восстановлением и коммуникациями.
3. Метрики/скриншоты, подтверждающие детекцию.
4. Follow-up задачи (с владельцами/датами) и необходимость обновления automation/runbooks.

## 5. Доказательства и audit trail

- **Архив артефактов:** Хранить bundles, manifests и telemetry exports в
  `artifacts/nexus/<lane>/<date>/`.
- **Снапшоты конфига:** Редактированный `config.toml` + вывод `trace-config` для
  каждого релиза.
- **Связь с governance:** Ноты совета и подписанные решения, указанные в
  тикете онбординга или инцидента.
- **Telemetry exports:** Еженедельные snapshots Prometheus TSDB chunks, связанные
  с lane, прикрепленные к audit share минимум на 12 месяцев.
- **Версионирование ранбука:** Любое значимое изменение этого файла должно
  включать запись в changelog в `docs/source/project_tracker/nexus_config_deltas/README.md`,
  чтобы аудиторы могли отслеживать изменения требований.

## 6. Связанные ресурсы

- `docs/source/nexus_overview.md` — архитектура/высокоуровневое резюме.
- `docs/source/nexus.md` — полная техническая спецификация.
- `docs/source/nexus_lanes.md` — геометрия lanes.
- `docs/source/nexus_transition_notes.md` — roadmap миграции.
- `docs/source/cbdc_lane_playbook.md` — CBDC-специфичные политики.
- `docs/source/sora_nexus_operator_onboarding.md` — release/onboarding флоу.
- `docs/source/nexus_telemetry_remediation_plan.md` — guardrails телеметрии.

Держите эти ссылки в актуальном состоянии при продвижении NX-14 или при появлении
новых классов lanes, правил телеметрии или governance hooks.
