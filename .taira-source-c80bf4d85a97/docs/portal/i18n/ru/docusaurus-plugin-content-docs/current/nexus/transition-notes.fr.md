---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
заголовок: Примечания к переходу Nexus
описание: Miroir de `docs/source/nexus_transition_notes.md`, учитывает ограничения перехода к этапу B, календарь аудита и меры по смягчению последствий.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Примечания к переходу Nexus

Журнал Ce le travail restant de **Phase B — Nexus Transition Foundations** просто завершает контрольный список многополосного прорезывания. Завершите входы вехи в `roadmap.md` и просмотрите предварительные ссылки по параметрам B1-B4 в одном месте, где необходимо управление, SRE и ведущие SDK, участвующие в истинном источнике мемов.

## Порт и каденция

- Couvre les Audits Routed-Trace и les Guardrails de Telemetrie (B1/B2), l'ensemble de deltas de Configuration Approuve Par la Governance (B3) и les suivis de повторение многополосной связи (B4).
- Заменить временную ноту cadence qui vivait ici; После аудита в первом квартале 2026 года подробные сведения о взаимопонимании проживают в `docs/source/nexus_routed_trace_audit_report_2026q1.md`, а затем на этой странице поддерживаются календарь и регистрация мер по смягчению последствий.
- Меттез a jour les table apres chaque fenetre Routed-Trace, голосование по управлению или повторение прокола. Используя бугентные артефакты, отобразите новое размещение на этой странице для того, чтобы имеющиеся документы (статус, информационные панели, портальные SDK) были более стабильными.

## Снимок предварительных результатов (1-2 кв. 2026 г.)

| Рабочий поток | Преве | Владелец(и) | Статут | Заметки |
|------------|----------|----------|--------|-------|
| **B1 — Аудит трассировки маршрутизации** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Завершено (1 квартал 2026 г.) | Trois Fenetres d'Audit зарегистрированных лиц; Задержка TLS от `TRACE-CONFIG-DELTA` закрывает повторный запуск Q2. |
| **B2 – Ремонт телеметрии и ограждений** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | Завершить | Пакет оповещений, политика различий между ботами и хвостами OTLP (журнал `nexus.scheduler.headroom` + панель Grafana запаса) в ливрах; aucun отказ от права. |
| **B3 – Утверждение отклонений конфигурации** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Завершить | Голосуйте GOV-2026-03-19, зарегистрируйтесь; комплект продуктов питания, пакет телеметрии cite plus bas. |
| **B4 — Многополосное повторение прокола** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Завершено (2 квартал 2026 г.) | Повторный запуск Q2 для смягчения задержки TLS; Манифест валидатора + `.sha256` захватывает пляж слотов 912-936, начальное значение рабочей нагрузки `NEXUS-REH-2026Q2` и хэш профиля TLS регистрирует подвеску повторного запуска. |

## Календарь триместров аудита маршрутизированной трассировки| Идентификатор трассировки | Фенетре (UTC) | Результат | Заметки |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | Реусси | Вход в очередь P95 est reste bien en dessous de la cible <=750 мс. Требование к действию Aucune. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | Реусси | Хэши воспроизведения OTLP присоединяют `status.md`; la parite du diff bot SDK и подтверждение смещения нуля. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Резолу | Задержка TLS происходит во время повторного запуска Q2; Пакет телеметрии для `NEXUS-REH-2026Q2` зарегистрирует хэш профиля TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (voir `artifacts/nexus/tls_profile_rollout_2026q2/`) и не будет тормозить. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | Реусси | Начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`; пакет телеметрии + манифест/дайджест в `artifacts/nexus/rehearsals/2026q1/` (диапазон слотов 912–936) с повесткой дня в `artifacts/nexus/rehearsals/2026q2/`. |

Будущие триместры doivent ajouter de nouvelles lignes et deplacer les entrees terminees vers une Appe quand la table depasse le Trimestre Courant. Ссылка на этот раздел содержит трассировку связей или минут управления с использованием l'ancre `#quarterly-routed-trace-audit-schedule`.

## Смягчения и устранение невыполненной работы

| Товар | Описание | Владелец | Сибель | Устав / Примечания |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Завершите распространение профиля TLS с помощью подвески задержки `TRACE-CONFIG-DELTA`, захватите превью повторного запуска и зарегистрируйтесь для смягчения последствий. | @release-eng, @sre-core | Fenetre Routed-Trace, второй квартал 2026 г. | Закрытие — хэш профиля TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`, захват в `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; Повторите запуск и подтвердите отсутствие тормозов. |
| `TRACE-MULTILANE-CANARY` подготовка | Программист повторения Q2, объединение приборов и пакетов телеметрии и гарантия того, что SDK использует повторное использование действительного помощника. | @telemetry-ops, программа SDK | Обращение к планированию 30 апреля 2026 г. | Завершено — запас повестки дня в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` со слотом метаданных/рабочей нагрузкой; повторное использование ремня безопасности в трекере. |
| Вращение дайджеста пакета телеметрии | Исполнитель `scripts/telemetry/validate_nexus_telemetry_pack.py` до повторения/выпуска и регистрации файлов составляет часть отслеживания изменений конфигурации. | @telemetry-ops | Кандидат на выпуск по номиналу | Полный — `telemetry_manifest.json` + `.sha256` emis dans `artifacts/nexus/rehearsals/2026q1/` (диапазон слотов `912-936`, начальное число `NEXUS-REH-2026Q2`); дайджесты копий в трекере и индексе предварительных просмотров. |

## Интеграция пакета дельта-конфигурации- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` осталось каноническое резюме различий. Когда появятся новые `defaults/nexus/*.toml` или изменения в генезисе, вы увидите, как трекер прерывает работу, и вы сможете отразить точки, которые были указаны.
- Подписанные пакеты конфигурации содержат повторный пакет телеметрии. Пакет, действующий по номеру `scripts/telemetry/validate_nexus_telemetry_pack.py`, должен быть опубликован и опубликован с предварительным изменением конфигурации до тех пор, пока операторы, которые могут использовать артефакты, точно используют подвеску B4.
- Les Bundles Iroha 2 оставшихся без дорожек: les configs avec `nexus.enabled = false` rejettent maintenant les overrides de Lane/dataspace/routing sauf si le profil Nexus est active (`--sora`), не подключайте разделы `nexus.*` однополосные шаблоны.
- Запишите журнал голосования по управлению (GOV-2026-03-19) с помощью трекера и этой записи для мощного копировального аппарата будущих голосов в формате без повторного оформления ритуала одобрения.

## Повторение прокола

- `docs/source/runbooks/nexus_multilane_rehearsal.md` захватывает канарский план, список участников и этапы отката; Mettez a jour le runbook, когда изменилась топология полос или экспортеров телеметрии.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` список проверенных артефактов во время повторения 9 апреля и содержание примечаний/программы подготовки Q2. Добавляйте повторения фьючерсов или трекер мемов или специальные трекеры, чтобы следить за монотонностью.
- Публикация фрагментов сборщика OTLP и экспорта Grafana (voir `docs/source/telemetry.md`), а также группировка изменений экспортера; la mise a jour Q1 a porte la Taille de Lot a 256 echantillons, чтобы исключить запас по высоте.
- Многополосное обслуживание CI/tests в `integration_tests/tests/nexus/multilane_pipeline.rs` и в режиме рабочего процесса `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), замена эталонного устаревшего `pytests/nexus/test_multilane_pipeline.py`; сохраните хэш для `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) и синхронизируйте его с трекером для отслеживания пакетов повторений.

## Цикл выполнения дорожек

- Планы цикла жизни дорожек во время выполнения действительны для обслуживания привязок пространства данных и прерывания, а также согласования Kura/stockage и Paliers echoue, без изменения каталога. Помощники удаляют кэши-ретрансляторы для выхода на пенсию, чтобы синтезировать эти реестры слияний и повторно не использовать устаревшие доказательства.
- Применение планов с помощью помощников конфигурации/жизненного цикла Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) для добавления/удаления дорожек без восстановления; маршрутизация, снимки TEU и реестры манифестов автоматически пополняются после повторного выполнения плана.
- Руководство оператора: когда отображается план, проверяйте пустые пространства данных или корни хранилища, которые невозможно создать (многоуровневый холодный корень/репертуары Kura по полосе). Corrigez les chemins de base et reessayez; Планы повторно используются для отображения различий в полосе телеметрии/пространстве данных, чтобы панели мониторинга отражали новую топологию.

## Телеметрия NPoS и предотвращение противодавленияРетро-повторение прокола Фаза B требует захвата телеметрических данных, определяющих, что кардиостимулятор NPoS и кушетки для сплетен остаются в пределах пределов противодавления. Интеграция в `integration_tests/tests/sumeragi_npos_performance.rs` позволяет выполнять эти сценарии и получать резюме в формате JSON (`sumeragi_baseline_summary::<scenario>::...`) при поступлении новых показателей. Местоположение Ланцеса:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Определения `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` или `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` для исследования топологий и напряжений; les valeurs по умолчанию, отражающие профиль коллекционеров 1 s/`k=3`, используйте в B4.

| Сценарий/тест | Кувертюр | Телеметрия |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Блокируйте 12 раундов с блокировкой времени повторения для регистратора конвертов задержки EMA, файловых менеджеров и датчиков избыточной отправки перед сериализатором пакета предварительных сообщений. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Кроме того, файл транзакций служит гарантией того, что отсрочки доступа являются активными по факту и что файл экспортируется для счетчиков емкости/насыщения. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Изменяется дрожание кардиостимулятора и тайм-ауты, которые показывают, что диапазон +/-125 промилле является аппликацией. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Большие полезные нагрузки RBC ограничены мягкими/жесткими ограничениями хранилища для отслеживания ежедневных сеансов и обработки байтов, возврата и стабилизации без отключения хранилища. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Сила повторных передач для того, чтобы датчики соотношения избыточной отправки и счетчики сборщиков-на-цели опережали время, доказывая, что телеметрия требуется, как ретро, ​​​​эту сквозную связь. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Laisse tomber des chunks a Intervals Deterministes for verifier que les moniteurs de backlog signalent des fautes au aleu de silenceusement les payloads. | И18НИ00000105Х, И18НИ00000106Х, И18НИ00000107Х. |

Работайте с линиями JSON, подключая их к жгуту с помощью Prometheus, чтобы захватить подвеску выполнения, чтобы управление требовало превентивных мер, соответствующих тревогам противодавления в топологии повторения.

## Контрольный список дел на день

1. Добавьте новые окна, проложенные по маршруту, и удалите старые предыдущие триместры турнира.
2. Отметьте таблицу смягчения последствий после использования Alertmanager, мем, если действие состоит из билета.
3. При изменении конфигурации, внесите изменения в трекер, эту заметку и список дайджестов пакета телеметрии в запросе на включение мема.
4. Liez ici рекламирует новый артефакт повторения/телеметрии для того, чтобы фьючерсы в течение дня не были связаны с дорожной картой, мощный справочный материал и один документ вместо специальных примечаний рассеивается.

## Индекс превью| Актив | Размещение | Заметки |
|-------|----------|-------|
| Отчет об аудите маршрутизации (1 квартал 2026 г.) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник для превью, Фаза B1; мируар для порта су `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Трекер изменений конфигурации | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Содержит резюме различий TRACE-CONFIG-DELTA, инициалы рецензентов и журнал голосования GOV-2026-03-19. |
| План восстановления телеметрии | `docs/source/nexus_telemetry_remediation_plan.md` | Пакет документов, информация о партии OTLP и ограничения бюджета экспорта относятся к категории B2. |
| Многополосный трекер повторения | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Список артефактов проверяется во время повторения 9 апреля, валидатор манифеста/дайджеста, примечания/повестка дня Q2 и предотвращение отката. |
| Манифест/дайджест пакета телеметрии (последняя версия) | И18НИ00000113X (+ `.sha256`) | Зарегистрируйте пляж 912-936, начальное значение `NEXUS-REH-2026Q2` и хеши артефактов для пакетов управления. |
| Манифест профиля TLS | И18НИ00000116X (+ `.sha256`) | Hash du profil TLS утверждает подвеску захвата и повторный запуск Q2; Citez-le в приложениях маршрутизированной трассировки. |
| Программа TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Примечания к планированию для повторения Q2 (окно, диапазон слотов, начальное значение рабочей нагрузки, действия владельцев). |
| Сборник повторений прорезывания | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Контрольный список операций для постановки -> выполнение -> откат; Mettre a jour quand la топология полос движения или советов экспортеров изменилась. |
| Валидатор пакета телеметрии | `scripts/telemetry/validate_nexus_telemetry_pack.py` | Ссылка на CLI в стиле ретро B4; Архивируйте дайджесты с трекером, чтобы изменить пакет. |
| Многополосная регрессия | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Действуйте `nexus.enabled = true` для многоканальных конфигураций, сохраняйте хэши каталога Sora и обеспечивайте поля Kura/merge-log по полосе (`blocks/lane_{id:03}_{slug}`) через `ConfigLaneRouter` перед публикацией дайджестов артефактов. |