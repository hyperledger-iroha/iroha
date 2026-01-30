---
lang: ja
direction: ltr
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/soranet/testnet-rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8eaeac10e4f2fd4527931682bb2f26826543bfd51212c51fa87d9bcfb9a15a2f
source_last_modified: "2026-01-03T18:07:57+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
id: testnet-rollout
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Esta pagina refleja el plan de rollout SNNet-10 en `docs/source/soranet/testnet_rollout_plan.md`. Mantengan ambas copias alineadas hasta que los docs heredados se retiren.
:::

SNNet-10 coordina la activacion escalonada del overlay de anonimato de SoraNet en toda la red. Usa este plan para traducir el bullet del roadmap en entregables concretos, runbooks y gates de telemetria para que cada operador entienda las expectativas antes de que SoraNet sea el transporte por defecto.

## Fases de lanzamiento

| Fase | Cronograma (objetivo) | Alcance | Artefactos requeridos |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet cerrado** | Q4 2026 | 20-50 relays en >=3 ASNs operados por contribuyentes core. | Kit de onboarding de testnet, smoke suite de guard pinning, baseline de latencia + metricas de PoW, log de brownout drill. |
| **T1 - Beta publica** | Q1 2027 | >=100 relays, guard rotation habilitada, exit bonding enforced, SDK betas por defecto en SoraNet con `anon-guard-pq`. | Kit de onboarding actualizado, checklist de verificacion de operadores, SOP de publicacion de directory, paquete de dashboards de telemetria, reportes de rehearsal de incidentes. |
| **T2 - Mainnet por defecto** | Q2 2027 (condicionado a completar SNNet-6/7/9) | La red de produccion por defecto en SoraNet; transportes obfs/MASQUE y enforcement de PQ ratchet habilitados. | Minutas de aprobacion de governance, procedimiento de rollback direct-only, alarmas de downgrade, reporte firmado de metricas de exito. |

No hay **ruta de salto**: cada fase debe entregar la telemetria y los artefactos de governance de la etapa anterior antes de la promocion.

## Kit de onboarding de testnet

Cada operador de relay recibe un paquete deterministico con los siguientes archivos:

| Artefacto | Descripcion |
|----------|-------------|
| `01-readme.md` | Resumen, puntos de contacto y cronograma. |
| `02-checklist.md` | Checklist de pre-flight (hardware, reachability de red, verificacion de guard policy). |
| `03-config-example.toml` | Configuracion minima de relay + orchestrator SoraNet alineada con bloques de compliance SNNet-9, incluyendo un bloque `guard_directory` que fija el hash del snapshot de guard mas reciente. |
| `04-telemetry.md` | Instrucciones para cablear los dashboards de privacy metrics de SoraNet y los thresholds de alertas. |
| `05-incident-playbook.md` | Procedimiento de respuesta a brownout/downgrade con matriz de escalamiento. |
| `06-verification-report.md` | Plantilla que los operadores completan y devuelven una vez que pasan los smoke tests. |

Una copia renderizada vive en `docs/examples/soranet_testnet_operator_kit/`. Cada promocion refresca el kit; los numeros de version siguen la fase (por ejemplo, `testnet-kit-vT0.1`).

Para operadores de beta publica (T1), el brief conciso en `docs/source/soranet/snnet10_beta_onboarding.md` resume prerequisitos, entregables de telemetria y el flujo de envio, mientras apunta al kit deterministico y a los helpers de validacion.

`cargo xtask soranet-testnet-feed` genera el feed JSON que agrega la ventana de promocion, roster de relays, reporte de metricas, evidencia de drills y hashes de adjuntos referenciados por la plantilla de stage-gate. Firma los drill logs y adjuntos con `cargo xtask soranet-testnet-drill-bundle` primero para que el feed registre `drill_log.signed = true`.

## Metricas de exito

La promocion entre fases se habilita con la siguiente telemetria, recolectada por un minimo de dos semanas:

- `soranet_privacy_circuit_events_total`: 95% de los circuits completan sin eventos de brownout o downgrade; el 5% restante se limita por el supply PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: <1% de las sesiones de fetch por dia activan brownout fuera de drills programados.
- `soranet_privacy_gar_reports_total`: variacion dentro de +/-10% del mix esperado de categorias GAR; los picos deben explicarse por updates de policy aprobados.
- Tasa de exito de tickets PoW: >=99% dentro de la ventana objetivo de 3 s; reportado via `soranet_privacy_throttles_total{scope="congestion"}`.
- Latencia (percentil 95) por region: <200 ms una vez que los circuits estan completamente armados, capturado via `soranet_privacy_rtt_millis{percentile="p95"}`.

Los templates de dashboards y alertas viven en `dashboard_templates/` y `alert_templates/`; reflejalos en tu repositorio de telemetria y agregalos a checks de lint de CI. Usa `cargo xtask soranet-testnet-metrics` para generar el reporte de cara a governance antes de pedir promocion.

Las presentaciones de stage-gate deben seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que enlaza el formulario Markdown listo para copiar bajo `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Checklist de verificacion

Los operadores deben aprobar lo siguiente antes de entrar en cada fase:

- ✅ Relay advert firmado con el admission envelope actual.
- ✅ Smoke test de guard rotation (`tools/soranet-relay --check-rotation`) aprobado.
- ✅ `guard_directory` apunta al artefacto `GuardDirectorySnapshotV2` mas reciente y `expected_directory_hash_hex` coincide con el digest del comite (el arranque del relay registra el hash validado).
- ✅ Las metricas de PQ ratchet (`sorafs_orchestrator_pq_ratio`) se mantienen por encima de los thresholds objetivo para la etapa solicitada.
- ✅ El config de compliance GAR coincide con el tag mas reciente (ver el catalogo SNNet-9).
- ✅ Simulacion de alarma de downgrade (deshabilitar collectors, esperar alerta en 5 min).
- ✅ Drill PoW/DoS ejecutado con pasos de mitigacion documentados.

Una plantilla prellenada se incluye en el kit de onboarding. Los operadores envian el reporte completado a la mesa de ayuda de governance antes de recibir credenciales de produccion.

## Governance y reportes

- **Control de cambios:** las promociones requieren aprobacion del Governance Council registrada en las minutas del consejo y adjunta a la pagina de estado.
- **Resumen de estado:** publicar actualizaciones semanales que resuman conteo de relays, ratio PQ, incidentes de brownout y action items pendientes (se almacenan en `docs/source/status/soranet_testnet_digest.md` cuando inicie la cadencia).
- **Rollbacks:** mantener un plan de rollback firmado que regrese la red a la fase anterior en 30 minutos, incluyendo invalidacion de DNS/guard cache y plantillas de comunicacion con clients.

## Activos de soporte

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa el kit de onboarding desde `xtask/templates/soranet_testnet/` hacia el directorio destino (por defecto `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` evalua las metricas de exito SNNet-10 y emite un reporte estructurado pass/fail adecuado para revisiones de governance. Un snapshot de ejemplo vive en `docs/examples/soranet_testnet_metrics_sample.json`.
- Las plantillas de Grafana y Alertmanager viven en `dashboard_templates/soranet_testnet_overview.json` y `alert_templates/soranet_testnet_rules.yml`; copialas en tu repositorio de telemetria o conectalas a checks de lint en CI.
- La plantilla de comunicacion de downgrade para mensajes de SDK/portal reside en `docs/source/soranet/templates/downgrade_communication_template.md`.
- Los digests semanales de estado deben usar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canonica.

Los pull requests deben actualizar esta pagina junto con cualquier cambio de artefactos o telemetria para que el plan de rollout se mantenga canonico.
