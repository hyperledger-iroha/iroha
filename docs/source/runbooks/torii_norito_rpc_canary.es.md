---
lang: es
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

# Runbook de canary Torii Norito-RPC (NRPC-2C)

Este runbook operacionaliza el plan de rollout **NRPC-2** al describir cómo
promover el transporte Norito‑RPC desde validación en laboratorio de staging
hacia el stage de “canary” en producción. Debe leerse junto con:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (contrato del protocolo)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## Roles e insumos

| Rol | Responsabilidad |
|------|----------------|
| Torii Platform TL | Aprueba deltas de configuración, firma los smoke tests. |
| NetOps | Aplica cambios de ingress/envoy y monitorea la salud del pool canary. |
| Enlace de observabilidad | Verifica dashboards/alertas y captura evidencia. |
| Platform Ops | Lleva el ticket de cambio, coordina el rehearsal de rollback, actualiza trackers. |

Artefactos requeridos:

- Último parche Norito de `iroha_config` con `transport.norito_rpc.stage = "canary"` y
  `transport.norito_rpc.allowed_clients` poblado.
- Snippet de configuración Envoy/Nginx que preserve `Content-Type: application/x-norito` y
  haga cumplir el perfil mTLS del cliente canary (`defaults/torii_ingress_mtls.yaml`).
- Allowlist de tokens (YAML o manifiesto Norito) para los clientes canary.
- URL de Grafana + token API para `dashboards/grafana/torii_norito_rpc_observability.json`.
- Acceso al harness de smoke de paridad
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) y al script de drill de alertas
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## Checklist pre‑flight

1. **Confirmar freeze de especificación.** Asegure que el hash de
   `docs/source/torii/nrpc_spec.md` coincide con el último release firmado y que
   no haya PRs pendientes que toquen el header/layout Norito.
2. **Validación de configuración.** Ejecute
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   para confirmar que las nuevas entradas `transport.norito_rpc.*` se parsean.
3. **Caps de esquema.** Configure `torii.preauth_scheme_limits.norito_rpc` de forma
   conservadora (p. ej., 25 conexiones concurrentes) para que los llamados binarios
   no ahoguen el tráfico JSON.
4. **Rehearsal de ingress.** Aplique el patch de Envoy en staging, reejecute el
   test negativo (`cargo test -p iroha_torii -- norito_ingress`) y confirme que
   los headers removidos son rechazados con HTTP 415.
5. **Sanidad de telemetría.** En staging, ejecute `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` y adjunte el bundle de evidencia generado.
6. **Inventario de tokens.** Verifique que la allowlist canary incluye al menos
   dos operadores por región; guarde el manifiesto en
   `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **Ticketing.** Abra el ticket de cambio con ventana de inicio/fin, plan de rollback
   y enlaces a este runbook más la evidencia de telemetría.

## Procedimiento de promoción a canary

1. **Aplicar el patch de configuración.**
   - Despliegue el delta de `iroha_config` (stage=`canary`, allowlist poblada,
     límites de esquema configurados) a través de admisión.
   - Reinicie o recargue Torii en caliente, confirmando que el patch se reconoce
     vía logs `torii.config.reload`.
2. **Actualizar ingress.**
   - Despliegue la configuración Envoy/Nginx que habilita el ruteo de headers Norito
     y el perfil mTLS para el pool canary.
   - Verifique que respuestas `curl -vk --cert <client.pem>` incluyen los headers
     Norito `X-Iroha-Error-Code` cuando corresponde.
3. **Smoke tests.**
   - Ejecute `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     desde el bastión canary. Capture transcripciones JSON + Norito y guárdelas en
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - Registre hashes en `docs/source/torii/norito_rpc_stage_reports.md`.
4. **Observar telemetría.**
   - Observe `torii_active_connections_total{scheme="norito_rpc"}` y
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` por al menos 30 minutos.
   - Exporte el dashboard de Grafana vía API y adjúntelo al ticket de cambio.
5. **Rehearsal de alertas.**
   - Ejecute `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` para
     inyectar sobres Norito malformados; asegure que Alertmanager registre el incidente
     sintético y se limpie automáticamente.
6. **Captura de evidencia.**
   - Actualice `docs/source/torii/norito_rpc_stage_reports.md` con:
     - Digest de configuración
     - Hash del manifiesto de allowlist
     - Timestamp de smoke test
     - Checksum del export de Grafana
     - ID del drill de alertas
   - Suba artefactos a `artifacts/norito_rpc/<YYYYMMDD>/`.

## Monitoreo y criterios de salida

Permanezca en canary hasta que todas las condiciones siguientes sean ciertas por ≥72 horas:

- Tasa de error (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % y sin picos
  sostenidos en `torii_norito_decode_failures_total`.
- Paridad de latencia (`p95` Norito vs JSON) dentro del 10 %.
- Dashboard de alertas silencioso salvo drills programados.
- Operadores en la allowlist envían reportes de paridad sin desajustes de esquema.

Documente el estado diario en el ticket de cambio y capture snapshots en
`docs/source/status/norito_rpc_canary_log.md` (si existe).

## Procedimiento de rollback

1. Cambie `transport.norito_rpc.stage` de vuelta a `"disabled"` y limpie
   `allowed_clients`; aplique vía admisión.
2. Elimine el route/mTLS stanza de Envoy/Nginx, recargue proxies y confirme que
   nuevas conexiones Norito son rechazadas.
3. Revoque tokens canary (o desactive credenciales bearer) para que caigan las
   sesiones pendientes.
4. Observe `torii_active_connections_total{scheme="norito_rpc"}` hasta que llegue a cero.
5. Reejecute el harness de smoke solo‑JSON para asegurar la funcionalidad base.
6. Cree un stub de post‑mortem en `docs/source/postmortems/norito_rpc_rollback.md`
   dentro de 24 horas y actualice el ticket de cambio con resumen de impacto + métricas.

## Cierre post‑canary

Cuando los criterios de salida se cumplan:

1. Actualice `docs/source/torii/norito_rpc_stage_reports.md` con la recomendación GA.
2. Agregue una entrada en `status.md` que resuma resultados del canary y bundles de evidencia.
3. Notifique a los leads de SDK para que cambien fixtures de staging a Norito para paridad.
4. Prepare el patch de config GA (stage=`ga`, remover allowlist) y programe la
   promoción según el plan NRPC-2.

Seguir este runbook asegura que cada promoción canary recolecte la misma evidencia,
mantenga el rollback determinista y cumpla los criterios de aceptación del roadmap NRPC-2.
