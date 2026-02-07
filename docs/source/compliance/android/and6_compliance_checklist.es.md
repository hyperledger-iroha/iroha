---
lang: es
direction: ltr
source: docs/source/compliance/android/and6_compliance_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2a0ce1be46f9c468915f50de5e38e2f34657b26bf4243fb5ea45dab175789393
source_last_modified: "2026-01-04T11:42:43.489571+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificación de cumplimiento de Android AND6

Esta lista de verificación rastrea los entregables de cumplimiento que marcan el hito **AND6 -
Refuerzo de CI y cumplimiento**. Consolida los artefactos regulatorios solicitados
en `roadmap.md` y define el diseño de almacenamiento en
`docs/source/compliance/android/` para ingeniería de lanzamiento, soporte y aspectos legales
Puede hacer referencia a la misma evidencia establecida antes de aprobar las versiones de Android.

## Alcance y propietarios

| Área | Entregables | Propietario principal | Copia de seguridad / Revisor |
|------|--------------|---------------|-------------|
| Paquete regulatorio de la UE | Objetivo de seguridad ETSI EN 319 401, resumen GDPR DPIA, atestación SBOM, registro de evidencia | Cumplimiento y Legal (Sofia Martins) | Ingeniería de lanzamiento (Alexei Morozov) |
| Paquete regulatorio de Japón | Lista de verificación de controles de seguridad FISC, paquetes de certificación StrongBox bilingües, registro de evidencia | Cumplimiento y Legal (Daniel Park) | Líder del programa de Android |
| Preparación del laboratorio de dispositivos | Seguimiento de capacidad, desencadenantes de contingencias, registro de escalamiento | Líder de laboratorio de hardware | Observabilidad de Android TL |

## Matriz de artefactos| Artefacto | Descripción | Ruta de almacenamiento | Actualizar cadencia | Notas |
|----------|-------------|--------------|-----------------|-------|
| Objetivo de seguridad ETSI EN 319 401 | Narrativa que describe los objetivos/supuestos de seguridad para los archivos binarios del SDK de Android. | `docs/source/compliance/android/eu/security_target.md` | Revalide cada versión de GA + LTS. | Debe citar hashes de procedencia de compilación para el tren de liberación. |
| Resumen de la DPIA del RGPD | Evaluación del impacto de la protección de datos que cubre la telemetría/registro. | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Anual + antes de cambios materiales de telemetría. | Política de redacción de referencias en `sdk/android/telemetry_redaction.md`. |
| Certificación SBOM | Procedencia de SBOM plus SLSA firmada para los artefactos Gradle/Maven. | `docs/source/compliance/android/eu/sbom_attestation.md` | Cada lanzamiento de GA. | Ejecute `scripts/android_sbom_provenance.sh <version>` para generar informes CycloneDX, paquetes de firma conjunta y sumas de verificación. |
| Lista de verificación de controles de seguridad FISC | Lista de verificación completa que asigna los controles del SDK a los requisitos de FISC. | `docs/source/compliance/android/jp/fisc_controls_checklist.md` | Anual + antes de pilotos socios de JP. | Proporcionar títulos bilingües (EN/JP). |
| Paquete de certificación StrongBox (JP) | Resumen de certificación por dispositivo + cadena para reguladores de Japón. | `docs/source/compliance/android/jp/strongbox_attestation.md` | Cuando ingresa nuevo hardware al grupo. | Señale artefactos en bruto en `artifacts/android/attestation/<device>/`. |
| Memorándum de aprobación legal | Resumen del abogado que cubre el alcance de ETSI/GDPR/FISC, la postura de privacidad y la cadena de custodia de los artefactos adjuntos. | `docs/source/compliance/android/eu/legal_signoff_memo.md` | Cada vez que cambia el paquete de artefactos o se agrega una nueva jurisdicción. | El memorando hace referencia a hashes del registro de evidencia y enlaces al paquete de contingencia dispositivo-laboratorio. |
| Registro de pruebas | Índice de artefactos enviados con metadatos hash/marca de tiempo. | `docs/source/compliance/android/evidence_log.csv` | Actualizado cada vez que cambia cualquier entrada anterior. | Agregue el enlace Buildkite + aprobación del revisor. |
| Paquete de instrumentación dispositivo-laboratorio | Evidencia de telemetría, cola y certificación de ranuras específicas registradas con el proceso definido en `device_lab_instrumentation.md`. | `artifacts/android/device_lab/<slot>/` (ver `docs/source/compliance/android/device_lab_instrumentation.md`) | Cada ranura reservada + simulacro de conmutación por error. | Capture manifiestos SHA-256 y haga referencia a la ID de la ranura en el registro de evidencia + lista de verificación. |
| Registro de reservas de laboratorio de dispositivos | Flujo de trabajo de reservas, aprobaciones, instantáneas de capacidad y escalera de escalamiento utilizados para mantener los grupos de StrongBox ≥80 % durante las congelaciones. | `docs/source/compliance/android/device_lab_reservation.md` | Actualizar cada vez que se crean/cambian reservas. | Consulte los ID de ticket `_android-device-lab` y la exportación del calendario semanal que se indican en el procedimiento. |
| Runbook y paquete de ejercicios de conmutación por error de laboratorio de dispositivos | Plan de ensayo trimestral y manifiesto de artefactos que demuestran los carriles alternativos, la cola de ráfagas de Firebase y la preparación del retenedor externo StrongBox. | `docs/source/compliance/android/device_lab_failover_runbook.md` + `artifacts/android/device_lab_contingency/<YYYYMMDD>-failover-drill/` | Trimestralmente (o después de cambios en la lista de hardware). | Registre los ID de perforación en el registro de evidencia y adjunte el hash del manifiesto + exportación de PagerDuty anotado en el runbook. |

> **Consejo:** Al adjuntar archivos PDF o artefactos firmados externamente, almacene un breve
> Contenedor de Markdown en la ruta presentada que enlaza con el artefacto inmutable en
> la participación en la gobernanza. Esto mantiene el repositorio liviano y al mismo tiempo preserva el
> pista de auditoría.

## Paquete regulatorio de la UE (ETSI/GDPR)El paquete de la UE reúne los tres artefactos anteriores más el memorando legal:

- Actualice `security_target.md` con el identificador de versión, hash de manifiesto Torii,
  y resumen SBOM para que los auditores puedan hacer coincidir los archivos binarios con el alcance declarado.
- Mantener el resumen de EIPD alineado con la última política de redacción de telemetría y
  adjunte el extracto de diferenciación de Norito al que se hace referencia en `docs/source/sdk/android/telemetry_redaction.md`.
- La entrada de certificación SBOM debe incluir: hash CycloneDX JSON, procedencia
  hash del paquete, declaración de cofirma y la URL del trabajo de Buildkite que los generó.
- `legal_signoff_memo.md` debe capturar el consejo/fecha, enumerar cada artefacto +
  SHA-256, describe los controles de compensación y vincula a la fila del registro de evidencia
  más el ID del ticket de PagerDuty que realizó el seguimiento de la aprobación.

## Paquete regulatorio de Japón (FISC/StrongBox)

Los reguladores de Japón esperan un paquete paralelo con documentación bilingüe:

- `fisc_controls_checklist.md` refleja la hoja de cálculo oficial; llenar tanto el
  columnas EN y JA y haga referencia a la sección específica de `sdk/android/security.md`
  o el paquete de atestación StrongBox que satisface cada control.
- `strongbox_attestation.md` resume las últimas ejecuciones de
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  (sobres JSON + Norito por dispositivo). Insertar enlaces a los artefactos inmutables.
  en `artifacts/android/attestation/<device>/` y observe la cadencia de rotación.
- Registre la plantilla de carta de presentación bilingüe que se envía con las presentaciones en su interior.
  `docs/source/compliance/android/jp/README.md` para que el soporte pueda reutilizarlo.
- Actualizar el registro de evidencia con una sola fila que haga referencia a la lista de verificación, el
  hash del paquete de atestación y cualquier ID de boleto del socio de JP vinculado a la entrega.

## Flujo de trabajo de envío

1. **Borrador**: el propietario prepara el artefacto, registra el nombre del archivo planificado
   la tabla anterior y abre un PR que contiene el código auxiliar de Markdown actualizado más un
   suma de comprobación del archivo adjunto externo.
2. **Revisión**: Release Engineering confirma que los hashes de procedencia coinciden con los de la etapa
   binarios; El cumplimiento verifica el lenguaje regulatorio; El soporte garantiza los SLA y
   Se hace referencia correctamente a las políticas de telemetría.
3. **Aprobación**: los aprobadores agregan sus nombres y fechas a la tabla `Sign-off`.
   abajo. El registro de evidencia se actualiza con la URL de PR y la ejecución de Buildkite.
4. **Publicar**: después de la aprobación de la gobernanza de SRE, vincule el artefacto en
   `status.md` y actualice las referencias del Playbook de soporte de Android.

### Registro de aprobación

| Artefacto | Revisado por | Fecha | Relaciones Públicas / Evidencia |
|----------|-------------|------|---------------|
| *(pendiente)* | - | - | - |

## Reserva de laboratorio de dispositivos y plan de contingencia

Para mitigar el riesgo de **disponibilidad del laboratorio de dispositivos** mencionado en la hoja de ruta:- Seguimiento de la capacidad semanal en `docs/source/compliance/android/evidence_log.csv`
  (columna `device_lab_capacity_pct`). Alertar a Ingeniería de Liberación si hay disponibilidad
  cae por debajo del 70 % durante dos semanas consecutivas.
- Reserva StrongBox/carriles generales siguiendo
  `docs/source/compliance/android/device_lab_reservation.md` por delante de todos
  congelación, ensayo o barrido de cumplimiento para que las solicitudes, aprobaciones y artefactos
  se capturan en la cola `_android-device-lab`. Vincular los ID de ticket resultantes
  en el registro de evidencia al registrar instantáneas de capacidad.
- **Grupos de reserva:** ir primero al grupo de píxeles compartido; si todavía está saturado,
  Programe pruebas de humo de Firebase Test Lab para la validación de CI.
- **Retenedor de laboratorio externo:** mantenga el retenedor con el socio StrongBox
  lab para que podamos reservar hardware durante las ventanas de congelación (anticipación mínima de 7 días).
- **Escalada:** genera el incidente `AND6-device-lab` en PagerDuty cuando tanto el
  Los grupos primarios y alternativos caen por debajo del 50 % de su capacidad. El líder del laboratorio de hardware
  coordina con la SRE para volver a priorizar los dispositivos.
- **Paquetes de pruebas de conmutación por error:** almacene cada ensayo en
  `artifacts/android/device_lab_contingency/<YYYYMMDD>/` con la reserva
  solicitud, exportación de PagerDuty, manifiesto de hardware y transcripción de recuperación. Referencia
  el paquete de `device_lab_contingency.md` y agregue el SHA-256 al registro de evidencia
  para que Legal pueda probar que se ejerció el flujo de trabajo de contingencia.
- **Ejercicios trimestrales:** ejercitar el runbook en
  `docs/source/compliance/android/device_lab_failover_runbook.md`, conecte el
  ruta del paquete resultante + hash de manifiesto al ticket `_android-device-lab`, y
  refleje la identificación del simulacro tanto en el registro de contingencia como en el registro de evidencia.

Documentar cada activación del plan de contingencia en
`docs/source/compliance/android/device_lab_contingency.md` (incluye fecha,
desencadenante, acciones y seguimientos).

## Prototipo de análisis estático

- `make android-lint` envuelve `ci/check_android_javac_lint.sh`, compilando
  `java/iroha_android` y las fuentes `java/norito_java` compartidas con
  `javac --release 21 -Xlint:all -Werror` (con las categorías marcadas indicadas en
- Después de la compilación, el script aplica la política de dependencia AND6 con
  `jdeps --summary`, fallando si algún módulo está fuera de la lista de permitidos aprobados
  (`java.base`, `java.net.http`, `jdk.httpserver`) aparece. Esto mantiene el
  Superficie de Android alineada con las “dependencias JDK no ocultas” del consejo SDK
  requisito antes de las revisiones de cumplimiento de StrongBox.
- CI ahora ejecuta la misma puerta a través de
  `.github/workflows/android-lint.yml`, que invoca
  `ci/check_android_javac_lint.sh` en cada pulsación/PR que toca el Android o
  fuentes y cargas de Java Norito compartidas `artifacts/android/lint/jdeps-summary.txt`
  para que las revisiones de cumplimiento puedan hacer referencia a una lista de módulos firmados sin volver a ejecutar el
  guión localmente.
- Configure `ANDROID_LINT_KEEP_WORKDIR=1` cuando necesite conservar el temporal
  espacio de trabajo. El script ya copia el resumen del módulo generado en
  `artifacts/android/lint/jdeps-summary.txt`; conjunto
  `ANDROID_LINT_SUMMARY_OUT=docs/source/compliance/android/evidence/android_lint_jdeps.txt`
  (o similar) cuando necesite un artefacto versionado adicional para auditorías.
  Los ingenieros aún deben ejecutar el comando localmente antes de enviar los PR de Android
  que tocan las fuentes de Java y adjuntan el resumen/registro grabado al cumplimiento
  revisiones. Consúltelo en las notas de la versión como “Android javac lint + dependencia
  escanear”.

## Evidencia de CI (pelusa, pruebas, certificación)- `.github/workflows/android-and6.yml` ahora ejecuta todas las puertas AND6 (javac lint +
  escaneo de dependencias, conjunto de pruebas de Android, verificador de atestación StrongBox y
  validación de ranura de laboratorio de dispositivo) en cada PR/empuje que toque la superficie de Android.
- `ci/run_android_tests.sh` envuelve `ci/run_android_tests.sh` y emite
  un resumen determinista en `artifacts/android/tests/test-summary.json` mientras
  persistir el registro de la consola en `artifacts/android/tests/test.log`. Adjunte ambos
  archivos a paquetes de cumplimiento al hacer referencia a ejecuciones de CI.
- `scripts/android_strongbox_attestation_ci.sh --summary-out` produce
  `artifacts/android/attestation/ci-summary.json`, validando el paquete
  cadenas de certificación bajo `artifacts/android/attestation/**` para StrongBox y
  Piscinas TEE.
- `scripts/check_android_device_lab_slot.py --root fixtures/android/device_lab`
  verifica la ranura de muestra (`slot-sample/`) utilizada en CI y puede apuntarse a
  ejecuciones reales bajo `artifacts/android/device_lab/<slot-id>/` con
  `--require-slot --json-out <dest>` para demostrar que siguen los paquetes de instrumentación
  el diseño documentado. CI escribe el resumen de validación para
  `artifacts/android/device_lab/summary.json`; la ranura de muestra incluye
  Telemetría de marcador de posición/certificación/cola/extractos de registro más un registro
  `sha256sum.txt` para hashes reproducibles.

## Flujo de trabajo de instrumentación de laboratorio y dispositivo

Cada ensayo de reserva o conmutación por error debe seguir las
Guía `device_lab_instrumentation.md` para telemetría, cola y atestación
Los artefactos se alinean con el registro de reservas:

1. **Artefactos de ranura de semillas.** Crear
   `artifacts/android/device_lab/<slot>/` con las subcarpetas estándar y ejecute
   `shasum` después de que se cierre la ranura (consulte la sección "Diseño de artefacto" del nuevo
   guía).
2. **Ejecutar comandos de instrumentación.** Ejecutar la telemetría/captura de cola,
   anular el resumen, el arnés StrongBox y el escaneo de pelusa/dependencia exactamente como
   documentado para que las salidas reflejen CI.
3. **Presentar evidencia.** Actualización
   `docs/source/compliance/android/evidence_log.csv` y el ticket de reserva
   con el ID de la ranura, la ruta del manifiesto SHA-256 y el panel/Buildkite correspondiente
   enlaces.

Adjunte la carpeta de artefactos y el manifiesto hash al paquete de lanzamiento AND6 para
la ventana de congelación afectada. Los revisores de gobernanza rechazarán las listas de verificación que no
No citar un identificador de ranura más la guía de instrumentación.

### Evidencia de preparación para reservas y conmutación por error

El elemento de la hoja de ruta "Aprobaciones reglamentarias de artefactos y contingencias de laboratorio" requiere más
que la instrumentación. Cada paquete AND6 también debe hacer referencia al protocolo proactivo
flujo de trabajo de reserva y ensayo trimestral de conmutación por error:- **Libro de estrategias de reserva (`device_lab_reservation.md`).** Sigue la reserva
  tabla (plazos de entrega, propietarios, duración de los espacios), exporte el calendario compartido a través de
  `scripts/android_device_lab_export.py` y registro `_android-device-lab`
  ID de boleto junto con instantáneas de capacidad en `evidence_log.csv`. el libro de jugadas
  detalla la escala de escalada y los factores desencadenantes de contingencias; copia esos detalles
  en la entrada de la lista de verificación cuando las reservas cambian o la capacidad cae por debajo del
  Objetivo de la hoja de ruta del 80%.
- **Runbook de perforación de conmutación por error (`device_lab_failover_runbook.md`).** Ejecute el
  ensayo trimestral (simular apagón → promover carriles alternativos → participar
  Firebase burst + socio externo de StrongBox) y almacenar los artefactos en
  `artifacts/android/device_lab_contingency/<drill-id>/`. Cada paquete debe
  contiene el manifiesto, exportación de PagerDuty, enlaces de ejecución de Buildkite, ráfaga de Firebase
  informe y reconocimiento del anticipo anotados en el runbook. Referencia el
  ID de simulacro, manifiesto SHA-256 y ticket de seguimiento tanto en el registro de evidencia como en el
  esta lista de verificación.

En conjunto, estos documentos demuestran que la planificación de la capacidad de los dispositivos, los ensayos de apagones,
y los paquetes de instrumentación comparten el mismo rastro auditado exigido por el
hoja de ruta y revisores legales.

## Revisar cadencia

- **Trimestral** - Validar que los artefactos de UE/JP estén actualizados; actualizar
  hashes de registro de evidencia; ensayar la captura de procedencia.
- **Prelanzamiento** - Ejecute esta lista de verificación durante cada transición de GA/LTS y adjunte
  el registro completo al RFC de lanzamiento.
- **Post-incidente** - Si un incidente de Sev 1/2 afecta la telemetría, la firma o
  atestación, actualice los resguardos de artefactos relevantes con notas de corrección y
  capturar la referencia en el registro de evidencia.