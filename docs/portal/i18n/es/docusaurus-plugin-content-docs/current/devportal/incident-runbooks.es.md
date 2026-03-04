---
lang: es
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbooks de incidentes y ejercicios de rollback

## propuesta

El item del roadmap **DOCS-9** exige playbooks accionables mas un plan de práctica para que
los operadores del portal puedan recuperarse de fallas de entrega sin adivinar. esta nota
cubre tres incidentes de alta señal: despliegues fallidos, degradación de replicación y
caidas de analitica, y documenta los ejercicios trimestrales que prueban que el rollback de
alias y la validacion sintetica siguen funcionando de extremo a extremo.

### Material relacionado

- [`devportal/deploy-guide`](./deploy-guide) - flujo de embalaje, firma y promoción de alias.
- [`devportal/observability`](./observability) - etiquetas de liberación, analítica y sondas referenciados abajo.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria del registro y umbrales de escalada.
- `docs/portal/scripts/sorafs-pin-release.sh` y ayudantes `npm run probe:*`
  referenciados en las listas de verificación.

### Telemetría y herramientas compartidas| Señal / Herramienta | propuesta |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (cumplido/incumplido/pendiente) | Detecta bloqueos de replicación y brechas de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Cuantifica la profundidad del backlog y la latencia de completado para el triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Muestra fallas del gateway que a menudo siguen a un despliegue defectuoso. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Sondas sintéticas que gatean liberaciones y validan reversiones. |
| `npm run check:links` | Puerta de enlaces rotos; se usa después de cada mitigación. |
| `sorafs_cli manifest submit ... --alias-*` (usado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promoción/reversión de alias. |
| Placa `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de rechazos/alias/TLS/replicacion. Alertas de PagerDuty hacen referencia a estos paneles como evidencia. |

## Runbook - Despliegue fallido o artefacto incorrecto

### Condiciones de disparo

- Fallan las sondas de vista previa/producción (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` después de un lanzamiento.
- QA manual detecta rutas rotas o fallas del proxy Pruébalo inmediatamente después de la
  promoción del alias.

### Contención inmediata1. **Congelar despliegues:** marca el pipeline CI con `DEPLOY_FREEZE=1` (entrada del flujo de trabajo de
   GitHub) o pausar el trabajo de Jenkins para que no salgan más artefactos.
2. **Capturar artefactos:** descarga `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, y la salida de sondas del build fallido para que
   el rollback referencia los resúmenes exactos.
3. **Notificar a las partes interesadas:** Storage SRE, lead de Docs/DevRel, y el oficial de guardia de
   gobernanza para concientización (especialmente cuando `docs.sora` esta impactado).

### Procedimiento de reversión

1. Identifica el manifiesto último conocido bueno (LKG). El flujo de trabajo de producción los guarda.
   bajo `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincula el alias a ese manifest con el helper de envío:

```bash
cd docs/portal
./scripts/sorafs-pin-release.sh \
  --build-dir build \
  --artifact-dir artifacts/revert-$(date +%Y%m%d%H%M) \
  --sorafs-dir artifacts/revert-$(date +%Y%m%d%H%M)/sorafs \
  --pin-min-replicas 5 \
  --alias "docs-prod-revert" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --torii-url "${TORII_URL}" \
  --submitted-epoch "$(date +%Y%m%d)" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --skip-submit

# swap in the LKG artefacts before submission
cp /secure/archive/lkg/portal.manifest.to artifacts/.../sorafs/portal.manifest.to
cp /secure/archive/lkg/portal.manifest.bundle.json artifacts/.../sorafs/

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest artifacts/.../sorafs/portal.manifest.to \
  --chunk-plan artifacts/.../sorafs/portal.plan.json \
  --torii-url "${TORII_URL}" \
  --authority "${AUTHORITY}" \
  --private-key "${PRIVATE_KEY}" \
  --alias-namespace "${PIN_ALIAS_NAMESPACE}" \
  --alias-name "${PIN_ALIAS_NAME}" \
  --alias-proof "${PIN_ALIAS_PROOF_PATH}" \
  --metadata rollback_from="${FAILED_RELEASE}" \
  --summary-out artifacts/.../sorafs/rollback.submit.json
```

3. Registre el resumen del rollback en el ticket del incidente junto con los digests del
   manifest LKG y del manifest fallido.

### Validación

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (ver la guía de despliegue) para confirmar que el manifiesto re-promocionado sigue
   coincidiendo con el CAR archivado.
4. `npm run probe:tryit-proxy` para asegurar que el proxy Try-It staging regreso.

### Post-incidente1. Rehabilita el pipeline de despliegue solo después de entender la causa raiz.
2. Rellena entradas de "Lecciones aprendidas" en [`devportal/deploy-guide`](./deploy-guide)
   con nuevas notas, si aplica.
3. Abre defectos para el conjunto de pruebas fallidas (sonda, verificador de enlaces, etc.).

## Runbook - Degradación de replicación

### Condiciones de disparo

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  abrazadera_min(suma(torii_sorafs_replication_sla_total{resultado=~"cumplido|perdido"}), 1) <
  0,95` por 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` por 10 minutos (ver
  `pin-registry-ops.md`).
- Gobernanza reporta disponibilidad lenta del alias despues de un lanzamiento.

### Triaje

1. Inspecciona paneles de control de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) párr.
   Confirmar si el backlog está localizado en una clase de almacenamiento o en una flota de proveedores.
2. Cruza logs de Torii para advertencias `sorafs_registry::submit_manifest` para determinar si
   las presentaciones están fallando.
3. Muestrea salud de replicas via `sorafs_cli manifest status --manifest ...` (lista resultados
   de replicación por proveedor).

### Mitigación1. Reemite el manifest con mayor conteo de réplicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que el planificador distribuya carga en un conjunto mayor
   de proveedores. Registre el nuevo resumen en el registro del incidente.
2. Si el backlog esta atado a un proveedor único, deshabilitalo temporalmente vía el
   planificador de replicacion (documentado en `pin-registry-ops.md`) y envia un nuevo
   manifest forzando a los otros proveedores a refrescar el alias.
3. Cuando la frescura del alias es mas critica que la paridad de replicacion, re-vincula el
   alias a un manifest caliente ya staged (`docs-preview`), luego publica un manifest de
   seguimiento una vez que SRE limpie el backlog.

### Recuperación y cierre

1. Monitorea `torii_sorafs_replication_sla_total{outcome="missed"}` para asegurar que el
   conteo se estabiliza.
2. Captura la salida de `sorafs_cli manifest status` como evidencia de que cada réplica esta
   de nuevo en cumplimiento.
3. Abre o actualiza el post-mortem del backlog de replicacion con los siguientes pasos
   (escalado de proveedores, tuning del fragmentador, etc.).

## Runbook - Caída de análisis o telemetría

### Condiciones de disparo

- `npm run probe:portal` tiene exito pero los tableros dejan de ingerir eventos de
  `AnalyticsTracker` por >15 minutos.
- La revisión de privacidad detecta un aumento inesperado en eventos descartados.
- `npm run probe:tryit-proxy` falla en caminos `/probe/analytics`.

###Respuesta1. Verifica entradas de compilación: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` en el artefacto del lanzamiento (`build/release.json`).
2. Reejecuta `npm run probe:portal` con `DOCS_ANALYTICS_ENDPOINT` apuntando al
   Collector de staging para confirmar que el tracker sigue emitiendo cargas útiles.
3. Si los coleccionistas están caídos, setea `DOCS_ANALYTICS_ENDPOINT=""` y reconstruir
   para que el tracker haga cortocircuito; registra la ventana de corte en la
   linea de tiempo del incidente.
4. Valida que `scripts/check-links.mjs` siga la toma de huellas dactilares `checksums.sha256`
   (las caídas de analítica *no* deben bloquear la validación del mapa del sitio).
5. Cuando el coleccionista se recupera, corre `npm run test:widgets` para ejecutar los
   pruebas unitarias del helper de analitica antes de republicar.

### Post-incidente

1. Actualiza [`devportal/observability`](./observability) con nuevas limitaciones del
   colector o requisitos de muestreo.
2. Emite aviso de gobernanza si se perdieron o se redactaron datos de analítica fuera
   de política.

## Ejercicios trimestrales de resiliencia

Ejecuta ambos simulacros durante el **primer martes de cada trimestre** (Ene/Abr/Jul/Oct)
o inmediatamente después de cualquier cambio mayor de infraestructura. Guarda artefactos bajo
`artifacts/devportal/drills/<YYYYMMDD>/`.| Taladro | Pasos | Pruebas |
| ----- | ----- | -------- |
| Ensayo de reversión de alias | 1. Repita el rollback de "Despliegue fallido" usando el manifiesto de producción más reciente.2. Re-vincular a producción una vez que los probes pasen.3. Registrar `portal.manifest.submit.summary.json` y logs de probes en la carpeta del taladro. | `rollback.submit.json`, salida de sondas, y etiqueta de liberación del ensayo. |
| Auditorio de validación sintética | 1. Ejecutar `npm run probe:portal` y `npm run probe:tryit-proxy` contra producción y puesta en escena.2. Ejecutar `npm run check:links` y archivar `build/link-report.json`.3. Adjuntar capturas de pantalla/exportaciones de paneles Grafana confirmando el exito del probe. | Logs de probe + `link-report.json` referenciando la huella digital del manifiesto. |

Escala los taladros perdidos al manager de Docs/DevRel y a la revisión de gobernanza de SRE,
ya que el roadmap exige evidencia trimestral determinista de que el rollback de alias y los
sondas del portal siguen saludables.

## Coordinacion de PagerDuty y on-call- El servicio PagerDuty **Docs Portal Publishing** es dueno de las alertas generadas desde
  `dashboards/grafana/docs_portal.json`. Las reglas `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry` paginan al primario de Docs/DevRel
  con Almacenamiento SRE como secundario.
- Cuando se página, incluye el `DOCS_RELEASE_TAG`, adjunta capturas de pantalla de los paneles Grafana
  afectados y enlaza la salida de probe/link-check en las notas del incidente antes de
  iniciar mitigación.
- Después de la mitigación (rollback o reimplementar), reejecuta `npm run probe:portal`,
  `npm run check:links`, y captura instantáneas Grafana frescos mostrando las métricas
  de nuevo dentro de umbrales. Adjunta toda la evidencia al incidente de PagerDuty
  antes de resolverlo.
- Si dos alertas disparan al mismo tiempo (por ejemplo expiracion TLS mas backlog), triage
  rechazos primero (detener publicación), ejecuta el procedimiento de rollback, luego
  limpia items de TLS/backlog con Storage SRE en el bridge.