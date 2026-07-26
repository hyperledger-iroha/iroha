---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: índice-sdk-desarrollador
título: Guías de SDK de SoraFS
sidebar_label: Guías de SDK
descripción: Fragmentos específicos por lenguaje para integrar artefactos de SoraFS.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/developer/sdk/index.md`. Mantén ambas copias sincronizadas.
:::

Usa este hub para seguir los helpers por lenguaje que se envían con el toolchain de SoraFS.
Para fragmentos específicos de Rust ve a [fragmentos de Rust SDK](./developer-sdk-rust.md).

## Ayudantes por lenguaje- **Python** — `sorafs_multi_fetch_local` (pruebas de humo del orquestador local) y
  `sorafs_gateway_fetch` (ejercicios E2E del gateway) ahora aceptan un `telemetry_region`
  opcional más una anulación de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` o `"direct-only"`), reflejando los mandos de
  implementación del CLI. Cuando se levanta un proxy QUIC local,
  `sorafs_gateway_fetch` devuelve el manifiesto del navegador en
  `local_proxy_manifest` para que las pruebas entreguen el paquete de confianza a los adaptadores
  del navegador.
- **JavaScript** — `sorafsMultiFetchLocal` refleja el helper de Python, devolviendo
  bytes de carga útil y resúmenes de recibos, mientras `sorafsGatewayFetch` ejercita
  gateways de Torii, encadena manifests de proxy local y exponen los mismos overrides
  de telemetría/transporte que el CLI.
- **Rust** — los servicios pueden incrustar el planificador directamente vía
  `sorafs_car::multi_fetch`; consulta la referencia de
  [Fragmentos de Rust SDK](./developer-sdk-rust.md) para ayudantes de prueba de flujo e integración
  del orquestador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reutiliza el ejecutor HTTP
  de Torii y respeta `GatewayFetchOptions`. Combínalo con
  `ClientConfig.Builder#setSorafsGatewayUri` y el indicio de subida PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) cuando las subidas deban ceñirse
  a rutas solo PQ.

## Marcador y perillas de política

Los ayudantes de Python (`sorafs_multi_fetch_local`) y JavaScript
(`sorafsMultiFetchLocal`) exponen el marcador del planificador con telemetría usado
por el CLI:- Los binarios de producción habilitan el marcador por defecto; establece
  `use_scoreboard=True` (o proporciona entradas de `telemetry`) al reproducir
  accesorios para que el ayudante derive el orden ponderado de proveedores a partir de
  metadatos de anuncios e instantáneas de telemetría recientes.
- Establece `return_scoreboard=True` para recibir los pesos calculados junto a los
  recibos de chunk y permitir que los registros de CI capturen diagnósticos.
- Usa arreglos `deny_providers` o `boost_providers` para rechazar pares o agregar un
  `priority_delta` cuando el planificador selecciona proveedores.
- Mantén la postura predeterminada `"soranet-first"` salvo que prepare un downgrade;
  proporciona `"direct-only"` solo cuando una región de cumplimiento debe evitar relés
  o al ensayar el fallback SNNet-5a, y reserva `"soranet-strict"` para pilotos PQ-only
  con aprobación de gobernanza.
- Los helpers de gateway también exponen `scoreboardOutPath` y `scoreboardNowUnixSecs`.
  Configure `scoreboardOutPath` para persistir el marcador calculado (refleja el flag
  `--scoreboard-out` del CLI) para que `cargo xtask sorafs-adoption-check` valide artefactos
  de SDK, y usa `scoreboardNowUnixSecs` cuando los accesorios requieren un valor estable de
  `assume_now` para metadatos reproducibles. En el ayudante de JavaScript también puedes
  establecer `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; cuando se omite
  la etiqueta deriva `region:<telemetryRegion>` (con respaldo a `sdk:js`). El ayudante dePython emite automáticamente `telemetry_source="sdk:python"` cuando persiste un marcador
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