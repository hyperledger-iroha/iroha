---
lang: es
direction: ltr
source: docs/source/confidential_assets.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 969ffd4cee6ee4880d5f754fb36adaf30dde532a29e4c6397cf0f358438bb57e
source_last_modified: "2026-01-22T15:38:30.657840+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
SPDX-License-Identifier: Apache-2.0
-->
# Activos confidenciales y diseño de transferencia ZK

## Motivación
- Ofrecer flujos de activos protegidos de forma voluntaria para que los dominios puedan preservar la privacidad de las transacciones sin alterar la circulación transparente.
- Proporcionar a auditores y operadores controles del ciclo de vida (activación, rotación, revocación) de circuitos y parámetros criptográficos.

## Modelo de amenaza
- Los validadores son honestos pero curiosos: ejecutan el consenso fielmente pero intentan inspeccionar el libro mayor/estado.
- Network observers see block data and gossiped transactions; sin asumir canales privados de chismes.
- Fuera de alcance: análisis de tráfico fuera del libro mayor, adversarios cuánticos (seguidos por separado según la hoja de ruta de PQ), ataques de disponibilidad del libro mayor.

## Descripción general del diseño
- Los activos pueden declarar un *grupo protegido* además de los saldos transparentes existentes; La circulación protegida está representada a través de compromisos criptográficos.
- Las notas encapsulan `(asset_id, amount, recipient_view_key, blinding, rho)` con:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independiente del orden de las notas.
  - Carga útil cifrada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Las transacciones transportan cargas útiles `ConfidentialTransfer` codificadas con Norito que contienen:
  - Aportes públicos: ancla Merkle, anuladores, nuevos compromisos, identificación de activos, versión del circuito.
  - Cargas útiles cifradas para destinatarios y auditores opcionales.
  - Prueba de conocimiento cero que acredite la conservación, propiedad y autorización del valor.
- Las claves de verificación y los conjuntos de parámetros se controlan a través de registros en el libro mayor con ventanas de activación; Los nodos se niegan a validar pruebas que hagan referencia a entradas desconocidas o revocadas.
- Los encabezados de consenso se comprometen con el resumen activo de funciones confidenciales, por lo que los bloques solo se aceptan cuando el estado del registro y del parámetro coinciden.
- La construcción de prueba utiliza una pila Halo2 (Plonkish) sin una configuración confiable; Groth16 u otras variantes de SNARK no son compatibles intencionalmente con la versión 1.

### Accesorios deterministas

Los sobres para notas confidenciales ahora se envían con un accesorio canónico en `fixtures/confidential/encrypted_payload_v1.json`. El conjunto de datos captura una envolvente v1 positiva más muestras negativas con formato incorrecto para que los SDK puedan afirmar la paridad de análisis. Las pruebas del modelo de datos de Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) cargan el dispositivo directamente, garantizando que la codificación Norito, las superficies de error y la cobertura de regresión permanezcan alineadas a medida que evoluciona el códec.

Los SDK de Swift ahora pueden emitir instrucciones de protección sin pegamento JSON personalizado: construya un
`ShieldRequest` con compromiso de nota de 32 bytes, carga útil cifrada y metadatos de débito,
luego llame a `IrohaSDK.submit(shield:keypair:)` (o `submitAndWait`) para firmar y transmitir el
transacción sobre `/v2/pipeline/transactions`. El ayudante valida la duración del compromiso,
Enhebra `ConfidentialEncryptedPayload` en el codificador Norito y refleja el `zk::Shield`.
diseño que se describe a continuación para que las billeteras se mantengan al día con Rust.## Compromisos de consenso y control de capacidades
- Los encabezados de bloque exponen `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; el resumen participa en el hash de consenso y debe ser igual a la vista del registro local para la aceptación del bloque.
- La gobernanza puede realizar actualizaciones programando `next_conf_features` con un futuro `activation_height`; hasta esa altura, los productores de bloques deben continuar emitiendo el resumen anterior.
- Los nodos validadores DEBEN operar con `confidential.enabled = true` e `assume_valid = false`. Las comprobaciones de inicio se niegan a unirse al conjunto de validadores si alguna de las condiciones falla o si el `conf_features` local diverge.
- Los metadatos del protocolo de enlace P2P ahora incluyen `{ enabled, assume_valid, conf_features }`. Los pares que anuncian funciones no compatibles se rechazan con `HandshakeConfidentialMismatch` y nunca entran en la rotación de consenso.
- Los observadores no validadores pueden configurar `assume_valid = true`; aplican ciegamente deltas confidenciales pero no influyen en la seguridad del consenso.## Políticas de activos
- Cada definición de activo lleva un `AssetConfidentialPolicy` establecido por el creador o mediante gobernanza:
  - `TransparentOnly`: modo predeterminado; sólo se permiten instrucciones transparentes (`MintAsset`, `TransferAsset`, etc.) y se rechazan las operaciones blindadas.
  - `ShieldedOnly`: todas las emisiones y transferencias deben utilizar instrucciones confidenciales; `RevealConfidential` está prohibido, por lo que los saldos nunca aparecen públicamente.
  - `Convertible`: los poseedores pueden mover valor entre representaciones transparentes y blindadas usando las instrucciones de entrada y salida a continuación.
- Las políticas siguen un MEV restringido para evitar que los fondos queden varados:
  - `TransparentOnly → Convertible` (habilitación inmediata de piscina blindada).
  - `TransparentOnly → ShieldedOnly` (requiere transición pendiente y ventana de conversión).
  - `Convertible → ShieldedOnly` (retraso mínimo obligatorio).
  - `ShieldedOnly → Convertible` (se requiere un plan de migración para que los billetes protegidos sigan siendo utilizables).
  - `ShieldedOnly → TransparentOnly` no está permitido a menos que el grupo protegido esté vacío o el gobierno codifique una migración que desproteja los billetes pendientes.
- Las instrucciones de gobernanza establecen `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` a través del ISI `ScheduleConfidentialPolicyTransition` y pueden cancelar los cambios programados con `CancelConfidentialPolicyTransition`. La validación de Mempool garantiza que ninguna transacción se extienda a la altura de la transición y la inclusión falla de manera determinista si una verificación de política cambiara a mitad del bloque.
- Las transiciones pendientes se aplican automáticamente cuando se abre un nuevo bloque: una vez que la altura del bloque ingresa a la ventana de conversión (para actualizaciones `ShieldedOnly`) o alcanza el `effective_height` programado, el tiempo de ejecución actualiza `AssetConfidentialPolicy`, actualiza los metadatos `zk.policy` y borra la entrada pendiente. Si el suministro transparente permanece cuando madura una transición `ShieldedOnly`, el tiempo de ejecución anula el cambio y registra una advertencia, dejando intacto el modo anterior.
- Las perillas de configuración `policy_transition_delay_blocks` e `policy_transition_window_blocks` imponen avisos mínimos y períodos de gracia para permitir que las billeteras conviertan billetes alrededor del interruptor.
- `pending_transition.transition_id` también funciona como identificador de auditoría; La gobernanza debe citarlo al finalizar o cancelar transiciones para que los operadores puedan correlacionar los informes de entrada y salida.
- `policy_transition_window_blocks` tiene un valor predeterminado de 720 (≈12 horas con un tiempo de bloqueo de 60 s). Los nodos restringen las solicitudes de gobernanza que intentan avisar con menos antelación.
- Los manifiestos de Génesis y los flujos CLI muestran las políticas actuales y pendientes. La lógica de admisión lee la política en el momento de la ejecución para confirmar que cada instrucción confidencial esté autorizada.
- Lista de verificación de migración: consulte "Secuencia de migración" a continuación para conocer el plan de actualización por etapas que sigue Milestone M0.

#### Monitoreo de transiciones a través de ToriiCarteras y auditores encuestan `GET /v2/confidential/assets/{definition_id}/transitions` para inspeccionar
el activo `AssetConfidentialPolicy`. La carga útil JSON siempre incluye la información canónica.
ID de activo, la última altura de bloque observada, el `current_mode` de la política, el modo que es
efectivo a esa altura (las ventanas de conversión informan temporalmente `Convertible`), y el
identificadores de parámetros esperados `vk_set_hash`/Poseidon/Pedersen. Los consumidores de Swift SDK pueden llamar
`ToriiClient.getConfidentialAssetPolicy` para recibir los mismos datos que los DTO escritos sin
decodificación escrita a mano. Cuando está pendiente una transición de gobernanza, la respuesta también incluye:

- `transition_id`: identificador de auditoría devuelto por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
- `effective_height`.
- `conversion_window` and the derived `window_open_height` (the block where wallets must
  comenzar la conversión para cortes ShieldedOnly).

Respuesta de ejemplo:

```json
{
  "asset_id": "rose#wonderland",
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

Una respuesta `404` indica que no existe una definición de activo coincidente. Cuando no hay transición
El campo `pending_transition` programado es `null`.

### Máquina de estado de políticas| Modo actual | Modo siguiente | Requisitos previos | Manejo de altura efectiva | Notas |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | La gobernanza ha activado las entradas del registro de verificadores/parámetros. Envíe `ScheduleConfidentialPolicyTransition` con `effective_height ≥ current_height + policy_transition_delay_blocks`. | Transition executes exactly at `effective_height`; La piscina protegida estará disponible de inmediato.                   | Ruta predeterminada para habilitar la confidencialidad manteniendo flujos transparentes.               |
| Sólo transparente | Sólo blindado | Igual que el anterior, más `policy_transition_window_blocks ≥ 1`.                                                         | El tiempo de ejecución ingresa automáticamente `Convertible` en `effective_height - policy_transition_window_blocks`; cambia a `ShieldedOnly` en `effective_height`. | Proporciona una ventana de conversión determinista antes de que se deshabiliten las instrucciones transparentes.   |
| Descapotable | Sólo blindado | Transición programada con `effective_height ≥ current_height + policy_transition_delay_blocks`. La gobernanza DEBE certificar (`transparent_supply == 0`) mediante metadatos de auditoría; El tiempo de ejecución aplica esto en el corte. | Semántica de ventana idéntica a la anterior. Si el suministro transparente es distinto de cero en `effective_height`, la transición se cancela con `PolicyTransitionPrerequisiteFailed`. | Bloquea el activo en circulación totalmente confidencial.                                     |
| Sólo blindado | Descapotable | Transición programada; sin retiro de emergencia activo (`withdraw_height` desarmado).                                    | El estado cambia en `effective_height`; Las rampas de revelación se vuelven a abrir mientras los billetes protegidos siguen siendo válidos.                           | Se utiliza para ventanas de mantenimiento o revisiones de auditores.                                          |
| Sólo blindado | Sólo transparente | La gobernanza debe demostrar `shielded_supply == 0` o presentar un plan `EmergencyUnshield` firmado (se requieren firmas del auditor). | Runtime abre una ventana `Convertible` delante de `effective_height`; en el apogeo, las instrucciones confidenciales fallan y el activo vuelve al modo solo transparente. | Salida de último recurso. La transición se cancela automáticamente si alguna nota confidencial pasa durante el período. |
| Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` borra el cambio pendiente.                                                        | `pending_transition` eliminado inmediatamente.                                                                          | Mantiene el status quo; mostrado para que esté completo.                                             |Las transiciones que no se enumeran anteriormente se rechazan durante la presentación de gobernanza. Runtime comprueba los requisitos previos justo antes de aplicar una transición programada; Las condiciones previas fallidas hacen que el activo vuelva a su modo anterior y emite `PolicyTransitionPrerequisiteFailed` a través de telemetría y eventos de bloqueo.

### Secuenciación de migración

2. **Etapa la transición:** Envíe `ScheduleConfidentialPolicyTransition` con un `effective_height` que respete `policy_transition_delay_blocks`. Cuando avance hacia `ShieldedOnly`, especifique una ventana de conversión (`window ≥ policy_transition_window_blocks`).
3. **Publicar guía para el operador:** Registre el `transition_id` devuelto y haga circular un runbook de entrada y salida. Las billeteras y los auditores se suscriben a `/v2/confidential/assets/{id}/transitions` para conocer la altura de apertura de la ventana.
4. **Aplicación de ventanas:** Cuando se abre la ventana, el tiempo de ejecución cambia la política a `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }` y comienza a rechazar solicitudes de gobernanza conflictivas.
5. **Finalizar o cancelar:** En `effective_height`, el tiempo de ejecución verifica los requisitos previos de la transición (suministro transparente cero, sin retiros de emergencia, etc.). El éxito cambia la política al modo solicitado; El error emite `PolicyTransitionPrerequisiteFailed`, borra la transición pendiente y deja la política sin cambios.
6. **Actualizaciones del esquema:** Después de una transición exitosa, la gobernanza aumenta la versión del esquema de activos (por ejemplo, `asset_definition.v2`) y las herramientas CLI requieren `confidential_policy` al serializar manifiestos. Los documentos de actualización de Genesis instruyen a los operadores a agregar configuraciones de políticas y huellas digitales de registro antes de reiniciar los validadores.

Las nuevas redes que comienzan con la confidencialidad habilitada codifican la política deseada directamente en la génesis. Todavía siguen la lista de verificación anterior cuando cambian los modos después del lanzamiento para que las ventanas de conversión sigan siendo deterministas y las billeteras tengan tiempo de adaptarse.

### Activación y control de versiones del manifiesto Norito- Los manifiestos de Génesis DEBEN incluir un `SetParameter` para la clave personalizada `confidential_registry_root`. La carga útil es Norito JSON que coincide con `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omita el campo (`null`) cuando no haya entradas de verificador activas; de lo contrario, proporcione una cadena hexadecimal de 32 bytes (`0x…`) igual al hash producido por `compute_vk_set_hash` sobre las instrucciones del verificador enviadas en el manifiesto. Los nodos se niegan a iniciarse si falta el parámetro o si el hash no coincide con las escrituras codificadas del registro.
- El `ConfidentialFeatureDigest::conf_rules_version` en línea incorpora la versión de diseño del manifiesto. Para redes v1 DEBE permanecer `Some(1)` y es igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Cuando el conjunto de reglas evolucione, elimine la constante, regenere los manifiestos y despliegue los binarios al mismo tiempo; mezclar versiones hace que los validadores rechacen bloques con `ConfidentialFeatureDigestMismatch`.
- Los manifiestos de activación DEBEN agrupar actualizaciones de registro, cambios en el ciclo de vida de los parámetros y transiciones de políticas para que el resumen se mantenga consistente:
  1. Aplique las mutaciones de registro planificadas (`Publish*`, `Set*Lifecycle`) en una vista de estado fuera de línea y calcule el resumen posterior a la activación con `compute_confidential_feature_digest`.
  2. Emita `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})` utilizando el hash calculado para que los pares rezagados puedan recuperar el resumen correcto incluso si omiten las instrucciones de registro intermedias.
  3. Adjunte las instrucciones `ScheduleConfidentialPolicyTransition`. Cada instrucción debe citar el `transition_id` emitido por el gobierno; los manifiestos que lo olviden serán rechazados por el tiempo de ejecución.
  4. Conserve los bytes del manifiesto, una huella digital SHA-256 y el resumen utilizado en el plan de activación. Los operadores verifican los tres artefactos antes de votar para que el manifiesto entre en vigor para evitar particiones.
- Cuando los despliegues requieran un corte diferido, registre la altura objetivo en un parámetro personalizado complementario (por ejemplo, `custom.confidential_upgrade_activation_height`). Esto les brinda a los auditores una prueba codificada con Norito de que los validadores respetaron la ventana de notificación antes de que el cambio del resumen entrara en vigencia.## Ciclo de vida del verificador y de los parámetros
### Registro ZK
- El libro mayor almacena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` donde `proving_system` actualmente está fijado en `Halo2`.
- Los pares `(circuit_id, version)` son únicos a nivel mundial; el registro mantiene un índice secundario para búsquedas por metadatos de circuito. Los intentos de registrar un par duplicado se rechazan durante la admisión.
- `circuit_id` no debe estar vacío y se debe proporcionar `public_inputs_schema_hash` (normalmente un hash Blake2b-32 de la codificación canónica de entrada pública del verificador). La admisión rechaza los registros que omiten estos campos.
- Las instrucciones de gobernanza incluyen:
  - `PUBLISH` para agregar una entrada `Proposed` solo con metadatos.
  - `ACTIVATE { vk_id, activation_height }` para programar la activación de entrada en un límite de época.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar la altura final donde las pruebas pueden hacer referencia a la entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para parada de emergencia; Los activos afectados congelan los gastos confidenciales después del máximo de retiro hasta que se activen nuevas entradas.
- Los manifiestos de Génesis emiten automáticamente un parámetro personalizado `confidential_registry_root` cuyo `vk_set_hash` coincide con las entradas activas; La validación compara este resumen con el estado del registro local antes de que un nodo pueda unirse al consenso.
- Registrar o actualizar un verificador requiere un `gas_schedule_id`; La verificación exige que la entrada de registro sea `Active`, presente en el índice `(circuit_id, version)`, y que las pruebas de Halo2 proporcionen un `OpenVerifyEnvelope` cuyo `circuit_id`, `vk_hash` e `public_inputs_schema_hash` coincidan con el registro de registro.

### Claves de demostración
- Las claves de prueba permanecen fuera del libro mayor, pero se hace referencia a ellas mediante identificadores de contenido (`pk_cid`, `pk_hash`, `pk_len`) publicados junto con los metadatos del verificador.
- Los SDK de Wallet obtienen datos PK, verifican hashes y almacenan en caché localmente.

### Parámetros de Pedersen y Poseidón
- Registros separados (`PedersenParams`, `PoseidonParams`) controles de ciclo de vida del verificador espejo, cada uno con `params_id`, hashes de generadores/constantes, activación, desaprobación y alturas de retiro.

## Ordenamiento determinista y anuladores
- Cada activo mantiene un `CommitmentTree` con `next_leaf_index`; los bloques añaden compromisos en orden determinista: iteran transacciones en orden de bloques; dentro de cada transacción, itere las salidas blindadas mediante el `output_idx` serializado ascendente.
- `note_position` se deriva de las compensaciones del árbol pero **no** forma parte del anulador; sólo alimenta caminos de membresía dentro del testigo de prueba.
- La estabilidad del anulador bajo reorganizaciones está garantizada por el diseño del PRF; la entrada PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }` y los anclajes hacen referencia a raíces históricas de Merkle limitadas por `max_anchor_age_blocks`.## Flujo del libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - Requiere política de activos `Convertible` o `ShieldedOnly`; la admisión verifica la autoridad del activo, recupera el `params_id` actual, muestra el `rho`, emite el compromiso y actualiza el árbol Merkle.
   - Emite `ConfidentialEvent::Shielded` con el nuevo compromiso, delta raíz de Merkle y hash de llamada de transacción para pistas de auditoría.
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - VM syscall verifica la prueba utilizando la entrada del registro; El host garantiza que los anuladores no se utilicen, los compromisos se añaden de forma determinista y el ancla es reciente.
   - El libro mayor registra entradas `NullifierSet`, almacena cargas útiles cifradas para destinatarios/auditores y emite anuladores de resumen `ConfidentialEvent::Transferred`, salidas ordenadas, hash de prueba y raíces de Merkle.
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - Disponible sólo para activos `Convertible`; la prueba valida que el valor del billete sea igual al monto revelado, el libro mayor acredita el saldo transparente y quema el billete blindado marcando el anulador como gastado.
   - Emite `ConfidentialEvent::Unshielded` con el monto público, anuladores consumidos, identificadores de prueba y hash de llamada de transacción.

## Adiciones al modelo de datos
- `ConfidentialConfig` (nueva sección de configuración) con indicador de habilitación, `assume_valid`, perillas de límite/gas, ventana de anclaje, backend del verificador.
- Esquemas `ConfidentialNote`, `ConfidentialTransfer` e `ConfidentialMint` Norito con byte de versión explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` envuelve los bytes de nota AEAD con `{ version, ephemeral_pubkey, nonce, ciphertext }`, de forma predeterminada es `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para el diseño XChaCha20-Poly1305.
- Los vectores canónicos de derivación de claves se encuentran en `docs/source/confidential_key_vectors.json`; Tanto el punto final CLI como Torii retroceden contra estos dispositivos. Los derivados de billetera para la escalera de gasto/anulación/visualización se publican en `fixtures/confidential/keyset_derivation_v1.json` y se ejercitan mediante las pruebas del SDK de Rust + Swift para garantizar la paridad entre idiomas.
- `asset::AssetDefinition` gana `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste el enlace `(backend, name, commitment)` para verificadores de transferencia/desprotección; la ejecución rechaza las pruebas cuya clave de verificación en línea o referenciada no coincide con el compromiso registrado y verifica las pruebas de transferencia/desprotección con la clave de backend resuelta antes de mutar el estado.
- `CommitmentTree` (por activo con puntos de control fronterizos), `NullifierSet` con clave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` almacenados en estado mundial.
- Mempool mantiene estructuras transitorias `NullifierIndex` e `AnchorIndex` para la detección temprana de duplicados y comprobaciones de antigüedad del anclaje.
- Las actualizaciones del esquema Norito incluyen ordenamiento canónico para entradas públicas; Las pruebas de ida y vuelta garantizan el determinismo de codificación.
- Los viajes de ida y vuelta de carga útil cifrada se bloquean mediante pruebas unitarias (`crates/iroha_data_model/src/confidential.rs`), y los vectores de derivación de claves de billetera anteriores anclan las derivaciones de sobre AEAD para los auditores. `norito.md` documenta el encabezado en línea del sobre.## IVM Integración y llamada al sistema
- Introduzca la llamada al sistema `VERIFY_CONFIDENTIAL_PROOF` aceptando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, y resultante `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall carga metadatos del verificador desde el registro, impone límites de tamaño/tiempo, cobra gas determinista y solo aplica delta si la prueba tiene éxito.
- El host expone el rasgo `ConfidentialLedger` de solo lectura para recuperar instantáneas de la raíz de Merkle y el estado del anulador; La biblioteca Kotodama proporciona ayudas de ensamblaje de testigos y validación de esquemas.
- Documentos de Pointer-ABI actualizados para aclarar el diseño del búfer de prueba y los identificadores de registro.

## Negociación de capacidad de nodo
- El protocolo de enlace anuncia `feature_bits.confidential` junto con un `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participación del validador requiere `confidential.enabled=true`, `assume_valid=false`, identificadores de backend del verificador idénticos y resúmenes coincidentes; las discrepancias fallan en el protocolo de enlace con `HandshakeConfidentialMismatch`.
- La configuración admite `assume_valid` solo para nodos observadores: cuando está deshabilitado, encontrar instrucciones confidenciales produce `UnsupportedInstruction` determinista sin pánico; cuando está habilitado, los observadores aplican deltas de estado declarados sin verificar pruebas.
- Mempool rechaza transacciones confidenciales si la capacidad local está deshabilitada. Los filtros de chismes evitan enviar transacciones protegidas a pares sin capacidad de comparación, mientras que reenvían a ciegas ID de verificadores desconocidos dentro de los límites de tamaño.

### Revelar la política de poda y retención de anuladores

Los libros de contabilidad confidenciales deben conservar suficiente historial para demostrar la actualidad de los billetes y
reproducir auditorías impulsadas por la gobernanza. La política predeterminada, aplicada por
`ConfidentialLedger`, es:

- **Retención de anuladores:** mantenga los anuladores gastados durante *mínimo* `730` días (24
  meses) después de la altura del gasto, o la ventana exigida por el regulador si es más larga.
  Los operadores pueden ampliar la ventana a través de `confidential.retention.nullifier_days`.
  Los anuladores más jóvenes que la ventana de retención DEBEN seguir siendo consultables a través de Torii para que
  Los auditores pueden probar la ausencia por doble gasto.
- **Poda de revelación:** revelaciones transparentes (`RevealConfidential`) poda el
  compromisos de notas asociadas inmediatamente después de que finalice el bloque, pero el
  El anulador consumido permanece sujeto a la regla de retención anterior. Revelado relacionado
  eventos (`ConfidentialEvent::Unshielded`) registran el monto público, el destinatario,
  y hash de prueba, por lo que reconstruir revelaciones históricas no requiere la poda
  texto cifrado.
- **Puntos de control fronterizos:** las fronteras de compromiso mantienen puntos de control continuos
  cubriendo el mayor de `max_anchor_age_blocks` y la ventana de retención. Nodos
  compacte los puntos de control más antiguos solo después de que expiren todos los anuladores dentro del intervalo.
- **Corrección de resumen obsoleto:** si `HandshakeConfidentialMismatch` se genera debido
  Para digerir la deriva, los operadores deben (1) verificar que las ventanas de retención del anulador
  alinear en todo el clúster, (2) ejecutar `iroha_cli app confidential verify-ledger` para
  regenerar el resumen contra el conjunto de anuladores retenido y (3) volver a implementar el
  manifiesto actualizado. Cualquier anulador podado prematuramente debe ser restaurado desde
  cold storage before rejoining the network.Documentar las anulaciones locales en el runbook de operaciones; políticas de gobernanza que se extienden
la ventana de retención debe actualizar la configuración del nodo y los planes de almacenamiento de archivos en
paso a paso.

### Flujo de desalojo y recuperación

1. Durante el marcado, `IrohaNetwork` compara las capacidades anunciadas. Cualquier discrepancia genera `HandshakeConfidentialMismatch`; la conexión se cierra y el par permanece en la cola de descubrimiento sin ser promovido a `Ready`.
2. La falla aparece a través del registro de servicio de red (incluido el resumen remoto y el backend) y Sumeragi nunca programa al par para realizar propuestas o votaciones.
3. Los operadores remedian alineando los registros de verificadores y los conjuntos de parámetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) o organizando `next_conf_features` con un `activation_height` acordado. Una vez que el resumen coincide, el siguiente protocolo de enlace se realizará automáticamente.
4. Si un par obsoleto logra transmitir un bloque (por ejemplo, mediante repetición de archivo), los validadores lo rechazan de manera determinista con `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, manteniendo el estado del libro mayor consistente en toda la red.

### Flujo de protocolo de enlace seguro para reproducción

1. Cada intento de salida asigna material de claves Noise/X25519 nuevo. La carga útil del protocolo de enlace que está firmada (`handshake_signature_payload`) concatena las claves públicas efímeras locales y remotas, la dirección de socket anunciada codificada con Norito y, cuando se compila con `handshake_chain_id`, el identificador de cadena. El mensaje se cifra con AEAD antes de salir del nodo.
2. El respondedor vuelve a calcular la carga útil con el orden de clave local/par invertido y verifica la firma Ed25519 incrustada en `HandshakeHelloV1`. Debido a que tanto las claves efímeras como la dirección anunciada son parte del dominio de firma, reproducir un mensaje capturado contra otro par o recuperar una conexión obsoleta falla la verificación de manera determinista.
3. Las banderas de capacidad confidencial y el `ConfidentialFeatureDigest` viajan dentro de `HandshakeConfidentialMeta`. El receptor compara la tupla `{ enabled, assume_valid, verifier_backend, digest }` con su `ConfidentialHandshakeCaps` configurada localmente; cualquier discrepancia sale temprano con `HandshakeConfidentialMismatch` antes de que el transporte pase a `Ready`.
4. Los operadores DEBEN volver a calcular el resumen (a través de `compute_confidential_feature_digest`) y reiniciar los nodos con los registros/políticas actualizados antes de volver a conectarse. Los pares que anuncian resúmenes antiguos siguen fallando en el protocolo de enlace, lo que impide que el estado obsoleto vuelva a ingresar al conjunto de validadores.
5. Los éxitos y fracasos del protocolo de enlace actualizan los contadores estándar `iroha_p2p::peer` (`handshake_failure_count`, ayudantes de taxonomía de errores) y emiten entradas de registro estructuradas etiquetadas con el ID del par remoto y la huella digital resumida. Supervise estos indicadores para detectar intentos de repetición o configuraciones incorrectas durante la implementación.## Gestión de claves y cargas útiles
- Jerarquía de derivación de claves por cuenta:
  - `sk_spend` → `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Las cargas útiles de notas cifradas utilizan AEAD con claves compartidas derivadas de ECDH; Se pueden adjuntar claves opcionales de vista del auditor a los resultados según la política de activos.
- Adiciones de CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, herramientas de auditor para descifrar notas y el asistente `iroha app zk envelope` para producir/inspeccionar sobres de notas Norito sin conexión. Torii expone el mismo flujo de derivación a través de `POST /v2/confidential/derive-keyset`, devolviendo formatos hexadecimal y base64 para que las billeteras puedan recuperar jerarquías clave mediante programación.

## Controles de gas, límites y DoS
- Horario de gas determinista:
  - Halo2 (Plonkish): gas base `250_000` + gas `2_000` por entrada pública.
  - Gas `5` por byte de prueba, más cargos por anulador (`300`) y por compromiso (`500`).
  - Los operadores pueden anular estas constantes mediante la configuración del nodo (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); Los cambios se propagan al inicio o cuando la capa de configuración se recarga en caliente y se aplican de manera determinista en todo el clúster.
- Límites estrictos (valores predeterminados configurables):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Las pruebas que exceden `verify_timeout_ms` anulan la instrucción de manera determinista (las boletas de gobernanza emiten `proof verification exceeded timeout`, `VerifyProof` devuelve un error).
- Cuotas adicionales garantizan la vida: constructores de bloques enlazados `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` e `max_public_inputs`; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) rige la retención en los puntos de control fronterizos.
- La ejecución en tiempo de ejecución ahora rechaza las transacciones que exceden estos límites por transacción o por bloque, lo que emite errores deterministas `InvalidParameter` y deja el estado del libro mayor sin cambios.
- Mempool filtra previamente las transacciones confidenciales por `vk_id`, longitud de la prueba y antigüedad del anclaje antes de invocar al verificador para mantener limitado el uso de recursos.
- La verificación se detiene de manera determinista cuando se agota el tiempo de espera o se viola el límite; las transacciones fallan con errores explícitos. Los backends SIMD son opcionales pero no alteran la contabilidad del gas.

### Líneas base de calibración y puertas de aceptación
- **Plataformas de referencia.** Las ejecuciones de calibración DEBEN cubrir los tres perfiles de hardware siguientes. Las ejecuciones que no logran capturar todos los perfiles se rechazan durante la revisión.| Perfil | Arquitectura | CPU/instancia | Banderas del compilador | Propósito |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) o Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Establecer valores mínimos sin vectores intrínsecos; Se utiliza para ajustar las tablas de costos alternativos. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | versión predeterminada | Valida la ruta AVX2; comprueba que las aceleraciones SIMD se mantengan dentro de la tolerancia del gas neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versión predeterminada | Garantiza que el backend de NEON siga siendo determinista y alineado con los cronogramas x86. |

- **Arnés de referencia.** Todos los informes de calibración de gas DEBEN producirse con:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar el accesorio determinista.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` cada vez que cambian los costos del código de operación de VM.

- **Aleatoriedad fija.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de ejecutar bancos para que `iroha_test_samples::gen_account_in` cambie a la ruta determinista `KeyPair::from_seed`. El arnés imprime `IROHA_CONF_GAS_SEED_ACTIVE=…` una vez; si falta la variable, la revisión DEBE fallar. Cualquier utilidad de calibración nueva debe seguir respetando esta var env al introducir aleatoriedad auxiliar.

- **Captura de resultados.**
  - Cargue resúmenes de criterios (`target/criterion/**/raw.csv`) para cada perfil en el artefacto de lanzamiento.
  - Almacenar métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) en `docs/source/confidential_assets_calibration.md` junto con la confirmación de git y la versión del compilador utilizadas.
  - Mantener las dos últimas líneas base por perfil; elimine las instantáneas más antiguas una vez que se valide el informe más reciente.

- **Tolerancias de aceptación.**
  - Los deltas de gas entre `baseline-simd-neutral` e `baseline-avx2` DEBEN permanecer ≤ ±1,5%.
  - Los deltas de gas entre `baseline-simd-neutral` e `baseline-neon` DEBEN permanecer ≤ ±2,0%.
  - Las propuestas de calibración que superen estos umbrales requieren ajustes de cronograma o un RFC que explique la discrepancia y la mitigación.

- **Review checklist.** Submitters are responsible for:
  - Incluir extractos de `uname -a`, `/proc/cpuinfo` (modelo, paso a paso) e `rustc -Vv` en el registro de calibración.
  - Verificación de que `IROHA_CONF_GAS_SEED` repitió en la salida del banco (los bancos imprimen la semilla activa).
  - Garantizar que las banderas de funciones de marcapasos y verificadores confidenciales reflejen la producción (`--features confidential,telemetry` cuando se ejecutan bancos con Telemetría).

## Configuración y operaciones
- `iroha_config` gana la sección `[confidential]`:
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
- La telemetría emite métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` e `confidential_policy_transitions_total`, y nunca expone datos de texto sin formato.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Estrategia de prueba
- Determinismo: la mezcla aleatoria de transacciones dentro de bloques produce raíces de Merkle y conjuntos de anuladores idénticos.
- Resiliencia de reorganización: simula reorganizaciones de múltiples bloques con anclajes; los anuladores permanecen estables y las anclas obsoletas se rechazan.
- Invariantes de gas: verifique el uso de gas idéntico en todos los nodos con y sin aceleración SIMD.
- Pruebas de límites: pruebas en techos de tamaño/gas, recuentos máximos de entrada/salida, cumplimiento del tiempo de espera.
- Ciclo de vida: operaciones de gobierno para la activación/desaprobación de verificadores y parámetros, pruebas de gasto de rotación.
- Política FSM: transiciones permitidas/no permitidas, retrasos de transición pendientes y rechazo de mempool alrededor de alturas efectivas.
- Emergencias de registro: el retiro de emergencia congela los activos afectados en `withdraw_height` y rechaza las pruebas posteriores.
- Control de capacidad: validadores con bloques de rechazo `conf_features` no coincidentes; los observadores con `assume_valid=true` se mantienen al día sin afectar el consenso.
- Equivalencia de estado: los nodos validador/completo/observador producen raíces de estado idénticas en la cadena canónica.
- Fuzzing negativo: pruebas mal formadas, cargas útiles sobredimensionadas y colisiones de anuladores se rechazan de forma determinista.

## Trabajo excepcional
- Compare los conjuntos de parámetros de Halo2 (tamaño del circuito, estrategia de búsqueda) y registre los resultados en el libro de jugadas de calibración para que los valores predeterminados de gas/tiempo de espera se puedan actualizar junto con la próxima actualización de `confidential_assets_calibration.md`.
- Finalizar las políticas de divulgación del auditor y las API de visualización selectiva asociadas, conectando el flujo de trabajo aprobado a Torii una vez que se apruebe el borrador de gobernanza.
- Ampliar el esquema de cifrado de testigos para cubrir salidas de múltiples destinatarios y memorandos por lotes, documentando el formato del sobre para los implementadores del SDK.
- Commission an external security review of circuits, registries, and parameter-rotation procedures and archive the findings next to the internal audit reports.
- Especificar las API de conciliación del gasto del auditor y publicar una guía de alcance de clave de vista para que los proveedores de billeteras puedan implementar la misma semántica de certificación.## Fases de implementación
1. **Fase M0 — Endurecimiento de parada de envío**
   - ✅ La derivación del anulador ahora sigue el diseño Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) con un orden de compromiso determinista aplicado en las actualizaciones del libro mayor.
   - ✅ La ejecución aplica límites de tamaño de prueba y cuotas confidenciales por transacción/por bloque, rechazando transacciones que exceden el presupuesto con errores deterministas.
   - ✅ El protocolo de enlace P2P anuncia `ConfidentialFeatureDigest` (resumen de backend + huellas digitales de registro) y falla en las discrepancias de forma determinista a través de `HandshakeConfidentialMismatch`.
   - ✅ Elimine los pánicos en rutas de ejecución confidenciales y agregue control de roles para nodos sin capacidad de coincidencia.
   - ⚪ Hacer cumplir los presupuestos de tiempo de espera de los verificadores y reorganizar los límites de profundidad para los puntos de control fronterizos.
     - ✅ Se aplican presupuestos de tiempo de espera de verificación; Las pruebas que superan `verify_timeout_ms` ahora fallan de forma determinista.
     - ✅ Los puntos de control fronterizos ahora respetan `reorg_depth_bound`, eliminando los puntos de control más antiguos que la ventana configurada y manteniendo instantáneas deterministas.
   - Introducir `AssetConfidentialPolicy`, política FSM y puertas de cumplimiento para instrucciones de acuñación/transferencia/revelación.
   - Confirme `conf_features` en los encabezados de bloque y rechace la participación del validador cuando los resúmenes de registros/parámetros diverjan.
2. **Fase M1: Registros y parámetros**
   - Registros terrestres `ZkVerifierEntry`, `PedersenParams` e `PoseidonParams` con operaciones de gobernanza, anclaje de génesis y administración de caché.
   - Cablear llamada al sistema para requerir búsquedas de registro, ID de programación de gas, hash de esquema y comprobaciones de tamaño.
   - Incluye formato de carga útil cifrada v1, vectores de derivación de claves de billetera y compatibilidad con CLI para administración de claves confidenciales.
3. **Fase M2: Gas y rendimiento**
   - Implementar programación determinista de gas, contadores por bloque y arneses de referencia con telemetría (verificar latencia, tamaños de prueba, rechazos de mempool).
   - Reforzar los puntos de control de CommitmentTree, la carga de LRU y los índices anulados para cargas de trabajo de múltiples activos.
4. **Fase M3: Rotación y herramientas de cartera**
   - Habilitar la aceptación de pruebas de múltiples parámetros y múltiples versiones; Admite la activación/desuso impulsada por la gobernanza con runbooks de transición.
   - Ofrecer flujos de migración de SDK/CLI de billetera, flujos de trabajo de escaneo del auditor y herramientas de conciliación de gastos.
5. **Fase M4: Auditoría y operaciones**
   - Proporcionar flujos de trabajo clave del auditor, API de divulgación selectiva y runbooks operativos.
   - Programe una revisión externa de criptografía/seguridad y publique los hallazgos en `status.md`.

Cada fase actualiza los hitos de la hoja de ruta y las pruebas asociadas para mantener garantías de ejecución deterministas para la red blockchain.

### SDK y cobertura de accesorios (Fase M1)

La carga útil cifrada v1 ahora se envía con accesorios canónicos para que cada SDK produzca el
mismos sobres Norito y hashes de transacciones. Los artefactos dorados viven en
`fixtures/confidential/wallet_flows_v1.json` y son ejercidos directamente por el
Suites Rust y Swift (`crates/iroha_data_model/tests/confidential_wallet_fixtures.rs`,
`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialWalletFixturesTests.swift`):

```bash
# Rust parity (verifies the signed hex + hash for every case)
cargo test -p iroha_data_model confidential_wallet_fixtures

# Swift parity (builds the same envelopes via TxBuilder/NativeBridge)
cd IrohaSwift && swift test --filter ConfidentialWalletFixturesTests
```Cada dispositivo registra el identificador del caso, el hexadecimal de la transacción firmada y el valor esperado.
picadillo. Cuando el codificador Swift aún no puede producir el caso: `zk-transfer-basic` es
todavía cerrado por el constructor `ZkTransfer`: el conjunto de pruebas emite `XCTSkip`, por lo que el
La hoja de ruta rastrea claramente qué flujos aún requieren vinculaciones. Actualizando el aparato
archivo sin modificar la versión del formato fallará en ambas suites, manteniendo los SDK
e implementación de referencia de Rust en bloque.

#### Constructores rápidos
`TxBuilder` expone ayudas asincrónicas y basadas en devolución de llamadas para cada
solicitud confidencial (`IrohaSwift/Sources/IrohaSwift/TxBuilder.swift:1183`).
Los constructores confían en las exportaciones `connect_norito_bridge`
(`crates/connect_norito_bridge/src/lib.rs:3337`,
`IrohaSwift/Sources/IrohaSwift/NativeBridge.swift:1014`) so the generated
Las cargas útiles coinciden con los codificadores del host Rust byte por byte. Ejemplo:

```swift
let account = AccountId.make(publicKey: keypair.publicKey, domain: "wonderland")
let request = RegisterZkAssetRequest(
    chainId: chainId,
    authority: account,
    assetDefinitionId: "rose#wonderland",
    zkParameters: myZkParams,
    ttlMs: 60_000
)
let envelope = try TxBuilder(client: client)
    .buildRegisterZkAsset(request: request, keypair: keypair)
try await TxBuilder(client: client)
    .submit(registerZkAsset: request, keypair: keypair)
```

El blindaje/desprotección sigue el mismo patrón (`submit(shield:)`,
`submit(unshield:)`), y las pruebas del dispositivo Swift vuelven a ejecutar los constructores con
material clave determinista para garantizar que los hashes de transacción generados permanezcan
iguales a los almacenados en `wallet_flows_v1.json`.

#### Constructores de JavaScript
El SDK de JavaScript refleja los mismos flujos a través de los ayudantes de transacciones exportados.
de `javascript/iroha_js/src/transaction.js`. Constructores como
`buildRegisterZkAssetTransaction` y `buildRegisterZkAssetInstruction`
(`javascript/iroha_js/src/instructionBuilders.js:1832`) normalizar clave de verificación
identificadores y emiten cargas útiles Norito que el host Rust puede aceptar sin ningún tipo de
adaptadores. Ejemplo:

```js
import {
  buildRegisterZkAssetTransaction,
  signTransaction,
  ToriiClient,
} from "@hyperledger/iroha";

const unsigned = buildRegisterZkAssetTransaction({
  registration: {
    authority: "i105...",
    assetDefinitionId: "rose#wonderland",
    zkParameters: {
      commit_params: "vk_shield",
      reveal_params: "vk_unshield",
    },
    metadata: { displayName: "Rose (Shielded)" },
  },
  chainId: "00000000-0000-0000-0000-000000000000",
});
const signed = signTransaction(unsigned, myKeypair);
await new ToriiClient({ baseUrl: "https://torii" }).submitTransaction(signed);
```

Los constructores de escudo, transferencia y desprotección siguen el mismo patrón, dándole a JS
quienes llaman tienen la misma ergonomía que Swift y Rust. Pruebas bajo
`javascript/iroha_js/test/transactionBuilder.test.js` cubre la normalización
lógica, mientras que los accesorios anteriores mantienen consistentes los bytes de transacción firmados.

### Telemetría y Monitoreo (Fase M2)

La fase M2 ahora exporta el estado de CommitmentTree directamente a través de Prometheus e Grafana:

- `iroha_confidential_tree_commitments`, `iroha_confidential_tree_depth`, `iroha_confidential_root_history_entries` e `iroha_confidential_frontier_checkpoints` exponen la frontera viva de Merkle por activo, mientras que `iroha_confidential_root_evictions_total` / `iroha_confidential_frontier_evictions_total` cuentan los recortes de LRU aplicados por `zk.root_history_cap` y la ventana de profundidad del punto de control.
- `iroha_confidential_frontier_last_checkpoint_height` e `iroha_confidential_frontier_last_checkpoint_commitments` publican el recuento de altura + compromiso del punto de control fronterizo más reciente para que los simulacros de reorganización y los retrocesos puedan demostrar que los puntos de control avanzan y retienen el volumen de carga útil esperado.
- La placa Grafana (`dashboards/grafana/confidential_assets.json`) incluye una serie de profundidad, paneles de tasa de desalojo y los widgets de caché de verificadores existentes para que los operadores puedan demostrar que la profundidad de CommitmentTree nunca colapsa incluso cuando los puntos de control se agitan.
- La alerta `ConfidentialTreeDepthZero` (en `dashboards/alerts/confidential_assets_rules.yml`) se activa una vez que se observan los compromisos, pero la profundidad informada se mantiene en cero durante cinco minutos.

Puede verificar las métricas localmente antes de cablear Grafana:

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

Combine esto con `rg 'iroha_confidential_tree_depth'` en el mismo raspado para confirmar que la profundidad crece con nuevos compromisos, mientras que los contadores de desalojo solo aumentan cuando el historial limita las entradas recortadas. Estos valores deben coincidir con la exportación del panel Grafana que adjunta a los paquetes de evidencia de gobernanza.

#### Telemetría y alertas del horario de gasolinaLa Fase M2 también incorpora los multiplicadores de gas configurables al canal de telemetría para que los operadores puedan demostrar que cada validador comparte los mismos costos de verificación antes de aprobar una liberación:

- `iroha_confidential_gas_base_verify` refleja `confidential.gas.proof_base` (predeterminado `250_000`).
- `iroha_confidential_gas_per_public_input`, `iroha_confidential_gas_per_proof_byte`, `iroha_confidential_gas_per_nullifier` e `iroha_confidential_gas_per_commitment` reflejan sus respectivas perillas en `ConfidentialConfig`. Los valores se actualizan al inicio y cada vez que la configuración se recarga en caliente; `irohad` (`crates/irohad/src/main.rs:1591,1642`) envía la programación activa a través de `Telemetry::set_confidential_gas_schedule`.

Raspe los indicadores junto a las métricas de CommitmentTree para confirmar que las perillas sean idénticas en todos los pares:

```bash
# compare active multipliers across validators
for host in validator-a validator-b validator-c; do
  curl -s "http://$host:8180/metrics" \
    | rg 'iroha_confidential_gas_(base_verify|per_public_input|per_proof_byte|per_nullifier|per_commitment)'
done
```

El tablero Grafana `confidential_assets.json` ahora incluye un panel de "Programación de gasolina" que muestra los cinco indicadores y resalta la divergencia. Alert rules in `dashboards/alerts/confidential_assets_rules.yml` cover:
- `ConfidentialGasMismatch`: verifica el máximo/mínimo de cada multiplicador en todos los objetivos y páginas de raspado cuando alguno diverge durante más de 3 minutos, lo que solicita a los operadores alinear `confidential.gas` mediante recarga en caliente o reimplementación.
- `ConfidentialGasTelemetryMissing`: advierte cuando Prometheus no puede eliminar ninguno de los cinco multiplicadores durante 5 minutos, lo que indica que falta un objetivo de raspado o que la telemetría está deshabilitada.

Tenga a mano el siguiente PromQL para investigaciones de guardia:

```promql
# ensure every multiplier matches across validators (uses the same projection as the alert)
(max without(instance, job) (iroha_confidential_gas_per_public_input)
  - min without(instance, job) (iroha_confidential_gas_per_public_input)) == 0
```

La desviación debe permanecer cero fuera de las implementaciones de configuración controladas. Al cambiar la tabla de gases, capture los raspados antes y después, adjúntelos a la solicitud de cambio y actualice `docs/source/confidential_assets_calibration.md` con los nuevos multiplicadores para que los revisores de gobernanza puedan vincular la evidencia de telemetría al informe de calibración.