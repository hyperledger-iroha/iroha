---
id: kpi-dashboard
lang: es
direction: ltr
source: docs/portal/docs/sns/kpi-dashboard.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
# Panel KPI del Servicio de Nombres Sora

El panel de KPI ofrece a stewards, guardians y reguladores un solo lugar para revisar senales de adopcion, errores e ingresos antes de la cadencia mensual del anexo (SN-8a). La definicion de Grafana se incluye en el repositorio en `dashboards/grafana/sns_suffix_analytics.json` y el portal refleja los mismos paneles mediante un iframe incrustado para que la experiencia coincida con la instancia interna de Grafana.

## Filtros y fuentes de datos

- **Filtro de sufijo** - impulsa las consultas `sns_registrar_status_total{suffix}` para que `.sora`, `.nexus` y `.dao` se inspeccionen de forma independiente.
- **Filtro de liberacion masiva** - acota las metricas `sns_bulk_release_payment_*` para que finanzas pueda conciliar un manifiesto de registrador especifico.
- **Metricas** - extrae de Torii (`sns_registrar_status_total`, `torii_request_duration_seconds`), de la CLI de guardian (`guardian_freeze_active`), `sns_governance_activation_total`, y las metricas del helper de onboarding masivo.

## Paneles

1. **Registros (ultimas 24h)** - numero de eventos de registrador exitosos para el sufijo seleccionado.
2. **Activaciones de gobernanza (30d)** - mociones de carta/enmienda registradas por la CLI.
3. **Throughput del registrador** - tasa por sufijo de acciones exitosas del registrador.
4. **Modos de error del registrador** - tasa de 5 minutos de contadores `sns_registrar_status_total` etiquetados como error.
5. **Ventanas de congelacion de guardian** - selectores activos donde `guardian_freeze_active` reporta un ticket de congelacion abierto.
6. **Unidades netas de pago por activo** - totales reportados por `sns_bulk_release_payment_net_units` por activo.
7. **Solicitudes masivas por sufijo** - volumenes de manifiestos por id de sufijo.
8. **Unidades netas por solicitud** - calculo tipo ARPU derivado de las metricas de release.

## Checklist mensual de revision de KPI

El lider de finanzas conduce una revision recurrente el primer martes de cada mes:

1. Abre la pagina del portal **Analytics -> SNS KPI** (o el dashboard de Grafana `sns-kpis`).
2. Captura una exportacion PDF/CSV de las tablas de throughput del registrador y de ingresos.
3. Compara sufijos para detectar incumplimientos de SLA (picos de tasa de error, selectores congelados >72 h, diferencias de ARPU >10 %).
4. Registra resumenes + acciones en la entrada de anexo correspondiente bajo `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
5. Adjunta los artefactos exportados del dashboard al commit del anexo y enlazalos en la agenda del consejo.

Si la revision detecta incumplimientos de SLA, abre un incidente de PagerDuty para el propietario afectado (registrar duty manager, guardian on-call o steward program lead) y rastrea la remediacion en el log del anexo.
