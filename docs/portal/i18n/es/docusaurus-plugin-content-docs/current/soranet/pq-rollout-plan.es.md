---
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-despliegue-pq
título: Plan de despliegue poscuántico SNNet-16G
sidebar_label: Plan de implementación PQ
descripción: Guía operativa para promover el handshake hibrido X25519+ML-KEM de SoraNet desde canary hasta default en relés, clientes y SDK.
---

:::nota Fuente canónica
Esta página refleja `docs/source/soranet/pq_rollout_plan.md`. Manten ambas copias sincronizadas.
:::

SNNet-16G completa el despliegue poscuántico para el transporte de SoraNet. Los controles `rollout_phase` permiten a los operadores coordinar una promoción determinista desde el requisito actual de guardia de Stage A hasta la cobertura mayoritaria de Stage B y la postura PQ estricta de Stage C sin editar JSON/TOML en crudo para cada superficie.

Este libro de jugadas cubre:

- Definiciones de fases y los nuevos botones de configuración (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) cableados en el código base (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeo de flags de SDK y CLI para que cada cliente pueda seguir el rollout.
- Expectativas de programación de canary Relay/client mas los paneles de gobierno que gatean la promoción (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback y referencias al runbook de fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases| `rollout_phase` | Etapa de anonimato efectiva | Efecto por defecto | Uso típico |
|-----------------|---------------------|----------------|-----------------------|
| `canary` | `anon-guard-pq` (Etapa A) | Requiere al menos un guardia PQ por circuito mientras la flota se calienta. | Baseline y primeras semanas de canary. |
| `ramp` | `anon-majority-pq` (Etapa B) | Sesga la selección hacia relés PQ para >= dos tercios de cobertura; Los relevos clásicos permanecen como respaldo. | Canarias por región de relevos; alterna la vista previa en SDK. |
| `default` | `anon-strict-pq` (Etapa C) | Aplica circuitos solo PQ y soporta las alarmas de downgrade. | Promoción final una vez completada la telemetria y el sign-off de gobernanza. |

Si una superficie también define un `anonymity_policy` explícito, esto anulará la fase para ese componente. Omitir la etapa explícita ahora difiere al valor de `rollout_phase` para que los operadores puedan cambiar la fase una sola vez por entorno y dejar que los clientes la hereden.

## Referencia de configuración

### Orquestador (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

El cargador del orquestador resuelve la etapa de respaldo en tiempo de ejecución (`crates/sorafs_orchestrator/src/lib.rs:2229`) y el exponente vía `sorafs_orchestrator_policy_events_total` y `sorafs_orchestrator_pq_ratio_*`. Ver `docs/examples/sorafs_rollout_stage_b.toml` e `docs/examples/sorafs_rollout_stage_c.toml` para fragmentos listos para aplicar.

### Cliente Rust / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` ahora registra la fase parseada (`crates/iroha/src/client.rs:2315`) para que helpers (por ejemplo `iroha_cli app sorafs fetch`) puedan reportar la fase actual junto con la política de anonimato por defecto.

## Automatización

Dos ayudantes `cargo xtask` automatizan la generación del horario y la captura de artefactos.

1. **Generar el horario regional**

   ```bash
   cargo xtask soranet-rollout-plan \
     --regions us-east,eu-west,apac \
     --start 2026-04-01T00:00:00Z \
     --window 6h \
     --spacing 24h \
     --client-offset 8h \
     --phase ramp \
     --environment production
   ```

   Las duraciones aceptan sufijos `s`, `m`, `h` o `d`. El comando emite `artifacts/soranet_pq_rollout_plan.json` y un resumen en Markdown (`artifacts/soranet_pq_rollout_plan.md`) que puede enviarse con la solicitud de cambio.

2. **Capturar artefactos del taladro con firmas**

   ```bash
   cargo xtask soranet-rollout-capture \
     --log logs/pq_fire_drill.log \
     --artifact kind=scoreboard,path=artifacts/canary.scoreboard.json \
     --artifact kind=fetch-summary,path=artifacts/canary.fetch.json \
     --key secrets/pq_rollout_signing_ed25519.hex \
     --phase ramp \
     --label "beta-canary" \
     --note "Relay canary - APAC first"
   ```

   El comando copia los archivos suministrados en `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula resúmenes BLAKE3 para cada artefacto y escribe `rollout_capture.json` con metadatos más una firma Ed25519 sobre la carga útil. Usa la misma clave privada que firma las minutas del fire-drill para que el gobierno valide la captura rápidamente.

## Matriz de banderas de SDK y CLI| Superficie | Canarias (Etapa A) | Rampa (Etapa B) | Incumplimiento (Etapa C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` buscar | `--anonymity-policy stage-a` o confiar en la fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Configuración del orquestador JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Configuración del cliente Rust (`iroha.toml`) | `rollout_phase = "canary"` (predeterminado) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| Comandos firmados `iroha_cli` | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, opcional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, opcional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, opcional `.ANON_STRICT_PQ` |
| Ayudantes del orquestador de JavaScript | `rolloutPhase: "canary"` o `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Pitón `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Rápido `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos los toggles de SDK mapean al mismo analizador de etapas usado por el orquestador (`crates/sorafs_orchestrator/src/lib.rs:365`), por lo que implementaciones multilenguaje se mantienen en lock-step con la fase configurada.

## Lista de verificación de programación de canarias

1. **Verificación previa (T menos 2 semanas)**- Confirmar que la tasa de brownout de Stage A sea =70% por región (`sorafs_orchestrator_pq_candidate_ratio`).
   - Programar el slot de Governance Review que aprueba la ventana de Canarias.
   - Actualizar `sorafs.gateway.rollout_phase = "ramp"` en staging (editar el JSON del orquestador y redeployar) y simulacro del pipeline de promoción.

2. **Relevo canario (día T)**

   - Promover una región por vez configurando `rollout_phase = "ramp"` en el orquestador y en los manifiestos de retransmisión de participantes.
   - Monitorear "Eventos de política por resultado" y "Tasa de apagones" en el tablero PQ Ratchet (que ahora incluye el panel de implementación) durante el doble del TTL de guardia caché.
   - Cortar instantáneas de `sorafs_cli guard-directory fetch` antes y después de la ejecución para almacenamiento de auditoria.

3. **Cliente/SDK canario (T más 1 semana)**

   - Cambie a `rollout_phase = "ramp"` en las configuraciones de cliente o pase overrides `stage-b` para las cohortes de SDK designadas.
   - Capturar diferencias de telemetría (`sorafs_orchestrator_policy_events_total` agrupado por `client_id` y `region`) y adjuntarlos al registro de incidentes del rollout.

4. **Promoción predeterminada (T más 3 semanas)**

   - Una vez que el gobierno firme, cambiar tanto orquestador como configuraciones de cliente a `rollout_phase = "default"` y rotar el checklist de readiness firmado hacia los artefactos de liberación.

## Lista de verificación de gobernanza y evidencia| Cambio de fase | Puerta de promoción | Paquete de evidencia | Paneles y alertas |
|----------------------|----------------|-----------------|---------------------|
| Canarias -> Rampa *(vista previa de la etapa B)* | Tasa de brownout Stage A = 0.7 por región promovida, Argon2 ticket verificar p95  Predeterminado *(aplicación de la Etapa C)* | Burn-in de telemetria SN16 de 30 días cumplido, `sn16_handshake_downgrade_total` plano en baseline, `sorafs_orchestrator_brownouts_total` en cero durante el canary de client, y el rehearsal del proxy toggle registrado. | Transcripción de `sorafs_cli proxy set-mode --mode gateway|direct`, salida de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log de `sorafs_cli guard-directory verify`, y paquete firmado `cargo xtask soranet-rollout-capture --label default`. | Mismo tablero PQ Ratchet mas los paneles de downgrade SN16 documentados en `docs/source/sorafs_orchestrator_rollout.md` y `dashboards/grafana/soranet_privacy_metrics.json`. || Degradación de emergencia/preparación para reversión | Se activa cuando los contadores de downgrade suben, falla la verificación del directorio de guardia o el buffer `/policy/proxy-toggle` registra eventos de downgrade sostenidos. | Checklist de `docs/source/ops/soranet_transport_rollback.md`, logs de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes y plantillas de notificación. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` y ambos paquetes de alertas (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Guarda cada artefacto bajo `artifacts/soranet_pq_rollout/<timestamp>_<label>/` con el `rollout_capture.json` generado para que los paquetes de gobierno contengan el marcador, trazas de promtool y digests.
- Adjunta digest SHA256 de evidencia subida (minutas PDF, paquete de captura, instantáneas de guardia) a las minutas de promoción para que las aprobaciones del Parlamento puedan reproducirse sin acceso al cluster de staging.
- Referencia el plan de telemetria en el ticket de promoción para probar que `docs/source/soranet/snnet16_telemetry_plan.md` sigue siendo la fuente canónica de vocabulario de downgrade y umbrales de alerta.

## Actualizaciones de paneles y telemetria

`dashboards/grafana/soranet_pq_ratchet.json` ahora incluye un panel de anotaciones "Rollout Plan" que enlaza a este playbook y muestra la fase actual para que las revisiones de gobernanza confirmen que etapa esta activa. Mantenga la descripción del panel sincronizada con cambios futuros en las perillas de configuración.Para alertar, asegúrese de que las reglas existentes utilicen la etiqueta `stage` para que las fases canary y default disparen umbrales de política separada (`dashboards/alerts/soranet_handshake_rules.yml`).

## Ganchos de reversión

### Predeterminado -> Rampa (Etapa C -> Etapa B)

1. Baja el orquestador con `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (y refleja la misma fase en configuraciones de SDK) para que Stage B vuelva a toda la flota.
2. Fuerza a los clientes al perfil de transporte seguro vía `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando la transcripción para que el flujo de trabajo de remediacion `/policy/proxy-toggle` siga siendo auditable.
3. Ejecuta `cargo xtask soranet-rollout-capture --label rollback-default` para archivar diferencias de directorio de guardia, salida de promtool y capturas de pantalla de paneles bajo `artifacts/soranet_pq_rollout/`.

### Rampa -> Canarias (Etapa B -> Etapa A)

1. Importa el snapshot del guard directorio capturado antes de la promoción con `sorafs_cli guard-directory import --guard-directory guards.json` y vuelve a ejecutar `sorafs_cli guard-directory verify` para que el paquete de demostración incluya hashes.
2. Ajusta `rollout_phase = "canary"` (o override con `anonymity_policy stage-a`) en configuraciones de orquestador y cliente, y luego repite el taladro de trinquete PQ desde el [PQ ratchet runbook](./pq-ratchet-runbook.md) para probar el pipeline de downgrade.
3. Adjunta capturas de pantalla actualizadas de PQ Ratchet y telemetria SN16 mas los resultados de alertas al registro de incidentes antes de notificar a gobierno.

### Recordatorios de guardrail- Referencia `docs/source/ops/soranet_transport_rollback.md` cada vez que ocurre una degradación y registra cualquier mitigación temporal como un artículo `TODO:` en el rollout tracker para trabajo posterior.
- Mantener `dashboards/alerts/soranet_handshake_rules.yml` y `dashboards/alerts/soranet_privacy_rules.yml` bajo cobertura de `promtool test rules` antes y después de un rollback para que la deriva de alertas quede documentado junto al capture bundle.