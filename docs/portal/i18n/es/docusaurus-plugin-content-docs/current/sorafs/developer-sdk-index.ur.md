---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: índice-sdk-desarrollador
título: Guías del SDK SoraFS
sidebar_label: guías del SDK
descripción: Los artefactos SoraFS integran fragmentos de código ۔
---

:::nota مستند ماخذ
:::

اس hub کو استعمال کریں تاکہ SoraFS cadena de herramientas کے ساتھ آنے والے seguimiento de ayudantes de idioma ہو سکیں۔
Fragmentos de Rust کے لیے [fragmentos de Rust SDK](./developer-sdk-rust.md) دیکھیں۔

## Ayudantes de idiomas- **Python** — `sorafs_multi_fetch_local` (pruebas de humo del orquestador local)
  `sorafs_gateway_fetch` (ejercicios de puerta de enlace E2E) o opcional `telemetry_region` o
  `transport_policy` anular قبول کرتے ہیں
  (`"soranet-first"`, `"soranet-strict"` o `"direct-only"`), perillas desplegables CLI en color negro
  Este es el proxy QUIC local y el manifiesto del navegador `sorafs_gateway_fetch`.
  `local_proxy_manifest` میں واپس کرتا ہے تاکہ pruebas paquete de confianza کو adaptadores de navegador تک
  پہنچا سکیں۔
- **JavaScript** — `sorafsMultiFetchLocal` Ayudante de Python کو mirror کرتا ہے، bytes de carga útil
  اور resúmenes de recibos واپس کرتا ہے، جبکہ `sorafsGatewayFetch` Torii gateways کو ejercicio کرتا ہے،
  manifiestos de proxy local کو hilo کرتا ہے، اور CLI جیسے anulaciones de telemetría/transporte exponen کرتا ہے۔
- **Rust** — programador de servicios کو براہ راست `sorafs_car::multi_fetch` کے ذریعے incrustar کر سکتے ہیں؛
  ayudantes de flujo de prueba e integración del orquestador کے لیے [fragmentos de Rust SDK](./developer-sdk-rust.md) دیکھیں۔
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` Torii Reutilización del ejecutor HTTP کرتا ہے اور
  `GatewayFetchOptions` کو honor کرتا ہے۔ اسے `ClientConfig.Builder#setSorafsGatewayUri` اور
  Sugerencia de carga de PQ (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) کے ساتھ combinar کریں جب cargas
  کو Rutas exclusivas de PQ پر رکھنا ضروری ہو۔

## Cuadro de indicadores y botones de política

Ayudantes de Python (`sorafs_multi_fetch_local`) y JavaScript (`sorafsMultiFetchLocal`) CLI کے
Marcador del programador con reconocimiento de telemetría کو exponer کرتے ہیں:- Marcador de binarios de producción predeterminado طور پر habilitar کرتے ہیں؛ repetición de partidos کرتے وقت
  `use_scoreboard=True` (یا `telemetry` entradas) دیں تاکہ metadatos de anuncios auxiliares اور telemetría reciente
  instantáneas سے pedido de proveedor ponderado derivar کرے۔
- `return_scoreboard=True` establece pesos calculados, recibos de fragmentos, registros de CI
  captura de diagnóstico کر سکیں۔
- `deny_providers` یا `boost_providers` matrices استعمال کریں تاکہ pares rechazan ہوں یا `priority_delta`
  agregue ہو جب proveedores de programación seleccione کرے۔
- Postura predeterminada `"soranet-first"` برقرار رکھیں جب تک etapa de degradación نہ ہو؛ `"direct-only"` صرف تب دیں
  جب región de cumplimiento کو relés سے بچنا ہو یا ensayo de reserva SNNet-5a ہو، اور `"soranet-strict"`
  کو Pilotos exclusivos de PQ کے لیے aprobación de gobernanza کے ساتھ reserva کریں۔
- Ayudantes de puerta de enlace `scoreboardOutPath` اور `scoreboardNowUnixSecs` بھی exponen کرتے ہیں۔ `scoreboardOutPath`
  establecer کریں تاکہ marcador calculado persistir ہو (CLI `--scoreboard-out` flag کی طرح) اور
  Los artefactos del SDK `cargo xtask sorafs-adoption-check` validan کر سکے، اور `scoreboardNowUnixSecs` تب دیں جب
  accesorios کو metadatos reproducibles کے لیے valor `assume_now` estable چاہیے ہو۔ Asistente de JavaScript میں
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` بھی conjunto کیے جا سکتے ہیں؛ اگر etiqueta omitir ہو تو
  Y `region:<telemetryRegion>` deriva کرتا ہے (respaldo `sdk:js`). Ayudante de Python جب marcador persiste کرتا ہے تو
  `telemetry_source="sdk:python"` خودکار طور پر emitir کرتا ہے اور metadatos implícitos کو deshabilitados رکھتا ہے۔```python
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