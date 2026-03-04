---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
Название: ملاحظات انتقال Nexus
описание: Установленный `docs/source/nexus_transition_notes.md`, установленный в фазе B, для завершения процесса.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Дополнительная информация Nexus

يتتبع هذا السجل العمل المتبقي **Phase B - Nexus Transition Foundations** حتى تكتمل قائمة فحص اطلاق الـ многополосный. В 2009 году он был выбран для `roadmap.md` и был установлен на B1-B4 в Китае. Вы можете использовать его для создания приложений SRE и SDK для создания дополнительных приложений.

## النطاق والوتيرة

- Прокладка трассировки маршрутизированных ограждений и ограждений (B1/B2), а также дельта-зондирование. На переулках (B3) и на автомагистралях (B4).
- Написано в журнале "Канада" Дата выпуска: первый квартал 2026 г., в календаре `docs/source/nexus_routed_trace_audit_report_2026q1.md`, в январе 2026 г. Он сказал, что это не так.
- Вы можете настроить маршрутизированную трассировку и выполнить поиск в режиме реального времени. عندما تتحرك artefacts, اعكس الموقع الجديد داخل هذه الصفحة كي تتمكن الوثائق Настройки (статус, информационные панели, SDK بوابات) в разделе «Обзор».

## لقطة ادلة (1-2 кварталы 2026 г.)

| مسار العمل | الادلة | الملاك | حالة | ملاحظات |
|------------|----------|----------|--------|-------|
| **B1 — Аудит трассировки маршрутизации** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | Начало (1 квартал 2026 г.) | Он сказал Сэлау, что он Для этого используется TLS для `TRACE-CONFIG-DELTA` и повторный запуск Q2. |
| **B2 – Исправление телеметрии и ограждения** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مكتمل | Пакет предупреждений позволяет использовать diff-бот и использовать OTLP (журнал `nexus.scheduler.headroom` + запас по запасу для Grafana). Ла Сетт отказывается от участия. |
| **B3 – Утверждения изменений конфигурации** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم تسجيل تصويت GOV-2026-03-19; Воспользуйтесь пакетом телеметрии. |
| **B4 – Репетиция запуска многополосного движения** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Начало (2 квартал 2026 г.) | Повторный показ второго квартала на TLS; Манифест валидатора + `.sha256` включает слоты 912–936 и начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`, хеш-код TLS и повторный запуск. |

## Вызов маршрутизированной трассировки

| Идентификатор трассировки | США (UTC) | Новости | ملاحظات |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ناجح | Допуск в очередь P95 в случае задержки <= 750 мс. لا يلزم اي اجراء. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ناجح | Для хэшей воспроизведения OTLP: `status.md`; Обеспечивает четность, используя SDK diff-бот и дрейф. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تم الحل | В ходе TLS был повторен Q2; Пакет телеметрии с хеш-кодом `NEXUS-REH-2026Q2` и TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/`) для проверки. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ناجح | Начальное значение рабочей нагрузки `NEXUS-REH-2026Q2`; пакет телеметрии + манифест/дайджест для `artifacts/nexus/rehearsals/2026q1/` (диапазон слотов 912–936) для повестки дня для `artifacts/nexus/rehearsals/2026q2/`. |Он сказал, что это будет означать, что он хочет, чтобы он сделал это. Он был убит в 2008 году. Вы можете использовать Routed-Trace в режиме поиска маршрутизации. `#quarterly-routed-trace-audit-schedule`.

## Отставание в работе

| عنصر | الوصف | المالك | الهدف | الحالة / الملاحظات |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | После запуска TLS был установлен `TRACE-CONFIG-DELTA`, был выполнен повторный запуск, а затем снова. | @release-eng, @sre-core | Начало маршрутизированной трассировки, второй квартал 2026 г. | Код - хеш в TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` для `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; Будет повтор фильма "Вечеринка". |
| `TRACE-MULTILANE-CANARY` подготовка | جدولة تدريب Q2, ارفاق приспособления بحزمة التيليمتري وضمان اعادة استخدام SDK ремни безопасности المعتمد. | @telemetry-ops, программа SDK | اجتماع التخطيط 30 апреля 2026 г. | مكتمل - повестка дня для `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` в слоте метаданных/рабочей нагрузке; Воспользуйтесь ремнем безопасности и трекером. |
| Вращение дайджеста пакета телеметрии | Обновление `scripts/telemetry/validate_nexus_telemetry_pack.py` в разделе/выпуске содержит дайджесты обновлений трекера и изменений конфигурации. | @telemetry-ops | لكل кандидат на выпуск | Соединение - `telemetry_manifest.json` + `.sha256` для `artifacts/nexus/rehearsals/2026q1/` (диапазон слотов `912-936`, начальное число `NEXUS-REH-2026Q2`); Он дайджест нового трекера. |

## Добавить дельта-конфигурацию

- يظل `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` в режиме онлайн. Создан для `defaults/nexus/*.toml` и создан для отслеживания Genesis, а также трекера для отслеживания событий. Хана.
- Подписаны пакеты конфигурации حزمة تيليمتري التدريب. Для получения дополнительной информации о `scripts/telemetry/validate_nexus_telemetry_pack.py`, необходимо изменить конфигурацию дельты. Откройте для себя новые артефакты в формате B4.
- Задание Iroha 2 полосы движения: конфигурации в `nexus.enabled = false` Задание переопределяет полосу движения/пространство данных/маршрутизацию в зависимости от конфигурации. Установите Nexus (`--sora`) и установите `nexus.*` в однополосном режиме.
- ابق سجل تصويت الحوكمة (GOV-2026-03-19) Создан трекер, созданный в 2026 году в Лос-Анджелесе. Это было сделано в честь Дня независимости США.

## متابعات تدريب الاطلاق

- Для `docs/source/runbooks/nexus_multilane_rehearsal.md` необходимо выполнить откат и выполнить откат; Создает Runbook для работы с полосами движения и экспортерами.
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` в артефакте, который появился в 9-м выпуске журнала «Agenda». Ответ Q2. اضف التدريبات المستقبلية الى نفس tracker بدلا من فتح trackers مستقلة للحفاظ على تسلسل الادلة.
- фрагменты кода OTLP и экспорта для Grafana (راجع `docs/source/telemetry.md`) для пакетной обработки экспортера; Размер партии в 1 квартале: 256 дюймов, с запасом по высоте.
- Поддержка CI/тестов для многополосного тестирования в `integration_tests/tests/nexus/multilane_pipeline.rs` и рабочего процесса `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), Дополнительный файл `pytests/nexus/test_multilane_pipeline.py`; Хеш-код `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) для отслеживания пакетов تدريب.

## دورة حياة переулки в وقت التشغيل- Для создания дорожек в разделе "Объекты привязок" в пространстве данных. Во время примирения Кура / التخزين الطبقي، и ترك الكتالوج Дэниэл Стив. Эти помощники пересылают свои следы по полосам движения в Ла-Майне, доказывая, что они синтезируют эти реестры слияний.
- Создание помощников для настройки конфигурации/жизненного цикла в Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) День независимости Для маршрутизации, моментальных снимков TEU и реестров манифестов необходимо выполнить маршрутизацию.
- Проверка: создание и настройка пространств данных и корневых систем хранения данных. (многоуровневый холодный корень/مجلدات Кура لكل переулок). اصلح المسارات الاساسية وحاول مجددا؛ Выполните поиск различий в полосе движения/пространстве данных в информационных панелях управления.

## Использование NPoS для противодавления

Завершите фазу B, чтобы остановить кардиостимулятор. بـ NPoS и сплетни о противодавлении. Используйте ремень безопасности для `integration_tests/tests/sumeragi_npos_performance.rs`, чтобы просмотреть файл JSON. (`sumeragi_baseline_summary::<scenario>::...`) Ответ Миссисипи:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS` и `SUMERAGI_NPOS_STRESS_COLLECTORS_K` и `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`. Установите флажок 1 s/`k=3` для B4.

| السيناريو / тест | تغطية | تيليمتري اساسية |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Время блокировки 12 минут, время блокировки, конверты, задержка EMA, а также датчики и избыточная отправка. В комплект поставки входит комплект. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Обвинительный приговор Лёрсуну Тэкулу отсрочен при поступлении. لقدرة/التشبع. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Дрожание от кардиостимулятора и тайм-ауты при просмотре результатов измерения +/-125 промилле. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | يدفع payloads RBC كبيرة حتى حدود soft/hard للـ store ﻿Магазин «Нью-Йорк Таймс». | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Встроенные датчики, резервная отправка и сборщики-на-цели, а также избыточная отправка المطلوبة مرتبطة от начала до конца. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Созданы куски, которые были загружены в журнале невыполненной работы по ошибкам, когда были обнаружены полезные нагрузки. | И18НИ00000105Х, И18НИ00000106Х, И18НИ00000107Х. |

Создайте файл JSON, создайте жгут и очистите его от Prometheus. Установите противодавление и установите противодавление.

## قائمة التحديث1. Используйте маршрутизированную трассировку, чтобы выполнить поиск по маршрутизации.
2. Нажмите кнопку «Уведомление» для «Alertmanager» и нажмите кнопку «Уведомление».
3. Создайте дельта-конфигурацию, создайте трекер и создайте дайджест нового пакета телеметрии и запроса на извлечение.
4. Создание артефакта для создания дорожной карты / дорожной карты для будущего проекта. Он был создан для специальных мероприятий.

## فهرس الادلة

| الاصل | الموقع | ملاحظات |
|-------|----------|-------|
| Проверка маршрутизации маршрутизации (1 квартал 2026 г.) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Завершить фазу B1; Был установлен код `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Изменение конфигурации трекера | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Откройте TRACE-CONFIG-DELTA и выполните команду GOV-2026-03-19. |
| خطة исправление للتيليمتري | `docs/source/nexus_telemetry_remediation_plan.md` | Пакет оповещений включает в себя OTLP и ограждения, установленные для B2. |
| Трекер تدريب многополосный | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد artefacts تدريب 9 وبريل وادلة الخاص بالـ validator وملاحظات/agenda Q2 и откат. |
| Манифест/дайджест пакета телеметрии (последняя версия) | И18НИ00000113X (+ `.sha256`) | Диапазон слотов 912–936 и начальное число `NEXUS-REH-2026Q2` позволяют хэшировать артефакты. |
| Манифест профиля TLS | И18НИ00000116X (+ `.sha256`) | хеш-код TLS для повторного запуска Q2; Используйте маршрутизированную трассировку. |
| Программа TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Начало работы во втором квартале (диапазон слотов, начальное значение рабочей нагрузки, контрольный список). |
| Runbook Справочник | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Контрольный список: подготовка -> выполнение -> откат; حدثها عند تغير طوبولوجيا переулки и международные экспортеры. |
| Валидатор пакета телеметрии | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI в стиле ретро B4; ارشف дайджест трекера عند تغير الحزمة. |
| Многополосная регрессия | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Многополосные конфигурации `nexus.enabled = true`, хэши каталога Sora и Kura/merge-log по полосе (`blocks/lane_{id:03}_{slug}`). `ConfigLaneRouter` — это дайджест современных артефактов. |