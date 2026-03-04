---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-default-lane-quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-carril-predeterminado-inicio rápido
título: carril predeterminado کوئیک اسٹارٹ (NX-5)
sidebar_label: carril predeterminado کوئیک اسٹارٹ
descripción: Nexus Retorno de carril predeterminado Configurar Verificar Torii SDK carriles públicos Lane_id Omitir Carril público
---

:::nota Fuente canónica
یہ صفحہ `docs/source/quickstart/default_lane.md` کی عکاسی کرتا ہے۔ جب تک barrido de localización پورٹل تک نہیں پہنچتی، دونوں کاپیوں کو alineado رکھیں۔
:::

# carril predeterminado کوئیک اسٹارٹ (NX-5)

> **Contexto de la hoja de ruta:** NX-5: integración de carril público predeterminado۔ tiempo de ejecución con `nexus.routing_policy.default_lane` respaldo, configuración de puntos finales Torii REST/gRPC y SDK y `lane_id` طریقے سے omitir کر سکیں جب ٹریفک carril público canónico سے تعلق رکھتا ہو۔ یہ گائیڈ operadores کو configuración del catálogo کرنے، `/status` میں respaldo verificar کرنے، اور ejercicio de comportamiento del cliente de extremo a extremo کرنے میں رہنمائی کرتی ہے۔

## Requisitos previos

- `irohad` کا Sora/Nexus build (`irohad --sora --config ...` چلائیں).
- repositorio de configuración تک رسائی تاکہ `nexus.*` secciones editar کیے جا سکیں۔
- `iroha_cli` جو cluster de destino سے بات کرنے کے لئے configurado ہو۔
- Torii `/status` carga útil inspeccionar کرنے کے لئے `curl`/`jq` (equivalente a یا).

## 1. carril اور catálogo de espacio de datos بیان کریںred پر موجود ہونے والے carriles اور espacios de datos کو declarar کریں۔ Un fragmento de código (`defaults/nexus/config.toml` سے) de carriles públicos y un registro de alias de espacio de datos coincidente:

```toml
[nexus]
lane_count = 3

[[nexus.lane_catalog]]
index = 0
alias = "core"
description = "Primary execution lane"
dataspace = "universal"

[[nexus.lane_catalog]]
index = 1
alias = "governance"
description = "Governance & parliament traffic"
dataspace = "governance"

[[nexus.lane_catalog]]
index = 2
alias = "zk"
description = "Zero-knowledge attachments"
dataspace = "zk"

[[nexus.dataspace_catalog]]
alias = "universal"
id = 0
description = "Single-lane data space"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "governance"
id = 1
description = "Governance proposals & manifests"
fault_tolerance = 1

[[nexus.dataspace_catalog]]
alias = "zk"
id = 2
description = "Zero-knowledge proofs and attachments"
fault_tolerance = 1
```

ہر `index` منفرد اور ہونا چاہیے۔ Identificadores de espacio de datos con valores de 64 bits ہیں؛ اوپر والے مثالیں وضاحت کے لئے carril índices کے برابر valores numéricos استعمال کرتی ہیں۔

## 2. valores predeterminados de enrutamiento y anulaciones opcionales سیٹ کریں

`nexus.routing_policy` سیکشن carril alternativo کو control کرتا ہے اور مخصوص instrucciones یا prefijos de cuenta کے لئے anulación de enrutamiento کرنے دیتا ہے۔ اگر کوئی regla de coincidencia نہ کرے تو programador ٹرانزیکشن کو configurado `default_lane` اور `default_dataspace` پر ruta کرتا ہے۔ Lógica del enrutador `crates/iroha_core/src/queue/router.rs` میں ہے اور Torii Superficies REST/gRPC پر پالیسی شفاف انداز میں apply کرتا ہے۔

```toml
[nexus.routing_policy]
default_lane = 0                # use the "core" lane when no rules match
default_dataspace = "universal"    # reuse the public dataspace for the fallback

[[nexus.routing_policy.rules]]
lane = 1
dataspace = "governance"
[nexus.routing_policy.rules.matcher]
instruction = "governance"
description = "Route governance instructions to the governance lane"

[[nexus.routing_policy.rules]]
lane = 2
dataspace = "zk"
[nexus.routing_policy.rules.matcher]
instruction = "smartcontract::deploy"
description = "Route contract deployments to the zk lane for proof tracking"
```


## 3. پالیسی کے ساتھ arranque de nodo کریں

```bash
IROHA_CONFIG=/path/to/nexus/config.toml
irohad --sora --config "${IROHA_CONFIG}"
```

inicio del nodo کے دوران política de enrutamiento derivada لاگ کرتا ہے۔ کوئی بھی errores de validación (índices faltantes, alias duplicados, identificadores de espacio de datos no válidos) chismes شروع ہونے سے پہلے سامنے آ جاتے ہیں۔

## 4. estado de gobernanza del carril کنفرم کریں

nodo en línea ہونے کے بعد، CLI helper استعمال کریں تاکہ carril predeterminado sellado (manifiesto cargado) اور tráfico کے لئے listo ہو۔ Vista de resumen ہر carril کے لئے ایک fila پرنٹ کرتا ہے:

```bash
iroha_cli app nexus lane-report --summary
```

Salida de ejemplo:

```
Lane  Alias            Module           Status  Quorum  Validators  Detail
   0  core             parliament       ready      05           07  manifest ok
   1  governance       parliament       ready      05           07  manifest ok
   2  zk               parliament       sealed     03           05  manifest required
```El carril predeterminado `sealed` permite el tráfico externo y el runbook de gobernanza del carril. `--fail-on-sealed` bandera CI کے لئے مفید ہے۔

## 5. Las cargas útiles de estado Torii inspeccionan کریں

`/status` Política de enrutamiento de respuesta اور فی-Lane Scheduler Instantánea دونوں exponer کرتا ہے۔ `curl`/`jq` Los valores predeterminados configurados de la telemetría de carril de respaldo producen:

```bash
curl -s http://127.0.0.1:8080/status | jq '.nexus.routing_policy'
```

Salida de muestra:

```json
{
  "default_lane": 0,
  "default_dataspace": "universal",
  "rules": [
    {"lane": 1, "dataspace": "governance", "matcher": {"instruction": "governance"}},
    {"lane": 2, "dataspace": "zk", "matcher": {"instruction": "smartcontract::deploy"}}
  ]
}
```

carril `0` کے لئے contadores del programador en vivo دیکھنے کے لئے:

```bash
curl -s http://127.0.0.1:8080/status \
  | jq '.nexus_scheduler_lane_teu_status[] | select(.lane_id == 0)
        | {lane_id, alias, dataspace_alias, committed, manifest_ready, scheduler_utilization_pct}'
```

یہ کنفرم کرتا ہے کہ Instantánea de TEU, alias metadatos, اور configuración de indicadores de manifiesto کے ساتھ align ہیں۔ یہی paneles Grafana de carga útil کے tablero de ingesta de carriles میں استعمال ہوتا ہے۔

## 6. Ejercicio de valores predeterminados del cliente کریں- **Rust/CLI.** `iroha_cli` اور Caja de cliente de Rust `lane_id` campo کو omit کرتے ہیں جب آپ `--lane-id` / `LaneSelector` pass نہیں کرتے۔ اس لئے enrutador de cola `default_lane` پر respaldo کرتا ہے۔ Indicadores explícitos `--lane-id`/`--dataspace-id` de carril no predeterminado y de destino y de destino.
- **JS/Swift/Android.** Versiones del SDK `laneId`/`lane_id` Opcional مانتے ہیں اور `/status` میں اعلان کردہ valor de reserva کرتے ہیں۔ Política de enrutamiento کو puesta en escena اور producción میں sincronización رکھیں تاکہ aplicaciones móviles کو reconfiguraciones de emergencia نہ کرنی پڑیں۔
- **Pruebas de canalización/SSE.** filtros de eventos de transacción Predicados `tx_lane_id == <u32>` قبول کرتے ہیں (دیکھیں `docs/source/pipeline.md`). `/v1/pipeline/events/transactions` کو اس filtro کے ساتھ suscribirse کریں تاکہ یہ ثابت ہو کہ carril explícito کے بغیر بھیجی گئی escribe ID de carril alternativo کے تحت پہنچتی ہیں۔

## 7. Ganchos de observabilidad y gobernanza- `/status` `nexus_lane_governance_sealed_total` اور `nexus_lane_governance_sealed_aliases` بھی publicar کرتا ہے تاکہ Alertmanager warn کر سکے جب کوئی carril اپنا manifiesto کھو دے۔ ان alertas کو devnets میں بھی habilitado رکھیں۔
- mapa de telemetría del programador اور panel de control de carril (`dashboards/grafana/nexus_lanes.json`) catálogo کے alias/campos slug esperar کرتے ہیں۔ اگر آپ alias cambiar nombre کریں تو متعلقہ Directorios Kura کو reetiqueta کریں تاکہ auditores rutas deterministas رکھ سکیں (NX-1 کے تحت pista ہوتا ہے)۔
- carriles predeterminados کے لئے aprobaciones del parlamento میں plan de reversión شامل ہونا چاہیے۔ hash manifiesto اور evidencia de gobernanza کو اس inicio rápido کے ساتھ اپنے operador runbook میں registro کریں تاکہ rotaciones futuras مطلوبہ estado کا اندازہ نہ لگائیں۔