---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/developer-sdk-index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f7bebf8db52d96c1473aade55890fd3e357a0a82e7f6e85d736c92d0aa3abe9e
source_last_modified: "2025-11-14T04:43:21.669324+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/developer/sdk/index.md`. Mantén ambas copias sincronizadas.
:::

Usa este hub para seguir los helpers por lenguaje que se envían con el toolchain de SoraFS.
Para snippets específicos de Rust ve a [Rust SDK snippets](./developer-sdk-rust.md).

## Helpers por lenguaje

- **Python** — `sorafs_multi_fetch_local` (tests de humo del orquestador local) y
  `sorafs_gateway_fetch` (ejercicios E2E del gateway) ahora aceptan un `telemetry_region`
  opcional más un override de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` o `"direct-only"`), reflejando los knobs de
  rollout del CLI. Cuando se levanta un proxy QUIC local,
  `sorafs_gateway_fetch` devuelve el manifest del navegador en
  `local_proxy_manifest` para que los tests entreguen el trust bundle a los adaptadores
  del navegador.
- **JavaScript** — `sorafsMultiFetchLocal` refleja el helper de Python, devolviendo
  bytes de payload y resúmenes de recibos, mientras `sorafsGatewayFetch` ejercita
  gateways de Torii, encadena manifests de proxy local y expone los mismos overrides
  de telemetría/transporte que el CLI.
- **Rust** — los servicios pueden incrustar el scheduler directamente vía
  `sorafs_car::multi_fetch`; consulta la referencia de
  [Rust SDK snippets](./developer-sdk-rust.md) para helpers de proof-stream e integración
  del orquestador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reutiliza el ejecutor HTTP
  de Torii y respeta `GatewayFetchOptions`. Combínalo con
  `ClientConfig.Builder#setSorafsGatewayUri` y el hint de subida PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) cuando las subidas deban ceñirse
  a rutas solo PQ.

## Scoreboard y knobs de política

Los helpers de Python (`sorafs_multi_fetch_local`) y JavaScript
(`sorafsMultiFetchLocal`) exponen el scoreboard del scheduler con telemetría usado
por el CLI:

- Los binarios de producción habilitan el scoreboard por defecto; establece
  `use_scoreboard=True` (o proporciona entradas de `telemetry`) al reproducir
  fixtures para que el helper derive el orden ponderado de proveedores a partir de
  metadatos de adverts y snapshots de telemetría recientes.
- Establece `return_scoreboard=True` para recibir los pesos calculados junto a los
  recibos de chunk y permitir que los logs de CI capturen diagnósticos.
- Usa arreglos `deny_providers` o `boost_providers` para rechazar peers o añadir un
  `priority_delta` cuando el scheduler selecciona proveedores.
- Mantén la postura predeterminada `"soranet-first"` salvo que prepares un downgrade;
  proporciona `"direct-only"` solo cuando una región de compliance deba evitar relays
  o al ensayar el fallback SNNet-5a, y reserva `"soranet-strict"` para pilotos PQ-only
  con aprobación de gobernanza.
- Los helpers de gateway también exponen `scoreboardOutPath` y `scoreboardNowUnixSecs`.
  Configura `scoreboardOutPath` para persistir el scoreboard calculado (refleja el flag
  `--scoreboard-out` del CLI) para que `cargo xtask sorafs-adoption-check` valide artefactos
  de SDK, y usa `scoreboardNowUnixSecs` cuando los fixtures requieran un valor estable de
  `assume_now` para metadatos reproducibles. En el helper de JavaScript también puedes
  establecer `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; cuando se omite
  la etiqueta deriva `region:<telemetryRegion>` (con fallback a `sdk:js`). El helper de
  Python emite automáticamente `telemetry_source="sdk:python"` cuando persiste un scoreboard
  y mantiene deshabilitados los metadatos implícitos.

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
