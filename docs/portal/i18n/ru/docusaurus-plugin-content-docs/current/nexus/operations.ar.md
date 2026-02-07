---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/operations.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: нексус-операции
Название: دليل تشغيل Nexus
описание: ملخص عملي لخطوات تشغيل مشغل Nexus, يعكس `docs/source/nexus_operations.md`.
---

Он был установлен на `docs/source/nexus_operations.md`. В 1990-х годах в Нью-Йорке, в Нью-Йорке, в Нью-Йорке, в Нью-Йорке, США Это сообщение Nexus اتباعها.

## قائمة دورة الحياة

| عرحلة | إجراءات | أدلة |
|-------|--------|----------|
| ما قبل الإقلاع | Он был создан в بصمات/تواقيع الإصدار, أكد `profile = "iroha3"`, и был отправлен в продажу. | Введите `scripts/select_release_profile.py`, проверьте контрольную сумму, установите флажок. |
| Новости | Он был создан `[nexus]`, в Стокгольме и DA в Нью-Йорке. `--trace-config`. | Код `irohad --sora --config ... --trace-config` был введен для регистрации. |
| فحص الدخان والتحويل | Выполняется `irohad --sora --config ... --trace-config`, выполняется настройка CLI (`FindNetworkStatus`), а также выполняется настройка интерфейса командной строки. القبول. | Используйте функцию оповещения + Менеджер оповещений. |
| حالة مستقرة | Он работает в режиме онлайн и работает с configs/runbooks. تغير البيانات. | Он был создан Сэнсэем, Льюисом Лоуренсом и его коллегой по футболу. |

Он присоединился к самому себе `docs/source/sora_nexus_operator_onboarding.md`.

## إدارة التغيير

1. **Проверка** - Проверка работоспособности в `status.md`/`roadmap.md`; Проведите адаптацию в рамках PR-проекта.
2. **Переулок Переулка** — отображается в каталоге Space Directory в каталоге `docs/source/project_tracker/nexus_config_deltas/`.
3. **Обратный доступ** — в области `config/config.toml` используется область дорожки/пространства данных. Он был убит в 1990-х годах, когда он был убит.
4. **Убийство ** - Дэйр Сэнсэй Сэнсэйл / الاستعادة/فحص دخان؛ Был установлен `docs/source/project_tracker/nexus_config_deltas/<date>-rollback.md`.
5. **Миниатюрное соединение** — на линии движения CBDC / CBDC, расположенной в центре города. Он был отправлен DA в 2017 году (18NI00000038X).

## القياس и SLO

- Доступны: `dashboards/grafana/nexus_lanes.json`, `nexus_settlement.json`, добавлена версия SDK (например, `android_operator_console.json`).
- Код: `dashboards/alerts/nexus_audit_rules.yml` вместо Torii/Norito (`dashboards/alerts/torii_norito_rpc_rules.yml`).
- Сообщение от президента США:
  - `nexus_lane_height{lane_id}` - تنبيه عند عدم التقدم لثلاث خانات.
  - `nexus_da_backlog_chunks{lane_id}` - تنبيه عند تجاوز العتبات لكل переулок (64 общедоступных / 8 частных).
  - `nexus_settlement_latency_seconds{lane_id}` - Время восстановления P99: от 900 мс (общедоступный) до 1200 мс (частный).
  - `torii_request_failures_total{scheme="norito_rpc"}` - Отображается в режиме 5 дней 2%.
  - `telemetry_redaction_override_total` - Сев 2. Он был отправлен в Лондон.
- Для этого необходимо установить [загрузить файл Nexus](./nexus-telemetry-remediation) على الأقل. Он был убит в 2007 году в Нью-Йорке.

## مصفوفة الحوادث| شدة | تعريف | Новости |
|----------|------------|----------|
| Сев 1 | Он начал работать с пространством данных, 15 февраля 2018 года, в 15:00 по местному времени. | Nexus Primary + Release Engineering + Compliance, проверка соответствия, проверка соответствия 30 дней, для развертывания в Лос-Анджелесе. | Nexus Primary + SRE, код <=4, который необходимо установить в случае необходимости. |
| Сев 3 | انحراف غير معطل (документы, تنبيهات). | Скончался в Нью-Йорке и в Вашингтоне. |

Доступ к идентификаторам и идентификаторам полосы движения/пространства данных. В 2007 году он был выбран в 2017 году.

## أرشيف الأدلة

- خزّن الحزم/البيانات/صادرات القياس تحت `artifacts/nexus/<lane>/<date>/`.
- Установите флажок + установите `--trace-config` для проверки.
- أرفق محاضر المجلس + القرارات الموقعة عند تطبيق تغييرات الإعداد أو البيانات.
- Установите Prometheus на 12-й день после установки Nexus.
- Дед Мороз на `docs/source/project_tracker/nexus_config_deltas/README.md` был отправлен в США.

## مواد ذات صلة

- Добавлено: [Nexus обзор](./nexus-overview)
- Сообщение: [Спецификация Nexus](./nexus-spec)
- Переулок: [модель полосы Nexus](./nexus-lane-model)
- Дополнительная информация: [Nexus примечания к переходу](./nexus-transition-notes)
- подключение оператора: [Sora Nexus подключение оператора](./nexus-operator-onboarding)
- Сообщение: [Nexus план исправления телеметрии](./nexus-telemetry-remediation)