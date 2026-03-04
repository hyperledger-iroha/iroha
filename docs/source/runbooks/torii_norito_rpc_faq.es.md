---
lang: es
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969f24fd53af5e5714eb4b257dc086b3990550a5085c9464805f689c7d8ee324
source_last_modified: "2025-12-14T09:53:36.247266+00:00"
translation_last_reviewed: 2025-12-28
---

# FAQ operativa de Norito-RPC

Esta FAQ destila los knobs de despliegue/rollback, telemetría y artefactos de
evidencia referenciados en los ítems del roadmap **NRPC-2** y **NRPC-4** para que
los operadores tengan una única página de consulta durante canarios, brownouts
o drills de incidentes. Trátelo como la puerta de entrada para handoffs on-call;
los procedimientos detallados siguen en
`docs/source/torii/norito_rpc_rollout_plan.md` y
`docs/source/runbooks/torii_norito_rpc_canary.md`.

## 1. Knobs de configuración

| Ruta | Propósito | Valores permitidos / notas |
|------|---------|------------------------|
| `torii.transport.norito_rpc.enabled` | Interruptor on/off duro para el transporte Norito. | `true` mantiene los handlers HTTP registrados; `false` los deshabilita sin importar el stage. |
| `torii.transport.norito_rpc.require_mtls` | Exigir TLS mutuo para endpoints Norito. | Por defecto `true`. Apáguelo solo en pools de staging aislados. |
| `torii.transport.norito_rpc.allowed_clients` | Lista blanca de cuentas de servicio / tokens API que pueden usar Norito. | Proporcione bloques CIDR, hashes de token u OIDC client IDs según su despliegue. |
| `torii.transport.norito_rpc.stage` | Stage de rollout anunciado a los SDKs. | `disabled` (rechaza Norito, fuerza JSON), `canary` (solo allowlist, telemetría reforzada), `ga` (por defecto para cada cliente autenticado). |
| `torii.preauth_scheme_limits.norito_rpc` | Concurrencia por esquema + presupuesto de burst. | Refleje las claves usadas en los throttles HTTP/WS (p. ej., `max_in_flight`, `rate_per_sec`). Subir el cap sin actualizar Alertmanager invalida el guardarraíl. |
| `transport.norito_rpc.*` en `docs/source/config/client_api.md` | Overrides de cara al cliente (CLI / SDK discovery). | Use `cargo xtask client-api-config diff` para revisar cambios pendientes antes de enviarlos a Torii. |

**Flujo recomendado de brownout**

1. Configure `torii.transport.norito_rpc.stage=disabled`.
2. Mantenga `enabled=true` para que probes/tests de alerta sigan ejercitando los handlers.
3. Cambie `torii.preauth_scheme_limits.norito_rpc.max_in_flight` a cero si
   necesita detener de inmediato (por ejemplo, mientras espera la propagación de config).
4. Actualice el log operativo y adjunte el digest de la nueva config al reporte de stage.

## 2. Checklists operativos

- **Ejecuciones canary / staging** — siga `docs/source/runbooks/torii_norito_rpc_canary.md`.
  Ese runbook referencia las mismas claves de config y lista los artefactos de
  evidencia capturados por `scripts/run_norito_rpc_smoke.sh` +
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh`.
- **Promoción a producción** — ejecute la plantilla de stage report en
  `docs/source/torii/norito_rpc_stage_reports.md`. Registre el hash de config,
  el hash de allowlist, el digest del bundle de smoke, el hash del export de
  Grafana y el identificador del drill de alertas.
- **Rollback** — cambie `stage` de vuelta a `disabled`, mantenga la allowlist
  y documente el cambio en el stage report + log del incidente. Cuando se
  arregle la causa raíz, reejecute el checklist canary antes de fijar `stage=ga`.

## 3. Telemetría y alertas

| Activo | Ubicación | Notas |
|-------|----------|-------|
| Dashboard | `dashboards/grafana/torii_norito_rpc_observability.json` | Rastrea tasa de solicitudes, códigos de error, tamaños de payload, fallos de decodificación y % de adopción. |
| Alertas | `dashboards/alerts/torii_norito_rpc_rules.yml` | Puertas `NoritoRpcErrorBudget`, `NoritoRpcDecodeFailures`, y `NoritoRpcFallbackSpike`. |
| Script de caos | `scripts/telemetry/test_torii_norito_rpc_alerts.sh` | Falla CI cuando derivan las expresiones de alerta. Ejecútelo tras cada cambio de config. |
| Smoke tests | `python/iroha_python/scripts/run_norito_rpc_smoke.sh`, `cargo xtask norito-rpc-verify` | Incluya sus logs en el bundle de evidencia para cada promoción. |

Los dashboards deben exportarse y adjuntarse al ticket de release (`make
docs-portal-dashboards` en CI) para que los on-call puedan reproducir las
métricas sin acceso a Grafana de producción.

## 4. Preguntas frecuentes

**¿Cómo permito un nuevo SDK durante canary?**  
Agregue la cuenta de servicio/token a `torii.transport.norito_rpc.allowed_clients`,
recargue Torii y registre el cambio en `docs/source/torii/norito_rpc_tracker.md`
bajo NRPC-2R. El dueño del SDK también debe capturar una ejecución de fixtures
vía `scripts/run_norito_rpc_fixtures.sh --sdk <label>`.

**¿Qué ocurre si la decodificación Norito falla a mitad de rollout?**  
Deje `stage=canary`, mantenga `enabled=true` y triagee las fallas vía
`torii_norito_decode_failures_total`. Los dueños de SDK pueden volver a JSON
omitiendo `Accept: application/x-norito`; Torii seguirá sirviendo JSON hasta que
el stage vuelva a `ga`.

**¿Cómo pruebo que el gateway sirve el manifiesto correcto?**  
Ejecute `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario norito-rpc --host
<gateway-host>` para que la sonda registre los encabezados `Sora-Proof` junto con
el digest de config Norito. Adjunte la salida JSON al stage report.

**¿Dónde debo capturar overrides de redacción?**  
Documente cada override temporal en la columna `Notes` del stage report y registre
el parche de config Norito bajo control de cambios. Los overrides expiran
automáticamente en el archivo de config; esta entrada garantiza que el on-call
recuerde limpiar después de incidentes.

Para cualquier pregunta no cubierta aquí, escale por los canales listados en el
runbook canary (`docs/source/runbooks/torii_norito_rpc_canary.md`).

## 5. Fragmento de nota de release (seguimiento OPS-NRPC)

El ítem **OPS-NRPC** del roadmap requiere una nota de release lista para usar
para que los operadores anuncien el rollout Norito-RPC de forma consistente.
Copie el bloque de abajo en el próximo post de release (reemplace los campos
entre corchetes) y adjunte el bundle de evidencia descrito debajo.

> **Transporte Torii Norito-RPC** — Los sobres Norito ahora se sirven junto con la
> API JSON. El flag `torii.transport.norito_rpc.stage` se entrega configurado en
> **[stage: disabled/canary/ga]** y sigue el checklist de rollout por etapas en
> `docs/source/torii/norito_rpc_rollout_plan.md`. Los operadores pueden optar por
> salir temporalmente estableciendo `torii.transport.norito_rpc.stage=disabled`
> mientras mantienen `torii.transport.norito_rpc.enabled=true`; los SDKs volverán
> automáticamente a JSON. Los dashboards de telemetría
> (`dashboards/grafana/torii_norito_rpc_observability.json`) y los drills de
> alerta (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) siguen siendo
> obligatorios antes de elevar el stage, y los artefactos canary/smoke capturados
> por `python/iroha_python/scripts/run_norito_rpc_smoke.sh` deben adjuntarse al
> ticket de release.

Antes de publicar:

1. Reemplace el marcador **[stage: …]** con el stage anunciado en Torii.
2. Enlace el ticket de release al stage report más reciente en
   `docs/source/torii/norito_rpc_stage_reports.md`.
3. Suba los exports de Grafana/Alertmanager mencionados arriba junto con los
   hashes del bundle de smoke de `scripts/run_norito_rpc_smoke.sh`.

Este fragmento mantiene satisfecho el requisito de nota de release OPS-NRPC sin
forzar a los comandantes de incidentes a reformular el estado del rollout cada vez.
