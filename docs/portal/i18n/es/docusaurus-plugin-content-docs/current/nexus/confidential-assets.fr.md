---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Actifs confidentiels et transferts ZK
descripción: Plano Fase C para la circulación ciega, los registros y los controles operativos.
slug: /nexus/activos-confidenciales
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseño de actividades confidenciales y transferencias ZK

## Motivación
- Librero de flujo de actifs blindes opt-in para que los dominios preserven la privacidad de las transacciones sin modificar la circulación transparente.
- Guarde una ejecución determinada en hardware heterogéneo de validadores y conservadores Norito/Kotodama ABI v1.
- Donner aux auditeurs et operators des controles de Cycle de vie (activación, rotación, revocación) para les circuitos y parámetros criptográficos.

## Modelo de amenaza
- Los validadores son honestos pero curiosos: ejecutan el consenso fiel del inspector del libro mayor/estado.
- Les observateurs reseau voient les donnees de bloc et transactions gossipees; Aucune hipothese de canaux de gossip prives.
- Fuera de alcance: analizar el tráfico fuera del libro mayor, adversarios cuánticos (suivi sous la hoja de ruta PQ), ataques de disponibilidad del libro mayor.## Vista del conjunto del diseño.
- Los activos pueden declarar un *grupo blindado* en más de los saldos transparentes existentes; la circulación ciega está representada a través de los compromisos criptográficos.
- Las notas encapsuladas `(asset_id, amount, recipient_view_key, blinding, rho)` con:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independiente del orden de las notas.
  - Chiffre de carga útil: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Las transacciones de transporte de cargas útiles `ConfidentialTransfer` codifican el contenido Norito:
  - Entradas públicas: ancla Merkle, anuladores, nuevos compromisos, identificación de activos, versión de circuito.
  - Chiffres de carga útil para destinatarios y auditores opcionales.
  - Preuve conocimiento cero atestado conservación de valor, propiedad y autorización.
- Las claves de verificación y conjuntos de parámetros son controles a través de los registros en el libro mayor con ventanas de activación; les noeuds rechazant de valider desproofs qui referencent des entrees inconnues ou revoquees.
- Los encabezados de consenso involucran el resumen de funciones confidenciales activas a fin de que los bloques se acepten solo si el registro/parámetros correspondientes.
- La construcción de pruebas utiliza una pila Halo2 (Plonkish) sin configuración confiable; Groth16 u otras variantes de SNARK no son compatibles con la v1.

### Los accesorios son determinantesLes sobres de notas confidenciales libres desormais un accesorio canónico en `fixtures/confidential/encrypted_payload_v1.json`. El conjunto de datos captura un sobre v1 positivo más echantillons malformados porque los SDK pueden afirmar la parte de análisis. Las pruebas del modelo de datos Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) cargan directamente el dispositivo, garantizando que la codificación Norito, las superficies de error y la cobertura de regresión permanecen alineadas durante la evolución del códec.

Los SDK Swift pueden mantener instrucciones de escudo sin pegamento JSON a medida: construir un
`ShieldRequest` con el compromiso de 32 bytes, el código de carga útil y los metadatos de débito,
Luego llame `IrohaSDK.submit(shield:keypair:)` (o `submitAndWait`) para firmar y transmitir la
transacción a través de `/v1/pipeline/transactions`. Le ayudante valida los longueurs de compromiso,
Inserte `ConfidentialEncryptedPayload` en el codificador Norito y refleje el diseño `zk::Shield`.
Ci-dessous afin que las billeteras se sincronizan con Rust.## Compromisos de consenso y activación de capacidades
- Les encabezados de bloques expuestos `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; El resumen participa en el hash de consenso y debe ser igual a la vista local del registro para aceptar un bloque.
- La gobernanza puede estar en escena de actualizaciones programadas `next_conf_features` con un `activation_height` futuro; Justqu a cette hauteur, les producteurs de blocs doivent continuer a emettre le dig precedente.
- Los nuevos validadores DOIVENT funcionan con `confidential.enabled = true` e `assume_valid = false`. Los cheques de desembolso rechazados entran en el conjunto de validadores si unas condiciones hacen eco o si el `conf_features` local diverge.
- Los metadatos del protocolo de enlace P2P incluyen `{ enabled, assume_valid, conf_features }`. Les peers annoncant des feature non prises en charge sont rechazados avec `HandshakeConfidentialMismatch` et n entrent jamais dans la rotación de consenso.
- Los resultados del protocolo de enlace entre validadores, observadores y pares se capturan en la matriz de protocolo de enlace en [Negociación de capacidad de nodo] (#node-capability-negotiation). Les echac de handshake exponennt `HandshakeConfidentialMismatch` y gardent le peer fuera de la rotación de consenso justo como son digest corresponden.
- Los observadores no validadores pueden definir `assume_valid = true`; ils appliquent les deltas confidiels a l aveugle mais n influencent pas la securite du consenso.## Políticas activas
- Cada definición de actividad de transporte un `AssetConfidentialPolicy` defini par le createur ou via Governance:
  - `TransparentOnly`: modo por defecto; Solo las instrucciones transparentes (`MintAsset`, `TransferAsset`, etc.) son permitidas y las operaciones protegidas son rechazadas.
  - `ShieldedOnly`: toda emisión y toda transferencia deben utilizarse instrucciones confidenciales; `RevealConfidential` est interdit pour que les balances ne soient jamais expone publiquement.
  - `Convertible`: Los soportes pueden cambiar el valor entre representaciones transparentes y blindadas mediante las instrucciones de la rampa de entrada/salida ci-dessous.
- Les politiques suivent un FSM contraint pour evitarr les fonds bloques:
  - `TransparentOnly -> Convertible` (activación inmediata de piscina blindada).
  - `TransparentOnly -> ShieldedOnly` (requiere colgante de transición y ventana de conversión).
  - `Convertible -> ShieldedOnly` (imposición de retraso mínimo).
  - `ShieldedOnly -> Convertible` (plan de migración requerido para que las notas protegidas se puedan gastar).
  - `ShieldedOnly -> TransparentOnly` est interdit sauf si le blinded pool est vide ou si la gobernanza codifica una migración que desprotege las notas restantes.- Les instrucciones de gobierno fijan `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` vía l ISI `ScheduleConfidentialPolicyTransition` y pueden anular los programas de cambios con `CancelConfidentialPolicyTransition`. La validación del mempool asegura que cualquier transacción no controle la hauteur de transición y la inclusión se haga eco determinista si un control de política cambia en el medio del bloque.
- Las transiciones colgantes se aplican automáticamente a la apertura de un nuevo bloque: una vez que la alta velocidad entre las ventanas de conversión (para las actualizaciones `ShieldedOnly`) o atteint `effective_height`, el tiempo de ejecución se reunió con el día `AssetConfidentialPolicy`, rafraichit los metadatos `zk.policy` y borre la entrada colgante. Si el suministro transparente permanece no nulo cuando llega una transición `ShieldedOnly`, el tiempo de ejecución anula el cambio y registra un aviso, dejando intacto el modo precedente.
- Las perillas de configuración `policy_transition_delay_blocks` e `policy_transition_window_blocks` imponen un mínimo previo y períodos de gracia para permitir que las billeteras se conviertan automáticamente en el interruptor.
- `pending_transition.transition_id` sirve además del mango de auditoría; La gobernanza debe citar lors de la finalización o anulación para que los operadores puedan correler las relaciones de entrada y salida.
- `policy_transition_window_blocks` tiene un valor predeterminado de 720 (~12 horas con un tiempo de bloqueo de 60 s). Les noeuds Clampent les requetes de Governance qui tentent un preavis plus Court.- Génesis manifiesta y fluye CLI exponen las políticas corrientes y pendientes. La lógica de admisión enciende la política en el momento de la ejecución para confirmar que cada instrucción confidencial está autorizada.
- Lista de verificación de migración: consulte "Secuenciación de migración" para el plan de actualización de las etapas siguientes del Milestone M0.

#### Monitoreo de transiciones vía Torii

Wallets et auditeurs interrogent `GET /v1/confidential/assets/{definition_id}/transitions` pour inspecter l `AssetConfidentialPolicy` actif. La carga útil JSON incluye siempre el ID de activo canónico, la última altura de bloque observada, el `current_mode` de la política, el modo efectivo a esta altura (las barreras de conversión temporales `Convertible`), y los identificadores presentes en `vk_set_hash`/Poseidón/Pedersen. Cuando una transición de gobernanza está en atención a la respuesta incorporada también:

- `transition_id` - identificador de auditoría d identificador par `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` y el `window_open_height` derivan (el bloque o las billeteras no deben comenzar la conversión para los cut-overs ShieldedOnly).

Ejemplo de respuesta:

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

Una respuesta `404` indica qué definición de actividad correspondiente existe. Lorsqu aucune transición n est planifiee le champ `pending_transition` est `null`.

### Máquinas de estados políticos| Modo actual | Modo siguiente | Requisitos previos | Gestion de la hauteur efectiva | Notas |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | Gobernanza activa de las entradas de registro de verificador/parámetros. Soumettre `ScheduleConfidentialPolicyTransition` con `effective_height >= current_height + policy_transition_delay_blocks`. | La transición se ejecuta exactamente en `effective_height`; La piscina protegida está disponible inmediatamente.        | Chemin par defaut pour activer la confidencialite tout engardant les flux transparents. |
| Sólo transparente | Sólo blindado | Ídem ci-dessus, más `policy_transition_window_blocks >= 1`.                                                         | El tiempo de ejecución entre automáticamente en `Convertible` a `effective_height - policy_transition_window_blocks`; pase un `ShieldedOnly` a `effective_height`. | La función de conversión se determina antes de desactivar las instrucciones transparentes. || Descapotable | Sólo blindado | Programa de transición con `effective_height >= current_height + policy_transition_delay_blocks`. Certificador de gobernanza DEVRAIT (`transparent_supply == 0`) mediante auditoría de metadatos; le runtime l applique au cut-over. | Semantique de fenetre identique. Si el suministro transparente no es nulo en `effective_height`, la transición se realizará con `PolicyTransitionPrerequisiteFailed`. | Verrouille l actif en circulacion entierement confidencialle.                             |
| Sólo blindado | Descapotable | Programa de transición; aucun retrait d urgence actif (`withdraw_height` non defini).                                  | L etat bascule a `effective_height`; Las rampas revelan rouvrent tandis que les notes blinded restent valides.       | Utilice para ventanas de mantenimiento o revisiones de auditores.                               |
| Sólo blindado | Sólo transparente | La gobernanza debe proporcionar `shielded_supply == 0` o preparar un plan `EmergencyUnshield` signe (firmas de requisitos del auditor). | El tiempo de ejecución abre una ventana `Convertible` antes de `effective_height`; A la alta velocidad, las instrucciones confidenciales se repiten durante el tiempo y la actividad se vuelve a realizar en modo transparente únicamente. | Sortie dernier recours. La transición automática anual si una nota confidencial depende de la ventana. || Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` nettoie o changement attente.                                              | `pending_transition` est retirado inmediatamente.                                                                       | Mantener el status quo; indicar para completar.                                         |

Les Transitions Non Listees Ci-Dessus Sont Rejetees Lors De La Soumission Governance. El tiempo de ejecución verifica los requisitos previos antes de aplicar un programa de transición; en cas d echec, il repousse l actif au mode precedente et emet `PolicyTransitionPrerequisiteFailed` vía telemetría y eventos de bloque.

### Secuenciación de migración1. **Preparador de registros:** active todas las entradas verificadas y parámetros de referencia por la política cible. Les noeuds annoncent le `conf_features` resultante para que los pares verifiquen la coherencia.
2. **Planificador de la transición:** soumettre `ScheduleConfidentialPolicyTransition` con un `effective_height` respectant `policy_transition_delay_blocks`. En allant vers `ShieldedOnly`, precisioner une fenetre de conversion (`window >= policy_transition_window_blocks`).
3. **Publicar el operador de guía:** registrar el retorno `transition_id` y difundir un runbook de rampa de entrada/salida. Wallets et auditors s abonnent a `/v1/confidential/assets/{id}/transitions` pour connaitre la hauteur d ouverture de fenetre.
4. **Aplicación de la ventana:** a la apertura, le runtime bascule la politique en `Convertible`, emet `PolicyTransitionWindowOpened { transition_id }`, y comience a rechazar las demandas de gobernanza en conflicto.
5. **Finalizar o finalizar:** en `effective_height`, el tiempo de ejecución verifica los requisitos previos (suministro de cero transparente, paso de retirada de urgencia, etc.). En éxito, la política pasó al modo demandado; en echec, `PolicyTransitionPrerequisiteFailed` est emis, la transición colgante es nettoyee et la politique reste inchangee.6. **Actualizaciones de esquema:** después de una transición reussie, la gobernanza aumenta la versión de esquema del activo (por ejemplo `asset_definition.v2`) y la herramienta CLI exige `confidential_policy` para la serialización de manifiestos. Los documentos de actualización génesis instruyen a los operadores para agregar la configuración política y el registro de empresas antes de canjear los validadores.

Les nouveaux reseaux qui demarrent avec la confidencialite activee codent la politique wantede directement dans genesis. Esto sucede cuando la lista de verificación se ci-dessus lors des cambios posteriores al lanzamiento a fin de que las ventanas de conversión permanezcan determinadas y que las billeteras aient le temps de s ajuster.

### Versión y activación de manifiestos Norito- Les genesis manifests DOIVENT incluyen un `SetParameter` para la clave personalizada `confidential_registry_root`. La carga útil es un Norito JSON correspondiente a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: quitar el campeón (`null`) cuando aún no está activo, sinonir una cadena hexadecimal de 32 bytes (`0x...`) igual al producto hash `compute_vk_set_hash` en las instrucciones del verificador del manifiesto. Les noeudsnegant demarrer si le parametre manque ou si le hash diverge des ecritures registratees codificates.
- Le on-wire `ConfidentialFeatureDigest::conf_rules_version` embarca la versión de diseño del manifiesto. Para las investigaciones v1, el DOIT rester `Some(1)` y el mismo `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Cuando el conjunto de reglas evoluciona, incrementa la constante, regenera los manifiestos y despliega los binarios en bloque; Cambiar las versiones hace rechazar los bloques de los validadores con `ConfidentialFeatureDigestMismatch`.
- Les activación manifiesta DEVRAIENT reagrupador mises a jour de registro, cambios de ciclo de vida de parámetros y transiciones de política afin que le digest reste coherente:
  1. Aplique las mutaciones de los planes de registro (`Publish*`, `Set*Lifecycle`) en una vista sin conexión del estado y calcule el resumen posterior a la activación con `compute_confidential_feature_digest`.
  2. Emmettre `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` utiliza el cálculo de hash para que los pares recuperen el resumen de los memes correctos según las instrucciones intermediarias.3. Agregue las instrucciones `ScheduleConfidentialPolicyTransition`. Cada instrucción debe citar le `transition_id` emisiones por gobierno; Les manifests qui l oublient seront rechazates par le runtime.
  4. Mantenga los bytes del manifiesto, una copia SHA-256 y el resumen utilizado en el plan de activación. Los operadores verifican los tres artefactos antes de votar el manifiesto para evitar las particiones.
- Cuando las implementaciones exigen una transferencia diferente, registre la alta velocidad en un parámetro personalizado (par ex `custom.confidential_upgrade_activation_height`). Cela fournit aux auditeurs une preuve Norito codifica ee que les validadores respetan la ventana de previo avant que le changement de digest prenne effet.## Ciclo de vida de verificadores y parámetros
### Registrar ZK
- El libro mayor `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` o `proving_system` está actualmente fijo en `Halo2`.
- Los pares `(circuit_id, version)` son únicos a nivel mundial; El registro mantiene un índice secundario para las búsquedas de metadatos del circuito. Las tentativas de registro de una pareja duplicada son rechazadas tras la admisión.
- `circuit_id` no es vídeo y `public_inputs_schema_hash` no lo hace (tipo hash Blake2b-32 de la codificación canónica de las entradas públicas del verificador). L admisión rejette les enregistrements qui omettent ces champs.
- Las instrucciones de gobierno incluyen:
  - `PUBLISH` para agregar un entrante `Proposed` con solo metadatos.
  - `ACTIVATE { vk_id, activation_height }` para activación del programador a una época limitada.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar la alta final o las pruebas pueden hacer referencia al plato principal.
  - `WITHDRAW { vk_id, withdraw_height }` para apagado por urgencia; les actifs touches geleront les depenses confidentielles después de retirar la altura justo a la activación de nouvelles entrees.
- La génesis se manifiesta automáticamente con un parámetro personalizado `confidential_registry_root` no corresponde `vk_set_hash` a las entradas activas; la validación croise ce digest avec l etat local du registro avant qu un noeud puisse rejoindre le consenso.- Registrar o marcar un día con un verificador requiere un `gas_schedule_id`; la verificación impone que la entrada sea `Active`, presente en el índice `(circuit_id, version)`, y que las pruebas Halo2 se proporcionen un `OpenVerifyEnvelope` no `circuit_id`, I18NI000000160X, et `public_inputs_schema_hash` correspondientes a la entrada du registro.

### Claves de demostración
- Las claves de prueba se encuentran fuera del libro mayor, pero son referencias de las direcciones identificadoras del contenido (`pk_cid`, `pk_hash`, `pk_len`) publicadas con los metadatos del verificador.
- Las billeteras SDK recuperan las PK, verifican los hashes y se almacenan en la ubicación de la caché.

### Parámetros Pedersen y Poseidón
- Los registros separados (`PedersenParams`, `PoseidonParams`) reflejan los controles de ciclo de vida de los verificadores, chacun con `params_id`, hashes de generadores/constantes, activación, desaprobación y valor de retiro.
- Las fuentes de compromisos y hashes una separación de dominio según `params_id` afin de que la rotación de parámetros no reutiliza nunca los patrones de bits de los conjuntos obsoletos; l ID está embarcado en las notas de compromiso y las etiquetas de dominio anulador.
- Los circuitos soportan la selección de multiparámetros y la verificación; Los conjuntos de parámetros depreces restent gastable jusqu a leur `deprecation_height`, y los conjuntos retirados sont rechazados exactamente a `withdraw_height`.## Orden determinista y anuladores
- Chaque actif maintient un `CommitmentTree` con `next_leaf_index`; les blocs ajoutent les engagements dans un ordre deterministe: iterer les transactions dans l ordre de block; En cada transacción, iterar las salidas blindadas por `output_idx` serializar ascendente.
- `note_position` derivan las compensaciones del árbol más **ne** fait pas partie du nullifier; il ne sert qu aux chemins de member dans le testigo de preuve.
- La estabilidad de los anuladores bajo reorganización está garantizada por el diseño PRF; l input PRF se encuentra `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, y los anclajes hacen referencia a las raíces históricas limitadas de Merkle par `max_anchor_age_blocks`.## Flujo del libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - Requiert la politique d actif `Convertible` o `ShieldedOnly`; l admisión verifique l autorite, recupere `params_id`, echantillonne `rho`, emet le engagement, met a jour l arbre Merkle.
   - Emet `ConfidentialEvent::Shielded` con el nuevo compromiso, raíz delta de Merkle y hash de llamada de transacción para pistas de auditoría.
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - Le syscall VM verifica la prueba a través de la entrada del registro; El anfitrión asegura que los anuladores son inútiles, que los compromisos son más deterministas y que el ancla es reciente.
   - El libro mayor registra las entradas `NullifierSet`, almacena las cargas útiles para destinatarios/auditores y emet `ConfidentialEvent::Transferred` anuladores de resultados, órdenes de salida, hash de prueba y raíces Merkle.
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - Disponible únicamente para los activos `Convertible`; la prueba valide que el valor de la nota egale le montant revele, le ledger credite le balance transparent et brule la note blinded en marquant le nullifier comme depense.- Emet `ConfidentialEvent::Unshielded` avec le montant public, nullifiers consommes, identifiants deproof et hash d apelación de transacción.## Ampliar el modelo de datos
- `ConfidentialConfig` (nueva sección de configuración) con bandera de activación, `assume_valid`, perillas de gas/limites, ventana de anclaje, backend de verificateur.
- Esquemas `ConfidentialNote`, `ConfidentialTransfer` y `ConfidentialMint` Norito con bytes de versión explícita (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` sobre de bytes de nota AEAD con `{ version, ephemeral_pubkey, nonce, ciphertext }`, por defecto `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para el diseño XChaCha20-Poly1305.
- Les vecteurs canoniques de derivation de cle vivent dans `docs/source/confidential_key_vectors.json`; La CLI y el punto final Torii son accesorios de superficie regresivos.
- `asset::AssetDefinition` gagn `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste en el enlace `(backend, name, commitment)` para los verificadores transfer/unshield; La ejecución rechaza las pruebas de verificación de referencia clave o en línea que no corresponden al compromiso de registro.
- `CommitmentTree` (par actif avec frontier checkpoints), `NullifierSet` cle `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` stockes en el estado mundial.
- Mempool mantiene las estructuras transitorias `NullifierIndex` e `AnchorIndex` para detectar precoces de doblones y comprobar la antigüedad del anclaje.
- Las actualizaciones del esquema Norito incluyen un pedido canónico de entradas públicas; Las pruebas de ida y vuelta aseguran el determinismo de la codificación.- Los viajes de ida y vuelta de carga útil cifrada se bloquean mediante pruebas unitarias (`crates/iroha_data_model/src/confidential.rs`). Des vecteurs wallet de suivi adjuntoront des transcripts AEAD canonices pour auditeurs. `norito.md` documente el encabezado en el cable del sobre.

## Integración IVM y syscall
- Introduzca el aceptador de llamada al sistema `VERIFY_CONFIDENTIAL_PROOF`:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` y el `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultante.
  - La llamada al sistema carga el verificador de metadatos después del registro, aplica los límites de tamaño/tiempo, genera un gas determinante y aplica el delta que si la prueba se reutiliza.
- El host expone un rasgo de solo lectura `ConfidentialLedger` para recuperar instantáneas de Merkle root y el estado de anuladores; La biblioteca Kotodama cuenta con ayudantes de ensamblaje de testigos y validación de esquemas.
- Los documentos Pointer-ABI son un día para aclarar el diseño del buffer de prueba y los identificadores del registro.## Negociación de capacidades de noeud
- El apretón de manos anuncia `feature_bits.confidential` con `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. La participación de los validadores requiere `confidential.enabled=true`, `assume_valid=false`, identificadores de backend verificateur identiques y resúmenes correspondientes; Las discrepancias reflejan el protocolo de enlace con `HandshakeConfidentialMismatch`.
- La configuración compatible con `assume_valid` para los observadores solos: cuando se desactiva, se encuentran las instrucciones confidenciales del producto `UnsupportedInstruction` determinista sin pánico; Cuando está activo, los observadores aplican los deltas declaran sin verificador de pruebas.
- Mempool rechaza las transacciones confidenciales si la capacidad local está desactivada. Los filtros de chismes evitan el envío de transacciones protegidas entre pares sin capacidades correspondientes y transmiten al verificador ID desconocidos en los límites de tamaño.

### Matriz de apretón de manos| Anuncio lejano | Resultados para validadores | Operador de notas |
|---------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, coincidencia de backend, coincidencia de resumen | Aceptar | Le peer atteint l etat `Ready` et participe a la proposition, au vote, et au fan-out RBC. Aucune acción manuelle requise. |
| `enabled=true`, `assume_valid=false`, coincidencia de backend, resumen obsoleto o ausente | Rechazar (`HandshakeConfidentialMismatch`) | El control remoto debe aplicar el registro/parámetros de activación en atención o asistencia al planificado `activation_height`. Tant que corrige, le noeud reste decouvrable mais n entre jamais en rotación de consenso. |
| `enabled=true`, `assume_valid=true` | Rechazar (`HandshakeConfidentialMismatch`) | Los validadores exigen la verificación de pruebas; Configure el observador remoto como observador con ingreso Torii solamente o bascule `assume_valid=false` después de completar la activación de la verificación. |
| `enabled=false`, campeones omitidos (compilación obsoleta), o verificador de backend diferente | Rechazar (`HandshakeConfidentialMismatch`) | Las actualizaciones parciales o obsoletas de pares no pueden unirse a la búsqueda de consenso. Continúe el día con la corriente de lanzamiento y asegúrese de que la tupla backend + digest corresponda antes de la reconexión. |Los observadores que omiten voluntariamente la verificación de las pruebas no deben lograr un consenso en las conexiones con los validadores activos. Es posible que siempre se ingieran bloques a través de Torii o API de archivo, pero la búsqueda de consenso les rechaza justo lo que se anuncia con las capacidades correspondientes.

### Política de poda, revelación y retención de anuladores.

Los libros de contabilidad confidenciales deben conservar el análisis histórico para comprobar la fraicheur des notes et rejouer des audits gouvernance. La política por defecto, aplicada por `ConfidentialLedger`, est:- **Retención de anuladores:** conserva los anuladores dependiendo de un *mínimo* de `730` días (24 meses) después de la alta dependencia, o la ventana impuesta por el regulador si es más larga. Los operadores pueden abrir la ventana a través de `confidential.retention.nullifier_days`. Les nullifiers plus jeunes que la fenetre DOIVENT rester interrogeables via Torii afin que les auditeurs prouvent l ausencia de doble gasto.
- **Poda de revelaciones:** les revela transparencias (`RevealConfidential`) poda les compromisos asociados inmediatamente después de la finalización del bloque, pero le anula el consumo restante a la regla de retención ci-dessus. Los eventos `ConfidentialEvent::Unshielded` registran el montante público, el destinatario y el hash de prueba para que la reconstrucción de las revelaciones históricas no requiera la eliminación del texto cifrado.
- **Puntos de control fronterizos:** las fronteras de compromiso mantienen los puntos de control roulant couvrant le plus grand de `max_anchor_age_blocks` et de la fenetre de retención. Les noeuds compactant les checkpoints plus anciens seulement apres expiration de tous les nullifiers dans l intervalole.- **Resumen de remediación obsoleto:** si `HandshakeConfidentialMismatch` sobrevive a una causa de una deriva de resumen, los operadores deben (1) verificar que los protectores de retención de anuladores están alineados en el clúster, (2) lanzar `iroha_cli app confidential verify-ledger` para regenerar el resumen en el conjunto de anuladores retenidos y (3) redistribuir el manifiesto. rafraichi. Tout nullifier prune trop tot doit etre restaure depuis le stockage froid avant de rejoindre le reseau.

Documente las anulaciones locales en el runbook de operaciones; Las políticas de gobernanza que etenden la ventana de retención deben cumplir cada día la configuración de los nuevos y los planes de almacenamiento y archivo en paralelo.

### Flujo de desalojo y recuperación1. Colgante del dial, `IrohaNetwork` compara las capacidades anunciadas. Todo nivel de falta de coincidencia `HandshakeConfidentialMismatch`; La conexión está cerrada y el par restante en el archivo de descubrimiento sin estar disponible en `Ready`.
2. L echec est expuesto a través del registro del servicio de investigación (incluido el resumen remoto y el backend), y Sumeragi no planifica nunca al par para proponer o votar.
3. Los operadores deben alinear los registros verificadores y conjuntos de parámetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) o programar `next_conf_features` con un `activation_height` convenu. Una vez que la digestión se alinea, el apretón de manos de la cadena se reutiliza automáticamente.
4. Si un par obsoleto reutiliza un difusor en bloque (por ejemplo, a través de la reproducción de un archivo), los validadores le rechazan el determinismo con `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, gardant l etat du ledger coherent dans le reseau.

### Flujo de protocolo de enlace seguro para reproducción1. Chaque tentative sortante alloue un nouveau material de cle Noise/X25519. La carga útil de la señal de protocolo de enlace (`handshake_signature_payload`) concatena las claves públicas efímeras locales y distantes, la dirección del socket anunciada codificada por Norito y, cuando se compila con `handshake_chain_id`, el identificador de la cadena. Le message est chiffre AEAD avant de quitter le noeud.
2. El respondedor vuelve a calcular la carga útil con el orden de pares/local inverso y verifica la firma Ed25519 embarcada en `HandshakeHelloV1`. Debido a que las dos efímeras y la dirección anunciada son parte del dominio de firma, activan la captura de un mensaje contra otro par o recuperan una conexión obsoleta y determinista.
3. Les flags de capacite confidentielle et le `ConfidentialFeatureDigest` voyagent dans `HandshakeConfidentialMeta`. El receptor compara la tupla `{ enabled, assume_valid, verifier_backend, digest }` con su hijo `ConfidentialHandshakeCaps` local; Todo tipo de falta de coincidencia con `HandshakeConfidentialMismatch` antes de que el transporte no pase a `Ready`.
4. Les operatorurs DOIVENT recomputer le digest (vía `compute_confidential_feature_digest`) et redemarrer les noeuds avec registries/politiques mises a jour avant de reconnecter. Los pares anuncian los resúmenes antiguos y continúan haciendo eco del apretón de manos, empeñando un estado obsoleto de volver a entrar en el conjunto de validadores.5. Les succes et echecs de handshake mettent a jour les compteurs standard `iroha_p2p::peer` (`handshake_failure_count`, helpers de taxonomie d erreur) et emettent des logsstructures avec l ID du peer distant et l empreinte du digest. Vigile estos indicadores para detectar repeticiones o configuraciones incorrectas durante el lanzamiento.

## Gestión de archivos y cargas útiles
- Jerarquía de derivación por cuenta:
  - `sk_spend` -> `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Las cargas útiles de notas chiffre se utilizan AEAD con claves compartidas derivadas de ECDH; Las opciones de vista de claves del auditor pueden incluir agregados aux salidas según la política de la acción.
- Incluye CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, herramienta de auditoría para descifrar notas y ayudante `iroha app zk envelope` para producir/inspeccionar sobres Norito sin conexión. Torii expone el flujo de derivación de memes a través de `POST /v1/confidential/derive-keyset`, que regresa de formas hexadecimales y base64 para que las billeteras puedan recuperar las jerarquías de claves programáticas.## Gas, límites y controles DoS
- Horario de determinación de gas:
  - Halo2 (Plonkish): base `250_000` gas + `2_000` gas por entrada pública.
  - `5` byte de prueba de par de gas, más cargos por anulador (`300`) y compromiso de par (`500`).
  - Los operadores pueden sobrecargar estas constantes a través del nodo de configuración (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); Los cambios se propagan al finalizar o cuando la cama de configuración de recarga en caliente y son aplicaciones deterministas en el clúster.
- Limites dures (configurables por defecto):
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Les pruebas depassant `verify_timeout_ms` abortent l instrucción determinista (votos de gobierno emettent `proof verification exceeded timeout`, `VerifyProof` retourne une erreur).
- Cuotas adicionales que aseguran la vida útil: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` y `max_public_inputs` nacen de los constructores de bloques; `reorg_depth_bound` (>= `max_anchor_age_blocks`) rige la retención de los puntos de control fronterizos.
- El tiempo de ejecución rechaza el mantenimiento de las transacciones que superan los límites de la transacción o del bloque, generando errores `InvalidParameter` determinados y deja intacto el estado del libro mayor.- Mempool prefiltra las transacciones confidenciales según `vk_id`, longitud de prueba y edad de anclaje antes de llamar al verificador para nacer en el uso de los recursos.
- La verificación s arrete deterministiquement sur timeout ou violation de borne; les transactions echouent avec des erreurs explicites. Los backends SIMD son opcionales pero no modifican la compatibilidad del gas.

### Líneas base de calibración y puertas de aceptación
- **Placas-formas de referencia.** Las ejecuciones de calibración DOIVENT cubren los tres perfiles de hardware ci-dessous. Les run ne couvrant pas todos los perfiles sont rechazados en revisión.

  | Perfil | Arquitectura | CPU/instancia | Compilador de banderas | Objetivo |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) o Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Etablit des valeurs plancher sans intrinsics vectorielles; Utilice para regular las tablas de respaldo. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | lanzamiento por defecto | Valide la ruta AVX2; Verifique que los aceleradores SIMD estén en la tolerancia del gas neutro. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | lanzamiento por defecto | Asegúrese de que el backend NEON esté determinado y alineado con los horarios x86. |- **Arnés de referencia.** Todas las relaciones de gas de calibración DOIVENT etre produits avec:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar el accesorio determinado.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` cuando se cambiaron los códigos de operación de VM.

- **Aleatoriedad fija.** Exportador `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de lanzar los bancos para que `iroha_test_samples::gen_account_in` bascule sur la voie deterministe `KeyPair::from_seed`. El arnés imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` una sola vez; si la variable manque, la revisión DOIT echouer. Toute nouvelle utilite de calibración doit continuer a honorer cette env var lors de l introducción d alea auxiliaire.

- **Captura de resultados.**
  - Cargue los currículums Criterion (`target/criterion/**/raw.csv`) para cada perfil en el artefacto de lanzamiento.
  - Almacene las mediciones derivadas (`ns/op`, `gas/op`, `ns/gas`) en el [Libro de calibración de gas confidencial](./confidential-gas-calibration) con el compromiso de git y la versión del compilador que utiliza.
  - Conservar las dos últimas líneas de base por perfil; supprimer les snapshots plus anciens une fois le rapport le plus Recent valide.- **Tolerancias de aceptación.**
  - Les deltas de gas entre `baseline-simd-neutral` et `baseline-avx2` DOIVENT rester <= +/-1.5%.
  - Les deltas de gas entre `baseline-simd-neutral` et `baseline-neon` DOIVENT rester <= +/-2.0%.
  - Las propuestas de calibración eliminadas exigen ajustes de programación o un RFC explicativo de la tarjeta electrónica y la mitigación.

- **Lista de verificación de revisión.** Los remitentes son responsables de:
  - Incluye `uname -a`, extraits de `/proc/cpuinfo` (modelo, paso a paso) y `rustc -Vv` en el registro de calibración.
  - Verifique que `IROHA_CONF_GAS_SEED` apparait dans la sortie bench (les benches impriment la seed active).
  - Asegúrese de que las banderas de función del marcapasos y verifique la confianza del espejo de producción (`--features confidential,telemetry` lors des benches avec Telemetry).

## Configurar operaciones
- `iroha_config` agregue la sección `[confidential]`:
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
- Telemetría emet des mériques agregees: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, et `confidential_policy_transitions_total`, sans Jamais Exposer des Donnees en Clair.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Estrategia de pruebas
- Determinismo: la mezcla aleatoria de transacciones en los bloques de raíces de Merkle y conjuntos anulados idénticos.
- Resiliencia aux reorg: simulación de reorganizaciones multibloques con anclajes; les nullifiers restent stables et les Anchors stale sont rejetes.
- Invariantes de gas: verificador de un uso de gas idéntico entre nuevos valores con y sin aceleración SIMD.
- Pruebas de límite: pruebas aux plafonds de taille/gas, recuentos máximos de entrada/salida, cumplimiento de tiempos de espera.
- Ciclo de vida: operaciones de gobierno para activación/desaprobación de verificador y parámetros, pruebas de dependencia después de la rotación.
- Política FSM: transiciones autorizadas/interditas, demoras de transición colgante, et rejet mempool autour des hauteurs Effectives.
- Urgencias de registro: retrait d urgence fige les actifs a `withdraw_height` et rejette lesproofs apres.
- Control de capacidad: los validadores con `conf_features` no coinciden con los bloques rechazados; observadores con `assume_valid=true` suivent sans afector le consenso.
- Equivalencia de estado: noeuds validator/full/observer produisent desroots d etat identiques sur la chaine canonique.
- Fuzzing negativo: pruebas malformadas, cargas útiles sobredimensionadas y colisiones de anulación rechazadas determinísticamente.## Migración
- Implementación controlada por funciones: justo como se determina la fase C3, `enabled` por defecto es `false`; les noeuds annoncent leurs capacites avant de rejoindre le set de validadores.
- Los activos transparentes no son afectados; Las instrucciones confidenciales requieren el registro de entradas y una negociación de capacidades.
- Les noeuds compiles sans support confidentiel rejettent les blocs pertinents deterministiquement; No es posible volver a unirse al conjunto de validadores, pero es posible que funcionen como observadores con `assume_valid=true`.
- Los manifiestos del Génesis incluyen el registro de las entradas iniciales, los conjuntos de parámetros, las políticas confidenciales para los activos y las claves del auditor opcional.
- Los operadores que siguen los runbooks públicos para la rotación del registro, las transiciones políticas y la retirada de urgencia para mantener las actualizaciones determinadas.## Restante del trabajo de parto
- Compare los conjuntos de parámetros Halo2 (cola de circuito, estrategia de búsqueda) y registre los resultados en el libro de jugadas de calibración para medir cada día los valores predeterminados de gas/tiempo de espera con la actualización de la cadena `confidential_assets_calibration.md`.
- Finalizador de las políticas de divulgación del auditor y de las API de los asociados de visualización selectiva, y cableando el flujo de trabajo aprobado en Torii una vez firmado el borrador de gobernanza.
- Emplear el esquema de cifrado de testigos para cubrir las salidas de múltiples destinatarios y lotes de notas, y documentar el formato de sobre para los implementadores SDK.
- Comisionado de una revista de seguridad externa de circuitos, registros y procedimientos de rotación de parámetros y archivador de conclusiones en una comisión de informes de auditoría interna.
- Especificador de API de reconciliación de gastos para auditores y publicación de la guía de alcance de vista clave para que los proveedores de billeteras implementen los memes semánticos de atestación.## Implementación por fases
1. **Fase M0 - Endurecimiento de parada de envío**
   - [x] La derivación del traje anulador mantiene el diseño Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) con un ordenamiento determinante de los compromisos impuestos en las actualizaciones del libro mayor.
   - [x] L ejecución de apliques de plafones de cola de prueba y de cuotas confidenciales por transacción/par bloque, rechazando las transacciones fuera del presupuesto con errores deterministas.
   - [x] El protocolo de enlace P2P anuncia `ConfidentialFeatureDigest` (resumen de backend + registro de huellas digitales) y detecta las discrepancias determinadas a través de `HandshakeConfidentialMismatch`.
   - [x] Retire los pánicos en los caminos de ejecución confidencial y ajouter un rol gating pour les noeuds non pris en charge.
   - [ ] Aplique los presupuestos de tiempo de espera del verificador y los bornes de profundidad de reorganización para los puntos de control fronterizos.
     - [x] Presupuestos de tiempo de espera de apliques de verificación; lesproofs depassant `verify_timeout_ms` echouent maintenant deterministiquement.
     - [x] Los puntos de control fronterizos respetan el mantenimiento `reorg_depth_bound`, eliminando los puntos de control más antiguos que la ventana se configura todo en función de las instantáneas determinadas.
   - Introducción `AssetConfidentialPolicy`, política FSM y puertas de cumplimiento para las instrucciones mint/transfer/reveal.- Confirme `conf_features` en los encabezados de bloque y rechace la participación de los validadores cuando los resúmenes de registro/parámetros divergentes.
2. **Fase M1 - Registros y parámetros**
   - Libere los registros `ZkVerifierEntry`, `PedersenParams` y `PoseidonParams` con control de operaciones, generación de almacenamiento y gestión de caché.
   - Cableado de llamadas al sistema para registro de búsquedas de imponentes, ID de programación de gas, hash de esquema y comprobaciones de cola.
   - Publica el formato de código de carga útil v1, vectores de derivación de claves para billeteras y admite CLI para la gestión de claves confidenciales.
3. **Fase M2 - Gas y rendimiento**
   - Implementador de cronograma de determinación de gas, computadoras por bloque y arneses de benchmark con telemetría (latencia de verificación, colas de prueba, rechazos de memoria).
   - Crear puntos de control de CommitmentTree, LRU de carga e índices de anulación para cargas de trabajo de múltiples activos.
4. **Fase M3 - Cartera de rotación y herramientas**
   - Activar la aceptación de pruebas multiparámetros y multiversión; partidario l activación/desaprobación pilotado por gobernanza con runbooks de transición.
   - Liberar los flujos de migración SDK/CLI, flujos de trabajo de escaneo auditor y herramientas de reconciliación de gastos.
5. **Fase M4 - Auditoría y operaciones**
   - Proporcionar flujos de trabajo de claves de auditoría, API de divulgación selectiva y runbooks operativos.- Planificador de una revista externa de criptografía/seguridad y publicación de conclusiones en `status.md`.

Esta fase cumplió un día con los hitos de la hoja de ruta y las asociaciones de pruebas para mantener las garantías de ejecución determinantes de la cadena de bloques.