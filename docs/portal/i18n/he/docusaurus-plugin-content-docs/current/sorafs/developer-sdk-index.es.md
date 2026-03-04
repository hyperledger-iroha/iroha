---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-index
כותרת: Guías de SDK de SoraFS
sidebar_label: Guías de SDK
תיאור: Fragmentos específicos por lenguaje para integrar artefactos de SoraFS.
---

:::הערה Fuente canónica
Esta página refleja `docs/source/sorafs/developer/sdk/index.md`. Mantén ambas copias sincronizadas.
:::

Usa este hub para seguir los helpers por lenguaje que se envían con el toolchain de SoraFS.
Para snippets específicos de Rust ve a [Rust SDK snippets](./developer-sdk-rust.md).

## Helpers por lenguaje

- **Python** — `sorafs_multi_fetch_local` (בדיקות של הומו דל אורקסטדור מקומי) y
  `sorafs_gateway_fetch` (ejercicios E2E del gateway) ahora aceptan un `telemetry_region`
  אופציונלי לביטול של `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` או `"direct-only"`), relejando los knobs de
  השקה של CLI. יש צורך ב-Proxy QUIC מקומי,
  `sorafs_gateway_fetch` devuelve el manifest del navegador en
  `local_proxy_manifest` para que los tests entreguen el trust bundle a los adaptadores
  del navegador.
- **JavaScript** — `sorafsMultiFetchLocal` רפלג'ה או עוזרת של Python, devolviendo
  bytes de payload y resúmenes de recibos, mientras `sorafsGatewayFetch` ejercita
  gateways de Torii, encadena manifests de proxy local y expone los mismos overrides
  de telemetría/transporte que el CLI.
- **חלודה** - los servicios pueden incrustar el scheduler directamente vía
  `sorafs_car::multi_fetch`; consulta la referencia de
  [קטעי SDK של חלודה](./developer-sdk-rust.md) לעזרה של proof-stream ושילוב
  del orquestador.
- **אנדרואיד** — `HttpClientTransport.sorafsGatewayFetch(…)` השתמש מחדש ב-HTTP למוציא
  de Torii y respeta `GatewayFetchOptions`. קומבינלו קון
  `ClientConfig.Builder#setSorafsGatewayUri` y el hint de subida PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) cuando las subidas deban ceñirse
  a rutas solo PQ.

## לוח תוצאות וידיות פוליטיות

Los helpers de Python (`sorafs_multi_fetch_local`) y JavaScript
(`sorafsMultiFetchLocal`) לוח התוצאות של לוח התוצאות של מתזמן עם טלמטריה בארה"ב
על ידי אל CLI:- Los binarios de producción habilitan el board defecto por defecto; establece
  `use_scoreboard=True` (o proporciona entradas de `telemetry`) לשכפול
  אביזרי para que el helper derive el orden ponderado de proveedores a partir de
  מטה פרסומות וצילומי מצב של טלמטריה.
- Establece `return_scoreboard=True` para recibir los pesos calculados junto a los
  recibos de chunk y permitir que los logs de CI capturen diagnósticos.
- ארה"ב arreglos `deny_providers` o `boost_providers` עבור rechazar peers o añadir un
  `priority_delta` cuando el scheduler selecciona proveedores.
- Mantén la postura predeterminada `"soranet-first"` salvo que מכין את השדרוג לאחור;
  proporciona `"direct-only"` סולו cuando una region de compliance deba evitar relays
  o al ensayar el fallback SNNet-5a, y reserva `"soranet-strict"` para pilotos PQ-only
  con aprobación de gobernanza.
- Los helpers de gateway también exponen `scoreboardOutPath` y `scoreboardNowUnixSecs`.
  Configura `scoreboardOutPath` עבור חישוב לוח תוצאות מתמשך (רפליה אל הדגל
  `--scoreboard-out` del CLI) para que `cargo xtask sorafs-adoption-check` valide artefactos
  de SDK, y usa `scoreboardNowUnixSecs` cuando los fixtures requieran un valor estable de
  `assume_now` עבור ניתנים לשחזור מטאטאטים. En el helper de JavaScript también puedes
  establecer `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; cuando se omite
  la etiqueta deriva `region:<telemetryRegion>` (באמצעות `sdk:js`). אל עוזר דה
  Python emite automáticamente `telemetry_source="sdk:python"` cuando persiste un לוח תוצאות
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