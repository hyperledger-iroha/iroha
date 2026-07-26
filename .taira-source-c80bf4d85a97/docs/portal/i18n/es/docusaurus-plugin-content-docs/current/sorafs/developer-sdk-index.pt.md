---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: índice-sdk-desarrollador
título: Guías de SDK de SoraFS
sidebar_label: Guías de SDK
descripción: Trechos por linguagem para integrar artefatos da SoraFS.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas como copias sincronizadas.
:::

Utilice este centro para acompañar los ayudantes del idioma que acompañan la cadena de herramientas de SoraFS.
Para fragmentos específicos de Rust, va a [Rust SDK snippets](./developer-sdk-rust.md).

## Ayudantes por idioma- **Python** - `sorafs_multi_fetch_local` (las pruebas de humo las realiza el orquestador local) e
  `sorafs_gateway_fetch` (ejercicios de puerta de enlace E2E) ahora aceitam um `telemetry_region`
  opcionalmente más una anulación de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` o `"direct-only"`), tocando las perillas de
  implementación de CLI. Cuando un proxy QUIC local sobe, `sorafs_gateway_fetch` retorna o
  manifiesto del navegador en `local_proxy_manifest` para que las pruebas pasen el paquete de confianza
  para adaptadores de navegador.
- **JavaScript** - `sorafsMultiFetchLocal` espelha o helper de Python, retornando
  bytes de carga útil y currículums de recibos, mientras que el ejercicio `sorafsGatewayFetch`
  gateways Torii, encadeia manifiestos de proxy local y expone los mesmos anulaciones
  de telemetría/transporte de CLI.
- **Rust** - servicios que pueden ejecutarse o programarse directamente a través de
  `sorafs_car::multi_fetch`; veja una referencia de
  [Fragmentos de Rust SDK](./developer-sdk-rust.md) para ayudantes de prueba-stream e
  integracao do orquestador.
- **Android** - `HttpClientTransport.sorafsGatewayFetch(...)` reutiliza el ejecutor HTTP
  do Torii y honra `GatewayFetchOptions`. combinar com
  `ClientConfig.Builder#setSorafsGatewayUri` y sugerencia de carga PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) cuando las cargas son necesarias
  ficar em caminhos somente PQ.

## Marcador y botones de política

Los ayudantes de Python (`sorafs_multi_fetch_local`) y JavaScript
(`sorafsMultiFetchLocal`) expoema o marcador del planificador con telemetría usado
Pelo CLI:- Binarios de producao habilitam o marcador por padrao; definir `use_scoreboard=True`
  (ou forneca entradas `telemetry`) ao reproduzir accesorios para que o ayudante derive
  a ordenacao ponderada de proveedores a partir de metadados de advert e snapshots
  recientes de telemetría.
- Defina `return_scoreboard=True` para recibir los pesos calculados junto con recibos
  de fragmento, lo que permite que los registros de CI capturen diagnósticos.
- Utilice los arrays `deny_providers` o `boost_providers` para eliminar peers o agregar
  `priority_delta` cuando el programador selecciona proveedores.
- Mantenha a postura padrao `"soranet-first"` a menos que esteja preparando um downgrade;
  forneca `"direct-only"` apenas cuando una región de cumplimiento precisar evitar relés
  También puede probar el respaldo SNNet-5a y reservar `"soranet-strict"` para pilotos solo PQ.
  com aprobación de gobierno.
- Los asistentes de puerta de enlace también exponen `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Defina `scoreboardOutPath` para persistir el marcador calculado (espelha o flag
  `--scoreboard-out` de CLI) para que `cargo xtask sorafs-adoption-check` sea válido
  Artefatos de SDK, y uso `scoreboardNowUnixSecs` cuando los accesorios necesitan un
  valor `assume_now` estavel para metadados reproducidos. Sin ayuda de JavaScript,
  voce tambem pode definir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  Cuando la etiqueta se omite, ele deriva `region:<telemetryRegion>` (respaldo para `sdk:js`).El ayudante de Python emite automáticamente `telemetry_source="sdk:python"` cuando
  persiste um scoreboard e mantem metadados implícitos desabilitados.

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