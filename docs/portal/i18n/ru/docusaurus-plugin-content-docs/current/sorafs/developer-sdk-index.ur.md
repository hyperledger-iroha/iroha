---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-разработчика SDK
заголовок: SoraFS Руководства по SDK
Sidebar_label: Руководства по SDK
описание: Артефакты SoraFS интегрируют фрагменты фрагментов.
---

:::примечание
:::

В хабе есть инструмент SoraFS, инструментарий для отслеживания языковых помощников.
Rust مخصوص snippets کے لیے [Rust SDK snippets](./developer-sdk-rust.md) دیکھیں۔

## Языковые помощники

- **Python** — `sorafs_multi_fetch_local` (дымовые тесты локального оркестратора)
  `sorafs_gateway_fetch` (упражнения по шлюзу E2E) или дополнительно `telemetry_region`.
  `transport_policy` переопределить значение параметра
  (`"soranet-first"`, `"soranet-strict"` или `"direct-only"`), а также ручки развертывания CLI.
  Локальный прокси-сервер QUIC или манифест браузера `sorafs_gateway_fetch`
  `local_proxy_manifest` Тестирование тестового пакета доверия и адаптеров браузера.
  پہنچا سکیں۔
- **JavaScript** — `sorafsMultiFetchLocal` Помощник Python для зеркала и байтов полезной нагрузки
  Сводные квитанции можно просмотреть в разделе `sorafsGatewayFetch` Torii шлюзы для выполнения упражнений.
  манифесты локального прокси-сервера, потоки, интерфейс CLI, переопределения телеметрии/транспорта, выставление данных,
- **Rust** — планировщик служб, который можно использовать `sorafs_car::multi_fetch`, и который можно встроить в систему.
  Помощники доказательства потока и интеграция оркестратора کے لیے [Фрагменты Rust SDK](./developer-sdk-rust.md) دیکھیں۔
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii Повторное использование HTTP-исполнителя.
  `GatewayFetchOptions` کو честь کرتا ہے۔ اسے `ClientConfig.Builder#setSorafsGatewayUri` اور
  Подсказка по загрузке PQ (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) Как объединить несколько загрузок
  Пути только для PQ и другие пути

## Табло и ручки политики

Python (`sorafs_multi_fetch_local`) и JavaScript (`sorafsMultiFetchLocal`) помощники CLI
Табло планировщика с поддержкой телеметрии, которое можно использовать:- Табло по умолчанию для производственных двоичных файлов и возможность включения функции کرتے ہیں؛ повторы матчей کرتے وقت
  `use_scoreboard=True` (записи `telemetry`) — метаданные вспомогательной рекламы и недавние телеметрические данные.
  снимки سے взвешенный порядок провайдера производный کرے۔
- `return_scoreboard=True` устанавливает параметры получения расчетных весов для получения блоков данных и журналов CI.
  Диагностический захват کر سکیں۔
- `deny_providers` یا `boost_providers` массивы استعمال کریں تاکہ пиры отклоняют ہوں یا `priority_delta`
  добавить поставщиков планировщиков ہو جب, выбрать کرے۔
- Положение `"soranet-first"` по умолчанию. `"direct-only"` صرف تب دیں
  Регион соответствия требованиям, реле, резервная репетиция резервного варианта SNNet-5a, `"soranet-strict"`
  Пилотные проекты только для PQ. Наличие одобрения руководства. Наличие резерва.
- Помощники шлюза `scoreboardOutPath` اور `scoreboardNowUnixSecs` بھی выставляют کرتے ہیں۔ `scoreboardOutPath`
  set کریں تاکہ вычисленное табло persist ہو (CLI `--scoreboard-out` flag کی طرح) اور
  `cargo xtask sorafs-adoption-check` Артефакты SDK проверяют соответствие требованиям `scoreboardNowUnixSecs` تب دیں جب
  светильники воспроизводимые метаданные стабильные значения `assume_now` значения چاہیے ہو۔ Помощник по JavaScript
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` Набор параметров для установки Ярлык اگر опустить ہو تو
  Из `region:<telemetryRegion>` выводится کرتا ہے (резервный `sdk:js`). Помощник Python и табло сохраняется.
  `telemetry_source="sdk:python"` Вызовите эмитт, если неявные метаданные отключены.

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