---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/nexus/confidential-assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 615362bc50379e6e4bb68a3926d65677df3a084880bb6d476a6fb35ea5b64876
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Activos confidenciales y transferencias ZK
description: Plano de Phase C para circulacion blindada, registros y controles de operador.
slug: /nexus/confidential-assets
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseno de activos confidenciales y transferencias ZK

## Motivacion
- Entregar flujos de activos blindados opt-in para que los dominios preserven privacidad transaccional sin alterar la circulacion transparente.
- Mantener ejecucion determinista en hardware heterogeneo de validadores y conservar Norito/Kotodama ABI v1.
- Proveer a auditores y operadores controles de ciclo de vida (activacion, rotacion, revocacion) para circuitos y parametros criptograficos.

## Modelo de amenazas
- Los validadores son honest-but-curious: ejecutan consenso fielmente pero intentan inspeccionar ledger/state.
- Observadores de red ven datos de bloque y transacciones gossiped; no se asumen canales de gossip privados.
- Fuera de alcance: analisis de trafico off-ledger, adversarios cuanticos (seguido en el roadmap PQ), ataques de disponibilidad del ledger.

## Resumen de diseno
- Los activos pueden declarar un *shielded pool* ademas de los balances transparentes existentes; la circulacion blindada se representa via commitments criptograficos.
- Las notes encapsulan `(asset_id, amount, recipient_view_key, blinding, rho)` con:
  - Commitment: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Nullifier: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independiente del orden de las notes.
  - Payload encriptado: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Las transacciones transportan payloads `ConfidentialTransfer` codificados con Norito que contienen:
  - Inputs publicos: Merkle anchor, nullifiers, nuevos commitments, asset id, version de circuito.
  - Payloads encriptados para recipients y auditores opcionales.
  - Prueba zero-knowledge que atesta conservacion de valor, ownership y autorizacion.
- Verifying keys y conjuntos de parametros son controlados mediante registros on-ledger con ventanas de activacion; los nodos rechazan validar proofs que referencian entradas desconocidas o revocadas.
- Los headers de consenso comprometen el digest activo de capacidades confidenciales para que los bloques solo se acepten cuando el estado de registros y parametros coincida.
- La construccion de proofs usa un stack Halo2 (Plonkish) sin trusted setup; Groth16 u otras variantes SNARK se consideran intencionalmente no soportadas en v1.

### Fixtures deterministas

Los sobres de memo confidenciales ahora incluyen un fixture canonico en `fixtures/confidential/encrypted_payload_v1.json`. El dataset captura un sobre v1 positivo mas muestras negativas malformadas para que los SDKs puedan afirmar paridad de parsing. Los tests del data-model en Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y la suite de Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) cargan el fixture directamente, garantizando que el encoding Norito, las superficies de error y la cobertura de regresion permanezcan alineadas mientras evoluciona el codec.

Los SDKs de Swift ahora pueden emitir instrucciones shield sin glue JSON bespoke: construye un
`ShieldRequest` con el commitment de 32 bytes, el payload encriptado y metadata de debito,
luego llama `IrohaSDK.submit(shield:keypair:)` (o `submitAndWait`) para firmar y enviar la
transaccion sobre `/v1/pipeline/transactions`. El helper valida longitudes de commitment,
enhebra `ConfidentialEncryptedPayload` en el encoder Norito, y refleja el layout `zk::Shield`
descrito abajo para que los wallets se mantengan sincronizados con Rust.

## Commitments de consenso y capability gating
- Los headers de bloque exponen `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; el digest participa en el hash de consenso y debe igualar la vista local del registry para aceptar el bloque.
- Governance puede preparar upgrades programando `next_conf_features` con un `activation_height` futuro; hasta esa altura, los productores de bloques deben seguir emitiendo el digest previo.
- Los nodos validadores DEBEN operar con `confidential.enabled = true` y `assume_valid = false`. Los checks de inicio rechazan unirse al set de validadores si cualquiera falla o si el `conf_features` local diverge.
- El metadata del handshake P2P ahora incluye `{ enabled, assume_valid, conf_features }`. Peers que anuncien features no soportadas se rechazan con `HandshakeConfidentialMismatch` y nunca entran en rotacion de consenso.
- Los resultados de handshake entre validadores, observers y peers se capturan en la matriz de handshake bajo [Node Capability Negotiation](#node-capability-negotiation). Los fallos de handshake exponen `HandshakeConfidentialMismatch` y mantienen al peer fuera de la rotacion de consenso hasta que su digest coincida.
- Observers no validadores pueden fijar `assume_valid = true`; aplican deltas confidenciales a ciegas pero no influyen en la seguridad del consenso.

## Politicas de assets
- Cada definicion de asset lleva un `AssetConfidentialPolicy` fijado por el creador o via governance:
  - `TransparentOnly`: modo por defecto; solo se permiten instrucciones transparentes (`MintAsset`, `TransferAsset`, etc.) y las operaciones shielded se rechazan.
  - `ShieldedOnly`: toda emision y transferencias deben usar instrucciones confidenciales; `RevealConfidential` esta prohibido para que los balances nunca se expongan publicamente.
  - `Convertible`: los holders pueden mover valor entre representaciones transparentes y shielded usando las instrucciones de on/off-ramp de abajo.
- Las politicas siguen un FSM restringido para evitar fondos varados:
  - `TransparentOnly -> Convertible` (habilitacion inmediata del shielded pool).
  - `TransparentOnly -> ShieldedOnly` (requiere transicion pendiente y ventana de conversion).
  - `Convertible -> ShieldedOnly` (demora minima obligatoria).
  - `ShieldedOnly -> Convertible` (requiere plan de migracion para que las notes blindadas sigan siendo gastables).
  - `ShieldedOnly -> TransparentOnly` no se permite salvo que el shielded pool este vacio o governance codifique una migracion que des-blinde notes pendientes.
- Instrucciones de governance fijan `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` via el ISI `ScheduleConfidentialPolicyTransition` y pueden abortar cambios programados con `CancelConfidentialPolicyTransition`. La validacion del mempool asegura que ninguna transaccion cruce la altura de transicion y la inclusion falla de forma determinista si un check de politica cambiaria a mitad de bloque.
- Las transiciones pendientes se aplican automaticamente cuando abre un nuevo bloque: una vez que la altura entra en la ventana de conversion (para upgrades `ShieldedOnly`) o alcanza `effective_height`, el runtime actualiza `AssetConfidentialPolicy`, refresca la metadata `zk.policy` y limpia la entrada pendiente. Si queda supply transparente cuando madura una transicion `ShieldedOnly`, el runtime aborta el cambio y registra una advertencia, dejando el modo previo intacto.
- Knobs de config `policy_transition_delay_blocks` y `policy_transition_window_blocks` fuerzan aviso minimo y periodos de gracia para permitir conversiones de wallets alrededor del cambio.
- `pending_transition.transition_id` tambien funciona como handle de auditoria; governance debe citarlo al finalizar o cancelar transiciones para que los operadores correlacionen reportes de on/off-ramp.
- `policy_transition_window_blocks` default a 720 (aprox 12 horas con block time de 60 s). Los nodos limitan solicitudes de governance que intenten avisos mas cortos.
- Genesis manifests y flujos CLI exponen politicas actuales y pendientes. La logica de admission lee la politica en tiempo de ejecucion para confirmar que cada instruccion confidencial esta autorizada.
- Checklist de migracion - ver "Migration sequencing" abajo para el plan de upgrade por etapas que sigue el Milestone M0.

#### Monitoreo de transiciones via Torii

Wallets y auditores consultan `GET /v1/confidential/assets/{definition_id}/transitions` para inspeccionar el `AssetConfidentialPolicy` activo. El payload JSON siempre incluye el asset id canonico, la ultima altura de bloque observada, el `current_mode` de la politica, el modo efectivo en esa altura (las ventanas de conversion reportan temporalmente `Convertible`), y los identificadores esperados de `vk_set_hash`/Poseidon/Pedersen. Cuando hay una transicion pendiente la respuesta tambien incluye:

- `transition_id` - handle de auditoria devuelto por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` y el `window_open_height` derivado (el bloque donde wallets deben comenzar conversion para cut-overs ShieldedOnly).

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

Una respuesta `404` indica que no existe una definicion de asset coincidente. Cuando no hay transicion programada el campo `pending_transition` es `null`.

### Maquina de estados de politica

| Modo actual       | Modo siguiente     | Prerrequisitos                                                                 | Manejo de altura efectiva                                                                                         | Notas                                                                                     |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------|
| TransparentOnly    | Convertible      | Governance ha activado entradas de registro de verificador/parametros. Enviar `ScheduleConfidentialPolicyTransition` con `effective_height >= current_height + policy_transition_delay_blocks`. | La transicion se ejecuta exactamente en `effective_height`; el shielded pool queda disponible de inmediato.        | Ruta por defecto para habilitar confidencialidad mientras se mantienen flujos transparentes. |
| TransparentOnly    | ShieldedOnly     | Igual que arriba, mas `policy_transition_window_blocks >= 1`.                                                       | El runtime entra automaticamente en `Convertible` en `effective_height - policy_transition_window_blocks`; cambia a `ShieldedOnly` en `effective_height`. | Provee ventana de conversion determinista antes de deshabilitar instrucciones transparentes. |
| Convertible        | ShieldedOnly     | Transicion programada con `effective_height >= current_height + policy_transition_delay_blocks`. Governance DEBE certificar (`transparent_supply == 0`) via metadata de auditoria; el runtime lo aplica en el cut-over. | Semantica de ventana identica a la anterior. Si el supply transparente es no-cero en `effective_height`, la transicion aborta con `PolicyTransitionPrerequisiteFailed`. | Bloquea el asset en circulacion completamente confidencial.                                |
| ShieldedOnly       | Convertible      | Transicion programada; sin retiro de emergencia activo (`withdraw_height` no definido).                            | El estado cambia en `effective_height`; los reveal ramps se reabren mientras las notes blindadas siguen siendo validas. | Usado para ventanas de mantenimiento o revisiones de auditores.                            |
| ShieldedOnly       | TransparentOnly  | Governance debe probar `shielded_supply == 0` o preparar un plan `EmergencyUnshield` firmado (firmas de auditor requeridas). | El runtime abre una ventana `Convertible` antes de `effective_height`; en esa altura las instrucciones confidenciales fallan duro y el asset vuelve al modo transparent-only. | Salida de ultimo recurso. La transicion se auto-cancela si ocurre cualquier gasto de note confidencial durante la ventana. |
| Any                | Same as current  | `CancelConfidentialPolicyTransition` limpia el cambio pendiente.                                                    | `pending_transition` se elimina de inmediato.                                                                     | Mantiene el status quo; mostrado por completitud.                                          |

Transiciones no listadas arriba se rechazan durante el submission de governance. El runtime verifica los prerrequisitos justo antes de aplicar una transicion programada; si fallan, devuelve el asset al modo previo y emite `PolicyTransitionPrerequisiteFailed` via telemetria y eventos de bloque.

### Secuencia de migracion

1. **Preparar registros:** activar todas las entradas de verificador y parametros referenciados por la politica objetivo. Los nodos anuncian el `conf_features` resultante para que los peers verifiquen coherencia.
2. **Programar la transicion:** enviar `ScheduleConfidentialPolicyTransition` con un `effective_height` que respete `policy_transition_delay_blocks`. Al moverse hacia `ShieldedOnly`, especificar una ventana de conversion (`window >= policy_transition_window_blocks`).
3. **Publicar guia para operadores:** registrar el `transition_id` devuelto y circular un runbook de on/off-ramp. Wallets y auditores se suscriben a `/v1/confidential/assets/{id}/transitions` para conocer la altura de apertura de ventana.
4. **Aplicar ventana:** cuando abre la ventana, el runtime cambia la politica a `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, y empieza a rechazar requests de governance en conflicto.
5. **Finalizar o abortar:** en `effective_height`, el runtime verifica los prerrequisitos de transicion (supply transparente cero, sin emergencias, etc.). Si pasa, cambia la politica al modo solicitado; si falla, emite `PolicyTransitionPrerequisiteFailed`, limpia la transicion pendiente y deja la politica sin cambios.
6. **Upgrades de schema:** despues de una transicion exitosa, governance sube la version de schema del asset (por ejemplo, `asset_definition.v2`) y el tooling CLI requiere `confidential_policy` al serializar manifests. Los docs de upgrade de genesis instruyen a operadores a agregar settings de politica y fingerprints del registry antes de reiniciar validadores.

Nuevas redes que inician con confidencialidad habilitada codifican la politica deseada directamente en genesis. Aun asi siguen la checklist anterior al cambiar modos post-launch para que las ventanas de conversion sigan siendo deterministas y los wallets tengan tiempo de ajustar.

### Versionado y activacion de manifest Norito

- Genesis manifests DEBEN incluir un `SetParameter` para la key custom `confidential_registry_root`. El payload es Norito JSON que iguala `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omitir el campo (`null`) cuando no hay entradas activas, o proveer un string hex de 32 bytes (`0x...`) igual al hash producido por `compute_vk_set_hash` sobre las instrucciones de verificador enviadas en el manifest. Los nodos se niegan a iniciar si el parametro falta o si el hash difiere de las escrituras de registry codificadas.
- El on-wire `ConfidentialFeatureDigest::conf_rules_version` embebe la version del layout del manifest. Para redes v1 DEBE permanecer `Some(1)` y es igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Cuando el ruleset evolucione, sube la constante, regenera manifests y despliega binarios en lock-step; mezclar versiones hace que los validadores rechacen bloques con `ConfidentialFeatureDigestMismatch`.
- Activation manifests DEBERIAN agrupar actualizaciones de registry, cambios de ciclo de vida de parametros y transiciones de politica para que el digest se mantenga consistente:
  1. Aplica las mutaciones de registry planificadas (`Publish*`, `Set*Lifecycle`) en una vista offline del estado y calcula el digest post-activacion con `compute_confidential_feature_digest`.
  2. Emite `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando el hash calculado para que peers atrasados puedan recuperar el digest correcto aunque se pierdan instrucciones intermedias de registry.
  3. Anexa las instrucciones `ScheduleConfidentialPolicyTransition`. Cada instruccion debe citar el `transition_id` emitido por governance; manifests que lo omiten seran rechazados por el runtime.
  4. Persiste los bytes del manifest, un fingerprint SHA-256 y el digest usado en el plan de activacion. Los operadores verifican los tres artefactos antes de votar el manifest para evitar particiones.
- Cuando los rollouts requieran un cut-over diferido, registra la altura objetivo en un parametro custom acompanante (por ejemplo `custom.confidential_upgrade_activation_height`). Esto da a los auditores una prueba codificada en Norito de que los validadores respetaron la ventana de aviso antes de que el cambio de digest entrara en efecto.

## Ciclo de vida de verificadores y parametros
### Registro ZK
- El ledger almacena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` donde `proving_system` actualmente esta fijo a `Halo2`.
- Pares `(circuit_id, version)` son globalmente unicos; el registry mantiene un indice secundario para consultas por metadata de circuito. Intentos de registrar un par duplicado se rechazan durante admission.
- `circuit_id` debe ser no vacio y `public_inputs_schema_hash` debe ser provisto (tipicamente un hash Blake2b-32 del encoding canonico de public inputs del verificador). Admission rechaza registros que omiten estos campos.
- Las instrucciones de governance incluyen:
  - `PUBLISH` para agregar una entrada `Proposed` solo con metadata.
  - `ACTIVATE { vk_id, activation_height }` para programar activacion en un limite de epoch.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar la altura final donde proofs pueden referenciar la entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para apagado de emergencia; assets afectados congelan gastos confidenciales despues de withdraw height hasta que nuevas entradas se activen.
- Genesis manifests emiten automaticamente un parametro custom `confidential_registry_root` cuyo `vk_set_hash` coincide con entradas activas; la validacion cruza este digest contra el estado local del registry antes de que un nodo pueda unirse a consenso.
- Registrar o actualizar un verificador requiere `gas_schedule_id`; la verificacion exige que la entrada de registry este `Active`, presente en el indice `(circuit_id, version)`, y que los proofs Halo2 provean un `OpenVerifyEnvelope` cuyo `circuit_id`, `vk_hash`, y `public_inputs_schema_hash` coincidan con el registro del registry.

### Proving Keys
- Los proving keys se mantienen off-ledger pero se referencian por identificadores direccionados por contenido (`pk_cid`, `pk_hash`, `pk_len`) publicados junto a la metadata del verificador.
- Los SDKs de wallet obtienen datos de PK, verifican hashes, y cachean localmente.

### Parametros Pedersen y Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) reflejan controles de ciclo de vida del verificador, cada uno con `params_id`, hashes de generadores/constantes, activacion, deprecacion y alturas de retiro.
- Commitments y hashes separan dominios por `params_id` para que la rotacion de parametros nunca reutilice patrones de bits de sets deprecados; el ID se embebe en commitments de notes y tags de dominio de nullifier.
- Los circuitos soportan seleccion multi-parametro en tiempo de verificacion; sets de parametros deprecados siguen siendo gastables hasta su `deprecation_height`, y sets retirados se rechazan exactamente en su `withdraw_height`.

## Orden determinista y nullifiers
- Cada asset mantiene un `CommitmentTree` con `next_leaf_index`; los bloques agregan commitments en orden determinista: iterar transacciones en orden de bloque; dentro de cada transaccion iterar outputs blindados por `output_idx` serializado ascendente.
- `note_position` se deriva de offsets del arbol pero **no** es parte del nullifier; solo alimenta rutas de membresia dentro del witness de prueba.
- La estabilidad del nullifier bajo reorgs esta garantizada por el diseno PRF; el input PRF enlaza `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, y los anchors referencian roots Merkle historicos limitados por `max_anchor_age_blocks`.

## Flujo de ledger
1. **MintConfidential { asset_id, amount, recipient_hint }**
   - Requiere politica de asset `Convertible` o `ShieldedOnly`; admission verifica autoridad de asset, obtiene `params_id` actual, muestrea `rho`, emite commitment, actualiza arbol Merkle.
   - Emite `ConfidentialEvent::Shielded` con el nuevo commitment, delta de Merkle root y hash de llamada de transaccion para audit trails.
2. **TransferConfidential { asset_id, proof, circuit_id, version, nullifiers, new_commitments, enc_payloads, anchor_root, memo }**
   - Syscall del VM verifica proof usando la entrada del registry; el host asegura nullifiers no usados, commitments anexados de forma determinista, anchor reciente.
   - El ledger registra entradas de `NullifierSet`, almacena payloads encriptados para recipients/auditores y emite `ConfidentialEvent::Transferred` resumiendo nullifiers, outputs ordenados, hash de proof y Merkle roots.
3. **RevealConfidential { asset_id, proof, circuit_id, version, nullifier, amount, recipient_account, anchor_root }**
   - Disponible solo para assets `Convertible`; la proof valida que el valor de la note iguala el monto revelado, el ledger acredita balance transparente y quema la note blindada marcando el nullifier como gastado.
   - Emite `ConfidentialEvent::Unshielded` con el monto publico, nullifiers consumidos, identificadores de proof y hash de llamada de transaccion.

## Adiciones al data model
- `ConfidentialConfig` (nueva seccion de config) con flag de habilitacion, `assume_valid`, knobs de gas/limites, ventana de anchor, backend de verificador.
- `ConfidentialNote`, `ConfidentialTransfer`, y `ConfidentialMint` schemas Norito con byte de version explicito (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envuelve bytes de memo AEAD con `{ version, ephemeral_pubkey, nonce, ciphertext }`, con default `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para el layout XChaCha20-Poly1305.
- Vectores canonicos de key-derivation viven en `docs/source/confidential_key_vectors.json`; tanto el CLI como el endpoint Torii regresan contra estos fixtures.
- `asset::AssetDefinition` agrega `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste el binding `(backend, name, commitment)` para verifiers de transfer/unshield; la ejecucion rechaza proofs cuyo verifying key referenciado o inline no coincida con el commitment registrado.
- `CommitmentTree` (por asset con frontier checkpoints), `NullifierSet` con llave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` almacenados en world state.
- Mempool mantiene estructuras transitorias `NullifierIndex` y `AnchorIndex` para deteccion temprana de duplicados y checks de edad de anchor.
- Actualizaciones de schema Norito incluyen ordering canonico para public inputs; tests de round-trip aseguran determinismo de encoding.
- Roundtrips de payload encriptado quedan fijados via unit tests (`crates/iroha_data_model/src/confidential.rs`). Vectores de wallet de seguimiento adjuntaran transcripts AEAD canonicos para auditores. `norito.md` documenta el header on-wire para el envelope.

## Integracion con IVM y syscall
- Introducir syscall `VERIFY_CONFIDENTIAL_PROOF` aceptando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, y el `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - El syscall carga metadata del verificador desde el registry, aplica limites de tamano/tiempo, cobra gas determinista, y solo aplica el delta si la proof tiene exito.
- El host expone un trait de solo lectura `ConfidentialLedger` para recuperar snapshots de Merkle root y estado de nullifier; la libreria Kotodama provee helpers de assembly de witness y validacion de schema.
- Los docs de pointer-ABI se actualizan para aclarar layout del buffer de proof y handles de registry.

## Negociacion de capacidades de nodo
- El handshake anuncia `feature_bits.confidential` junto con `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participacion de validadores requiere `confidential.enabled=true`, `assume_valid=false`, identificadores de backend de verificador identicos y digests coincidentes; discrepancias fallan el handshake con `HandshakeConfidentialMismatch`.
- La config soporta `assume_valid` solo para nodos observers: cuando esta deshabilitado, encontrar instrucciones confidenciales produce `UnsupportedInstruction` determinista sin panic; cuando esta habilitado, observers aplican deltas declarados sin verificar proofs.
- Mempool rechaza transacciones confidenciales si la capacidad local esta deshabilitada. Los filtros de gossip evitan enviar transacciones blindadas a peers sin capacidades coincidentes mientras reenvian a ciegas IDs de verificador desconocidos dentro de limites de tamano.

### Matriz de handshake

| Anuncio remoto | Resultado para nodos validadores | Notas para operadores |
|----------------------|-----------------------------|----------------|
| `enabled=true`, `assume_valid=false`, backend coincide, digest coincide | Aceptado | El peer llega al estado `Ready` y participa en propuesta, voto y fan-out RBC. No se requiere accion manual. |
| `enabled=true`, `assume_valid=false`, backend coincide, digest stale o faltante | Rechazado (`HandshakeConfidentialMismatch`) | El remoto debe aplicar activaciones pendientes de registry/parametros o esperar el `activation_height` programado. Hasta corregir, el nodo sigue visible pero nunca entra en rotacion de consenso. |
| `enabled=true`, `assume_valid=true` | Rechazado (`HandshakeConfidentialMismatch`) | Los validadores requieren verificacion de proofs; configura el remoto como observer con ingreso solo Torii o cambia `assume_valid=false` tras habilitar verificacion completa. |
| `enabled=false`, campos omitidos (build desactualizado), o backend de verificador distinto | Rechazado (`HandshakeConfidentialMismatch`) | Peers desactualizados o parcialmente actualizados no pueden unirse a la red de consenso. Actualizalos al release actual y asegura que el tuple backend + digest coincida antes de reconectar. |

Observers que intencionalmente omiten verificacion de proofs no deben abrir conexiones de consenso contra validadores con capability gates. Aun pueden ingerir bloques via Torii o APIs de archivo, pero la red de consenso los rechaza hasta que anuncien capacidades coincidentes.

### Politica de pruning de reveals y retencion de nullifiers

Los ledgers confidenciales deben retener suficiente historial para probar frescura de notes y reproducir auditorias impulsadas por governance. La politica por defecto, aplicada por `ConfidentialLedger`, es:

- **Retencion de nullifiers:** mantener nullifiers gastados por un *minimo* de `730` dias (24 meses) despues de la altura de gasto, o la ventana regulatoria obligatoria si es mayor. Los operadores pueden extender la ventana via `confidential.retention.nullifier_days`. Nullifiers mas recientes que la ventana DEBEN seguir consultables via Torii para que auditores prueben ausencia de double-spend.
- **Pruning de reveals:** los reveals transparentes (`RevealConfidential`) podan los commitments asociados inmediatamente despues de que el bloque finaliza, pero el nullifier consumido sigue sujeto a la regla de retencion anterior. Eventos relacionados con reveal (`ConfidentialEvent::Unshielded`) registran el monto publico, recipient y hash de proof para que reconstruir reveals historicos no requiera el ciphertext podado.
- **Frontier checkpoints:** los frontiers de commitment mantienen checkpoints en rolling que cubren el mayor de `max_anchor_age_blocks` y la ventana de retencion. Los nodos compactan checkpoints mas antiguos solo despues de que todos los nullifiers dentro del intervalo expiran.
- **Remediacion por digest stale:** si se levanta `HandshakeConfidentialMismatch` por drift de digest, los operadores deben (1) verificar que las ventanas de retencion de nullifiers coincidan en el cluster, (2) ejecutar `iroha_cli app confidential verify-ledger` para regenerar el digest contra el conjunto de nullifiers retenidos, y (3) redeployar el manifest actualizado. Cualquier nullifier podado prematuramente debe restaurarse desde almacenamiento frio antes de reingresar a la red.

Documenta overrides locales en el runbook de operaciones; las politicas de governance que extienden la ventana de retencion deben actualizar configuracion de nodo y planes de almacenamiento de archivo en lockstep.

### Flujo de eviction y recuperacion

1. Durante el dial, `IrohaNetwork` compara las capacidades anunciadas. Cualquier mismatch levanta `HandshakeConfidentialMismatch`; la conexion se cierra y el peer permanece en la cola de discovery sin ser promovido a `Ready`.
2. El fallo se expone via el log del servicio de red (incluye digest remoto y backend), y Sumeragi nunca programa al peer para propuesta o voto.
3. Los operadores remedian alineando registries de verificador y sets de parametros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) o programando `next_conf_features` con un `activation_height` acordado. Una vez que el digest coincide, el siguiente handshake tiene exito automaticamente.
4. Si un peer stale logra difundir un bloque (por ejemplo, via replay de archivo), los validadores lo rechazan de forma determinista con `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, manteniendo el estado del ledger consistente en la red.

### Flujo de handshake seguro ante replay

1. Cada intento saliente asigna material de clave Noise/X25519 nuevo. El payload de handshake que se firma (`handshake_signature_payload`) concatena las claves publicas efimeras local y remota, la direccion de socket anunciada codificada en Norito y, cuando se compila con `handshake_chain_id`, el identificador de chain. El mensaje se encripta con AEAD antes de salir del nodo.
2. El receptor recomputa el payload con el orden de claves peer/local invertido y verifica la firma Ed25519 embebida en `HandshakeHelloV1`. Debido a que ambas claves efimeras y la direccion anunciada forman parte del dominio de firma, reproducir un mensaje capturado contra otro peer o recuperar una conexion stale falla la verificacion de forma determinista.
3. Flags de capacidad confidencial y el `ConfidentialFeatureDigest` viajan dentro de `HandshakeConfidentialMeta`. El receptor compara el tuple `{ enabled, assume_valid, verifier_backend, digest }` contra su `ConfidentialHandshakeCaps` local; cualquier mismatch sale temprano con `HandshakeConfidentialMismatch` antes de que el transporte transicione a `Ready`.
4. Los operadores DEBEN recomputar el digest (via `compute_confidential_feature_digest`) y reiniciar nodos con los registries/politicas actualizados antes de reconectar. Peers que anuncian digests antiguos siguen fallando el handshake, evitando que estado stale reingrese al set de validadores.
5. Exitos y fallos del handshake actualizan los contadores estandar `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomia de errores) y emiten logs estructurados con el peer ID remoto y el fingerprint del digest. Monitorea estos indicadores para detectar replays o configuraciones incorrectas durante el rollout.

## Gestion de claves y payloads
- Jerarquia de derivacion por account:
  - `sk_spend` -> `nk` (nullifier key), `ivk` (incoming viewing key), `ovk` (outgoing viewing key), `fvk`.
- Payloads de notes encriptadas usan AEAD con shared keys derivadas por ECDH; se pueden adjuntar view keys de auditores opcionales a outputs segun la politica del asset.
- Adiciones al CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, tooling de auditor para descifrar memos, y el helper `iroha app zk envelope` para producir/inspeccionar envelopes Norito offline.

## Gas, limites y controles DoS
- Schedule de gas determinista:
  - Halo2 (Plonkish): base `250_000` gas + `2_000` gas por public input.
  - `5` gas por proof byte, mas cargos por nullifier (`300`) y por commitment (`500`).
  - Los operadores pueden sobrescribir estas constantes via la configuracion del nodo (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); los cambios se propagan al inicio o cuando la capa de config hot-reload y se aplican de forma determinista en el cluster.
- Limites duros (defaults configurables):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Proofs que exceden `verify_timeout_ms` abortan la instruccion de forma determinista (ballots de governance emiten `proof verification exceeded timeout`, `VerifyProof` retorna error).
- Cuotas adicionales aseguran liveness: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, y `max_public_inputs` acotan block builders; `reorg_depth_bound` (>= `max_anchor_age_blocks`) gobierna la retencion de frontier checkpoints.
- La ejecucion runtime ahora rechaza transacciones que exceden estos limites por transaccion o por bloque, emitiendo errores `InvalidParameter` deterministas y dejando el estado del ledger sin cambios.
- Mempool prefiltro transacciones confidenciales por `vk_id`, longitud de proof y edad de anchor antes de invocar el verificador para mantener acotado el uso de recursos.
- La verificacion se detiene de forma determinista en timeout o violacion de limites; las transacciones fallan con errores explicitos. Backends SIMD son opcionales pero no alteran el accounting de gas.

### Baselines de calibracion y gates de aceptacion
- **Plataformas de referencia.** Las corridas de calibracion DEBEN cubrir los tres perfiles de hardware abajo. Corridas que no capturan todos los perfiles se rechazan durante la revision.

  | Perfil | Arquitectura | CPU / Instancia | Flags de compilador | Proposito |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) o Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Establece valores piso sin intrinsics vectoriales; se usa para ajustar tablas de costo fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | release por defecto | Valida el path AVX2; revisa que los speedups SIMD se mantengan dentro de tolerancia del gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | release por defecto | Asegura que el backend NEON permanezca determinista y alineado con los schedules x86. |

- **Benchmark harness.** Todos los reportes de calibracion de gas DEBEN producirse con:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar el fixture determinista.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` cuando cambian costos de opcode del VM.

- **Randomness fija.** Exporta `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de correr benches para que `iroha_test_samples::gen_account_in` cambie a la ruta determinista `KeyPair::from_seed`. El harness imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` una vez; si falta la variable, la revision DEBE fallar. Cualquier utilidad nueva de calibracion debe seguir respetando esta env var al introducir aleatoriedad auxiliar.

- **Captura de resultados.**
  - Subir resmenes de Criterion (`target/criterion/**/raw.csv`) para cada perfil al artefacto de release.
  - Guardar metricas derivadas (`ns/op`, `gas/op`, `ns/gas`) en el [Confidential Gas Calibration ledger](./confidential-gas-calibration) junto con el commit de git y la version de compilador usada.
  - Mantener los ultimos dos baselines por perfil; eliminar snapshots mas antiguos una vez validado el reporte mas nuevo.

- **Tolerancias de aceptacion.**
  - Deltas de gas entre `baseline-simd-neutral` y `baseline-avx2` DEBEN permanecer <= +/-1.5%.
  - Deltas de gas entre `baseline-simd-neutral` y `baseline-neon` DEBEN permanecer <= +/-2.0%.
  - Propuestas de calibracion que exceden estos umbrales requieren ajustes de schedule o un RFC que explique la discrepancia y su mitigacion.

- **Checklist de revision.** Los submitters son responsables de:
  - Incluir `uname -a`, extractos de `/proc/cpuinfo` (model, stepping), y `rustc -Vv` en el log de calibracion.
  - Verificar que `IROHA_CONF_GAS_SEED` se vea en la salida de bench (las benches imprimen la seed activa).
  - Asegurar que los feature flags del pacemaker y del verificador confidencial espejen produccion (`--features confidential,telemetry` al correr benches con Telemetry).

## Config y operaciones
- `iroha_config` agrega la seccion `[confidential]`:
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
- Telemetria emite metricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, y `confidential_policy_transitions_total`, nunca exponiendo datos en claro.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`

## Estrategia de testing
- Determinismo: el shuffle aleatorio de transacciones dentro de bloques produce Merkle roots y sets de nullifier identicos.
- Resiliencia a reorg: simular reorgs multi-bloque con anchors; nullifiers se mantienen estables y anchors stale se rechazan.
- Invariantes de gas: verificar uso de gas identico entre nodos con y sin aceleracion SIMD.
- Tests de borde: proofs en techos de tamano/gas, max in/out counts, enforcement de timeout.
- Lifecycle: operaciones de governance para activacion/deprecacion de verificador y parametros, tests de gasto tras rotacion.
- Policy FSM: transiciones permitidas/no permitidas, delays de transicion pendiente y rechazo de mempool alrededor de alturas efectivas.
- Emergencias de registry: retiro de emergencia congela assets afectados en `withdraw_height` y rechaza proofs despues.
- Capability gating: validadores con `conf_features` mismatched rechazan bloques; observers con `assume_valid=true` avanzan sin afectar consenso.
- Equivalencia de estado: nodos validator/full/observer producen roots de estado identicos en la cadena canonica.
- Fuzzing negativo: proofs malformadas, payloads sobredimensionados y colisiones de nullifier se rechazan deterministamente.

## Migracion
- Rollout con feature flag: hasta que Phase C3 se complete, `enabled` default a `false`; los nodos anuncian capacidades antes de unirse al set validador.
- Assets transparentes no se afectan; las instrucciones confidenciales requieren entradas de registry y negociacion de capacidades.
- Nodos compilados sin soporte confidencial rechazan bloques relevantes de forma determinista; no pueden unirse al set validador pero pueden operar como observers con `assume_valid=true`.
- Genesis manifests incluyen entradas iniciales del registry, sets de parametros, politicas confidenciales para assets y keys de auditor opcionales.
- Los operadores siguen runbooks publicados para rotacion de registry, transiciones de politica y retiro de emergencia para mantener upgrades deterministas.

## Trabajo pendiente
- Benchmarks de parametros Halo2 (tamano de circuito, estrategia de lookup) y registrar resultados en el playbook de calibracion para que defaults de gas/timeout se actualicen junto al proximo refresh de `confidential_assets_calibration.md`.
- Finalizar politicas de disclosure de auditor y APIs de selective-viewing asociadas, conectando el flujo aprobado en Torii una vez que el borrador de governance se firme.
- Extender el esquema de witness encryption para cubrir outputs multi-recipient y memos en batch, documentando el formato del envelope para implementadores de SDK.
- Encargar una revision de seguridad externa de circuitos, registries y procedimientos de rotacion de parametros y archivar hallazgos junto a los reportes internos de auditoria.
- Especificar APIs de reconciliacion de spentness para auditores y publicar guia de alcance de view-key para que vendors de wallets implementen las mismas semanticas de atestacion.

## Fases de implementacion
1. **Phase M0 - Endurecimiento Stop-Ship**
   - [x] La derivacion de nullifier ahora sigue el diseno Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) con orden determinista de commitments forzado en actualizaciones del ledger.
   - [x] La ejecucion aplica limites de tamano de proof y cuotas confidenciales por transaccion/por bloque, rechazando transacciones fuera de presupuesto con errores deterministas.
   - [x] El handshake P2P anuncia `ConfidentialFeatureDigest` (digest de backend + fingerprints de registry) y falla mismatches de forma determinista via `HandshakeConfidentialMismatch`.
   - [x] Remover panics en paths de ejecucion confidencial y agregar role gating para nodos sin soporte.
   - [ ] Aplicar presupuestos de timeout de verificador y limites de profundidad de reorg para frontier checkpoints.
     - [x] Presupuestos de timeout de verificacion aplicados; proofs que exceden `verify_timeout_ms` ahora fallan deterministamente.
     - [x] Frontier checkpoints ahora respetan `reorg_depth_bound`, podando checkpoints mas antiguos que la ventana configurada mientras mantienen snapshots deterministas.
   - Introducir `AssetConfidentialPolicy`, policy FSM y gates de enforcement para instrucciones mint/transfer/reveal.
   - Comprometer `conf_features` en headers de bloque y rechazar participacion de validadores cuando los digests de registry/parametros divergen.
2. **Phase M1 - Registries y parametros**
   - Entregar registries `ZkVerifierEntry`, `PedersenParams`, y `PoseidonParams` con ops de governance, anclaje de genesis y manejo de cache.
   - Conectar syscall para requerir lookups de registry, IDs de gas schedule, hashing de schema y checks de tamano.
   - Enviar formato de payload encriptado v1, vectores de derivacion de keys para wallet, y soporte CLI para gestion de claves confidenciales.
3. **Phase M2 - Gas y performance**
   - Implementar schedule de gas determinista, contadores por bloque, y harnesses de benchmark con telemetria (latencia de verificacion, tamanos de proof, rechazos de mempool).
   - Endurecer CommitmentTree checkpoints, carga LRU, e indices de nullifier para workloads multi-asset.
4. **Phase M3 - Rotacion y tooling de wallet**
   - Habilitar aceptacion de proofs multi-parametro y multi-version; soportar activacion/deprecacion impulsada por governance con runbooks de transicion.
   - Entregar flujos de migracion en SDK/CLI, workflows de escaneo de auditor, y tooling de reconciliacion de spentness.
5. **Phase M4 - Audit y ops**
   - Proveer workflows de keys de auditor, APIs de disclosure selectiva, y runbooks operativos.
   - Programar revision externa de criptografia/seguridad y publicar hallazgos en `status.md`.

Cada fase actualiza milestones del roadmap y tests asociados para mantener garantias de ejecucion determinista en la red blockchain.
