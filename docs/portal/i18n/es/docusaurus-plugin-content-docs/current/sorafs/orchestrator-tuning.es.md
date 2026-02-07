---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: afinación del orquestador
título: Despliegue y ajuste del orquestador
sidebar_label: Ajuste del orquestador
descripción: Valores predeterminados prácticos, guía de ajuste y puntos de auditoría para llevar el orquestador multiorigen a GA.
---

:::nota Fuente canónica
Refleja `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantenga ambas copias alineadas hasta que se retire el conjunto de documentación heredado.
:::

# Guía de despliegue y ajuste del orquestador

Esta guía se basa en la [referencia de configuración](orchestrator-config.md) y el
[runbook de implementación multiorigen](multi-source-rollout.md). explica
cómo ajustar el orquestador para cada fase de despliegue, cómo interpretar los
artefactos del marcador y qué señales de telemetría deben estar listas antes
de ampliar el tráfico. Aplique las recomendaciones de forma consistente en la
CLI, los SDK y la automatización para que cada nodo siga la misma política de
buscar determinista.

## 1. Conjuntos de parámetros base

Parte de una plantilla de configuración compartida y ajusta un conjunto pequeño
de perillas a medida que progresa el despliegue. La tabla siguiente recoge los
valores recomendados para las fases más comunes; los valores no listados vuelven
a los predeterminados en `OrchestratorConfig::default()` y `FetchOptions::default()`.| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|------|-----------------|-------------------------------|------------------------------------|-----------------------|------------------------------|-------|
| **Laboratorio/CI** | `3` | `2` | `2` | `2500` | `300` | Un límite de latencia y una ventana de gracia estrictos exponen telemetría ruidosa rápidamente. Mantén los reintentos bajos para descubrir manifiestos inválidos antes. |
| **Puesta en escena** | `4` | `3` | `3` | `4000` | `600` | Refleja los valores de producción dejando margen para pares exploratorios. |
| **Canarias** | `6` | `3` | `3` | `5000` | `900` | Igual a los valores por defecto; Configure `telemetry_region` para que los paneles puedan segmentar el tráfico canario. |
| **Disponibilidad general** | `None` (usar todos los elegibles) | `4` | `4` | `5000` | `900` | Incrementa los umbrales de reintentos y fallos para absorber fallos transitorios mientras las auditorías siguen reforzando el determinismo. |- `scoreboard.weight_scale` se mantiene en el valor predeterminado `10_000` salvo que un sistema aguas abajo requiera otra resolución entera. Aumentar la escala no cambia el orden de los proveedores; solo emite una distribución de créditos más densa.
- Al migrar entre fases, persista el paquete JSON y usa `--scoreboard-out` para que el rastro de auditoría registre el conjunto exacto de parámetros.

## 2. Higiene del marcador

El marcador combina requisitos del manifiesto, anuncios de proveedores y telemetría.
Antes de avanzar:1. **Valida la frescura de la telemetría.** Asegúrate de que las instantáneas referenciados por
   `--telemetry-json` fueron capturados dentro de la ventana de gracia configurada. Las entradas
   más antiguas que `telemetry_grace_secs` fallan con `TelemetryStale { last_updated }`.
   Trátalo como un bloqueo duro y actualiza la exportación de telemetría antes de continuar.
2. **Inspecciona los motivos de elegibilidad.** Persiste los artefactos con
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. cada entrada
   Incluye un bloque `eligibility` con la causa exacta del fallo. No sobreescribas
   desajustes de capacidades o anuncios caducados; Corrija la carga útil aguas arriba.
3. **Revisa los cambios de peso.** Compara el campo `normalised_weight` con el
   soltar anterior. Desplazamientos de peso >10% deben correlacionarse con cambios
   deliberados en anuncios o telemetría y registrarse en el log de despliegue.
4. **Archiva los artefactos.** Configura `scoreboard.persist_path` para que cada
   ejecución emite la instantánea final del marcador. Adjunta el artefacto al registro
   de liberación junto al manifiesto y el paquete de telemetría.
5. **Registra la evidencia de la mezcla de proveedores.** La metadata de `scoreboard.json`
   y el `summary.json` correspondiente deben exponer `provider_count`,
   `gateway_provider_count` y la etiqueta derivada `provider_mix` para que los revisoresprueben si la ejecución fue `direct-only`, `gateway-only` o `mixed`. Las capturas de
   gateway reportan `provider_count=0` y `provider_mix="gateway-only"`, mientras que las
   las ejecuciones mixtas requieren conteos no cero para ambos orígenes. `cargo xtask sorafs-adoption-check`
   impone estos campos (y falla si los conteos/etiquetas no coinciden), así que ejecútalo
   siempre junto con `ci/check_sorafs_orchestrator_adoption.sh` o tu script de captura para
   producir el paquete de evidencia `adoption_report.json`. Cuando haya puertas de enlace Torii,
   conserva `gateway_manifest_id`/`gateway_manifest_cid` en los metadatos del marcador
   para que el gate de adopción pueda correlacionar el sobre del manifiesto con la
   mezcla de proveedores capturada.

Para definiciones detalladas de campos, consulte
`crates/sorafs_car/src/scoreboard.rs` y la estructura de resumen de CLI expuesta por
`sorafs_cli fetch --json-out`.

## Referencia de banderas de CLI y SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) y el envoltorio
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) comparte la
misma superficie de configuración del orquestador. Usa los siguientes flags al
capture evidencia de despliegue o al reproducir los aparatos canónicos:

Referencia compartida de flags multi-origen (mantén la ayuda de CLI y los docs en
sincronía editando solo este archivo):- `--max-peers=<count>` limita cuántos proveedores elegibles sobreviven al filtro del marcador. Déjalo sin configurar para transmitir desde todos los proveedores elegibles y ponlo en `1` solo cuando se ejerza intencionalmente el fallback de una sola fuente. Refleja la perilla `maxPeers` en los SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` reenvía al límite de reintentos por trozo que aplica `FetchOptions`. Utilice la tabla de rollout en la guía de ajuste para valores recomendados; las ejecuciones de CLI que recopilan evidencia deben coincidir con los valores por defecto de los SDK para mantener la paridad.
- `--telemetry-region=<label>` etiqueta las series Prometheus `sorafs_orchestrator_*` (y los relés OTLP) con una etiqueta de región/entorno para que los tableros separen tráfico de lab, staging, canary y GA.
- `--telemetry-json=<path>` inyecta la instantánea referenciada por el marcador. Persiste el JSON junto al marcador para que los auditores puedan reproducir la ejecución (y para que `cargo xtask sorafs-adoption-check --require-telemetry` pruebe qué stream OTLP alimentó la captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) habilitan los ganchos del puente observador. Cuando se configura, el orquestador transmite fragmentos a través del proxy local Norito/Kaigi para que los clientes de navegador, guard caches y salas Kaigi reciban los mismos recibos emitidos por Rust.- `--scoreboard-out=<path>` (opcionalmente con `--scoreboard-now=<unix_secs>`) persiste la instantánea de elegibilidad para los auditores. Emparejar siempre el JSON persistido con los artefactos de telemetría y manifiesto referenciados en el ticket de liberación.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplican ajustes deterministas sobre los metadatos de anuncios. Usa estas banderas solo para ensayos; las degradaciones en producción deben pasar por artefactos de gobernanza para que cada nodo aplique el mismo paquete de políticas.
- `--provider-metrics-out` / `--chunk-receipts-out` conservan las métricas de salud por proveedor y los recibos de trozos referenciados por la checklist de rollout; ambos artefactos se adjuntarán al presentar la evidencia de adopción.

Ejemplo (usando el accesorio publicado):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Los SDK consumen la misma configuración mediante `SorafsGatewayFetchOptions` en
el cliente Rust (`crates/iroha/src/client.rs`), las fijaciones JS
(`javascript/iroha_js/src/sorafs.js`) y el SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantén esas ayudas en
sincronización con los valores por defecto de la CLI para que los operadores puedan
Copie políticas entre automatización sin capas de traducción a medida.

## 3. Ajuste de la política de búsqueda

`FetchOptions` controla el comportamiento de reintentos, concurrencia y verificación.
Al ajustar:- **Reintentos:** Elevar `per_chunk_retry_limit` por encima de `4` aumenta el tiempo
  de recuperación pero puede ocultar fallos de proveedores. Prefiere mantener `4`
  como techo y confiar en la rotación de proveedores para detectar a los de bajo rendimiento.
- **Umbral de fallos:** `provider_failure_threshold` determina cuándo un proveedor
  se deshabilita para el resto de la sesión. Alinea este valor con la política de
  reintentos: un umbral menor que el presupuesto de reintentos obliga al orquestador
  a expulsar un peer antes de agotar todos los reintentos.
- **Concurrencia:** Deja `global_parallel_limit` sin configurar (`None`) a menos
  que un entorno específico no pueda saturar los rangos anunciados. cuando se
  configure, asegúrese de que el valor sea ≤ a la suma de los presupuestos de
  corrientes de los proveedores para evitar la inanición.
- **Toggles de verificación:** `verify_lengths` y `verify_digests` deben permanecer
  habilitados en producción. Garantizan el determinismo cuando hay flotas mixtas
  de proveedores; solo desactívalos en entornos de fuzzing aislados.

## 4. Etapas de transporte y anonimato

Usa los campos `rollout_phase`, `anonymity_policy` y `transport_policy` para
representar la postura de privacidad:- Prefiere `rollout_phase="snnet-5"` y permite que la política de anonimato por
  defecto siga los hitos de SNNet-5. Sobrescribir con `anonymity_policy_override`
  solo cuando la gobernanza emite una directiva firmada.
- Mantén `transport_policy="soranet-first"` como base mientras SNNet-4/5/5a/5b/6a/7/8/12/13 estén 🈺
  (ver `roadmap.md`). Usa `transport_policy="direct-only"` solo para degradaciones
  documentadas o simulacros de cumplimiento, y espera la revisión de cobertura PQ
  antes de promover `transport_policy="soranet-strict"`—ese nivel fallará rápido si
  solo quedan relés clásicos.
- `write_mode="pq-only"` solo debe imponerse cuando cada ruta de escritura (SDK,
  orquestador, tooling de gobernanza) pueda satisfacer los requisitos PQ. durante
  los rollouts mantén `write_mode="allow-downgrade"` para que las respuestas de
  emergencia puedan apoyarse en rutas directas mientras la telemetría marca la
  degradación.
- La selección de guardias y la preparación de circuitos dependen del directorio.
  de SoraNet. Proporciona el snapshot firmado de `relay_directory` y persiste la
  cache de `guard_set` para que el churn de guardias se mantenga dentro de la
  ventana de retención acordada. La huella del caché registrado por
  `sorafs_cli fetch` forma parte de la evidencia de implementación.

## 5. Ganchos de degradación y cumplimiento

Dos subsistemas del orquestador ayudan a imponer la política sin intervención manual:- **Remediación de degradaciones** (`downgrade_remediation`): monitorea eventos
  `handshake_downgrade_total` y, después de que el `threshold` se configure
  excede dentro de `window_secs`, fuerza el proxy local al `target_mode` (por
  solo metadatos de defecto). Mantenga los valores predeterminados (`threshold=3`,
  `window=300`, `cooldown=900`) salvo que los postmortems indiquen un patrón
  distinto. Documente cualquier anulación en el registro de implementación y asegúrese de que
  Los tableros indican `sorafs_proxy_downgrade_state`.
- **Política de cumplimiento** (`compliance`): los carve-outs por jurisdicción y
  El manifiesto fluye a través de listas de exclusión administradas por la gobernanza.
  Nunca insertes anulaciones ad hoc en el paquete de configuración; en su lugar,
  solicita una actualizacion firmada de
  `governance/compliance/soranet_opt_outs.json` y vuelve a desplegar el JSON generado.

Para ambos sistemas, persista el paquete de configuración resultante e inclúyelo
en las evidencias de liberación para que los auditores puedan rastrear cómo se
Activaron las reducciones.

## 6. Telemetría y paneles de control

Antes de ampliar el despliegue, confirme que las siguientes señales están activas
en el entorno objetivo:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  debe ser cero después de que complete el canario.
- `sorafs_orchestrator_retries_total` y
  `sorafs_orchestrator_retry_ratio` — deben estabilizarse por debajo del 10%
  durante el canario y mantenerse por debajo del 5% tras GA.
- `sorafs_orchestrator_policy_events_total` — valida que la etapa de implementación
  esperada está activa (etiqueta `stage`) y registra apagones vía `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — rastrean el suministro de relés PQ
  frente a las expectativas de la política.
- Objetivos de log `telemetry::sorafs.fetch.*` — deben fluir al agregador de logs
  compartido con búsquedas guardadas para `status=failed`.

Carga el tablero canónico de Grafana desde
`dashboards/grafana/sorafs_fetch_observability.json` (exportado en el portal
bajo **SoraFS → Fetch Observability**) para que los selectores de región/manifest,
el heatmap de reintentos por proveedor, los histogramas de latencia de chunks y
los contadores de atascos coinciden con lo que revisa SRE durante los burn-ins.
Conecte las reglas de Alertmanager en `dashboards/alerts/sorafs_fetch_rules.yml`
y valida la sintaxis de Prometheus con `scripts/telemetry/test_sorafs_fetch_alerts.sh`
(el ayudante ejecuta automáticamente `promtool test rules` localmente o en Docker).
Las transferencias de alertas requieren el mismo bloque de enrutamiento que imprime
el script para que los operadores puedan adjuntar la evidencia al ticket de rollout.

### Flujo de burn-in de telemetríaEl elemento de hoja de ruta **SF-6e** requiere un burn-in de telemetría de 30 días antes
de cambiar el orquestador multi-origen a sus valores GA. Usa los scripts del
repositorio para capturar un paquete de artefactos reproducible cada día en la
ventana:

1. Ejecuta `ci/check_sorafs_orchestrator_adoption.sh` con las variables de
   entorno de burn-in configurado. Ejemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```El ayudante reproduce `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   escribe `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` y `adoption_report.json` bajo
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impone un número mínimo de
   proveedores elegibles mediante `cargo xtask sorafs-adoption-check`.
2. Cuando las variables de burn-in están presentes, el script también emite
   `burn_in_note.json`, capturando la etiqueta, el índice de día, el id del
   manifiesto, la fuente de telemetría y los digests de los artefactos. adjunto
   este JSON al log de rollout para que sea evidente qué captura cubría cada día
   de la ventana de 30 días.
3. Importa el tablero de Grafana actualizado (`dashboards/grafana/sorafs_fetch_observability.json`)
   en el espacio de trabajo de staging/producción, etiquétalo con la etiqueta de burn-in
   y confirme que cada panel muestra muestras para el manifiesto/región en prueba.
4. Ejecuta `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o `promtool test rules …`)
   cuando cambie `dashboards/alerts/sorafs_fetch_rules.yml` para documentar que
   el enrutamiento de alertas coincide con las métricas exportadas durante el burn-in.
5. Archiva la instantánea del tablero, la salida de la prueba de alertas y el
   tail de logs de las búsquedas `telemetry::sorafs.fetch.*` junto a los
   artefactos del orquestador para que la gobernanza pueda reproducir la
   evidencia sin extraer métricas de sistemas en vivo.

## 7. Lista de verificación de implementación1. Regenera los marcadores en CI usando la configuración candidata y captura
   los artefactos bajo control de versiones.
2. Ejecuta el fetch determinista de accesorios en cada entorno (laboratorio, preparación,
   canary, producción) y adjunta los artefactos `--scoreboard-out` y `--json-out`
   al registro de lanzamiento.
3. Revisa los paneles de telemetría con el ingeniero de guardia, asegurando que
   todas las métricas anteriores tengan muestras en vivo.
4. Registre la ruta de configuración final (normalmente vía `iroha_config`) y el
   commit git del registro de gobernanza usado para anuncios y cumplimiento.
5. Actualiza el tracker de rollout y notifica a los equipos de SDK sobre los
   nuevos valores por defecto para que las integraciones de clientes se mantengan
   alineadas.

Seguir esta guía mantiene los despliegues del orquestador deterministas y
auditables, mientras aporta ciclos de retroalimentación claros para ajustar
presupuestos de reintentos, capacidad de proveedores y postura de privacidad.