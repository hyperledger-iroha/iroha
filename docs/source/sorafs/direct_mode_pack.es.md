---
lang: es
direction: ltr
source: docs/source/sorafs/direct_mode_pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 25b819d27b4456839f8c142f7676d0d37b962695eea2d2482fd9dcbd75e31805
source_last_modified: "2025-11-22T08:38:05.762995+00:00"
translation_last_reviewed: "2026-01-30"
---

# Pack de fallback de modo directo SoraFS (SNNet-5a)

Los circuitos SoraNet siguen siendo el transporte default para SoraFS, pero el
item del roadmap **SNNet-5a** requiere un fallback regulado para que los
operadores mantengan acceso de lectura determinista mientras se completa el
rollout de anonimato. Este pack captura los knobs de CLI/SDK, perfiles de
configuracion, tests de compliance y checklist de despliegue necesarios para
correr SoraFS en modo Torii/QUIC directo sin tocar los transportes de
privacidad.

El fallback aplica a entornos de staging y produccion regulada hasta que
SNNet-5 a SNNet-9 superen sus gates de readiness. Mantener los artefactos
abajo junto al collateral de despliegue SoraFS usual para que los operadores
puedan alternar entre modos anonimos y directos bajo demanda.

## 1. Flags de CLI y SDK

- `sorafs_cli fetch --transport-policy=direct-only …` deshabilita el scheduling
  de relays y fuerza transportes Torii/QUIC. El help de CLI ahora lista
  `direct-only` como valor aceptado.
- Los SDKs deben setear
  `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)`
  cuando expongan un toggle de “direct mode”. Los bindings generados en
  `iroha::ClientOptions` y `iroha_android` reenvian el mismo enum.
- Los harnesses de gateway (`sorafs_fetch`, bindings Python) pueden parsear el
  toggle direct-only via helpers Norito JSON compartidos para que la
  automatizacion reciba el comportamiento identico.

Documentar el flag en runbooks orientados a partners y cablear feature toggles
via `iroha_config` en lugar de variables de entorno.

## 2. Perfiles de politica de gateway

Usar Norito JSON para persistir configuracion determinista del orquestador. El
nuevo perfil de ejemplo vive en `docs/examples/sorafs_direct_mode_policy.json` y
codifica:

- `transport_policy: "direct_only"` — rechaza providers que solo anuncian
  transportes relay SoraNet.
- `max_providers: 2` — limita peers directos a los endpoints Torii/QUIC mas
  confiables. Ajustar segun allowances de compliance regional.
- `telemetry_region: "regulated-eu"` — etiqueta metricas emitidas para que
  dashboards/ auditorias distingan corridas fallback.
- Budgets de retry conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`)
  para evitar ocultar gateways mal configurados.

Cargar el JSON via `sorafs_cli fetch --config` (automatizacion) o bindings SDK
(`config_from_json`) antes de exponer la politica a operadores. Persistir el
output del scoreboard (`persist_path`) para trails de auditoria.

Los knobs de enforcement del lado gateway se capturan en
`docs/examples/sorafs_gateway_direct_mode.toml`. La plantilla espeja el output
 de `iroha app sorafs gateway direct-mode enable`, deshabilitando checks de
 envelope/admission, cableando defaults de rate-limit y poblando la tabla
 `direct_mode` con hostnames y digests de manifiesto derivados del plan.
 Reemplaza los valores placeholder con el plan de rollout antes de commitear
 el snippet al sistema de gestion de configuracion.

## 3. Suite de tests de compliance

La readiness de modo directo ahora incluye cobertura en los crates de
orquestador y CLI:

- `direct_only_policy_rejects_soranet_only_providers` garantiza que
  `TransportPolicy::DirectOnly` falle rapido cuando cada advert candidato solo
  soporta relays SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` asegura que
  transportes Torii/QUIC se usen cuando estan presentes y que relays SoraNet
  queden excluidos de la sesion.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` parsea
  `docs/examples/sorafs_direct_mode_policy.json` para asegurar que la
  documentacion se mantiene alineada con los helpers.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` ejerce `sorafs_cli fetch
  --transport-policy=direct-only` contra un gateway Torii mockeado, dando un
  smoke test para entornos regulados que pinnean transportes directos.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` envuelve el mismo comando con el
  policy JSON y persistencia del scoreboard para automatizacion de rollout.

Ejecutar el suite enfocado antes de publicar updates:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si la compilacion del workspace falla por cambios upstream, registrar el error
bloqueante en `status.md` y re-ejecutar cuando la dependencia se ponga al dia.

## 4. Smoke runs automatizados

La cobertura de CLI sola no expone regresiones especificas del entorno (p. ej.,
policy drift del gateway o mismatches de manifiesto). Un helper de smoke
 dedicado vive en `scripts/sorafs_direct_mode_smoke.sh` y envuelve `sorafs_cli
 fetch` con politica de orquestador direct-mode, persistencia de scoreboard y
 captura de resumen.

Ejemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- El script respeta flags de CLI y archivos config key=value (ver
  `docs/examples/sorafs_direct_mode_smoke.conf`). Poblar digest de manifiesto y
  entradas de provider advert con valores de produccion antes de correr.
- `--policy` default a `docs/examples/sorafs_direct_mode_policy.json`, pero se
  puede suministrar cualquier JSON de orquestador producido por
  `sorafs_orchestrator::bindings::config_to_json`. El CLI ahora acepta la
  politica via `--orchestrator-config=PATH`, habilitando corridas reproducibles
  sin tunear flags a mano.
- Cuando `sorafs_cli` no esta en `PATH` el helper lo build-ea desde el crate
  `sorafs_orchestrator` (perfil release) para que el smoke run use la politica
  direct-mode y plumbing de scoreboard shipping.
- Outputs:
  - Payload ensamblado (`--output`, default a
    `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumen de fetch (`--summary`, default junto al payload) que contiene la
    region de telemetria y reportes de provider usados como evidencia de rollout.
  - Snapshot de scoreboard persistido en la ruta declarada en el policy JSON
    (p. ej., `fetch_state/direct_mode_scoreboard.json`). Archivar junto al
    resumen en tickets de cambio.
- Automatizacion de gate de adopcion: una vez que el fetch termina el helper
  invoca `cargo xtask sorafs-adoption-check` usando las rutas persistidas de
  scoreboard y summary. El quorum requerido default al numero de providers
  suministrados en la linea de comandos; override con `--min-providers=<n>`
  cuando se necesita una muestra mayor. Los reportes de adopcion se escriben
  junto al resumen (`--adoption-report=<path>` puede fijar ubicacion custom) y
  el helper pasa `--require-direct-only` por defecto (alineado al modo fallback)
  y `--require-telemetry` cuando se provee el flag CLI correspondiente.
  Usa `XTASK_SORAFS_ADOPTION_FLAGS` para reenviar argumentos adicionales de
  xtask (por ejemplo `--allow-single-source` durante un downgrade aprobado para
  que el gate de adopcion tanto tolere como haga cumplir el fallback). Solo
  omitir el gate con `--skip-adoption-check` cuando se ejecuten diagnosticos
  locales; el roadmap requiere que toda corrida direct-mode regulada incluya el
  bundle de reporte de adopcion.

## 5. Checklist de rollout

1. **Freeze de configuracion:** almacenar el perfil JSON de direct-mode en tu
   repo `iroha_config` y registrar el hash en el ticket de cambio.
2. **Audit de gateway:** confirmar que endpoints Torii hacen cumplir TLS, TLVs
   de capacidades y logging de auditoria antes de activar direct mode. Publicar
   el perfil de politica del gateway a operadores.
3. **Sign-off de compliance:** compartir el playbook actualizado con reviewers
   de compliance/regulatorio y capturar aprobaciones para correr fuera del
   overlay de anonimato.
4. **Dry run:** ejecutar el suite de tests de compliance mas un fetch staging
   contra providers Torii conocidos. Archivar outputs de scoreboard y resúmenes
   de CLI.
5. **Cutover de produccion:** anunciar la ventana de cambio, cambiar
   `transport_policy` a `direct_only` (si se habia optado por `soranet-first`),
   y monitorear dashboards direct-mode (latencia `sorafs_fetch`, counters de
   fallas de provider). Documentar el plan de rollback para volver a
   SoraNet-first cuando SNNet-4/5/5a/5b/6a/7/8/12/13 gradue en
   `roadmap.md:532`.
6. **Post-change review:** adjuntar snapshots de scoreboard, resúmenes de fetch y
   resultados de monitoreo al ticket de cambio. Actualizar `status.md` con la
   fecha efectiva y cualquier anomalia.

Mantener el checklist junto al runbook `sorafs_node_ops` para que operadores
ensayen el workflow antes de un switchover en vivo. Cuando SNNet-5 gradue a GA,
retirar el fallback tras confirmar paridad en telemetria de produccion.

## 6. Requisitos de evidencia y adoption gate

Las corridas direct-mode siguen sujetas a los checks de adopcion SF-6c. Cada
captura que dependa del fallback debe empaquetar el scoreboard, summary, reporte
 de adopcion y artefactos de manifiesto que `cargo xtask sorafs-adoption-check`
requiere. El gate falla cuando los campos abajo faltan o son inconsistentes, por
eso registrarlos en el ticket de cambio junto a logs CLI/SDK.

- **Metadata de transporte:** `scoreboard.json` debe anunciar
  `transport_policy="direct_only"` (y setear `transport_policy_override=true`
  cuando el CLI/SDK forzo el downgrade). Mantener los flags
  `anonymity_policy`/`anonymity_policy_override` intactos aun cuando se heredan
  del perfil base para que governance detecte drift del plan de anonimato stageado.
- **Contadores de provider:** capturas solo-gateway deben persistir
  `provider_count=0` y poblar `gateway_provider_count=<n>` con el numero de
  providers Torii en la corrida. No editar el scoreboard a mano—`sorafs_cli fetch`
  y `sorafs_fetch` derivan los conteos del roster resuelto, y el gate de adopcion
  rechaza recordings que omitan el split porque son indistinguibles de capturas
  mixed-mode.
- **Evidencia de manifiesto:** al invocar gateways Torii, pasar el
  `--gateway-manifest-envelope <path>` firmado (o la opcion equivalente de SDK)
  para que la metadata setee `gateway_manifest_provided=true` y registre
  `gateway_manifest_id`/`gateway_manifest_cid`. `summary.json` debe llevar el
  mismo par `manifest_id`/`manifest_cid`; mismatches hacen fallar
  `cargo xtask sorafs-adoption-check`.
- **Expectativas de telemetria:** si la captura incluye un stream OpenTelemetry,
  correr el adoption check con `--require-telemetry` para que el reporte pruebe
  que las metricas emitidas se ingirieron (el comando avisa cuando falta
  telemetria, lo cual es requerido para ensayos air-gapped).

Ejemplo de invocacion del adoption check:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Adjuntar el `adoption_report.json` resultante al ticket de rollout junto con el
scoreboard, summary, sobre de manifiesto y bundle de smoke-log. Estos campos
 dan a governance review la misma evidencia que el job de adopcion en CI
(`ci/check_sorafs_orchestrator_adoption.sh`) y mantienen auditables los
downgrades de modo directo.
