---
lang: es
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2026-01-03T18:07:56.917770+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Carril de cálculo (SSC-1)

La línea de cálculo acepta llamadas deterministas de estilo HTTP y las asigna a Kotodama.
puntos de entrada y registros de medición/recibos para revisión de facturación y gobernanza.
Este RFC congela el esquema de manifiesto, los sobres de llamada/recibo, las barreras de seguridad del sandbox,
y valores de configuración predeterminados para la primera versión.

## Manifiesto

- Esquema: `crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`).
- `abi_version` está fijado a `1`; Se rechazan los manifiestos con una versión diferente.
  durante la validación.
- Cada ruta declara:
  - `id` (`service`, `method`)
  - `entrypoint` (nombre del punto de entrada Kotodama)
  - lista de códecs permitidos (`codecs`)
  - Tapas TTL/gas/solicitud/respuesta (`ttl_slots`, `gas_budget`, `max_*_bytes`)
  - determinismo/clase de ejecución (`determinism`, `execution_class`)
  - Descriptores de ingreso/modelo SoraFS (`input_limits`, `model` opcional)
  - familia de precios (`price_family`) + perfil de recursos (`resource_profile`)
  - política de autenticación (`auth`)
- Las barandillas de Sandbox se encuentran en el bloque manifiesto `sandbox` y son compartidas por todos
  rutas (modo/aleatoriedad/almacenamiento y rechazo de llamada al sistema no determinista).

Ejemplo: `fixtures/compute/manifest_compute_payments.json`.

## Llamadas, solicitudes y recibos

- Esquema: `ComputeRequest`, `ComputeCall`, `ComputeCallSummary`, `ComputeReceipt`,
  `ComputeMetering`, `ComputeOutcome` en
  `crates/iroha_data_model/src/compute/mod.rs`.
- `ComputeRequest::hash()` produce el hash de solicitud canónico (los encabezados se mantienen
  en un determinista `BTreeMap` y la carga útil se transporta como `payload_hash`).
- `ComputeCall` captura el espacio de nombres/ruta, códec, TTL/gas/límite de respuesta,
  perfil de recursos + familia de precios, autenticación (`Public` o vinculado a UAID
  `ComputeAuthn`), determinismo (`Strict` vs `BestEffort`), clase de ejecución
  sugerencias (CPU/GPU/TEE), bytes/fragmentos de entrada SoraFS declarados, patrocinador opcional
  presupuesto, y el sobre de solicitud canónica. El hash de solicitud se utiliza para
  protección de reproducción y enrutamiento.
- Las rutas pueden incorporar referencias de modelo SoraFS opcionales y límites de entrada
  (tapas en línea/en trozos); Las reglas manifiestas de la zona de pruebas abren sugerencias de GPU/TEE.
- `ComputePriceWeights::charge_units` convierte los datos de medición en cálculo facturado
  unidades a través de división de techo en ciclos y bytes de salida.
- `ComputeOutcome` informa `Success`, `Timeout`, `OutOfMemory`,
  `BudgetExhausted` o `InternalError` y opcionalmente incluye hashes de respuesta/
  tamaños/códec para auditoría.

Ejemplos:
- Llamada: `fixtures/compute/call_compute_payments.json`
- Recibo: `fixtures/compute/receipt_compute_payments.json`

## Sandbox y perfiles de recursos- `ComputeSandboxRules` bloquea el modo de ejecución en `IvmOnly` de forma predeterminada,
  genera aleatoriedad determinista a partir del hash de solicitud, permite solo lectura SoraFS
  acceso y rechaza llamadas al sistema no deterministas. Las sugerencias de GPU/TEE están controladas por
  `allow_gpu_hints`/`allow_tee_hints` para mantener la ejecución determinista.
- `ComputeResourceBudget` establece límites por perfil en ciclos, memoria lineal, pila
  tamaño, presupuesto de IO y salida, además de opciones para sugerencias de GPU y ayudantes WASI-lite.
- Los valores predeterminados incluyen dos perfiles (`cpu-small`, `cpu-balanced`) en
  `defaults::compute::resource_profiles` con respaldos deterministas.

## Unidades de fijación de precios y facturación

- Las familias de precios (`ComputePriceWeights`) asignan ciclos y bytes de salida al proceso
  unidades; los valores predeterminados cargan `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)` con
  `unit_label = "cu"`. Las familias están codificadas por `price_family` en manifiestos y
  aplicado en el momento de la admisión.
- Los registros de medición llevan `charged_units` más ciclo/ingreso/salida/duración sin procesar
  totales para la conciliación. Los cargos se amplifican por la clase de ejecución y
  multiplicadores de determinismo (`ComputePriceAmplifiers`) y limitados por
  `compute.economics.max_cu_per_call`; la salida está sujeta por
  `compute.economics.max_amplification_ratio` a amplificación de respuesta ligada.
- Los presupuestos del patrocinador (`ComputeCall::sponsor_budget_cu`) se aplican contra
  límites por llamada/diarios; las unidades facturadas no deben exceder el presupuesto declarado del patrocinador.
- Las actualizaciones de precios de gobernanza utilizan los límites de clase de riesgo en
  `compute.economics.price_bounds` y las familias de referencia registradas en
  `compute.economics.price_family_baseline`; utilizar
  `ComputeEconomics::apply_price_update` para validar deltas antes de actualizar
  el mapa familiar activo. Uso de actualizaciones de configuración Torii
  `ConfigUpdate::ComputePricing`, y kiso lo aplica con los mismos límites a
  mantener las ediciones de gobernanza deterministas.

## Configuración

La nueva configuración informática se encuentra en `crates/iroha_config/src/parameters`:

- Vista de usuario: `Compute` (`user.rs`) con anulaciones de entorno:
  - `COMPUTE_ENABLED` (predeterminado `false`)
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- Precios/economía: capturas `compute.economics`
  `max_cu_per_call`/`max_amplification_ratio`, división de tarifas, límites de patrocinadores
  (CU por llamada y diaria), líneas base de familias de precios + clases/límites de riesgo para
  actualizaciones de gobernanza y multiplicadores de clase de ejecución (GPU/TEE/mejor esfuerzo).
- Real/valores predeterminados: exposición `actual.rs`/`defaults.rs::compute` analizada
  Configuración `Compute` (espacios de nombres, perfiles, familias de precios, sandbox).
- Configuraciones no válidas (espacios de nombres vacíos, falta perfil/familia predeterminados, límite de TTL)
  inversiones) aparecen como `InvalidComputeConfig` durante el análisis.

## Pruebas y accesorios

- Los ayudantes deterministas (`request_hash`, fijación de precios) y los recorridos de ida y vuelta de los dispositivos viven en
  `crates/iroha_data_model/src/compute/mod.rs` (ver `fixtures_round_trip`,
  `request_hash_is_stable`, `pricing_rounds_up_units`).
- Los dispositivos JSON se encuentran en `fixtures/compute/` y son ejercidos por el modelo de datos.
  pruebas de cobertura de regresión.

## Arnés y presupuestos de SLO- La configuración `compute.slo.*` expone las perillas SLO de la puerta de enlace (cola en vuelo
  profundidad, límite de RPS y objetivos de latencia) en
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`. Valores predeterminados: 32
  en vuelo, 512 en cola por ruta, 200 RPS, p50 25 ms, p95 75 ms, p99 120 ms.
- Ejecute el arnés de banco liviano para capturar resúmenes de SLO y una solicitud/salida
  instantánea: `cargo run -p xtask --bin compute_gateway -- bench [manifest_path]
  [iteraciones] [concurrencia] [out_dir]` (defaults: `fixtures/compute/manifest_compute_paids.json`,
  128 iteraciones, concurrencia 16, salidas bajo
  `artifacts/compute_gateway/bench_summary.{json,md}`). El banco utiliza
  cargas útiles deterministas (`fixtures/compute/payload_compute_payments.json`) y
  encabezados por solicitud para evitar colisiones de repetición durante el ejercicio
  Puntos de entrada `echo`/`uppercase`/`sha3`.

## Accesorios de paridad SDK/CLI

- Los dispositivos canónicos se encuentran bajo `fixtures/compute/`: manifiesto, llamada, carga útil y
  el diseño de respuesta/recibo estilo puerta de enlace. Los hashes de carga útil deben coincidir con la llamada
  `request.payload_hash`; la carga útil del ayudante vive en
  `fixtures/compute/payload_compute_payments.json`.
- La CLI envía `iroha compute simulate` e `iroha compute invoke`:

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` vive en
  `javascript/iroha_js/src/compute.js` con pruebas de regresión bajo
  `javascript/iroha_js/test/computeExamples.test.js`.
- Swift: `ComputeSimulator` carga los mismos dispositivos, valida hashes de carga útil,
  y simula los puntos de entrada con pruebas en
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`.
- Todos los asistentes CLI/JS/Swift comparten los mismos dispositivos Norito para que los SDK puedan
  validar la construcción de la solicitud y el manejo de hash fuera de línea sin llegar a un
  puerta de enlace en ejecución.