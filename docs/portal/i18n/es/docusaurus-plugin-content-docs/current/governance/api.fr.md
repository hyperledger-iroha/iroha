---
lang: es
direction: ltr
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Estatuto: brouillon/esquisse pour acompagner les taches d'implementation de la gouvernance. Las formas pueden cambiar durante la implementación. El determinismo y la política del RBAC son normas contraintes; Torii puede firmar/soumetre las transacciones cuando `authority` y `private_key` son proporcionados, sinon los clientes construyen y soumetten a `/transaction`.Apercú
- Todos los puntos finales envían JSON. Para el flujo que producen las transacciones, las respuestas incluyen `tx_instructions` - un cuadro de una o más instrucciones:
  - `wire_id`: identificador de registro para el tipo de instrucción
  - `payload_hex`: bytes de carga útil Norito (hexadecimal)
- Si `authority` y `private_key` son proporcionados (o `private_key` en el DTO de las boletas), Torii firma y recibe la transacción y el envío cuando meme `tx_instructions`.
- Sinon, los clientes ensamblan un SignedTransaction con su autoridad y chain_id, luego firman y POST versiones `/transaction`.
- SDK de cobertura:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` envío `GovernanceProposalResult` (normaliza el estado/tipo de los campeones), `ToriiClient.get_governance_referendum_typed` envío `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` envío `GovernanceTally`, `ToriiClient.get_governance_locks_typed` envío `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` envío `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` envío `GovernanceInstancesPage`, imposant un tipo de acceso sobre todos la superficie de gobierno con ejemplos de uso en el archivo README.
- Cliente Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` que envían los paquetes tipos `GovernanceInstructionDraft` (que encapsulan el esqueleto `tx_instructions` de Torii), evitan el análisis de JSON manuel quand les scripts composent des flux Finalize/Enact.- JavaScript (`@iroha/iroha-js`): `ToriiClient` expone los tipos de ayuda para propuestas, referendos, recuentos, bloqueos, estadísticas de desbloqueo y mantiene `listGovernanceInstances(namespace, options)` más el consejo de puntos finales (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que los clientes Node.js puedan paginar `/v1/gov/instances/{ns}` y pilotear los flujos de trabajo VRF en paralelo al listado de instancias de contratos existentes.

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
      "autoridad": "i105…?",
      "clave_privada": "...?"
    }
  - Respuesta (JSON):
    { "ok": verdadero, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validación: les noeuds canonisent `abi_hash` pour l'`abi_version` fourni et rejettent les incoherences. Para `abi_version = "v1"`, el valor presente es `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.Contratos de API (implementación)
- PUBLICACIÓN `/v1/contracts/deploy`
  - Requete: { "autoridad": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamiento: calcule `code_hash` después del cuerpo del programa IVM et `abi_hash` después del ente `abi_version`, después de `RegisterSmartContractCode` (manifestado) et `RegisterSmartContractBytes` (bytes `.to` completos) para `authority`.
  - Respuesta: { "ok": verdadero, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Mentira:
    - GET `/v1/contracts/code/{code_hash}` -> enviar el manifiesto en stock
    - OBTENER `/v1/contracts/code-bytes/{code_hash}` -> enviar `{ code_b64 }`
- PUBLICACIÓN `/v1/contracts/instance`
  - Solicitud: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: implemente el bytecode fourni y active inmediatamente el mapeo `(namespace, contract_id)` a través de `ActivateContractInstance`.
  - Respuesta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Servicio de alias
- PUBLICACIÓN `/v1/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - Respuesta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` refleja la implementación del evaluador. Valor actual: `blake2b512-mock`.
  - Notas: evaluador simulado deterministe qui applique Blake2b512 con separación de dominio `iroha.alias.voprf.mock.v1`. Prevus pour l'outillage de test justqu'a ce que le pipeline VOPRF de producción soit depende de Iroha.
  - Errores: HTTP `400` en formato hexadecimal incorrecto. Torii envíe un sobre Norito `ValidationFail::QueryFailed::Conversion` con el mensaje de error del decodificador.
- PUBLICACIÓN `/v1/aliases/resolve`
  - Solicitud: { "alias": "GB82 OESTE 1234 5698 7654 32" }
  - Respuesta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: requiere la preparación del puente ISO en tiempo de ejecución (`[iso_bridge.account_aliases]` y `iroha_config`). Torii normaliza los alias retirando espacios y mettantes en mayúsculas antes de la búsqueda. Retorne 404 si el alias está ausente y 503 si el puente ISO en tiempo de ejecución está desactivado.
- PUBLICACIÓN `/v1/aliases/resolve_index`
  - Requete: {"índice": 0}
  - Respuesta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }- Notas: los índices de alias son asignados determinísticamente según el orden de configuración (basado en 0). Los clientes pueden colocar caché fuera de línea para construir pistas de auditoría para eventos de certificación de alias.

Tapa de cola de código
- Parámetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Controle la talla máxima autorizada (en bytes) para el almacenamiento de código de contrato en cadena.
  - Predeterminado: 16 MiB. Les noeuds rejettent `RegisterSmartContractBytes` lorsque la taille de l'image `.to` depasse le cap cone une error d'invariant.
  - Los operadores pueden ajustar mediante `SetParameter(Custom)` con `id = "max_contract_code_bytes"` y una carga útil numérica.- PUBLICACIÓN `/v1/gov/ballots/zk`
  - Requete: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas:
    - Cuando las entradas públicas del circuito incluyen `owner`, `amount` y `duration_blocks`, y que antes de verificar con la configuración de VK, el nodo cree o tiende un error de gobierno para `election_id` con esto `owner`. La dirección reste cachee (`unknown`); El monto/vencimiento del mismo no es un día. Las re-votaciones son monótonas: cantidad y vencimiento no fuente que aumenta (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Las nuevas votaciones ZK que intentan reducir el importe o la caducidad son rechazadas en el servicio de diagnóstico `BallotRejected`.
    - La ejecución del contrato debe apelar `ZK_VOTE_VERIFY_BALLOT` antes de presentar `SubmitBallot`; Los hosts imponen un pestillo a una sola vez.- PUBLICACIÓN `/v1/gov/ballots/plain`
  - Requete: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sí|No|Abstenerse" }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas: les re-votes sont en extension seule - una nueva votación no puede reducir el monto o el vencimiento del verrou existente. El `owner` tiene la misma autoridad de transacción. La duración mínima es `conviction_step_blocks`.- PUBLICACIÓN `/v1/gov/finalize`
  - Requete: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efecto en cadena (scaffold actuel): promulgar una proposición de implementación aprobada insertando un `ContractManifest` mínimo cle `code_hash` con el `abi_hash` asistente y marca la proposición promulgada. Si un manifiesto existe deja para le `code_hash` con un `abi_hash` diferente, la promulgación está rechazada.
  - Notas:
    - Para las elecciones ZK, los chemins de contrat doivent appeler `ZK_VOTE_VERIFY_TALLY` antes del ejecutor `FinalizeElection`; Los hosts imponen un bloqueo de uso único. `FinalizeReferendum` Rejette les referendums ZK tant que le tally n'est pas finalise.
    - La cloture automatique a `h_end` emet Aprobado/Rechazado únicamente para los referéndums Plain; les referendums ZK restent Closed jusqu'a ce qu'un tally finalize soit soumis et que `FinalizeReferendum` soit run.
    - Las verificaciones de participación se utilizan solo para aprobar+rechazar; abstenerse ne compte pas pour le participación.- PUBLICACIÓN `/v1/gov/enact`
  - Requete: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii soumet la transacción firmante quand `authority`/`private_key` sont fournis; Sinon il renvoie un squelette pour firm et soumission client. La preimagen es opcional e informativa para el instante.

- OBTENER `/v1/gov/proposals/{id}`
  - Ruta `{id}`: id de proposición hexadecimal (64 caracteres)
  - Respuesta: {"encontrado": bool, "propuesta": {...}? }

- OBTENER `/v1/gov/locks/{rid}`
  - Ruta `{rid}`: ID de cadena de referéndum
  - Respuesta: { "found": bool, "referendum_id": "rid", "locks": {...}? }

- OBTENER `/v1/gov/council/current`
  - Respuesta: { "época": N, "miembros": [{ "account_id": "..." }, ...] }
  - Notas: renvoie le Council persiste si presente; Sinon deriva un respaldo determinista con el activo de participación configurado y los seguros (miroir de la spec VRF justqu'a ce que des preuves VRF en direct soient persistees on-chain).- PUBLICACIÓN `/v1/gov/council/derive-vrf` (característica: gov_vrf)
  - Requete: {"committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Pequeño", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: verifique la preuve VRF de cada candidato contre la entrada canónica derivada de `chain_id`, `epoch` y la baliza del último hash de bloque; intente por bytes de sortie desc con desempates; Envíe a los principales miembros `committee_size`. Ne persiste pas.
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk en G1, prueba en G2 (96 bytes). Pequeño = pk en G2, prueba en G1 (48 bytes). Las entradas están separadas por dominio e incluyen `chain_id`.

### Valores predeterminados de gobierno (iroha_config `gov.*`)

El consejo alternativo utiliza el par Torii cuando la lista persiste y no existe este parámetro a través de `iroha_config`:

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

Anula los equivalentes ambientales:

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

`parliament_committee_size` limita el nombre de los miembros fallback renvoyes cuando el consejo no persiste, `parliament_term_blocks` define la duración de la época utilizada para la derivación de semillas (`epoch = floor(height / term_blocks)`), `parliament_min_stake` impone el mínimo de participación (en unidades mínimas) en l'asset d'eligibilite, et `parliament_eligibility_asset_id` selecciónne quel solde d'asset est scanne lors de la construcción du set de candidats.La verificación VK de gobierno no pasa por alto: la verificación de la boleta requiere siempre un bloque `Active` con bytes en línea, y los entornos no deben tocarse en los interruptores de prueba para saltear la verificación.

RBAC
- La ejecución en cadena requiere permisos:
  - Propuestas: `CanProposeContractDeployment{ contract_id }`
  - Boletas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgación: `CanEnactGovernance`
  - Dirección del consejo (futur): `CanManageParliament`Protegidos de espacios de nombres
- El parámetro personalizado `gov_protected_namespaces` (matriz de cadenas JSON) activa la puerta de admisión para implementar en las listas de espacios de nombres.
- Los clientes deben incluir los metadatos de transacciones para implementar frente a los espacios de nombres protegidos:
  - `gov_namespace`: el espacio de nombres disponible (por ejemplo, "aplicaciones")
  - `gov_contract_id`: ID de contrato logístico en el espacio de nombres
- `gov_manifest_approvers`: matriz JSON opcional de ID de cuenta de validadores. Cuando un manifiesto de carril declara un quórum > 1, la admisión requiere la autoridad de la transacción más las listas de cuentas para satisfacer el quórum del manifiesto.
- La telemetrie exponen des compteurs d'admission via `governance_manifest_admission_total{result}` afin que les operatorurs distinguint les admite reussis des chemins `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- La telemetría expone el camino de cumplimiento a través de `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para auditar las aprobaciones manquantes.
- Los carriles aplican la lista de espacios de nombres permitidos publicados en sus manifiestos. Toda transacción que solucione `gov_namespace` debe incluirse en `gov_contract_id`, y el espacio de nombres debe aparecer en el conjunto `protected_namespaces` del manifiesto. Las transmisiones `RegisterSmartContractCode` sin estos metadatos se rechazan cuando la protección está activa.- L'admission imponer qu'une proposition de gouvernance Enacted existe pour le tuple `(namespace, contract_id, code_hash, abi_hash)`; Sinon la validación se hace eco con un error NotPermitted.

Ganchos de actualización del tiempo de ejecución
- Los manifiestos de carril pueden declarar `hooks.runtime_upgrade` para acceder a las instrucciones de actualización del tiempo de ejecución (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos del gancho:
  - `allow` (bool, predeterminado `true`): cuando `false`, todas las instrucciones de actualización del tiempo de ejecución son rechazadas.
  - `require_metadata` (bool, predeterminado `false`): exige la entrada de metadatos especificada por `metadata_key`.
  - `metadata_key` (cadena): nombre de metadatos aplicado por el gancho. Predeterminado `gov_upgrade_id` cuando los metadatos son necesarios o qué lista de permitidos está presente.
  - `allowed_ids` (matriz de cadenas): metadatos de opciones de valores de lista permitida (después del recorte). Rejette quand la valeur fournie n'est pas listee.
- Cuando el gancho está presente, la admisión del archivo aplica los metadatos políticos antes de la entrada de la transacción en el archivo. Los metadatos almacenados, valores de videos o listas de permitidos producen un error determinante NotPermitted.
- La telemetría rastrea los resultados a través de `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Las transacciones que satisfacen el gancho deben incluir los metadatos `gov_upgrade_id=<value>` (o la clave definida por el manifiesto) y las aprobaciones de validadores requeridas por el quorum del manifiesto.Punto final de mercancía
- POST `/v1/gov/protected-namespaces` - aplique `gov_protected_namespaces` directement sur le noeud.
  - Requete: { "espacios de nombres": ["aplicaciones", "sistema"] }
  - Respuesta: { "ok": verdadero, "aplicado": 1 }
  - Notas: destinado a l'admin/testing; Requiere una API de token si se configura. Para la producción, prefiera un firmante de transacción con `SetParameter(Custom)`.CLI de ayuda
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Recupere las instancias de contrato para el espacio de nombres y verifique que:
    - Torii almacena el código de bytes para cada `code_hash`, y su resumen Blake2b-32 corresponde a `code_hash`.
    - Le manifeste stocke sous `/v1/contracts/code/{code_hash}` rapporte des valeurs `code_hash` et `abi_hash` corresponsales.
    - Existe una propuesta de gobierno promulgada para `(namespace, contract_id, code_hash, abi_hash)` derivada del meme hash de propuesta-id que el noeud utiliza.
  - Ordenar una relación JSON con `results[]` por contrato (problemas, currículums de manifiesto/código/propuesta) más un currículum en una línea sauf supresión (`--no-summary`).
  - Utilidad para auditar los espacios de nombres protegidos o verificar los flujos de trabajo de implementación de controles de gobernanza.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emet el esqueleto JSON de metadatos utiliza las implementaciones en los espacios de nombres protegidos, incluidas las opciones `gov_manifest_approvers` para satisfacer las reglas de quórum del manifiesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`: las sugerencias de bloqueo son necesarias después de `min_bond_amount > 0`, y todo el conjunto de sugerencias proporcionadas debe incluir `owner`, `amount` e `duration_blocks`.
  - Valida los identificadores de cuentas canónicas, canonicaliza las sugerencias de anulación de 32 bytes y combina las sugerencias en `public_inputs_json` (con `--public <path>` para anulaciones adicionales).- El anulador se deriva del compromiso de prueba (entrada pública) más `domain_tag`, `chain_id` e `election_id`; `--nullifier` se valida con la prueba cuando se suministra.
  - El currículum en una línea expone el mantenimiento de un `fingerprint=<hex>` que determina la codificación `CastZkBallot` además de las sugerencias decodificadas (`owner`, `amount`, `duration_blocks`, `direction` si fournis).
  - Las respuestas de la CLI indican `tx_instructions[]` con `payload_fingerprint_hex` además de los campeones decodificadores para que las herramientas posteriores verifiquen el esqueleto sin reimplementar la decodificación Norito.
  - Fournir les tips de lock permet au noeud d'emettre des events `LockCreated`/`LockExtended` pour les ballots ZK una vez que le circuito expone los memes valeurs.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` reflejan los nombres de banderas ZK para la partición de secuencias de comandos.
  - La salida del currículum refl ete `vote --mode zk` e incluye la huella digital de la instrucción codificada y las boletas de votación libres (`owner`, `amount`, `duration_blocks`, `direction`), para una confirmación rápida antes de la firma del esqueleto.Listado de instancias
- GET `/v1/gov/instances/{ns}`: lista las instancias de contrato activas para un espacio de nombres.
  - Parámetros de consulta:
    - `contains`: filtro par sous-chaine de `contract_id` (distingue entre mayúsculas y minúsculas)
    - `hash_prefix`: filtro por prefijo hexadecimal de `code_hash_hex` (minúsculas)
    - `offset` (predeterminado 0), `limit` (predeterminado 100, máx. 10_000)
    - `order`: un des `cid_asc` (predeterminado), `cid_desc`, `hash_asc`, `hash_desc`
  - Respuesta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) o `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Balayage d'unlocks (Operador/Auditoría)
- OBTENER `/v1/gov/unlocks/stats`
  - Respuesta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` refleja la hauteur de bloc la plus Recente ou les locks expires ont ete balayes et persistes. `expired_locks_now` está calculando y explorando los registros de bloqueo con `expiry_height <= height_current`.
- PUBLICACIÓN `/v1/gov/ballots/zk-v1`
  - Requete (estilo DTO v1):
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
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }- PUBLICACIÓN `/v1/gov/ballots/zk-v1/ballot-proof` (característica: `zk-ballot`)
  - Acepte un JSON `BallotProof` directamente y envíe un esqueleto `CastZkBallot`.
  - Solicitar:
    {
      "autoridad": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "votación": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 del contenido ZK1 o H2*
        "root_hint": null, // cadena hexadecimal de 32 bytes opcional (raíz de elegibilidad)
        "propietario": nulo, // Opción AccountId si el circuito confirma el propietario
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
    - El servidor de mapas `root_hint`/`owner`/`nullifier` tiene opciones de votación frente a `public_inputs_json` para `CastZkBallot`.
    - Los bytes del sobre se vuelven a codificar en base64 para la carga útil de la instrucción.
    - La respuesta `reason` pasó a `submitted transaction` cuando Torii soumet le ballot.
    - Este punto final está disponible únicamente si la función `zk-ballot` está activa.Rutas de verificación CastZkBallot
- `CastZkBallot` decodifica la fuente base64 anterior y rechaza las cargas útiles de videos o formas incorrectas (`BallotRejected` con `invalid or empty proof`).
- El host resulta en la clave de verificación de la boleta después del referéndum (`vk_ballot`) o los valores predeterminados de gobierno y exige que exista el registro, por lo que `Active` y transporte de bytes en línea.
- Los bytes de las existencias de verificación son rehashes con `hash_vk`; Todo desajuste de compromiso de ejecución previa a la verificación para proteger las entradas corrompidas del registro (`BallotRejected` con `verifying key commitment mismatch`).
- Los bytes anteriores se envían al registro backend a través de `zk::verify_backend`; les transcriptions invalides remontent en `BallotRejected` avec `invalid proof` et l'instruction echoue deterministiquement.
- La prueba debe exponer un compromiso electoral y una raíz de elegibilidad como aportes públicos; la raíz debe coincidir con el `eligible_root` de la elección y el anulador derivado debe coincidir con cualquier sugerencia proporcionada.
- Les preuves reussies emettent `BallotAccepted`; anuladores duplicados, raíces de elegibilidad perimes o regresiones de bloqueo continuadas de producir las razones de rechazo existentes decrites más alto en este documento.

## Mauvaise conducto de validadores y conjunto de consenso

### Flujo de trabajo de corte y encarcelamientoEl consenso emet `Evidence` codifica en Norito cuando se valida el protocolo. Cada carga útil llega en `EvidenceStore` en memoria y, si no se edita, se materializa en el mapa `consensus_evidence` adossee en WSV. Los registros más antiguos que `sumeragi.npos.reconfig.evidence_horizon_blocks` (bloques `7200` predeterminados) son rechazados para guardar el archivo guardado, pero el rechazo está registrado para los operadores. La evidencia dentro del horizonte también respeta `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el retraso de corte `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`); la gobernanza puede cancelar las sanciones con `CancelConsensusEvidencePenalty` antes de que se aplique la reducción.

Les crime reconnues se mappent un-a-un sur `EvidenceKind`; Los discriminantes son estables e imponen según el modelo de datos:

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

- **DoublePrepare/DoubleCommit**: la validación de la firma de hashes en conflicto para la tupla meme `(phase,height,view,epoch)`.
- **InvalidQc** - un agregador de chismes y un certificado de confirmación no se hace eco de las verificaciones determinantes (por ejemplo, mapas de bits de signataires vide).
- **InvalidProposal**: un líder propone un bloque que se hace eco de la estructura de validación (por ejemplo, viola la regla de cadena cerrada).
- **Censura**: los recibos de envío firmados muestran una transacción que nunca se propuso ni se comprometió.

Los operadores y servicios pueden inspeccionar y retransmitir las cargas útiles a través de:- Torii: `GET /v1/sumeragi/evidence` y `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count` y `... submit --evidence-hex <payload>`.

La gobernancia debe traicionar los bytes de evidencia como preuve canonique:

1. **Recoge la carga útil** antes de que caduque. Archiva los bytes Norito de forma bruta con la altura/vista de metadatos.
2. **Preparar la penalización** para embarcar la carga útil en un referéndum o una instrucción sudo (ej., `Unregister::peer`). La ejecución vuelve a validar la carga útil; La evidencia mal forma o rancia está rechazada por el determinismo.
3. **Planificador de topología de seguimiento** para que el validador seleccionado no pueda recuperarse inmediatamente. Les flux typiques incluyen `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` avec le roster mis a jour.
4. **Audite los resultados** vía `/v1/sumeragi/evidence` e `/v1/sumeragi/status` para confirmar que le compteur d'evidence a avance et que la gouvernance a applique le retrait.

### Secuencia del consenso conjunto

El consenso conjunto garantiza que el conjunto de validadores finalizará el bloque de fronteras antes de que el nuevo conjunto comience a proponer. El tiempo de ejecución impone la regla mediante los parámetros que aparecen:- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` deben cometerse en el **bloque de memes**. `mode_activation_height` doit étre estricto superieur a la hauteur du block qui a porte la mise a jour, donnant au moins un block de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) es la garde de configuración que inicia las transferencias con retraso cero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`) retrasa la reducción del consenso para que la gobernanza pueda cancelar las sanciones antes de que se apliquen.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- El tiempo de ejecución y la CLI exponen los parámetros organizados a través de `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, para que los operadores confirmen los botones de activación y las listas de validadores.
- La automatización de la gobernanza de siempre:
  1. Finalizar la decisión de retiro (ou reintegración) sustentada por prueba.
  2. Realice una reconfiguración posterior con `mode_activation_height = h_current + activation_lag_blocks`.
  3. Surveiller `/v1/sumeragi/status` jusqu'a ce que `effective_consensus_mode` bascule a la hauteur escort.

Todos los scripts que hacen girar los validadores o aplicar una barra **ne doit pass** tienen que activar un retardo cero o medir los parámetros de transferencia; Estas transacciones son rechazadas y liberadas del resultado en el modo precedente.

## Superficies de telemetría- Las métricas Prometheus exportan la actividad de gobierno:
  - `governance_proposals_status{status}` (calibre) se adapta a los contadores de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (contador) aumenta cuando la admisión de espacios de nombres protegidos acepta o rechaza una implementación.
  - `governance_manifest_activations_total{event}` (contador) registra las inserciones de manifiesto (`event="manifest_inserted"`) y los enlaces de espacio de nombres (`event="instance_bound"`).
- `/status` incluye un objeto `governance` que refleja los contadores de propuestas, informa todos los espacios de nombres protegidos y lista las activaciones recientes del manifiesto (espacio de nombres, identificación del contrato, código/hash ABI, altura del bloque, marca de tiempo de activación). Los operadores pueden sondear este campeón para confirmar que las promulgaciones durante el día son los manifiestos y que las puertas de los espacios de nombres protegidos se imponen.
- Una plantilla Grafana (`docs/source/grafana_governance_constraints.json`) y el runbook de telemetría en `telemetry.md` cablean alertas para bloqueos de propuestas, activaciones de manifiestos manuales o rechazos de espacios de nombres protegidos durante las actualizaciones del tiempo de ejecución.