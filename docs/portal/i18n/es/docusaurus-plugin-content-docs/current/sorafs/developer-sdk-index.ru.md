---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: índice-sdk-desarrollador
título: Руководства по SDK SoraFS
sidebar_label: Configuración del SDK
descripción: Языковые сниппеты для интеграции артефактов SoraFS.
---

:::nota Канонический источник
:::

Utilice esta herramienta para eliminar los asistentes de idioma instalados en la cadena de herramientas SoraFS.
Los fragmentos de código avanzados de Rust incluyen [fragmentos del SDK de Rust] (./developer-sdk-rust.md).

## ayudantes de Языковые- **Python** — `sorafs_multi_fetch_local` (pruebas de humo локального оркестратора) и
  `sorafs_gateway_fetch` (puerta de enlace E2E actualizada) modelo de configuración opcional
  `telemetry_region` más anulación para `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` o `"direct-only"`), otras perillas desplegables
  CLI. Un potente servidor proxy QUIC local, `sorafs_gateway_fetch`
  manifiesto del navegador en `local_proxy_manifest`, algunas pruebas pueden eliminar el paquete de confianza
  браузерным адаптерам.
- **JavaScript** — `sorafsMultiFetchLocal` utiliza el asistente de Python, incluye bytes de carga útil
  y resúmenes квитанций, тогда как `sorafsGatewayFetch` упражняет Torii gateways,
  прокидывает manifiesta el proxy local y раскрывает те же anulaciones de telemetría/transporte,
  что и CLI.
- **Rust** — сервисы могут встраивать planificador напрямую через `sorafs_car::multi_fetch`;
  см. [Fragmentos de Rust SDK](./developer-sdk-rust.md) para ayudantes de flujo de prueba e integración
  orquestador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` ejecutable Torii Ejecutor HTTP
  и учитывает `GatewayFetchOptions`. Комбинируйте с
  `ClientConfig.Builder#setSorafsGatewayUri` y sugerencia de carga PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) когда загрузки должны идти по PQ-only путям.

## Cuadro de indicadores y botones de política

Ayudantes de Python (`sorafs_multi_fetch_local`) y JavaScript (`sorafsMultiFetchLocal`)
выставляют marcador del programador compatible con telemetría, CLI disponible:- Producción binaria включают marcador по умолчанию; instalación `use_scoreboard=True`
  (или передайте entradas `telemetry`) при проигрывании accesorios, чтобы ayudante выводил
  взвешенный порядок провайдеров из metadatos anuncios y varias instantáneas de telemetría.
- Установите `return_scoreboard=True`, чтобы получать рассчитанные веса вместе с trozos de recibos,
  позволяя CI логам фиксировать диагностику.
- Utilice las masas `deny_providers` o `boost_providers` para la exclusión de sus pares o para otras personas.
  `priority_delta`, когда planificador выбирает провайдеров.
- Сохраняйте позу `"soranet-first"` по умолчанию, если только не готовите downgrade;
  указывайте `"direct-only"` лишь когда conformidad región обязан избегать relés или при
  repeticiones de respaldo SNNet-5a y reserva `"soranet-strict"` para pilotos solo PQ
  одобрением gobernanza.
- Los asistentes de puerta de enlace incluyen `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Задайте `scoreboardOutPath` для сохранения вычисленного marcador (соответствует флагу
  CLI `--scoreboard-out`), puede validar el `cargo xtask sorafs-adoption-check`
  Artefactos SDK, e implementado `scoreboardNowUnixSecs`, accesorios de когда требуется
  стабильное значение `assume_now` для воспроизводимых metadatos. В Ayudante de JavaScript можно
  дополнительно установить `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  Esta etiqueta está activada en `region:<telemetryRegion>` (respaldo en `sdk:js`). ayudante de pitónавтоматически пишет `telemetry_source="sdk:python"` при сохранении marcador y держит
  metadatos implícitos выключенными.

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