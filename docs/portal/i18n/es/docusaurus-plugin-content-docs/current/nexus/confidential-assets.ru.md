---
lang: es
direction: ltr
source: docs/portal/docs/nexus/confidential-assets.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Конфиденциальные активы и ZK-переводы
descripción: Блюпринт Phase C для blindados циркуляции, реестров и операторских контролей.
slug: /nexus/activos-confidenciales
---
<!--
SPDX-License-Identifier: Apache-2.0
-->
# Дизайн конфиденциальных активов и ZK-переводов

## Motivación
- Дать opt-in blinded потоки активов, чтобы домены могли сохранять транзакционную приватность, не изменяя прозрачную циркуляцию.
- Configuración de la configuración de parámetros de validación y compatibilidad con Norito/Kotodama ABI v1.
- Predisposición del control por parte del auditor y del operador del ciclo de funcionamiento (actividad, rotación y activación) de circuitos y criptomonedas parámetros.

## Модель угроз
- Валидаторы честные-но-любопытные (honesto-pero-curioso): исполняют консенсус корректно, но пытаются изучать ledger/state.
- Наблюдатели сети видят данные блоков и chismes транзакции; приватные gossip-kanalы не предполагаются.
- Вне области: análisis del tráfico fuera del libro mayor, квантовые противники (отдельно в PQ roadmap), атаки доступности libro mayor.## Обзор дизайна
- Активы могут объявлять *shielded pool* помимо существующих прозрачных балансов; blindado циркуляция представляется криптографическими compromisos.
- Notas incluidas `(asset_id, amount, recipient_view_key, blinding, rho)` с:
  - Compromiso: `Comm = Pedersen(params_id || asset_id || amount || recipient_view_key || blinding)`.
  - Anulador: `Null = Poseidon(domain_sep || nk || rho || asset_id || chain_id)`, независим от порядка notes.
  - Carga útil cifrada: `enc_payload = AEAD_XChaCha20Poly1305(ephemeral_shared_key, note_plaintext)`.
- Transacciones no disponibles Norito - cargas útiles codificadas `ConfidentialTransfer`, compatibles:
  - Aportes públicos: ancla de Merkle, anuladores, nuevos compromisos, identificación de activos, circuito de versiones.
  - Cargas útiles adicionales para usuarios y auditores opcionales.
  - Prueba de conocimiento cero, подтверждающую сохранение стоимости, propiedad y autorización.
- Verificación de claves y parámetros de control del registro en el libro mayor con actividades activas; узлы отказываются валидировать pruebas, которые ссылаются на неизвестные или отозванные записи.
- Заголовки консенсуса коммитятся к активному digest конфиденциальных возможностей, поэтому блоки принимаются только при совпадении состояния registro y parámetros.
- Las pruebas de publicación utilizan la configuración confiable de Halo2 (Plonkish); Groth16 y otras variantes de SNARK no se pueden utilizar en v1.

### Детерминированные фикстурыLas conversiones de memorandos confidenciales deben colocarse en dispositivos canónicos en `fixtures/confidential/encrypted_payload_v1.json`. Набор данных включает положительный v1-конверт y негативные поврежденные образцы, чтобы SDK могли проверять паритет parsinga. Las pruebas del modelo de datos de Rust (`crates/iroha_data_model/tests/confidential_encrypted_payload_vectors.rs`) y la suite Swift (`IrohaSwift/Tests/IrohaSwiftTests/ConfidentialEncryptedPayloadTests.swift`) configuran el archivo de configuración, la garantía, según la codificación Norito, поверхности ошибок y регрессионное покрытие остаются согласованными по мере эволюции кодека.

Los SDK de Swift se pueden usar con pegamento JSON personalizado: `ShieldRequest` con notas de compromiso de 32 bits, carga útil y metadatos de débito de 32 bits вызовите `IrohaSDK.submit(shield:keypair:)` (или `submitAndWait`), чтобы подписать and отправить транзакцию через `/v1/pipeline/transactions`. Ayudante para validar compromisos, programa `ConfidentialEncryptedPayload` y codificador Norito y diseño de teclado `zk::Shield`, descripción detallada кошельки оставались синхронизированы с Rust.## Коммитменты консенсуса и Gating возможностей
- Заголовки блоков раскрывают `conf_features = { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`; digest участвует в хэше консенсуса и должен совпадать с локальным представлением registro для принятия блока.
- Gobernanza puede permitir el uso de programas `next_conf_features` con bloques `activation_height`; до этой высоты продюсеры блоков обязаны продолжать публиковать прежний digest.
- Los validadores de datos trabajan con `confidential.enabled = true` e `assume_valid = false`. Los programas de inicio no se utilizan en validadores locales, sino que se utilizan de forma local o local `conf_features` расходятся.
- El protocolo de enlace P2P metadano se activa `{ enabled, assume_valid, conf_features }`. Peers, рекламирующие несовместимые возможности, отклоняются с `HandshakeConfidentialMismatch` and никогда не входят в ротацию консенсуса.
- Recursos para validadores, observadores y pares usuarios en el protocolo de enlace de matriz [Capacidad de nodo] Negociación](#node-capability-negotiation). El apretón de manos de otros usuarios muestra como `HandshakeConfidentialMismatch` y otros pares en el acuerdo de rotación, pero el resumen no se admite.
- Observadores, не являющиеся валидаторами, могут выставить `assume_valid = true`; они слепо применяют конфиденциальные дельты, но не влияют на безопасность консенсуса.## Políticas activas
- La operación de activación necesaria no es `AssetConfidentialPolicy`, que se actualiza según su gobernanza:
  - `TransparentOnly`: режим по умолчанию; разрешены только прозрачные инструкции (`MintAsset`, `TransferAsset` и т.д.), а операции отклоняются.
  - `ShieldedOnly`: вся эмиссия и переводы должны использовать конфиденциальные инструкции; `RevealConfidential` запрещен, поэтому балансы никогда не становятся публичными.
  - `Convertible`: Los vehículos eléctricos pueden permanecer estables y protegidos, implementar instrucciones de rampa de entrada/salida ниже.
- Las políticas del FSM no deben bloquearse:
  - `TransparentOnly → Convertible` (piscina protegida sin protección).
  - `TransparentOnly → ShieldedOnly` (un período de instalación y otras conversaciones).
  - `Convertible → ShieldedOnly` (обязательная чимальная задержка).
  - `ShieldedOnly → Convertible` (нужен план миграции, чтобы notas blindadas оставались тратимыми).
  - `ShieldedOnly → TransparentOnly` запрещен, если blinded pool не пуст или gobernance не кодирует миграцию, которая раскрывает оставшиеся notas.
- Las instrucciones de gobierno incluyen `pending_transition { new_mode, effective_height, previous_mode, transition_id, conversion_window }` y ISI `ScheduleConfidentialPolicyTransition` y pueden modificarse según `CancelConfidentialPolicyTransition`. La garantía de validez de mempool, que no se realiza durante el proceso de transferencia, no se realiza durante el período, ni se realiza ninguna transferencia de datos determinada. проверка политики изменилась бы посреди блока.- Запланированные переходы применяются автоматически ри открытии нового блока: когда высота входит в окно конверсии (для апгрейдов `ShieldedOnly`) o descargado `effective_height`, tiempo de ejecución activado `AssetConfidentialPolicy`, actualizado metadatos `zk.policy` y eliminado entrada pendiente. Si en el período de ejecución `ShieldedOnly` se establece una configuración de configuración y inicio de sesión, el tiempo de ejecución предупреждение, оставляя прежний режим.
- Las perillas de configuración `policy_transition_delay_blocks` y `policy_transition_window_blocks` reducen los períodos de gracia y los periodos de gracia, y permiten que las cosas se ajusten a sus necesidades. конвертировать notas вокруг переключения.
- `pending_transition.transition_id` также служит manija de auditoría; La gobernanza se utiliza para finalizar las operaciones o para los períodos en los que los operadores pueden correlacionar las rampas de entrada y salida.
- `policy_transition_window_blocks` con un volumen de 720 (un total de 12 horas antes de un bloque de 60 segundos). Узлы ограничивают запросы gobernancia, пытающиеся задать более короткое уведомление.
- Génesis se manifiesta y CLI потоки показывают текущие ожидающие политики. La admisión de lógica es una política de la aplicación, la política de acceso, la instrucción confidencial.
- Lista de verificación de migraciones — см. “Secuenciación de migración” no está disponible para el plan de aprendizaje, ya que está excluido Milestone M0.

#### Monitoreo de períodos según ToriiLas llaves y los auditores utilizan `GET /v1/confidential/assets/{definition_id}/transitions` y activan `AssetConfidentialPolicy`. Carga útil JSON que incluye ID de activo canónico, configuración de bloque de bloque, política `current_mode`, corrección y efecto на этой высоте (окна конверсии временно отдают `Convertible`), and ожидаемые идентификаторы параметров `vk_set_hash`/Poseidon/Pedersen. Durante el período de gobernanza, se recomienda:

- `transition_id` — identificador de auditoría, nombre de usuario `ScheduleConfidentialPolicyTransition`.
- `previous_mode`/`new_mode`.
-`effective_height`.
- `conversion_window` y `window_open_height` (bloque, donde las personas que deseen realizar una conversión para corte ShieldedOnly).

Primer comentario:

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

Otvett `404` означает, что соответствующее определение актива не найдено. Когда переход не запланирован, поле `pending_transition` равно `null`.

### Машина состояний политики| Текущий режим | Следующий режим | Предпосылки | Altura efectiva de altura | Примечания |
|--------------------|------------------|-------------------------------------------------------------------------------|-------------------------------------------------------------------------------------------------------------------|-------------------------------------------------------------------------------------|
| Sólo transparente | Descapotable | Gobernanza активировал записи verificador/parámetro de registro. Utilice `ScheduleConfidentialPolicyTransition` con `effective_height ≥ current_height + policy_transition_delay_blocks`. | Переход выполняется ровно на `effective_height`; piscina protegida становится доступен сразу.                   | Póngase en contacto con personas que tengan información confidencial sobre los alimentos.               |
| Sólo transparente | Sólo blindado | Para esto, además, `policy_transition_window_blocks ≥ 1`.                                                         | Runtime автоматически входит в `Convertible` на `effective_height - policy_transition_window_blocks`; переключается на `ShieldedOnly` на `effective_height`. | Estas son algunas de las conversaciones que se realizan según las instrucciones de uso.   || Descapotable | Sólo blindado | Запланированный переход с `effective_height ≥ current_height + policy_transition_delay_blocks`. La gobernanza DEBE incluir (`transparent_supply == 0`) en los metadatos de auditoría; runtime proporciona esto en corte. | Та же семантика окна, что выше. Si no se utiliza ninguna tecla en `effective_height`, primero se debe prever en `PolicyTransitionPrerequisiteFailed`. | Закрепляет актив в полностью конфиденсиальной циркуляции.                                     |
| Sólo blindado | Descapotable | Запланированный переход; нет активного retiro de emergencia (`withdraw_height` не задан).                                    | Состояние переключается на `effective_height`; rampas de revelación снова открываются, пока notas blindadas остаются валидными.                           | Utilice un dispositivo de control o un auditor para comprobarlo.                                          |
| Sólo blindado | Sólo transparente | Gobernanza para el `shielded_supply == 0` o para el plan de auditoría `EmergencyUnshield` (nuestros auditores). | El tiempo de ejecución está activado `Convertible` antes de `effective_height`; на этой высоте конфиденциальные инструкции жестко проваливаются, и актив возвращается в режим только прозрачных operación. | Выход последней инстанции. Переход автоматически отменяется, если какая-либо конфиденциальная note расходуется в окне. || Cualquiera | Igual que la actual | `CancelConfidentialPolicyTransition` очищает ожидающее изменение.                                                        | `pending_transition` удаляется немедленно.                                                                          | Сохраняет статус-кво; показано для полноты.                                             |

Переходы, не указанные выше, отклоняются при подаче в gobernancia. Runtime проверяет предпосылки прямо перед применением запланированного перехода; несоответствие возвращает актив в предыдущий режим and отправляет `PolicyTransitionPrerequisiteFailed` в телеметрию и события блока.

### Последовательность миграции1. **Подготовить реестры:** Activar все записи verificador y parámetros, которые требуются целевой политике. Si utiliza la información `conf_features`, estos pares pueden demostrar su conformidad.
2. **Período de configuración:** Utilice `ScheduleConfidentialPolicyTransition` con `effective_height`, junto con `policy_transition_delay_blocks`. Para los televisores `ShieldedOnly`, utilice una conversación (`window ≥ policy_transition_window_blocks`).
3. **Úlpidas instrucciones para los operadores:** Limpie el modelo `transition_id` y reproduzca la rampa de entrada y salida del runbook. Las llaves y los auditores colocados en `/v1/confidential/assets/{id}/transitions`, deben usar su propia autorización.
4. **Advertencia:** Cuando se activa, el tiempo de ejecución se ajusta a la política en `Convertible`, emite `PolicyTransitionWindowOpened { transition_id }` y se activa отклонять конфликтующие gobernanza-запросы.
5. **Finalización y configuración:** En el tiempo de ejecución `effective_height` se muestran los tiempos de ejecución predeterminados (no se requiere ningún tiempo de ejecución, отсутствие retiros de emergencia и т.п.). Успех переключает политику в запрошенный режим; ошибка эмитирует `PolicyTransitionPrerequisiteFailed`, очищает pendiente de transición y оставляет политику без изменений.
6. **Programas de actualización:** Una versión posterior de la gobernanza del período de uso puede habilitar varias versiones de programas activos (por ejemplo, `asset_definition.v2`), un programa de herramientas CLI `confidential_policy` se manifiesta en serie. Los documentos relacionados con la instrucción de génesis de los operadores deben incluir registros políticos y de registro antes de los validadores.Новые сети, начинающие с включенной конфиденциальностью, кодируют желаемую политику непосредственно в genesis. Они все равно следуют чек-listу выше при зменении режимов после запуска, чтобы окна конверсии оставались детерминированными, а кошельки успевали адаптироваться.

### Versión y activación de manifiestos Norito- Génesis manifiesta ДОЛЖНЫ включать `SetParameter` для кастомного ключа `confidential_registry_root`. Carga útil: Norito JSON, соответствующий `ConfidentialRegistryMeta { vk_set_hash: Option<String> }`: опускайте поле (`null`), когда активных verifier entradas netas, иначе передайте 32-байтную hex-строку (`0x…`), равную хэшу, вычисленному `compute_vk_set_hash` по verifier инструкциям в manifest. Cuando se inicia el proceso, estos parámetros no se eliminan ni se actualizan con el registro inicial.
- Manifiesto de diseño en línea `ConfidentialFeatureDigest::conf_rules_version` встраивает версию. La versión 1 del juego está instalada en `Some(1)` y en cuervo `iroha_config::parameters::defaults::confidential::RULES_VERSION`. Este conjunto de reglas evoluciona, mantiene una constante, mantiene manifiestos y sincroniza los binarios; смешение версий заставляет валидаторов отклонять блоки с `ConfidentialFeatureDigestMismatch`.
- Manifiestos de activación ДОЛЖНЫ связывать обновления registro, изменения жизненного цикла параметров и переходы политик, чтобы digest оставался constante:
  1. Primero, registre la configuración del plan (`Publish*`, `Set*Lifecycle`) en la lista completa y en el resumen de actividades por `compute_confidential_feature_digest`.
  2. Emitir `SetParameter::custom(confidential_registry_root, {"vk_set_hash": "0x…"})`, используя вычисленный хэш, чтобы отстающие peers могли восстановить корректный digest, даже если они пропустили промежуточные instrucciones de registro.3. Utilice las instrucciones `ScheduleConfidentialPolicyTransition`. Каждая инструкция должна цитировать выданный Governance `transition_id`; manifiestos, которые его забывают, будут отклонены runtime.
  4. Сохраните байты manifest, SHA-256 отпечаток и digest, использованный в plan de activación. Los operadores comprobarán tres artefactos antes de realizar la operación.
- Cuando se implementa un sistema de corte de transición, se introduce la televisión en el parámetro de configuración estándar (por ejemplo, `custom.confidential_upgrade_activation_height`). Este auditor Norito-кодированное доказательство того, что валидаторы соблюли окно уведомления до вступления resumen de изменения.## Verificador y parámetros de ciclos completos
### Registro ZK
- El libro mayor `ZkVerifierEntry { vk_id, circuit_id, version, proving_system, curve, public_inputs_schema_hash, vk_hash, vk_len, max_proof_bytes, gas_schedule_id, activation_height, deprecation_height, withdraw_height, status, metadata_uri_cid, vk_bytes_cid }`, el `proving_system` se basa en el procesador `Halo2`.
- Пары `(circuit_id, version)` globalmente único; registro поддерживает вторичный индекс для поиска по METаданным circuito. Las ventanas emergentes están cerradas antes de la admisión.
- `circuit_id` не может быть пустым, а `public_inputs_schema_hash` обязателен (обычно Blake2b-32 хэш канонического публичного ввода verifier). La entrada se realiza según el horario establecido.
- Инструкции gobernanza включают:
  - `PUBLISH` para la entrada `Proposed` que contiene metadatos.
  - `ACTIVATE { vk_id, activation_height }` para la entrada de actividades de planificación en grandes épocas.
  - `DEPRECATE { vk_id, deprecation_height }` Para obtener más información sobre las pruebas, es posible que haya pruebas en la entrada.
  - `WITHDRAW { vk_id, withdraw_height }` para la configuración externa; затронутые активы замораживают конфиденциальные траты после retirar altura, пока не активируются новые entradas.
- Génesis manifiesta los parámetros automáticos emitidos por el parámetro `confidential_registry_root` con `vk_set_hash`, compatibles con entradas activas; валидация кросс-проверяет этот digest с локальным состоянием registro до вступления узла в консенсус.- Registro o verificación del verificador требует `gas_schedule_id`; Proverka требует, чтобы запись registro была `Active`, присутствовала в индексе `(circuit_id, version)`, y чтобы Halo2 pruebas содержали `OpenVerifyEnvelope`, у которого `circuit_id`, `vk_hash` y `public_inputs_schema_hash` se combinan con un registro pequeño.

### Claves de demostración
- Las claves de prueba están fuera del libro mayor, no hay identificadores de direcciones de contenido (`pk_cid`, `pk_hash`, `pk_len`), públicos вместе с verificador de metadatos.
- Los SDK de Wallet descargan PK de forma segura, almacenan y almacenan localmente.

### Parámetros de Pedersen y Poseidón
- Registro antiguo (`PedersenParams`, `PoseidonParams`) зеркалируют контроль жизненного цикла verifier, каждая с `params_id`, хэшами generadores/constantes, acciones activas, desaprobación y retirada.
- Compromisos y estos compromisos domésticos `params_id`, поэтому ротация параметров никогда не переиспользует битовые патерны из устаревших наборов; ID встраивается в compromisos notas и доменные теги anulador.## Детерминированный порядок и anuladores
- Каждый актив поддерживает `CommitmentTree` с `next_leaf_index`; блоки добавляют compromisos в детерминированном порядке: итерация транзакций по порядку блока; внутри каждой транзакции — salidas blindadas по возрастанию сериализованного `output_idx`.
- `note_position` выводится из оффсетов дерева, но **не** входит в nullifier; он используется только для путей membresía в prueba testigo.
- Стабильность anulador при reorg обеспечивается дизайном PRF; вход PRF связывает `{ nk, note_preimage_hash, asset_id, chain_id, params_id }`, а Anchors ссылаются на исторические Merkle Roots, ограниченные `max_anchor_age_blocks`.## Libro mayor
1. **MintConfidential {id_activo, monto, sugerencia_destinatario}**
   - La política política activa `Convertible` o `ShieldedOnly`; admisión проверяет autoridad activa, извлекает текущий `params_id`, сэмплирует `rho`, эмитирует compromiso, обновляет árbol Merkle.
   - Emitido `ConfidentialEvent::Shielded` con nuevo compromiso, Merkle root delta y hash вызова транзакции для audit trail.
2. **TransferConfidential {id_activo, prueba, id_circuito, versión, anuladores, nuevos_compromisos, enc_payloads, raíz_ancla, memo}**
   - Syscall VM proporciona prueba de registro; anfitrión убеждается, что anuladores не использованы, compromisos добавлены детерминированно, а ancla свежий.
   - El libro mayor записывает `NullifierSet` entradas, хранит зашифрованные payloads для получателей/аудиторов и эмитирует `ConfidentialEvent::Transferred`, суммируя anuladores, Salidas adicionales, hash de prueba y raíces de Merkle.
3. **RevelarConfidencial {id_activo, prueba, id_circuito, versión, anulador, monto, cuenta_destinatario, raíz_ancla}**
   - Disponible para activos `Convertible`; prueba подтверждает, что значение nota равно раскрытой сумме, libro mayor начисляет прозрачный balances y сжигает nota blindada, помечая anulador как потраченный.
   - Emite `ConfidentialEvent::Unshielded` con una suma pública que utiliza anuladores, pruebas de identificación y hash de transferencias.## Modelo de datos de Дополнения
- `ConfidentialConfig` (nueva configuración de configuración) con la bandera de encendido, `assume_valid`, perillas de gas/limitadores, anclaje de encendido y verificador de backend.
- `ConfidentialNote`, `ConfidentialTransfer` y `ConfidentialMint` junto con Norito en nuevas versiones de baño (`CONFIDENTIAL_ASSET_V1 = 0x01`).
- `ConfidentialEncryptedPayload` incorpora bytes de notas AEAD en `{ version, ephemeral_pubkey, nonce, ciphertext }`, junto con `version = CONFIDENTIAL_ENCRYPTED_PAYLOAD_V1` para discos XChaCha20-Poly1305.
- La derivación de claves de vectores canónicos se realiza en `docs/source/confidential_key_vectors.json`; y CLI, y el punto final Torii se registran en esta configuración.
- `asset::AssetDefinition` получает `confidential_policy: AssetConfidentialPolicy { mode, vk_set_hash, poseidon_params_id, pedersen_params_id, pending_transition }`.
- `ZkAssetState` сохраняет привязку `(backend, name, commitment)` для transfer/unshield verificadores; Las pruebas de exclusión de datos, los archivos a los que se hace referencia o la clave de verificación en línea no cumplen con el compromiso de registro.
- `CommitmentTree` (по активу с puestos de control fronterizos), `NullifierSet` с ключом `(chain_id, asset_id, nullifier)`, `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` хранятся в estado mundial.
- Mempool admite las estructuras `NullifierIndex` e `AnchorIndex` para una amplia gama de duplicaciones y dispositivos de anclaje.
- Обновления схемы Norito включают канонический порядок entradas públicas; pruebas de ida y vuelta garantizan la conformidad determinada.- Pruebas unitarias de зашифрованных de cargas útiles de ida y vuelta (`crates/iroha_data_model/src/confidential.rs`). Los archivos vectoriales que contienen transcripciones canónicas de la AEAD para los auditores. `norito.md` документирует encabezado en cable para sobre.

## Integración con IVM y syscall
- Ввести syscall `VERIFY_CONFIDENTIAL_PROOF`, modelo:
  - `circuit_id`, `version`, `scheme`, `public_inputs`, `proof` y el resultado `ConfidentialStateDelta { asset_id, nullifiers, commitments, enc_payloads }`.
  - Syscall activa el verificador de metadatos del registro, establece límites de temperatura/intensidad, detecta gas determinado y detecta delta para pruebas de uso.
- El host incluye el rasgo de solo lectura `ConfidentialLedger` para la raíz de Merkle y el anulador de estado; La biblioteca Kotodama contiene ayudantes para testigos y validaciones.
- Función de puntero ABI de documentación, cómo analizar el búfer de prueba de diseño y los identificadores de registro.## Согласование возможностей узлов
- El apretón de manos se relaciona con `feature_bits.confidential` junto con `ConfidentialFeatureDigest { vk_set_hash, poseidon_params_id, pedersen_params_id, conf_rules_version }`. El validador de archivos `confidential.enabled=true`, `assume_valid=false`, identificadores de backend verificadores y resúmenes compatibles; несоответствия завершают handshake с `HandshakeConfidentialMismatch`.
- La configuración puede ser `assume_valid` para el observador: когда выключено, встреча конфиденциальных инструкций дает детерминированный `UnsupportedInstruction` без pánico; когда включено, los observadores применяют объявленные deltas estatales без проверки pruebas.
- Mempool ofrece transacciones confidenciales y capacidad local disponible. Los filtros de chismes protegen a los pares no identificados de tráfico, ni identifican los verificadores de identidad en los límites de los filtros.

### Матрица совместимости apretón de manos| Artículos de primera necesidad | Итог для валидаторских узлов | Примечания оператора |
|--------------------------------|------------------------------|----------------------|
| `enabled=true`, `assume_valid=false`, soporte de backend, soporte de resumen | Prinyat | Peer достигает состояния `Ready` y участвует в propuesta, votación y RBC fan-out. Ручных действий не требуется. |
| `enabled=true`, `assume_valid=false`, soporte de backend, resumen de usuario o de soporte | Отклонен (`HandshakeConfidentialMismatch`) | El control remoto debe activar el registro/parámetros de activación o el instalador `activation_height`. Пока не исправлено, узел остается обнаруживаемым, но не входит в ротацию консенсуса. |
| `enabled=true`, `assume_valid=true` | Отклонен (`HandshakeConfidentialMismatch`) | Валидаторы требуют проверку pruebas; Conecte el observador remoto con el ingreso a través de Torii o conecte `assume_valid=false` después de una prueba de seguridad. |
| `enabled=false`, se envía un protocolo de enlace (imagen antigua) o un verificador de backend | Отклонен (`HandshakeConfidentialMismatch`) | Los pares habituales y exclusivos no pueden votar en este acuerdo. Обновите и до текущего релиза и убедитесь, что backend + digest совпадают до переподключения. |Observer узлы, намеренно пропускающие проверку проверку pruebas, не должны открывать консенсусные соединения с валидаторами, работающими с capacidades gates. Si puede configurar bloques con Torii o API avanzadas, no debe establecer un consenso sobre cómo hacerlo, pero no está permitido. совместимые возможности.

### Política de revelación y anulador de retención

Libros de contabilidad confidenciales que contienen historias actualizadas sobre notas de actualidad y auditores de gobernanza. Política de privacidad, primero `ConfidentialLedger`, tal como:- **Anulador de anulación:** хранить потраченные anuladores minимум `730` дней (24 meses) после высоты траты или дольше, если требуется регулятором. Los operadores pueden utilizar el sistema `confidential.retention.nullifier_days`. Anuladores, которые моложе окна, ДОЛЖНЫ оставаться доступными через Torii, чтобы аудиторы могли доказать отсутствие doble gasto.
- **Удаление revelación:** прозрачные revelación (`RevealConfidential`) удаляют соответствующие compromisos немедленно после финализации блока, но использованный anulador остается под правилом retención выше. События reveló (`ConfidentialEvent::Unshielded`) фиксируют публичную сумму, получателя и hashproof, чтобы реконструкция исторических не требовала удаленного texto cifrado.
- **Puntos de control fronterizos:** los compromisos fronterizos pueden incluir puntos de control rodantes, grandes descuentos en `max_anchor_age_blocks` y otras retenciones. Hay muchos puntos de control más compactos que están equipados con sistemas de anulación en el interior.
- **Ремедиация устаревшего digest:** если `HandshakeConfidentialMismatch` поднят из-за дрейфа digest, оperatoram следует (1) проверить, что окна anulador de retención совпадают по кластеру, (2) запустить `iroha_cli app confidential verify-ledger` для пересчета digest по сохраненному набору nulifier, y (3) переразвернуть обновленный manifiesto. Любые anuladores, удаленные раньше срока, должны быть восстановлены из холодного хранения перед повторным входом в сеть.Documente la experiencia local en el runbook de operaciones; políticas de gobernanza, retención de datos actualizada, cambios de configuración sincronizados y planes de gestión.

### Процедура эвикции и восстановления

1. Marque `IrohaNetwork` para conocer las capacidades. Любое несоответствие поднимает `HandshakeConfidentialMismatch`; соединение закрывается, un par se coloca en la cola de descubrimiento antes de `Ready`.
2. Ошибка фиксируется в логе сетевого сервиса (включая удаленный digest and backend), y Sumeragi никогда не планирует peer для propuesta o votación.
3. Los operadores detectan problemas, modifican registros y parámetros (`vk_set_hash`, `pedersen_params_id`, `poseidon_params_id`) o подготавливая `next_conf_features` согласованной `activation_height`. Как только digest совпадает, следующий handshake проходит автоматически.
4. Если устаревший peer сумеет разослать блок (por ejemplo, через архивный replay), validadores determineros de ego s `BlockRejectionReason::ConfidentialFeatureDigestMismatch`, el libro mayor de contabilidad consistente en cada conjunto.

### Flujo de protocolo de enlace seguro para reproducción1. Каждая исходящая попытка выделяет свежий материал Ruido/X25519. La carga útil del protocolo de enlace (`handshake_signature_payload`) contiene claves públicas efímeras locales y actualizadas, la dirección de socket anunciada Norito y — если скомпилировано с `handshake_chain_id` — cadena identificadora. Сообщение AEAD-шифруется перед отправкой.
2. La carga útil mejorada se realiza mediante el protocolo peer/local y el proveedor Ed25519, instalado en `HandshakeHelloV1`. Поскольку обе claves efímeras y dirección anunciada входят в домен подписи, repetición захваченного сообщения против другого peer o восстановление устаревшего соединения детерминированно проваливает валидацию.
3. Banderas de capacidad confidencial y `ConfidentialFeatureDigest` en el directorio `HandshakeConfidentialMeta`. Seleccione la tupla `{ enabled, assume_valid, verifier_backend, digest }` con su propia configuración local `ConfidentialHandshakeCaps`; Любое несоответствие завершает handshake s `HandshakeConfidentialMismatch` до перехода транспорта в `Ready`.
4. Los operadores de ДОЛЖНЫ пересчитать digest (через `compute_confidential_feature_digest`) и перезапустить узлы с обновленными registros/políticas перед повторным подключением. Compañeros, рекламирующие старые resúmenes, продолжают проваливать apretón de manos, предотвращая возвращение устаревшего состояния в набор validatorov.5. Las respuestas y el protocolo de enlace de funciones modifican los esquemas estándar `iroha_p2p::peer` (`handshake_failure_count`, ayudantes de taxonomía de errores) y emiten estructuras Registros de identificación remota de pares y resumen de huellas dactilares. Utilice estos indicadores para activar la reproducción o nunca para la configuración de su implementación.

## Управление ключами и payloads
- Иерархия derivación ключей на аккаунт:
  - `sk_spend` → `nk` (clave anuladora), `ivk` (clave de visualización entrante), `ovk` (clave de visualización saliente), `fvk`.
- Las notas de carga útil de Зашифрованные incluyen claves compartidas derivadas de AEAD con ECDH; Las teclas de vista de auditor opcionales pueden activar las salidas en la actividad política.
- CLI de configuración: `confidential create-keys`, `confidential send`, `confidential export-view-key`, instrumentos de auditoría para notas de registro y ayudante `iroha app zk envelope` para создания/инспекции Norito sobres para notas офлайн. Torii proporciona una derivación de flujo a partir de `POST /v1/confidential/derive-keyset`, formas hexadecimal y base64, programas de gran tamaño получать иерархии ключей.## Gases, limitaciones y controles DoS
- Horario de gas determinado:
  - Halo2 (Plonkish): базовый `250_000` gas + `2_000` gas на каждый entrada pública.
  - `5` gas a prueba de байт, más por anulador (`300`) y por compromiso (`500`) начисления.
  - Los operadores pueden revisar estas constantes de configuración del usuario (`confidential.gas.{proof_base, per_public_input, per_proof_byte, per_nullifier, per_commitment}`); Las configuraciones previas al inicio o la recarga en caliente se realizan en configuraciones y ajustes determinados en el claustro.
- Жесткие лимиты (настраиваемые значения по умолчанию):
-`max_proof_size_bytes = 262_144`.
- `max_nullifiers_per_tx = 8`, `max_commitments_per_tx = 8`, `max_confidential_ops_per_block = 256`.
- `verify_timeout_ms = 750`, `max_anchor_age_blocks = 10_000`. Pruebas, превышающие `verify_timeout_ms`, детерминированно прерывают инструкцию (gobernanza-голосования эмитят `proof verification exceeded timeout`, `VerifyProof` возвращает ошибку).
- Bloques de programación disponibles: `max_proof_bytes_block`, `max_verify_calls_per_tx`, `max_verify_calls_per_block` y `max_public_inputs`. constructores; `reorg_depth_bound` (≥ `max_anchor_age_blocks`) управляет puntos de control de la frontera de retención.
- Runtime теперь отклоняет транзакции, превышающие эти por transacción o por límites de bloque, эмитируя детерминированные ошибки `InvalidParameter` и не изменяя состояние libro mayor.
- Mempool filtra la información confidencial de las transmisiones según `vk_id`, prueba y anclaje del verificador, ограничить потребление ресурсов.- Los parámetros establecidos por el tiempo de espera o los límites establecidos; транзакции падают с явными ошибками. Los backends SIMD son opcionales, no hay ningún tipo de contabilidad contable.

### Базовые калибровки y критерии приемки
- **Plataformas de referencia.** Калибровочные прогоны ДОЛЖНЫ покрывать три профиля оборудования ниже. Прогоны без всех профилей отклоняются при ревью.

  | Perfil | Arquitecto | CPU/instancia | Compilador de banderas | Назначение |
  | --- | --- | --- | --- | --- |
  | `baseline-simd-neutral` | `x86_64` | AMD EPYC 7B12 (32c) y Intel Xeon Gold 6430 (24c) | `RUSTFLAGS="-C target-feature=-avx,-avx2,-fma"` | La configuración básica se realiza según instrucciones vectoriales; используется для настройки tablas de costos de respaldo. |
  | `baseline-avx2` | `x86_64` | Intel Xeon Gold 6430 (24c) | versión predeterminada | Validar poner AVX2; проверяет, что SIMD ускорения укладываются в допуск нейтрального gas. |
  | `baseline-neon` | `aarch64` | AWS Graviton3 (c7g.4xlarge) | versión predeterminada | Garantizamos que el backend NEON está configurado y configurado con compatibilidad x86. |

- **Arnés de referencia.** Все отчеты по калибровке газа ДОЛЖНЫ быть получены с:
  - `CRITERION_HOME=target/criterion cargo bench -p iroha_core isi_gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>`
  - `cargo test -p iroha_core bench_repro -- --ignored` para los equipos de calefacción.
  - `CRITERION_HOME=target/criterion cargo bench -p ivm gas_calibration -- --sample-size 200 --warm-up-time 5 --save-baseline <profile-label>` всякий раз, когда меняются стоимости VM opcode.- **Se corrigió la aleatoriedad.** Exportar `IROHA_CONF_GAS_SEED=conf-gas-seed-2026Q1` antes del banco de datos, чтобы `iroha_test_samples::gen_account_in` directamente en el ordenador. `KeyPair::from_seed`. Arnés печатает `IROHA_CONF_GAS_SEED_ACTIVE=…` один раз; если переменная отсутствует, ревью ДОЛЖНО провалиться. Любые новые утилиты калибровки должны продолжать учитывать эту env var при введении вспомогательной случайности.

- **Captura de resultados.**
  - Загружайте Resúmenes de criterios (`target/criterion/**/raw.csv`) para el perfil del archivo en el artefacto de lanzamiento.
  - Храните производные метрики (`ns/op`, `gas/op`, `ns/gas`) en [Libro mayor de calibración de gas confidencial](./confidential-gas-calibration) junto con git commit y версией компилятора.
  - Держите последние две baselines на perfil; удаляйте более старые снимки после проверки нового отчета.

- **Tolerancias de aceptación.**
  - Los gases de escape `baseline-simd-neutral` e `baseline-avx2` están ajustados a ≤ ±1,5%.
  - Los gases de escape `baseline-simd-neutral` e `baseline-neon` están ajustados a ≤ ±2,0%.
  - Calibres de calibración, registros de estas porciones, descargas de correctores rápidos y RFC con objeciones расхождения и мерами.- **Revisar lista de verificación.** Отправители отвечают за:
  - Включение `uname -a`, выдержек `/proc/cpuinfo` (modelo, paso a paso) y `rustc -Vv` en калибровочный лог.
  - El proveedor `IROHA_CONF_GAS_SEED` contiene semillas activas.
  - Убедиться, что marcapasos y verificador confidencial indicadores de funciones совпадают с producción (`--features confidential,telemetry` при запуске бенчей с Telemetría).

## Configuración y operaciones
- `iroha_config` de la sección `[confidential]`:
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
- Los televisores emiten métricas agregadas: `confidential_proof_verified`, `confidential_verifier_latency_ms`, `confidential_proof_bytes_total`, `confidential_nullifier_spent`, `confidential_commitments_appended`, `confidential_mempool_rejected_total{reason}` y `confidential_policy_transitions_total` no contienen texto plano.
- Superficies RPC:
  - `GET /confidential/capabilities`
  - `GET /confidential/zk_registry`
  - `GET /confidential/params`## Prueba de estrategia
- Determinación: transferencia aleatoria de bloques entre raíces de Merkle y conjuntos de anuladores.
- Устойчивость к reorg: симуляция многоблочных reorg с Anchors; anuladores остаются стабильными, а устаревшие anclajes отклоняются.
- Инварианты газа: одинаковый расход газа на узлах с и без SIMD ускорения.
- Pruebas de gran tamaño: pruebas de encendido/apagado de gas, máximo agua/agua, tiempo de espera de aplicación.
- Ciclo de vida: control de operaciones para activaciones/desactivaciones, verificador y parámetros, pruebas de rotación.
- Política FSM: разрешенные/запрещенные переходы, задержки pendientes de transición y отклонения mempool вокруг alturas efectivas.
- Registro de situaciones actuales: retiro de emergencia замораживает затронутые активы на `withdraw_height` y отклоняет после нее.
- Control de capacidad: validadores con nuevos bloques `conf_features`; Observers с `assume_valid=true` продолжают следовать без влияния на консенсус.
- Эквивалентность состояния: validador/completo/observador que utiliza raíces de estado idénticas en patrones canónicos.
- Fuzzing negativo: pruebas de gran tamaño, cargas útiles sobredimensionadas y anulador de colisiones determinados por la exclusión.## Миграция и совместимость
- Implementación controlada por funciones: para la fase C3 `enabled` para la fase `false`; узлы объявляют capacidades перед входом в набор валидаторов.
- Прозрачные активы не затронуты; конфиденциальные инструкции требуют записей registro y capacidad de peregovorov.
- Узлы, собранные без поддержки confidencial, детерминированно отклоняют соответствующие блоки; они не могут входить в набор валидаторов, но могут работать как observers с `assume_valid=true`.
- Génesis manifiesta varias entradas de registro, parámetros de configuración, políticas confidenciales para activos y claves de auditor opcionales.
- Los operadores utilizan runbooks públicos para el registro de rotaciones, procedimientos políticos y retiros de emergencia, qué pueden hacer con los termómetros апгрейды.## Незавершенная работа
- Pruebe los parámetros de referencia de Halo2 (circuito de configuración, búsqueda de estrategias) y evalúe los resultados del libro de jugadas de calibración, así como los programas de gas/tiempo de espera. следующим обновлением `confidential_assets_calibration.md`.
- Eliminación de políticas de auditoría y API de visualización selectiva, que incluyen un flujo de trabajo completo en Torii después de la gobernanza.
- Actualice el cifrado de testigos para salidas de múltiples destinatarios y memorandos por lotes, evalúe el formato del sobre para los implementadores del SDK.
- Заказать внешнюю circuitos de revisión de seguridad, registro y процедур ротации параметров y архивировать результаты рядом с внутренними informes de auditoría.
- Especificación del gasto de API para los auditores y publicación de la guía para el alcance de la clave de vista, los proveedores que realizan las semánticas attestasias.## Этапы реализации
1. **Fase M0 — Endurecimiento de parada de envío**
   - ✅ Anulador de derivación теперь следует дизайну Poseidon PRF (`nk`, `rho`, `asset_id`, `chain_id`) с детерминированным порядком compromisos в обновлениях libro mayor.
   - ✅ Исполнение применяет лимиты размера размера и квоты конфиденциальных операций por transacción/por bloque, отклоняя транзакции сверх лимитов детерминированными ошибками.
   - ✅ El protocolo de enlace P2P está activado por `ConfidentialFeatureDigest` (resumen de backend + registro externo) y está determinado por no disponible desde `HandshakeConfidentialMismatch`.
   - ✅ Los pánicos habituales en las rutas de ejecución confidenciales y la activación de roles de los usuarios no deseados.
   - ⚪ Activar el tiempo de espera del verificador y reorganizar los límites de los puntos de control fronterizos.
     - ✅ Бюджеты timeout для проверки применены; pruebas, превышающие `verify_timeout_ms`, теперь детерминированно падают.
     - ✅ Puntos de control fronterizos теперь учитывают `reorg_depth_bound`, удаляя checkpoints старше заданного окна при сохранении детерминированных снимков.
   - Ввести `AssetConfidentialPolicy`, política FSM y puertas de cumplimiento para la instrucción mint/transfer/reveal.
   - Inserte `conf_features` en bloques de almacenamiento y elimine los validadores de resúmenes de registros/parámetros.
2. **Fase M1: Registros y parámetros**- Внедрить registro `ZkVerifierEntry`, `PedersenParams`, `PoseidonParams` con operaciones de gobernanza, anclaje de génesis y управлением кэшем.
   - Puede realizar llamadas al sistema para realizar búsquedas de registros, ID de programas de gas, hash de esquemas y comprobaciones de tamaño.
   - Publicar el formato de carga útil v1, derivación de vectores, clave de acceso y conexión CLI para actualizar claves confidenciales.
3. **Fase M2: Gas y rendimiento**
   - Realización de programas de gas determinados, programas por bloque y arneses de referencia con televisores (verificar latencia, tamaños de prueba, rechazos de mempool).
   - Crear puntos de control CommitmentTree, carga LRU e índices anulados para múltiples actividades.
4. **Fase M3: Rotación y herramientas de billetera**
   - Включить поддержку pruebas multiparámetro y multiversión; Utilice la activación/desaprobación basada en la gobernanza en runbooks de transición.
   - Поставить миграционные потоки для wallet SDK/CLI, flujo de trabajo, análisis de auditoría y herramientas de gasto.
5. **Fase M4: Auditoría y operaciones**
   - Actualización de flujos de trabajo para claves de auditor, API de divulgación selectiva y runbooks operativos.
   - Introduzca la información criptográfica/biográfica actualizada y publíquela en `status.md`.

Cada vez que se actualizan los hitos de la hoja de ruta y se realizan pruebas, se garantizan determinadas garantías de implementación de la cadena de bloques.