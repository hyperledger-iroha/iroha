---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-разработчика SDK
title: Добавлен SDK в SoraFS.
Sidebar_label: Добавлен SDK
описание: Создан بكل لغة لدمج آرتيفاكتات SoraFS.
---

:::примечание
Был установлен `docs/source/sorafs/developer/sdk/index.md`. Он был создан в честь Сфинкса.
:::

Он был создан для того, чтобы помочь ему в работе над проектом. Это SoraFS.
Загрузите Rust в [Rust SDK](./developer-sdk-rust.md).

## مساعدات اللغات

- **Python** — `sorafs_multi_fetch_local` (название приложения لمُنسِّق المحلي) и
  `sorafs_gateway_fetch` (подключен к E2E) `telemetry_region`
  Код `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` и `"direct-only"`)
  Интерфейс командной строки. Для использования прокси-сервера QUIC используется `sorafs_gateway_fetch`.
  `local_proxy_manifest` был создан в соответствии с пакетом доверия, установленным для проверки.
- **JavaScript** — код `sorafsMultiFetchLocal` для Python, используемый для редактирования.
  Установите прокси-сервер `sorafsGatewayFetch` и установите Torii. المحلية،
  Вы можете получить доступ к интерфейсу командной строки/интерфейсу командной строки.
- **Ржавчина** — يمكن للخدمات تضمين المُجدول مباشرةً عبر `sorafs_car::multi_fetch`; راجع
  [Файл Rust SDK](./developer-sdk-rust.md) Доступен поток корректуры для проверки.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Поддержка HTTP-интерфейса.
  Torii или `GatewayFetchOptions`. ادمجه مع
  `ClientConfig.Builder#setSorafsGatewayUri` для PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`)
  بمسارات PQ فقط.

## مفاتيح табло والسياسات

Использование Python (`sorafs_multi_fetch_local`) и JavaScript
(`sorafsMultiFetchLocal`) Открыть табло с помощью CLI:

- Табло تمكّن الثنائيات الإنتاجية افتراضيًا؛ اضبط `use_scoreboard=True`
  (أو وفّر إدخالات `telemetry`) عند إعادة تشغيل светильники
  Рекламные ролики, сделанные в Нью-Йорке, посвящены праздничным дням.
- اضبط `return_scoreboard=True` для получения фрагмента фрагмента.
  Это CI в التقاط التشخيصات.
- استخدم مصفوفتَي `deny_providers` أو `boost_providers` لرفض الأقران أو إضافة
  `priority_delta` находится в центре внимания.
- على الوضع الافتراضي `"soranet-first"` в честь президента США. قدّم
  `"direct-only"` был отправлен в США в 2017 году.
  Подключен SNNet-5a, установлен `"soranet-strict"` и доступен только PQ.
- Установите флажок `scoreboardOutPath` и `scoreboardNowUnixSecs`. اضبط
  `scoreboardOutPath` Открытие табло таблицы (с интерфейсом CLI `--scoreboard-out`)
  Создан `cargo xtask sorafs-adoption-check` в базе данных SDK, в приложении.
  `scoreboardNowUnixSecs` Замените светильники и установите `assume_now`.
  وصفية قابلة لإعادة الإنتاج. В JavaScript используется أيضًا ضبط
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; وعند حذف الملصق
  Используется `region:<telemetryRegion>` (резервный вариант `sdk:js`). Использование Python
  `telemetry_source="sdk:python"` Табло для табло ويُبقي البيانات الوصفية
  الضمنية معطّلة.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```