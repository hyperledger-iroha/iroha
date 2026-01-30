---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Runbooks de incidentes y drills de rollback

## Proposito

El item del roadmap **DOCS-9** exige playbooks accionables mas un plan de practica para que
los operadores del portal puedan recuperarse de fallas de entrega sin adivinar. Esta nota
cubre tres incidentes de alta senal: despliegues fallidos, degradacion de replicacion y
caidas de analitica, y documenta los drills trimestrales que prueban que el rollback de
alias y la validacion sintetica siguen funcionando end to end.

### Material relacionado

- [`devportal/deploy-guide`](./deploy-guide) - flujo de packaging, signing y promocion de alias.
- [`devportal/observability`](./observability) - release tags, analitica y probes referenciados abajo.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria del registro y umbrales de escalamiento.
- `docs/portal/scripts/sorafs-pin-release.sh` y helpers `npm run probe:*`
  referenciados en los checklists.

### Telemetria y tooling compartidos

| Senal / Tool | Proposito |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (met/missed/pending) | Detecta bloqueos de replicacion y brechas de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Cuantifica la profundidad del backlog y la latencia de completado para el triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Muestra fallas del gateway que a menudo siguen a un deploy defectuoso. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Probes sinteticos que gatean releases y validan rollbacks. |
| `npm run check:links` | Gate de enlaces rotos; se usa despues de cada mitigacion. |
| `sorafs_cli manifest submit ... --alias-*` (usado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promocion/reversion de alias. |
| `Docs Portal Publishing` Grafana board (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de refusals/alias/TLS/replicacion. Alertas de PagerDuty referencian estos paneles como evidencia. |

## Runbook - Despliegue fallido o artefacto incorrecto

### Condiciones de disparo

- Fallan los probes de preview/produccion (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` despues de un rollout.
- QA manual detecta rutas rotas o fallas del proxy Try it inmediatamente despues de la
  promocion del alias.

### Contencion inmediata

1. **Congelar despliegues:** marca el pipeline CI con `DEPLOY_FREEZE=1` (input del workflow de
   GitHub) o pausa el job de Jenkins para que no salgan mas artefactos.
2. **Capturar artefactos:** descarga `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, y la salida de probes del build fallido para que
   el rollback referencie los digests exactos.
3. **Notificar stakeholders:** storage SRE, lead de Docs/DevRel, y el oficial de guardia de
   gobernanza para awareness (especialmente cuando `docs.sora` esta impactado).

### Procedimiento de rollback

1. Identifica el manifest ultimo conocido bueno (LKG). El workflow de produccion los guarda
   bajo `artifacts/devportal/<release>/sorafs/portal.manifest.to`.
2. Re-vincula el alias a ese manifest con el helper de envio:

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

3. Registra el resumen del rollback en el ticket del incidente junto con los digests del
   manifest LKG y del manifest fallido.

### Validacion

1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (ver la guia de despliegue) para confirmar que el manifest re-promocionado sigue
   coincidiendo con el CAR archivado.
4. `npm run probe:tryit-proxy` para asegurar que el proxy Try-It staging regreso.

### Post-incidente

1. Rehabilita el pipeline de despliegue solo despues de entender la causa raiz.
2. Rellena entradas de "Lessons learned" en [`devportal/deploy-guide`](./deploy-guide)
   con nuevas notas, si aplica.
3. Abre defects para el suite de pruebas fallidas (probe, link checker, etc.).

## Runbook - Degradacion de replicacion

### Condiciones de disparo

- Alerta: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95` por 10 minutos.
- `torii_sorafs_replication_backlog_total > 10` por 10 minutos (ver
  `pin-registry-ops.md`).
- Gobernanza reporta disponibilidad lenta del alias despues de un release.

### Triage

1. Inspecciona dashboards de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   confirmar si el backlog esta localizado en una clase de storage o en un fleet de providers.
2. Cruza logs de Torii para warnings `sorafs_registry::submit_manifest` para determinar si
   las submissions estan fallando.
3. Muestrea salud de replicas via `sorafs_cli manifest status --manifest ...` (lista resultados
   de replicacion por provider).

### Mitigacion

1. Reemite el manifest con mayor conteo de replicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que el scheduler distribuya carga en un set mayor
   de providers. Registra el nuevo digest en el log del incidente.
2. Si el backlog esta atado a un provider unico, deshabilitalo temporalmente via el
   scheduler de replicacion (documentado en `pin-registry-ops.md`) y envia un nuevo
   manifest forzando a los otros providers a refrescar el alias.
3. Cuando la frescura del alias es mas critica que la paridad de replicacion, re-vincula el
   alias a un manifest caliente ya staged (`docs-preview`), luego publica un manifest de
   seguimiento una vez que SRE limpie el backlog.

### Recuperacion y cierre

1. Monitorea `torii_sorafs_replication_sla_total{outcome="missed"}` para asegurar que el
   conteo se estabiliza.
2. Captura la salida de `sorafs_cli manifest status` como evidencia de que cada replica esta
   de nuevo en cumplimiento.
3. Abre o actualiza el post-mortem del backlog de replicacion con siguientes pasos
   (escalado de providers, tuning del chunker, etc.).

## Runbook - Caida de analitica o telemetria

### Condiciones de disparo

- `npm run probe:portal` tiene exito pero los dashboards dejan de ingerir eventos de
  `AnalyticsTracker` por >15 minutos.
- Privacy review detecta un aumento inesperado en eventos descartados.
- `npm run probe:tryit-proxy` falla en paths `/probe/analytics`.

### Respuesta

1. Verifica inputs de build: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` en el artefacto del release (`build/release.json`).
2. Re-ejecuta `npm run probe:portal` con `DOCS_ANALYTICS_ENDPOINT` apuntando al
   collector de staging para confirmar que el tracker sigue emitiendo payloads.
3. Si los collectors estan caidos, setea `DOCS_ANALYTICS_ENDPOINT=""` y rebuild
   para que el tracker haga short-circuit; registra la ventana de outage en la
   linea de tiempo del incidente.
4. Valida que `scripts/check-links.mjs` siga fingerprinting `checksums.sha256`
   (las caidas de analitica *no* deben bloquear la validacion del sitemap).
5. Cuando el collector se recupere, corre `npm run test:widgets` para ejecutar los
   unit tests del helper de analitica antes de republish.

### Post-incidente

1. Actualiza [`devportal/observability`](./observability) con nuevas limitaciones del
   collector o requisitos de muestreo.
2. Emite aviso de gobernanza si se perdieron o se redactoron datos de analitica fuera
   de politica.

## Drills trimestrales de resiliencia

Ejecuta ambos drills durante el **primer martes de cada trimestre** (Ene/Abr/Jul/Oct)
o inmediatamente despues de cualquier cambio mayor de infraestructura. Guarda artefactos bajo
`artifacts/devportal/drills/<YYYYMMDD>/`.

| Drill | Pasos | Evidencia |
| ----- | ----- | -------- |
| Ensayo de rollback de alias | 1. Repetir el rollback de "Despliegue fallido" usando el manifest de produccion mas reciente.<br/>2. Re-vincular a produccion una vez que los probes pasen.<br/>3. Registrar `portal.manifest.submit.summary.json` y logs de probes en la carpeta del drill. | `rollback.submit.json`, salida de probes, y release tag del ensayo. |
| Auditoria de validacion sintetica | 1. Ejecutar `npm run probe:portal` y `npm run probe:tryit-proxy` contra produccion y staging.<br/>2. Ejecutar `npm run check:links` y archivar `build/link-report.json`.<br/>3. Adjuntar screenshots/exports de paneles Grafana confirmando el exito del probe. | Logs de probe + `link-report.json` referenciando el fingerprint del manifest. |

Escala los drills perdidos al manager de Docs/DevRel y a la revision de gobernanza de SRE,
ya que el roadmap exige evidencia trimestral determinista de que el rollback de alias y los
probes del portal siguen saludables.

## Coordinacion de PagerDuty y on-call

- El servicio PagerDuty **Docs Portal Publishing** es dueno de las alertas generadas desde
  `dashboards/grafana/docs_portal.json`. Las reglas `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry` paginan al primary de Docs/DevRel
  con Storage SRE como secundario.
- Cuando se page, incluye el `DOCS_RELEASE_TAG`, adjunta screenshots de los paneles Grafana
  afectados y enlaza la salida de probe/link-check en las notas del incidente antes de
  iniciar mitigacion.
- Despues de la mitigacion (rollback o redeploy), re-ejecuta `npm run probe:portal`,
  `npm run check:links`, y captura snapshots Grafana frescos mostrando las metricas
  de nuevo dentro de umbrales. Adjunta toda la evidencia al incidente de PagerDuty
  antes de resolverlo.
- Si dos alertas disparan al mismo tiempo (por ejemplo expiracion TLS mas backlog), triage
  refusals primero (detener publishing), ejecuta el procedimiento de rollback, luego
  limpia items de TLS/backlog con Storage SRE en el bridge.
