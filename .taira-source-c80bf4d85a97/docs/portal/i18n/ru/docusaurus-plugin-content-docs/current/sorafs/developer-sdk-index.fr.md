---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-разработчика SDK
заголовок: Руководства SDK SoraFS
Sidebar_label: Руководства SDK
описание: Дополнительные языковые возможности для интеграции артефактов SoraFS.
---

:::note Источник канонический
:::

Используйте этот концентратор для управления помощниками на языке книг с помощью цепочки инструментов SoraFS.
Для фрагментов Rust используйте [Фрагменты Rust SDK] (./developer-sdk-rust.md).

## Помощники по языку

- **Python** — `sorafs_multi_fetch_local` (тестирует дым локального оркестратора) и др.
  `sorafs_gateway_fetch` (упражнения шлюза E2E) принимаются некорректно
  Опция `telemetry_region` плюс переопределение `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` или `"direct-only"`), в мире ручек
  развертывание CLI. Lorsqu'un proxy QUIC местный демарр,
  `sorafs_gateway_fetch` отправьте манифест навигации через
  `local_proxy_manifest` Afin que les tests Transmettent le Trust Bundle aux
  адаптеры навигации.
- **JavaScript** — `sorafsMultiFetchLocal` отображает вспомогательный Python, отсылает к файлам
  байты полезной нагрузки и резюме результатов, а также `sorafsGatewayFetch` упражнения
  шлюзы Torii, пропустите манифесты локального прокси и откройте мемы
  переопределяет телеметрию/транспорт, используемый CLI.
- **Rust** — службы могут блокировать управление планировщиком через
  `sorafs_car::multi_fetch` ; проконсультируйтесь по ссылке
  [Фрагменты Rust SDK](./developer-sdk-rust.md) для проверки потока помощников и др.
  Интеграция оркестратора.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` повторно использовать HTTP-исполнитель.
  Torii и уважение `GatewayFetchOptions`. Комбинез-ле-Авек
  `ClientConfig.Builder#setSorafsGatewayUri` и индекс загрузки PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) загрузка файлов doivent
  rester sur des chemins только для PQ.

## Табло и политические ручки

Помощники Python (`sorafs_multi_fetch_local`) и JavaScript
(`sorafsMultiFetchLocal`) открывает табло планировщика на базе телеметрии
параметр CLI:- Активные производственные бинеры - табло по умолчанию; определенный
  `use_scoreboard=True` (или четыре входа `telemetry`) для повторов
  Светильники afin que le helper приведут к порядку, когда поставщики будут участвовать в
  метадонические рекламные объявления и недавние снимки телеметрии.
- Définissez `return_scoreboard=True` для получения расчетов с помощью весов
  извлекает фрагмент из журналов CI, записывающих диагностику.
- Используйте таблицы `deny_providers` или `boost_providers` для очистки одноранговых узлов.
  или добавьте `priority_delta`, когда планировщик выбран поставщиками.
- Сохранять положение по умолчанию `"soranet-first"` в случае перехода на более раннюю версию; Фурниссе
  `"direct-only"` выберите соответствующий регион, чтобы исключить реле или
  для повторения резервного варианта SNNet-5a и резерва `"soranet-strict"` для дополнительных пилотов
  Только PQ с одобрением управления.
- Шлюз помощников открыт на Aussi `scoreboardOutPath` и `scoreboardNowUnixSecs`.
  Définissez `scoreboardOutPath` для сохранения табло в расчете (miroir du flag
  CLI `--scoreboard-out`) afin que `cargo xtask sorafs-adoption-check` действительные артефакты
  SDK и использование `scoreboardNowUnixSecs` для всех необходимых устройств.
  `assume_now` стабильный для воспроизводимых метадонных материалов. В помощнике JavaScript,
  vous pouvez aussi définir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` ;
  Если метка отсутствует, он получит `region:<telemetryRegion>` (с резервной версией `sdk:js`).
  Помощник Python работает автоматически `telemetry_source="sdk:python"`, что нужно сделать
  сохраняйте табло и следите за скрытыми метадонами.

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