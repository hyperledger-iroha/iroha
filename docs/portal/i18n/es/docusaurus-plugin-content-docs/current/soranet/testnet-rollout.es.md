---
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lanzamiento de testnet
título: Despliegue de testnet de SoraNet (SNNet-10)
sidebar_label: Despliegue de testnet (SNNet-10)
descripción: Plan de activación por fases, kit de onboarding y puertas de telemetría para promociones de testnet de SoraNet.
---

:::nota Fuente canónica
Esta página refleja el plan de implementación SNNet-10 en `docs/source/soranet/testnet_rollout_plan.md`. Mantengan ambas copias alineadas hasta que los documentos heredados se retiren.
:::

SNNet-10 coordina la activación escalada de la superposición de anonimato de SoraNet en toda la red. Usa este plan para traducir el bullet del roadmap en entregables concretos, runbooks y gates de telemetria para que cada operador entienda las expectativas antes de que SoraNet sea el transporte por defecto.

## Fases de lanzamiento| Fase | Cronograma (objetivo) | Alcance | Artefactos requeridos |
|-------|-------------------|-------|--------------------|
| **T0 - Testnet cerrado** | Cuarto trimestre de 2026 | 20-50 relevos en >=3 ASNs operados por contribuyentes core. | Kit de incorporación de testnet, conjunto de humo de fijación de guardia, línea base de latencia + métricas de PoW, registro de simulacro de apagón. |
| **T1 - Beta pública** | Primer trimestre de 2027 | >=100 relés, rotación de guardia habilitada, vinculación de salida aplicada, SDK betas por defecto en SoraNet con `anon-guard-pq`. | Kit de onboarding actualizado, checklist de verificación de operadores, SOP de publicación de directorio, paquete de paneles de telemetría, reportes de ensayo de incidentes. |
| **T2 - Red principal por defecto** | Q2 2027 (condicionado a completar SNNet-6/7/9) | La red de producción por defecto en SoraNet; transportes obfs/MASQUE y cumplimiento de PQ ratchet habilitados. | Minutas de aprobación de gobernanza, procedimiento de rollback direct-only, alarmas de downgrade, informe firmado de métricas de éxito. |

No hay **ruta de salto**: cada fase debe entregar la telemetría y los artefactos de gobierno de la etapa anterior antes de la promoción.

## Kit de incorporación de testnet

Cada operador de retransmisión recibe un paquete determinístico con los siguientes archivos:| Artefacto | Descripción |
|----------|-------------|
| `01-readme.md` | Resumen, puntos de contacto y cronograma. |
| `02-checklist.md` | Lista de verificación previa al vuelo (hardware, accesibilidad de red, política de verificación de guardia). |
| `03-config-example.toml` | Configuración mínima de relé + orquestador SoraNet alineada con bloques de cumplimiento SNNet-9, incluyendo un bloque `guard_directory` que fija el hash del snapshot de guard más reciente. |
| `04-telemetry.md` | Instrucciones para cablear los paneles de métricas de privacidad de SoraNet y los umbrales de alertas. |
| `05-incident-playbook.md` | Procedimiento de respuesta a brownout/downgrade con matriz de escalada. |
| `06-verification-report.md` | Plantilla que los operadores completan y devuelven una vez que pasan las pruebas de humo. |

Una copia renderizada vive en `docs/examples/soranet_testnet_operator_kit/`. Cada promoción refresca el kit; los números de versión siguen la fase (por ejemplo, `testnet-kit-vT0.1`).

Para operadores de beta publica (T1), el breve conciso en `docs/source/soranet/snnet10_beta_onboarding.md` resume prerequisitos, entregables de telemetria y el flujo de envío, mientras apunta al kit determinístico y a los ayudantes de validación.`cargo xtask soranet-testnet-feed` genera el feed JSON que agrega la ventana de promoción, roster de relés, informe de métricas, evidencia de ejercicios y hashes de adjuntos referenciados por la plantilla de stage-gate. Firme los registros de perforación y adjuntos con `cargo xtask soranet-testnet-drill-bundle` primero para que el registro de alimentación `drill_log.signed = true`.

## Métricas de éxito

La promoción entre fases se habilita con la siguiente telemetría, recolectada por un mínimo de dos semanas:

- `soranet_privacy_circuit_events_total`: 95% de los circuitos se completan sin eventos de brownout o downgrade; El 5% restante se limita al suministro PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: =99% dentro de la ventana objetivo de 3 s; reportado vía `soranet_privacy_throttles_total{scope="congestion"}`.
- Latencia (percentil 95) por región: <200 ms una vez que los circuitos están completamente armados, capturado vía `soranet_privacy_rtt_millis{percentile="p95"}`.

Los templates de tableros y alertas viven en `dashboard_templates/` y `alert_templates/`; reflejalos en tu repositorio de telemetría y agregalos a checks de lint de CI. Usa `cargo xtask soranet-testnet-metrics` para generar el reporte de cara a gobernanza antes de pedir promoción.Las presentaciones de stage-gate deben seguir `docs/source/soranet/snnet10_stage_gate_template.md`, que enlaza el formulario Markdown listo para copiar bajo `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Lista de verificación de verificación

Los operadores deben aprobar lo siguiente antes de entrar en cada fase:

- ✅ Anuncio de retransmisión firmado con el sobre de admisión actual.
- ✅ Prueba de humo de rotación de guardia (`tools/soranet-relay --check-rotation`) aprobada.
- ✅ `guard_directory` apunta al artefacto `GuardDirectorySnapshotV2` más reciente y `expected_directory_hash_hex` coinciden con el digest del comité (el arranque del relé registra el hash validado).
- ✅ Las métricas de PQ ratchet (`sorafs_orchestrator_pq_ratio`) se mantienen por encima de los umbrales objetivo para la etapa solicitada.
- ✅ La configuración de cumplimiento GAR coincide con la etiqueta más reciente (ver el catálogo SNNet-9).
- ✅ Simulación de alarma de downgrade (deshabilitar recolectores, esperar alerta en 5 min).
- ✅ Drill PoW/DoS ejecutado con pasos de mitigación documentados.

Una plantilla prellenada se incluye en el kit de onboarding. Los operadores envían el informe completado a la mesa de ayuda de gobernanza antes de recibir credenciales de producción.

## Gobernanza y reportes- **Control de cambios:** las promociones requieren aprobación del Governance Council registrada en las minutas del consejo y adjunta a la página de estado.
- **Resumen de estado:** publica actualizaciones semanales que resuman conteo de relés, ratio PQ, incidentes de apagón y elementos de acción pendientes (se almacenan en `docs/source/status/soranet_testnet_digest.md` cuando inicie la cadencia).
- **Rollbacks:** mantenga un plan de rollback firmado que regrese la red a la fase anterior en 30 minutos, incluyendo invalidación de DNS/guard cache y plantillas de comunicación con clientes.

## Activos de soporte

- `cargo xtask soranet-testnet-kit [--out <dir>]` materializa el kit de onboarding desde `xtask/templates/soranet_testnet/` hacia el directorio de destino (por defecto `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` evalúa las métricas de éxito SNNet-10 y emite un informe estructurado pasa/no pasa adecuado para revisiones de gobernanza. Una instantánea de ejemplo vive en `docs/examples/soranet_testnet_metrics_sample.json`.
- Las plantillas de Grafana y Alertmanager viven en `dashboard_templates/soranet_testnet_overview.json` y `alert_templates/soranet_testnet_rules.yml`; copialas en tu repositorio de telemetria o conectalas a checks de lint en CI.
- La plantilla de comunicación de downgrade para mensajes de SDK/portal reside en `docs/source/soranet/templates/downgrade_communication_template.md`.
- Los resúmenes semanales de estado deben usar `docs/source/status/soranet_testnet_weekly_digest.md` como forma canónica.

Los pull request deben actualizar esta página junto con cualquier cambio de artefactos o telemetría para que el plan de implementación se mantenga canónico.