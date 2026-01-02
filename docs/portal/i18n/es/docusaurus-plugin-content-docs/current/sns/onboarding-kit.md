---
lang: es
direction: ltr
source: docs/portal/docs/sns/onboarding-kit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Metricas SNS y kit de onboarding

El item de roadmap **SN-8** agrupa dos compromisos:

1. Publicar dashboards que expongan registros, renovaciones, ARPU, disputas y ventanas de freeze para `.sora`, `.nexus`, y `.dao`.
2. Entregar un kit de onboarding para que registrars y stewards conecten DNS, pricing y APIs de forma consistente antes de que cualquier sufijo salga en vivo.

Esta pagina refleja la version fuente
[`docs/source/sns/onboarding_kit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/onboarding_kit.md)
para que revisores externos sigan el mismo procedimiento.

## 1. Paquete de metricas

### Dashboard de Grafana y embed del portal

- Importa `dashboards/grafana/sns_suffix_analytics.json` en Grafana (o en otro host de analitica)
  via la API estandar:

```bash
curl -H "Content-Type: application/json"          -H "Authorization: Bearer ${GRAFANA_TOKEN}"          -X POST https://grafana.sora.net/api/dashboards/db          --data-binary @dashboards/grafana/sns_suffix_analytics.json
```

- El mismo JSON alimenta el iframe de esta pagina del portal (ver **SNS KPI Dashboard**).
  Cada vez que actualices el dashboard, ejecuta
  `npm run build && npm run serve-verified-preview` dentro de `docs/portal` para
  confirmar que Grafana y el embed permanecen sincronizados.

### Paneles y evidencia

| Panel | Metricas | Evidencia de gobernanza |
|-------|---------|-------------------------|
| Registros y renovaciones | `sns_registrar_status_total` (success + renewal resolver labels) | Throughput por sufijo + seguimiento de SLA. |
| ARPU / unidades netas | `sns_bulk_release_payment_net_units`, `sns_bulk_release_payment_gross_units` | Finanzas puede empatar manifests del registrar con ingresos. |
| Disputas y freezes | `guardian_freeze_active`, `sns_dispute_outcome_total`, `sns_governance_activation_total` | Muestra freezes activos, cadencia de arbitraje y carga del guardian. |
| SLA / tasas de error | `torii_request_duration_seconds`, `sns_registrar_status_total{status="error"}` | Destaca regresiones de API antes de impactar clientes. |
| Seguimiento de manifest bulk | `sns_bulk_release_manifest_total`, metricas de pago con labels `manifest_id` | Conecta drops CSV con tickets de settlement. |

Exporta un PDF/CSV desde Grafana (o el iframe embebido) durante la revision mensual de KPI
y adjuntalo a la entrada de anexo correspondiente bajo
`docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Los stewards tambien capturan el SHA-256
del bundle exportado bajo `docs/source/sns/reports/` (por ejemplo,
`steward_scorecard_2026q1.md`) para que las auditorias puedan reproducir la ruta de evidencia.

### Automatizacion de anexos

Genera archivos de anexo directamente desde la exportacion del dashboard para que los
revisores obtengan un resumen consistente:

```bash
cargo xtask sns-annex       --suffix .sora       --cycle 2026-03       --dashboard dashboards/grafana/sns_suffix_analytics.json       --dashboard-artifact artifacts/sns/regulatory/.sora/2026-03/sns_suffix_analytics.json       --output docs/source/sns/reports/.sora/2026-03.md       --regulatory-entry docs/source/sns/regulatory/eu-dsa/2026-03.md       --portal-entry docs/portal/docs/sns/regulatory/eu-dsa-2026-03.md
```

- El helper hashea la exportacion, captura el UID/tags/numero de paneles, y escribe
  un anexo Markdown bajo `docs/source/sns/reports/.<suffix>/<cycle>.md` (ver el ejemplo
  `.sora/2026-03` junto a este doc).
- `--dashboard-artifact` copia la exportacion en
  `artifacts/sns/regulatory/<suffix>/<cycle>/` para que el anexo apunte a la ruta
  de evidencia canonica; usa `--dashboard-label` solo si necesitas apuntar a
  un archivo fuera de banda.
- `--regulatory-entry` apunta al memo regulatorio. El helper inserta (o reemplaza)
  un bloque `KPI Dashboard Annex` que registra la ruta del anexo, el artefacto del
  dashboard, el digest y el timestamp para mantener la evidencia sincronizada.
- `--portal-entry` mantiene alineada la copia de Docusaurus
  (`docs/portal/docs/sns/regulatory/*.md`) para que los revisores no tengan que
  comparar resutidos de anexos separados manualmente.
- Si omites `--regulatory-entry`/`--portal-entry`, adjunta el archivo generado a los
  memos manualmente y aun sube los snapshots PDF/CSV capturados desde Grafana.
- Para exportaciones recurrentes, lista los pares sufijo/ciclo en
  `docs/source/sns/regulatory/annex_jobs.json` y ejecuta
  `python3 scripts/run_sns_annex_jobs.py --verbose`. El helper recorre cada entrada,
  copia la exportacion del dashboard (por defecto `dashboards/grafana/sns_suffix_analytics.json`
  cuando no se especifica), y refresca el bloque de anexo dentro de cada memo regulatorio
  (y, cuando existe, memo del portal) en una sola pasada.
- Ejecuta `python3 scripts/check_sns_annex_schedule.py --jobs docs/source/sns/regulatory/annex_jobs.json --regulatory-root docs/source/sns/regulatory --report-root docs/source/sns/reports` (o `make check-sns-annex`) para probar que la lista de trabajos permanezca ordenada/sin duplicados, que cada memo tenga el marcador `sns-annex`, y que exista el stub de anexo. El helper escribe `artifacts/sns/annex_schedule_summary.json` junto a los resumentes de locale/hash usados en paquetes de gobernanza.
Esto elimina pasos manuales de copiar/pegar y mantiene la evidencia del anexo SN-8 consistente mientras protege contra drift de agenda, marcador y localizacion en CI.

## 2. Componentes del kit de onboarding

### Cableado de sufijo

- Esquema del registro + reglas de selector:
  [`docs/source/sns/registry_schema.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registry_schema.md)
  y [`docs/source/sns/local_to_global_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/local_to_global_toolkit.md).
- Helper de esqueleto DNS:
  [`scripts/sns_zonefile_skeleton.py`](https://github.com/hyperledger-iroha/iroha/blob/master/scripts/sns_zonefile_skeleton.py)
  con el flujo de ensayo documentado en el
  [runbook de gateway/DNS](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_owner_runbook.md).
- Para cada lanzamiento de registrar, archiva una nota corta bajo
  `docs/source/sns/reports/` con muestras de selector, pruebas GAR y hashes DNS.

### Cheatsheet de precios

| Longitud del label | Tarifa base (USD equiv) |
|--------------|---------------------|
| 3 | $240 |
| 4 | $90 |
| 5 | $30 |
| 6-9 | $12 |
| 10+ | $8 |

Coeficientes de sufijo: `.sora` = 1.0x, `.nexus` = 0.8x, `.dao` = 1.3x.  
Multiplicadores de termino: 2-year -5%, 5-year -12%; grace window = 30 days, redemption
= 60 days (20% fee, min $5, max $200). Registra desviaciones negociadas en el
ticket del registrar.

### Subastas premium vs renovaciones

1. **Pool premium** -- commit/reveal con oferta sellada (SN-3). Registra las pujas con
   `sns_premium_commit_total`, y publica el manifest bajo
   `docs/source/sns/reports/`.
2. **Dutch reopen** -- despues de que expiren grace + redemption, inicia una venta Dutch de 7-day
   a 10x que decae 15% por dia. Etiqueta manifests con `manifest_id` para que el
   dashboard pueda mostrar el progreso.
3. **Renovaciones** -- monitorea `sns_registrar_status_total{resolver="renewal"}` y
   captura el checklist de autorenew (notificaciones, SLA, rails de pago de fallback)
   dentro del ticket del registrar.

### APIs de desarrollador y automatizacion

- Contratos de API: [`docs/source/sns/registrar_api.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/registrar_api.md).
- Helper bulk y esquema CSV:
  [`docs/source/sns/bulk_onboarding_toolkit.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sns/bulk_onboarding_toolkit.md).
- Comando de ejemplo:

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv       --ndjson artifacts/sns/releases/2026q2/requests.ndjson       --submission-log artifacts/sns/releases/2026q2/submissions.log       --submit-torii-url https://torii.sora.net       --submit-token-file ~/.config/sora/tokens/registrar.token
```

Incluye el manifest ID (salida de `--submission-log`) en el filtro del dashboard KPI
para que finanzas pueda reconciliar los paneles de ingreso por release.

### Paquete de evidencia

1. Ticket del registrar con contactos, alcance del sufijo y rails de pago.
2. Evidencia DNS/resolver (zonefile skeletons + pruebas GAR).
3. Hoja de precios + overrides aprobados por gobernanza.
4. Artefactos de smoke-test de API/CLI (ejemplos `curl`, transcripts de CLI).
5. Screenshot del dashboard KPI + exportacion CSV, adjunto al anexo mensual.

## 3. Checklist de lanzamiento

| Paso | Responsable | Artefacto |
|------|-------|----------|
| Dashboard importado | Product Analytics | Respuesta de la API de Grafana + dashboard UID |
| Embed del portal validado | Docs/DevRel | logs de `npm run build` + screenshot de preview |
| Ensayo DNS completo | Networking/Ops | salidas de `sns_zonefile_skeleton.py` + log del runbook |
| Dry run de automatizacion del registrar | Registrar Eng | log de submissions de `sns_bulk_onboard.py` |
| Evidencia de gobernanza archivada | Governance Council | link del anexo + SHA-256 del dashboard exportado |

Completa la checklist antes de activar un registrar o un sufijo. El bundle firmado
despeja el gate del roadmap SN-8 y entrega a los auditores una sola referencia al
revisar lanzamientos del marketplace.
