---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-разработчика SDK
заголовок: Руководства по SDK SoraFS
Sidebar_label: Руководства по SDK
описание: Языковые фрагменты для документов SoraFS.
---

:::note Канонический источник
:::

Используйте этот хаб для отслеживания языковых помощников, поставляемых с цепочкой инструментов SoraFS.
Для фрагментов, специфичных для Rust, можно перейти к [фрагментам Rust SDK] (./developer-sdk-rust.md).

##Языковые помощники

- **Python** — `sorafs_multi_fetch_local` (дымовые тесты локального оркестратора) и
  `sorafs_gateway_fetch` (шлюз E2E) теперь принимает опциональные
  `telemetry_region` плюс переопределение для `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` или `"direct-only"`), отражая выкатные ручки
  Интерфейс командной строки. Когда поднимается локальный QUIC-прокси, возвращается `sorafs_gateway_fetch`.
  Манифест браузера в формате `local_proxy_manifest`, чтобы тесты помогли передать пакет доверия
  браузерным адаптерам.
- **JavaScript** — `sorafsMultiFetchLocal` указывает помощник Python, возвращая байты полезной нагрузки
  и резюме квитанций, тогда как `sorafsGatewayFetch` упражняет шлюзы Torii,
  прокидывает манифесты локального прокси-сервера и раскрывает те же переопределения телеметрии/транспорта,
  что и CLI.
- **Rust** — сервисы могут настраивать планировщик напрямую через `sorafs_car::multi_fetch`;
  см. [Фрагменты Rust SDK](./developer-sdk-rust.md) для помощников по проверке потоков и специалистов
  оркестратора.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` переиспользует Torii HTTP-исполнитель.
  и наблюдает `GatewayFetchOptions`. Комбинируйте с
  `ClientConfig.Builder#setSorafsGatewayUri` и подсказка по загрузке PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`), когда загрузка должна идти по пути только для PQ.

## Табло и ручки политики

Помощники Python (`sorafs_multi_fetch_local`) и JavaScript (`sorafsMultiFetchLocal`)
вы одни табло планировщика с поддержкой телеметрии, прогноз CLI:- Производство бинари включает табло по умолчанию; установите `use_scoreboard=True`
  (или передайте записи `telemetry`) при проигрывании фикстур, чтобы помощник вывел
  взвешенный порядок провайдеров метаданных, рекламы и свежих снимков телеметрии.
- Установите `return_scoreboard=True`, чтобы получать расчетные веса вместе с квитанциями о кусках,
  запустить CI логам фиксировать исправления.
- Используйте массивы `deny_providers` или `boost_providers` для удаления пиров или добавления.
  `priority_delta`, когда планировщик выбирает провайдеров.
- Сохраняйте позу `"soranet-first"` по умолчанию, если только не готовы понизить версию;
  указывайте `"direct-only"` только в тех случаях, когда соответствие региону требует соблюдения реле или при
  повторения резервного варианта SNNet-5a и резервируйте `"soranet-strict"` для пилотов, использующих только PQ, с
  одобрением управления.
- Помощники шлюза также раскрывают `scoreboardOutPath` и `scoreboardNowUnixSecs`.
  Задайте `scoreboardOutPath` для сохранения вычисленного табло (соответствует флагу
  CLI `--scoreboard-out`), чтобы `cargo xtask sorafs-adoption-check` мог валидировать
  SDK-артефакты и воспользуйтесь `scoreboardNowUnixSecs`, когда требуются приспособления.
  стабильное значение `assume_now` для воспроизведения метаданных. В помощнике JavaScript можно
  дополнительно установите `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  если метка опущена, он выводит `region:<telemetryRegion>` (резервный вариант на `sdk:js`). Помощник Python
  автоматически записывает `telemetry_source="sdk:python"` при сохранении табло и держит
  неявные метаданные выключенными.

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