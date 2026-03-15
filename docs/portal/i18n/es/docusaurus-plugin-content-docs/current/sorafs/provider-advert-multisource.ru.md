---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Мульти-источниковые объявления провайдеров и планирование

Esta página tiene especificaciones técnicas en
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilice este documento para el archivo Norito y el registro de cambios; версия портала
Aquí hay instrucciones de operación, archivos SDK y dispositivos de telemetría para el sistema.
набора runbooks SoraFS.

## Дополнения к схеме Norito

### Диапазонная возможность (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – intervalo máximo (baйты) en zapatos, `>= 1`.
- `min_granularity` – búsqueda de búsqueda, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets`: no hay compensaciones necesarias en el modo de cierre.
- `requires_alignment` – если true, compensaciones должны выравниваться по `min_granularity`.
- `supports_merkle_proof` – указывает поддержку свидетельств PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` обеспечивают каноническое
кодирование, чтобы payloads chismes оставались детерминированными.

### `StreamBudgetV1`
- Polo: `max_in_flight`, `max_bytes_per_sec`, opcional `burst_bytes`.
- Правила валидации (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` o `> 0` y `<= max_bytes_per_sec`.

### `TransportHintV1`
- Polo: `protocol: TransportProtocol`, `priority: u8` (de 0 a 15 controles
  `TransportHintV1::validate`).
- Protocolos actuales: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Дублирующиеся записи протоколов на провайдера отклоняются.### Дополнения к `ProviderAdvertBodyV1`
- Opcional `stream_budget: Option<StreamBudgetV1>`.
- Opcional `transport_hints: Option<Vec<TransportHintV1>>`.
- Оба поля проходят через `ProviderAdmissionProposalV1`, sobres de gobernanza,
  Accesorios CLI y JSON telemétricos.

## Validez y privacidad de la gobernanza

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
отклоняют поврежденные метаданные:

- Ampliación de los requisitos de decoración correctos y límites superiores
  диапазона/гранулярности.
- Presupuestos de flujo / sugerencias de transporte требуют TLV `CapabilityType::ChunkRangeFetch`
  и непустого списка sugerencias.
- Duplicados protocolos de transporte y prioridades no correctas en el uso de objetos
  валидации до chismes рассылки anuncios.
- Sobres de admisión сравнивают propuestas/anuncios по диапазонным метаданным через
  `compare_core_fields`, чтобы несовпадающие gossip payloads отклонялись заранее.

Регрессионное покрытие находится в
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Instrumentos y accesorios- Cargas útiles объявлений провайдеров должны включать `range_capability`, `stream_budget`
  y `transport_hints`. Pruebe через ответы `/v2/sorafs/providers` y accesorios de admisión;
  JSON-резюме должны включать разобранную capacidad, presupuesto de transmisión y sugerencias masivas
  для телеметрического ingerir.
- `cargo xtask sorafs-admission-fixtures` выводит presupuestos de flujo y sugerencias de transporte
  En muchos artefactos JSON, estos paneles de control están equipados con funciones novedosas.
- Accesorios en `fixtures/sorafs_manifest/provider_admission/` теперь включают:
  - anuncios publicitarios canónicos,
  - `multi_fetch_plan.json`, el SDK contiene muchos parámetros de configuración
    plan de búsqueda multi-peer.

## Integración con el orquestador y Torii

- Torii `/v2/sorafs/providers` возвращает разобранные метаданные диапазонных возможностей
  вместе с `stream_budget` y `transport_hints`. Предупреждения degradación срабатывают, когда
  Los proveedores utilizan nuevas metadanas, un rango de puntos finales que se basa en la organización.
  для прямых клиентов.
- Мульти-источниковый оркестратор (`sorafs_car::multi_fetch`) теперь применяет лимиты дапазона,
  выравнивание возможностей и flujo de presupuestos при распределении работы. Unit-testы покрывают
  случаи слишком больших trozo, разреженного buscar y estrangular.
- `sorafs_car::multi_fetch` permite bajar la señal (ошибки выравнивания,
  estrangulado запросы), чтобы операторы могли понимать, почему конкретные провайдеры
  были пропущены при планировании.

## Справочник телеметрииBúsqueda de rango de instrumentos en Torii en el tablero de Grafana **SoraFS Observabilidad de búsqueda**
(`dashboards/grafana/sorafs_fetch_observability.json`) y alertas actualizadas
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Consejo | Metálicas | Descripción |
|---------|-----|-------|----------|
| `torii_sorafs_provider_range_capability_total` | Calibre | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Провайдеры, объявляющие funkции диапазонной возможности. |
| `torii_sorafs_range_fetch_throttle_events_total` | Mostrador | `reason` (`quota`, `concurrency`, `byte_rate`) | La recuperación de rango popular con aceleración, сгруппированные по политике. |
| `torii_sorafs_range_fetch_concurrency_current` | Calibre | — | Активные защищенные потоки, потребляющие общий бюджет конкуренции. |

Primeros PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Si no se acelera, se pueden acelerar los tiempos antes de que se aceleren.
дефолтов мульти-источникового оркестратора, и поднимайте алерты, когда конкуренция
приближается к максимальным значениям flujo de presupuesto по вашей флотилии.