<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: pq-rollout-plan
lang: es
direction: ltr
source: docs/portal/docs/soranet/pq-rollout-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Esta pagina refleja `docs/source/soranet/pq_rollout_plan.md`. Manten ambas copias sincronizadas.
:::

SNNet-16G completa el despliegue poscuantico para el transporte de SoraNet. Los controles `rollout_phase` permiten a los operadores coordinar una promocion determinista desde el requisito actual de guard de Stage A hasta la cobertura mayoritaria de Stage B y la postura PQ estricta de Stage C sin editar JSON/TOML en crudo para cada superficie.

Este playbook cubre:

- Definiciones de fases y los nuevos knobs de configuracion (`sorafs.gateway.rollout_phase`, `sorafs.rollout_phase`) cableados en el codebase (`crates/iroha_config/src/parameters/actual.rs:2230`, `crates/iroha/src/config/user.rs:251`).
- Mapeo de flags de SDK y CLI para que cada client pueda seguir el rollout.
- Expectativas de scheduling de canary relay/client mas los dashboards de governance que gatean la promocion (`dashboards/grafana/soranet_pq_ratchet.json`).
- Hooks de rollback y referencias al runbook de fire-drill ([PQ ratchet runbook](./pq-ratchet-runbook.md)).

## Mapa de fases

| `rollout_phase` | Etapa de anonimato efectiva | Efecto por defecto | Uso tipico |
|-----------------|---------------------------|----------------|---------------|
| `canary`        | `anon-guard-pq` (Stage A) | Requiere al menos un guard PQ por circuito mientras la flota se calienta. | Baseline y primeras semanas de canary. |
| `ramp`          | `anon-majority-pq` (Stage B) | Sesga la seleccion hacia relays PQ para >= dos tercios de cobertura; relays clasicos permanecen como fallback. | Canaries por region de relays; toggles de preview en SDK. |
| `default`       | `anon-strict-pq` (Stage C) | Aplica circuits solo PQ y endurece las alarmas de downgrade. | Promocion final una vez completada la telemetria y el sign-off de governance. |

Si una superficie tambien define un `anonymity_policy` explicito, este overridea la fase para ese componente. Omitir la etapa explicita ahora difiere al valor de `rollout_phase` para que los operadores puedan cambiar la fase una sola vez por entorno y dejar que los clients la hereden.

## Referencia de configuracion

### Orchestrator (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

El loader del orchestrator resuelve la etapa de fallback en runtime (`crates/sorafs_orchestrator/src/lib.rs:2229`) y la expone via `sorafs_orchestrator_policy_events_total` y `sorafs_orchestrator_pq_ratio_*`. Ver `docs/examples/sorafs_rollout_stage_b.toml` y `docs/examples/sorafs_rollout_stage_c.toml` para snippets listos para aplicar.

### Rust client / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```

`iroha::Client` ahora registra la fase parseada (`crates/iroha/src/client.rs:2315`) para que helpers (por ejemplo `iroha_cli sorafs fetch`) puedan reportar la fase actual junto con la politica de anonimato por defecto.

## Automatizacion

Dos helpers `cargo xtask` automatizan la generacion del schedule y la captura de artefactos.

1. **Generar el schedule regional**

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

2. **Capturar artefactos del drill con firmas**

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

   El comando copia los archivos suministrados en `artifacts/soranet_pq_rollout/<timestamp>_<label>/`, calcula digests BLAKE3 para cada artefacto y escribe `rollout_capture.json` con metadata mas una firma Ed25519 sobre el payload. Usa la misma private key que firma las minutas del fire-drill para que governance valide la captura rapidamente.

## Matriz de flags de SDK y CLI

| Superficie | Canary (Stage A) | Ramp (Stage B) | Default (Stage C) |
|---------|------------------|----------------|-------------------|
| `sorafs_cli` fetch | `--anonymity-policy stage-a` o confiar en la fase | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Orchestrator config JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| Rust client config (`iroha.toml`) | `rollout_phase = "canary"` (default) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` signed commands | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| Java/Android `GatewayFetchOptions` | `setRolloutPhase("canary")`, optional `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`, optional `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`, optional `.ANON_STRICT_PQ` |
| JavaScript orchestrator helpers | `rolloutPhase: "canary"` o `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| Python `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| Swift `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

Todos los toggles de SDK mapean al mismo parser de etapas usado por el orchestrator (`crates/sorafs_orchestrator/src/lib.rs:365`), por lo que despliegues multi-lenguaje se mantienen en lock-step con la fase configurada.

## Checklist de scheduling de canary

1. **Preflight (T minus 2 weeks)**

- Confirmar que la tasa de brownout de Stage A sea <1% en las dos semanas previas y que la cobertura PQ sea >=70% por region (`sorafs_orchestrator_pq_candidate_ratio`).
   - Programar el slot de governance review que aprueba la ventana de canary.
   - Actualizar `sorafs.gateway.rollout_phase = "ramp"` en staging (editar el JSON del orchestrator y redeployar) y dry-run del pipeline de promocion.

2. **Relay canary (T day)**

   - Promover una region por vez configurando `rollout_phase = "ramp"` en el orchestrator y en los manifests de relay participantes.
   - Monitorear "Policy Events per Outcome" y "Brownout Rate" en el dashboard PQ Ratchet (que ahora incluye el panel de rollout) durante el doble del TTL de guard cache.
   - Cortar snapshots de `sorafs_cli guard-directory fetch` antes y despues de la ejecucion para almacenamiento de auditoria.

3. **Client/SDK canary (T plus 1 week)**

   - Cambiar a `rollout_phase = "ramp"` en configs de client o pasar overrides `stage-b` para los cohorts de SDK designados.
   - Capturar diffs de telemetria (`sorafs_orchestrator_policy_events_total` agrupado por `client_id` y `region`) y adjuntarlos al log de incidentes del rollout.

4. **Default promotion (T plus 3 weeks)**

   - Una vez que governance firme, cambiar tanto orchestrator como configs de client a `rollout_phase = "default"` y rotar el checklist de readiness firmado hacia los artefactos de release.

## Checklist de governance y evidencia

| Cambio de fase | Gate de promocion | Bundle de evidencia | Dashboards y alertas |
|--------------|----------------|-----------------|---------------------|
| Canary -> Ramp *(Stage B preview)* | Tasa de brownout Stage A <1% en los ultimos 14 dias, `sorafs_orchestrator_pq_candidate_ratio` >= 0.7 por region promovida, Argon2 ticket verify p95 < 50 ms, y el slot de governance para la promocion reservado. | Par JSON/Markdown de `cargo xtask soranet-rollout-plan`, snapshots emparejados de `sorafs_cli guard-directory fetch` (antes/despues), bundle firmado `cargo xtask soranet-rollout-capture --label canary`, y minutas de canary referenciando [PQ ratchet runbook](./pq-ratchet-runbook.md). | `dashboards/grafana/soranet_pq_ratchet.json` (Policy Events + Brownout Rate), `dashboards/grafana/soranet_privacy_metrics.json` (SN16 downgrade ratio), referencias de telemetria en `docs/source/soranet/snnet16_telemetry_plan.md`. |
| Ramp -> Default *(Stage C enforcement)* | Burn-in de telemetria SN16 de 30 dias cumplido, `sn16_handshake_downgrade_total` plano en baseline, `sorafs_orchestrator_brownouts_total` en cero durante el canary de client, y el rehearsal del proxy toggle registrado. | Transcripcion de `sorafs_cli proxy set-mode --mode gateway|direct`, salida de `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`, log de `sorafs_cli guard-directory verify`, y bundle firmado `cargo xtask soranet-rollout-capture --label default`. | Mismo tablero PQ Ratchet mas los paneles de downgrade SN16 documentados en `docs/source/sorafs_orchestrator_rollout.md` y `dashboards/grafana/soranet_privacy_metrics.json`. |
| Emergency demotion / rollback readiness | Se activa cuando los contadores de downgrade suben, falla la verificacion del guard directory o el buffer `/policy/proxy-toggle` registra eventos de downgrade sostenidos. | Checklist de `docs/source/ops/soranet_transport_rollback.md`, logs de `sorafs_cli guard-directory import` / `guard-cache prune`, `cargo xtask soranet-rollout-capture --label rollback`, tickets de incidentes y templates de notificacion. | `dashboards/grafana/soranet_pq_ratchet.json`, `dashboards/grafana/soranet_privacy_metrics.json` y ambos packs de alertas (`dashboards/alerts/soranet_handshake_rules.yml`, `dashboards/alerts/soranet_privacy_rules.yml`). |

- Guarda cada artefacto bajo `artifacts/soranet_pq_rollout/<timestamp>_<label>/` con el `rollout_capture.json` generado para que los paquetes de governance contengan el scoreboard, trazas de promtool y digests.
- Adjunta digests SHA256 de evidencia subida (minutes PDF, capture bundle, guard snapshots) a las minutas de promocion para que las aprobaciones de Parliament puedan reproducirse sin acceso al cluster de staging.
- Referencia el plan de telemetria en el ticket de promocion para probar que `docs/source/soranet/snnet16_telemetry_plan.md` sigue siendo la fuente canonica de vocabulario de downgrade y umbrales de alerta.

## Actualizaciones de dashboards y telemetria

`dashboards/grafana/soranet_pq_ratchet.json` ahora incluye un panel de anotaciones "Rollout Plan" que enlaza a este playbook y muestra la fase actual para que las revisiones de governance confirmen que etapa esta activa. Manten la descripcion del panel sincronizada con cambios futuros en los knobs de configuracion.

Para alerting, asegurate de que las reglas existentes usen la etiqueta `stage` para que las fases canary y default disparen thresholds de politica separados (`dashboards/alerts/soranet_handshake_rules.yml`).

## Hooks de rollback

### Default -> Ramp (Stage C -> Stage B)

1. Baja el orchestrator con `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (y refleja la misma fase en configs de SDK) para que Stage B vuelva a toda la flota.
2. Fuerza a los clients al perfil de transporte seguro via `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`, capturando la transcripcion para que el workflow de remediacion `/policy/proxy-toggle` siga siendo auditable.
3. Ejecuta `cargo xtask soranet-rollout-capture --label rollback-default` para archivar diffs de guard directory, salida de promtool y screenshots de dashboards bajo `artifacts/soranet_pq_rollout/`.

### Ramp -> Canary (Stage B -> Stage A)

1. Importa el snapshot del guard directory capturado antes de la promocion con `sorafs_cli guard-directory import --guard-directory guards.json` y vuelve a ejecutar `sorafs_cli guard-directory verify` para que el paquete de democion incluya hashes.
2. Ajusta `rollout_phase = "canary"` (o override con `anonymity_policy stage-a`) en configs de orchestrator y client, y luego repite el PQ ratchet drill desde el [PQ ratchet runbook](./pq-ratchet-runbook.md) para probar el pipeline de downgrade.
3. Adjunta screenshots actualizados de PQ Ratchet y telemetria SN16 mas los resultados de alertas al log de incidentes antes de notificar a governance.

### Recordatorios de guardrail

- Referencia `docs/source/ops/soranet_transport_rollback.md` cada vez que ocurra una democion y registra cualquier mitigacion temporal como un item `TODO:` en el rollout tracker para trabajo posterior.
- Mantener `dashboards/alerts/soranet_handshake_rules.yml` y `dashboards/alerts/soranet_privacy_rules.yml` bajo cobertura de `promtool test rules` antes y despues de un rollback para que el drift de alertas quede documentado junto al capture bundle.
