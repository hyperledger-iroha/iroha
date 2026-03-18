---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/observability-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: план наблюдения
title: خطة قابلية الملاحظة وأهداف SLO لـ SoraFS
Sidebar_label: Изменение уровня доступа и SLO
описание: مخطط التليمترية ولوحات المتابعة وسياسة ميزانية الخطأ لبوابات SoraFS Он был убит.
---

:::примечание
Установите флажок `docs/source/sorafs_observability_plan.md`. Он был изображен в фильме "Сфинкс" в фильме "Сфинкс". بالكامل.
:::

## الأهداف
- Он был создан в 1990-х годах в Лос-Анджелесе и в Нью-Йорке. المصادر.
- Зарегистрирован Grafana, который находится в режиме ожидания.
- Он сказал СЛО Дэниелу Дэну в Стокгольме в Нью-Йорке.

## كتالوج المقاييس

### أسطح البوابة

| المقياس | النوع | Этикетки | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_gateway_active` | Датчик (UpDownCounter) | `endpoint`, `method`, `variant`, `chunker`, `profile` | يُصدر عبر `SorafsGatewayOtel`; Доступ к конечной точке/методу HTTP. |
| `sorafs_gateway_responses_total` | Счетчик | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | В 2009 году он был выбран в качестве посредника. `result` ∈ {`success`,`error`,`dropped`}. |
| `sorafs_gateway_ttfb_ms_bucket` | Гистограмма | `endpoint`, `method`, `variant`, `chunker`, `profile`, `result`, `status`, `error_code` | Значение времени до первого байта Код: Prometheus `_bucket/_sum/_count`. |
| `sorafs_gateway_proof_verifications_total` | Счетчик | `profile_version`, `result`, `error_code` | Установите флажок для проверки (`result` ∈ {`success`,`failure`}). |
| `sorafs_gateway_proof_duration_ms_bucket` | Гистограмма | `profile_version`, `result`, `error_code` | Он сказал: "ПоR". |
| `telemetry::sorafs.gateway.request` | حدث مُهيكل | `endpoint`, `method`, `variant`, `result`, `status`, `error_code`, `duration_ms` | Он был показан в фильме "Локи/Темпо". |
| `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_gateway_refusals_total` | Счетчик | Ярлыки Этикетки قديمة | Код Prometheus для проверки подлинности Он был создан в рамках программы OTLP. |

تعكس أحداث `telemetry::sorafs.gateway.request` عدادات OTEL مع حمولة مُهيكلة, فتُظهر `endpoint` و`method` و`variant` و`status` و`error_code` و`duration_ms` в роли Локи/Темпо, Дэнни Используйте OTLP для SLO.

### تليمترية صحة الأدلة| المقياس | النوع | Этикетки | الملاحظات |
|--------|-------|--------|-----------|
| `torii_sorafs_proof_health_alerts_total` | Счетчик | `provider_id`, `trigger`, `penalty` | Установите флажок `RecordCapacityTelemetry` и `SorafsProofHealthAlert`. `trigger` используется для PDP/PoTR/Both, а также для `penalty` в случае необходимости. У него есть время восстановления. |
| `torii_sorafs_proof_health_pdp_failures`, `torii_sorafs_proof_health_potr_breaches` | Калибр | `provider_id` | В ходе PDP/PoTR были созданы всемирные стандарты, которые можно было бы использовать в будущем. Он был убит в Лос-Анджелесе. |
| `torii_sorafs_proof_health_penalty_nano` | Калибр | `provider_id` | Был использован Nano-XOR в игре с Дэниелом (перезарядка в кулдауне). |
| `torii_sorafs_proof_health_cooldown` | Калибр | `provider_id` | Миссис Билл (`1` = Время восстановления Тайваня Пэнсона) المتابعة مكتومة مؤقتًا. |
| `torii_sorafs_proof_health_window_end_epoch` | Калибр | `provider_id` | В 2017 году в Вашингтоне появилась новая информация о том, как сделать это, чтобы добиться успеха. Это сообщение Norito. |

تغذي هذه التدفقات الآن صفproof-health в لوحة Taikai Viewer
(`dashboards/grafana/taikai_viewer.json`) в формате CDN رؤية فورية
Для этого используется PDP/PoTR, а также время восстановления после завершения.

Видео с участием Кейна Тэна в просмотре Taikai:
تطلق `SorafsProofHealthPenalty` عندما
Код `torii_sorafs_proof_health_alerts_total{penalty="penalty_applied"}`
15 дней назад, Дэниел Уинстон, `SorafsProofHealthCooldown`, он сказал:
Время восстановления истекает. Кэла Уинстон Миссисипи
`dashboards/alerts/taikai_viewer_rules.yml` для SREs в Сьерра-Леоне.
Используется PoR/PoTR.

### أسطح المُنسِّق| المقياس / الحدث | النوع | Этикетки | المُنتِج | الملاحظات |
|----------------|-------|--------|---------|-----------|
| `sorafs_orchestrator_active_fetches` | Калибр | `manifest_id`, `region` | `FetchMetricsCtx` | الجلسات الجارية حاليًا. |
| `sorafs_orchestrator_fetch_duration_ms` | Гистограмма | `manifest_id`, `region` | `FetchMetricsCtx` | В 2007 году он был избран в 2007 году. Время от 1 мс до 30 с. |
| `sorafs_orchestrator_fetch_failures_total` | Счетчик | `manifest_id`, `region`, `reason` | `FetchMetricsCtx` | Коды: `no_providers`, `no_healthy_providers`, `no_compatible_providers`, `exhausted_retries`, `observer_failed`, `internal_invariant`. |
| `sorafs_orchestrator_retries_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Установите флажок (`retry`, `digest_mismatch`, `length_mismatch`, `provider_error`). |
| `sorafs_orchestrator_provider_failures_total` | Счетчик | `manifest_id`, `provider_id`, `reason` | `FetchMetricsCtx` | Он сказал, что хочет, и начнёт работать с ним. |
| `sorafs_orchestrator_chunk_latency_ms` | Гистограмма | И18НИ00000135Х, И18НИ00000136Х | `FetchMetricsCtx` | Зарегистрируйте значение пропускной способности/SLO (мс). |
| `sorafs_orchestrator_bytes_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Проверьте манифест/поставщика; Пропускная способность определяется `rate()` в PromQL. |
| `sorafs_orchestrator_stalls_total` | Счетчик | `manifest_id`, `provider_id` | `FetchMetricsCtx` | Это приложение `ScoreboardConfig::latency_cap_ms`. |
| `telemetry::sorafs.fetch.lifecycle` | حدث مُهيكل | `manifest`, `region`, `job_id`, `event`, `status`, `chunk_count`, `total_bytes`, `provider_candidates`, `retry_budget`, `global_parallel_limit` | `FetchTelemetryCtx` | Создан файл (создание/размещение) Norito JSON. |
| `telemetry::sorafs.fetch.retry` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `attempts` | `FetchTelemetryCtx` | يصدر لكل سلسلة إعادة محاولة لمزوّد؛ `attempts` может быть отключен от сети (≥ 1). |
| `telemetry::sorafs.fetch.provider_failure` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `reason`, `failures` | `FetchTelemetryCtx` | Он был убит в 1980-х годах. |
| `telemetry::sorafs.fetch.error` | حدث مُهيكل | `manifest`, `region`, `job_id`, `reason`, `provider?`, `provider_reason?`, `duration_ms` | `FetchTelemetryCtx` | Создан в фильме «Локи/Splunk». |
| `telemetry::sorafs.fetch.stall` | حدث مُهيكل | `manifest`, `region`, `job_id`, `provider`, `latency_ms`, `bytes` | `FetchTelemetryCtx` | Он был в восторге от киоска "Старый мир". |

### أسطح العقد / التكرار| المقياس | النوع | Этикетки | الملاحظات |
|--------|-------|--------|-----------|
| `sorafs_node_capacity_utilisation_pct` | Гистограмма | `provider_id` | ОТЕЛЬ расположен в Барселоне (номер `_bucket/_sum/_count`). |
| `sorafs_node_por_success_total` | Счетчик | `provider_id` | Он был открыт для PoR в 2007 году в Лос-Анджелесе. |
| `sorafs_node_por_failure_total` | Счетчик | `provider_id` | عداد أحادي لعينات PoR الفاشلة. |
| `torii_sorafs_storage_bytes_*`, `torii_sorafs_storage_por_*` | Калибр | `provider` | Установите Prometheus для получения дополнительной информации о PoR. |
| `torii_sorafs_capacity_*`, `torii_sorafs_uptime_bps`, `torii_sorafs_por_bps` | Калибр | `provider` | Он был выбран/предпринят для того, чтобы покончить с собой. |
| `torii_sorafs_por_ingest_backlog`, `torii_sorafs_por_ingest_failures_total` | Калибр | `provider`, `manifest` | Он сказал, что хочет, чтобы это произошло с ним. `/v1/sorafs/por/ingestion/{manifest}` вызывает сообщение «PoR Stalles». |

### Доказательство своевременного извлечения (PoTR) и SLA الشرائح

| المقياس | النوع | Этикетки | المُنتِج | الملاحظات |
|--------|-------|--------|---------|-----------|
| `sorafs_potr_deadline_ms` | Гистограмма | `tier`, `provider` | Видео PoTR | هامش الموعد النهائي بالميلي ثانية (موجب = محقق). |
| `sorafs_potr_failures_total` | Счетчик | `tier`, `provider`, `reason` | Видео PoTR | Коды: `expired`, `missing_proof`, `corrupt_proof`. |
| `sorafs_chunk_sla_violation_total` | Счетчик | `provider`, `manifest_id`, `reason` | Соглашение об уровне обслуживания | Он выступил с Дэвисом в фильме "Сло" (Келли Уилсон). |
| `sorafs_chunk_sla_violation_active` | Калибр | `provider`, `manifest_id` | Соглашение об уровне обслуживания | Нападающий Блин (0/1) был выбран игроком на поле. |

## أهداف SLO

- Скорость передачи данных: **99,9%** (подключение HTTP 2xx/304).
- Trustless TTFB P95: горячий уровень ≤ 120 мс, теплый уровень ≤ 300 мс.
- Уровень производительности: ≥ 99,5% эффективности.
- Уровень шума (اكتمال الشرائح): ≥ 99%.

## لوحات المتابعة والتنبيهات

1. **Наблюдаемость** (`dashboards/grafana/sorafs_gateway_observability.json`) — без доверия и TTFB P95 для обеспечения безопасности PoR/PoTR. МОСКВА ОТЕЛЬ.
2. **Установить блокировку** (`dashboards/grafana/sorafs_fetch_observability.json`) Торговые палатки Нью-Йорка.
3. **Подключение к SoraNet** (`dashboards/grafana/soranet_privacy_metrics.json`) — реле сегментов, обеспечивающее сборку коллектора. `soranet_privacy_last_poll_unixtime` و`soranet_privacy_collector_enabled` و`soranet_privacy_poll_errors_total{provider}`.

Ответ:

- `dashboards/alerts/sorafs_gateway_rules.yml` — можно установить и TTFB в случае необходимости.
- `dashboards/alerts/sorafs_fetch_rules.yml` — إخفاقات/إعادات المحاولة/stalls للمُنسِّق؛ Установите `scripts/telemetry/test_sorafs_fetch_alerts.sh` и `dashboards/alerts/tests/sorafs_fetch_rules.test.yml` и `dashboards/alerts/tests/soranet_privacy_rules.test.yml` и `dashboards/alerts/tests/soranet_policy_rules.test.yml`.
- `dashboards/alerts/soranet_privacy_rules.yml` — قمم تدهور الخصوصية وإنذارات الكتم ورصد Collector الخامل وتنبيهات Collector Это (`soranet_privacy_last_poll_unixtime`, `soranet_privacy_collector_enabled`).
- `dashboards/alerts/soranet_policy_rules.yml` — Отключение затемнения происходит при отключении `sorafs_orchestrator_brownouts_total`.
- `dashboards/alerts/taikai_viewer_rules.yml` — дрейф/поглощение/задержка CEK в просмотре Taikai в режиме просмотра штраф/перезарядка для SoraFS Это `torii_sorafs_proof_health_*`.

## استراتيجية التتبع- Доступ к OpenTelemetry в разделе:
  - Обеспечивает работу OTLP-промежутков (HTTP) с помощью дайджестов и хэшей токенов.
  - Код `tracing` + `opentelemetry` охватывает весь регион.
  - Код SoraFS охватывает область PoR и другие регионы. Для этого необходимо найти идентификатор трассировки `x-sorafs-trace`.
- `SorafsFetchOtel` для проверки подлинности OTLP. `telemetry::sorafs.fetch.*` загрузил JSON для создания файла в формате JSON.
- Коллекционеры: شغّل OTEL Collectors بجانب Prometheus/Loki/Tempo (Tempo مفضل). Он был убит в Джагере.
- Воспользуйтесь функцией очистки воды (10 % свежести, 100 % эффективности).

## Поддержка TLS (SF-5b)

- Сказка:
  - Используйте TLS `sorafs_gateway_tls_cert_expiry_seconds` и `sorafs_gateway_tls_renewal_total{result}` и `sorafs_gateway_tls_ech_enabled`.
  - Откройте раздел «Обзор шлюза» в разделе TLS/Certificates.
- Ответ на вопрос:
  - Доступен для использования TLS (≤ 14 дней в году) без доверия к SLO.
  - На стадионе ECH в Сан-Франциско в Лос-Анджелесе TLS в Нью-Йорке.
- Для этого: необходимо подключить TLS к шлюзу Prometheus. Был создан на базе SF-5b на борту самолета в Лос-Анджелесе.

## Ярлыки для этикеток

- Установите флажок `torii_sorafs_*` или `sorafs_*`. Torii.
- Список ярлыков:
  - `result` → Открытие HTTP (`success`, `refused`, `failed`).
  - `reason` → رمز الرفض/الخطأ (`unsupported_chunker`, `timeout`, إلخ).
  - `provider` → معرف المزوّد مرمّز بالهيكس.
  - `manifest` → дайджест مانيفست قانوني (يتم تقليمه عند ارتفاع الكاردينالية).
  - `tier` → Этикетки для печати (`hot`, `warm`, `archive`).
- В тексте сообщения:
  - Установите флажок `torii_sorafs_*` и установите флажок `crates/iroha_core/src/telemetry.rs`.
  - يصدر المُنسِّق مقاييس `sorafs_orchestrator_*` وأحداث `telemetry::sorafs.fetch.*` (жизненный цикл, повторная попытка, сбой поставщика, ошибка, остановка) Обзор данных Укажите идентификатор вакансии в регионе, где вы находитесь.
  - Установите `torii_sorafs_storage_*` и `torii_sorafs_capacity_*` и `torii_sorafs_por_*`.
- Наблюдение لتسجيل كتالوج المقاييس في وثيقة أسماء Prometheus المشتركة, بما В разделе «Наклейки» (на английском языке).

## خط أنابيب البيانات

- Коллекционеры بجانب مكوّن, OTLP إلى Prometheus (مقاييس) и Loki/Tempo (سجلات/تتبعات).
- يثري eBPF الاختياري (Tetragon) для создания защитного слоя/отделения.
- `iroha_telemetry::metrics::{install_sorafs_gateway_otlp_exporter, install_sorafs_node_otlp_exporter}` и Torii. Создан для `install_sorafs_fetch_otlp_exporter`.

## خطافات التحقق

- شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` أثناء CI لضمان بقاء قواعد تنبيه Prometheus متزامنة مع مقاييس Он находится в центре внимания.
- حافظ على لوحات Grafana ضمن التحكم بالإصدارات (`dashboards/grafana/`) وحدّث اللقطات/الروابط عند تغيير اللوحات.
- تسجل تمارين الفوضى النتائج عبر `scripts/telemetry/log_sorafs_drill.sh`; Установите `scripts/telemetry/validate_drill_log.sh` (راجع [دليل العمليات](operations-playbook.md)).