<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan de despliegue de adverts de proveedores SoraFS

Este plan coordina el cut-over desde adverts permisivos de providers hacia la
superficie gobernada `ProviderAdvertV1` requerida para la recuperacion multi-origen
de chunks. Se centra en tres deliverables:

- **Guia de operadores.** Acciones paso a paso que los storage providers deben
  completar antes de cada gate.
- **Cobertura de telemetria.** Dashboards y alerts que Observabilidad y Ops usan
  para confirmar que la red solo acepta adverts compatibles.
  para que los equipos de SDK y tooling planifiquen sus releases.

El rollout se alinea con los hitos SF-2b/2c del
[roadmap de migracion de SoraFS](./migration-roadmap) y asume que la politica de
admision del [provider admission policy](./provider-admission-policy) ya esta en
vigor.

## Cronograma de fases

| Fase | Ventana (objetivo) | Comportamiento | Acciones del operador | Enfoque de observabilidad |
|-------|-----------------|-----------|------------------|-------------------|

## Checklist de operadores

1. **Inventariar adverts.** Lista cada advert publicado y registra:
   - Ruta del governing envelope (`defaults/nexus/sorafs_admission/...` o equivalente en produccion).
   - `profile_id` y `profile_aliases` del advert.
   - Lista de capabilities (se espera al menos `torii_gateway` y `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (requerido cuando hay TLVs reservados por vendor).
2. **Regenerar con tooling de providers.**
   - Reconstruye el payload con tu publisher de provider advert, asegurando:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` con un `max_span` definido
     - `allow_unknown_capabilities=<true|false>` cuando haya TLVs GREASE
   - Valida via `/v1/sorafs/providers` y `sorafs_fetch`; las advertencias sobre
     capabilities desconocidas deben ser triageadas.
3. **Validar readiness multi-origen.**
   - Ejecuta `sorafs_fetch` con `--provider-advert=<path>`; el CLI ahora falla
     cuando falta `chunk_range_fetch` y muestra advertencias para capabilities
     desconocidas ignoradas. Captura el reporte JSON y archivelo con los logs
     de operaciones.
4. **Preparar renovaciones.**
   - Envia envelopes `ProviderAdmissionRenewalV1` al menos 30 dias antes de
     enforcement en gateway (R2). Las renovaciones deben conservar el handle
     canonico y el set de capabilities; solo stake, endpoints o metadata deben
     cambiar.
5. **Comunicar a los equipos dependientes.**
   - Los owners de SDK deben liberar versiones que expongan advertencias a los
     operadores cuando los adverts sean rechazados.
   - DevRel anuncia cada transicion de fase; incluir enlaces a dashboards y la
     logica de umbral de abajo.
6. **Instalar dashboards y alerts.**
   - Importa el export de Grafana y colocalo bajo **SoraFS / Provider
     Rollout** con el UID `sorafs-provider-admission`.
   - Asegura que las reglas de alertas apunten al canal compartido
     `sorafs-advert-rollout` en staging y produccion.

## Telemetria y dashboards

Las siguientes metricas ya estan expuestas via `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — cuenta aceptados, rechazados
  y resultados con advertencias. Las razones incluyen `missing_envelope`, `unknown_capability`,
  `stale` y `policy_violation`.

Export de Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importa el archivo en el repositorio compartido de dashboards (`observability/dashboards`)
y actualiza solo el UID del datasource antes de publicar.

El tablero se publica bajo la carpeta de Grafana **SoraFS / Provider Rollout** con
el UID estable `sorafs-provider-admission`. Las reglas de alertas
`sorafs-admission-warn` (warning) y `sorafs-admission-reject` (critical) estan
preconfiguradas para usar la politica de notificacion `sorafs-advert-rollout`;
ajusta ese contact point si la lista de destino cambia en lugar de editar el
JSON del dashboard.

Paneles Grafana recomendados:

| Panel | Query | Notes |
|-------|-------|-------|
| **Admission outcome rate** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Stack chart para visualizar accept vs warn vs reject. Alerta cuando warn > 0.05 * total (warning) o reject > 0 (critical). |
| **Warning ratio** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Timeseries de una sola linea que alimenta el umbral del pager (5% warning rate rolling 15 minutos). |
| **Rejection reasons** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guia el triage del runbook; adjunta enlaces a pasos de mitigacion. |
| **Refresh debt** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica providers que no cumplen el refresh deadline; cruza con logs de discovery cache. |

Artefactos de CLI para dashboards manuales:

- `sorafs_fetch --provider-metrics-out` escribe contadores `failures`, `successes` y
  `disabled` por provider. Importa en dashboards ad-hoc para monitorear
  dry-runs de orchestrator antes de cambiar providers en produccion.
- Los campos `chunk_retry_rate` y `provider_failure_rate` del reporte JSON
  resaltan throttling o sintomas de payloads stale que suelen preceder rechazos
  de admision.

### Layout del dashboard de Grafana

Observability publica un board dedicado — **SoraFS Provider Admission
Rollout** (`sorafs-provider-admission`) — bajo **SoraFS / Provider Rollout**
con los siguientes IDs canonicos de panel:

- Panel 1 — *Admission outcome rate* (stacked area, unidad "ops/min").
- Panel 2 — *Warning ratio* (single series), emitiendo la expresion
  `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) /
   sum(rate(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Rejection reasons* (series de tiempo agrupada por `reason`), ordenada por
  `rate(...[5m])`.
- Panel 4 — *Refresh debt* (stat), reflejando la query de la tabla anterior y
  anotada con los refresh deadlines de adverts extraidos del migration ledger.

Copia (o crea) el esqueleto JSON en el repo de dashboards de infraestructura en
`observability/dashboards/sorafs_provider_admission.json`, luego actualiza solo el
UID del datasource; los IDs de panel y las reglas de alertas se referencian en
los runbooks de abajo, asi que evita renumerarlos sin revisar esta documentacion.

Para conveniencia el repositorio ya incluye una definicion de dashboard de
referencia en `docs/source/grafana_sorafs_admission.json`; copiala en tu carpeta
Grafana si necesitas un punto de partida para pruebas locales.

### Reglas de alertas de Prometheus

Agrega el siguiente grupo de reglas a
`observability/prometheus/sorafs_admission.rules.yml` (crea el archivo si este es
el primer grupo de reglas SoraFS) e incluyelo desde tu configuracion de
Prometheus. Reemplaza `<pagerduty>` con la etiqueta de enrutamiento real para tu
rotacion on-call.

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

Ejecuta `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de subir cambios para asegurar que la sintaxis pase `promtool check rules`.

## Matriz de rollout

| Caracteristicas del advert | R0 | R1 | R2 | R3 |
|------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, aliases canonicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Carece de la capability `chunk_range_fetch` | ⚠️ Warn (ingest + telemetry) | ⚠️ Warn | ❌ Reject (`reason="missing_capability"`) | ❌ Reject |
| TLVs de capability desconocida sin `allow_unknown_capabilities=true` | ✅ | ⚠️ Warn (`reason="unknown_capability"`) | ❌ Reject | ❌ Reject |
| `refresh_deadline` expirado | ❌ Reject | ❌ Reject | ❌ Reject | ❌ Reject |
| `signature_strict=false` (fixtures de diagnostico) | ✅ (solo desarrollo) | ⚠️ Warn | ⚠️ Warn | ❌ Reject |

Todos los horarios usan UTC. Las fechas de enforcement se reflejan en el migration
ledger y no se moveran sin un voto del council; cualquier cambio requiere actualizar
este archivo y el ledger en el mismo PR.

> **Nota de implementacion:** R1 introduce la serie `result="warn"` en
> `torii_sorafs_admission_total`. El patch de ingesta Torii que agrega la nueva
> etiqueta se sigue junto con las tareas de telemetria SF-2; hasta que llegue,

## Comunicacion y manejo de incidentes

- **Mailer semanal de estado.** DevRel circula un resumen breve de metricas de
  admision, advertencias pendientes y deadlines proximos.
- **Respuesta a incidentes.** Si los alerts `reject` se activan, on-call:
  1. Recupera el advert ofensivo via discovery de Torii (`/v1/sorafs/providers`).
  2. Re-ejecuta la validacion del advert en el pipeline del provider y compara con
     `/v1/sorafs/providers` para reproducir el error.
  3. Coordina con el provider la rotacion del advert antes del siguiente refresh
     deadline.
- **Congelamientos de cambios.** No se aplican cambios de schema de capabilities
  durante R1/R2 a menos que el comite de rollout lo apruebe; los trials GREASE deben
  programarse durante la ventana semanal de mantenimiento y registrarse en el
  migration ledger.

## Referencias

- [SoraFS Node/Client Protocol](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Provider Admission Policy](./provider-admission-policy)
- [Migration Roadmap](./migration-roadmap)
- [Provider Advert Multi-Source Extensions](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)
