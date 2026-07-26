---
lang: es
direction: ltr
source: docs/source/compliance/android/device_lab_reservation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 05dc578338882ddfcdf2410b0643774ceb8212f28739ba94ac83edf087b9b5dc
source_last_modified: "2026-01-03T18:07:59.245516+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Procedimiento de reserva de laboratorio de dispositivos Android (AND6/AND7)

Este manual describe cómo el equipo de Android reserva, confirma y audita el dispositivo.
tiempo de laboratorio para los hitos **AND6** (CI y refuerzo de cumplimiento) y **AND7**
(preparación para la observabilidad). Complementa el registro de contingencias
`docs/source/compliance/android/device_lab_contingency.md` asegurando capacidad
En primer lugar, se evitan los déficits.

## 1. Objetivos y alcance

- Mantener los grupos de dispositivos generales de StrongBox + por encima del 80% exigido por la hoja de ruta
  objetivo de capacidad durante las ventanas de congelación.
- Proporcionar un calendario determinista para que la CI, los barridos de certificación y el caos
  los ensayos nunca compiten por el mismo hardware.
- Capture un rastro auditable (solicitudes, aprobaciones, notas posteriores a la ejecución) que alimente
  la lista de verificación de cumplimiento AND6 y el registro de evidencia.

Este procedimiento cubre los carriles de Pixel dedicados, el grupo de respaldo compartido y
el retenedor de laboratorio externo StrongBox al que se hace referencia en la hoja de ruta. Emulador ad hoc
el uso está fuera de alcance.

## 2. Ventanas de reserva

| Piscina/carril | Ferretería | Longitud de ranura predeterminada | Plazo de entrega de reserva | Propietario |
|-------------|----------|---------------------|-------------------|-------|
| `pixel8pro-strongbox-a` | Pixel8Pro (caja fuerte) | 4h | 3 días hábiles | Líder de laboratorio de hardware |
| `pixel8a-ci-b` | Pixel8a (CI general) | 2h | 2 días hábiles | Fundamentos de Android TL |
| `pixel7-fallback` | Piscina compartida Pixel7 | 2h | 1 día hábil | Ingeniería de lanzamiento |
| `firebase-burst` | Cola de humo del laboratorio de pruebas de Firebase | 1h | 1 día hábil | Fundamentos de Android TL |
| `strongbox-external` | Retenedor de laboratorio externo StrongBox | 8h | 7 días naturales | Líder de programa |

Las plazas se reservan en UTC; las reservas superpuestas requieren aprobación explícita
del líder del laboratorio de hardware.

## 3. Solicitar flujo de trabajo

1. **Preparar el contexto**
   - Actualización `docs/source/sdk/android/android_strongbox_device_matrix.md` con
     Los dispositivos que planeas hacer ejercicio y la etiqueta de preparación.
     (`attestation`, `ci`, `chaos`, `partner`).
   - Recopile la última instantánea de capacidad de
     `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. **Enviar solicitud**
   - Presente un ticket en la cola `_android-device-lab` usando la plantilla en
     `docs/examples/android_device_lab_request.md` (propietario, fechas, cargas de trabajo,
     requisito de reserva).
   - Adjunte cualquier dependencia regulatoria (por ejemplo, barrido de atestación AND6, AND7
     ejercicio de telemetría) y enlace a la entrada correspondiente de la hoja de ruta.
3. **Aprobación**
   - El líder del laboratorio de hardware revisa dentro de un día hábil y confirma el lugar en el
     calendario compartido (`Android Device Lab – Reservations`) y actualiza el
     columna `device_lab_capacity_pct` en
     `docs/source/compliance/android/evidence_log.csv`.
4. **Ejecución**
   - Ejecutar los trabajos programados; Registre los ID de ejecución de Buildkite o los registros de herramientas.
   - Anotar cualquier desviación (cambios de hardware, excesos).
5. **Cierre**
   - Comentar el ticket con artefactos/enlaces.
   - Si la ejecución estuvo relacionada con el cumplimiento, actualice
     `docs/source/compliance/android/and6_compliance_checklist.md` y agregue una fila
     a `evidence_log.csv`.

Las solicitudes que afectan las demostraciones de socios (AND8) deben enviarse en copia de Partner Engineering.

## 4. Cambio y cancelación- **Reprogramar:** volver a abrir el ticket original, proponer un nuevo horario y actualizar el
  entrada de calendario. Si la nueva ranura está dentro de las 24 horas, haga ping al líder del laboratorio de hardware + SRE
  directamente.
- **Cancelación de emergencia:** sigue el plan de contingencia
  (`device_lab_contingency.md`) y registre las filas de activación/acción/seguimiento.
- **Excesos:** si una carrera excede su espacio en >15 minutos, publique una actualización y confirme
  si la próxima reserva puede proceder; de lo contrario, entregarlo al respaldo
  piscina o carril de ráfaga de Firebase.

## 5. Evidencia y auditoría

| Artefacto | Ubicación | Notas |
|----------|----------|-------|
| Reserva de billetes | Cola `_android-device-lab` (Jira) | Exportar resumen semanal; vincular los ID de los tickets en el registro de pruebas. |
| Exportación de calendario | `artifacts/android/device_lab/<YYYY-WW>-calendar.{ics,json}` | Ejecute `scripts/android_device_lab_export.py --ics-url <calendar_ics_feed>` todos los viernes; el asistente guarda el archivo `.ics` filtrado más un resumen JSON para la semana ISO para que las auditorías puedan adjuntar ambos artefactos sin descargas manuales. |
| Instantáneas de capacidad | `docs/source/compliance/android/evidence_log.csv` | Actualización después de cada reserva/cierre. |
| Notas posteriores a la ejecución | `docs/source/compliance/android/device_lab_contingency.md` (si es contingencia) o comentario de ticket | Requerido para auditorías. |

Durante las revisiones de cumplimiento trimestrales, adjunte la exportación del calendario, el resumen del ticket,
y extracto del registro de evidencia del envío de la lista de verificación AND6.

### Automatización de exportación de calendario

1. Obtenga la URL del feed de ICS (o descargue un archivo `.ics`) para "Android Device Lab - Reservas".
2. Ejecutar

   ```bash
   python3 scripts/android_device_lab_export.py \
     --ics-url "https://calendar.example/ical/export" \
     --week <ISO week, defaults to current>
   ```

   El script escribe tanto `artifacts/android/device_lab/<YYYY-WW>-calendar.ics`
   e `...-calendar.json`, capturando la semana ISO seleccionada.
3. Cargue los archivos generados con el paquete de evidencia semanal y haga referencia al
   Resumen JSON en `docs/source/compliance/android/evidence_log.csv` cuando
   capacidad del laboratorio del dispositivo de registro.

## 6. Escalera de escalada

1. Líder del laboratorio de hardware (principal)
2. Fundamentos de Android TL
3. Líder de programa/Ingeniería de lanzamiento (para ventanas congeladas)
4. Contacto de laboratorio externo de StrongBox (cuando se invoca el retenedor)

Las escaladas deben registrarse en el ticket y reflejarse en el Android semanal.
correo de estado.

## 7. Documentos relacionados

- `docs/source/compliance/android/device_lab_contingency.md` — registro de incidentes para
  déficits de capacidad.
- `docs/source/compliance/android/and6_compliance_checklist.md` — maestro
  lista de verificación de entregables.
- `docs/source/sdk/android/android_strongbox_device_matrix.md` — hardware
  rastreador de cobertura.
- `docs/source/sdk/android/android_strongbox_attestation_run_log.md` —
  Evidencia de certificación StrongBox a la que hace referencia AND6/AND7.

Mantener este procedimiento de reserva satisface el elemento de acción de la hoja de ruta “definir
procedimiento de reserva de laboratorio de dispositivos” y mantiene los artefactos de cumplimiento de cara a los socios
en sincronización con el resto del plan de preparación de Android.

## 8. Procedimiento y contactos de simulacro de conmutación por error

El elemento AND6 de la hoja de ruta también requiere un ensayo de conmutación por error trimestral. El completo,
instrucciones paso a paso en vivo en
`docs/source/compliance/android/device_lab_failover_runbook.md`, pero el alto
El flujo de trabajo de nivel se resume a continuación para que los solicitantes puedan planificar simulacros junto con
reservas de rutina.1. **Programar el simulacro:** Bloquear los carriles afectados (`pixel8pro-strongbox-a`,
   grupo de reserva, `firebase-burst`, retenedor de StrongBox externo) en el compartido
   calendario y cola `_android-device-lab` al menos 7 días antes del simulacro.
2. **Simular interrupción:** Desagrupar el carril principal, activar PagerDuty
   (`AND6-device-lab`) incidente y anotar los trabajos de Buildkite dependientes con
   el ID del taladro anotado en el runbook.
3. **Conmutación por error:** Promocionar el carril alternativo de Pixel7, iniciar la ráfaga de Firebase
   suite e interactúe con el socio externo de StrongBox en un plazo de 6 horas. Capturar
   URL de ejecución de Buildkite, exportaciones de Firebase y reconocimientos de retención.
4. **Validar y restaurar:** Verificar los tiempos de ejecución de atestación + CI, restablecer el
   carriles originales y actualice `device_lab_contingency.md` más el registro de evidencia
   con la ruta del paquete + sumas de verificación.

### Referencia de contacto y derivación

| Rol | Contacto principal | Canal(es) | Orden de escalada |
|------|-----------------|------------|------------------|
| Líder de laboratorio de hardware | Priya Ramanathan | `@android-lab` Holgura · +81-3-5550-1234 | 1 |
| Operaciones de laboratorio de dispositivos | Mateo Cruz | Cola `_android-device-lab` | 2 |
| Fundamentos de Android TL | Elena Vorobeva | `@android-foundations` Holgura | 3 |
| Ingeniería de lanzamiento | Alexéi Morózov | `release-eng@iroha.org` | 4 |
| Laboratorio de caja fuerte externo | Instrumentos Sakura NOC | `noc@sakura.example` · +81-3-5550-9876 | 5 |

Incrementar secuencialmente si el simulacro descubre problemas de bloqueo o si hay algún retroceso
El carril no se puede poner en línea en 30 minutos. Registre siempre la escalada
notas en el ticket `_android-device-lab` y reflejarlas en el registro de contingencia.