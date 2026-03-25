---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Activos confidenciales y transferencias ZK
descripción: Plano de Fase C para circulación blindada, registros y controles de operador.
slug: /nexus/activos-confidenciales
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseño de activos confidenciales y transferencias ZK

## Motivación
- Entregar flujos de activos blindados opt-in para que los dominios preserven la privacidad transaccional sin alterar la circulación transparente.
- Mantener la ejecución determinista en hardware heterogéneo de validadores y conservar Norito/Kotodama ABI v1.
- Proveer a auditores y operadores controles de ciclo de vida (activación, rotación, revocación) para circuitos y parámetros criptográficos.

## Modelo de amenazas
- Los validadores son honestos-pero-curiosos: ejecutan consenso fielmente pero intentan inspeccionar ledger/state.
- Observadores de red ven datos de bloque y transacciones gossiped; no se suponen canales de chismes privados.
- Fuera de alcance: análisis de tráfico off-ledger, adversarios cuanticos (seguido en el roadmap PQ), ataques de disponibilidad del ledger.## Resumen de diseño
- Los activos pueden declarar un *shielded pool* además de los saldos transparentes existentes; la circulacion blindada se representa vía compromisos criptográficos.
- Las notas encapsulan `(asset_id, amount, recipient_view_key, blinding, rho)` con:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independiente del orden de las notas.
  - Carga útil cifrada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Las transacciones transportan payloads `ConfidentialTransfer` codificados con Norito que contienen:
  - Entradas públicas: ancla Merkle, anuladores, nuevos compromisos, id de activo, versión de circuito.
  - Cargas útiles encriptadas para destinatarios y auditores opcionales.
  - Prueba de conocimiento cero que atesta conservación de valor, propiedad y autorización.
- Verificación de claves y conjuntos de parámetros son controlados mediante registros en el libro mayor con ventanas de activación; los nudos rechazan validar pruebas que referencian entradas desconocidas o revocadas.
- Los encabezados de consenso comprometen el resumen activo de capacidades confidenciales para que los bloques solo se acepten cuando el estado de registros y parámetros coinciden.
- La construcción de pruebas usa una pila Halo2 (Plonkish) sin configuración confiable; Groth16 u otras variantes SNARK se consideran intencionalmente no soportadas en v1.

### Calendario deterministaLos sobres de memo confidenciales ahora incluyen un accesorio canónico en `fixtures/confidential/encrypted_payload_v1.json`. El conjunto de datos captura un sobre v1 positivo más muestras negativas malformadas para que los SDK puedan afirmar paridad de análisis. Las pruebas del modelo de datos en Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y la suite de Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) cargan el dispositivo directamente, garantizando que la codificación Norito, las superficies de error y la cobertura de regresión permanecerán alineadas mientras evoluciona el códec.

Los SDK de Swift ahora pueden emitir instrucciones escudo sin pegamento JSON a medida: construye un
`ShieldRequest` con el compromiso de 32 bytes, la carga útil encriptada y metadatos de débito,
luego llama `IrohaSDK.submit(shield:keypair:)` (o `submitAndWait`) para firmar y enviar la
transacción sobre `/v1/pipeline/transactions`. El ayudante valida longitudes de compromiso,
enhebra `ConfidentialEncryptedPayload` en el codificador Norito, y refleja el diseño `zk::Shield`
Se describe a continuación para que las billeteras se mantengan sincronizadas con Rust.## Compromisos de consenso y capacidad de activación
- Los encabezados de bloque exponente `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; el digest participa en el hash de consenso y debe igualar la vista local del registro para aceptar el bloque.
- Governance puede preparar actualizaciones programando `next_conf_features` con un `activation_height` futuro; hasta esa altura, los productores de bloques deben seguir emitiendo el resumen anterior.
- Los nodos validadores DEBEN operan con `confidential.enabled = true` y `assume_valid = false`. Los cheques de inicio rechazan unirse al conjunto de validadores si cualquiera falla o si el `conf_features` local diverge.
- Los metadatos del handshake P2P ahora incluyen `{ enabled, assume_valid, conf_features }`. Peers que anuncien características no soportadas se rechazan con `HandshakeConfidentialMismatch` y nunca entran en rotación de consenso.
- Los resultados de handshake entre validadores, observadores y pares se capturan en la matriz de handshake bajo [Node Capability Negotiation](#node-capability-negotiation). Los fallos de handshake exponen `HandshakeConfidentialMismatch` y mantienen al peer fuera de la rotación de consenso hasta que su digest coincide.
- Observadores no validadores pueden fijar `assume_valid = true`; aplican deltas confidenciales a ciegas pero no influyen en la seguridad del consenso.## Políticas de activos
- Cada definición de activo lleva un `AssetConfidentialPolicy` fijada por el creador o vía gobernanza:
  - `TransparentOnly`: modo por defecto; solo se permiten instrucciones transparentes (`MintAsset`, `TransferAsset`, etc.) y las operaciones blindadas se rechazan.
  - `ShieldedOnly`: toda emisión y transferencias deben usar instrucciones confidenciales; `RevealConfidential` esta prohibido para que los saldos nunca se expongan públicamente.
  - `Convertible`: los titulares pueden mover valor entre representaciones transparentes y blindadas usando las instrucciones de on/off-ramp de abajo.
- Las políticas siguen un FSM restringido para evitar fondos varados:
  - `TransparentOnly -> Convertible` (habilitacion inmediata de piscina blindada).
  - `TransparentOnly -> ShieldedOnly` (requiere transición pendiente y ventana de conversión).
  - `Convertible -> ShieldedOnly` (demora mínima obligatoria).
  - `ShieldedOnly -> Convertible` (requiere plan de migración para que las notas blindadas sigan siendo gastables).
  - `ShieldedOnly -> TransparentOnly` no se permite salvo que el blinded pool este vacio o Governance codifique una migración que des-blinde notes pendientes.- Instrucciones de gobierno fijan `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` vía el ISI `ScheduleConfidentialPolicyTransition` y pueden abortar cambios programados con `CancelConfidentialPolicyTransition`. La validación del mempool asegura que ninguna transacción cruce la altura de transición y la inclusión falla de forma determinista si un check de política cambiaria a mitad de bloque.
- Las transiciones pendientes se aplican automáticamente cuando abre un nuevo bloque: una vez que la altura entra en la ventana de conversión (para actualizaciones `ShieldedOnly`) o alcanza `effective_height`, el runtime actualiza `AssetConfidentialPolicy`, refresca los metadatos `zk.policy` y limpia la entrada pendiente. Si queda suministro transparente cuando madura una transición `ShieldedOnly`, el runtime aborta el cambio y registra una advertencia, dejando el modo anterior intacto.
- Knobs de config `policy_transition_delay_blocks` y `policy_transition_window_blocks` fuerzan aviso mínimo y periodos de gracia para permitir conversiones de billeteras alrededor del cambio.
- `pending_transition.transition_id` también funciona como manija de auditorio; Governance debe citarlo al finalizar o cancelar transiciones para que los operadores correlacionen reportes de on/off-ramp.
- `policy_transition_window_blocks` por defecto es 720 (aprox 12 horas con tiempo de bloque de 60 s). Los nodos limitan solicitudes de gobernanza que intenten avisos más cortos.- Génesis manifiesta y flujos CLI exponen políticas actuales y pendientes. La lógica de admisión lee la política en el tiempo de ejecución para confirmar que cada instrucción confidencial está autorizada.
- Lista de verificación de migración - ver "Secuenciación de migración" abajo para el plan de actualización por etapas que sigue el Milestone M0.

#### Monitoreo de transiciones vía Torii

Wallets y auditores consultan `GET /v1/confidential/assets/{definition_id}/transitions` para inspeccionar el `AssetConfidentialPolicy` activo. El payload JSON siempre incluye el active id canonico, la última altura de bloque observada, el `current_mode` de la política, el modo efectivo en esa altura (las ventanas de conversión reportan temporalmente `Convertible`), y los identificadores esperados de `vk_set_hash`/Poseidon/Pedersen. Cuando hay una transición pendiente la respuesta también incluye:

- `transition_id` - manija de auditoria devuelta por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` y el `window_open_height` derivado (el bloque donde las billeteras deben comenzar la conversión para cut-overs ShieldedOnly).

Ejemplo de respuesta:

```json
{
  "asset_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  "block_height": 4217,
  "current_mode": "Convertible",
  "effective_mode": "Convertible",
  "vk_set_hash": "8D7A4B0A95AB1C33F04944F5D332F9A829CEB10FB0D0797E2D25AEFBAAF1155D",
  "poseidon_params_id": 7,
  "pedersen_params_id": 11,
  "pending_transition": {
    "transition_id": "BF2C6F9A4E9DF389B6F7E5E6B5487B39AE00D2A4B7C0FBF2C9FEF6D0A961C8ED",
    "previous_mode": "Convertible",
    "new_mode": "ShieldedOnly",
    "effective_height": 5000,
    "conversion_window": 720,
    "window_open_height": 4280
  }
}
```

Una respuesta `404` indica que no existe una definición de activo coincidente. Cuando no hay transición programada el campo `pending_transition` es `null`.

### Máquina de estados de política| Modo actual | Modo siguiente | Prerrequisitos | Manejo de altura efectiva | Notas |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | Governance ha activado entradas de registro de verificador/parametros. Enviar `ScheduleConfidentialPolicyTransition` con `effective_height >= current_height + policy_transition_delay_blocks`. | La transición se ejecuta exactamente en `effective_height`; La piscina protegida queda disponible de inmediato.        | Ruta por defecto para habilitar la confidencialidad mientras se mantienen flujos transparentes. |
| Sólo transparente | Sólo blindado | Igual que arriba, más `policy_transition_window_blocks >= 1`.                                                       | El runtime entra automáticamente en `Convertible` en `effective_height - policy_transition_window_blocks`; cambia a `ShieldedOnly` en `effective_height`. | Provee ventana de conversión determinista antes de deshabilitar instrucciones transparentes. || Descapotable | Sólo blindado | Transición programada con `effective_height >= current_height + policy_transition_delay_blocks`. Gobernanza DEBE certificar (`transparent_supply == 0`) vía metadatos de auditoría; el runtime lo aplica en el cut-over. | Semántica de ventana idéntica a la anterior. Si el suministro transparente es no-cero en `effective_height`, la transición aborta con `PolicyTransitionPrerequisiteFailed`. | Bloquea el activo en circulación completamente confidencial.                                |
| Sólo blindado | Descapotable | Transición programada; sin retiro de emergencia activo (`withdraw_height` no definido).                            | El estado cambia en `effective_height`; los revela rampas se reabren mientras las notas blindadas siguen siendo validas. | Usado para ventanas de mantenimiento o revisión de auditores.                            |
| Sólo blindado | Sólo transparente | Governance debe probar `shielded_supply == 0` o preparar un plan `EmergencyUnshield` firmado (firmas de auditor requeridas). | El runtime abre una ventana `Convertible` antes de `effective_height`; en esa altura las instrucciones confidenciales fallan duro y el activo vuelve al modo transparent-only. | Salida de último recurso. La transición se auto-cancela si ocurre cualquier gasto de nota confidencial durante la ventana. || Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` limpia el cambio pendiente.                                                    | `pending_transition` se elimina de inmediato.                                                                     | Mantiene el status quo; mostrado por completo.                                          |

Transiciones no listadas arriba se rechazan durante la sumisión de gobernanza. El runtime verifica los prerrequisitos justo antes de aplicar una transición programada; si fallan, devuelve el activo al modo anterior y emite `PolicyTransitionPrerequisiteFailed` vía telemetría y eventos de bloque.

### Secuencia de migración1. **Preparar registros:** activar todas las entradas de verificador y parámetros referenciados por la política objetivo. Los nodos anuncian el `conf_features` resultante para que los pares verifiquen coherencia.
2. **Programar la transición:** enviar `ScheduleConfidentialPolicyTransition` con un `effective_height` que respeta `policy_transition_delay_blocks`. Al moverse hacia `ShieldedOnly`, especifique una ventana de conversión (`window >= policy_transition_window_blocks`).
3. **Guía pública para operadores:** registrar el `transition_id` devuelto y circular un runbook de on/off-ramp. Wallets y auditores se suscriben a `/v1/confidential/assets/{id}/transitions` para conocer la altura de apertura de ventana.
4. **Aplicar ventana:** cuando abre la ventana, el runtime cambia la política a `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, y empieza a rechazar solicitudes de gobernanza en conflicto.
5. **Finalizar o abortar:** en `effective_height`, el runtime verifica los prerrequisitos de transición (supply transparente cero, sin emergencias, etc.). Si pasa, cambia la política al modo solicitado; si falla, emite `PolicyTransitionPrerequisiteFailed`, limpia la transición pendiente y deja la política sin cambios.6. **Actualizaciones de esquema:** después de una transición exitosa, el gobierno sube la versión de esquema del activo (por ejemplo, `asset_definition.v2`) y la herramienta CLI requiere `confidential_policy` para serializar manifiestos. Los documentos de actualización de génesis instruyen a los operadores a agregar configuraciones de política y huellas digitales del registro antes de reiniciar validadores.

Nuevas redes que inician con confidencialidad habilitada codifican la política deseada directamente en génesis. Aun así siguen la checklist anterior al cambiar modos post-lanzamiento para que las ventanas de conversión sigan siendo deterministas y las wallets tengan tiempo de ajustar.

### Versionado y activación de manifiesto Norito- Genesis manifests DEBEN incluir un `SetParameter` para la clave personalizada `confidential_registry_root`. El payload es Norito JSON que iguala `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omitir el campo (`null`) cuando no hay entradas activas, o proporcionar un string hex de 32 bytes (`0x...`) igual al hash producido por `compute_vk_set_hash` sobre las instrucciones de verificador enviadas en el manifiesto. Los nodos se niegan a iniciar si el parámetro falta o si el hash difiere de las escrituras de registro codificadas.
- El on-wire `ConfidentialFeatureDigest::conf_rules_version` incluye la versión del diseño del manifiesto. Para redes v1 DEBE permanecer `Some(1)` y es igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Cuando el conjunto de reglas evoluciona, sube la constante, se regenera manifiesta y despliega binarios en bloque; mezclar versiones hace que los validadores rechacen bloques con `ConfidentialFeatureDigestMismatch`.
- Manifiestos de activación DEBERIAN agrupar actualizaciones de registro, cambios de ciclo de vida de parámetros y transiciones de política para que el resumen se mantenga consistente:
  1. Aplica las mutaciones de registro planificadas (`Publish*`, `Set*Lifecycle`) en una vista offline del estado y calcula el resumen post-activación con `compute_confidential_feature_digest`.
  2. Emite `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando el hash calculado para que pares atrasados ​​puedan recuperar el digest correcto aunque se pierdan instrucciones intermedias de registro.3. Anexo las instrucciones `ScheduleConfidentialPolicyTransition`. Cada instrucción debe citar el `transition_id` emitido por gobernanza; manifests que lo omiten serán rechazados por el runtime.
  4. Persiste los bytes del manifiesto, una huella digital SHA-256 y el resumen usado en el plan de activación. Los operadores verifican los tres artefactos antes de votar el manifiesto para evitar particiones.
- Cuando los rollouts requieran un cut-over diferido, registre la altura objetivo en un parámetro personalizado acompañante (por ejemplo `custom.confidential_upgrade_activation_height`). Esto da a los auditores una prueba codificada en Norito de que los validadores respetaron la ventana de aviso antes de que el cambio de digest entrara en efecto.## Ciclo de vida de verificadores y parámetros
### Registro ZK
- El libro mayor almacena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` donde `proving_system` actualmente está fijo a `Halo2`.
- Pares `(circuit_id, version)` son globalmente únicos; el registro mantiene un índice secundario para consultas por metadatos de circuito. Intentos de registrador un par duplicado se rechazan durante la admisión.
- `circuit_id` debe ser no vacio y `public_inputs_schema_hash` debe ser provisto (típicamente un hash Blake2b-32 de la codificación canónica de entradas públicas del verificador). Admisión rechaza registros que omiten estos campos.
- Las instrucciones de gobierno incluyen:
  - `PUBLISH` para agregar una entrada `Proposed` solo con metadatos.
  - `ACTIVATE { vk_id, activation_height }` para programar activación en un límite de época.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar la altura final donde las pruebas pueden referenciar la entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para apagado de emergencia; activos afectados congelan gastos confidenciales después de retirar altura hasta que nuevas entradas se activen.
- Genesis manifests emite automáticamente un parámetro personalizado `confidential_registry_root` cuyo `vk_set_hash` coincide con entradas activas; la validacion cruza este resumen contra el estado local del registro antes de que un nodo pueda llegar a consenso.- Registrar o actualizar un verificador requiere `gas_schedule_id`; la verificación exige que la entrada de registro este `Active`, presente en el índice `(circuit_id, version)`, y que las pruebas Halo2 provenan un `OpenVerifyEnvelope` cuyo `circuit_id`, `vk_hash`, y `public_inputs_schema_hash` coinciden con el registro del registro.

### Claves de demostración
- Las claves de prueba se mantienen fuera del libro mayor pero se referencian por identificadores direccionados por contenido (`pk_cid`, `pk_hash`, `pk_len`) publicados junto a la metadatos del verificador.
- Los SDK de wallet obtienen datos de PK, verifican hashes y cachean localmente.

### Parámetros Pedersen y Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) que reflejan controles de ciclo de vida del verificador, cada uno con `params_id`, hashes de generadores/constantes, activación, deprecacion y alturas de retiro.
- Los compromisos y hashes separan dominios por `params_id` para que la rotación de parámetros nunca reutilice patrones de bits de conjuntos obsoletos; el ID se embebe en compromisos de notas y etiquetas de dominio de nulificador.
- Los circuitos soportan selección multiparámetro en tiempo de verificación; sets de parametros deprecados siguen siendo gastables hasta su `deprecation_height`, y sets retirados se rechazan exactamente en su `withdraw_height`.## Orden determinista y anuladores
- Cada activo mantiene un `CommitmentTree` con `next_leaf_index`; los bloques agregan compromisos en orden determinista: iterar transacciones en orden de bloque; dentro de cada transacción iterar salidas blindadas por `output_idx` serializado ascendente.
- `note_position` se deriva de offsets del árbol pero **no** es parte del nullifier; solo alimenta rutas de membresia dentro del testigo de prueba.
- La estabilidad del nullifier bajo reorgs esta garantizada por el diseño PRF; el input PRF enlaza `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, y los anclajes referencian raíces Merkle historicos limitados por `max_anchor_age_blocks`.## Flujo de libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - Requiere política de activo `Convertible` o `ShieldedOnly`; admisión verifica autoridad de activo, obtiene `params_id` actual, muestrea `rho`, emite compromiso, actualiza árbol Merkle.
   - Emite `ConfidentialEvent::Shielded` con el nuevo compromiso, delta de Merkle root y hash de llamada de transacción para pistas de auditoría.
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - Syscall del VM verifica la prueba usando la entrada del registro; el anfitrión asegura anuladores no usados, compromisos anexados de forma determinista, presentador reciente.
   - El libro mayor registra entradas de `NullifierSet`, almacena payloads cifrados para destinatarios/auditores y emite `ConfidentialEvent::Transferred` resumiendo anuladores, salidas ordenadas, hash de prueba y raíces Merkle.
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - Disponible solo para activos `Convertible`; la prueba valida que el valor de la nota iguala el monto revelado, el libro mayor acredita saldo transparente y quema la nota ciega marcando el anulador como gastado.
   - Emite `ConfidentialEvent::Unshielded` con el monto público, anuladores consumidos, identificadores de prueba y hash de llamada de transacción.## Adiciones al modelo de datos
- `ConfidentialConfig` (nueva sección de config) con bandera de habilitación, `assume_valid`, perillas de gas/limites, ventana de anclaje, backend de verificador.
- Esquemas `ConfidentialNote`, `ConfidentialTransfer`, y `ConfidentialMint` Norito con byte de versión explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envuelve bytes de memo AEAD con `{ version, ephemeral_pubkey, nonce, ciphertext }`, por defecto `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para el diseño XChaCha20-Poly1305.
- Los vectores canónicos de derivación de claves viven en `docs/source/confidential_key_vectors.json`; tanto el CLI como el endpoint Torii regresan contra estos accesorios.
- `asset::AssetDefinition` agrega `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste el enlace `(backend, name, commitment)` para verificadores de transferencia/unshield; la ejecucion rechaza pruebas cuya clave de verificación referenciado o inline no coincide con el compromiso registrado.
- `CommitmentTree` (por activo con puntos de control fronterizos), `NullifierSet` con llave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` almacenados en estado mundial.
- Mempool mantiene estructuras transitorias `NullifierIndex` y `AnchorIndex` para detección temprana de duplicados y comprobaciones de edad de anclaje.
- Actualizaciones de esquema Norito incluyen ordenamiento canónico para entradas públicas; Las pruebas de ida y vuelta aseguran el determinismo de codificación.- Roundtrips de payload encriptado quedaron fijados vía pruebas unitarias (`crates/iroha_data_model/src/confidential.rs`). Vectores de wallet de seguimiento adjuntarán transcripciones AEAD canónicos para auditores. `norito.md` documenta el header on-wire para el sobre.

## Integración con IVM y syscall
- Introducir syscall `VERIFY_CONFIDENTIAL_PROOF` aceptando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, y el `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - El syscall carga metadatos del verificador desde el registro, aplica limites de tamano/tiempo, cobra gas determinista, y solo aplica el delta si la prueba tiene exito.
- El anfitrión expone un rasgo de solo lectura `ConfidentialLedger` para recuperar instantáneas de Merkle root y estado de nullifier; la libreria Kotodama proporciona ayudantes de montaje de testigo y validación de esquema.
- Los documentos de pointer-ABI se actualizan para aclarar el diseño del buffer de prueba y los identificadores de registro.## Negociación de capacidades de nodo
- El apretón de manos anuncia `feature_bits.confidential` junto con `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participación de validadores requiere `confidential.enabled=true`, `assume_valid=false`, identificadores de backend de verificador identicos y digests coincidentes; discrepancias fallan el handshake con `HandshakeConfidentialMismatch`.
- La config soporta `assume_valid` solo para nodos observers: cuando esta deshabilitado, encontrar instrucciones confidenciales produce `UnsupportedInstruction` determinista sin pánico; cuando esté habilitado, los observadores aplican deltas declarados sin verificar pruebas.
- Mempool rechaza transacciones confidenciales si la capacidad local está deshabilitada. Los filtros de chismes evitan enviar transacciones ciegas a pares sin capacidades coincidentes mientras reenvian a ciegas IDs de verificador desconocidos dentro de límites de tamano.

### Matriz de apretón de manos| Anuncio remoto | Resultado para nodos validadores | Notas para operadores |
|---------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, el backend coincide, el resumen coincide | aceptado | El peer llega al estado `Ready` y participa en propuesta, voto y fan-out RBC. No se requiere acción manual. |
| `enabled=true`, `assume_valid=false`, el backend coincide, el resumen está obsoleto o faltante | Rechazado (`HandshakeConfidentialMismatch`) | El control remoto debe aplicar activaciones pendientes de registro/parámetros o esperar el `activation_height` programado. Hasta corregir, el nudo sigue visible pero nunca entra en rotación de consenso. |
| `enabled=true`, `assume_valid=true` | Rechazado (`HandshakeConfidentialMismatch`) | Los validadores requieren verificación de pruebas; configure el control remoto como observador con ingreso solo Torii o cambia `assume_valid=false` después de habilitar verificación completa. |
| `enabled=false`, campos omitidos (build desactualizado), o backend de verificador distinto | Rechazado (`HandshakeConfidentialMismatch`) | Peers desactualizados o parcialmente actualizados no pueden unirse a la red de consenso. Actualizalos al release actual y asegura que el tuple backend + digest coincide antes de reconectar. |Los observadores que intencionalmente omiten la verificación de pruebas no deben abrir conexiones de consenso contra validadores con puertas de capacidad. Aún pueden ingerir bloques vía Torii o API de archivo, pero la red de consenso los rechaza hasta que coinciden cien capacidades.

### Politica de poda de revelaciones y retencion de anuladores

Los libros de contabilidad confidenciales deben retener suficiente historial para probar la frescura de las notas y reproducir auditorías impulsadas por la gobernanza. La política por defecto, aplicada por `ConfidentialLedger`, es:- **Retencion de nullifiers:** mantener nullifiers gastados por un *minimo* de `730` dias (24 meses) despues de la altura de gasto, o la ventana regulatoria obligatoria si es mayor. Los operadores pueden extender la ventana vía `confidential.retention.nullifier_days`. Nullifiers mas recientes que la ventana DEBEN seguir consultables vía Torii para que auditores prueben ausencia de doble gasto.
- **Poda de velos:** los velos transparentes (`RevealConfidential`) podan los compromisos asociados inmediatamente después de que el bloque finaliza, pero el nulificador consumido sigue sujeto a la regla de retención anterior. Eventos relacionados con revelación (`ConfidentialEvent::Unshielded`) registran el monto público, destinatario y hash de prueba para que reconstruir revela históricos no requieren el texto cifrado podado.
- **Puntos de control fronterizos:** las fronteras de compromiso mantienen puntos de control en rodadura que cubren el mayor de `max_anchor_age_blocks` y la ventana de retención. Los nodos compactan checkpoints mas antiguos solo despues de que todos los nullifiers dentro del intervalo expiran.- **Remediación por digest stale:** si se levanta `HandshakeConfidentialMismatch` por deriva de digest, los operadores deben (1) verificar que las ventanas de retención de nullifiers coinciden en el cluster, (2) ejecutar `iroha_cli app confidential verify-ledger` para regenerar el digest contra el conjunto de nullifiers retenidos, y (3) redeployar el manifest actualizado. Cualquier anulador podado prematuramente debe restaurarse desde almacenamiento frío antes de reingresar a la red.

La documentación anula las configuraciones regionales en el runbook de operaciones; las políticas de gobernanza que extienden la ventana de retención deben actualizar la configuración de nodo y planos de almacenamiento de archivo en lockstep.

### Flujo de desalojo y recuperación1. Durante el dial, `IrohaNetwork` compara las capacidades anunciadas. Cualquier discrepancia levanta `HandshakeConfidentialMismatch`; la conexión se cierra y el peer permanece en la cola de descubrimiento sin ser promovido a `Ready`.
2. El fallo se exponen vía el log del servicio de red (incluye digest remoto y backend), y Sumeragi nunca programa al peer para propuesta o voto.
3. Los operadores remedian alineando registros de verificador y conjuntos de parámetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) o programando `next_conf_features` con un `activation_height` acordado. Una vez que el resumen coincide, el siguiente apretón de manos sale automáticamente.
4. Si un peer stale logra difundir un bloque (por ejemplo, vía repetición de archivo), los validadores lo rechazan de forma determinista con `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, manteniendo el estado del ledger consistente en la red.

### Flujo de apretón de manos seguro ante repetición1. Cada intento saliente asigna material de clave Noise/X25519 nuevo. El payload de handshake que se firma (`handshake_signature_payload`) concatena las claves públicas efimeras local y remota, la dirección de socket anunciada codificada en Norito y, cuando se compila con `handshake_chain_id`, el identificador de cadena. El mensaje se encripta con AEAD antes de salir del nodo.
2. El receptor recomputa el payload con el orden de claves peer/local invertido y verifica la firma Ed25519 embebida en `HandshakeHelloV1`. Debido a que ambas claves efimeras y la dirección anunciada forman parte del dominio de firma, reproducir un mensaje capturado contra otro par o recuperar una conexión obsoleta falla la verificación de forma determinista.
3. Flags de capacidad confidencial y el `ConfidentialFeatureDigest` viajan dentro de `HandshakeConfidentialMeta`. El receptor compara la tupla `{ enabled, assume_valid, verifier_backend, digest }` contra su `ConfidentialHandshakeCaps` local; cualquier desajuste sale temprano con `HandshakeConfidentialMismatch` antes de que el transporte transicione a `Ready`.
4. Los operadores DEBEN recomputar el digest (vía `compute_confidential_feature_digest`) y reiniciar nodos con los registries/politicas actualizados antes de reconectar. Peers que anuncian digests antiguos siguen fallando el handshake, evitando que estado stale reingrese al conjunto de validadores.5. Éxitos y fallos del handshake actualizan los contadores estándar `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomía de errores) y emiten registros estructurados con el peer ID remoto y la huella digital del digest. Monitorea estos indicadores para detectar repeticiones o configuraciones incorrectas durante el lanzamiento.

## Gestión de claves y cargas útiles
- Jerarquía de derivación por cuenta:
  - `sk_spend` -> `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Payloads de notas encriptadas usan AEAD con claves compartidas derivadas por ECDH; se pueden adjuntar ver claves de auditores opcionales a salidas según la política del activo.
- Adiciones al CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, herramientas de auditor para descifrar memos, y el ayudante `iroha app zk envelope` para producir/inspeccionar sobres Norito fuera de línea. Torii exponen el mismo flujo de derivación vía `POST /v1/confidential/derive-keyset`, retornando formas hexadecimal y base64 para que wallets obtengan jerarquias de claves programáticamente.## Gas, límites y controles DoS
- Horario determinista de gas:
  - Halo2 (Plonkish): gas base `250_000` + gas `2_000` por entrada del público.
  - `5` gas por byte de prueba, mas cargas por anulador (`300`) y por compromiso (`500`).
  - Los operadores pueden sobrescribir estas constantes vía la configuración del nodo (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); los cambios se propagan al inicio o cuando la capa de config hot-reload y se aplica de forma determinista en el cluster.
- Limites duros (configurables por defecto):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Pruebas que exceden `verify_timeout_ms` abortan la instrucción de forma determinista (ballots de Governance emiten `proof verification exceeded timeout`, `VerifyProof` retorna error).
- Cuotas adicionales aseguran vida útil: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, y `max_public_inputs` acotan block builders; `reorg_depth_bound` (>= `max_anchor_age_blocks`) gobierna la retención de puntos de control fronterizos.
- La ejecución runtime ahora rechaza transacciones que exceden estos límites por transacción o por bloque, emitiendo errores `InvalidParameter` deterministas y dejando el estado del libro mayor sin cambios.
- Mempool prefiltro transacciones confidenciales por `vk_id`, longitud de prueba y edad de anclaje antes de invocar el verificador para mantener acotado el uso de recursos.- La verificación se detiene de forma determinista en tiempo de espera o violación de límites; las transacciones fallan con errores explícitos. Los backends SIMD son opcionales pero no alteran la contabilidad de gas.

### Líneas base de calibración y puertas de aceptación
- **Plataformas de referencia.** Las corridas de calibración DEBEN cubren los tres perfiles de hardware abajo. Corridas que no capturan todos los perfiles se rechazan durante la revisión.

  | Perfil | Arquitectura | CPU/instancia | Banderas de compilador | propuesta |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) o Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Establece valores piso sin intrinsics vector; se usa para ajustar tablas de costo fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | liberación por defecto | Valida la ruta AVX2; revisa que los speedups SIMD se mantengan dentro de la tolerancia del gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | liberación por defecto | Asegura que el backend NEON permanezca determinista y alineado con los horarios x86. |

- **Arnés de referencia.** Todos los informes de calibracion de gas DEBEN producirse con:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar el determinista del aparato.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` cuando cambian los costos de código de operación de la VM.- **Randomness fija.** Exporta `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de correr bancos para que `iroha_test_samples::gen_account_in` cambie a la ruta determinista `KeyPair::from_seed`. El arnés imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` una vez; si falta la variable, la revisión DEBE fallar. Cualquier utilidad nueva de calibración debe seguir respetando este env var al introducir aleatoriedad auxiliar.

- **Captura de resultados.**
  - Subir resmenes de Criterion (`target/criterion/**/raw.csv`) para cada perfil al artefacto de liberación.
  - Guardar métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) en el [Confidential Gas Calibration ledger](./confidential-gas-calibration) junto con el commit de git y la versión de compilador usada.
  - Mantener los últimos dos líneas base por perfil; eliminar instantáneas más antiguas una vez validado el reporte más nuevo.

- **Tolerancias de aceptación.**
  - Deltas de gas entre `baseline-simd-neutral` y `baseline-avx2` DEBEN permanecer <= +/-1.5%.
  - Deltas de gas entre `baseline-simd-neutral` y `baseline-neon` DEBEN permanecer <= +/-2.0%.
  - Las propuestas de calibración que superen estos umbrales requieren ajustes de programación o un RFC que explique la discrepancia y su mitigación.- **Lista de verificación de revisión.** Los remitentes son responsables de:
  - Incluye `uname -a`, extractos de `/proc/cpuinfo` (modelo, paso a paso), e `rustc -Vv` en el registro de calibración.
  - Verificar que `IROHA_CONF_GAS_SEED` se vea en la salida de bench (las benches imprimen la semilla activa).
  - Asegurar que los feature flags del marcapasos y del verificador confidencial espejen producción (`--features confidential,telemetry` al correr bancos con Telemetría).

## Configurar y operaciones
- `iroha_config` agrega la sección `[confidential]`:
  ```toml
  [confidential]
  enabled = true
  assume_valid = false
  verifier_backend = "ark_bls12_381"
  max_proof_size_bytes = 262144
  max_nullifiers_per_tx = 8
  max_commitments_per_tx = 8
  max_confidential_ops_per_block = 256
  verify_timeout_ms = 750
  max_anchor_age_blocks = 10000
  max_proof_bytes_block = 1048576
  max_verify_calls_per_tx = 4
  max_verify_calls_per_block = 128
  max_public_inputs = 32
  reorg_depth_bound = 10000
  policy_transition_delay_blocks = 100
  policy_transition_window_blocks = 200
  tree_roots_history_len = 10000
  tree_frontier_checkpoint_interval = 100
  registry_max_vk_entries = 64
  registry_max_params_entries = 32
  registry_max_delta_per_block = 4
  ```
- Telemetria emite métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, y `confidential_policy_transitions_total`, nunca exponiendo datos en claro.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Estrategia de pruebas
- Determinismo: el shuffle aleatorio de transacciones dentro de bloques produce raíces Merkle y conjuntos de nulificadores idénticos.
- Resiliencia a reorg: reorganizaciones similares multi-bloque con anclajes; Los anuladores se mantienen estables y los anclajes rancios se rechazan.
- Invariantes de gas: verificar uso de gas idéntico entre nodos con y sin aceleración SIMD.
- Pruebas de borde: pruebas en techos de tamano/gas, conteos máximos de entrada/salida, cumplimiento de tiempo de espera.
- Ciclo de vida: operaciones de gobierno para activación/deprecacion de verificador y parámetros, pruebas de gasto tras rotación.
- Política FSM: transiciones permitidas/no permitidas, retrasos de transición pendiente y rechazo de mempool alrededor de alturas efectivas.
- Emergencias de registro: retiro de emergencia congela activos afectados en `withdraw_height` y rechaza pruebas después.
- Capability gating: validadores con `conf_features` no coincidentes rechazan bloques; observers con `assume_valid=true` avanzan sin afectar consenso.
- Equivalencia de estado: nodos validator/full/observer producen raíces de estado idénticos en la cadena canónica.
- Fuzzing negativo: pruebas malformadas, cargas útiles sobredimensionadas y colisiones de anulador se rechazan deterministamente.## Migración
- Lanzamiento del indicador de función: hasta que se complete la Fase C3, `enabled` por defecto es `false`; los nodos anuncian capacidades antes de unirse al conjunto validador.
- Bienes transparentes no se afectan; las instrucciones confidenciales requieren entradas de registro y negociación de capacidades.
- Nodos compilados sin soporte confidencial rechazan bloques relevantes de forma determinista; no pueden unirse al conjunto validador pero pueden operar como observadores con `assume_valid=true`.
- Los manifiestos de génesis incluyen entradas iniciales del registro, conjuntos de parámetros, políticas confidenciales para activos y claves de auditor opcionales.
- Los operadores siguen runbooks publicados para rotación de registro, transiciones de política y retiro de emergencia para mantener actualizaciones deterministas.## Trabajo pendiente
- Benchmarks de parámetros Halo2 (tamano de circuito, estrategia de búsqueda) y registrar resultados en el playbook de calibración para que defaults de gas/timeout se actualicen junto al próximo refresco de `confidential_assets_calibration.md`.
- Finalizar políticas de divulgación de auditor y APIs de visualización selectiva asociadas, conectando el flujo aprobado en Torii una vez que el borrador de gobernanza se firme.
- Extender el esquema de cifrado de testigos para cubrir salidas de múltiples destinatarios y notas por lotes, documentando el formato del sobre para implementadores de SDK.
- Encargar una revisión de seguridad externa de circuitos, registros y procedimientos de rotación de parámetros y archivar hallazgos junto a los informes internos de auditoría.
- Especificar API de reconciliación de gasto para auditores y publicar guía de alcance de view-key para que los proveedores de billeteras implementen las mismas semánticas de atestación.## Fases de implementación
1. **Fase M0 - Endurecimiento Parada-Envío**
   - [x] La derivacion de nullifier ahora sigue el diseño Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) con orden determinista de compromisos forzado en actualizaciones del libro mayor.
   - [x] La ejecucion aplica limites de tamaño de prueba y cuotas confidenciales por transacción/por bloque, rechazando transacciones fuera de presupuesto con errores deterministas.
   - [x] El handshake P2P anuncia `ConfidentialFeatureDigest` (digest de backend + huellas dactilares de registro) y falla desajustes de forma determinista vía `HandshakeConfidentialMismatch`.
   - [x] Remover pánicos en caminos de ejecucion confidencial y agregar role gating para nodos sin soporte.
   - [ ] Aplicar presupuestos de tiempo de espera de verificador y límites de profundidad de reorganización para puntos de control fronterizos.
     - [x] Presupuestos de tiempo de espera de verificación aplicados; pruebas que exceden `verify_timeout_ms` ahora fallan deterministamente.
     - [x] Frontier checkpoints ahora respetan `reorg_depth_bound`, podando checkpoints mas antiguos que la ventana configurada mientras mantienen instantáneas deterministas.
   - Introducir `AssetConfidentialPolicy`, política FSM y puertas de cumplimiento para instrucciones mint/transfer/reveal.
   - Comprómetro `conf_features` en encabezados de bloque y rechazar participación de validadores cuando los resúmenes de registro/parámetros divergen.2. **Fase M1 - Registros y parámetros**
   - Entregar registros `ZkVerifierEntry`, `PedersenParams`, y `PoseidonParams` con operaciones de gobernanza, anclaje de génesis y manejo de caché.
   - Conectar syscall para requerir búsquedas de registro, ID de programación de gas, hash de esquema y comprobaciones de tamano.
   - Enviar formato de carga útil encriptado v1, vectores de derivación de claves para billetera, y soporte CLI para gestión de claves confidenciales.
3. **Fase M2 - Gas y rendimiento**
   - Implementar cronograma de gas determinista, contadores por bloque, y arneses de benchmark con telemetria (latencia de verificación, tamanos de prueba, rechazos de mempool).
   - Endurecer puntos de control de CommitmentTree, carga LRU e índices de anulación para cargas de trabajo de múltiples activos.
4. **Fase M3 - Rotación y utillaje de billetera**
   - Habilitar aceptación de pruebas multiparámetro y multiversión; soportar activación/deprecacion impulsada por gobernanza con runbooks de transición.
   - Entregar flujos de migración en SDK/CLI, flujos de trabajo de escaneo de auditor y herramientas de reconciliación de gastos.
5. **Fase M4 - Auditoría y operaciones**
   - Proveer flujos de trabajo de claves de auditor, APIs de divulgación selectiva y runbooks operativos.
   - Programar revisión externa de criptografía/seguridad y publicar resultados en `status.md`.Cada fase actualiza hitos del roadmap y pruebas asociadas para mantener garantías de ejecución determinista en la red blockchain.