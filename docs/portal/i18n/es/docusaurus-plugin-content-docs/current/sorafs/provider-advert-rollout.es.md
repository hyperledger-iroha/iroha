---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: "Plan de implementación de anuncios de proveedores SoraFS"
---

> Adaptado de [`docs/source/sorafs/provider_advert_rollout.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_rollout.md).

# Plan de implementación de anuncios de proveedores SoraFS

Este plan coordina el cut-over desde anuncios permisivos de proveedores hacia la
superficie gobernada `ProviderAdvertV1` requerida para la recuperación multi-origen
de trozos. Se centra en tres entregables:

- **Guia de operadores.** Acciones paso a paso que los proveedores de almacenamiento deben
  Complete antes de cada puerta.
- **Cobertura de telemetria.** Dashboards y alertas que usan Observabilidad y Ops
  para confirmar que la red solo acepta anuncios conformes.
  para que los equipos de SDK y herramientas planifiquen sus lanzamientos.

El rollout se alinea con los hitos SF-2b/2c del
[roadmap de migracion de SoraFS](./migration-roadmap) y asume que la politica de
admision del [política de admisión del proveedor](./provider-admission-policy) ya esta en
vigor.

## Cronograma de fases

| Fase | Ventana (objetivo) | Comportamiento | Acciones del operador | Enfoque de observabilidad |
|-------|-----------------|-----------|------------------|-------------------|

## Lista de verificación de operadores1. **Inventariar anuncios.** Lista cada anuncio publicado y registro:
   - Ruta del envolvente gobernante (`defaults/nexus/sorafs_admission/...` o equivalente en producción).
   - `profile_id` y `profile_aliases` del anuncio.
   - Lista de capacidades (se espera al menos `torii_gateway` y `chunk_range_fetch`).
   - Flag `allow_unknown_capabilities` (requerido cuando hay TLVs reservados por el proveedor).
2. **Regenerar con herramientas de proveedores.**
   - Reconstruye el payload con tu editor de proveedor advert, asegurando:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` con un `max_span` definido
     - `allow_unknown_capabilities=<true|false>` cuando haya TLVs GRASA
   - Validada vía `/v2/sorafs/providers` y `sorafs_fetch`; las advertencias sobre
     capacidades desconocidas deben ser triageadas.
3. **Validar preparación multiorigen.**
   - Ejecuta `sorafs_fetch` con `--provider-advert=<path>`; el CLI ahora falla
     cuando falta `chunk_range_fetch` y muestra advertencias para capacidades
     desconocidas ignoradas. Captura el informe JSON y archivalo con los logs.
     de operaciones.
4. **Preparar renovaciones.**
   - Envia sobres `ProviderAdmissionRenewalV1` al menos 30 días antes de
     aplicación en puerta de enlace (R2). Las renovaciones deben conservar el mango.
     canónico y el conjunto de capacidades; participación individual, puntos finales o metadatos deben
     cambiar.
5. **Comunicar a los equipos dependientes.**
   - Los propietarios de SDK deben liberar versiones que expongan advertencias a losoperadores cuando los anuncios sean rechazados.
   - DevRel anuncia cada transición de fase; incluir enlaces a paneles y la
     Lógica de umbral de abajo.
6. **Instalar paneles y alertas.**
   - Importa el export de Grafana y colocalo bajo **SoraFS / Provider
     Implementación** con el UID `sorafs-provider-admission`.
   - Asegura que las reglas de alertas apunten al canal compartido
     `sorafs-advert-rollout` en puesta en escena y producción.

## Telemetría y paneles de control

Las siguientes métricas ya están expuestas vía `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — cuenta aceptada, rechazada
  y resultados con advertencias. Las razones incluyen `missing_envelope`, `unknown_capability`,
  `stale` y `policy_violation`.

Exportar de Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importa el archivo en el repositorio compartido de paneles (`observability/dashboards`)
y actualice solo el UID del origen de datos antes de publicarlo.

El tablero se publica bajo la carpeta de Grafana **SoraFS / Provider Rollout** con
el UID estable `sorafs-provider-admission`. Las reglas de alerta
`sorafs-admission-warn` (advertencia) y `sorafs-admission-reject` (crítico) están
preconfiguradas para usar la política de notificación `sorafs-advert-rollout`;
ajusta ese punto de contacto si la lista de destino cambia en lugar de editar el
JSON del panel.

Paneles Grafana recomendados:| Paneles | Consulta | Notas |
|-------|-------|-------|
| **Tasa de resultados de admisión** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pila para visualizar aceptar vs advertir vs rechazar. Alerta cuando warn > 0.05 * total (advertencia) o rechazo > 0 (crítico). |
| **Proporción de advertencia** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Serie temporal de una sola línea que alimenta el umbral del buscapersonas (tasa de advertencia del 5% durante 15 minutos). |
| **Motivos del rechazo** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guía el triaje del runbook; adjunta enlaces a pasos de mitigación. |
| **Actualizar deuda** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indica proveedores que no cumplen con el plazo de actualización; cruza con registros de caché de descubrimiento. |

Artefactos de CLI para manuales de paneles de control:

- `sorafs_fetch --provider-metrics-out` escribe contadores `failures`, `successes` y
  `disabled` por proveedor. Importa paneles de control ad-hoc para monitorear
  ensayos de orquestador antes de cambiar proveedores en producción.
- Los campos `chunk_retry_rate` y `provider_failure_rate` del informe JSON
  resaltan throttling o síntomas de cargas útiles obsoletas que suelen preceder a los rechazos
  de admisión.

### Diseño del tablero de Grafana

Observabilidad publica un board dedicado — **SoraFS Admisión del proveedor
Lanzamiento** (`sorafs-provider-admission`) — bajo **SoraFS / Lanzamiento del proveedor**
con los siguientes ID canónicos de panel:- Panel 1 — *Tasa de resultados de admisión* (área apilada, unidad "ops/min").
- Panel 2 — *Warning ratio* (serie única), emitiendo la expresión
  `suma(tasa(torii_sorafs_admission_total{result="warn"}[5m])) /
   suma(tasa(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motivos de rechazo* (series de tiempo agrupada por `reason`), ordenada por
  `rate(...[5m])`.
- Panel 4 — *Actualizar deuda* (stat), reflejando la consulta de la tabla anterior y
  anotada con los plazos de actualización de anuncios extraidos del libro mayor de migración.

Copia (o crea) el esqueleto JSON en el repositorio de paneles de infraestructura en
`observability/dashboards/sorafs_provider_admission.json`, luego actualiza solo el
UID del origen de datos; los IDs de panel y las reglas de alertas se referencian en
los runbooks de abajo, asi que evita renumerarlos sin revisar esta documentacion.

Para conveniencia el repositorio ya incluye una definición de tablero de
referencia en `docs/source/grafana_sorafs_admission.json`; copiala en tu carpeta
Grafana si necesita un punto de partida para pruebas locales.

### Reglas de alertas de Prometheus

Agrega el siguiente grupo de reglas a
`observability/prometheus/sorafs_admission.rules.yml` (crea el archivo si este es
el primer grupo de reglas SoraFS) e incluyelo desde tu configuración de
Prometheus. Reemplaza `<pagerduty>` con la etiqueta de enrutamiento real para tu
rotación de guardia.

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
```Ejecuta `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de subir cambios para asegurar que la sintaxis pase `promtool check rules`.

## Matriz de implementación

| Características del anuncio | R0 | R1 | R2 | R3 |
|--------------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, alias canónicos, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Cuidado de la capacidad `chunk_range_fetch` | ⚠️ Avisar (ingesta + telemetría) | ⚠️ Advertir | ❌ Rechazar (`reason="missing_capability"`) | ❌ Rechazar |
| TLV de capacidad desconocida sin `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rechazar | ❌ Rechazar |
| `refresh_deadline` caducado | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar |
| `signature_strict=false` (accesorios de diagnóstico) | ✅ (desarrollo en solitario) | ⚠️ Advertir | ⚠️ Advertir | ❌ Rechazar |

Todos los horarios usan UTC. Las fechas de cumplimiento se reflejan en la migración
ledger y no se moverán sin un voto del consejo; cualquier cambio requiere actualizar
este archivo y el libro mayor en el mismo PR.

> **Nota de implementación:** R1 introduce la serie `result="warn"` en
> `torii_sorafs_admission_total`. El parche de ingesta Torii que agrega la nueva
> etiqueta se sigue junto con las tareas de telemetria SF-2; hasta que llegue,

## Comunicación y manejo de incidentes- **Mailer semanal de estado.** DevRel circula un resumen breve de métricas de
  admisión, advertencias pendientes y plazos próximos.
- **Respuesta a incidentes.** Si las alertas `reject` se activan, de guardia:
  1. Recupera el anuncio ofensivo vía descubrimiento de Torii (`/v2/sorafs/providers`).
  2. Re-ejecuta la validación del anuncio en el pipeline del proveedor y compara con
     `/v2/sorafs/providers` para reproducir el error.
  3. Coordina con el proveedor la rotación del anuncio antes del siguiente refresco.
     plazo.
- **Congelamientos de cambios.** No se aplican cambios de esquema de capacidades
  durante R1/R2 a menos que el comité de rollout lo apruebe; los ensayos GREASE deben
  programarse durante la ventana semanal de mantenimiento y registrarse en el
  libro de migraciones.

## Referencias

- [SoraFS Protocolo de cliente/nodo](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de admisión de proveedores](./provider-admission-policy)
- [Hoja de ruta de migración](./migration-roadmap)
- [Extensiones de múltiples fuentes de anuncios de proveedores](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)