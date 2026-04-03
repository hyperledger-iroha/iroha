<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/source/soracloud/manifest_schemas.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0724ed92da90d8d78a7095b5fc75523ea5dd4a1c885059e28c1bbb8118d1f8c
source_last_modified: "2026-03-26T06:12:11.480497+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Esquemas de manifiesto de SoraCloud V1

Esta página define los primeros esquemas deterministas Norito para SoraCloud
implementación en Iroha 3:

- `SoraContainerManifestV1`
-`SoraServiceManifestV1`
- `SoraStateBindingV1`
- `SoraDeploymentBundleV1`
- `AgentApartmentManifestV1`
- `FheParamSetV1`
- `FheExecutionPolicyV1`
- `FheGovernanceBundleV1`
- `FheJobSpecV1`
- `DecryptionAuthorityPolicyV1`
- `DecryptionRequestV1`
- `CiphertextQuerySpecV1`
- `CiphertextQueryResponseV1`
- `SecretEnvelopeV1`
- `CiphertextStateRecordV1`

Las definiciones de Rust se encuentran en `crates/iroha_data_model/src/soracloud.rs`.

Los registros de tiempo de ejecución privado del modelo cargado son intencionalmente una capa separada de
estos manifiestos de implementación de SCR. Deberían ampliar el avión modelo Soracloud
y reutilizar `SecretEnvelopeV1` / `CiphertextStateRecordV1` para bytes cifrados
y estado nativo del texto cifrado, en lugar de codificarse como un nuevo servicio/contenedor
manifiesta. Ver `uploaded_private_models.md`.

## Alcance

Estos manifiestos están diseñados para `IVM` + Sora Container Runtime personalizado.
(SCR) dirección (sin WASM, sin dependencia Docker en la admisión en tiempo de ejecución).- `SoraContainerManifestV1` captura la identidad del paquete ejecutable, el tipo de tiempo de ejecución,
  política de capacidad, recursos, configuración de sondeo del ciclo de vida y
  exportaciones de configuración requerida al entorno de ejecución o revisión montada
  árbol.
- `SoraServiceManifestV1` captura la intención de implementación: identidad del servicio,
  hash/versión del manifiesto del contenedor al que se hace referencia, enrutamiento, política de implementación y
  vinculaciones estatales.
- `SoraStateBindingV1` captura el alcance y los límites deterministas de escritura de estado
  (prefijo de espacio de nombres, modo de mutabilidad, modo de cifrado, cuotas totales/de artículos).
- `SoraDeploymentBundleV1` combina contenedor + manifiestos de servicio y aplica
  comprobaciones de admisión deterministas (enlace de hash de manifiesto, alineación de esquema y
  capacidad/consistencia vinculante).
- `AgentApartmentManifestV1` captura la política de tiempo de ejecución del agente persistente:
  límites de herramientas, límites de políticas, límites de gasto, cuota estatal, salida de red y
  comportamiento de actualización.
- `FheParamSetV1` captura conjuntos de parámetros FHE administrados por gobernanza:
  identificadores deterministas de backend/esquema, perfil de módulo, seguridad/profundidad
  límites y alturas del ciclo de vida (`activation`/`deprecation`/`withdraw`).
- `FheExecutionPolicyV1` captura límites deterministas de ejecución de texto cifrado:
  tamaños de carga útil admitidos, entrada/salida fan-in, límites de profundidad/rotación/arranque,
  y modo de redondeo canónico.
- `FheGovernanceBundleV1` combina un conjunto de parámetros y una política para determinista
  validación de admisión.- `FheJobSpecV1` captura la admisión/ejecución del trabajo de texto cifrado determinista
  solicitudes: clase de operación, compromisos de entrada ordenados, clave de salida y límites
  Demanda de profundidad/rotación/arranque vinculada a una política + conjunto de parámetros.
- `DecryptionAuthorityPolicyV1` captura la política de divulgación gestionada por la gobernanza:
  modo de autoridad (servicio mantenido por el cliente versus servicio de umbral), quórum/miembros de aprobación,
  asignación por rotura de cristales, etiquetado de jurisdicción, requisito de prueba de consentimiento,
  Límites TTL y etiquetado de auditoría canónico.
- `DecryptionRequestV1` captura intentos de divulgación vinculados a políticas:
  referencia de clave de texto cifrado (`binding_name` + `state_key` + compromiso),
  justificación, etiqueta de jurisdicción, hash de evidencia de consentimiento opcional, TTL,
  intención/razón de romper el cristal y vínculo de hash de gobernanza.
- `CiphertextQuerySpecV1` captura la intención determinista de consulta de solo texto cifrado:
  alcance de servicio/enlace, filtro de prefijo de clave, límite de resultados limitado, metadatos
  nivel de proyección y alternancia de inclusión de prueba.
- `CiphertextQueryResponseV1` captura resultados de consultas con divulgación minimizada:
  referencias clave orientadas al resumen, metadatos de texto cifrado, pruebas de inclusión opcionales,
  y contexto de secuencia/truncamiento a nivel de respuesta.
- `SecretEnvelopeV1` captura el material de carga útil cifrado:
  modo de cifrado, identificador/versión de clave, nonce, bytes de texto cifrado y
  compromisos de integridad.
- `CiphertextStateRecordV1` captura entradas de estado nativo de texto cifrado quecombinar metadatos públicos (tipo de contenido, etiquetas de políticas, compromiso, tamaño de carga útil)
  con un `SecretEnvelopeV1`.
- Los paquetes de modelos privados cargados por el usuario deben basarse en estos textos cifrados nativos.
  registros:
  Los fragmentos de peso/configuración/procesador cifrados viven en el estado, mientras que el registro del modelo,
  El linaje de peso, los perfiles de compilación, las sesiones de inferencia y los puntos de control permanecen
  Registros Soracloud de primera clase.

## Versionado

-`SORA_CONTAINER_MANIFEST_VERSION_V1 = 1`
- `SORA_SERVICE_MANIFEST_VERSION_V1 = 1`
- `SORA_STATE_BINDING_VERSION_V1 = 1`
- `SORA_DEPLOYMENT_BUNDLE_VERSION_V1 = 1`
- `AGENT_APARTMENT_MANIFEST_VERSION_V1 = 1`
- `FHE_PARAM_SET_VERSION_V1 = 1`
- `FHE_EXECUTION_POLICY_VERSION_V1 = 1`
- `FHE_GOVERNANCE_BUNDLE_VERSION_V1 = 1`
- `FHE_JOB_SPEC_VERSION_V1 = 1`
- `DECRYPTION_AUTHORITY_POLICY_VERSION_V1 = 1`
- `DECRYPTION_REQUEST_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_SPEC_VERSION_V1 = 1`
-`CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 = 1`
- `CIPHERTEXT_QUERY_PROOF_VERSION_V1 = 1`
- `SECRET_ENVELOPE_VERSION_V1 = 1`
- `CIPHERTEXT_STATE_RECORD_VERSION_V1 = 1`

La validación rechaza versiones no compatibles con
`SoraCloudManifestError::UnsupportedVersion`.

## Reglas de validación deterministas (V1)- Manifiesto del contenedor:
  - `bundle_path` e `entrypoint` no deben estar vacíos.
  - `healthcheck_path` (si está configurado) debe comenzar con `/`.
  - `config_exports` puede hacer referencia solo a configuraciones declaradas en
    `required_config_names`.
  - Los objetivos de entorno de config-export deben usar nombres de variables de entorno canónicos
    (`[A-Za-z_][A-Za-z0-9_]*`).
  - Los destinos del archivo de exportación de configuración deben permanecer relativos, usar separadores `/` y
    no debe contener segmentos vacíos, `.` o `..`.
  - Las exportaciones de configuración no deben apuntar a la misma var de entorno o ruta de archivo relativa.
    más de una vez.
- Manifiesto de servicio:
  - `service_version` no debe estar vacío.
  - `container.expected_schema_version` debe coincidir con el esquema de contenedor v1.
  - `rollout.canary_percent` debe ser `0..=100`.
  - `route.path_prefix` (si está configurado) debe comenzar con `/`.
  - Los nombres de enlace de estado deben ser únicos.
- Vinculación estatal:
  - `key_prefix` no debe estar vacío y comenzar con `/`.
  - `max_item_bytes <= max_total_bytes`.
  - Los enlaces `ConfidentialState` no pueden utilizar cifrado de texto sin formato.
- Paquete de implementación:
  - `service.container.manifest_hash` debe coincidir con la codificación canónica
    hash del manifiesto del contenedor.
  - `service.container.expected_schema_version` debe coincidir con el esquema del contenedor.
  - Los enlaces de estado mutables requieren `container.capabilities.allow_state_writes=true`.
  - Las rutas públicas requieren `container.lifecycle.healthcheck_path`.
- Manifiesto del apartamento del agente:
  - `container.expected_schema_version` debe coincidir con el esquema de contenedor v1.
  - Los nombres de las capacidades de las herramientas no deben estar vacíos y ser únicos.- Los nombres de las capacidades de las políticas deben ser únicos.
  - Los activos con límite de gasto no deben estar vacíos y ser únicos.
  - `max_per_tx_nanos <= max_per_day_nanos` para cada límite de gasto.
  - La política de red de la lista de permitidos debe incluir hosts únicos que no estén vacíos.
- Conjunto de parámetros FHE:
  - `backend` e `ciphertext_modulus_bits` no deben estar vacíos.
  - El tamaño de bits de cada módulo de texto cifrado debe estar dentro de `2..=120`.
  - El orden de la cadena del módulo de texto cifrado no debe ser creciente.
  - `plaintext_modulus_bits` debe ser menor que el módulo de texto cifrado más grande.
  - `slot_count <= polynomial_modulus_degree`.
  - `max_multiplicative_depth < ciphertext_modulus_bits.len()`.
  - el orden de altura del ciclo de vida debe ser estricto:
    `activation < deprecation < withdraw` cuando esté presente.
  - requisitos de estado del ciclo de vida:
    - `Proposed` no permite niveles de desaprobación/retirada.
    - `Active` requiere `activation_height`.
    - `Deprecated` requiere `activation_height` + `deprecation_height`.
    - `Withdrawn` requiere `activation_height` + `withdraw_height`.
- Política de ejecución de la FHE:
  -`max_plaintext_bytes <= max_ciphertext_bytes`.
  - `max_output_ciphertexts <= max_input_ciphertexts`.
  - el enlace del conjunto de parámetros debe coincidir con `(param_set, version)`.
  - `max_multiplication_depth` no debe exceder la profundidad establecida por los parámetros.
  - La admisión de políticas rechaza el ciclo de vida del conjunto de parámetros `Proposed` o `Withdrawn`.
- Paquete de gobernanza FHE:
  - valida la compatibilidad de política + conjunto de parámetros como una carga útil de admisión determinista.
- Especificaciones del trabajo FHE:
  - `job_id` e `output_state_key` no deben estar vacíos (`output_state_key` comienza con `/`).- el conjunto de entrada no debe estar vacío y las claves de entrada deben ser rutas canónicas únicas.
  - las restricciones específicas de la operación son estrictas (entrada múltiple `Add`/`Multiply`,
    `RotateLeft`/`Bootstrap` de entrada única, con perillas de profundidad/rotación/arranque mutuamente excluyentes).
  - la admisión vinculada a políticas impone:
    - Los identificadores y versiones de políticas/parámetros coinciden.
    - Los límites de recuento/bytes de entrada, profundidad, rotación y arranque están dentro de los límites de la política.
    - Los bytes de salida proyectados deterministas se ajustan a los límites de texto cifrado de la política.
- Política de autoridad de descifrado:
  - `approver_ids` no debe estar vacío, ser único y estar estrictamente ordenado lexicográficamente.
  - El modo `ClientHeld` requiere exactamente un aprobador, `approver_quorum=1`,
    y `allow_break_glass=false`.
  - El modo `ThresholdService` requiere al menos dos aprobadores y
    `approver_quorum <= approver_ids.len()`.
  - `jurisdiction_tag` no debe estar vacío y no debe contener caracteres de control.
  - `audit_tag` no debe estar vacío y no debe contener caracteres de control.
- Solicitud de descifrado:
  - `request_id`, `state_key` e `justification` no deben estar vacíos
    (`state_key` comienza con `/`).
  - `jurisdiction_tag` no debe estar vacío y no debe contener caracteres de control.
  - `break_glass_reason` se requiere cuando `break_glass=true` y se debe omitir cuando
    `break_glass=false`.
  - la admisión vinculada a políticas impone la igualdad de nombres de políticas, no solicita TTLsuperior a `policy.max_ttl_blocks`, igualdad de etiquetas de jurisdicción, rotura de cristales
    requisitos de selección y prueba de consentimiento cuando
    `policy.require_consent_evidence=true` para solicitudes sin rotura de cristales.
- Especificaciones de consulta de texto cifrado:
  - `state_key_prefix` no debe estar vacío y comenzar con `/`.
  - `max_results` está limitado de forma determinista (`<=256`).
  - la proyección de metadatos es explícita (`Minimal` solo resumen versus `Standard` clave visible).
- Respuesta a la consulta de texto cifrado:
  - `result_count` debe ser igual al recuento de filas serializadas.
  - La proyección `Minimal` no debe exponer `state_key`; `Standard` debe exponerlo.
  - Las filas nunca deben mostrar el modo de cifrado de texto sin formato.
  - las pruebas de inclusión (cuando estén presentes) deben incluir identificadores de esquema no vacíos y
    `anchor_sequence >= event_sequence`.
- Sobre secreto:
  - `key_id`, `nonce` e `ciphertext` no deben estar vacíos.
  - la longitud nonce está limitada (`<=256` bytes).
  - La longitud del texto cifrado está limitada (`<=33554432` bytes).
- Registro de estado de texto cifrado:
  - `state_key` no debe estar vacío y comenzar con `/`.
  - el tipo de contenido de metadatos no debe estar vacío; Las etiquetas deben ser cadenas únicas y no vacías.
  - `metadata.payload_bytes` debe ser igual a `secret.ciphertext.len()`.
  - `metadata.commitment` debe ser igual a `secret.commitment`.

## Accesorios canónicos

Los dispositivos JSON canónicos se almacenan en:- `fixtures/soracloud/sora_container_manifest_v1.json`
- `fixtures/soracloud/sora_service_manifest_v1.json`
- `fixtures/soracloud/sora_state_binding_v1.json`
- `fixtures/soracloud/sora_deployment_bundle_v1.json`
- `fixtures/soracloud/agent_apartment_manifest_v1.json`
-`fixtures/soracloud/fhe_param_set_v1.json`
- `fixtures/soracloud/fhe_execution_policy_v1.json`
- `fixtures/soracloud/fhe_governance_bundle_v1.json`
- `fixtures/soracloud/fhe_job_spec_v1.json`
- `fixtures/soracloud/decryption_authority_policy_v1.json`
- `fixtures/soracloud/decryption_request_v1.json`
- `fixtures/soracloud/ciphertext_query_spec_v1.json`
- `fixtures/soracloud/ciphertext_query_response_v1.json`
-`fixtures/soracloud/secret_envelope_v1.json`
- `fixtures/soracloud/ciphertext_state_record_v1.json`

Pruebas de fijación/ida y vuelta:

- `crates/iroha_data_model/tests/soracloud_manifest_fixtures.rs`