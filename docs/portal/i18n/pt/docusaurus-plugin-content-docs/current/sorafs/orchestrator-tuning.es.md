---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-tuning
title: Despliegue y ajuste del orquestador
sidebar_label: Ajuste del orquestador
description: Valores predeterminados prácticos, guía de ajuste y puntos de auditoría para llevar el orquestador multi-origen a GA.
---

:::note Fuente canónica
Refleja `docs/source/sorafs/developer/orchestrator_tuning.md`. Mantén ambas copias alineadas hasta que se retire el conjunto de documentación heredado.
:::

# Guía de despliegue y ajuste del orquestador

Esta guía se basa en la [referencia de configuración](orchestrator-config.md) y el
[runbook de despliegue multi-origen](multi-source-rollout.md). Explica
cómo ajustar el orquestador para cada fase de despliegue, cómo interpretar los
artefactos del scoreboard y qué señales de telemetría deben estar listas antes
de ampliar el tráfico. Aplica las recomendaciones de forma consistente en la
CLI, los SDK y la automatización para que cada nodo siga la misma política de
fetch determinista.

## 1. Conjuntos de parámetros base

Parte de una plantilla de configuración compartida y ajusta un conjunto pequeño
de perillas a medida que progresa el despliegue. La tabla siguiente recoge los
valores recomendados para las fases más comunes; los valores no listados vuelven
a los predeterminados en `OrchestratorConfig::default()` y `FetchOptions::default()`.

| Fase | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | Notas |
|------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|-------|
| **Lab / CI** | `3` | `2` | `2` | `2500` | `300` | Un límite de latencia y una ventana de gracia estrictos exponen telemetría ruidosa rápidamente. Mantén los reintentos bajos para descubrir manifiestos inválidos antes. |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | Refleja los valores de producción dejando margen para peers exploratorios. |
| **Canary** | `6` | `3` | `3` | `5000` | `900` | Igual a los valores por defecto; configura `telemetry_region` para que los dashboards puedan segmentar el tráfico canario. |
| **Disponibilidad general** | `None` (usar todos los elegibles) | `4` | `4` | `5000` | `900` | Incrementa los umbrales de reintentos y fallos para absorber fallos transitorios mientras las auditorías siguen reforzando el determinismo. |

- `scoreboard.weight_scale` se mantiene en el valor predeterminado `10_000` salvo que un sistema aguas abajo requiera otra resolución entera. Aumentar la escala no cambia el orden de los proveedores; solo emite una distribución de créditos más densa.
- Al migrar entre fases, persiste el paquete JSON y usa `--scoreboard-out` para que el rastro de auditoría registre el conjunto exacto de parámetros.

## 2. Higiene del scoreboard

El scoreboard combina requisitos del manifiesto, anuncios de proveedores y telemetría.
Antes de avanzar:

1. **Valida la frescura de la telemetría.** Asegúrate de que los snapshots referenciados por
   `--telemetry-json` fueron capturados dentro de la ventana de gracia configurada. Las entradas
   más antiguas que `telemetry_grace_secs` fallan con `TelemetryStale { last_updated }`.
   Trátalo como un bloqueo duro y actualiza la exportación de telemetría antes de continuar.
2. **Inspecciona los motivos de elegibilidad.** Persiste los artefactos con
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. Cada entrada
   incluye un bloque `eligibility` con la causa exacta del fallo. No sobreescribas
   desajustes de capacidades o anuncios expirados; corrige el payload upstream.
3. **Revisa los cambios de peso.** Compara el campo `normalised_weight` con el
   release anterior. Desplazamientos de peso >10 % deben correlacionarse con cambios
   deliberados en anuncios o telemetría y registrarse en el log de despliegue.
4. **Archiva los artefactos.** Configura `scoreboard.persist_path` para que cada
   ejecución emita el snapshot final del scoreboard. Adjunta el artefacto al registro
   de release junto al manifiesto y el paquete de telemetría.
5. **Registra la evidencia de la mezcla de proveedores.** La metadata de `scoreboard.json`
   y el `summary.json` correspondiente deben exponer `provider_count`,
   `gateway_provider_count` y la etiqueta derivada `provider_mix` para que los revisores
   prueben si la ejecución fue `direct-only`, `gateway-only` o `mixed`. Las capturas de
   gateway reportan `provider_count=0` y `provider_mix="gateway-only"`, mientras que las
   ejecuciones mixtas requieren conteos no cero para ambos orígenes. `cargo xtask sorafs-adoption-check`
   impone estos campos (y falla si los conteos/etiquetas no coinciden), así que ejecútalo
   siempre junto con `ci/check_sorafs_orchestrator_adoption.sh` o tu script de captura para
   producir el bundle de evidencia `adoption_report.json`. Cuando haya gateways Torii,
   conserva `gateway_manifest_id`/`gateway_manifest_cid` en la metadata del scoreboard
   para que el gate de adopción pueda correlacionar el sobre del manifiesto con la
   mezcla de proveedores capturada.

Para definiciones detalladas de campos, consulta
`crates/sorafs_car/src/scoreboard.rs` y la estructura de resumen de CLI expuesta por
`sorafs_cli fetch --json-out`.

## Referencia de flags de CLI y SDK

`sorafs_cli fetch` (ver `crates/sorafs_car/src/bin/sorafs_cli.rs`) y el wrapper
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) comparten la
misma superficie de configuración del orquestador. Usa los siguientes flags al
capturar evidencia de despliegue o al reproducir los fixtures canónicos:

Referencia compartida de flags multi-origen (mantén la ayuda de CLI y los docs en
sincronía editando solo este archivo):

- `--max-peers=<count>` limita cuántos proveedores elegibles sobreviven al filtro del scoreboard. Déjalo sin configurar para transmitir desde todos los proveedores elegibles y ponlo en `1` solo cuando se ejerza intencionalmente el fallback de una sola fuente. Refleja la perilla `maxPeers` en los SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` reenvía al límite de reintentos por chunk que aplica `FetchOptions`. Usa la tabla de rollout en la guía de ajuste para valores recomendados; las ejecuciones de CLI que recopilan evidencia deben coincidir con los valores por defecto de los SDK para mantener la paridad.
- `--telemetry-region=<label>` etiqueta las series Prometheus `sorafs_orchestrator_*` (y los relés OTLP) con una etiqueta de región/entorno para que los dashboards separen tráfico de lab, staging, canary y GA.
- `--telemetry-json=<path>` inyecta el snapshot referenciado por el scoreboard. Persiste el JSON junto al scoreboard para que los auditores puedan reproducir la ejecución (y para que `cargo xtask sorafs-adoption-check --require-telemetry` pruebe qué stream OTLP alimentó la captura).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) habilitan los hooks del observador bridge. Cuando se configuran, el orquestador transmite chunks a través del proxy local Norito/Kaigi para que los clientes de navegador, guard caches y salas Kaigi reciban los mismos recibos emitidos por Rust.
- `--scoreboard-out=<path>` (opcionalmente con `--scoreboard-now=<unix_secs>`) persiste el snapshot de elegibilidad para los auditores. Empareja siempre el JSON persistido con los artefactos de telemetría y manifiesto referenciados en el ticket de release.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` aplican ajustes deterministas sobre la metadata de anuncios. Usa estas flags solo para ensayos; las degradaciones en producción deben pasar por artefactos de gobernanza para que cada nodo aplique el mismo paquete de políticas.
- `--provider-metrics-out` / `--chunk-receipts-out` conservan los métricas de salud por proveedor y los recibos de chunks referenciados por la checklist de rollout; adjunta ambos artefactos al presentar la evidencia de adopción.

Ejemplo (usando el fixture publicado):

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
el cliente Rust (`crates/iroha/src/client.rs`), las bindings JS
(`javascript/iroha_js/src/sorafs.js`) y el SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantén esas ayudas en
sincronía con los valores por defecto de la CLI para que los operadores puedan
copiar políticas entre automatización sin capas de traducción a medida.

## 3. Ajuste de la política de fetch

`FetchOptions` controla el comportamiento de reintentos, concurrencia y verificación.
Al ajustar:

- **Reintentos:** Elevar `per_chunk_retry_limit` por encima de `4` aumenta el tiempo
  de recuperación pero puede ocultar fallos de proveedores. Prefiere mantener `4`
  como techo y confiar en la rotación de proveedores para detectar a los de bajo rendimiento.
- **Umbral de fallos:** `provider_failure_threshold` determina cuándo un proveedor
  se deshabilita para el resto de la sesión. Alinea este valor con la política de
  reintentos: un umbral menor que el presupuesto de reintentos obliga al orquestador
  a expulsar un peer antes de agotar todos los reintentos.
- **Concurrencia:** Deja `global_parallel_limit` sin configurar (`None`) a menos
  que un entorno específico no pueda saturar los rangos anunciados. Cuando se
  configure, asegúrate de que el valor sea ≤ a la suma de los presupuestos de
  streams de los proveedores para evitar inanición.
- **Toggles de verificación:** `verify_lengths` y `verify_digests` deben permanecer
  habilitados en producción. Garantizan el determinismo cuando hay flotas mixtas
  de proveedores; solo desactívalos en entornos de fuzzing aislados.

## 4. Etapas de transporte y anonimato

Usa los campos `rollout_phase`, `anonymity_policy` y `transport_policy` para
representar la postura de privacidad:

- Prefiere `rollout_phase="snnet-5"` y permite que la política de anonimato por
  defecto siga los hitos de SNNet-5. Sobrescribe con `anonymity_policy_override`
  solo cuando la gobernanza emita una directiva firmada.
- Mantén `transport_policy="soranet-first"` como base mientras SNNet-4/5/5a/5b/6a/7/8/12/13 estén 🈺
  (ver `roadmap.md`). Usa `transport_policy="direct-only"` solo para degradaciones
  documentadas o simulacros de cumplimiento, y espera la revisión de cobertura PQ
  antes de promover `transport_policy="soranet-strict"`—ese nivel fallará rápido si
  solo quedan relés clásicos.
- `write_mode="pq-only"` solo debe imponerse cuando cada ruta de escritura (SDK,
  orquestador, tooling de gobernanza) pueda satisfacer los requisitos PQ. Durante
  los rollouts mantén `write_mode="allow-downgrade"` para que las respuestas de
  emergencia puedan apoyarse en rutas directas mientras la telemetría marca la
  degradación.
- La selección de guardias y la preparación de circuitos dependen del directorio
  de SoraNet. Proporciona el snapshot firmado de `relay_directory` y persiste la
  cache de `guard_set` para que el churn de guardias se mantenga dentro de la
  ventana de retención acordada. La huella del cache registrada por
  `sorafs_cli fetch` forma parte de la evidencia de rollout.

## 5. Ganchos de degradación y cumplimiento

Dos subsistemas del orquestador ayudan a imponer la política sin intervención manual:

- **Remediación de degradaciones** (`downgrade_remediation`): monitorea eventos
  `handshake_downgrade_total` y, después de que el `threshold` configurado se
  excede dentro de `window_secs`, fuerza el proxy local al `target_mode` (por
  defecto metadata-only). Mantén los valores predeterminados (`threshold=3`,
  `window=300`, `cooldown=900`) salvo que los postmortems indiquen un patrón
  distinto. Documenta cualquier override en el log de rollout y asegúrate de que
  los dashboards sigan `sorafs_proxy_downgrade_state`.
- **Política de cumplimiento** (`compliance`): los carve-outs por jurisdicción y
  manifiesto fluyen a través de listas de exclusión administradas por gobernanza.
  Nunca insertes overrides ad hoc en el bundle de configuración; en su lugar,
  solicita una actualización firmada de
  `governance/compliance/soranet_opt_outs.json` y vuelve a desplegar el JSON generado.

Para ambos sistemas, persiste el bundle de configuración resultante e inclúyelo
en las evidencias de release para que los auditores puedan rastrear cómo se
activaron las reducciones.

## 6. Telemetría y dashboards

Antes de ampliar el despliegue, confirma que las siguientes señales estén activas
en el entorno objetivo:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  debe ser cero después de que complete el canary.
- `sorafs_orchestrator_retries_total` y
  `sorafs_orchestrator_retry_ratio` — deben estabilizarse por debajo del 10 %
  durante el canary y mantenerse por debajo del 5 % tras GA.
- `sorafs_orchestrator_policy_events_total` — valida que la etapa de rollout
  esperada está activa (label `stage`) y registra brownouts vía `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — rastrean el suministro de relés PQ
  frente a las expectativas de la política.
- Objetivos de log `telemetry::sorafs.fetch.*` — deben fluir al agregador de logs
  compartido con búsquedas guardadas para `status=failed`.

Carga el dashboard canónico de Grafana desde
`dashboards/grafana/sorafs_fetch_observability.json` (exportado en el portal
bajo **SoraFS → Fetch Observability**) para que los selectores de región/manifest,
el heatmap de reintentos por proveedor, los histogramas de latencia de chunks y
los contadores de atascos coincidan con lo que revisa SRE durante los burn-ins.
Conecta las reglas de Alertmanager en `dashboards/alerts/sorafs_fetch_rules.yml`
y valida la sintaxis de Prometheus con `scripts/telemetry/test_sorafs_fetch_alerts.sh`
(el helper ejecuta automáticamente `promtool test rules` localmente o en Docker).
Las transferencias de alertas requieren el mismo bloque de routing que imprime
el script para que los operadores puedan adjuntar la evidencia al ticket de rollout.

### Flujo de burn-in de telemetría

El ítem de roadmap **SF-6e** requiere un burn-in de telemetría de 30 días antes
de cambiar el orquestador multi-origen a sus valores GA. Usa los scripts del
repositorio para capturar un bundle de artefactos reproducible cada día en la
ventana:

1. Ejecuta `ci/check_sorafs_orchestrator_adoption.sh` con las variables de
   entorno de burn-in configuradas. Ejemplo:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   El helper reproduce `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   escribe `scoreboard.json`, `summary.json`, `provider_metrics.json`,
   `chunk_receipts.json` y `adoption_report.json` bajo
   `artifacts/sorafs_orchestrator/<timestamp>/`, e impone un número mínimo de
   proveedores elegibles mediante `cargo xtask sorafs-adoption-check`.
2. Cuando las variables de burn-in están presentes, el script también emite
   `burn_in_note.json`, capturando la etiqueta, el índice de día, el id del
   manifiesto, la fuente de telemetría y los digests de los artefactos. Adjunta
   este JSON al log de rollout para que sea evidente qué captura cubrió cada día
   de la ventana de 30 días.
3. Importa el tablero de Grafana actualizado (`dashboards/grafana/sorafs_fetch_observability.json`)
   en el workspace de staging/producción, etiquétalo con la etiqueta de burn-in
   y confirma que cada panel muestra muestras para el manifiesto/región en prueba.
4. Ejecuta `scripts/telemetry/test_sorafs_fetch_alerts.sh` (o `promtool test rules …`)
   cuando cambie `dashboards/alerts/sorafs_fetch_rules.yml` para documentar que
   el routing de alertas coincide con las métricas exportadas durante el burn-in.
5. Archiva el snapshot del dashboard, la salida de la prueba de alertas y el
   tail de logs de las búsquedas `telemetry::sorafs.fetch.*` junto a los
   artefactos del orquestador para que la gobernanza pueda reproducir la
   evidencia sin extraer métricas de sistemas en vivo.

## 7. Lista de verificación de rollout

1. Regenera los scoreboards en CI usando la configuración candidata y captura
   los artefactos bajo control de versiones.
2. Ejecuta el fetch determinista de fixtures en cada entorno (lab, staging,
   canary, producción) y adjunta los artefactos `--scoreboard-out` y `--json-out`
   al registro de rollout.
3. Revisa los dashboards de telemetría con el ingeniero on-call, asegurando que
   todas las métricas anteriores tengan muestras en vivo.
4. Registra la ruta de configuración final (normalmente vía `iroha_config`) y el
   commit git del registro de gobernanza usado para anuncios y cumplimiento.
5. Actualiza el tracker de rollout y notifica a los equipos de SDK sobre los
   nuevos valores por defecto para que las integraciones de clientes se mantengan
   alineadas.

Seguir esta guía mantiene los despliegues del orquestador deterministas y
auditables, mientras aporta ciclos de retroalimentación claros para ajustar
presupuestos de reintentos, capacidad de proveedores y postura de privacidad.
