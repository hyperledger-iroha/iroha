---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Anuncios de proveedores multi-origen y agendamento

Esta pagina resume una especificacao canonica em
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Utilice este documento para esquemas Norito palabra por palabra y registros de cambios; una copia del portal
Mantenga la orientación para los operadores, notas de SDK y referencias de telemetría correspondientes al resto.
dos runbooks SoraFS.

## Adicoes ao esquema Norito

### Capacidad de alcance (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - mayor intervalo continuo (bytes) por solicitud, `>= 1`.
- `min_granularity` - resolución de búsqueda, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - permite compensaciones nao contiguos em uma requisicao.
- `requires_alignment` - Cuando es verdadero, las compensaciones devem alinhar com `min_granularity`.
- `supports_merkle_proof` - indica soporte a testemunhas PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplicación de codificación canónica
para que payloads de chismes permanecam deterministas.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcionales.
- Registros de validación (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, cuando presente, debe ser `> 0` e `<= max_bytes_per_sec`.

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8` (janela 0-15 aplicada por
  `TransportHintV1::validate`).
- Protocolos conhecidos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Entradas duplicadas de protocolo por proveedor sao rejeitadas.### Adicoes a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Ambos os campos agora passam por `ProviderAdmissionProposalV1`, sobres de gobierno,
  Accesorios de CLI y JSON de telemetría.

## Validacao e vinculacao comgobernanza

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
metadatos rejeitam malformados:

- Las capacidades de alcance deben decodificarse y cumplir los límites de amplitud/granularidad.
- Presupuestos de flujo/sugerencias de transporte exigen un TLV `CapabilityType::ChunkRangeFetch`
  corresponsal y lista de sugerencias nao vazia.
- Protocolos de transporte duplicados y prioridades invalidadas en caso de errores de validación
  antes de los anuncios sonem chismes.
- Sobres de admisión comparar propuesta/anuncios para metadatos de rango vía
  `compare_core_fields` para que payloads de chismes divergentes sejam rejeitados cedo.

Una cobertura de regressao vive em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Herramientas y accesorios- Las cargas útiles de anuncios de proveedor deben incluir metadatos `range_capability`,
  `stream_budget` e `transport_hints`. Validar vía respuestas de `/v2/sorafs/providers`
  e accesorios de admisión; Los resúmenes JSON deben incluir una capacidad analizada o transmitida.
  presupuesto y matrices de sugerencias para ingestao de telemetría.
- `cargo xtask sorafs-admission-fixtures` muestra presupuestos de flujo y sugerencias de transporte dentro
  Sus artefactos JSON para que los paneles de control acompañen la incorporación de una función.
- Los accesorios sollozan `fixtures/sorafs_manifest/provider_admission/` ahora incluyen:
  - anuncios canónicos de múltiples orígenes,
  - `multi_fetch_plan.json` para que las suites de SDK reproduzcan un plano de recuperación
    determinístico multi-peer.

## Integracao com orquestador e Torii

- Torii `/v2/sorafs/providers` retorna metadatos de rango parseada junto com
  `stream_budget` e `transport_hints`. Avisos de downgrade disparam quando
  Los proveedores omiten los nuevos metadatos y los puntos finales de alcance de las aplicaciones de puerta de enlace.
  mesmas restringidas para clientes directos.
- O Orchestrator Multi-Origem (`sorafs_car::multi_fetch`) aplica ahora límites de
  rango, mejora de capacidades y presupuestos de flujo para atribuir trabajo. Unidad
  Prueba cobrem escenarios de trozos muy grandes, búsqueda dispersa y estrangulamiento.
- `sorafs_car::multi_fetch` transmite sinais de downgrade (falhas de alinhamento,
  requisicoes throttled) para que operadores rastreiem por que proveedores especificos
  foram ignorados durante el planejamento.

## Referencia de telemetriaA instrumentacao de range fetch de Torii alimenta o tablero Grafana
**SoraFS Obtención de observabilidad** (`dashboards/grafana/sorafs_fetch_observability.json`) e
como regras de alerta asociadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Tipo | Etiquetas | Descripción |
|---------|------|--------|-----------|
| `torii_sorafs_provider_range_capability_total` | Calibre | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Proveedores que anunciam características de capacidad de alcance. |
| `torii_sorafs_range_fetch_throttle_events_total` | Mostrador | `reason` (`quota`, `concurrency`, `byte_rate`) | Tentativas de rango de alcance estrangulado agrupadas por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Calibre | - | Streams ativos protegidos consumiendo o presupuesto compartilhado de concorrencia. |

Ejemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilice el contador de aceleración para confirmar la aplicación de cuotas antes de activar
aos defaults do Orchestrator multi-origem e alerta cuando a concorrencia se aproxima
dos máximos del presupuesto de flujo da sua frota.