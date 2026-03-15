---
lang: es
direction: ltr
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Estado: borrador/boceto para acompañar las tareas de implementación de gobernanza. Las formas pueden cambiar durante la implementación. El determinismo y la política RBAC son restricciones normativas; Torii puede firmar/enviar transacciones cuando se proporciona `authority` y `private_key`, de lo contrario los clientes construyen y envían a `/transaction`.resumen
- Todos los puntos finales devuelven JSON. Para flujos que producen transacciones, las respuestas incluyen `tx_instructions` - un arreglo de una o más instrucciones esqueleto:
  - `wire_id`: identificador de registro para el tipo de instrucción
  - `payload_hex`: bytes de carga útil Norito (hexadecimal)
- Si se proporciona `authority` y `private_key` (o `private_key` en DTOs de ballots), Torii firma y envia la transacción y aun devuelve `tx_instructions`.
- De lo contrario, los clientes arman una SignedTransaction usando su autoridad y chain_id, luego firman y hacen POST a `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` devuelve `GovernanceProposalResult` (normaliza campos status/kind), `ToriiClient.get_governance_referendum_typed` devuelve `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` devuelve `GovernanceTally`, `ToriiClient.get_governance_locks_typed` devuelve `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` devuelve `GovernanceUnlockStats`, y `ToriiClient.list_governance_instances_typed` devuelve `GovernanceInstancesPage`, imponiendo acceso tipado en toda la superficie de gobernanza con ejemplos de uso en README.
- Cliente ligero Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal` devuelven paquetes tipados `GovernanceInstructionDraft` (envolviendo el esqueleto `tx_instructions` de Torii), evitando parseo manual JSON cuando scripts componen flujos Finalizar/promulgar.- JavaScript (`@iroha/iroha-js`): `ToriiClient` exponen helpers tipados para propuestas, referendos, recuentos, bloqueos, desbloqueo de estadísticas, y ahora `listGovernanceInstances(namespace, options)` mas los endpoints del consejo (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que los clientes Node.js puedan paginar `/v2/gov/instances/{ns}` y conducir flujos respaldados por VRF junto con el listado de instancias de contrato existente.

Puntos finales

- PUBLICACIÓN `/v2/gov/proposals/deploy-contract`
  - Solicitud (JSON):
    {
      "espacio de nombres": "aplicaciones",
      "contract_id": "mi.contrato.v1",
      "code_hash": "blake2b32:..." | "...64hexadecimales",
      "abi_hash": "blake2b32:..." | "...64hexadecimales",
      "abi_version": "1",
      "ventana": { "inferior": 12345, "superior": 12400 },
      "autoridad": "i105…?",
      "clave_privada": "...?"
    }
  - Respuesta (JSON):
    { "ok": verdadero, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validacion: los nodos canonizan `abi_hash` para el `abi_version` provisto y rechazan desajustes. Para `abi_version = "v1"`, el valor esperado es `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contratos (implementar)
- PUBLICACIÓN `/v2/contracts/deploy`
  - Solicitud: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamiento: calcula `code_hash` del cuerpo del programa IVM y `abi_hash` del header `abi_version`, luego envia `RegisterSmartContractCode` (manifiesto) y `RegisterSmartContractBytes` (bytes `.to` completos) en nombre de `authority`.
  - Respuesta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v2/contracts/code/{code_hash}` -> devuelve el manifiesto almacenado
    - GET `/v2/contracts/code-bytes/{code_hash}` -> devuelve `{ code_b64 }`
- PUBLICACIÓN `/v2/contracts/instance`
  - Solicitud: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: despliega el bytecode provisto y activa de inmediato el mapeo `(namespace, contract_id)` vía `ActivateContractInstance`.
  - Respuesta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Servicio de alias
- PUBLICACIÓN `/v2/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - Respuesta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` refleja la implementación del evaluador. Valor actual: `blake2b512-mock`.
  - Notas: evaluador simulado determinista que aplica Blake2b512 con separacion de dominio `iroha.alias.voprf.mock.v1`. Diseñado para herramientas de prueba hasta que el pipeline VOPRF de producción este cableado en Iroha.
  - Errores: HTTP `400` en entrada hexadecimal mal formada. Torii devuelve un sobre Norito `ValidationFail::QueryFailed::Conversion` con el mensaje de error del decodificador.
- PUBLICACIÓN `/v2/aliases/resolve`
  - Solicitud: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Respuesta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: requiere la preparación del puente ISO en tiempo de ejecución (`[iso_bridge.account_aliases]` en `iroha_config`). Torii normaliza alias eliminando espacios y pasando a mayusculas antes del lookup. Devuelve 404 cuando el alias no existe y 503 cuando el runtime ISO bridge está deshabilitado.
- PUBLICACIÓN `/v2/aliases/resolve_index`
  - Solicitud: { "index": 0 }
  - Respuesta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }- Notas: los índices de alias se asignan de forma determinista según el orden de configuración (basado en 0). Los clientes pueden cachear respuestas fuera de línea para construir trazas de auditoria de eventos de atestación de alias.

Tope de tamano de codigo
- Parámetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Controla el tamano máximo permitido (en bytes) para almacenamiento de código de contrato on-chain.
  - Predeterminado: 16 MiB. Los nodos rechazan `RegisterSmartContractBytes` cuando la imagen `.to` excede el tope con un error de violación de invariante.
  - Los operadores pueden ajustar enviando `SetParameter(Custom)` con `id = "max_contract_code_bytes"` y un payload numérico.- PUBLICACIÓN `/v2/gov/ballots/zk`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas:
    - Cuando las entradas públicas del circuito incluyen `owner`, `amount` y `duration_blocks`, y la prueba verifica contra la VK configurada, el nodo crea o extiende un bloqueo de gobernanza para `election_id` con ese `owner`. La dirección permanece oculta (`unknown`); solo se actualizan importe/vencimiento. Las revotaciones son monótonas: cantidad y vencimiento solo aumentan (el nodo aplica max(amount, prev.amount) y max(expiry, prev.expiry)).
    - Las revotaciones ZK que intentan reducir la cantidad o caducidad se rechazan del lado del servidor con diagnósticos `BallotRejected`.
    - La ejecución del contrato debe llamar `ZK_VOTE_VERIFY_BALLOT` antes de encolar `SubmitBallot`; los anfitriones imponen un pestillo de una sola vez.- PUBLICACIÓN `/v2/gov/ballots/plain`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sí|No|Abstenerse" }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas: las revotaciones son de solo extensión - una nueva boleta no puede reducir el monto o vencimiento del bloqueo existente. El `owner` debe igualar la autoridad de la transacción. La duración mínima es `conviction_step_blocks`.- PUBLICACIÓN `/v2/gov/finalize`
  - Solicitud: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efecto on-chain (andamiaje actual): promulgar una propuesta de implementación aprobada inserta un `ContractManifest` minimo con clave `code_hash` con el `abi_hash` esperado y marca la propuesta como Enacted. Si ya existe un manifiesto para el `code_hash` con un `abi_hash` distinto, la promulgación se rechaza.
  - Notas:
    - Para elecciones ZK, las rutas del contrato deben llamar `ZK_VOTE_VERIFY_TALLY` antes de ejecutar `FinalizeElection`; los anfitriones imponen un pestillo de una sola vez. `FinalizeReferendum` rechaza referendos ZK hasta que el recuento de la elección este finalizado.
    - El cierre automatico en `h_end` emite Aprobado/Rechazado solo para referendos Plain; los referendos ZK permanecen cerrados hasta que se envíe un tally finalizado y se ejecute `FinalizeReferendum`.
    - Las comprobaciones de participación usan solo aprobar+rechazar; abstenerse no cuenta para el resultado.- PUBLICACIÓN `/v2/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii envía la transacción firmada cuando se proporciona `authority`/`private_key`; de lo contrario devuelve un esqueleto para que los clientes firmen y envien. La preimagen es opcional y actualmente informativa.

- OBTENER `/v2/gov/proposals/{id}`
  - Ruta `{id}`: id de propuesta hexadecimal (64 caracteres)
  - Respuesta: { "encontrado": bool, "propuesta": {... }? }

- OBTENER `/v2/gov/locks/{rid}`
  - Ruta `{rid}`: cadena de identificación del referéndum
  - Respuesta: { "found": bool, "referendum_id": "rid", "locks": {... }? }

- OBTENER `/v2/gov/council/current`
  - Respuesta: { "época": N, "miembros": [{ "account_id": "..." }, ...] }
  - Notas: devuelve el consejo persistido cuando existe; de lo contrario deriva un respaldo determinista usando el activo de participación configurado y umbrales (refleja la especificación VRF hasta que pruebas VRF en vivo se persistan on-chain).- PUBLICACIÓN `/v2/gov/council/derive-vrf` (característica: gov_vrf)
  - Solicitud: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Pequeño", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: verifica la prueba VRF de cada candidato contra el input canonico derivado de `chain_id`, `epoch` y el beacon del ultimo hash de bloque; ordena por bytes de salida desc con desempates; devuelve los top `committee_size` miembros. No persistir.
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk en G1, prueba en G2 (96 bytes). Pequeño = pk en G2, prueba en G1 (48 bytes). Los insumos están separados por dominio e incluyen `chain_id`.

### Valores predeterminados de gobernanza (iroha_config `gov.*`)

El consejo de respaldo usado por Torii cuando no existe una lista persistente se parametriza vía `iroha_config`:

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

Anula los equivalentes del entorno:

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

`parliament_committee_size` limita la cantidad de miembros de respaldo devueltos cuando no hay consejo persistido, `parliament_term_blocks` define la longitud de época usada para derivación de semilla (`epoch = floor(height / term_blocks)`), `parliament_min_stake` aplica el mínimo de participación (en unidades mínimas) sobre el activo de elegibilidad, y `parliament_eligibility_asset_id` selecciona que balance de activos se escanea al construir el conjunto de candidatos.La verificación VK de gobernanza no tiene bypass: la verificación de boletas siempre requiere una clave verificadora `Active` con bytes en línea, y los entornos no deben depender de toggles de prueba para omitir la verificación.

RBAC
- La ejecución on-chain requiere permisos:
  - Propuestas: `CanProposeContractDeployment{ contract_id }`
  - Boletas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgación: `CanEnactGovernance`
  - Dirección del consejo (futuro): `CanManageParliament`Espacios de nombres protegidos
- El parámetro personalizado `gov_protected_namespaces` (matriz de cadenas JSON) habilita la puerta de admisión para implementar listados de espacios de nombres.
- Los clientes deben incluir claves de metadatos de transacción para despliegues dirigidos a espacios de nombres protegidos:
  - `gov_namespace`: el espacio de nombres objetivo (ej., "apps")
  - `gov_contract_id`: el id del contrato lógico dentro del espacio de nombres
- `gov_manifest_approvers`: Matriz JSON opcional de ID de cuenta de validadores. Cuando un manifiesto de carril declara un quórum mayor a uno, la admisión requiere la autoridad de la transacción más las cuentas listadas para satisfacer el quórum del manifiesto.
- La telemetria exponen contadores de admisión vía `governance_manifest_admission_total{result}` para que operadores distingan admite exitosos de rutas `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, y `runtime_hook_rejected`.
- La telemetria expone la ruta de cumplimiento vía `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para que operadores auditen aprobaciones faltantes.
- Los carriles aplican la lista permitida de espacios de nombres publicada en sus manifiestos. Cualquier transacción que fije `gov_namespace` debe proporcionar `gov_contract_id`, y el namespace debe aparecer en el set `protected_namespaces` del manifiesto. Los envíos `RegisterSmartContractCode` sin estos metadatos se rechazan cuando la protección está habilitada.- La admisión impone que existe una propuesta de gobernanza promulgada para el tupla `(namespace, contract_id, code_hash, abi_hash)`; de lo contrario la validación falla con un error NotPermitted.

Ganchos de actualización del tiempo de ejecución
- Los manifiestos de carril pueden declarar `hooks.runtime_upgrade` para controlar instrucciones de actualización de tiempo de ejecución (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos del gancho:
  - `allow` (bool, predeterminado `true`): cuando es `false`, se rechazan todas las instrucciones de actualización de tiempo de ejecución.
  - `require_metadata` (bool, predeterminado `false`): exige la entrada de metadatos especificada por `metadata_key`.
  - `metadata_key` (cadena): nombre de metadatos aplicados por el gancho. Predeterminado `gov_upgrade_id` cuando se requieren metadatos o hay lista de permitidos.
  - `allowed_ids` (array de strings): lista permitida opcional de valores de metadatos (tras trim). Rechaza cuando el valor provisto no está listado.
- Cuando el gancho está presente, admisión de la cola aplica la política de metadatos antes de que la transacción entre a la cola. Metadatos faltantes, valores vacíos o valores fuera de la lista permitida producen un error determinista NotPermitted.
- La telemetria rastrea resultados vía `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Las transacciones que cumplen el gancho deben incluir metadatos `gov_upgrade_id=<value>` (o la clave definida por el manifiesto) junto con cualquier aprobación de validadores requerida por el quórum del manifiesto.Punto final de conveniencia
- POST `/v2/gov/protected-namespaces` - aplica `gov_protected_namespaces` directamente en el nodo.
  - Solicitud: { "espacios de nombres": ["aplicaciones", "sistema"] }
  - Respuesta: { "ok": verdadero, "aplicado": 1 }
  - Notas: pensado para admin/testing; requiere token API si está configurado. Para producción, prefiere enviar una transacción firmada con `SetParameter(Custom)`.CLI de ayuda
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Obtiene instancias de contrato para el namespace y verifica que:
    - Torii almacena bytecode para cada `code_hash`, y su resumen Blake2b-32 coincide con el `code_hash`.
    - El manifiesto almacenado bajo `/v2/contracts/code/{code_hash}` reporta valores `code_hash` y `abi_hash` coinciden.
    - Existe una propuesta de gobernanza promulgada para `(namespace, contract_id, code_hash, abi_hash)` derivada por el mismo hash de propuesta-id que usa el nodo.
  - Emite un informe JSON con `results[]` por contrato (issues, resúmenes de manifest/code/proposal) mas un resumen de una línea salvo que se suprime (`--no-summary`).
  - Util para auditar espacios de nombres protegidos o verificar flujos de implementación controlados por gobernanza.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emite el esqueleto JSON de metadatos usado al enviar despliegues a espacios de nombres protegidos, incluyendo `gov_manifest_approvers` opcionales para satisfacer reglas de quorum del manifiesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — los lock tips son obligatorios cuando `min_bond_amount > 0`, y cualquier conjunto de tips proporcionados debe incluir `owner`, `amount` y `duration_blocks`.
  - Valida los identificadores de cuentas canónicas, canonicaliza las sugerencias de anulación de 32 bytes y combina las sugerencias en `public_inputs_json` (con `--public <path>` para anulaciones adicionales).- El anulador se deriva del compromiso de prueba (entrada pública) más `domain_tag`, `chain_id` e `election_id`; `--nullifier` se valida con la prueba cuando se suministra.
  - El resumen de una línea ahora exponen un `fingerprint=<hex>` determinista derivado del `CastZkBallot` codificado junto con sugerencias decodificados (`owner`, `amount`, `duration_blocks`, `direction` cuando se proporciona).
  - Las respuestas CLI anotan `tx_instructions[]` con `payload_fingerprint_hex` mas campos decodificados para que herramientas downstream verifiquen el esqueleto sin reimplementar decodificacion Norito.
  - Proveer los indicios de bloqueo permite que el nodo emita eventos `LockCreated`/`LockExtended` para boletas ZK una vez que el circuito exponga los mismos valores.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` reflejan los nombres de flags de ZK para paridad en scripts.
  - La salida de resumen refleja `vote --mode zk` al incluir la huella digital de la instrucción codificada y campos de votación legibles (`owner`, `amount`, `duration_blocks`, `direction`), ofreciendo confirmación rápida antes de firmar el esqueleto.Listado de instancias
- GET `/v2/gov/instances/{ns}` - lista de instancias de contrato activadas para un espacio de nombres.
  - Parámetros de consulta:
    - `contains`: filtrado por subcadena de `contract_id` (distingue entre mayúsculas y minúsculas)
    - `hash_prefix`: filtrado por prefijo hex de `code_hash_hex` (minúscula)
    - `offset` (predeterminado 0), `limit` (predeterminado 100, máx. 10_000)
    - `order`: uno de `cid_asc` (predeterminado), `cid_desc`, `hash_asc`, `hash_desc`
  - Respuesta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) o `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Barrido de desbloquea (Operador/Auditoria)
- OBTENER `/v2/gov/unlocks/stats`
  - Respuesta: { "height_current": H, "expired_locks_now": n, "referendo_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` refleja la altura del bloque más reciente donde los locks expirados fueron barridos y persistidos. `expired_locks_now` se calcula escaneando registros de bloqueo con `expiry_height <= height_current`.
- PUBLICACIÓN `/v2/gov/ballots/zk-v1`
  - Solicitud (DTO estilo v1):
    {
      "autoridad": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "sobre_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "propietario": "i105…?",
      "nulificador": "blake2b32:...64hex?"
    }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }- PUBLICACIÓN `/v2/gov/ballots/zk-v1/ballot-proof` (característica: `zk-ballot`)
  - Acepta un JSON `BallotProof` directo y devuelve un esqueleto `CastZkBallot`.
  - Solicitud:
    {
      "autoridad": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "votación": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 de contenedor ZK1 o H2*
        "root_hint": null, // cadena hexadecimal de 32 bytes opcional (raíz de elegibilidad)
        "owner": null, // AccountId opcional cuando el circuito compromete al propietario
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
    - El servidor mapea `root_hint`/`owner`/`nullifier` opcionales desde el ballot a `public_inputs_json` para `CastZkBallot`.
    - Los bytes del sobre se recodifican como base64 para la carga útil de la instrucción.
    - La respuesta `reason` cambia a `submitted transaction` cuando Torii envió el voto.
    - Este punto final solo está disponible cuando la función `zk-ballot` está habilitada.Ruta de verificación de CastZkBallot
- `CastZkBallot` decodifica la prueba base64 provista y rechaza payloads vacios o mal formados (`BallotRejected` con `invalid or empty proof`).
- El host resuelve la clave verificadora del ballot desde el referendum (`vk_ballot`) o defaults de gobernanza y requiere que el registro exista, sea `Active`, y lleve bytes inline.
- Los bytes de la clave verificadora almacenada se re-hashean con `hash_vk`; cualquier desajuste de compromiso aborta la ejecución antes de verificar para proteger contra entradas de registro adulteradas (`BallotRejected` con `verifying key commitment mismatch`).
- Los bytes de la prueba se envían al backend registrado vía `zk::verify_backend`; transcripciones invalidas aparecen como `BallotRejected` con `invalid proof` y la instrucción falla deterministamente.
- La prueba debe exponer un compromiso electoral y una raíz de elegibilidad como aportes públicos; la raíz debe coincidir con el `eligible_root` de la elección y el anulador derivado debe coincidir con cualquier sugerencia proporcionada.
- Pruebas exitosas emiten `BallotAccepted`; nullifiers duplicados, root de elegibilidad viejos o regresiones de lock siguen produciendo las razones de rechazo existentes descritas antes en este documento.

## Mala conducta de validadores y consenso conjunto

### Flujo de corte y encarcelamientoEl consenso emite `Evidence` codificada en Norito cuando un validador viola el protocolo. Cada carga útil llega al `EvidenceStore` en memoria y, si no se vio antes, se materializa en el mapa `consensus_evidence` respaldado por WSV. Los registros anteriores a `sumeragi.npos.reconfig.evidence_horizon_blocks` (predeterminado `7200` bloques) se rechazan para que el archivo permanezca acotado, pero el rechazo se registra para los operadores. La evidencia dentro del horizonte también respeta `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el retraso de corte `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`); la gobernanza puede cancelar las sanciones con `CancelConsensusEvidencePenalty` antes de que se aplique la reducción.

Las ofensas reconocidas se mapean uno a uno a `EvidenceKind`; los discriminantes son estables y están reforzados por el modelo de datos:

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

- **DoublePrepare/DoubleCommit** - el validador firmo hashes en conflicto para la misma tupla `(phase,height,view,epoch)`.
- **InvalidQc** - un agregador gossipeo un certificado de confirmación cuya forma falla chequeos deterministas (ej., mapa de bits de firmantes vacio).
- **InvalidProposal** - un líder propuso un bloque que falla la validación estructural (ej., rompe la regla de lock-chain).
- **Censura**: los recibos de envío firmados muestran una transacción que nunca se propuso ni se comprometió.

Operadores y herramientas pueden inspeccionar y retransmitir cargas útiles a través de:- Torii: `GET /v2/sumeragi/evidence` y `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, y `... submit --evidence-hex <payload>`.

La gobernanza debe tratar los bytes de evidencia como prueba canónica:

1. **Recolectar el payload** antes de que caduque. Archivar los bytes Norito crudos junto con metadata de height/view.
2. **Preparar la penalidad** embebiendo el payload en un referéndum o instrucción sudo (ej., `Unregister::peer`). La ejecucion revalida el payload; evidencia mal formada o rancia se rechaza deterministamente.
3. **Programar la topología de seguimiento** para que el validador infractor no pueda reingresar de inmediato. Flujos típicos encolan `SetParameter(Sumeragi::NextMode)` y `SetParameter(Sumeragi::ModeActivationHeight)` con el roster actualizado.
4. **Auditar resultados** vía `/v2/sumeragi/evidence` y `/v2/sumeragi/status` para asegurar que el contador de evidencia avanza y que la gobernanza aplica la remocion.

### Secuenciación de consenso conjunto

El conjunto de consenso garantiza que el conjunto de validadores salientes finalice el bloque de frontera antes de que el nuevo conjunto comience a proponer. El tiempo de ejecución impone la regla mediante parámetros pareados:- `SumeragiParameter::NextMode` y `SumeragiParameter::ModeActivationHeight` deben confirmarse en el **mismo bloque**. `mode_activation_height` debe ser estrictamente mayor que la altura del bloque que carga el update, proporcionando al menos un bloque de retraso.
- `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) es el protector de configuración que previene las transferencias con retraso cero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`) retrasa la reducción del consenso para que la gobernanza pueda cancelar las sanciones antes de que se apliquen.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Los parámetros de exponente de tiempo de ejecución y CLI organizados a través de `/v2/sumeragi/params` y `iroha --output-format text ops sumeragi params`, para que los operadores confirmen alturas de activación y listas de validadores.
- La automatización de gobernanza siempre debe:
  1. Finalizar la decisión de remoción (o reinstalación) respaldada por evidencia.
  2. Introduzca una reconfiguración de seguimiento con `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorear `/v2/sumeragi/status` hasta que `effective_consensus_mode` cambie a la altura esperada.

Cualquier script que rote validadores o aplique slashing **no debe** intentar activar con lag cero u omitir los parámetros de hand-off; esas transacciones se rechazan y dejan la red en el modo anterior.

## Superficies de telemetría- Las métricas Prometheus exportan actividad de gobernanza:
  - `governance_proposals_status{status}` (gauge) rastrea conteos de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (contador) incrementa cuando la admisión de espacios de nombres protegidos permite o rechaza un despliegue.
  - `governance_manifest_activations_total{event}` (contador) registra inserciones de manifiesto (`event="manifest_inserted"`) y enlaces de espacio de nombres (`event="instance_bound"`).
- `/status` incluye un objeto `governance` que refleja los recuentos de propuestas, reporta totales de espacios de nombres protegidos y lista de activaciones recientes de manifiesto (espacio de nombres, identificación de contrato, código/hash ABI, altura del bloque, marca de tiempo de activación). Los operadores pueden consultar este campo para confirmar que las promulgaciones actualizan manifiestos y que las puertas de espacios de nombres protegidos se aplican.
- Una plantilla Grafana (`docs/source/grafana_governance_constraints.json`) y el runbook de telemetría en `telemetry.md` muestran como cablear alertas para propuestas atascadas, activaciones de manifiestos faltantes, o rechazos inesperados de espacios de nombres protegidos durante las actualizaciones de runtime.