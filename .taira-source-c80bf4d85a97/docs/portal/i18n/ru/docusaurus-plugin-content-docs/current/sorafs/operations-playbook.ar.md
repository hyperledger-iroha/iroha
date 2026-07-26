---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Operations-Playbook
Название: دليل تشغيل عمليات SoraFS
Sidebar_label: Открыть
описание: Выбрано приложение для установки SoraFS.
---

:::примечание
Он был установлен в соответствии с `docs/source/sorafs_ops_playbook.md`. Он был показан в фильме "Сфинкс" в фильме "Сфинкс". بالكامل.
:::

## المراجع الرئيسية

- Запись: Grafana и `dashboards/grafana/`, установленный Prometheus в `dashboards/alerts/`.
- Код: `docs/source/sorafs_observability_plan.md`.
- Проверьте код: `docs/source/sorafs_orchestrator_plan.md`.

## مصفوفة التصعيد

| أولوية | أمثلة على المحفزات | المناوب الأساسي | الاحتياطي | ملاحظات |
|----------|--------------------|-----------------|----------|---------|
| Р1 | В случае успеха в PoR 5% (15 дней) Сообщение от 10 дней | Хранение СРЕ | Наблюдаемость TL | Он был убит 30 дней назад. |
| П2 | خرقSLO لزمن تأخر البوابة الإقليمي, قفزة إعادة المحاولة في المُنسِّق Соглашение об уровне обслуживания | Наблюдаемость TL | Хранение СРЕ | Он сказал Льюису Уилсону. |
| P3 | تنبيهات غير حرجة (диабетическое масло, 80–90%) | فرز الاستقبال | Операционная гильдия | عالج خلال يوم العمل التالي. |

## انقطاع البوابة / تدهور التوفر

**название**

- Коды: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Код: `dashboards/grafana/sorafs_gateway_overview.json`.

**Деньги **

1. Он сказал:
2. Дэниел Тэри Torii в роли диктатора (Дэмиэн Миссисипи) Устанавливается `sorafs_gateway_route_weights` в режиме восстановления (`docs/source/sorafs_gateway_self_cert.md`).
3. Выполните настройку «прямой выборки» CLI/SDK (`docs/source/sorafs_node_client_protocol.md`).

**название**

- Проведено в потоковом режиме استهلاك رموز в формате `sorafs_gateway_stream_token_limit`.
- Установите флажок TLS и нажмите кнопку «Установить».
- شغّل `scripts/telemetry/run_schema_diff.sh` للتأكد من أن المخطط الذي تصدّره البوابة يطابق الإصدار المتوقع.

**Получить информацию**

- أعد تشغيل عملية البوابة المتأثرة فقط؛ Он сказал, что Коллинз хочет, чтобы он сделал это.
- В этом случае поток будет составлять 10–15 % от скорости потока.
- Зарегистрируйтесь для самостоятельного подтверждения (`scripts/sorafs_gateway_self_cert.sh`).

**В день рождения**

- Вскрытие произошло в P1 باستخدام `docs/source/sorafs/postmortem_template.md`.
- Он сказал:

## ارتفاع مفاجئ لفشل الإثبات (PoR / PoTR)

**название**

- Коды: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Код: `dashboards/grafana/sorafs_proof_integrity.json`.
- Код: `torii_sorafs_proof_stream_events_total` или `sorafs.fetch.error` вместо `provider_reason=corrupt_proof`.

**Деньги **

1. Установите флажок для проверки работоспособности системы (`docs/source/sorafs/manifest_pipeline.md`).
2. Сделайте это в режиме онлайн.

**название**

- عمق قائمة تحديات PoR مقابل `sorafs_node_replication_backlog_total`.
- تحقّق من مسار التحقق من الإثبات (`crates/sorafs_node/src/potr.rs`) للعمليات المنشورة مؤخرًا.
- Установлена ​​прошивка, установленная в приложении.

**Получить информацию**

- Для получения дополнительной информации о PoR выберите `sorafs_cli proof stream` для проверки.
- Он сказал: Табло на сайте الحوكمة المُنسِّق على تحديث.

**В день рождения**- Он Сэнсэй Тэрри в PoR в Нью-Йорке.
- Дэн Уилсон в посмертном исследовании, проведенном в 1980-х годах в Нью-Йорке.

## تأخر التكرار / نمو التراكم

**название**

- Коды: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. استورد
  `dashboards/alerts/sorafs_capacity_rules.yml` وشغّل
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  Используйте функцию Alertmanager для управления оповещениями.
- Код: `dashboards/grafana/sorafs_capacity_health.json`.
- Коды: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Деньги **

1). الأساسية.
2. Он был главой государства, которого пригласили на работу в Вашингтоне. Он сказал, что это не так.

**название**

- Он сказал, что в фильме "Детский мир" есть все, что нужно для этого. تضخم التراكم.
- Было создано в 1999 году в Лос-Анджелесе Кейна (`sorafs_node_capacity_utilisation_percent`).
- راجع تغييرات الإعداد الأخيرة (تحديثات ملفات تعريف القطع، وتيرة) الإثباتات).

**Получить информацию**

- Установите `sorafs_cli` на `--rebalance` для проверки.
- В фильме «Старый город» в Лос-Анджелесе.
- Воспользуйтесь функцией TTL.

**В день рождения**

- Дэвид Тэрри сказал, что в 2007 году он будет работать в режиме реального времени.
- Зарегистрировано соглашение об уровне обслуживания SLA для `docs/source/sorafs_node_client_protocol.md`.

## وتيرة تدريبات الفوضى

- **Сыграйте**: Выполните поиск в режиме онлайн + дайте возможность провести время в режиме реального времени.
- **Сын Сэн**: Он выступил в роли PoR/PoTR, когда был убит в 2008 году.
- **В фильме "Семерка"**: Сонни Трэйз организовал постановку фильма "Мститель".
- Запись на страницу Настройки (`ops/drill-log.md`):

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- Сообщение в журнале "Страна":

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Установите `--status scheduled` в систему управления и `pass`/`fail` в режиме ожидания. و`follow-up` عندما تبقى بنود مفتوحة.
- غيّر التجريبية أو التحقق الآلي؛ Создан в السكربت в تحديث `ops/drill-log.md`.

## قالب ما بعد الحادث

Установите `docs/source/sorafs/postmortem_template.md` для подключения P1/P2 и установите флажок. Он выступил в роли президента США, а также в Вашингтоне. Воспользуйтесь услугами, которые вы можете получить, чтобы получить больше информации.