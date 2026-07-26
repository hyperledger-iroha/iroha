---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-разработчика SDK
Название: Руководство по SDK da SoraFS
Sidebar_label: Руководство по SDK
описание: Трех языков для интеграции артефактов SoraFS.
---

:::примечание Fonte canonica
Эта страница написана `docs/source/sorafs/developer/sdk/index.md`. Мантенья представился как копиас синхронизадас.
:::

Используйте этот хаб для поддержки помощников на языке, который сопровождает цепочку инструментов SoraFS.
Для конкретных фрагментов Rust используйте [фрагменты Rust SDK] (./developer-sdk-rust.md).

## Помощники по языку

- **Python** - `sorafs_multi_fetch_local` (дымовые тесты проводятся локальным заказчиком) e
  `sorafs_gateway_fetch` (упражнения E2E шлюза) перед началом работы с `telemetry_region`
  необязательно, но можно переопределить `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` или `"direct-only"`), используйте ручки
  развертывание сделать CLI. Когда локальный прокси-сервер QUIC, `sorafs_gateway_fetch` возвращается
  Манифест браузера em `local_proxy_manifest` для того, чтобы тесты прошли или пакет доверия
  для адаптеров навигации.
- **JavaScript** - `sorafsMultiFetchLocal` вызов помощника Python, возврат
  байты полезной нагрузки и резюме полученных данных, например `sorafsGatewayFetch` упражнения
  шлюзы Torii, encadeia манифестирует локальный прокси-сервер и выставляет переопределения ОС mesmos
  телеметрия/транспортировка через CLI.
- **Rust** — сервисы могут быть включены или планировщики напрямую через
  `sorafs_car::multi_fetch`; принести ссылку
  [Фрагменты Rust SDK](./developer-sdk-rust.md) для помощи в проверке потока
  Интеграция оркестратора.
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` повторное использование или исполнитель HTTP
  делаю Torii и честь `GatewayFetchOptions`. Объединить ком
  `ClientConfig.Builder#setSorafsGatewayUri` и подсказка по загрузке PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) когда загружается точно
  ficar em caminhos somente PQ.

## Табло и ручки политики

Помощники Python (`sorafs_multi_fetch_local`) и JavaScript
(`sorafsMultiFetchLocal`) показывает табло в планировщике с помощью телеметрии в США
пело CLI:- Бинарные файлы для профессионального производства или табло для планшета; определение `use_scoreboard=True`
  (или входные отверстия `telemetry`) для воспроизведения приспособлений, которые помогут вам получить
  порядок обдумывания материалов, часть метаданных рекламы и снимков
  последние телеметрии.
- Defina `return_scoreboard=True` для получения расчетов в песо вместе с получателями
  этот фрагмент позволяет регистрировать диагностику CI.
- Используйте массивы `deny_providers` или `boost_providers` для восстановления одноранговых узлов или добавления.
  `priority_delta` при выборе параметров планировщика.
- Поднимите позу `"soranet-first"` и выберите, что нужно подготовить к переходу на более раннюю версию;
  forneca `"direct-only"` apenas quando uma regiao de Complisar Precisar evitar реле
  Вы можете использовать резервный SNNet-5a и резервный `"soranet-strict"` для пилотов только для PQ
  com aprovacao degovanca.
- Помощники шлюза также выставляют `scoreboardOutPath` и `scoreboardNowUnixSecs`.
  Defina `scoreboardOutPath` для сохранения или расчета табло (укажите флаг или
  `--scoreboard-out` для CLI), чтобы `cargo xtask sorafs-adoption-check` был действительным
  Артефато SDK, и используйте `scoreboardNowUnixSecs`, когда точны приспособления
  доблесть `assume_now` установлена для воспроизводства метаданных. Нет помощника JavaScript,
  голос тамбем может определить `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  Когда метка и опущена, получается `region:<telemetryRegion>` (резервный пункт `sdk:js`).
  Помощник Python автоматически выдает `telemetry_source="sdk:python"` каждый раз, когда
  сохраняйте табло и мантемные метаданные, подразумеваемые дезабилитадос.

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