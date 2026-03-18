---
lang: es
direction: ltr
source: docs/source/governance_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9201c0027f05b1ab2c83fa6b3e1a1e6dad3ff9660a8ed23bac7667408d421ada
source_last_modified: "2026-01-22T15:38:30.665014+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guía de gobernanza

Este manual captura los rituales diarios que mantienen a Sora Network
consejo de gobierno alineado. Agrega las referencias autorizadas de la
repositorio para que las ceremonias individuales puedan seguir siendo concisas, mientras que los operadores siempre
tener un único punto de entrada para el proceso más amplio.

## Ceremonias del Consejo

- **Gobierno del accesorio** – Ver [Aprobación del accesorio del Parlamento de Sora](sorafs/signing_ceremony.md)
  para el flujo de aprobación en cadena que ahora el Panel de Infraestructura del Parlamento
  sigue al revisar las actualizaciones del fragmentador SoraFS.
- **Publicación del recuento de votos** – Consulte
  [Recuento de votos de gobernanza](governance_vote_tally.md) para la CLI paso a paso
  plantilla de flujo de trabajo y generación de informes.

## Runbooks operativos

- **Integraciones de API** – [Referencia de API de gobernanza](governance_api.md) enumera las
  Superficies REST/gRPC expuestas por los servicios del ayuntamiento, incluida la autenticación
  requisitos y reglas de paginación.
- **Paneles de telemetría**: las definiciones JSON Grafana en
  `docs/source/grafana_*` define las “Restricciones de Gobernanza” y el “Programador
  tableros TEU”. Exporte el JSON a Grafana después de cada lanzamiento para mantenerse alineado
  con el diseño canónico.

## Supervisión de la disponibilidad de datos

### Clases de retención

Los paneles del Parlamento que aprueban los manifiestos de la Fiscalía deben hacer referencia a la retención forzosa
política antes de votar. La siguiente tabla refleja los valores predeterminados aplicados a través de
`torii.da_ingest.replication_policy` para que los revisores puedan detectar discrepancias sin
buscando el TOML fuente.【docs/source/da/replication_policy.md:1】

| Etiqueta de gobernanza | Clase de burbuja | Retención en caliente | Retención de frío | Réplicas requeridas | Clase de almacenamiento |
|----------------|------------|---------------|----------------|-------------------|---------------|
| `da.taikai.live` | `taikai_segment` | 24h | 14d | 5 | `hot` |
| `da.sidecar` | `nexus_lane_sidecar` | 6h | 7d | 4 | `warm` |
| `da.governance` | `governance_artifact` | 12h | 180d | 3 | `cold` |
| `da.default` | _todas las demás clases_ | 6h | 30d | 3 | `warm` |

El Panel de Infraestructura debe adjuntar la plantilla completa de
`docs/examples/da_manifest_review_template.md` a cada boleta para que el manifiesto
El resumen, la etiqueta de retención y los artefactos Norito permanecen vinculados en la gobernanza.
registro.

### Pista de auditoría del manifiesto firmado

Antes de que una votación llegue a la agenda, el personal del consejo debe demostrar que el manifiesto
Los bytes bajo revisión coinciden con el sobre del Parlamento y el artefacto SoraFS. uso
las herramientas existentes para recopilar esa evidencia:1. Obtenga el paquete de manifiesto de Torii (`iroha app da get-blob --storage-ticket <hex>`
   o el asistente de SDK equivalente) para que todos apliquen hash a los mismos bytes que alcanzaron
   las puertas de enlace.
2. Ejecute el verificador de resguardo del manifiesto con el sobre firmado:
   ```
   cargo run -p sorafs_car --bin sorafs-manifest-stub -- manifest.json \
     --manifest-signatures-in=fixtures/sorafs_chunker/manifest_signatures.json \
     --json-out=/tmp/manifest_report.json
   ```
   Esto vuelve a calcular el resumen del manifiesto BLAKE3, valida el
   `chunk_digest_sha3_256` y verifica cada firma Ed25519 incrustada en
   `manifest_signatures.json`. Ver `docs/source/sorafs/manifest_pipeline.md`
   para opciones CLI adicionales.
3. Copie el resumen, `chunk_digest_sha3_256`, el identificador de perfil y la lista de firmantes en
   la plantilla de revisión. NOTA: si el verificador informa "perfil no coincidente" o una
   falta la firma, suspender la votación y solicitar un sobre corregido.
4. Almacene la salida del verificador (o el artefacto CI de
   `ci/check_sorafs_fixtures.sh`) junto con la carga útil Norito `.to` para que los auditores
   Puede reproducir la evidencia sin acceder a puertas de enlace internas.

El paquete de auditoría resultante debería permitir al Parlamento recrear cada hash y firma.
verifique incluso después de que el manifiesto salga del almacenamiento en caliente.

### Revisar la lista de verificación

1. Saque el sobre del manifiesto aprobado por el Parlamento (ver
   `docs/source/sorafs/signing_ceremony.md`) y registrar el resumen de BLAKE3.
2. Verifique que el bloque `RetentionPolicy` del manifiesto coincida con la etiqueta en la tabla
   arriba; Torii rechazará los desajustes, pero el consejo debe capturar los
   evidencia para auditores.【docs/source/da/replication_policy.md:32】
3. Confirme que la carga útil Norito enviada hace referencia a la misma etiqueta de retención.
   y clase de blob que aparece en el ticket de admisión.
4. Adjunte prueba de la verificación de políticas (salida CLI, `torii.da_ingest.replication_policy`
   volcado o artefacto de CI) al paquete de revisión para que SRE pueda reproducir la decisión.
5. Registrar los subsidios planificados o los ajustes de alquiler cuando la propuesta dependa de
   `docs/source/sorafs_reserve_rent_plan.md`.

### Matriz de escalamiento

| Tipo de solicitud | Panel propietario | Pruebas para adjuntar | Plazos y telemetría | Referencias |
|--------------|--------------|--------------------|-----------------------|------------|
| Subvención/ajuste del alquiler | Infraestructura + Tesorería | Paquete DA completo, delta de alquiler de `reserve_rentd`, proyección de reserva actualizada CSV, actas de votación del consejo | Tenga en cuenta el impacto en el alquiler antes de enviar la actualización del Tesoro; incluir telemetría de buffer de 30 días para que Finanzas pueda conciliar dentro de la próxima ventana de liquidación | `docs/source/sorafs_reserve_rent_plan.md`, `docs/examples/da_manifest_review_template.md` |
| Eliminación de moderación/acción de cumplimiento | Moderación + Cumplimiento | Ticket de cumplimiento (`ComplianceUpdateV1`), tokens de prueba, resumen de manifiesto firmado, estado de apelación | Siga el SLA de cumplimiento de la puerta de enlace (confirmación dentro de las 24 horas, eliminación completa ≤72 horas). Adjunte un extracto `TransparencyReportV1` que muestra la acción. | `docs/source/sorafs_gateway_compliance_plan.md`, `docs/source/sorafs_moderation_panel_plan.md` |
| Congelación/reversión de emergencia | Panel de moderación del Parlamento | Paquete de aprobación previa, nueva orden de congelación, resumen del manifiesto de reversión, registro de incidentes | Publicar un aviso de congelación de inmediato y programar el referéndum de reversión dentro del siguiente período de gobernanza; incluir saturación de buffer + telemetría de replicación DA para justificar la emergencia. | `docs/source/sorafs/signing_ceremony.md`, `docs/source/sorafs_moderation_panel_plan.md` |Utilice la tabla al clasificar los tickets de admisión para que cada panel reciba la información exacta.
los elementos necesarios para ejecutar su mandato.

### Informes entregables

Cada decisión DA-10 debe enviarse con los siguientes artefactos (adjuntarlos al
Entrada del DAG de gobernanza a la que se hace referencia en la votación):

- El paquete Markdown completo de
  `docs/examples/da_manifest_review_template.md` (ahora incluye firma y
  secciones de escalada).
- El manifiesto Norito firmado (`.to`) más el sobre `manifest_signatures.json`
  o registros del verificador de CI que prueban el resumen de recuperación.
- Cualquier actualización de transparencia provocada por la acción:
  - Delta `TransparencyReportV1` para eliminaciones o congelaciones impulsadas por el cumplimiento.
  - Delta del libro mayor de alquiler/reserva o instantánea `ReserveSummaryV1` para subsidios.
- Enlaces a instantáneas de telemetría recopiladas durante la revisión (profundidad de replicación,
  margen de amortiguación, retraso en la moderación) para que los observadores puedan verificar las condiciones
  después del hecho.

## Moderación y escalada

La eliminación de puertas de enlace, la recuperación de subsidios o la congelación de DA siguen el cumplimiento
tubería descrita en `docs/source/sorafs_gateway_compliance_plan.md` y el
herramientas de apelación en `docs/source/sorafs_moderation_panel_plan.md`. Los paneles deben:

1. Registre el ticket de cumplimiento de origen (`ComplianceUpdateV1` o
   `ModerationAppealV1`) y adjunte los tokens de prueba asociados. 【docs/source/sorafs_gateway_compliance_plan.md:20】
2. Confirmar si la solicitud invoca la vía de apelación de moderación (panel de ciudadanos
   votación) o una congelación de emergencia del Parlamento; Ambos flujos deben citar el manifiesto.
   etiqueta de resumen y retención capturada en la nueva plantilla.【docs/source/sorafs_moderation_panel_plan.md:1】
3. Enumerar los plazos de escalada (ventanas de compromiso/revelación de apelaciones,
   duración de la congelación) e indicar qué consejo o panel es el propietario del seguimiento.
4. Capture la instantánea de telemetría (espacio en el búfer, trabajo pendiente de moderación) utilizada para
   Justificar la acción para que las auditorías posteriores puedan hacer coincidir la decisión con la realidad.
   estado.

Los paneles de cumplimiento y moderación deberán sincronizar sus informes semanales de transparencia
con los operadores de routers del acuerdo para que las bajas y subvenciones afecten a los mismos
conjunto de manifiestos.

## Plantillas de informes

Todas las revisiones DA-10 ahora requieren un paquete Markdown firmado. Copiar
`docs/examples/da_manifest_review_template.md`, complete los metadatos del manifiesto,
tabla de verificación de retención y resumen de votación del panel, luego fije el formulario completo
documento (más los artefactos Norito/JSON con referencia) a la entrada del DAG de gobernanza.
Los paneles deben vincular el paquete en las actas de gobernanza para que futuras eliminaciones o
Las renovaciones de subsidios pueden citar el resumen del manifiesto original sin volver a ejecutar el
ceremonia entera.

## Flujo de trabajo de incidentes y revocación

Las acciones de emergencia ahora ocurren en cadena. Cuando es necesario liberar un dispositivo
retroceder, presentar una boleta de gobernanza y abrir una propuesta de reversión en el Parlamento
apuntando al resumen del manifiesto previamente aprobado. El Panel de Infraestructura
maneja la votación y, una vez finalizado, el tiempo de ejecución Nexus publica la reversión
evento que consumen los clientes intermedios. No se requieren artefactos JSON locales.

## Mantener actualizado el manual de estrategias- Actualice este archivo cada vez que llegue un nuevo runbook orientado a la gobernanza en el
  repositorio.
- Enlace cruzado nuevas ceremonias aquí para que el índice del consejo siga siendo visible.
- Si un documento al que se hace referencia se mueve (por ejemplo, una nueva ruta del SDK), actualice el enlace
  como parte de la misma solicitud de extracción para evitar punteros obsoletos.