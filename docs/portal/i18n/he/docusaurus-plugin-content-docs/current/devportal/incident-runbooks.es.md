---
lang: he
direction: rtl
source: docs/portal/docs/devportal/incident-runbooks.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# ספרי הפעלה ותרגילים להחזרה

## הצעה

El item del roadmap **DOCS-9** exige playbooks accionables mas un plan de practica para que
los operadores del portal puedan recuperarse de fallas de entrega sin adivinar. Esta Nota
cubre tres incidentes de alta senal: despliegues fallidos, degradacion de replicacion y
caidas de analitica, y documenta los drills trimestrales que prueban que el rollback de
כינוי y la validacion sintetica siguen funcionando מקצה לקצה.

### רלאציונדו חומרי

- [`devportal/deploy-guide`](./deploy-guide) - flujo de packaging, חתימה y promotion de alias.
- [`devportal/observability`](./observability) - תגיות שחרור, אנליטיקה ובדיקות רפרנסיות.
- `docs/source/sorafs_node_client_protocol.md`
  y [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops)
  - telemetria del registro y umbrales de escalamiento.
- `docs/portal/scripts/sorafs-pin-release.sh` y עוזרי `npm run probe:*`
  referenciados en los checklists.

### רכיבי טלמטריה ומכשירים

| סנאל / כלי | פרופוזיטו |
| ------------- | ------- |
| `torii_sorafs_replication_sla_total` (נפגש/הוחמצה/בהמתנה) | Detecta bloqueos de replicacion y brechas de SLA. |
| `torii_sorafs_replication_backlog_total`, `torii_sorafs_replication_completion_latency_epochs` | Cuantifica la profundidad del backlog y la latencia de completado para el triage. |
| `torii_sorafs_gateway_refusals_total`, `torii_sorafs_manifest_submit_total{status="error"}` | Muestra fallas del gateway que a menudo seuen un deployment defectuoso. |
| `npm run probe:portal` / `npm run probe:tryit-proxy` | Probes sinteticos que gatean משחרר y validan rollbacks. |
| `npm run check:links` | Gate de enlaces rotos; se usa despues de cada mitigacion. |
| `sorafs_cli manifest submit ... --alias-*` (USado por `scripts/sorafs-pin-release.sh`) | Mecanismo de promocion/reversion de alias. |
| לוח `Docs Portal Publishing` Grafana (`dashboards/grafana/docs_portal.json`) | Agrega telemetria de refusals/כינוי/TLS/replicasion. Alertas de PagerDuty referencian estos paneles como evidencia. |

## Runbook - Despliegue fallido o artefacto incorrecto

### תנאי ביטול

- Fallan los probes de preview/production (`npm run probe:portal -- --expect-release=...`).
- Alertas Grafana en `torii_sorafs_gateway_refusals_total` o
  `torii_sorafs_manifest_submit_total{status="error"}` דוחה את פתיחת ההשקה.
- QA ידני detecta rutas rotas o fallas del proxy נסה זאת במיידי
  קידום מכירות.

### Contencion inmediata

1. **Congelar despliegues:** marca el pipeline CI con `DEPLOY_FREEZE=1` (קלט זרימת עבודה de
   GitHub) o pausa el job de Jenkins para que no salgan mas artefactos.
2. ** חפצי קפטור:** הורד את `build/checksums.sha256`,
   `portal.manifest*.{json,to,bundle,sig}`, y la salida de probes del build fallido para que
   el rollback referencie los digests exactos.
3. **מודיעים לבעלי עניין:** אחסון SRE, lead de Docs/DevRel, y el oficial de guardia de
   גוברננסה למודעות (especialmente cuando `docs.sora` esta impactado).

### הליך החזרה לאחור

1. Identifica el manifest ultimo conocido bueno (LKG). זרימת העבודה של ייצור los guarda
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

3. רשם את קורות החיים של החזרה לאחור
   manifest LKG y del manifest fallido.

### תוקף1. `npm run probe:portal -- --expect-release=${LKG_TAG}`.
2. `npm run check:links`.
3. `sorafs_cli manifest verify-signature ...` y `sorafs_cli proof verify ...`
   (ver la guia de despliegue) para confirmar que el manifest re-promocionado sigue
   coincidendo con el CAR archivado.
4. `npm run probe:tryit-proxy` para asegurar que el proxy Try-It staging regreso.

### לאחר האירוע

1. Rehabilita el pipeline de despliegue solo despues de entender la causa raiz.
2. Rellena entradas de "Lessons learned" en [`devportal/deploy-guide`](./deploy-guide)
   con nuevas notas, si aplica.
3. Abre defects para el suite de pruebas fallidas (בדיקה, בודק קישורים וכו').

## Runbook - Degradacion de replicacion

### תנאי ביטול

- התראה: `sum(torii_sorafs_replication_sla_total{outcome="met"}) /
  clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) <
  0.95' ל-10 דקות.
- `torii_sorafs_replication_backlog_total > 10` ל-10 דקות (גרסה
  `pin-registry-ops.md`).
- Gobernanza reporta disponibilidad lenta del alias despues de un release.

### טריאז'

1. Inspecciona Dashboards de [`sorafs/pin-registry-ops`](../sorafs/pin-registry-ops) para
   confirmar si el backlog esta localizado en una clase de storage o en un flet de providers.
2. Cruza logs de Torii para warnings `sorafs_registry::submit_manifest` para determinar si
   לאס הגשות estan fallando.
3. Muestrea salud de replicas דרך `sorafs_cli manifest status --manifest ...` (ליסט תוצאות
   de replicacion por ספק).

### הקלה

1. Reemite el Manifest con Mayor conteo de replicas (`--pin-min-replicas 7`) usando
   `scripts/sorafs-pin-release.sh` para que el scheduler distribuya carga en un set ראש העיר
   דה ספקים. Registra el nuevo digest en el log del incidente.
2. Si el backlog esta atado a un provider unico, deshabilitalo temporalmente via el
   לוח זמנים להעתקה (מסמכים בכתובת `pin-registry-ops.md`) y envia un nuevo
   manifest forzando a los otros ספקים a refrescar el alias.
3. Cuando la frescura del alias es mas critica que la paridad de replicacion, re-vincula el
   כינוי un Manifest caliente ya מבוים (`docs-preview`), luego publica un manifest de
   זה לא יכול להיות SRE לימפה את הצטברות.

### Recuperacion y cierre

1. Monitorea `torii_sorafs_replication_sla_total{outcome="missed"}` para asegurar que el
   conteo se estabiliza.
2. Captura la salida de `sorafs_cli manifest status` como evidencia de que cada replica esta
   de nuevo en cumplimiento.
3. לאחר הנתיחה שלאחר המוות של צבר העתקות לאחר המוות
   (escalado de providers, tuning del chunker וכו').

## Runbook - Caida de analitica o telemetria

### תנאי ביטול

- `npm run probe:portal` יש יציאה מלוחות המחוונים dejan de ingerir eventos de
  `AnalyticsTracker` ל->15 דקות.
- סקירת פרטיות זיהתה את ההזדהות באירועים.
- `npm run probe:tryit-proxy` falla en paths `/probe/analytics`.

### תשובה1. Verifica inputs de build: `DOCS_ANALYTICS_ENDPOINT` y
   `DOCS_ANALYTICS_SAMPLE_RATE` en el artefacto del release (`build/release.json`).
2. Re-ejecuta `npm run probe:portal` con `DOCS_ANALYTICS_ENDPOINT` apuntando al
   אספן דה בימוי עבור אישורים que el tracker סיוe emitiendo payloads.
3. Si los collectors estan caidos, setea `DOCS_ANALYTICS_ENDPOINT=""` ובנייה מחדש
   para que el tracker haga קצר חשמלי; registra la ventana de outage en la
   linea de tiempo del incidente.
4. Valida que `scripts/check-links.mjs` siga טביעת אצבע `checksums.sha256`
   (las caidas de analitica *no* deben bloquear la validacion del sitemap).
5. Cuando el collector se recupere, corre `npm run test:widgets` para ejecutar los
   בדיקות יחידה של עוזר דה אנליטיקה לפני פרסום מחדש.

### לאחר האירוע

1. Actualiza [`devportal/observability`](./observability) con nuevas limitaciones del
   אספן או דרישות מוסטראו.
2. Emite aviso de gobernanza si se perdieron o se redactoron datos de analitica fuera
   דה פוליטיקה.

## Drills trimestrales de resiliencia

Ejecuta ambos drills durante el **primer martes de cada trimestre** (Ene/Abr/Jul/Oct)
o inmediatamente despues de cualquier cambio mayor de infraestructura. Guarda artefactos bajo
`artifacts/devportal/drills/<YYYYMMDD>/`.

| מקדחה | פאסוס | Evidencia |
| ----- | ----- | -------- |
| Ensayo de rollback de alias | 1. חזור על החזרה לאחור של "Despliegue fallido" usando el manifest de produccion mas reciente.<br/>2. Re-vincular a produccion una vez que los probes pasen.<br/>3. הרשם `portal.manifest.submit.summary.json` y logs de probes en la carpeta del drill. | `rollback.submit.json`, salida de probes, y release tag del ensayo. |
| Auditoria de validacion sintetica | 1. Ejecutar `npm run probe:portal` y `npm run probe:tryit-proxy` נגד הייצור.<br/>2. Ejecutar `npm run check:links` y archivar `build/link-report.json`.<br/>3. צילומי מסך משלימים/ייצוא של פאנלים Grafana מאשר את היציאה מהבדיקה. | יומני בדיקה + `link-report.json` מפנה אל טביעת האצבע של המניפסט. |

Escala los drills perdidos al manager de Docs/DevRel y a la revision de gobernanza de SRE,
ya que el roadmap exige evidencia trimestral determinista de que el rollback de alias y los
בדיקות של הפורטל סיואן ברכות.

## Coordinacion de PagerDuty y כוננות- El servicio PagerDuty **Docs Portal Publishing** es dueno de las alertas generadas desde
  `dashboards/grafana/docs_portal.json`. Las reglas `DocsPortal/GatewayRefusals`,
  `DocsPortal/AliasCache`, y `DocsPortal/TLSExpiry` העמוד הראשי של Docs/DevRel
  con Storage SRE como secundario.
- דף זה כולל, כולל `DOCS_RELEASE_TAG`, צילומי מסך משלבים Grafana
  afectados y enlaza la salida de probe/link-check en las notas del incidente antes de
  הקלה מוקדמת.
- Despues de la mitigacion (החזרה או פריסה מחדש), הוצא מחדש `npm run probe:portal`,
  `npm run check:links`, y captura תמונות Grafana ציורי קיר מוסטרנדו לאס מטריקס
  de nuevo dentro de umbrales. Adjunta toda la Evidencia al incidente de PagerDuty
  antes de resolverlo.
- Si dos alertas disparan al mismo tiempo (לפי אימפלו תפוגת TLS mas backlog), טריאז'
  סירובים primero (פרסום מעכב), ejecuta el procedimiento de rollback, luego
  לימפיה פריטים de TLS/backlog con Storage SRE en el bridge.