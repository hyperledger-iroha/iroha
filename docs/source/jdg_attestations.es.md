---
lang: es
direction: ltr
source: docs/source/jdg_attestations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 459e8ed4612da7cfa68053e4e299b2f68e7620d4f3b98a8a721ebf8327829ea1
source_last_modified: "2026-01-09T07:05:10.922933+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Atestaciones JDG: Guardia, Rotación y Retención

Esta nota documenta la protección de atestación JDG v1 que ahora se envía en `iroha_core`.

- **Manifiestos del comité:** Los paquetes `JdgCommitteeManifest` codificados con Norito llevan rotación por espacio de datos
  horarios (`committee_id`, miembros ordenados, umbral, `activation_height`, `retire_height`).
  Los manifiestos se cargan con `JdgCommitteeSchedule::from_path` y aplican estrictamente valores crecientes.
  alturas de activación con una superposición de gracia opcional (`grace_blocks`) entre retirar/activar
  comités.
- **Protección de atestación:** `JdgAttestationGuard` aplica el enlace del espacio de datos, la caducidad y los límites obsoletos.
  Coincidencia de identificación/umbral de comité, membresía de firmante, esquemas de firma admitidos y opciones opcionales.
  Validación SDN vía `JdgSdnEnforcer`. Los límites de tamaño, el retraso máximo y los esquemas de firma permitidos son
  parámetros del constructor; `validate(attestation, dataspace, current_height)` devuelve el activo
  comité o un error estructurado.
  - `scheme_id = 1` (`simple_threshold`): firmas por firmante, mapa de bits de firmante opcional.
  - `scheme_id = 2` (`bls_normal_aggregate`): firma normal BLS preagregada única sobre el
    hash de certificación; Mapa de bits del firmante opcional; el valor predeterminado es todos los firmantes en la certificación. BLS
    la validación agregada requiere un PoP válido por miembro del comité en el manifiesto; desaparecido o
    Los PoP no válidos rechazan la atestación.
  Configure la lista de permitidos a través de `governance.jdg_signature_schemes`.
- **Almacén de retención:** `JdgAttestationStore` rastrea las certificaciones por espacio de datos con un configurable
  límite por espacio de datos, eliminando las entradas más antiguas al insertar. Llame a `for_dataspace` o
  `for_dataspace_and_epoch` para recuperar paquetes de auditoría/reproducción.
- **Pruebas:** La cobertura de la unidad ahora ejerce una selección de comité válida, rechazo del firmante desconocido, obsoleto
  rechazo de atestación, identificaciones de esquemas no admitidos y poda de retención. Ver
  `crates/iroha_core/src/jurisdiction.rs`.

El guardia rechaza esquemas fuera de la lista de permitidos configurada.