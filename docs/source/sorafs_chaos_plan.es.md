---
lang: es
direction: ltr
source: docs/source/sorafs_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 43dfc53b7a951ec0b036a56941335a4fd05657da91c7a3f727e3846b8c647d23
source_last_modified: "2025-11-08T02:24:51.340739+00:00"
translation_last_reviewed: "2026-01-30"
---

# Plan de simulacros de caos y playbook de incidentes de SoraFS (Borrador)

## Escenarios de caos (iniciales)

1. **Caída de gateway**
   - Simular una falla de gateway a nivel de región.
   - Respuesta esperada: el orquestador reruta, se disparan alertas y se declara el incidente.
2. **Aumento de fallos de pruebas**
   - Inyectar pruebas corruptas en el gateway.
   - Esperado: alertas por fallos de pruebas, cuarentenas automatizadas, respuesta de incidentes.
3. **Retraso de replicación**
   - Retrasar respuestas de chunks para simular un proveedor lento.
   - Monitorear reintentos del orquestador y alarmas de utilización de capacidad.
4. **Desajuste de admisión**
   - Eliminar o expirar el sobre de admisión; verificar que se emita la telemetría de rechazo.

## Componentes del playbook

- **Checklist de incidente** (a quién avisar, canales de comunicación).
- **Runbook** por escenario con pasos de contención/remediación.
- **Plantilla de postmortem** con impacto en SLO y acciones correctivas.
- **Runlog** que capture cronología, snapshots de métricas y puntos de decisión.

## Cadencia y seguimiento de los simulacros

| Frecuencia | Paquete de escenarios | Participantes | Notas |
|-----------|------------------------|--------------|-------|
| Trimestral | Caída de gateway + tormenta de reintentos del orquestador | SRE de almacenamiento, líder de observabilidad, enlace de gobernanza | Ejecutar primero en staging; el ensayo en producción queda condicionado a éxito en staging. |
| Semestral | Aumento de fallos de pruebas (PoR + PoTR) | SRE de almacenamiento, Crypto WG, herramientas | Validar tooling de replay de pruebas y notificaciones de gobernanza. |
| Mensual | Revisión puntual de retraso de replicación | Rotación de SRE de almacenamiento | Usar manifiestos de staging y registrar tiempo de recuperación del backlog. |

Después de cada ejercicio, añade el resultado en `ops/drill-log.md` usando
`scripts/telemetry/log_sorafs_drill.sh` y registra la lista de acciones en la plantilla
de postmortem. Ejecuta `scripts/telemetry/validate_drill_log.sh` durante la revisión para
asegurar que el log permanezca bien formado. Usa `--status scheduled` para simulacros
planificados, `pass`/`fail` una vez finalice el ensayo, y `follow-up` cuando queden
acciones pendientes. El helper también acepta `--log <path>` para que pruebas en seco o
tests automatizados apunten a ubicaciones temporales sin tocar `ops/drill-log.md`.

Los simulacros de TLS/ECH también deben ejecutar `cargo xtask sorafs-gateway-probe` con
las nuevas flags `--drill-*`, `--summary-json` y `--pagerduty-*` para que la sonda del
gateway registre automáticamente la corrida, emita evidencia JSON bajo
`artifacts/sorafs_gateway_probe/` y (cuando se solicita) levante un incidente de
entrenamiento en PagerDuty si hay desvíos de encabezados o de políticas GAR. El helper
de telemetría (`scripts/telemetry/run_sorafs_gateway_probe.sh`) ahora reenvía estas flags,
por lo que los runbooks existentes pueden adoptar logging/paging nativo sin scripts
adicionales.

## Herramientas

- Controlador del harness (del plan de proveedor simulado) para inyectar fallos.
- Dashboards de observabilidad del plan de métricas usados durante los simulacros.
- Puntuación automatizada para evaluar tiempos de respuesta y completitud.

## Flujo de comunicación

- **Canales de incidente**
  - Primario: canal de Slack `#sorafs-incident` (auto-creado por incidente con el bot `inc-bot`).
  - Escalamiento: servicio de PagerDuty “SoraFS On-Call”. Alertas críticas paginan a L1; si no se reconoce en 5 minutos, se escala al líder L2.
  - Actualizaciones de estado: canal `#sorafs-status` en Slack para partes interesadas; mensajes plantilla generados por el bot.
- **Roles**
  - Incident Commander (IC): ingeniero on-call que reconoce la alerta de PagerDuty.
  - Communications Lead (CL): PM/DevRel designado para manejar actualizaciones internas/externas.
  - Scribe: rota por incidente; registra la cronología en `runbooks/incident_log.md`.
- **Checklist (por incidente)**
  1. Se dispara la alerta de PagerDuty → el IC la reconoce en 5 minutos.
  2. El IC abre el canal de incidente en Slack (comando `/incident create`).
  3. El CL publica el primer estado en `#sorafs-status` en 10 minutos.
  4. El Scribe inicia el documento de cronología en Notion (`Incident YYYY-MM-DD`). Se sincroniza al repositorio semanalmente.
  5. Cada 30 minutos, el IC publica actualización (estado, acciones, próximos pasos) hasta resolver.
  6. Tras la resolución, programar postmortem en 3 días hábiles.
- **Notificaciones de simulacros**
  - Los simulacros se anuncian previamente; el bot publica “DRILL – Sin impacto al cliente” para evitar confusiones.
  - PagerDuty usa modo “training” para evitar paginar al on-call real salvo intención explícita.

## Alineación con reportes de gobernanza

- **Vínculos GAR**
  - Cada escenario de caos se mapea a categorías del Governance Accountability Report (GAR). Ejemplo: la caída de gateway activa la métrica GAR de disponibilidad.
  - Durante simulacros/incidentes, el IC registra si se incumplieron umbrales GAR. Si es así, genera el payload Norito `GarIncidentReportV1` con:
    - `incident_id`, `metric`, `duration`, `impact`.
  - Los reportes se anexan al DAG de gobernanza para revisión mensual de cumplimiento.
- **Dashboards de transparencia**
  - Los resultados alimentan el plan de transparencia: métricas `sorafs_chaos_drill_duration_seconds` y `sorafs_gar_incidents_total`.
  - El boletín de gobernanza incluye un resumen de simulacros con preparación y acciones pendientes.

## Integración de procedimientos de incidentes TLS/ECH

- **Plan de referencia** `sorafs_gateway_tls_automation.md` (SF-5b). Los simulacros incluyen un escenario de fallo TLS/ECH:
  - Simular certificados expirados, automatización TLS mal emitida o deriva de configuración ECH.
  - Pasos del runbook:
    1. Detectar mediante alerta `tls_cert_expiry` o métricas de handshake fallidas.
    2. Disparar la canalización de renovación automática (`scripts/tls/renew_all.sh`) o recurrir a reemisión manual.
    3. Validar con el comando CLI `sorafs tls verify`.
    4. Documentar la rotación de ECH y confirmar la actualización de registros DNS.
- **Coordinación**
  - El equipo de seguridad participa en los simulacros TLS/ECH.
  - Resultados se integran en el plan de cumplimiento para demostrar la adhesión a la gestión de certificados.
  - El postmortem debe verificar que las mejoras de automatización (por ejemplo, probes sintéticos) se registren como acciones.
