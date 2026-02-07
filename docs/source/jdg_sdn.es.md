---
lang: es
direction: ltr
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% de certificaciones y rotación de JDG-SDN

Esta nota captura el modelo de cumplimiento para las certificaciones de Nodos de datos secretos (SDN).
utilizado por el flujo Jurisdiction Data Guardian (JDG).

## Formato de compromiso
- `JdgSdnCommitment` vincula el alcance (`JdgAttestationScope`), el cifrado
  hash de carga útil y la clave pública SDN. Los sellos son firmas mecanografiadas.
  (`SignatureOf<JdgSdnCommitmentSignable>`) sobre la carga útil etiquetada con el dominio
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- La validación estructural (`validate_basic`) exige:
  - `version == JDG_SDN_COMMITMENT_VERSION_V1`
  - rangos de bloques válidos
  - sellos no vacíos
  - igualdad de alcance frente a la certificación cuando se ejecuta a través de
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- La deduplicación la maneja el validador de atestación (firmante+hash de carga útil).
  unicidad) para evitar compromisos retenidos/duplicados.

## Política de registro y rotación
- Las claves SDN se encuentran en `JdgSdnRegistry`, codificadas por `(Algorithm, public_key_bytes)`.
- `JdgSdnKeyRecord` registra la altura de activación, altura de retiro opcional,
  y clave principal opcional.
- La rotación se rige por `JdgSdnRotationPolicy` (actualmente: `dual_publish_blocks`
  ventana superpuesta). El registro de una clave secundaria actualiza la jubilación principal a
  `child.activation + dual_publish_blocks`, con barandillas:
  - los padres desaparecidos son rechazados
  - las activaciones deben ser estrictamente crecientes
  - se rechazan las superposiciones que exceden la ventana de gracia
- Los asistentes de registro muestran los registros instalados (`record`, `keys`) para conocer el estado.
  y exposición API.

## Flujo de validación
- `JdgAttestation::validate_with_sdn_registry` envuelve el estructural
  verificaciones de atestación y cumplimiento de SDN. Hilos `JdgSdnPolicy`:
  - `require_commitments`: imponer presencia para PII/cargas secretas
  - `rotation`: ventana de gracia utilizada al actualizar el retiro de los padres
- Cada compromiso se verifica para:
  - validez estructural + coincidencia de certificación-alcance
  - presencia de clave registrada
  - ventana activa que cubre el rango de bloques atestiguados (límites de jubilación ya
    incluir la gracia de publicación dual)
  - sello válido sobre el cuerpo del compromiso etiquetado con el dominio
- Los errores estables emergen en el índice como evidencia del operador:
  `MissingSdnCommitments`, `UnknownSdnKey`, `InactiveSdnKey`, `InvalidSeal`,
  o fallas estructurales `Commitment`/`ScopeMismatch`.

## Libro de ejecución del operador
- **Provisión:** registre la primera clave SDN con `activated_at` en o antes del
  altura del primer bloque secreto. Publicar la huella digital clave para los operadores de JDG.
- **Rotar:** generar la clave sucesora, registrarla con `rotation_parent`
  apuntando a la clave actual y confirme que la jubilación del padre es igual
  `child_activation + dual_publish_blocks`. Volver a sellar los compromisos de carga útil con
  la clave activa durante la ventana de superposición.
- **Auditoría:** exponer instantáneas del registro (`record`, `keys`) a través de Torii/status
  superficies para que los auditores puedan confirmar la clave activa y las ventanas de retiro. Alerta
  si el rango comprobado queda fuera de la ventana activa.
- **Recuperación:** `UnknownSdnKey` → asegúrese de que el registro incluya la clave de sellado;
  `InactiveSdnKey` → girar o ajustar las alturas de activación; `InvalidSeal` →
  volver a sellar cargas útiles y actualizar certificaciones.## Ayudante de tiempo de ejecución
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) empaqueta una póliza +
  registro y valida certificaciones a través de `validate_with_sdn_registry`.
- Los registros se pueden cargar desde paquetes `JdgSdnKeyRecord` codificados con Norito (consulte
  `JdgSdnEnforcer::from_reader`/`from_path`) o ensamblado con
  `from_records`, que aplica las barandillas de rotación durante el registro.
- Los operadores pueden conservar el paquete Norito como prueba del estado/Torii.
  emerger a la superficie mientras la misma carga útil alimenta al ejecutor utilizado por la admisión y
  guardias de consenso. Se puede inicializar un único ejecutor global al inicio a través de
  `init_enforcer_from_path` y `enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  exponer la política activa + registros clave para las superficies status/Torii.

## Pruebas
- Cobertura de regresión en `crates/iroha_data_model/src/jurisdiction.rs`:
  `sdn_registry_accepts_active_commitment`, `sdn_registry_rejects_unknown_key`,
  `sdn_registry_rejects_inactive_key`, `sdn_registry_rejects_bad_signature`,
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`, junto con el existente
  certificación estructural/pruebas de validación SDN.