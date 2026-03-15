---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-rollout.fr.md
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

Este plan coordina la base de los anuncios permitidos de los proveedores frente a la superficie.
entièrement gouvernée `ProviderAdvertV1` requisito para la recuperación de trozos
multifuente. Il se concentre sur trois livrables:

- **Guía del operador.** Acciones que hacen los proveedores de almacenamiento
  Terminar delante de la puerta del chaque.
- **Cobertura de televisión.** Paneles de control y alertas de observación y operaciones.
  utilisent pour confirmer que le réseau n'accepte que des adverts conformes.
- **Calendario de implementación.** Fechas explícitas para rechazar los sobres.

El despliegue se alinea en los jalones SF-2b/2c de la
[hoja de ruta de migración SoraFS](./migration-roadmap) et supuesto que la política
admisión du [política de admisión del proveedor](./provider-admission-policy) est déjà en
vigoroso.

## Cronología de fases

| Fase | Ventana (cible) | Comportamiento | Operador de acciones | Observabilidad del foco |
|-------|-----------------|-----------|------------------|-------------------|

## Operador de lista de verificación1. **Inventario de anuncios.** Listar cada anuncio publicado y registrar:
   - Chemin de l'envelope gouvernant (`defaults/nexus/sorafs_admission/...` o equivalente en producción).
   - `profile_id` e `profile_aliases` del anuncio.
   - Lista de capacidades (al menos `torii_gateway` y `chunk_range_fetch`).
   - Bandera `allow_unknown_capabilities` (requisito cuando los TLV reservados del proveedor están presentes).
2. **Regénérer con el proveedor de herramientas.**
   - Reconstruya la carga útil con su anuncio del editor del proveedor, con garantía:
     - `profile_id=sorafs.sf1@1.0.0`
     - `capability=chunk_range_fetch` con un `max_span` definido
     - `allow_unknown_capabilities=<true|false>` cuando hay TLV GREASE presentes
   - Validador vía `/v2/sorafs/providers` y `sorafs_fetch`; les advertencias sobre des
     capacidades inconnues doivent être triés.
3. **Validar la preparación de múltiples fuentes.**
   - Ejecutor `sorafs_fetch` con `--provider-advert=<path>`; El CLI se hace eco
     Desorma cuando `chunk_range_fetch` muestra y muestra advertencias para
     capacidades inconnues ignoradas. Capturar la relación JSON y archivar con
     les logs d'operations.
4. **Preparar los cambios.**
   - Soumettre des sobres `ProviderAdmissionRenewalV1` al menos 30 días antes
     Puerta de enlace de cumplimiento (R2). Les renouvellements doivent conserver le handle
     canónica y el conjunto de capacidades; solos la estaca, los puntos finales o
     el cambiador de metadatos.
5. **Comunicador con los equipos dependientes.**- Los propietarios del SDK deben publicar las versiones que exponen las advertencias auxiliares.
     Los operadores cuando los anuncios son rechazados.
   - DevRel anuncia cada transición de fase; incluir los gravámenes de los tableros
     et la logique de seuil ci-dessous.
6. **Paneles de instalación y alertas.**
   - Importador de exportación Grafana y placer **SoraFS / Proveedor
     Implementación** con el UID `sorafs-provider-admission`.
   - Asegúrese de que las reglas de alerta apuntan hacia el canal compartido
     `sorafs-advert-rollout` en puesta en escena y producción.

## Telemétrie y paneles de control

Las siguientes medidas están expuestas a través de `iroha_telemetry`:

- `torii_sorafs_admission_total{result,reason}` — cuenta las aceptadas, rechazadas
  y anuncios. Las razones incluyen `missing_envelope`, `unknown_capability`,
  `stale` y `policy_violation`.

Exportar Grafana: [`docs/source/grafana_sorafs_admission.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/grafana_sorafs_admission.json).
Importe el archivo del repositorio compartido de paneles (`observability/dashboards`)
Y guarde cada día el UID de la fuente de datos antes de su publicación.

El tablero está publicado en el expediente Grafana **SoraFS / Implementación del proveedor** con
El UID estable `sorafs-provider-admission`. Las reglas de alerta
`sorafs-admission-warn` (advertencia) y `sorafs-admission-reject` (crítico) sont
preconfigurados para utilizar la política de notificación `sorafs-advert-rollout`;
ajuste este punto de contacto si la lista de destinos cambia el plutôt que d'éditer
el JSON del tablero.

Paneles Grafana recomendados:| Paneles | Consulta | Notas |
|-------|-------|-------|
| **Tasa de resultados de admisión** | `sum by(result)(rate(torii_sorafs_admission_total[5m]))` | Gráfico de pila para visualizador aceptar, advertir y rechazar. Alerta cuando warn > 0.05 * total (advertencia) o rechazo > 0 (crítico). |
| **Proporción de advertencia** | `sum(rate(torii_sorafs_admission_total{result="warn"}[5m])) / sum(rate(torii_sorafs_admission_total[5m]))` | Serie temporal en línea única que alimenta el buscapersonas (advertencia de taxi 5 % cada 15 minutos). |
| **Motivos del rechazo** | `sum by(reason)(rate(torii_sorafs_admission_total{result="reject"}[5m]))` | Guía del triaje del runbook; adjuntar des gravámenes vers les étapes de mitigation. |
| **Actualizar deuda** | `sum(rate(torii_sorafs_admission_total{reason="stale"}[1h]))` | Indique des proveedores qui ratent la fecha límite de actualización; croiser con los registros del caché de descubrimiento. |

Artefactos CLI para paneles manuales:

- `sorafs_fetch --provider-metrics-out` escrito de los ordenadores `failures`, `successes` y
  `disabled` por proveedor. Importar paneles de control ad hoc para vigilancia
  les dry-runs d'orchestrator avant de basculer les proveedores en producción.
- Los campos `chunk_retry_rate` y `provider_failure_rate` de la relación JSON
  Evitar estrangulamiento o síntomas de cargas útiles obsoletas que preceden
  recuerde los rechazos de admisión.

### Colocar en la página del tablero Grafana

Observabilité publie un board dédié — **SoraFS Admisión del proveedor
Lanzamiento** (`sorafs-provider-admission`) — sous **SoraFS / Lanzamiento del proveedor**
con los ID del panel canónico siguiente:- Panel 1 — *Tasa de resultados de admisión* (área apilada, unidad "ops/min").
- Panel 2 — *Relación de advertencia* (serie única), émettant l'expression
  `suma(tasa(torii_sorafs_admission_total{result="warn"}[5m])) /
   suma(tasa(torii_sorafs_admission_total[5m]))`.
- Panel 3 — *Motivos de rechazo* (series temporales reagrupadas por `reason`), triées par
  `rate(...[5m])`.
- Panel 4 — *Actualizar deuda* (estadística), reprenant la query du tableau ci-dessus et
  Annote con las fechas límite de actualización de los anuncios adicionales del libro mayor de migración.

Copie (o cree) el esqueleto JSON en el repositorio de paneles de infrarrojos
`observability/dashboards/sorafs_provider_admission.json`, después de la actualización
único UID de la fuente de datos; Los ID del panel y las reglas de alerta son
Referencias de los runbooks ci-dessous, évitez donc de les renuméroter sans
mettre à jour esta documentación.

Para mayor comodidad, el repositorio proporciona una definición de tablero de referencia
`docs/source/grafana_sorafs_admission.json`; Copiar la en su expediente Grafana
Si tienes un punto de partida para las pruebas locales, necesitas un punto de partida.

### Reglas de alerta Prometheus

Agregar el grupo de reglas siguientes
`observability/prometheus/sorafs_admission.rules.yml` (créez le fichier si c'est
le premier groupe de règles SoraFS) et incluez-le dans votre configuración
Prometheus. Reemplace `<pagerduty>` por la etiqueta de ruta real para usted
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
```Ejecute `scripts/check_prometheus_rules.sh observability/prometheus/sorafs_admission.rules.yml`
antes de introducir los cambios para verificar que la sintaxis pasó
`promtool check rules`.

## Matriz de despliegue

| Características del anuncio | R0 | R1 | R2 | R3 |
|--------------------------------|----|----|----|----|
| `profile_id = sorafs.sf1@1.0.0`, `chunk_range_fetch` presente, alias canoniques, `signature_strict=true` | ✅ | ✅ | ✅ | ✅ |
| Ausencia de capacidad `chunk_range_fetch` | ⚠️ Avisar (ingesta + telemetría) | ⚠️ Advertir | ❌ Rechazar (`reason="missing_capability"`) | ❌ Rechazar |
| TLV de capacidad inconnue sin `allow_unknown_capabilities=true` | ✅ | ⚠️ Avisar (`reason="unknown_capability"`) | ❌ Rechazar | ❌ Rechazar |
| `refresh_deadline` caducado | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar | ❌ Rechazar |
| `signature_strict=false` (diagnóstico de accesorios) | ✅ (desarrollo único) | ⚠️ Advertir | ⚠️ Advertir | ❌ Rechazar |

Todas las horas que utilizan UTC. Las fechas de aplicación son reflétées dans le
libro mayor de migración et ne bougeront pas sans vote du Council; todo cambio
Requiert la mise à jour de ce fichier et du ledger dans la même PR.

> **Nota de implementación:** R1 introduce la serie `result="warn"` en
> `torii_sorafs_admission_total`. Le patch d'ingestion Torii qui ajoute le nouveau
> la etiqueta se coloca junto con las pastillas de télémétrie SF-2; jusque-là, utilisez le

## Comunicación y gestión de incidentes- **Mailer hebdomadaire de statut.** DevRel difunde un breve currículum vitae des métriques
  d'admisión, des advertencias en curso y des plazos à venir.
- **Respuesta del incidente.** Si las alertas `reject` se debilitan, el guardia :
  1. Recupere el anuncio fautif vía descubrimiento Torii (`/v2/sorafs/providers`).
  2. Relance la validación del anuncio en el proveedor de canalización y compare con
     `/v2/sorafs/providers` para reproducir el error.
  3. Coordonne con el proveedor para hacer el tourner l'advert avant la prochaine
     fecha límite de actualización.
- **Gel de cambios.** Aucune modificación del esquema de capacidades colgante
  R1/R2 a menos que el comité de implementación no sea válido; les essais GREASE doivent
  être planifiés durant la fenêtre de mantenimiento hebdomadaire et periodisés
  en el libro mayor de migración.

## Referencias

- [SoraFS Protocolo de cliente/nodo](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/sorafs_node_client_protocol.md)
- [Política de admisión de proveedores](./provider-admission-policy)
- [Hoja de ruta de migración](./migration-roadmap)
- [Extensiones de múltiples fuentes de anuncios de proveedores](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)