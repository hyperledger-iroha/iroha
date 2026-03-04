---
lang: es
direction: ltr
source: docs/source/sorafs/provider_advert_rollout.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 72ed439c516b4ca532754b2ff44f44d9a51f155c5d11d5824bb5a6f5e23ca6f2
source_last_modified: "2025-11-22T07:05:40.516582+00:00"
translation_last_reviewed: "2026-01-30"
---

# Plan de rollout de Provider Advert SoraFS

Este plan coordina el cut-over desde adverts permisivos hacia la superficie
`ProviderAdvertV1` plenamente gobernada requerida para retrieval de chunks
multi-source. Se enfoca en tres entregables:

- **Guia de operador.** Acciones paso a paso que los providers de storage deben
  completar antes de cada gate.
- **Cobertura de telemetria.** Dashboards y alertas que Observability y Ops usan
  para confirmar que la red solo acepta adverts compatibles.

El rollout se alinea con los hitos SF-2b/2c en el
[roadmap de migracion SoraFS](migration_roadmap.md) y asume que la politica de
admision en [`provider_admission_policy.md`](provider_admission_policy.md) ya
esta en efecto.

## Requisitos actuales

SoraFS acepta solo payloads `ProviderAdvertV1` con sobre de governance. Se
aplican los siguientes requisitos en admision:

- `profile_id=sorafs.sf1@1.0.0` con `profile_aliases` canonicos presentes.
- Payloads de capacidad `chunk_range_fetch` incluidos para retrieval multi-source.
- `signature_strict=true` con firmas del council adjuntas al sobre del advert.
- `allow_unknown_capabilities` solo se permite durante drills GREASE explicitos
  y debe registrarse.

## Checklist de operador

1. **Inventariar adverts.** Listar cada advert publicado y registrar:
   - Ruta del sobre de governance (`defaults/nexus/sorafs_admission/...` o equivalente en produccion).
   - `profile_id` y `profile_aliases` del advert.
   - Lista de capacidades (se espera al menos `torii_gateway` y `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (requerido cuando hay TLVs vendor-reserved).
2. **Regenerar con tooling del provider.**
   - Rebuild del payload con tu publisher de provider advert, asegurando:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` con `max_span` definido
     - `allow_unknown_capabilities=<true|false>` cuando haya TLVs GREASE
   - Validar via `/v1/sorafs/providers` y `sorafs_fetch`; warnings por
     capabilities desconocidas deben triagearse.
3. **Validar readiness multi-source.**
   - Ejecutar `sorafs_fetch` con `--provider-advert=<path>`; el CLI ahora falla
     cuando falta `chunk_range_fetch` e imprime warnings por capabilities
     desconocidas ignoradas. Capturar el reporte JSON y archivarlo con logs de
     operaciones.
4. **Stagear renovaciones.**
   - Enviar sobres `ProviderAdmissionRenewalV1` al menos 30 dias antes de la
     expiracion. Las renovaciones deben retener el handle canonico y el set de
     capacidades; solo deben cambiar stake, endpoints o metadata.
5. **Comunicar a equipos dependientes.**
   - Owners de SDK deben liberar versiones que expongan warnings a operadores
     cuando adverts se rechazan.
   - DevRel anuncia cada transicion de fase; incluir links de dashboards y la
     logica de thresholds abajo.
6. **Instalar dashboards y alertas.**
   - Importar el export Grafana y colocarlo bajo **SoraFS / Provider Rollout**
     con dashboard UID `sorafs-provider-admission`.
   - Asegurar que las reglas de alerta apunten al canal de notificaciones
     compartido `sorafs-advert-rollout` en staging y produccion.

## Telemetria y dashboards

Las siguientes metricas ya se exponen via `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — cuenta outcomes aceptados,
  rechazados y warning. Razones incluyen `missing_envelope`, `unknown_capability`,
  `stale` y `policy_violation`.

Export Grafana: [`docs/source/grafana_sorafs_admission.json`](../grafana_sorafs_admission.json).
Importa el archivo al repo compartido de dashboards (`observability/dashboards`)
y actualiza solo el UID de datasource antes de publicar.

El board publica bajo el folder Grafana **SoraFS / Provider Rollout** con el
UID estable `sorafs-provider-admission`. Las reglas de alerta
`sorafs-admission-warn` (warning) y `sorafs-admission-reject` (critical) estan
preconfiguradas para usar la politica de notificacion `sorafs-advert-rollout`;
ajusta ese contact point si cambia la lista destino en lugar de editar el JSON
 del dashboard.

Paneles Grafana recomendados:

| Panel | Query | Notas |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Grafico apilado para visualizar accept vs warn vs reject. Alertar cuando warn > 0.05 * total (warning) o reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Serie unica que alimenta el umbral del pager (5% warning rate rolling 15 minutos). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Conduce el triage del runbook; adjuntar links a pasos de mitigacion. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica providers perdiendo el deadline de refresh; cross-reference con logs de cache discovery. |

Artefactos CLI para dashboards manuales:

- `sorafs_fetch --provider-metrics-out` escribe contadores `failures`, `successes`
  y `disabled` por provider. Importar en dashboards ad-hoc para monitorear
  dry-runs del orquestador antes de cambiar providers de produccion.
- Los campos `chunk_retry_rate` y `provider_failure_rate` del reporte JSON
  destacan sintomas de throttling o payloads obsoletos que suelen preceder
  rechazos de admision.

### Layout de dashboard Grafana

Observability publica un board dedicado — **SoraFS Provider Admission Rollout**
(`sorafs-provider-admission`) — bajo **SoraFS / Provider Rollout** con los
siguientes IDs de panel canonicos:

- Panel 1 — *Admission outcome rate* (stacked area, unidad "ops/min").
- Panel 2 — *Warning ratio* (serie unica), emitiendo la expresion
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (time series agrupada por `reason`), ordenada por
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat), espejando la query de la tabla anterior y
  anotada con los deadlines de refresh del advert tomados del migration ledger.

Copiar (o crear) el JSON skeleton en el repo de dashboards de infraestructura en
`observability/dashboards/sorafs_provider_admission.json`, luego actualizar solo
el UID de datasource; los IDs de panel y reglas de alerta son referenciados por
los runbooks abajo, por lo que evita renumerarlos sin revisar esta doc.

Por conveniencia el repo ahora incluye un dashboard de referencia en
`docs/source/grafana_sorafs_admission.json`; copialo en tu folder Grafana si
necesitas un punto de partida para pruebas locales.

### Reglas de alerta Prometheus

Agregar el siguiente grupo de reglas a
`observability/prometheus/sorafs_admission.rules.yml` (crear el archivo si es el
primer grupo de reglas SoraFS) e incluirlo desde tu configuracion Prometheus.
Reemplaza `<pagerduty>` con la etiqueta de ruteo real para tu rotacion on-call.

```yaml
groups:
  - name: torii_sorafs_admission
    rules:
      - alert: SorafsProviderAdvertWarnFlood
        expr: sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
              sum(rate(torii_sorafs_admission_total[5m])) > 0.05
        for: 15m
        labels:
          severity: warning
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts generating warnings"
          description: |
            Warn outcomes exceeded 5% of all admissions for 15 minutes.
            Inspect panel 3 on the sorafs/provider-admission dashboard and
            coordinate advert rotation with the affected operator.
      - alert: SorafsProviderAdvertReject
        expr: increase(torii_sorafs_admission_total{result="reject"}[5m]) > 0
        for: 5m
        labels:
          severity: critical
          route: <pagerduty>
        annotations:
          summary: "SoraFS provider adverts rejected"
          description: |
            Provider adverts have been rejected for the last five minutes.
            Check panel 4 (rejection reasons) and rotate envelopes before
            the refresh deadline elapses.
```

Ejecutar `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de enviar cambios para asegurar que la sintaxis pasa `promtool check rules`.

## Outcomes de admision

- Capacidad `chunk_range_fetch` faltante -> rechazo con `reason="missing_capability"`.
- TLVs de capability desconocidas sin `allow_unknown_capabilities=true` -> rechazo
  con `reason="unknown_capability"`.
- `signature_strict=false` -> rechazo (reservado para diagnosticos aislados).
- `refresh_deadline` expirado -> rechazo.

## Comunicacion y manejo de incidentes

- **Mailer semanal de status.** DevRel circula un resumen breve de metricas de
  admision, warnings pendientes y deadlines proximos.
- **Respuesta a incidentes.** Si disparan alertas `reject`, on-call engineers:
  1. Obtienen el advert ofensivo via discovery Torii (`/v1/sorafs/providers`).
  2. Re-ejecutan validacion de advert en el pipeline del provider y comparan con
     `/v1/sorafs/providers` para reproducir el error.
  3. Coordinar con el provider para rotar el advert antes del proximo deadline
     de refresh.
- **Congelamientos de cambios.** Ningun cambio de schema de capabilities aterriza
  durante R1/R2 salvo que el comite de rollout apruebe; los trials GREASE deben
  programarse durante la ventana semanal de mantenimiento y registrarse en el
  migration ledger.

## Referencias

- [Protocolo SoraFS Node/Client](../sorafs_node_client_protocol.md)
- [Politica de admision de providers](provider_admission_policy.md)
- [Migration Roadmap](migration_roadmap.md)
- [Extensiones multi-source de Provider Advert](provider_advert_multisource.md)
