---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/operations-playbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: manual de operaciones
título: SoraFS آپریشنز پلے بک
sidebar_label: آپریشنز پلے بک
descripción: SoraFS آپریٹرز کے لیے respuesta a incidentes گائیڈز اور simulacro de caos طریقۂ کار۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs_ops_playbook.md` میں برقرار رکھے گئے runbook کی عکاسی کرتا ہے۔ جب تک Sphinx ڈاکیومنٹیشن مکمل طور پر منتقل نہ ہو جائے دونوں نقول کو ہم آہنگ رکھیں۔
:::

## کلیدی حوالہ جات

- Activos de observabilidad: `dashboards/grafana/`, paneles de control Grafana y `dashboards/alerts/`, reglas de alerta Prometheus.
- Catálogo métrico: `docs/source/sorafs_observability_plan.md`.
- Superficies de telemetría del orquestador: `docs/source/sorafs_orchestrator_plan.md`.

## Matriz de escalamiento

| ترجیح | ٹرگر مثالیں | بنیادی de guardia | بیک اپ | نوٹس |
|--------|--------------|---------------|--------|------|
| P1 | عالمی interrupción de la puerta de enlace, tasa de falla de PoR> 5% (15 meses), acumulación de replicación ہر 10 منٹ میں دوگنا | Almacenamiento SRE | Observabilidad TL | اگر اثر 30 منٹ سے زیادہ ہو تو consejo de gobierno کو participar کریں۔ |
| P2 | علاقائی incumplimiento de SLO de latencia de puerta de enlace, pico de reintento del orquestador بغیر SLA اثر کے | Observabilidad TL | Almacenamiento SRE | lanzamiento جاری رکھیں مگر نئے manifiesta puerta کریں۔ |
| P3 | Alertas de غیر اہم (estancamiento manifiesto, capacidad 80–90%) | Triaje de admisión | Gremio de operaciones | اگلے کاروباری دن میں نمٹا دیں۔ |

## Interrupción de la puerta de enlace/disponibilidad degradada

**Detección**

- Alertas: `SoraFSGatewayAvailabilityDrop`, `SoraFSGatewayLatencySlo`.
- Tablero: `dashboards/grafana/sorafs_gateway_overview.json`.

**Acciones inmediatas**1. panel de tasa de solicitudes کے ذریعے alcance کنفرم کریں (proveedor único versus flota)۔
2. Multi-proveedor y configuración de operaciones (`docs/source/sorafs_gateway_self_cert.md`) `sorafs_gateway_route_weights` para el enrutamiento Torii para proveedores de servicios múltiples کریں۔
3. Los proveedores de servicios de terceros incluyen clientes CLI/SDK que utilizan el respaldo de “búsqueda directa” (`docs/source/sorafs_node_client_protocol.md`).

**Triaje**

- `sorafs_gateway_stream_token_limit` کے مقابل utilización del token de flujo دیکھیں۔
- Errores de admisión de TLS یا کے لیے registros de puerta de enlace inspeccionar کریں۔
- `scripts/telemetry/run_schema_diff.sh` چلائیں تاکہ puerta de enlace کے esquema exportado کا versión esperada سے coincidencia ثابت ہو۔

**Opciones de solución**

- صرف متاثرہ proceso de puerta de enlace ری اسٹارٹ کریں؛ پورے cluster کو reciclar نہ کریں جب تک متعدد proveedores فیل نہ ہوں۔
- Saturación y límite de token de flujo entre 10 y 15 %
- استحکام کے بعد autocertificado دوبارہ چلائیں (`scripts/sorafs_gateway_self_cert.sh`).

**Después del incidente**

- `docs/source/sorafs/postmortem_template.md` استعمال کرتے ہوئے P1 postmortem فائل کریں۔
- اگر remediación میں intervenciones manuales شامل ہوں تو seguimiento simulacro de caos شیڈول کریں۔

## Pico de falla de prueba (PoR / PoTR)

**Detección**

- Alertas: `SoraFSProofFailureSpike`, `SoraFSPoTRDeadlineMiss`.
- Tablero: `dashboards/grafana/sorafs_proof_integrity.json`.
- Telemetría: `torii_sorafs_proof_stream_events_total` اور `sorafs.fetch.error` eventos جن میں `provider_reason=corrupt_proof` ہو۔

**Acciones inmediatas**

1. Registro de manifiesto کو flag کر کے نئی congelación de admisiones del manifiesto کریں (`docs/source/sorafs/manifest_pipeline.md`).
2. Gobernanza کو notificar کریں تاکہ متاثرہ proveedores کے incentivos pausar ہوں۔**Triaje**

- Profundidad de la cola de desafío PoR کو `sorafs_node_replication_backlog_total` کے مقابل چیک کریں۔
- حالیہ implementaciones کے لیے canalización de verificación de prueba (`crates/sorafs_node/src/potr.rs`) validar کریں۔
- versiones de firmware del proveedor کو registro de operador سے comparar کریں۔

**Opciones de solución**

- Manifestación de manifiesto `sorafs_cli proof stream` Activación de repeticiones PoR Activación
- اگر pruebas مسلسل fallar ہوں تو registro de gobernanza اپڈیٹ کر کے proveedor کو conjunto activo سے نکالیں اور orquestador marcadores کو actualizar کرنے پر مجبور کریں۔

**Después del incidente**

- اگلے پروڈکشن implementar سے پہلے PoR escenario de simulacro de caos چلائیں۔
- plantilla postmortem میں captura de lecciones کریں اور lista de verificación de calificación del proveedor اپڈیٹ کریں۔

## Retraso de replicación/crecimiento del trabajo pendiente

**Detección**

- Alertas: `SoraFSReplicationBacklogGrowing`, `SoraFSCapacityPressure`. `dashboards/alerts/sorafs_capacity_rules.yml` امپورٹ کریں اور
  `promtool test rules dashboards/alerts/tests/sorafs_capacity_rules.test.yml`
  promoción سے پہلے چلائیں تاکہ Umbrales documentados por Alertmanager کو reflejan کرے۔
- Tablero: `dashboards/grafana/sorafs_capacity_health.json`.
- Métricas: `sorafs_node_replication_backlog_total`, `sorafs_node_manifest_refresh_age_seconds`.

**Acciones inmediatas**

1. trabajo pendiente کا alcance چیک کریں (proveedor único یا flota) اور غیر ضروری tareas de replicación روکیں۔
2. Aislamiento del trabajo pendiente, programador de replicación, pedidos de pedidos, proveedores alternativos, reasignación de pedidos

**Triaje**- telemetría del orquestador میں ráfagas de reintento دیکھیں جو backlog بڑھا سکتے ہیں۔
- objetivos de almacenamiento کے لیے headroom کنفرم کریں (`sorafs_node_capacity_utilisation_percent`).
- Cambios de configuración (actualizaciones de perfiles de fragmentos, cadencia de prueba) ریویو کریں۔

**Opciones de solución**

- `sorafs_cli` کو `--rebalance` کے ساتھ چلائیں تاکہ contenido redistribuir ہو۔
- متاثرہ proveedor کے لیے trabajadores de replicación کو escala horizontal کریں۔
- Las ventanas TTL alinean el activador de actualización del manifiesto کریں۔

**Después del incidente**

- fallo de saturación del proveedor پر مرکوز taladro de capacidad شیڈول کریں۔
- documentación de replicación SLA کو `docs/source/sorafs_node_client_protocol.md` میں اپڈیٹ کریں۔

## Cadencia de ejercicios del caos

- **Trimestral**: interrupción combinada de la puerta de enlace + simulación de reintento de tormenta del orquestador.
- **Semestral**: inyección de fallas de PoR/PoTR para proveedores y recuperación.
- **Verificación puntual mensual**: manifiestos de preparación en un escenario de retraso de replicación.
- simulacros de registro de runbook compartido (`ops/drill-log.md`) میں اس طرح ٹریک کریں:

  ```bash
  scripts/telemetry/log_sorafs_drill.sh \
    --scenario "Gateway outage chaos drill" \
    --status pass \
    --ic "Alex Morgan" \
    --scribe "Priya Patel" \
    --notes "Failover to west cluster succeeded" \
    --log ops/drill-log.md \
    --link "docs/source/sorafs/postmortem_template.md"
  ```

- confirma la validación del registro کریں:

  ```bash
  scripts/telemetry/validate_drill_log.sh
  ```

- Ejercicios y ejercicios کے لیے `--status scheduled`, مکمل ejecuta کے لیے `pass`/`fail`, اور کھلے elementos de acción کے لیے `follow-up` استعمال کریں۔
- simulacros یا verificación automatizada کے لیے destino `--log` سے anulación کریں؛ ورنہ اسکرپٹ `ops/drill-log.md` کو اپڈیٹ کرتا رہے گا۔

## Plantilla post mortemہر Incidente P1/P2 اور retrospectivas de ejercicios de caos کے لیے `docs/source/sorafs/postmortem_template.md` استعمال کریں۔ یہ cronograma de plantilla, cuantificación de impacto, factores contribuyentes, acciones correctivas, اور tareas de verificación de seguimiento کور کرتا ہے۔