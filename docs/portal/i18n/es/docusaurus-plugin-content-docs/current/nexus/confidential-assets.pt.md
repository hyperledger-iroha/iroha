---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Ativos confidencialis e transferencias ZK
descripción: Anteproyecto Fase C para circulación ciega, registros y controles de operador.
slug: /nexus/activos-confidenciales
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Diseño de activos confidenciales y transferencias ZK

## Motivacao
- Entregar flujos de ativos blindados opt-in para que los dominios preserven la privacidad transacional sin alterar la circulación transparente.
- Manter ejecutado determinista en hardware heterogéneo de validadores y preservar Norito/Kotodama ABI v1.
- Fornecer a auditores y operadores controles de ciclo de vida (ativacao, rotacao, revogacao) para circuitos y parámetros criptográficos.

## Modelo de amenaza
- Validadores sao honesto-pero-curioso: executam consenso fielmente mas tentam inspeccionar ledger/state.
- Observadores de rede veem dados de bloco e transacoes chismes; nenhuma suposicao de canais privados de chismes.
- Foros de escopo: analise de trafego off-ledger, adversarios quanticos (acompanhado no roadmap PQ), ataques de disponibilidade do ledger.## Visao general de diseño
- Los activos pueden declarar un *grupo blindado* además de los saldos transparentes existentes; a circulacao blindada e representada vía compromisos criptográficos.
- Notas encapsuladas `(asset_id, amount, recipient_view_key, blinding, rho)` con:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, independiente del orden de las notas.
  - Carga útil cifrada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transacoes transportam payloads `ConfidentialTransfer` codificados en Norito contendo:
  - Entradas públicas: ancla Merkle, anuladores, compromisos novos, identificación de activos, versao de circuito.
  - Cargas útiles encriptadas para destinatarios y auditores opcionales.
  - Prueba de conocimiento cero que atesta conservación de valor, propiedad e autorización.
- Verificación de claves y conjuntos de parámetros sao controlados a través de registros en el libro mayor con janelas de ativacao; Los nodos recusam validar pruebas que referenciam entradas desconhecidas ou revogadas.
- Headers de consenso comprometem o digest ativo de capacidade confidencial para que blocos sejam aceitos quando o estado de registro y parámetros coincidan.
- Construcción de pruebas usando una pila Halo2 (Plonkish) sin configuración confiable; Groth16 u otras variantes de SNARK no son compatibles intencionalmente con la v1.

### Calendario deterministaSobres de memo confidencial para enviar un accesorio canónico en `fixtures/confidential/encrypted_payload_v1.json`. La captura del conjunto de datos en un sobre v1 positivo pero demostramos negativas malformadas para que los SDK puedan afirmar la paridad del análisis. Las pruebas del modelo de datos en Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y una suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) cargan el dispositivo directamente, garantizando que la codificación Norito, como superficies de error y una cobertura de regreso permanente alineadas en cuanto a la evolución del códec.

SDK Swift ahora puede emitir instrucciones escudo sin pegamento JSON a medida: construya um
`ShieldRequest` con el compromiso de 32 bytes, la carga útil encriptada y los metadatos de débito,
y entao chame `IrohaSDK.submit(shield:keypair:)` (ou `submitAndWait`) para assinar e encaminhar a
transacao vía `/v1/pipeline/transactions`. Oh ayudante valida comprimentos de compromiso,
inserte `ConfidentialEncryptedPayload` sin codificador Norito, y espelha o diseño `zk::Shield`
Se describe a continuación para que wallets fiquem alinhadas com Rust.## Compromisos de consenso y activación de capacidades
- Encabezados de bloco expoem `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; o digerir participa do hash de consenso e debe igualar a visao local do registro para aceitacao de bloco.
- Gobernanza pode preparar actualizaciones programando `next_conf_features` con un `activation_height` futuro; ate essa altura, productores de bloco devem continuar emitiendo o digerir anterior.
- Nodos validadores DEVEM operar com `confidential.enabled = true` e `assume_valid = false`. Las comprobaciones de inicio recusam entrar no set validador se qualquer condicao falhar ou se o `conf_features` local divergir.
- Los metadatos del protocolo de enlace P2P incluyen `{ enabled, assume_valid, conf_features }`. Peers que anunciam feature incompativeis sao rejeitados com `HandshakeConfidentialMismatch` e nunca entram em rotacao de consenso.
- Resultados del protocolo de enlace entre validadores, observadores y pares capturados en la matriz de protocolo de enlace en [Negociación de capacidad de nodo] (#node-capability-negotiation). Falhas de handshake expoem `HandshakeConfidentialMismatch` e mantem o peer fora da rotacao de consenso ate que o digest coincide.
- Observadores nao validadores podem definir `assume_valid = true`; aplicam deltas confidenciais sem verificar pruebas, mas nao influenciam a seguranca do consenso.## Políticas de activos
- Cada definición de gestión de activos um `AssetConfidentialPolicy` definida por el criador o vía la gobernanza:
  - `TransparentOnly`: modo predeterminado; apenas instrucciones transparentes (`MintAsset`, `TransferAsset`, etc.) sao permitidas e operacoes blinded sao rejeitadas.
  - `ShieldedOnly`: todas las emisiones y transferencias deben usar instrucciones confidenciales; `RevealConfidential` e proibido para que balances nunca aparecam públicamente.
  - `Convertible`: soportes podem mover valor entre representacoes transparentes e blindados usando instrucciones de on/off-ramp abaixo.
- Políticas siguientes al restrito del FSM para evitar fondos encalhados:
  - `TransparentOnly -> Convertible` (habilitación inmediata de piscina blindada).
  - `TransparentOnly -> ShieldedOnly` (solicitud transicao pendente e janela de conversao).
  - `Convertible -> ShieldedOnly` (retraso mínimo obligatorio).
  - `ShieldedOnly -> Convertible` (plano de migración requerido para que notes blindadas continuem gastaveis).
  - `ShieldedOnly -> TransparentOnly` e proibido a menos que o blinded pool esteja vazio ou Governance codifique uma migracao que des-blinde notes pendentes.
- Las instrucciones de gobierno definen `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` vía o ISI `ScheduleConfidentialPolicyTransition` y pueden abortar mudancas programadas con `CancelConfidentialPolicyTransition`. A validacao do mempool garante que nenhuma transacao atravesse a altura de transicao e a inclusao falha deterministicamente se um check de politica mudaria no meio do bloco.- Los transicos colgantes se aplican automáticamente cuando se abre un nuevo bloque: cuando la altura entra en la conversación (para actualizaciones `ShieldedOnly`) o la activación de `effective_height`, el tiempo de ejecución se actualiza `AssetConfidentialPolicy`, refresca los metadatos `zk.policy` y limpia la entrada. pendiente. Si el suministro transparente permanece cuando una transicao `ShieldedOnly` amadurece, o runtime aborta a mudanca e registra un aviso, mantendo o modo anterior.
- Los botones de configuración `policy_transition_delay_blocks` e `policy_transition_window_blocks` imponen aviso mínimo y períodos de tolerancia para permitir conversaciones de billetera en torno de mudanca.
- `pending_transition.transition_id` también funciona como manija de auditorio; Governance deve cita-lo ao finalizar ou cancelar transicoes para que operadores correlacionem relatorios de on/off-ramp.
- `policy_transition_window_blocks` predeterminado a 720 (~12 horas con tiempo de bloqueo de 60 s). Los nodos limitan las solicitudes de gobernanza que tienen un aviso más breve.
- Génesis manifiesta e fluxos CLI expoem politicas atuais e pendentes. La lógica de admisión le a política em tempo de ejecución para confirmar que cada instrucción confidencial está autorizada.
- Lista de verificación de migración - ver "Secuenciación de migración" a continuación para el plano de actualización en etapas que acompaña el Milestone M0.

#### Monitorando transicos vía ToriiWallets e auditores consultan `GET /v1/confidential/assets/{definition_id}/transitions` para inspeccionar o `AssetConfidentialPolicy` activo. La carga útil JSON siempre incluye el ID de activo canónico, la última altura del bloque observada, el `current_mode` de la política, el modo efectivo sin altura (janelas de conversao reportam temporalmente `Convertible`), y los identificadores esperados de `vk_set_hash`/Poseidon/Pedersen. Quando uma transicao de Governance esta pendente a resposta tambem embute:

- `transition_id` - manija de auditoria retornada por `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` y `window_open_height` derivado (el bloque de billeteras debe comenzar a conversar para cut-overs ShieldedOnly).

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

Una respuesta `404` indica que no existe ninguna definición de corresponsal de activos. Quando nao ha transicao agendada o campo `pending_transition` e `null`.

### Máquina de estados de política| Modo actual | Proximo modo | Prerrequisitos | Tratamiento de altura efectiva | Notas |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | Gobernanza ativou entradas de registro de verificador/parametros. Submetro `ScheduleConfidentialPolicyTransition` con `effective_height >= current_height + policy_transition_delay_blocks`. | A transicao executa exactamente em `effective_height`; o piscina blindada fica disponivel inmediatamente.               | Camino predeterminado para habilitar la confidencialidad manteniendo flujos transparentes.          |
| Sólo transparente | Sólo blindado | Mesmo acima, mais `policy_transition_window_blocks >= 1`.                                                         | El tiempo de ejecución ingresa automáticamente en `Convertible` en `effective_height - policy_transition_window_blocks`; Muda para `ShieldedOnly` en `effective_height`. | Fornece janela de conversao determinista antes de desabilitar instrucciones transparentes.   || Descapotable | Sólo blindado | Transición programada con `effective_height >= current_height + policy_transition_delay_blocks`. Gobernanza DEVE certificar (`transparent_supply == 0`) vía metadatos de auditoría; La aplicación en tiempo de ejecución no tiene cortes. | Semántica de janela idéntica. Si se suministra transparente para nao-zero en `effective_height`, una transición aborta con `PolicyTransitionPrerequisiteFailed`. | Trava o active em circulacao totalmente confidencial.                                      |
| Sólo blindado | Descapotable | Transición programada; sem retiro de emergencia activo (`withdraw_height` indefinido).                              | O estado muda em `effective_height`; revelar rampas reabrem enquanto notes blindadas permanecem validas.             | Usado para janelas de manutencao ou revisiones de auditores.                                |
| Sólo blindado | Sólo transparente | Governance debe probar `shielded_supply == 0` o preparar un plano `EmergencyUnshield` assinado (asinaturas de auditor requeridas). | O runtime abre una janela `Convertible` antes de `effective_height`; na altura, instrucciones confidenciales falham duro y o activo retorna ao modo transparente-solo. | Saida de último recurso. A transicao se auto-cancela se qualquer note confidencial for gasta durante a janela. || Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` limpia a mudanca pendente.                                                    | `pending_transition` eliminado inmediatamente.                                                                        | Mantem o status quo; mostrado por completo.                                             |

Transicoes nao listadas acima sao rejeitadas durante la presentación de la gobernanza. O runtime checa prerrequisitos logo antes de aplicar una transición programada; Falhas devolvem o active ao modo anterior y emitem `PolicyTransitionPrerequisiteFailed` vía telemetría y eventos de bloco.

### Secuenciación de migración1. **Preparar registros:** ativar todas como entradas de verificador e parámetros referenciadas pela politica alvo. Nodes anunciam o `conf_features` resultante para que peers verifiquem coerencia.
2. **Agenda a transicao:** submeter `ScheduleConfidentialPolicyTransition` con un `effective_height` que respectivamente `policy_transition_delay_blocks`. Ao mover para `ShieldedOnly`, especifique una janela de conversación (`window >= policy_transition_window_blocks`).
3. **Guía pública para operadores:** registrador o `transition_id` retornado e circular um runbook on/off-ramp. Wallets e auditores assinam `/v1/confidential/assets/{id}/transitions` para aprender a altura de abertura da janela.
4. **Aplicar janela:** cuando a janela abre, el tiempo de ejecución cambia a política para `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }`, y come a rejeitar solicitudes de gobernanza conflitantes.
5. **Finalizar o cancelar:** en `effective_height`, o runtime verifica los prerrequisitos (suministro transparente cero, sin retiradas de emergencia, etc.). Sucesso muda a politica para o modo solicitado; falha emite `PolicyTransitionPrerequisiteFailed`, limpia a transicao pendente e deixa a politica inalterada.
6. **Actualizaciones de esquema:** apos uma transicao bem-sucedida, la gobernanza aumenta a versao de esquema de activo (por ejemplo, `asset_definition.v2`) y las herramientas CLI exigen `confidential_policy` para serializar manifiestos. Los operadores de documentos de actualización de genesis instruem para agregar configuraciones políticas y huellas digitales de registro antes de reiniciar validadores.Redes novas que iniciam com confidencialidade habilitada codificam a politica desejada directamente em génesis. Ainda assim seguem a checklist acima quando mudam modos pos-lanzamiento para que janelas de conversao sejam deterministas y wallets tenham tempo de ajustar.

### Versionamento y activación del manifiesto Norito- Los manifiestos de Génesis DEVEM incluyen un `SetParameter` para una clave personalizada `confidential_registry_root`. La carga útil y el JSON Norito que corresponde a `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: omitir el campo (`null`) cuando haya entradas activas, o fornecer una cadena hexadecimal de 32 bytes (`0x...`) igual al hash producido por `compute_vk_set_hash` sobre as instrucciones de verificador enviadas no manifest. Nodes recusam iniciar se o parámetro faltar ou se o hash divergir das escritas de registro codificadas.
- El cable `ConfidentialFeatureDigest::conf_rules_version` embute un versao de diseño manifiesto. Para redes v1 DEVE permanecer `Some(1)` e e igual a `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Cuando el conjunto de reglas evoluciona, aumenta a constante, regenera los manifiestos y ejecuta el despliegue de binarios en lock-step; misturar versos faz validadores rejeitarem blocos com `ConfidentialFeatureDigestMismatch`.
- Manifiestos de activación DEVEM agrupar actualizaciones de registro, mudancas de ciclo de vida de parámetros e transicoes de política para manter o digest consistente:
  1. Aplicar mutaciones de planos de registro (`Publish*`, `Set*Lifecycle`) en una vista sin conexión del estado y calcular o digerir posativacao con `compute_confidential_feature_digest`.
  2. Emitir `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x..."})` usando el hash calculado para que pares atrasados ​​recuperem o digest correto mesmo se pierdaem instrucciones intermediarias.3. Anexar instrucciones `ScheduleConfidentialPolicyTransition`. Cada instrucción debe citar o `transition_id` emitida por gobernanza; manifiesta que o esquecem seráo rejeitados pelo runtime.
  4. Persistir en los bytes del manifiesto, una huella digital SHA-256 y un resumen usado en el plano de activación. Operadores verificam os tres artefatos antes de votar o manifiesto para evitar particoes.
- Cuando los lanzamientos requieren un corte diferido, registre una altura alvo en un parámetro personalizado del compañero (por ejemplo, `custom.confidential_upgrade_activation_height`). Isso fornece aos auditores uma prova codificada em Norito de que validadores honraram a janela de aviso antes de digerir entrar em efeito.## Ciclo de vida de verificadores y parámetros.
### Registro ZK
- El libro mayor armazena `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }` onde `proving_system` actualmente y fijo en `Halo2`.
- Pares `(circuit_id, version)` sao globalmente unicos; El registro mantiene un índice secundario para buscar metadatos de circuito. Tentativas de registrador um par duplicado sao rejeitadas durante la admisión.
- `circuit_id` debe ser nao vazio e `public_inputs_schema_hash` debe ser fornecido (normalmente un hash Blake2b-32 para la codificación canónica de entradas públicas del verificador). Admisión rejeita registros que omitem esses campos.
- Las instrucciones de gobernanza incluyen:
  - `PUBLISH` para agregar una entrada `Proposed` solo con metadatos.
  - `ACTIVATE { vk_id, activation_height }` para programar activado en límite de época.
  - `DEPRECATE { vk_id, deprecation_height }` para marcar a altura final onde pruebas podem referenciar a entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para desligamento de emergencia; activos afetados congelam gastos confidenciais apos retirar altura ate novas entradas ativarem.
- Génesis manifiesta auto-emitem um parámetro personalizado `confidential_registry_root` cujo `vk_set_hash` coinciden con entradas activas; a validacao cruza esse digest com o estado local do registro antes de que un nodo pueda entrar sin consenso.- Registrar o actualizar una solicitud de verificador `gas_schedule_id`; a verificacao exige que una entrada al registro esteja `Active`, presente no indice `(circuit_id, version)`, y que las pruebas Halo2 fornecam um `OpenVerifyEnvelope` cujo `circuit_id`, `vk_hash`, e `public_inputs_schema_hash` corresponden al registro hacer registro.

### Claves de demostración
- Proving Keys ficam off-ledger mas sao referenciadas por identificadores content-addressed (`pk_cid`, `pk_hash`, `pk_len`) publicados al lado de los metadatos del verificador.
- Los SDK de billetera buscan datos de PK, verifican hashes y hacen caché local.

### Parámetros Pedersen y Poseidon
- Registros separados (`PedersenParams`, `PoseidonParams`) espelham controles de ciclo de vida de verificadores, cada uno con `params_id`, hashes de geradores/constantes, ativacao, deprecacao y retiro de alturas.
- Los compromisos y los hashes separan dominios por `params_id` para que una rotación de parámetros nunca reutilice padroes de bits de conjuntos obsoletos; o ID e embutido em compromisos de nota e etiquetas de dominio de nulificador.
- Circuitos compatibles con selección multiparámetro en tiempo de verificación; Los conjuntos de parámetros obsoletos permanecen gastables en `deprecation_height`, y los conjuntos retirados sao rejeitados exactamente en `withdraw_height`.## Ordenacao determinista e anuladores
- Cada activo mantiene un `CommitmentTree` con `next_leaf_index`; blocos acrescentam compromisos em ordem determinista: iterar transacoes na ordem do bloco; dentro de cada transacao iterar salidas blindadas por `output_idx` serializado ascendente.
- `note_position` e derivado dos offsets da arvore mas **nao** faz parte do nullifier; ele so alimenta caminos de membresía dentro del testigo da prueba.
- A estabilidade do nullifier sob reorgs e garantida pelo design PRF; o input PRF vincula `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, y anclajes referenciam raíces Merkle historicos limitados por `max_anchor_age_blocks`.## Flujo del libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - Solicitud política de activo `Convertible` o `ShieldedOnly`; admisión checa autoridade do activo, recupera `params_id` atual, amostra `rho`, emite compromiso, atualiza a arvore Merkle.
   - Emite `ConfidentialEvent::Shielded` con el nuevo compromiso, delta de Merkle root y hash de chamada da transacao para pistas de auditoría.
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - Syscall VM verifica la prueba usando una entrada del registro; o host garante anuladores nao usados, compromisos anexados determinísticamente y ancla reciente.
   - El libro mayor registra entradas `NullifierSet`, armazena payloads encriptados para destinatarios/auditores y emite `ConfidentialEvent::Transferred` resumindo anuladores, salidas ordenadas, hash de prueba y raíces Merkle.
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - Disponivel apenas para activos `Convertible`; aproof valida que o valor da note iguala o montante revelado, o ledger credita balance transparente e queima a note blinded marcando o nullifier como gasto.
   - Emite `ConfidentialEvent::Unshielded` com o montante publico, nullifiers consumidos, identificadores de prueba e hash de chamada da transacao.## Adicoes ao modelo de datos
- `ConfidentialConfig` (nova secao de config) con bandera de habilitación, `assume_valid`, perillas de gas/limites, janela de anclaje, backend de verifier.
- Los esquemas `ConfidentialNote`, `ConfidentialTransfer`, e `ConfidentialMint` Norito con byte de versao explícito (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` incluye bytes de nota AEAD con `{ version, ephemeral_pubkey, nonce, ciphertext }`, con `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` predeterminado para el diseño XChaCha20-Poly1305.
- Vectores canónicos de derivación de claves viven en `docs/source/confidential_key_vectors.json`; Tanto la CLI como el punto final Torii regresan contra esos accesorios.
- `asset::AssetDefinition` Ganha `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` persiste o vinculante `(backend, name, commitment)` para verificadores de transferencia/unshield; a execucao rejeita pruebas cujo verificación clave referenciado ou en línea nao corresponde ao compromiso registrado.
- `CommitmentTree` (por activos con puntos de control fronterizos), `NullifierSet` con chave `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` armados en el estado mundial.
- Mempool mantem estructuras transitorias `NullifierIndex` e `AnchorIndex` para detectar precoce de duplicados y comprobaciones de estado de anclaje.
- Las actualizaciones del esquema Norito incluyen el pedido canónico para entradas públicas; pruebas de determinismo de codificación de garantía de ida y vuelta.- Roundtrips de ficam de carga útil cifrada fijados mediante pruebas unitarias (`crates/iroha_data_model/src/confidential.rs`). Vectores de wallet de acompanhamento vao anexar transcripts AEAD canonicos para auditores. `norito.md` documenta el encabezado en el cable para el sobre.

## Integracao IVM y llamada al sistema
- Introducir syscall `VERIFY_CONFIDENTIAL_PROOF` aceitando:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof`, y o `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }` resultantes.
  - O syscall carga metadatos del verificador del registro, aplicaciones límites de tamaño/tempo, cobra gas determinista, y así la aplicación o delta es una prueba de éxito.
- El host expone el rasgo de solo lectura `ConfidentialLedger` para recuperar instantáneas de Merkle root y estado de anulador; La biblioteca Kotodama requiere ayudantes de ensamblaje de testigos y validación de esquema.
- Docs de pointer-ABI foram actualizados para aclarar el diseño del buffer de prueba y los identificadores de registro.## Negociacao de capacidades de nodo
- O handshake anuncia `feature_bits.confidential` junto com `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. Participacao de validadores requer `confidential.enabled=true`, `assume_valid=false`, identificadores de backend do verifier identicos e digests que corresponden; no coincide con falham o handshake com `HandshakeConfidentialMismatch`.
- Config soporte `assume_valid` sólo para observadores: cuando desabilitado, encontrar instrucciones confidenciales gera `UnsupportedInstruction` determinista sin pánico; Cuando habilitado, los observadores aplican deltas declarados sin verificar pruebas.
- Mempool rejeita transacoes confidenciais se a capacidade local estiver desabilitada. Filtros de chismes evitan enviar transacciones blindadas para pares incompatibles en cuanto encaminham cegamente ID de verificador desconhecidos dentro de los límites de tamaño.

### Matriz de apretón de manos| Anuncio remoto | Resultado para validadores de nodos | Notas del operador |
|---------------------|--------------------------------|----------------|
| `enabled=true`, `assume_valid=false`, coincidencia de backend, coincidencia de resumen | Aceito | O peer chega ao estado `Ready` e participa de propuesta, voto y fan-out de RBC. Nenhuma acao manual requerido. |
| `enabled=true`, `assume_valid=false`, coincidencia de backend, resumen obsoleto o ausente | Rejeitado (`HandshakeConfidentialMismatch`) | El control remoto debe aplicar activacos pendientes de registro/parámetros o guardar el `activation_height` programado. Ate corrigir, o node segue descobrivel mas nunca entra na rotacao de consenso. |
| `enabled=true`, `assume_valid=true` | Rejeitado (`HandshakeConfidentialMismatch`) | Validadores requieren verificación de pruebas; Configure el control remoto como Observer con Torii-only Ingress o Mude `assume_valid=false` para habilitar la verificación completa. |
| `enabled=false`, campos omitidos (compilación desatualizada), o backend de verificador diferente | Rejeitado (`HandshakeConfidentialMismatch`) | Los pares desatualizados o parcialmente atualizados no pueden entrar en la red de consenso. Atualize para o release atual y garantiza que o tuple backend + digest corresponde antes de reconectar. |Los observadores que intencionalmente pulam verificacao de pruebas nao deben abrir conexoes de consenso contra validadores con puertas de capacidad. Eles ainda podem ingerir blocos via Torii ou API de archivo, pero una red de consenso os rejeita ate anunciarem capacidades comparativas.

### Política de poda de revelación y retención de anulador

Ledgers confidenciais devem reter historico suficiente para provar frescor de notas e reproducir auditorias de gobierno. Una política predeterminada, aplicada por `ConfidentialLedger`, e:- **Retencao de nullifiers:** manter nullifiers gastos por un *mínimo* de `730` días (24 meses) apos a altura de gasto, ou a janela regulatoria obrigatoria se for maior. Operadores podem estender a janela vía `confidential.retention.nullifier_days`. Nullifiers mais novos que a janela DEVEM permanecerán consultaveis vía Torii para que auditores comprueben ausencia de doble gasto.
- **Poda de velos:** velos transparentes (`RevealConfidential`) podam engagements associados imediatamente apos o bloco finalizar, mas o nullifier consumido continua sujeito a regra de retencao acima. Eventos `ConfidentialEvent::Unshielded` registram o montante publico, recipiente y hash de prueba para que reconstruir revele historicos nao exija o ciphertext podado.
- **Puntos de control fronterizos:** fronteras de compromiso mantem checkpoints rodando cobrindo o maior entre `max_anchor_age_blocks` e a janela de retencao. Nodes compactam checkpoints antigos apenas depois que todos os nullifiers no intervalo expiram.
- **Remediacao de digest stale:** se `HandshakeConfidentialMismatch` ocorrer por drift de digest, los operadores deben (1) verificar que as janelas de retencao de nullifiers estao alinhadas no cluster, (2) rodar `iroha_cli app confidential verify-ledger` para regenerar o digest contra o conjunto de nullifiers retidos, e (3) redeployar o manifest atualizado. Los anuladores podados prematuramente deben ser restaurados en el almacenamiento en frío antes de reingresar a la red.Documente anula locais no runbook de operacoes; Las políticas de gobernanza que estendem a janela de retencao deben actualizar la configuración de nodos y planos de almacenamiento de archivo en lockstep.

### Flujo de desalojo y recuperación

1. Durante el dial, `IrohaNetwork` compara las capacidades anunciadas. Qualquer desajuste levanta `HandshakeConfidentialMismatch`; a conexao e fechada e o peer permanece na fila de descubrimiento sem ser promovido a `Ready`.
2. A falha aparece no log do servico de rede (inclui digest remoto and backend), e Sumeragi nunca agenda o peer para proposta ou voto.
3. Los operadores reparan registros de verificador y conjuntos de parámetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) o programan `next_conf_features` con un `activation_height` acordado. Una vez que el resumen coincide, el próximo apretón de manos se realiza automáticamente.
4. Se um peer stale consegue difundir um bloco (por ejemplo, vía repetición de archivo), validadores o rejeitam deterministicamente con `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, manteniendo o estado do ledger consistente na rede.

### Flujo de apretón de manos seguro contra repetición1. Cada tentativa saliente aloca material de chave Noise/X25519 novo. La carga útil de handshake assinado (`handshake_signature_payload`) concatena como chaves públicas efímeras locales y remotas, o endereco de socket anunciado codificado en Norito y, cuando compilado con `handshake_chain_id`, o identificador de cadena. Un mensaje y encriptado con AEAD antes de salir del nodo.
2. El respondedor recalcula la carga útil con una orden de chaves peer/local invertida y verifica la assinatura Ed25519 embutida en `HandshakeHelloV1`. Como ambas as chaves efemeras y o endereco anunciado fazem parte do dominio de assinatura, replay de uma mensagem capturadoda contra outro peer ou recuperar uma conexao stale falha deterministicamente.
3. Flags de capacidade confidencial e o `ConfidentialFeatureDigest` viajam dentro de `HandshakeConfidentialMeta`. O receptor compara la tupla `{ enabled, assume_valid, verifier_backend, digest }` con su `ConfidentialHandshakeCaps` local; qualquer mismatch sai cedo com `HandshakeConfidentialMismatch` antes de o transportar transitar para `Ready`.
4. Operadores DEVEM recomputar o digest (vía `compute_confidential_feature_digest`) y reiniciar nodes com registries/politicas atualizadas antes de reconectar. Peers anunciando digests antigos continuam falhando o handshake, evitando que estado stale reentre no set validador.5. Los éxitos y errores del protocolo de enlace actualizan los contadores padrao `iroha_p2p::peer` (`handshake_failure_count`, ayudantes de taxonomía de errores) y emiten registros estructurados con el ID de par remoto y el resumen de huellas dactilares. Monitoree estos indicadores para detectar repeticiones o configuraciones incorrectas durante el lanzamiento.

## Gestión de claves y cargas útiles
- Jerarquía de derivación por cuenta:
  - `sk_spend` -> `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Cargas útiles de notas encriptadas usando AEAD con claves compartidas derivadas por ECDH; ver claves de auditor opcionales podem ser anexadas a salidas conforme a política del activo.
- Adicoes ao CLI: `confidential create-keys`, `confidential send`, `confidential export-view-key`, herramientas de auditor para descriptografar notas, y el ayudante `iroha app zk envelope` para producir/inspeccionar sobres Norito fuera de línea. Torii expone o mesmo fluxo de derivacao via `POST /v1/confidential/derive-keyset`, retornando formas hexadecimal y base64 para que wallets busquen jerarquías de chave programáticamente.## Gas, límites y controles DoS
- Horario determinista de gas:
  - Halo2 (Plonkish): gas base `250_000` + gas `2_000` por entrada del público.
  - `5` gas por byte de prueba, más cargas por anulador (`300`) y por compromiso (`500`).
  - Operadores podem sobrescrever essas constantes via configuracao do node (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); Las mudanzas se propagan sin inicio ni recarga en caliente de la camada de configuración y se aplican de manera determinista sin clúster.
- Límites de duros (configurados por defecto):
- `max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Pruebas que excedem `verify_timeout_ms` abortam a instrucao deterministicamente (ballots de Governance emitem `proof verification exceeded timeout`, `VerifyProof` retorna erro).
- Cuotas adicionales que garantizan la vida útil: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block`, e `max_public_inputs` limitan a los constructores de bloques; `reorg_depth_bound` (>= `max_anchor_age_blocks`) gobierna la retención de puntos de control fronterizos.
- Un tiempo de ejecución de ejecución ahora rejeita transacoes que excedem esses limites por transacao ou por bloco, emitiendo errores `InvalidParameter` deterministas y manteniendo el estado del libro mayor inalterado.
- Mempool prefiltra transacoes confidenciais por `vk_id`, tamaño de prueba e idade de anclaje antes de invocar o verificador para mantener uso de recursos limitados.- Una verificación para determinar determinísticamente el tiempo de espera o la violación del límite; transacoes falham con errores explícitos. Los backends SIMD son opcionales pero no modifican la contabilidad de gas.

### Líneas base de calibracao y puertas de aceitacao
- **Plataformas de referencia.** Rodadas de calibracao DEVEM cobrir os tres perfis abaixo. Rodadas sem todos os perfis sao rejeitadas na review.

  | Perfil | Arquitectura | CPU/instancia | Banderas de compilador | propuesta |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) o Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | Estabelece valores piso sem intrinsics vetoriais; usado para ajustar tablas de custo fallback. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | liberar por defecto | Valida la ruta AVX2; Checa se os ganhos SIMD ficam dentro de la tolerancia del gas neutral. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | liberar por defecto | Garantía de que el backend NEON permanece determinista y actualizado en horarios x86. |

- **Arnés de referencia.** Todos los relatorios de calibracao de gas DEVEM ser produzidos com:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para confirmar o determinista del accesorio.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` siempre que el custodio de código de operación de VM se muda.- **Randomness fixa.** Exporte `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes de rodar bancos para que `iroha_test_samples::gen_account_in` mude para el camino determinista `KeyPair::from_seed`. O arnés imprime `IROHA_CONF_GAS_SEED_ACTIVE=...` una vez; se a variavel faltar, una revisión DEVE falhar. Qualquer utilidade nova de calibracao debe continuar honrando este env var ao introduzir randomness auxiliar.

- **Captura de resultados.**
  - Subir resúmenes Criterion (`target/criterion/**/raw.csv`) para cada perfil no artefato de liberación.
  - Armazenar métricas derivadas (`ns/op`, `gas/op`, `ns/gas`) no [Confidential Gas Calibration ledger](./confidential-gas-calibration) junto con el commit git y el versao de compilador usado.
  - Manter os dos ultimas líneas base por perfil; apagar instantáneas mais antigos uma vez validado o relatorio mais novo.

- **Tolerancias de aceitacao.**
  - Los deltas de gas entre `baseline-simd-neutral` e `baseline-avx2` DEVEM permanecen <= +/-1.5%.
  - Los deltas de gas entre `baseline-simd-neutral` e `baseline-neon` DEVEM permanecen <= +/-2.0%.
  - Las propuestas de calibración que exceden estos umbrales requieren ajustes de programación o un RFC explicando una discrepancia y una mitigación.- **Lista de verificación de revisión.** Los remitentes son respondidos por:
  - Incluye `uname -a`, trechos de `/proc/cpuinfo` (modelo, paso a paso), e `rustc -Vv` sin registro de calibración.
  - Verificar que `IROHA_CONF_GAS_SEED` aparece na saya do bench (as benches imprimem a seed ativa).
  - Garantir que feature flags do marcapasos y do verifier confidencial espelhem producao (`--features confidential,telemetry` ao rodar benches com Telemetry).

## Configuración y operaciones
- `iroha_config` adición a secado `[confidential]`:
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
- Telemetria emite métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}`, e `confidential_policy_transitions_total`, sin datos em exportados Claro.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Estrategia de testículos
- Determinismo: aleatorización de transacciones dentro de bloques gera Merkle raíces y conjuntos de anuladores idénticos.
- Resiliencia a reorg: reorganizaciones similares con anclajes de comunicaciones multibloco; anuladores permanecenm estaveis e anclas rancias sao rejeitados.
- Invariantes de gas: verificar el uso de gas idéntico entre nodos con y sin aceleración SIMD.
- Pruebas de límites: pruebas sin teto de tamanho/gas, conteos máximos de entrada/salida, cumplimiento de tiempo de espera.
- Ciclo de vida: operaciones de gobernanza para ativacao/deprecacao de verifier e parametros, pruebas de gasto apos rotacao.
- Política FSM: transicoes permitidas/negadas, delays de transicao pendente e rejeicao de mempool perto de alturas efetivas.
- Emergencias de registro: retiro de emergencia congela activos afetados em `withdraw_height` e rejeita pruebas depois.
- Control de capacidad: validadores con `conf_features` divergentes rejeitam blocos; observers com `assume_valid=true` acompanham sem afetar consenso.
- Equivalencia de estado: nodes validator/full/observer produzem root de estado identicos na cadeia canonica.
- Fuzzing negativo: pruebas malformadas, cargas útiles superdimensionadas y colisos de anulador sao rejeitados determinísticamente.## Migracao
- Indicador de función de implementación de com: finalizó la Fase C3, `enabled` por defecto es `false`; nodes anunciam capacidades antes de entrar no set validador.
- Bienes transparentes nao sao afetados; instrucciones confidenciales requieren entradas de registro y negociación de capacidades.
- Nodos compilados sin soporte confidencial rejeitam blocos relevantes determinísticamente; nao podem entrar no set validador mas podem operar como observers com `assume_valid=true`.
- Los manifiestos de Génesis incluyen entradas iniciais de registro, conjuntos de parámetros, políticas confidenciales para activos y claves de auditor opcionales.
- Operadores seguem runbooks publicados para rotación de registro, transicos de política y retiro de emergencia para manter actualizaciones deterministas.## Trabajo pendiente
- Benchmark de parámetros Halo2 (tamanho de circuito, estrategia de búsqueda) y registrar resultados en el libro de jugadas de calibración para que los valores predeterminados de gas/timeout se actualicen junto con la próxima actualización de `confidential_assets_calibration.md`.
- Finalizar políticas de divulgación de auditor y API de visualización selectiva asociadas, conectando el flujo de trabajo aprobado en Torii así como el borrador de gobernanza para assinado.
- Ender el esquema de cifrado de testigos para cobrir salidas de múltiples destinatarios y notas en lotes, documentando el formato del sobre para implementadores de SDK.
- Comissionar uma revisao de seguranca externa de circuitos, registros e procedimentos de rotacao de parametros e archivar os achados ao lado dos relatorios internos de auditoria.
- Especificar API de conciliación de gastos para auditores y guía pública de escopo de view-key para que los proveedores de billetera implementen como mesmas semánticas de atestacao.## Fases de implementación
1. **Fase M0 - Endurecimiento de parada de envío**
   - [x] Derivacao de nullifier segue o design Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) con orden determinista de compromisos aplicados en las actualizaciones del libro mayor.
   - [x] Execucao aplica limites de tamanho deproof e cuotas confidenciais por transacao/por bloco, rejeitando transacoes fora de presupuesto con errores deterministas.
   - [x] Handshake P2P anuncia `ConfidentialFeatureDigest` (resumen de backend + huellas dactilares de registro) y faltan discrepancias determinísticamente a través de `HandshakeConfidentialMismatch`.
   - [x] Elimina pánicos en rutas de ejecución confidencial y agrega activación de funciones para nodos incompatibles.
   - [] Aplicar presupuestos de tiempo de espera del verificador y límites de profundidad de reorganización para los puntos de control fronterizos.
     - [x] Presupuestos de tiempo de espera de verificación aplicados; pruebas que excedem `verify_timeout_ms` agora falham deterministicamente.
     - [x] Frontier checkpoints ahora respectivo `reorg_depth_bound`, podando checkpoints mais antigos que a janela configurada e mantendo snapshots deterministas.
   - Introducimos `AssetConfidentialPolicy`, política FSM y puertas de cumplimiento para instrucciones mint/transfer/reveal.
   - Confirme `conf_features` en los encabezados de bloque y recusar participar de validadores cuando compendios de registro/parámetros divergem.
2. **Fase M1 - Registros y parámetros**- Entregar los registros `ZkVerifierEntry`, `PedersenParams`, e `PoseidonParams` con operaciones de gobernanza, anclaje de génesis y gestión de caché.
   - Conectar syscall para solicitar búsquedas de registro, ID de programación de gas, hash de esquema y comprobaciones de tamaño.
   - Enviar formato de carga útil encriptado v1, vectores de derivación de claves para billetera, y soporte CLI para gestao de chaves confidenciais.
3. **Fase M2 - Rendimiento de gas e**
   - Implementar cronograma de determinista de gas, contadores por bloco y arneses de benchmark con telemetría (latencia de verificación, tamanhos de prueba, chequeos de mempool).
   - Endurecer puntos de control de CommitmentTree, carga LRU e índices de anulación para cargas de trabajo multiactivo.
4. **Fase M3 - Rotación y herramientas de billetera**
   - Habilitar aceitacao de pruebas multi-parametro y multi-versao; suportar ativacao/deprecacao guiado por gobernanza com runbooks de transicao.
   - Entregar flujos de migración SDK/CLI, flujos de trabajo de escaneo de auditor y herramientas de reconciliación de gastos.
5. **Fase M4 - Auditoría y operaciones**
   - Fornecer flujos de trabajo de claves de auditor, API de divulgación selectiva y runbooks operativos.
   - Agendar revisao externa de criptografia/seguranca e publicar achados em `status.md`.

Cada fase actualiza los hitos de la hoja de ruta y las pruebas asociadas para mantener las garantías de ejecución determinista de la red blockchain.