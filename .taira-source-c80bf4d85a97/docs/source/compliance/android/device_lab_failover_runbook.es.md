---
lang: es
direction: ltr
source: docs/source/compliance/android/device_lab_failover_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 473b2b49d32c32d2b884b670ba35e9aa3d0606cfd451d441a7ca927c1160311d
source_last_modified: "2026-01-03T18:07:59.262670+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Runbook de perforación de conmutación por error del laboratorio de dispositivos Android (AND6/AND7)

Este runbook captura el procedimiento, los requisitos de evidencia y la matriz de contactos.
utilizado al ejercer el **plan de contingencia dispositivo-laboratorio** al que se hace referencia en
`roadmap.md` (§“Aprobaciones reglamentarias de artefactos y contingencias de laboratorio”). Se complementa
el flujo de trabajo de reserva (`device_lab_reservation.md`) y el registro de incidentes
(`device_lab_contingency.md`) por lo que revisores de cumplimiento, asesoría legal y SRE
tener una única fuente de verdad sobre cómo validamos la preparación para la conmutación por error.

## Propósito y cadencia

- Demostrar que los grupos de dispositivos generales de Android StrongBox + pueden realizar conmutación por error
  a los carriles de píxeles alternativos, al grupo compartido, a la cola de ráfagas de Firebase Test Lab y
  Retenedor externo StrongBox sin faltar SLA AND6/AND7.
- Producir un paquete de evidencia que el departamento legal pueda adjuntar a las presentaciones de ETSI/FISC.
  antes de la revisión de cumplimiento de febrero.
- Ejecutar al menos una vez por trimestre, además cada vez que cambie la lista de hardware del laboratorio.
  (dispositivos nuevos, baja o mantenimiento superior a 24h).

| ID de perforación | Fecha | Escenario | Paquete de pruebas | Estado |
|----------|------|----------|-----------------|--------|
| DR-2026-02-Q1 | 2026-02-20 | Corte de carril simulado de Pixel8Pro + acumulación de certificaciones con ensayo de telemetría AND7 | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ✅ Completado: hashes del paquete registrados en `docs/source/compliance/android/evidence_log.csv`. |
| DR-2026-05-Q2 | 2026-05-22 (programado) | Superposición de mantenimiento de StrongBox + ensayo Nexus | `artifacts/android/device_lab_contingency/20260522-failover-drill/` *(pendiente)* — El billete `_android-device-lab` **AND6-DR-202605** mantiene las reservas; El paquete se completará después de la perforación. | 🗓 Programado: bloque de calendario agregado al “Laboratorio de dispositivos Android – Reservas” por cadencia AND6. |

## Procedimiento

### 1. Preparación previa a la perforación

1. Confirme la capacidad inicial en `docs/source/sdk/android/android_strongbox_capture_status.md`.
2. Exporte el calendario de reservas para la semana ISO objetivo a través de
   `python3 scripts/android_device_lab_export.py --week <ISO week>`.
3. Presentar ticket `_android-device-lab`
   `AND6-DR-<YYYYMM>` con alcance (“perforación de conmutación por error”), ranuras planificadas y afectados
   cargas de trabajo (certificación, humo de CI, caos de telemetría).
4. Actualice la plantilla de registro de contingencia en `device_lab_contingency.md` con un
   fila de marcador de posición para la fecha de perforación.

### 2. Simular condiciones de falla

1. Deshabilite o elimine el carril principal (`pixel8pro-strongbox-a`) dentro del laboratorio.
   programador y etiquete la entrada de la reserva como "simulacro".
2. Activar una alerta de interrupción simulada en PagerDuty (servicio `AND6-device-lab`) y
   capturar la exportación de notificaciones para el paquete de pruebas.
3. Anotar los trabajos de Buildkite que normalmente consumen el carril.
   (`android-strongbox-attestation`, `android-ci-e2e`) con el ID del taladro.

### 3. Ejecución de conmutación por error1. Ascienda el carril alternativo de Pixel7 al objetivo de CI principal y programe el
   cargas de trabajo planificadas en su contra.
2. Active el conjunto de ráfagas Firebase Test Lab a través del carril `firebase-burst` para
   las pruebas de humo de las billeteras minoristas mientras que la cobertura de StrongBox se traslada a las compartidas
   carril. Capture la invocación de CLI (o exportación de consola) en el ticket para auditoría
   paridad.
3. Coloque el retenedor de laboratorio externo StrongBox para realizar un breve barrido de certificación;
   registre el reconocimiento del contacto como se describe a continuación.
4. Registre todos los ID de ejecución de Buildkite, las URL de trabajos de Firebase y las transcripciones de anticipos en
   el ticket `_android-device-lab` y el manifiesto del paquete de pruebas.

### 4. Validación y reversión

1. Comparar los tiempos de ejecución de atestación/CI con la línea de base; deltas de bandera >10% al
   Líder de laboratorio de hardware.
2. Restaure el carril principal y actualice la instantánea de capacidad más la preparación
   matriz una vez que pasa la validación.
3. Agregue la última fila a `device_lab_contingency.md` con disparador, acciones,
   y seguimientos.
4. Actualice `docs/source/compliance/android/evidence_log.csv` con:
   ruta del paquete, manifiesto SHA-256, ID de ejecución de Buildkite, hash de exportación de PagerDuty y
   aprobación del revisor.

## Diseño del paquete de pruebas

| Archivo | Descripción |
|------|-------------|
| `README.md` | Resumen (ID del simulacro, alcance, propietarios, cronograma). |
| `bundle-manifest.json` | Mapa SHA-256 para cada archivo del paquete. |
| `calendar-export.{ics,json}` | Calendario de reservas de semanas ISO desde el script de exportación. |
| `pagerduty/incident_<id>.json` | Exportación de incidentes de PagerDuty que muestra el cronograma de alerta y confirmación. |
| `buildkite/<job>.txt` | Buildkite ejecuta URL y registros para los trabajos afectados. |
| `firebase/burst_report.json` | Resumen de ejecución de ráfagas de Firebase Test Lab. |
| `retainer/acknowledgement.eml` | Confirmación del laboratorio externo StrongBox. |
| `photos/` | Fotografías/capturas de pantalla opcionales de la topología del laboratorio si se volvió a cablear el hardware. |

Guarde el paquete en
`artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` y registrar
la suma de verificación del manifiesto dentro del registro de evidencia más la lista de verificación de cumplimiento AND6.

## Matriz de contacto y escalamiento

| Rol | Contacto principal | Canal(es) | Notas |
|------|-----------------|------------|-------|
| Líder de laboratorio de hardware | Priya Ramanathan | `@android-lab` Holgura · +81-3-5550-1234 | Posee acciones en sitio y actualizaciones de calendario. |
| Operaciones de laboratorio de dispositivos | Mateo Cruz | Cola `_android-device-lab` | Coordina boletos de reserva + carga de paquetes. |
| Ingeniería de lanzamiento | Alexéi Morózov | Liberación de holgura en inglés · `release-eng@iroha.org` | Valida la evidencia de Buildkite + publica hashes. |
| Laboratorio de caja fuerte externo | Instrumentos Sakura NOC | `noc@sakura.example` · +81-3-5550-9876 | Contacto de retención; confirmar disponibilidad en 6h. |
| Coordinador de ráfagas de Firebase | Tessa Wright | `@android-ci` Holgura | Activa la automatización de Firebase Test Lab cuando se necesita un respaldo. |

Escale en el siguiente orden si un simulacro descubre problemas de bloqueo:
1. Líder del laboratorio de hardware
2. Fundamentos de Android TL
3. Líder del programa/Ingeniería de lanzamiento
4. Líder de Cumplimiento + Asesor Legal (si el simulacro revela riesgo regulatorio)

## Informes y seguimientos- Vincular este runbook junto con el procedimiento de reserva siempre que se haga referencia
  preparación para conmutación por error en `roadmap.md`, `status.md` y paquetes de gobierno.
- Envíe por correo electrónico el resumen trimestral del simulacro a Cumplimiento + Legal con el paquete de evidencia
  tabla hash y adjunte la exportación del ticket `_android-device-lab`.
- Reflejar métricas clave (tiempo hasta la conmutación por error, cargas de trabajo restauradas, acciones pendientes)
  dentro de `status.md` y el rastreador de lista activa AND7 para que los revisores puedan rastrear el
  dependencia de un ensayo concreto.