---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: índice-sdk-desarrollador
título: Guías SDK SoraFS
sidebar_label: SDK de guías
descripción: Extraits par langage pour intégrer les artefactos SoraFS.
---

:::nota Fuente canónica
:::

Utilice este centro para ayudar a los asistentes en libros de idiomas con la cadena de herramientas SoraFS.
Para los fragmentos de Rust, vaya a [fragmentos de Rust SDK] (./developer-sdk-rust.md).

## Ayudantes por idioma- **Python** — `sorafs_multi_fetch_local` (pruebas de humo del orquestador local) y
  `sorafs_gateway_fetch` (ejercicios E2E de gateway) aceptar desormais un
  `telemetry_region` opcional más una anulación de `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` o `"direct-only"`), en el espejo de las perillas de
  implementación de CLI. Lorsqu'un proxy QUIC inicio local,
  `sorafs_gateway_fetch` envíe el navegador de manifiesto a través de
  `local_proxy_manifest` afin que les tests transmettent le trust bundle aux
  Navegador adaptadores.
- **JavaScript** — `sorafsMultiFetchLocal` refleja el asistente Python, actualizando archivos
  bytes de carga útil y currículums de recursos, tandis que `sorafsGatewayFetch` ejercita
  Las puertas de enlace Torii, pasan los manifiestos del proxy local y exponen los mismos.
  anula la télémétrie/transporte que le CLI.
- **Rust**: los servicios pueden embarcar el programador directamente a través de
  `sorafs_car::multi_fetch`; consulte la referencia
  [Fragmentos de Rust SDK](./developer-sdk-rust.md) para los asistentes de prueba de flujo y
  la integración del orquestador.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` vuelve a utilizar el ejecutor HTTP
  Torii y respeto `GatewayFetchOptions`. Combinez-le avec
  `ClientConfig.Builder#setSorafsGatewayUri` y el índice de carga PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) cuando se cargan archivos
  rester sur des chemins PQ-only.

## Cuadro de indicadores y mandos políticosLos ayudantes de Python (`sorafs_multi_fetch_local`) y JavaScript
(`sorafsMultiFetchLocal`) muestra el marcador del programador basado en télémétrie utilizado
por la CLI:- Los binarios de producción activan el marcador por defecto; definir
  `use_scoreboard=True` (ou fournissez des entrées `telemetry`) al reproducir des
  accesorios afin que le helper derivan el orden ponderado de los proveedores a partir de
  métadonnées d'advert y des snapshots de télémétrie récents.
- Définissez `return_scoreboard=True` para recibir los pesos calculados con les
  Reçus de chunk afin que los registros CI capturan los diagnósticos.
- Utilice las tablas `deny_providers` o `boost_providers` para rechazar pares
  o agregue un `priority_delta` cuando el programador seleccione los proveedores.
- Conserve la postura por defecto `"soranet-first"` salvo en caso de downgrade; fournissez
  `"direct-only"` solo cuando una región de conformidad debe evitar los relés o
  Lors d'une répétition du fallback SNNet-5a, y reserve `"soranet-strict"` aux pilotes
  PQ únicamente con aprobación de gobierno.
- Las puertas de enlace de ayudantes están expuestas además de `scoreboardOutPath` y `scoreboardNowUnixSecs`.
  Définissez `scoreboardOutPath` para persistir en el cálculo del marcador (mirar la bandera)
  CLI `--scoreboard-out`) después de que `cargo xtask sorafs-adoption-check` valide los artefactos
  SDK y uso `scoreboardNowUnixSecs` cuando los dispositivos necesitan un valor óptimo
  `assume_now` estable para metadonnées reproducibles. En el ayudante de JavaScript,
  También puedes definir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;Si se omite la etiqueta, se deriva `region:<telemetryRegion>` (con la versión alternativa `sdk:js`).
  El asistente Python se abre automáticamente `telemetry_source="sdk:python"` cada vez que lo haces
  persista en un marcador y guarde las métadonnées implícitamente desactivadas.

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