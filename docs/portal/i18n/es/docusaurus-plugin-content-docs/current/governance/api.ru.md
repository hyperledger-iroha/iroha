---
lang: es
direction: ltr
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Estado: черновик/набросок для сопровождения задач по реализации gobernancia. Формы могут меняться в ходе разработки. La determinación y política de RBAC son normas normativas; Torii puede transmitir/transmitir transmisiones con los clientes `authority` e `private_key`, cuando los clientes solicitan y отправляют в `/transaction`.Observar
- Todos los puntos finales contienen JSON. Para las máquinas, estas son las transmisiones, las otras conexiones `tx_instructions` - grandes o pequeñas instrucciones-esqueléticas:
  - `wire_id`: instrucciones del tipo de identificador de restablecimiento
  - `payload_hex`: carga útil de bloques Norito (hexadecimal)
- Los anteriores `authority` e `private_key` (y `private_key` en boletas DTO), Torii pueden y se muestran. транзакцию и все равно возвращает `tx_instructions`.
- Los clientes que utilizan SignedTransaction con su autoridad y chain_id, envían mensajes y POST en `/transaction`.
- SDK de contraseña:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` indica `GovernanceProposalResult` (normalmente según estado/tipo), `ToriiClient.get_governance_referendum_typed` indica `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` de `GovernanceTally`, `ToriiClient.get_governance_locks_typed` de `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` de `GovernanceUnlockStats`, y `ToriiClient.list_governance_instances_typed` incluye `GovernanceInstancesPage`, un descargador típico obligatorio para nuestra gobernanza de acceso público con aplicaciones iniciales en README.
- Cliente Python completo (`iroha_torii_client`): `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal` crean paquetes típicos `GovernanceInstructionDraft` Torii esqueleto `tx_instructions`), un archivo JSON predeterminado para archivos Finalizar/Promulgar flujos.- JavaScript (`@iroha/iroha-js`): `ToriiClient` proporciona ayudas tipográficas para propuestas, referendos, recuentos, bloqueos, desbloqueo de estadísticas y el método `listGovernanceInstances(namespace, options)` además de puntos finales del consejo. (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), los clientes de Node.js pueden paginar `/v1/gov/instances/{ns}` y descargar Los flujos de trabajo respaldados por VRF se basan en instalaciones contractuales.

Puntos finales

- PUBLICACIÓN `/v1/gov/proposals/deploy-contract`
  - Solicitud (JSON):
    {
      "espacio de nombres": "aplicaciones",
      "contract_id": "mi.contrato.v1",
      "code_hash": "blake2b32:..." | "...64hexadecimales",
      "abi_hash": "blake2b32:..." | "...64hexadecimales",
      "abi_version": "1",
      "ventana": { "inferior": 12345, "superior": 12400 },
      "autoridad": "ih58…?",
      "clave_privada": "...?"
    }
  - Respuesta (JSON):
    { "ok": verdadero, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validación: ноды канонизируют `abi_hash` для заданного `abi_version` и отвергают несовпадения. Para `abi_version = "v1"` o жидаемое значение - `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contratos (implementación)
- PUBLICACIÓN `/v1/contracts/deploy`
  - Solicitud: { "autoridad": "ih58...", "private_key": "...", "code_b64": "..." }
  - Comportamiento: вычисляет `code_hash` по телу IVM programas e `abi_hash` по заголовку `abi_version`, затем отправляет `RegisterSmartContractCode` (manifiesto) y `RegisterSmartContractBytes` (bajos bloques `.to`) de los iconos `authority`.
  - Respuesta: { "ok": verdadero, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> manifiesto de возвращает сохраненный
    - OBTENER `/v1/contracts/code-bytes/{code_hash}` -> возвращает `{ code_b64 }`
- PUBLICACIÓN `/v1/contracts/instance`
  - Solicitud: { "autoridad": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: implemente el código de bytes anterior y active la asignación `(namespace, contract_id)` desde `ActivateContractInstance`.
  - Respuesta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Servicio de alias
- PUBLICACIÓN `/v1/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - Respuesta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценщика. Текущее значение: `blake2b512-mock`.
  - Notas: детерминированный simulacro de оценщик, применяющий Blake2b512 с separación de dominio `iroha.alias.voprf.mock.v1`. Antes de las herramientas de prueba, la tubería VOPRF de producción no se puede conectar a Iroha.
  - Errores: HTTP `400` при некорректном hexadecimal вводе. Torii contiene el sobre Norito `ValidationFail::QueryFailed::Conversion` con una decoración decodificadora.
- PUBLICACIÓN `/v1/aliases/resolve`
  - Solicitud: { "alias": "GB82 OESTE 1234 5698 7654 32" }
  - Respuesta: { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - Notas: требует puesta en escena del tiempo de ejecución del puente ISO (`[iso_bridge.account_aliases]` en `iroha_config`). Torii нормализует alias, удаляя пробелы и приводя к верхнему регистру. Utilice el alias 404 y el 503, según el tiempo de ejecución del puente ISO.
- PUBLICACIÓN `/v1/aliases/resolve_index`
  - Solicitud: { "índice": 0 }
  - Respuesta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - Notas: индексы alias назначаются детерминированно по порядку конфигурации (basado en 0). Los clientes pueden solicitar información fuera de línea para realizar un seguimiento de auditoría mediante un alias de certificación.Código Tamaño Tapa
- Parámetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Управляет максимальным допустимым размером (в байтах) хранения кода контрактов.
  - Predeterminado: 16 MiB. Si no se bloquea `RegisterSmartContractBytes`, el parámetro `.to` bloquea el límite previo y se produce una violación del invariante.
  - Los operadores pueden identificar entre `SetParameter(Custom)` y `id = "max_contract_code_bytes"` y cada carga útil.

- PUBLICACIÓN `/v1/gov/ballots/zk`
  - Solicitud: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas:
    - Когда публичные входы схемы включают `owner`, `amount` e `duration_blocks`, y pruebas de verificación de datos personales VK, нода создает или продлевает gobernance lock для `election_id` с этим `owner`. Направление скрыто (`unknown`); обновляются только cantidad/vencimiento. Otros monótonos: cantidad y vencimiento para los usuarios (nuevamente, cantidad máxima (cantidad, cantidad anterior) y máx (expiración, vencimiento anterior)).
    - ZK vuelve a votar, aumenta la cantidad o el vencimiento, confirma el servidor con el diagnóstico `BallotRejected`.
    - El contrato de instalación debe conectarse a `ZK_VOTE_VERIFY_BALLOT` y a `SubmitBallot`; хосты hacer cumplir одноразовый pestillo.- PUBLICACIÓN `/v1/gov/ballots/plain`
  - Solicitud: { "authority": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Sí|No|Abstenerse" }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas: re-votos только на расширение - nueva boleta не может уменьшить cantidad o vencimiento candado. `owner` es compatible con las transmisiones de autoridad. Минимальная длительность - `conviction_step_blocks`.- PUBLICACIÓN `/v1/gov/finalize`
  - Solicitud: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efecto en cadena (andamio actual): promulgar la propuesta de implementación completa вставляетчимальный `ContractManifest`, привязанный к `code_hash`, с ожидаемым `abi_hash` и помечает propuesta как Promulgada. Если manifest уже существует для `code_hash` с другим `abi_hash`, promulgación отклоняется.
  - Notas:
    - Для ZK elecciones контракты должны вызвать `ZK_VOTE_VERIFY_TALLY` до `FinalizeElection`; хосты hacer cumplir одноразовый pestillo. `FinalizeReferendum` отклоняет ZK-референдумы, пока tally выборов не финализирован.
    - Автозакрытие на `h_end` emite Aprobado/Rechazado para usuarios simples; ZK-референдумы остаются Cerrado, пока не будет отправлен финализированный tally and выполнен `FinalizeReferendum`.
    - Проверки participación используют только aprobar+rechazar; abstenerse не учитывается в participación.- PUBLICACIÓN `/v1/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii отправляет подписанную транзакцию при наличии `authority`/`private_key`; иначе возвращает esqueleto для подписи и отправки клиентом. La preimagen es opcional y se muestra información sobre el personaje.

- OBTENER `/v1/gov/proposals/{id}`
  - Ruta `{id}`: ID de propuesta hexadecimal (64 caracteres)
  - Respuesta: {"encontrado": bool, "propuesta": {...}? }

- OBTENER `/v1/gov/locks/{rid}`
  - Ruta `{rid}`: cadena de identificación del referéndum
  - Respuesta: { "found": bool, "referendum_id": "rid", "locks": {...}? }

- OBTENER `/v1/gov/council/current`
  - Respuesta: { "época": N, "miembros": [{ "account_id": "..." }, ...] }
  - Notas: возвращает persistió el consejo при наличии; иначе деривирует детерминированный respaldo, используя настроенный participación de activos y umbrales (зеркалит спецификацию VRF до тех пор, пока VRF pruebas не будут сохранены en cadena).- PUBLICACIÓN `/v1/gov/council/derive-vrf` (característica: gov_vrf)
  - Solicitud: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Pequeño", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: prueba el código de prueba VRF en la entrada canónica, el código `chain_id`, `epoch` y el hash de bloque de baliza; сортирует по bytes de salida desc с desempates; возвращает top `committee_size` участников. Не сохраняется.
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk â G1, prueba â G2 (96 bytes). Pequeño = pk â G2, prueba â G1 (48 bytes). Entradas доменно разделены и включают `chain_id`.

### Valores predeterminados de gobernanza (iroha_config `gov.*`)

Consejo alternativo, utilizado Torii en la lista persistente de terceros, parámetro según `iroha_config`:

```toml
[gov]
  vk_ballot.backend = "halo2/ipa"
  vk_ballot.name    = "ballot_v1"
  vk_tally.backend  = "halo2/ipa"
  vk_tally.name     = "tally_v1"
  plain_voting_enabled = false
  conviction_step_blocks = 100
  max_conviction = 6
  approval_q_num = 1
  approval_q_den = 2
  min_turnout = 0
  parliament_committee_size = 21
  parliament_term_blocks = 43200
  parliament_min_stake = 1
  parliament_eligibility_asset_id = "SORA#stake"
```

Эквивалентные переменные окружения:

```
GOV_VK_BACKEND=halo2/ipa
GOV_VK_NAME=ballot_v1
GOV_PARLIAMENT_COMMITTEE_SIZE=21
GOV_PARLIAMENT_TERM_BLOCKS=43200
GOV_PARLIAMENT_MIN_STAKE=1
GOV_PARLIAMENT_ELIGIBILITY_ASSET_ID=SORA#stake
GOV_ALIAS_TEU_MINIMUM=0
GOV_ALIAS_FRONTIER_TELEMETRY=true
```

`parliament_committee_size` ограничивает число fallback членов при отсутствии consejo, `parliament_term_blocks` задает длину эпохи для derivation seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` es una participación mínima (en ediciones mínimas) en el activo de elegibilidad, y `parliament_eligibility_asset_id` se convierte en un activo de balance построении набора кандидатов.Verificación de gobernanza VK sin omitir: las boletas de verificación incluyen la clave de verificación `Active` con bytes en línea, y la operación no se realiza automáticamente en los conmutadores de prueba, como se muestra проверку.

RBAC
- выполнение требует разрешений en cadena:
  - Propuestas: `CanProposeContractDeployment{ contract_id }`
  - Boletas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgación: `CanEnactGovernance`
  - Dirección del consejo (futuro): `CanManageParliament`Espacios de nombres protegidos
- El parámetro personalizado `gov_protected_namespaces` (matriz JSON de cadenas) permite la puerta de admisión para implementar espacios de nombres específicos.
- Los clientes que desean implementar claves de metadatos en espacios de nombres protegidos:
  - `gov_namespace`: espacio de nombres del teléfono (nombre, "aplicaciones")
  - `gov_contract_id`: ID de contrato logístico en el espacio de nombres
- `gov_manifest_approvers`: ID de cuentas de matriz JSON opcionales validados. Когда carril manifiesto задает quórum > 1, admisión требует autoridad транзакции плюс перечисленные аккаунты для удовлетворения quórum maniфеста.
- Los mostradores de admisión de telemetría anteriores a `governance_manifest_admission_total{result}`, los operadores autorizados admiten usuarios de `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` y `runtime_hook_rejected`.
- La ruta de aplicación de la configuración telemétrica es `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`), la supervisión del operador недостающие aprobaciones.
- Los carriles aplican la lista permitida de espacios de nombres, опубликованный в manifests. Любая транзакция, устанавливающая `gov_namespace`, должна предоставить `gov_contract_id`, а namespace должен присутствовать в set `protected_namespaces` manifiesto. Envíos `RegisterSmartContractCode` basados ​​en estos metadatos отвергаются, когда защита включена.
- Admisión требует, чтобы существовал Propuesta de gobernanza promulgada для tupla `(namespace, contract_id, code_hash, abi_hash)`; иначе валидация завершается ошибкой No permitido.Ganchos de actualización en tiempo de ejecución
- Los manifiestos de carril pueden modificar `hooks.runtime_upgrade` para la instalación de actualización del tiempo de ejecución de puerta (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Gancho de polo:
  - `allow` (bool, predeterminado `true`): junto con `false`, todas las instrucciones de actualización de tiempo de ejecución.
  - `require_metadata` (bool, predeterminado `false`): tres metadatos de entrada, incluido `metadata_key`.
  - `metadata_key` (cadena): имя metadatos, gancho forzado. Predeterminado `gov_upgrade_id`, muchos metadatos están disponibles en la lista de permitidos.
  - `allowed_ids` (matriz de cadenas): lista blanca opcional de metadatos (después de recortar). Отклоняет, когда предоставленное значение не входит в список.
- Когда gancho присутствует, admisión очереди hacer cumplir la política de metadatos para la transferencia de datos en очередь. Los metadatos externos, pueden incluirse o incluirse en la lista de permitidos según el criterio de NotPermitted.
- La televisión muestra los resultados según `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Transacciones, ganchos de enlace, archivos de metadatos `gov_upgrade_id=<value>` (o clics, manifiestos antiguos) junto con aprobaciones del validador, требуемыми quórum manifiesto.Punto final de conveniencia
- POST `/v1/gov/protected-namespaces` - применяет `gov_protected_namespaces` напрямую на ноде.
  - Solicitud: { "espacios de nombres": ["aplicaciones", "sistema"]}
  - Respuesta: { "ok": verdadero, "aplicado": 1 }
  - Notas: предназначен для admin/testing; требует API token при конфигурации. En producción, la transmisión se realiza con `SetParameter(Custom)`.Ayudantes de CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Las instalaciones de contratación de espacio de nombres y proveedores, como:
    - Torii contiene el código de bytes del archivo `code_hash` y el resumen Blake2b-32 del archivo `code_hash`.
    - Manifiesto под `/v1/contracts/code/{code_hash}` сообщает совпадающие `code_hash` и `abi_hash`.
    - Se promulgó una propuesta de gobernanza para `(namespace, contract_id, code_hash, abi_hash)`, que deriva del hash de ID de propuesta, que no se implementa.
  - El formato JSON está escrito en `results[]` para el contrato de usuario (problemas, resúmenes de manifiesto/código/propuesta) y el resumen externo, no disponible (`--no-summary`).
  - Полезно для аудита espacios de nombres protegidos y proporciona flujos de trabajo de implementación controlados por la gobernanza.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Utilice metadatos de esqueleto JSON para implementar en espacios de nombres protegidos, incluida la opción `gov_manifest_approvers` para el quórum de unión del manifiesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — sugerencias de bloqueo disponibles en `min_bond_amount > 0`, y otras sugerencias de bloqueo incluidas en `owner`, `amount` y `duration_blocks`.
  - Valida los identificadores de cuentas canónicas, canonicaliza las sugerencias de anulación de 32 bytes y combina las sugerencias en `public_inputs_json` (con `--public <path>` para anulaciones adicionales).
  - El anulador se deriva del compromiso de prueba (entrada pública) más `domain_tag`, `chain_id` e `election_id`; `--nullifier` se valida con la prueba cuando se suministra.- Resumen del código de configuración del ordenador `fingerprint=<hex>` y consejos de codificación `CastZkBallot` (`owner`, `amount`, `duration_blocks`, `direction` por ejemplo).
  - CLI que muestra los polos `tx_instructions[]` y `payload_fingerprint_hex` y los polos decodificadores, las herramientas posteriores pueden proporcionar esqueleto sin problemas. Realización de la decodificación Norito.
  - Las sugerencias de bloqueo previas pueden permitir que el nodo emita `LockCreated`/`LockExtended` para las papeletas ZK, ya que se trata de una contraseña.
- `iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` contienen indicadores ZK para la paridad de los scripts.
  - Resumen de la información de `vote --mode zk`, de las instrucciones de registro de huellas dactilares y de la papeleta de votación (`owner`, `amount`, `duration_blocks`, `direction`), давая быстрое подтверждение перед подписью esqueleto.Listado de instancias
- GET `/v1/gov/instances/{ns}` - список активных контрактных инстансов для namespace.
  - Parámetros de consulta:
    - `contains`: filtro de la subcadena `contract_id` (distingue entre mayúsculas y minúsculas)
    - `hash_prefix`: фильтр по prefijo hexadecimal `code_hash_hex` (minúscula)
    - `offset` (predeterminado 0), `limit` (predeterminado 100, máx. 10_000)
    - `order`: `cid_asc` (predeterminado), `cid_desc`, `hash_asc`, `hash_desc`
  - Respuesta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Ayudante de SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) o `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Desbloquear barrido (Operador/Auditoría)
- OBTENER `/v1/gov/unlocks/stats`
  - Respuesta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` отражает последний altura del bloque, когда cerraduras caducadas были barrido и persisten. `expired_locks_now` está bloqueado para escanear registros de bloqueo con `expiry_height <= height_current`.
- PUBLICACIÓN `/v1/gov/ballots/zk-v1`
  - Solicitud (DTO estilo v1):
    {
      "autoridad": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "sobre_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "propietario": "ih58…?",
      "nulificador": "blake2b32:...64hex?"
    }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }- PUBLICACIÓN `/v1/gov/ballots/zk-v1/ballot-proof` (característica: `zk-ballot`)
  - El formato JSON `BallotProof` y el esqueleto `CastZkBallot`.
  - Solicitud:
    {
      "autoridad": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "votación": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // convertidor base64 ZK1 o H2*
        "root_hint": null, // cadena hexadecimal de 32 bytes opcional (raíz de elegibilidad)
        "propietario": nulo, // Opcional AccountId когда circuito фиксирует propietario
        "nullifier": null // cadena hexadecimal de 32 bytes opcional (pista de anulación)
      }
    }
  - Respuesta:
    {
      "bien": cierto,
      "aceptado": verdadero,
      "razón": "construir esqueleto de transacción",
      "tx_instrucciones": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notas:
    - Mapa del servidor opcional `root_hint`/`owner`/`nullifier` en la boleta en `public_inputs_json` para `CastZkBallot`.
    - Los bytes de sobre están codificados en base64 para las instrucciones de carga útil.
    - `reason` меняется на `submitted transaction`, когда Torii отправляет ballot.
    - Este punto final se utiliza con la característica `zk-ballot`.Ruta de verificación de CastZkBallot
- `CastZkBallot` codifica la prueba base64 y protege las cargas útiles de datos/bits (`BallotRejected` con `invalid or empty proof`).
- El anfitrión actualiza la clave de verificación de la boleta en el referéndum (`vk_ballot`) y los valores predeterminados de gobernanza y el tributo, los que cierran su cuenta, el bloque `Active` y el codificador. bytes en línea.
- Сохраненные bytes de clave de verificación повторно хешируются с `hash_vk`; любое несовпадение останавливает выполнение до проверки для защиты от подмененных entradas de registro (`BallotRejected` с `verifying key commitment mismatch`).
- Bytes de prueba отправляются в зарегистрированный backend через `zk::verify_backend`; transcripciones no válidas приводят к `BallotRejected` с `invalid proof` и инструкция детерминированно падает.
- La prueba debe exponer un compromiso electoral y una raíz de elegibilidad como aportes públicos; la raíz debe coincidir con el `eligible_root` de la elección y el anulador derivado debe coincidir con cualquier sugerencia proporcionada.
- Успешные pruebas emitidas `BallotAccepted`; повторные anuladores, устаревшие elegibilidad raíces или regresión bloqueo продолжают давать существующие причины отказа, описанные ранее.

## Ненадлежащее поведение валидаторов и совместный консенсус

### Proceso de corte y encarcelamientoEl consenso emite Norito con codificación `Evidence` antes de validar el protocolo de validación. La carga útil se incluye en la memoria `EvidenceStore` y, además, el nuevo material en `consensus_evidence` respaldado por WSV. La estrella `sumeragi.npos.reconfig.evidence_horizon_blocks` (bloques `7200` predeterminados) está desactivada, no hay registros disponibles para ella. operadores. La evidencia dentro del horizonte también respeta `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el retraso de corte `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`); la gobernanza puede cancelar las sanciones con `CancelConsensusEvidencePenalty` antes de que se aplique la reducción.

Распознанные нарушения отображаются один-к-одному на `EvidenceKind`; Discriminantes estables y seguros en el modelo de datos:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidQc,
    EvidenceKind::InvalidProposal,
    EvidenceKind::Censorship,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - validador de conflictos de configuración para el código `(phase,height,view,epoch)`.
- **InvalidQc** - Agregador de certificado de confirmación eliminado, esta forma no requiere pruebas determinadas (por ejemplo, mapa de bits del firmante).
- **InvalidProposal** - líder del bloque, que valida la estructura de la cadena bloqueada (por ejemplo, no aplica la regla de cadena cerrada).
- **Censura**: los recibos de envío firmados muestran una transacción que nunca se propuso ni se comprometió.

Los operadores e instrumentos pueden programar y distribuir automáticamente cargas útiles como:

- Torii: `GET /v1/sumeragi/evidence` y `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.Gobernanza должна рассматривать bytes de evidencia как каноническое доказательство:

1. **Собрать payload** до истечения срока. Архивировать сырые Norito bytes вместе с altura/ver metadatos.
2. **Подготовить штраф** встроив payload в referéndum o la instrucción sudo (por ejemplo, `Unregister::peer`). Исполнение повторно валидирует carga útil; evidencia mal formada o obsoleta отклоняется детерминированно.
3. **Запланировать seguimiento de la topología** чтобы нарушивший validador не смог сразу вернуться. Los tipos de fotos incluyen `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` en la lista abierta.
4. **Audit результатов** через `/v1/sumeragi/evidence` и `/v1/sumeragi/status`, чтобы убедиться, что счетчик вырос и gobernancia выполнила удаление.

### Последовательность consenso conjunto

Garantía de consenso conjunto, что исходный набор валидаторов финализирует пограничный блок до того, как новый набор начнет предлагать. El tiempo de ejecución aplica todos los parámetros de los parámetros:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` son números cometidos en **el bloque**. `mode_activation_height` должен быть строго больше высоты блока, который внес обновление, обеспечивая minимум один блок лаг.
- `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) - Esta configuración de guardia, который предотвращает hand-off sin logotipos:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`) retrasa la reducción del consenso para que la gobernanza pueda cancelar las sanciones antes de que se apliquen.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```- El tiempo de ejecución y la CLI incluyen parámetros preparados como `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, los operadores que permiten alturas de activación y validadores de lista.
- Автоматизация gobernanza всегда должна:
  1. Финализировать решение об исключении (или восстановлении), поддержанное evidencia.
  2. Registrar la reconfiguración de seguimiento con `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorice `/v1/sumeragi/status` para conectar `effective_consensus_mode` a su vehículo actual.

Los scripts completos, los validadores de rotadores y los primeros cortes, **no es necesario** activar la activación de retardo cero o desactivar los parámetros de transferencia; такие транзакции отклоняются оставляют сеть в предыдущем режиме.

## Superficies de telemetría- Prometheus métricas de gobernanza de actividad deportiva:
  - `governance_proposals_status{status}` (calibre) отслеживает количество propuestas по статусу.
  - `governance_protected_namespace_total{outcome}` (contador) que permite/rechaza la admisión de espacios de nombres protegidos.
  - `governance_manifest_activations_total{event}` (contador) archivo de manifiesto de archivos (`event="manifest_inserted"`) y enlaces de espacio de nombres (`event="instance_bound"`).
- `/status` incluye el objeto `governance`, varias propuestas de nombres, totales de espacios de nombres protegidos y activaciones de manifiesto no deseadas. (espacio de nombres, identificación del contrato, código/hash ABI, altura del bloque, marca de tiempo de activación). Los operadores pueden operar estos polos, cómo controlarlos, qué promulgaciones revelan manifiestos y qué puertas de espacio de nombres protegidos están disponibles.
- La plantilla Grafana (`docs/source/grafana_governance_constraints.json`) y el runbook de telemetría en `telemetry.md` incluyen opciones para crear propuestas y activaciones de manifiestos. Estos son nuevos espacios de nombres protegidos en algunas actualizaciones de tiempo de ejecución.