---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/direct-mode-pack.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ca64d5e6af0a6d5c153dd1c427102132f973af31be2ba34d319b8c7f15b989a1
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: direct-mode-pack
lang: es
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note Fuente canónica
Esta página refleja `docs/source/sorafs/direct_mode_pack.md`. Mantén ambas copias sincronizadas.
:::

Los circuitos SoraNet siguen siendo el transporte predeterminado para SoraFS, pero el ítem del roadmap **SNNet-5a** requiere un fallback regulado para que los operadores mantengan un acceso de lectura determinista mientras se completa el despliegue de anonimato. Este paquete recoge los knobs de CLI / SDK, los perfiles de configuración, las pruebas de cumplimiento y la lista de despliegue necesarios para ejecutar SoraFS en modo directo Torii/QUIC sin tocar los transportes de privacidad.

El fallback aplica a staging y a entornos de producción regulados hasta que SNNet-5 a SNNet-9 superen sus gates de preparación. Mantén los artefactos de abajo junto con el material habitual de despliegue de SoraFS para que los operadores puedan alternar entre los modos anónimo y directo bajo demanda.

## 1. Flags de CLI y SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` desactiva la programación de relés y fuerza los transportes Torii/QUIC. La ayuda del CLI ahora lista `direct-only` como valor aceptado.
- Los SDK deben establecer `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` siempre que expongan un toggle de "modo directo". Los bindings generados en `iroha::ClientOptions` y `iroha_android` reenvían el mismo enum.
- Los harnesses de gateway (`sorafs_fetch`, bindings de Python) pueden interpretar el toggle direct-only mediante los helpers Norito JSON compartidos para que la automatización reciba el mismo comportamiento.

Documenta el flag en runbooks orientados a partners y canaliza los toggles a través de `iroha_config` en lugar de variables de entorno.

## 2. Perfiles de política del gateway

Usa JSON de Norito para persistir una configuración determinista del orquestador. El perfil de ejemplo en `docs/examples/sorafs_direct_mode_policy.json` codifica:

- `transport_policy: "direct_only"` — rechaza proveedores que solo anuncian transportes de relé SoraNet.
- `max_providers: 2` — limita los peers directos a los endpoints Torii/QUIC más fiables. Ajusta según las concesiones de compliance regionales.
- `telemetry_region: "regulated-eu"` — etiqueta las métricas emitidas para que dashboards y auditorías distingan las ejecuciones de fallback.
- Presupuestos de reintento conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar enmascarar gateways mal configurados.

Carga el JSON mediante `sorafs_cli fetch --config` (automatización) o los bindings del SDK (`config_from_json`) antes de exponer la política a los operadores. Persiste la salida del scoreboard (`persist_path`) para las trazas de auditoría.

Los knobs de enforcement del lado del gateway están recogidos en `docs/examples/sorafs_gateway_direct_mode.toml`. La plantilla refleja la salida de `iroha app sorafs gateway direct-mode enable`, deshabilitando las comprobaciones de envelope/admission, cableando los valores por defecto de rate-limit y poblando la tabla `direct_mode` con hostnames derivados del plan y digests del manifest. Sustituye los valores de marcador de posición con tu plan de rollout antes de versionar el fragmento en la gestión de configuración.

## 3. Suite de pruebas de cumplimiento

La preparación de modo directo ahora incluye cobertura tanto en el orquestador como en los crates de CLI:

- `direct_only_policy_rejects_soranet_only_providers` garantiza que `TransportPolicy::DirectOnly` falle rápido cuando cada advert candidato solo admite relés SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` asegura que se usen transportes Torii/QUIC cuando estén presentes y que los relés SoraNet se excluyan de la sesión.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` analiza `docs/examples/sorafs_direct_mode_policy.json` para asegurar que la documentación se mantenga alineada con los helpers de utilidad.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` ejercita `sorafs_cli fetch --transport-policy=direct-only` contra un gateway Torii simulado, proporcionando una prueba de humo para entornos regulados que fijan transportes directos.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` envuelve el mismo comando con el JSON de política y la persistencia del scoreboard para la automatización del rollout.

Ejecuta la suite enfocada antes de publicar actualizaciones:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si la compilación del workspace falla por cambios upstream, registra el error bloqueante en `status.md` y vuelve a ejecutar cuando la dependencia se actualice.

## 4. Ejecuciones automatizadas de smoke

La cobertura de CLI por sí sola no revela regresiones específicas del entorno (por ejemplo, deriva de políticas del gateway o desajustes de manifests). Un helper de smoke dedicado vive en `scripts/sorafs_direct_mode_smoke.sh` y envuelve `sorafs_cli fetch` con la política de orquestador de modo directo, la persistencia del scoreboard y la captura de resúmenes.

Ejemplo de uso:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- El script respeta tanto flags de CLI como archivos de configuración key=value (consulta `docs/examples/sorafs_direct_mode_smoke.conf`). Rellena el digest del manifest y las entradas de adverts de proveedor con valores de producción antes de ejecutar.
- `--policy` por defecto es `docs/examples/sorafs_direct_mode_policy.json`, pero se puede suministrar cualquier JSON de orquestador producido por `sorafs_orchestrator::bindings::config_to_json`. El CLI acepta la política vía `--orchestrator-config=PATH`, habilitando ejecuciones reproducibles sin ajustar flags a mano.
- Cuando `sorafs_cli` no está en `PATH`, el helper lo compila desde el crate `sorafs_orchestrator` (perfil release) para que las pruebas de humo ejerciten el plumbing de modo directo que se envía.
- Salidas:
  - Payload ensamblado (`--output`, por defecto `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumen de fetch (`--summary`, por defecto junto al payload) que contiene la región de telemetría y los reportes de proveedores usados como evidencia de rollout.
  - Snapshot de scoreboard persistido en la ruta declarada en el JSON de política (por ejemplo, `fetch_state/direct_mode_scoreboard.json`). Archívalo junto al resumen en tickets de cambio.
- Automatización del gate de adopción: una vez que el fetch termina el helper invoca `cargo xtask sorafs-adoption-check` usando las rutas persistidas de scoreboard y summary. El quorum requerido por defecto es el número de proveedores suministrados en la línea de comandos; anúlalo con `--min-providers=<n>` cuando necesites una muestra mayor. Los informes de adopción se escriben junto al resumen (`--adoption-report=<path>` puede fijar una ubicación personalizada) y el helper pasa `--require-direct-only` por defecto (coincidiendo con el fallback) y `--require-telemetry` siempre que suministres el flag correspondiente. Usa `XTASK_SORAFS_ADOPTION_FLAGS` para reenviar argumentos adicionales de xtask (por ejemplo `--allow-single-source` durante un downgrade aprobado para que el gate tolere y haga cumplir el fallback). Solo omite el gate con `--skip-adoption-check` al ejecutar diagnósticos locales; el roadmap exige que cada ejecución regulada en modo directo incluya el bundle del informe de adopción.

## 5. Lista de verificación de despliegue

1. **Congelación de configuración:** guarda el perfil JSON de modo directo en tu repositorio `iroha_config` y registra el hash en tu ticket de cambio.
2. **Auditoría del gateway:** confirma que los endpoints Torii apliquen TLS, TLVs de capacidad y logging de auditoría antes de cambiar a modo directo. Publica el perfil de política del gateway para los operadores.
3. **Aprobación de compliance:** comparte el playbook actualizado con revisores de compliance / regulatorios y captura las aprobaciones para operar fuera del overlay de anonimato.
4. **Dry run:** ejecuta la suite de cumplimiento más un fetch en staging contra proveedores Torii de confianza. Archiva los outputs del scoreboard y los resúmenes del CLI.
5. **Corte en producción:** anuncia la ventana de cambio, cambia `transport_policy` a `direct_only` (si habías optado por `soranet-first`) y monitorea los dashboards de modo directo (latencia de `sorafs_fetch`, contadores de fallos de proveedores). Documenta el plan de rollback para volver a SoraNet-first una vez que SNNet-4/5/5a/5b/6a/7/8/12/13 gradúen en `roadmap.md:532`.
6. **Revisión post-cambio:** adjunta snapshots del scoreboard, resúmenes de fetch y resultados de monitoreo al ticket de cambio. Actualiza `status.md` con la fecha efectiva y cualquier anomalía.

Mantén la lista de verificación junto al runbook `sorafs_node_ops` para que los operadores puedan ensayar el flujo antes de un cambio en vivo. Cuando SNNet-5 llegue a GA, retira el fallback tras confirmar paridad en la telemetría de producción.

## 6. Requisitos de evidencia y gate de adopción

Las capturas de modo directo aún deben cumplir con el gate de adopción SF-6c. Agrupa el scoreboard, el resumen, el envelope de manifest y el informe de adopción en cada ejecución para que `cargo xtask sorafs-adoption-check` pueda validar la postura de fallback. Los campos faltantes hacen fallar el gate, así que registra el metadata esperado en los tickets de cambio.

- **Metadatos de transporte:** `scoreboard.json` debe declarar `transport_policy="direct_only"` (y activar `transport_policy_override=true` cuando forzaste el downgrade). Mantén los campos de política de anonimato emparejados incluso cuando hereden defaults para que los revisores vean si te desviaste del plan de anonimato por etapas.
- **Contadores de proveedores:** Las sesiones solo-gateway deben persistir `provider_count=0` y poblar `gateway_provider_count=<n>` con el número de proveedores Torii usados. Evita editar el JSON a mano: el CLI/SDK ya deriva los conteos y el gate de adopción rechaza capturas que omiten la separación.
- **Evidencia del manifest:** Cuando participen gateways Torii, pasa el `--gateway-manifest-envelope <path>` firmado (o equivalente del SDK) para que `gateway_manifest_provided` y los `gateway_manifest_id`/`gateway_manifest_cid` se registren en `scoreboard.json`. Asegura que `summary.json` lleve el mismo `manifest_id`/`manifest_cid`; la comprobación de adopción falla si cualquiera de los archivos omite el par.
- **Expectativas de telemetría:** Cuando la telemetría acompañe la captura, ejecuta el gate con `--require-telemetry` para que el informe pruebe que se emitieron métricas. Los ensayos en entornos aislados pueden omitir el flag, pero CI y tickets de cambio deben documentar la ausencia.

Ejemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

Adjunta `adoption_report.json` junto al scoreboard, el summary, el manifest envelope y el bundle de logs de smoke. Estos artefactos reflejan lo que aplica el job de adopción en CI (`ci/check_sorafs_orchestrator_adoption.sh`) y mantienen auditables los downgrades de modo directo.
