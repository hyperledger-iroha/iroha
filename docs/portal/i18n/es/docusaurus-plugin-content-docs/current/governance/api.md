---
id: api
lang: es
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Estado: borrador/boceto para acompanar las tareas de implementacion de gobernanza. Las formas pueden cambiar durante la implementacion. El determinismo y la politica RBAC son restricciones normativas; Torii puede firmar/enviar transacciones cuando se proporcionan `authority` y `private_key`, de lo contrario los clientes construyen y envian a `/transaction`.

Resumen
- Todos los endpoints devuelven JSON. Para flujos que producen transacciones, las respuestas incluyen `tx_instructions` - un arreglo de una o mas instrucciones esqueleto:
  - `wire_id`: identificador de registro para el tipo de instruccion
  - `payload_hex`: bytes de payload Norito (hex)
- Si se proporcionan `authority` y `private_key` (o `private_key` en DTOs de ballots), Torii firma y envia la transaccion y aun devuelve `tx_instructions`.
- De lo contrario, los clientes arman una SignedTransaction usando su authority y chain_id, luego firman y hacen POST a `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` devuelve `GovernanceProposalResult` (normaliza campos status/kind), `ToriiClient.get_governance_referendum_typed` devuelve `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` devuelve `GovernanceTally`, `ToriiClient.get_governance_locks_typed` devuelve `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` devuelve `GovernanceUnlockStats`, y `ToriiClient.list_governance_instances_typed` devuelve `GovernanceInstancesPage`, imponiendo acceso tipado en toda la superficie de gobernanza con ejemplos de uso en README.
- Cliente ligero Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal` devuelven bundles tipados `GovernanceInstructionDraft` (envolviendo el esqueleto `tx_instructions` de Torii), evitando parseo JSON manual cuando scripts componen flujos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` expone helpers tipados para proposals, referenda, tallies, locks, unlock stats, y ahora `listGovernanceInstances(namespace, options)` mas los endpoints del council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que clientes Node.js puedan paginar `/v1/gov/instances/{ns}` y conducir flujos respaldados por VRF junto con el listado de instancias de contrato existente.

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - Solicitud (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "alice@wonderland?",
      "private_key": "...?"
    }
  - Respuesta (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validacion: los nodos canonizan `abi_hash` para el `abi_version` provisto y rechazan desajustes. Para `abi_version = "v1"`, el valor esperado es `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API de contratos (deploy)
- POST `/v1/contracts/deploy`
  - Solicitud: { "authority": "alice@wonderland", "private_key": "...", "code_b64": "..." }
  - Comportamiento: calcula `code_hash` del cuerpo del programa IVM y `abi_hash` del header `abi_version`, luego envia `RegisterSmartContractCode` (manifiesto) y `RegisterSmartContractBytes` (bytes `.to` completos) en nombre de `authority`.
  - Respuesta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> devuelve el manifiesto almacenado
    - GET `/v1/contracts/code-bytes/{code_hash}` -> devuelve `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Solicitud: { "authority": "alice@wonderland", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: despliega el bytecode provisto y activa de inmediato el mapeo `(namespace, contract_id)` via `ActivateContractInstance`.
  - Respuesta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Servicio de alias
- POST `/v1/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - Respuesta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` refleja la implementacion del evaluador. Valor actual: `blake2b512-mock`.
  - Notas: evaluador mock determinista que aplica Blake2b512 con separacion de dominio `iroha.alias.voprf.mock.v1`. Disenado para tooling de prueba hasta que el pipeline VOPRF de produccion este cableado en Iroha.
  - Errores: HTTP `400` en input hex mal formado. Torii devuelve un envelope Norito `ValidationFail::QueryFailed::Conversion` con el mensaje de error del decoder.
- POST `/v1/aliases/resolve`
  - Solicitud: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Respuesta: { "alias": "GB82WEST12345698765432", "account_id": "...@...", "index": 0, "source": "iso_bridge" }
  - Notas: requiere el runtime ISO bridge staging (`[iso_bridge.account_aliases]` en `iroha_config`). Torii normaliza alias eliminando espacios y pasando a mayusculas antes del lookup. Devuelve 404 cuando el alias no existe y 503 cuando el runtime ISO bridge esta deshabilitado.
- POST `/v1/aliases/resolve_index`
  - Solicitud: { "index": 0 }
  - Respuesta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "...@...", "source": "iso_bridge" }
  - Notas: los indices de alias se asignan de forma determinista segun el orden de configuracion (0-based). Los clientes pueden cachear respuestas offline para construir trazas de auditoria de eventos de atestacion de alias.

Tope de tamano de codigo
- Parametro custom: `max_contract_code_bytes` (JSON u64)
  - Controla el tamano maximo permitido (en bytes) para almacenamiento de codigo de contrato on-chain.
  - Default: 16 MiB. Los nodos rechazan `RegisterSmartContractBytes` cuando la imagen `.to` excede el tope con un error de violacion de invariante.
  - Los operadores pueden ajustar enviando `SetParameter(Custom)` con `id = "max_contract_code_bytes"` y un payload numerico.

- POST `/v1/gov/ballots/zk`
  - Solicitud: { "authority": "alice@wonderland", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Respuesta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas:
    - Cuando las entradas publicas del circuito incluyen `owner`, `amount` y `duration_blocks`, y la prueba verifica contra la VK configurada, el nodo crea o extiende un bloqueo de gobernanza para `election_id` con ese `owner`. La direccion permanece oculta (`unknown`); solo se actualizan amount/expiry. Las revotaciones son monotonic: amount y expiry solo aumentan (el nodo aplica max(amount, prev.amount) y max(expiry, prev.expiry)).
    - Las revotaciones ZK que intenten reducir amount o expiry se rechazan del lado del servidor con diagnosticos `BallotRejected`.
    - La ejecucion del contrato debe llamar `ZK_VOTE_VERIFY_BALLOT` antes de encolar `SubmitBallot`; los hosts imponen un latch de una sola vez.

- POST `/v1/gov/ballots/plain`
  - Solicitud: { "authority": "alice@domain", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "alice@domain", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - Respuesta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas: las revotaciones son de solo extension - un nuevo ballot no puede reducir el amount o expiry del bloqueo existente. El `owner` debe igualar la authority de la transaccion. La duracion minima es `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Solicitud: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "alice@wonderland?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efecto on-chain (andamiaje actual): promulgar una propuesta de deploy aprobada inserta un `ContractManifest` minimo con clave `code_hash` con el `abi_hash` esperado y marca la propuesta como Enacted. Si ya existe un manifiesto para el `code_hash` con un `abi_hash` distinto, la promulgacion se rechaza.
  - Notas:
    - Para elecciones ZK, las rutas del contrato deben llamar `ZK_VOTE_VERIFY_TALLY` antes de ejecutar `FinalizeElection`; los hosts imponen un latch de una sola vez. `FinalizeReferendum` rechaza referendos ZK hasta que el tally de la eleccion este finalizado.
    - El cierre automatico en `h_end` emite Approved/Rejected solo para referendos Plain; los referendos ZK permanecen Closed hasta que se envíe un tally finalizado y se ejecute `FinalizeReferendum`.
    - Las comprobaciones de turnout usan solo approve+reject; abstain no cuenta para el turnout.

- POST `/v1/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "alice@wonderland?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii envia la transaccion firmada cuando se proporcionan `authority`/`private_key`; de lo contrario devuelve un esqueleto para que los clientes firmen y envien. La preimage es opcional y actualmente informativa.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: id de propuesta hex (64 chars)
  - Respuesta: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: string de referendum id
  - Respuesta: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notas: devuelve el council persistido cuando existe; de lo contrario deriva un respaldo determinista usando el asset de stake configurado y umbrales (refleja la especificacion VRF hasta que pruebas VRF en vivo se persistan on-chain).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - Solicitud: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: verifica la prueba VRF de cada candidato contra el input canonico derivado de `chain_id`, `epoch` y el beacon del ultimo hash de bloque; ordena por bytes de salida desc con tiebreakers; devuelve los top `committee_size` miembros. No persiste.
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk en G1, proof en G2 (96 bytes). Small = pk en G2, proof en G1 (48 bytes). Los inputs estan separados por dominio e incluyen `chain_id`.

### Defaults de gobernanza (iroha_config `gov.*`)

El council de respaldo usado por Torii cuando no existe un roster persistido se parametriza via `iroha_config`:

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

Overrides de entorno equivalentes:

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

`parliament_committee_size` limita la cantidad de miembros de respaldo devueltos cuando no hay council persistido, `parliament_term_blocks` define la longitud de epoca usada para derivacion de seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` aplica el minimo de stake (en unidades minimas) sobre el asset de elegibilidad, y `parliament_eligibility_asset_id` selecciona que balance de asset se escanea al construir el conjunto de candidatos.

La verificacion VK de gobernanza no tiene bypass: la verificacion de ballots siempre requiere una clave verificadora `Active` con bytes inline, y los entornos no deben depender de toggles de prueba para omitir verificacion.

RBAC
- La ejecucion on-chain requiere permisos:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (futuro): `CanManageParliament`

Namespaces protegidos
- Parametro custom `gov_protected_namespaces` (JSON array de strings) habilita admission gating para deploys en namespaces listados.
- Los clientes deben incluir claves de metadata de transaccion para deploys dirigidos a namespaces protegidos:
  - `gov_namespace`: el namespace objetivo (ej., "apps")
  - `gov_contract_id`: el contract id logico dentro del namespace
- `gov_manifest_approvers`: JSON array opcional de account IDs de validadores. Cuando un manifiesto de lane declara un quorum mayor a uno, admission requiere la authority de la transaccion mas las cuentas listadas para satisfacer el quorum del manifiesto.
- La telemetria expone contadores de admission via `governance_manifest_admission_total{result}` para que operadores distingan admits exitosos de rutas `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, y `runtime_hook_rejected`.
- La telemetria expone la ruta de enforcement via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para que operadores auditen aprobaciones faltantes.
- Los lanes aplican el allowlist de namespaces publicado en sus manifests. Cualquier transaccion que fije `gov_namespace` debe proporcionar `gov_contract_id`, y el namespace debe aparecer en el set `protected_namespaces` del manifiesto. Los envios `RegisterSmartContractCode` sin esta metadata se rechazan cuando la proteccion esta habilitada.
- La admission impone que exista una propuesta de gobernanza Enacted para el tuple `(namespace, contract_id, code_hash, abi_hash)`; de lo contrario la validacion falla con un error NotPermitted.

Hooks de runtime upgrade
- Los manifests de lane pueden declarar `hooks.runtime_upgrade` para controlar instrucciones de runtime upgrade (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos del hook:
  - `allow` (bool, default `true`): cuando es `false`, se rechazan todas las instrucciones de runtime upgrade.
  - `require_metadata` (bool, default `false`): exige la entrada de metadata especificada por `metadata_key`.
  - `metadata_key` (string): nombre de metadata aplicado por el hook. Default `gov_upgrade_id` cuando se requiere metadata o hay allowlist.
  - `allowed_ids` (array de strings): allowlist opcional de valores de metadata (tras trim). Rechaza cuando el valor provisto no esta listado.
- Cuando el hook esta presente, admission de la cola aplica la politica de metadata antes de que la transaccion entre a la cola. Metadata faltante, valores vacios o valores fuera del allowlist producen un error NotPermitted determinista.
- La telemetria rastrea resultados via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Las transacciones que cumplen el hook deben incluir metadata `gov_upgrade_id=<value>` (o la clave definida por el manifiesto) junto con cualquier aprobacion de validadores requerida por el quorum del manifiesto.

Endpoint de conveniencia
- POST `/v1/gov/protected-namespaces` - aplica `gov_protected_namespaces` directamente en el nodo.
  - Solicitud: { "namespaces": ["apps", "system"] }
  - Respuesta: { "ok": true, "applied": 1 }
  - Notas: pensado para admin/testing; requiere token API si esta configurado. Para produccion, prefiera enviar una transaccion firmada con `SetParameter(Custom)`.

Helpers CLI
- `iroha gov audit-deploy --namespace apps [--contains calc --hash-prefix deadbeef --summary-only]`
  - Obtiene instancias de contrato para el namespace y verifica que:
    - Torii almacena bytecode para cada `code_hash`, y su digest Blake2b-32 coincide con el `code_hash`.
    - El manifiesto almacenado bajo `/v1/contracts/code/{code_hash}` reporta valores `code_hash` y `abi_hash` coincidentes.
    - Existe una propuesta de gobernanza enacted para `(namespace, contract_id, code_hash, abi_hash)` derivada por el mismo hashing de proposal-id que usa el nodo.
  - Emite un reporte JSON con `results[]` por contrato (issues, resumenes de manifest/code/proposal) mas un resumen de una linea salvo que se suprima (`--no-summary`).
  - Util para auditar namespaces protegidos o verificar flujos de deploy controlados por gobernanza.
- `iroha gov deploy-meta --namespace apps --contract-id calc.v1 [--approver validator@wonderland --approver bob@wonderland]`
  - Emite el esqueleto JSON de metadata usado al enviar deployments a namespaces protegidos, incluyendo `gov_manifest_approvers` opcionales para satisfacer reglas de quorum del manifiesto.
- `iroha gov vote-zk --election-id <id> --proof-b64 <b64> [--owner <account>@<domain> --nullifier-hex <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — los lock hints son obligatorios cuando `min_bond_amount > 0`, y cualquier conjunto de hints proporcionado debe incluir `owner`, `amount` y `duration_blocks`.
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier-hex` is validated against the proof when supplied.
  - El resumen de una linea ahora expone un `fingerprint=<hex>` determinista derivado del `CastZkBallot` codificado junto con hints decodificados (`owner`, `amount`, `duration_blocks`, `direction` cuando se proporcionan).
  - Las respuestas CLI anotan `tx_instructions[]` con `payload_fingerprint_hex` mas campos decodificados para que herramientas downstream verifiquen el esqueleto sin reimplementar decodificacion Norito.
  - Proveer los hints de bloqueo permite que el nodo emita eventos `LockCreated`/`LockExtended` para ballots ZK una vez que el circuito exponga los mismos valores.
- `iroha gov vote-plain --referendum-id <id> --owner <account>@<domain> --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` reflejan los nombres de flags de ZK para paridad en scripts.
  - La salida de resumen refleja `vote-zk` al incluir el fingerprint de la instruccion codificada y campos de ballot legibles (`owner`, `amount`, `duration_blocks`, `direction`), ofreciendo confirmacion rapida antes de firmar el esqueleto.

Listado de instancias
- GET `/v1/gov/instances/{ns}` - lista instancias de contrato activas para un namespace.
  - Query params:
    - `contains`: filtra por substring de `contract_id` (case-sensitive)
    - `hash_prefix`: filtra por prefijo hex de `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: uno de `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - Respuesta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) o `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Barrido de unlocks (Operador/Auditoria)
- GET `/v1/gov/unlocks/stats`
  - Respuesta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` refleja el block height mas reciente donde los locks expirados fueron barridos y persistidos. `expired_locks_now` se calcula escaneando registros de lock con `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - Solicitud (DTO estilo v1):
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint_hex": "...64hex?",
      "owner": "alice@wonderland?",
      "nullifier_hex": "...64hex?"
    }
  - Respuesta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - Acepta un JSON `BallotProof` directo y devuelve un esqueleto `CastZkBallot`.
  - Solicitud:
    {
      "authority": "alice@wonderland",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 de contenedor ZK1 o H2*
        "root_hint": null,                // optional 32-byte array of bytes (eligibility root)
        "owner": null,                    // AccountId opcional cuando el circuito compromete owner
        "nullifier": null                 // optional 32-byte array of bytes (nullifier hint)
      }
    }
  - Respuesta:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notas:
    - El servidor mapea `root_hint`/`owner`/`nullifier` opcionales desde el ballot a `public_inputs_json` para `CastZkBallot`.
    - Los bytes del envelope se re-encodan como base64 para el payload de la instruccion.
    - La respuesta `reason` cambia a `submitted transaction` cuando Torii envia el ballot.
    - Este endpoint solo esta disponible cuando el feature `zk-ballot` esta habilitado.

Ruta de verificacion de CastZkBallot
- `CastZkBallot` decodifica la prueba base64 provista y rechaza payloads vacios o mal formados (`BallotRejected` con `invalid or empty proof`).
- El host resuelve la clave verificadora del ballot desde el referendum (`vk_ballot`) o defaults de gobernanza y requiere que el registro exista, sea `Active`, y lleve bytes inline.
- Los bytes de la clave verificadora almacenada se re-hashean con `hash_vk`; cualquier desajuste de compromiso aborta la ejecucion antes de verificar para proteger contra entradas de registro adulteradas (`BallotRejected` con `verifying key commitment mismatch`).
- Los bytes de la prueba se despachan al backend registrado via `zk::verify_backend`; transcripciones invalidas aparecen como `BallotRejected` con `invalid proof` y la instruccion falla deterministamente.
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- Pruebas exitosas emiten `BallotAccepted`; nullifiers duplicados, roots de elegibilidad viejos o regresiones de lock siguen produciendo las razones de rechazo existentes descritas antes en este documento.

## Mala conducta de validadores y consenso conjunto

### Flujo de slashing y jailing

El consenso emite `Evidence` codificada en Norito cuando un validador viola el protocolo. Cada payload llega al `EvidenceStore` en memoria y, si no se vio antes, se materializa en el mapa `consensus_evidence` respaldado por WSV. Los registros anteriores a `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` bloques) se rechazan para que el archivo permanezca acotado, pero el rechazo se registra para operadores.

Las ofensas reconocidas se mapean uno a uno a `EvidenceKind`; los discriminantes son estables y estan reforzados por el modelo de datos:

```rust
use iroha_data_model::block::consensus::EvidenceKind;

let offences = [
    EvidenceKind::DoublePrepare,
    EvidenceKind::DoubleCommit,
    EvidenceKind::InvalidCommitCertificate,
    EvidenceKind::InvalidProposal,
    EvidenceKind::DoubleExecVote,
];

for (expected, kind) in offences.iter().enumerate() {
    assert_eq!(*kind as u16, expected as u16);
}
```

- **DoublePrepare/DoubleCommit** - el validador firmo hashes en conflicto para el mismo tuple `(phase,height,view,epoch)`.
- **DoubleExecVote** - votos de ejecucion en conflicto anuncian roots de estado post distintos.
- **InvalidCommitCertificate** - un agregador gossipeo un commit certificate cuya forma falla chequeos deterministas (ej., bitmap de firmantes vacio).
- **InvalidProposal** - un lider propuso un bloque que falla la validacion estructural (ej., rompe la regla de locked-chain).

Operadores y tooling pueden inspeccionar y re-broadcast payloads a traves de:

- Torii: `GET /v1/sumeragi/evidence` y `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha sumeragi evidence list`, `... count`, y `... submit --evidence-hex <payload>`.

La gobernanza debe tratar los bytes de evidence como prueba canonica:

1. **Recolectar el payload** antes de que caduque. Archivar los bytes Norito crudos junto con metadata de height/view.
2. **Preparar la penalidad** embebiendo el payload en un referendum o instruccion sudo (ej., `Unregister::peer`). La ejecucion re-valida el payload; evidence mal formada o rancia se rechaza deterministamente.
3. **Programar la topologia de seguimiento** para que el validador infractor no pueda reingresar de inmediato. Flujos tipicos encolan `SetParameter(Sumeragi::NextMode)` y `SetParameter(Sumeragi::ModeActivationHeight)` con el roster actualizado.
4. **Auditar resultados** via `/v1/sumeragi/evidence` y `/v1/sumeragi/status` para asegurar que el contador de evidence avanzo y que la gobernanza aplico la remocion.

### Secuenciacion de consenso conjunto

El consenso conjunto garantiza que el conjunto de validadores saliente finalice el bloque de frontera antes de que el nuevo conjunto empiece a proponer. El runtime impone la regla via parametros pareados:

- `SumeragiParameter::NextMode` y `SumeragiParameter::ModeActivationHeight` deben confirmarse en el **mismo bloque**. `mode_activation_height` debe ser estrictamente mayor que la altura del bloque que cargo el update, proporcionando al menos un bloque de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) es el guard de configuracion que previene hand-offs con lag cero:

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- El runtime y la CLI exponen parametros staged via `/v1/sumeragi/params` y `iroha sumeragi params --summary`, para que operadores confirmen alturas de activacion y rosters de validadores.
- La automatizacion de gobernanza siempre debe:
  1. Finalizar la decision de remocion (o reinstalacion) respaldada por evidence.
  2. Encolar una reconfiguracion de seguimiento con `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorear `/v1/sumeragi/status` hasta que `effective_consensus_mode` cambie a la altura esperada.

Cualquier script que rote validadores o aplique slashing **no debe** intentar activacion con lag cero u omitir los parametros de hand-off; esas transacciones se rechazan y dejan la red en el modo previo.

## Superficies de telemetria

- Las metricas Prometheus exportan actividad de gobernanza:
  - `governance_proposals_status{status}` (gauge) rastrea conteos de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (counter) incrementa cuando admission de namespaces protegidos permite o rechaza un deploy.
  - `governance_manifest_activations_total{event}` (counter) registra inserciones de manifest (`event="manifest_inserted"`) y bindings de namespace (`event="instance_bound"`).
- `/status` incluye un objeto `governance` que refleja los conteos de propuestas, reporta totales de namespaces protegidos y lista activaciones recientes de manifest (namespace, contract id, code/ABI hash, block height, activation timestamp). Los operadores pueden consultar este campo para confirmar que las promulgaciones actualizaron manifests y que los gates de namespaces protegidos se aplican.
- Una plantilla Grafana (`docs/source/grafana_governance_constraints.json`) y el runbook de telemetria en `telemetry.md` muestran como cablear alertas para propuestas atascadas, activaciones de manifest faltantes, o rechazos inesperados de namespaces protegidos durante upgrades de runtime.
