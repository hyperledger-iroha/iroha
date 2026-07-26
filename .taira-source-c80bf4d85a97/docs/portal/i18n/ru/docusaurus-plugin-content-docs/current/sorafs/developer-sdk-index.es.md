---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-разработчика SDK
заголовок: Guías de SDK de SoraFS
Sidebar_label: Руководство по SDK
описание: Специальные фрагменты языка для интеграции артефактов SoraFS.
---

:::примечание Фуэнте каноника
Эта страница отражает `docs/source/sorafs/developer/sdk/index.md`. Mantén ambas copyas sincronizadas.
:::

Это концентратор, позволяющий следить за языком, который вы можете отправить с цепочкой инструментов SoraFS.
Для специальных фрагментов Rust есть [фрагменты Rust SDK] (./developer-sdk-rust.md).

## Помощники по языку

- **Python** — `sorafs_multi_fetch_local` (тесты местного пользователя) y
  `sorafs_gateway_fetch` (отправление шлюза E2E) сейчас принимается `telemetry_region`
  Дополнительное переопределение `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` или `"direct-only"`), отобразите ручки
  развертывание CLI. Когда вы оставите локальный прокси-сервер QUIC,
  `sorafs_gateway_fetch` разработайте манифест навигации в
  `local_proxy_manifest`, чтобы тестировать комплект доверия к адаптерам
  дель Навегадор.
- **JavaScript** — `sorafsMultiFetchLocal` отображает помощник Python, переведенный
  байты полезной нагрузки и резюме получения, сообщения `sorafsGatewayFetch`
  шлюзы Torii, encadena манифестирует локальный прокси и экспонирует переопределения los mismos
  телеметрия/транспортировка по CLI.
- **Rust** — службы могут включить планировщик напрямую через
  `sorafs_car::multi_fetch`; Проконсультируйтесь по ссылке
  [Фрагменты Rust SDK](./developer-sdk-rust.md) для помощи в проверке потока и интеграции
  дель оркесадор.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` повторное использование HTTP-эжектора
  де Torii и ответ `GatewayFetchOptions`. Комбинация с
  `ClientConfig.Builder#setSorafsGatewayUri` и подсказка о субиде PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) cuando las subidas deban ceñirse
  рутас соло PQ.

## Табло и ручки политики

Помощники Python (`sorafs_multi_fetch_local`) и JavaScript
(`sorafsMultiFetchLocal`) Отображение табло планировщика с использованием телеметрии
для CLI:- Произведенные ранее бинарники с табло из-за дефекта; поместье
  `use_scoreboard=True` (или пропорции `telemetry`) при воспроизведении
  приспособления для того, чтобы помощник получил приказ обдумывать условия участия в
  метаданные рекламы и снимки полученных телеметрических данных.
- Establece `return_scoreboard=True` для получения расчетных песо вместе с Лос-Анджелесом
  получение фрагментов и разрешение диагностических журналов CI.
- США arreglos `deny_providers` или `boost_providers` для обмена одноранговыми узлами или доступа к ним
  `priority_delta` при выборе планировщика.
- Подготовьте предопределенную позицию `"soranet-first"` для подготовки к понижению версии;
  Пропорция `"direct-only"` только в том случае, если реле соответствует региону соответствия
  оставьте резервный SNNet-5a и зарезервируйте `"soranet-strict"` для пилотов только для PQ
  с одобрением правительства.
- Помощники шлюза также показаны `scoreboardOutPath` и `scoreboardNowUnixSecs`.
  Настройте `scoreboardOutPath`, чтобы сохранить расчет табло (отображение флага
  `--scoreboard-out` из CLI) для действительных артефактов `cargo xtask sorafs-adoption-check`
  SDK, и США `scoreboardNowUnixSecs`, когда требуются установленные приспособления
  `assume_now` для воспроизводимых метаданных. Помощник по JavaScript также может быть использован
  establecer `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; когда вы опускаете
  la etiqueta deriva `region:<telemetryRegion>` (с резервным вариантом `sdk:js`). Эль помощник де
  Python автоматически выдает `telemetry_source="sdk:python"`, когда сохраняется табло
  и mantiene deshabilitados los Metadatos Implicitos.

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