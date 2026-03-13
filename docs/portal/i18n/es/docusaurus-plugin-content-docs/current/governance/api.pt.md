---
lang: es
direction: ltr
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Estado: rascunho/esboco para acompanhar as tarefas de implementacao degobernanca. As formas podem mudar durante la implementación. Determinismo y política RBAC sao restricoes normativas; Torii puede determinar/submeter transacoes cuando `authority` e `private_key` son fornecidos, caso contrario os clientes constroem y submetem para `/transaction`.Visao general
- Todos los puntos finales vuelven a JSON. Para flujos que producen transacciones, como respuestas incluyen `tx_instructions` - um array de uma ou mais instrucoes esqueleto:
  - `wire_id`: identificador de registro para o tipo de instrucción
  - `payload_hex`: bytes de carga útil Norito (hexadecimal)
- Se `authority` e `private_key` ante fornecidos (ou `private_key` em DTOs de ballots), Torii assina e submete a transacao e ainda retorna `tx_instructions`.
- Caso contrario, los clientes montan una SignedTransaction usando su autoridad y chain_id, luego assinam y hacen POST para `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` retorno `GovernanceProposalResult` (normaliza campos status/kind), `ToriiClient.get_governance_referendum_typed` retorno `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` retorno `GovernanceTally`, `ToriiClient.get_governance_locks_typed` retorna `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` retorno `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` retorno `GovernanceInstancesPage`, impondo acceso tipado en toda la superficie de gobierno con ejemplos de uso no README.
- Cliente Python nivel (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` retorna los paquetes tipados `GovernanceInstructionDraft` (encapsulando el esqueleto `tx_instructions` a Torii), evitando el análisis manual de JSON cuando los scripts componen fluxos Finalizar/promulgar.- JavaScript (`@iroha/iroha-js`): `ToriiClient` exponen ayudantes tipados para propuestas, referendos, recuentos, bloqueos, desbloqueo de estadísticas y ahora `listGovernanceInstances(namespace, options)` más los puntos finales del consejo (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que los clientes Node.js puedan paginar `/v2/gov/instances/{ns}` y ejecutar flujos de trabajo con VRF junto con el listado existente de instancias de contrato.

Puntos finales

- PUBLICACIÓN `/v2/gov/proposals/deploy-contract`
  - Requisición (JSON):
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
  - Validacao: os nos canonizam `abi_hash` para o `abi_version` fornecido e rejeitam divergencias. Para `abi_version = "v1"`, el valor esperado e `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contratos (implementar)
- PUBLICACIÓN `/v2/contracts/deploy`
  - Requisicao: { "autoridad": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamiento: calcula `code_hash` a partir del cuerpo del programa IVM e `abi_hash` a partir del encabezado `abi_version`, luego submete `RegisterSmartContractCode` (manifiesto) e `RegisterSmartContractBytes` (bytes `.to` completos) en el nombre de `authority`.
  - Respuesta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v2/contracts/code/{code_hash}` -> retorna o manifesto armazenado
    - OBTENER `/v2/contracts/code-bytes/{code_hash}` -> retorna `{ code_b64 }`
- PUBLICACIÓN `/v2/contracts/instance`
  - Requisicao: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: implementar o bytecode fornecido y activar inmediatamente o mapeamento `(namespace, contract_id)` vía `ActivateContractInstance`.
  - Respuesta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Servicio de alias
- PUBLICACIÓN `/v2/aliases/voprf/evaluate`
  - Requisición: { "blinded_element_hex": "..." }
  - Respuesta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` refleja una implementación del evaluador. Valor actual: `blake2b512-mock`.
  - Notas: evaluador simulado determinístico que aplica Blake2b512 con separacao de dominio `iroha.alias.voprf.mock.v1`. Destinado a herramienta de prueba ate o tubería VOPRF de producción ser integrada en Iroha.
  - Errores: HTTP `400` en entrada hexadecimal malformada. Torii retorna un sobre Norito `ValidationFail::QueryFailed::Conversion` con un mensaje de error en el decodificador.
- PUBLICACIÓN `/v2/aliases/resolve`
  - Requisicao: { "alias": "GB82 OESTE 1234 5698 7654 32" }
  - Respuesta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: Solicitud de preparación del puente ISO en tiempo de ejecución (`[iso_bridge.account_aliases]` y `iroha_config`). Torii normaliza alias eliminando espacios y convirtiéndolos para mayúsculas antes de realizar la búsqueda. Retorna 404 cuando el alias está ausente y 503 cuando el puente ISO de tiempo de ejecución está desactivado.
- PUBLICACIÓN `/v2/aliases/resolve_index`
  - Requisición: { "index": 0 }
  - Respuesta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }- Notas: índices de alias sao atribuidos de forma determinística pela ordem de configuracao (basados ​​en 0). Los clientes pueden cachear respuestas fuera de línea para construir trilhas de auditoria de eventos de atestacao de alias.

Limite de tamaño de código
- Parámetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Controla el tamaño máximo permitido (en bytes) para armazenamento de código de contrato on-chain.
  - Predeterminado: 16 MiB. Os nos rejeitam `RegisterSmartContractBytes` quando o tamanho da imagem `.to` excede o limite com um error de violacao de invariante.
  - Los operadores pueden ajustarse mediante `SetParameter(Custom)` con `id = "max_contract_code_bytes"` y una carga útil numérica.- PUBLICACIÓN `/v2/gov/ballots/zk`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas:
    - Cuando las entradas públicas del circuito incluyen `owner`, `amount` e `duration_blocks`, se verifica contra una VK configurada, o no se crea o se mantiene un bloqueo de gobierno para `election_id` como esse `owner`. A direcao permanece oculta (`unknown`); apenas importe/vencimiento sao atualizados. Nuevas votaciones son monótonas: cantidad y vencimiento apenas aumentan (o no aplica max(cantidad, cantidad anterior) y max(expiración, vencimiento anterior)).
    - Re-votes ZK que tentem reduzir cantidad o vencimiento sao rejeitados no servidor com diagnosticos `BallotRejected`.
    - A execucao do contrato deve chamar `ZK_VOTE_VERIFY_BALLOT` antes de enfileirar `SubmitBallot`; anfitriones impoem um latch de uma única vez.- PUBLICACIÓN `/v2/gov/ballots/plain`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sí|No|Abstenerse" }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas: re-votos sao de extensao apenas - um novo ballot nao pode reduzir cantidad o vencimiento do bloqueo existente. O `owner` deve igualar una autoridad da transacao. Duracao minima e `conviction_step_blocks`.- PUBLICACIÓN `/v2/gov/finalize`
  - Requisicao: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito on-chain (scaffold atual): promulgar una propuesta de implementación aprobada insere um `ContractManifest` minimo com chave `code_hash` com o `abi_hash` esperado e marca a propuesta como Enacted. Se um manifesto ja existir para o `code_hash` com `abi_hash` diferente, o enactment e rejeitado.
  - Notas:
    - Para eleicoes ZK, os caminhos do contrato devem chamar `ZK_VOTE_VERIFY_TALLY` antes de ejecutar `FinalizeElection`; Los hosts imponen un pestillo de uso único. `FinalizeReferendum` rejeita referendos ZK ate que o tally da eleicao esteja finalizado.
    - El auto-fechamento em `h_end` emite Aprobado/Rechazado sólo para referendos Plain; referendos ZK permanece cerrado ate que um tally finalizado seja enviado e `FinalizeReferendum` seja ejecutado.
    - Como checagens de participación usam apenas aprobar+rechazar; abstenerse nao conta para o participación.- PUBLICACIÓN `/v2/gov/enact`
  - Requisitos: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii submete a transacao assinada quando `authority`/`private_key` sao fornecidos; caso contrario retorna um esqueleto para clientes assinarem e submeterem. A preimagen e opcional e hoy informativo.

- OBTENER `/v2/gov/proposals/{id}`
  - Ruta `{id}`: id de propuesta hexadecimal (64 caracteres)
  - Respuesta: {"encontrado": bool, "propuesta": {...}? }

- OBTENER `/v2/gov/locks/{rid}`
  - Ruta `{rid}`: string de id de referendum
  - Respuesta: { "found": bool, "referendum_id": "rid", "locks": {...}? }

- OBTENER `/v2/gov/council/current`
  - Respuesta: { "época": N, "miembros": [{ "account_id": "..." }, ...] }
  - Notas: retorna o consejo persistido quando presente; En caso contrario, se deriva un respaldo determinístico usando activos de participación configurados y umbrales (especialmente la VRF específica que prueba VRF en la producción persistente en la cadena).- PUBLICACIÓN `/v2/gov/council/derive-vrf` (característica: gov_vrf)
  - Requisicao: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Pequeño", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: verifica a prueba VRF de cada candidato contra o input canonico derivado de `chain_id`, `epoch` e do ultimo hash de bloco; ordena por bytes de dicha desc con desempates; retorna los miembros superiores `committee_size`. Nao persiste.
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk en G1, prueba en G2 (96 bytes). Pequeño = pk em G2, prueba em G1 (48 bytes). Las entradas están separadas por dominio e incluyen `chain_id`.

### Valores predeterminados de gobierno (iroha_config `gov.*`)

El consejo alternativo usado pelo Torii cuando no existe lista persistente y parametrizado vía `iroha_config`:

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

Anula los equivalentes de ambiente:

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

`parliament_committee_size` limita el número de miembros fallback retornados cuando el consejo persiste, `parliament_term_blocks` define el compromiso de la época usada para derivacao de semilla (`epoch = floor(height / term_blocks)`), `parliament_min_stake` aplica el mínimo de participación (en unidades mínimas) sin activo de elegibilidad, e `parliament_eligibility_asset_id` seleciona qual saldo de activo y escaneado para construir o conjunto de candidatos.La verificación VK de gobierno no pasa por alto el tema: la verificación de la boleta siempre solicita una llave `Active` con bytes en línea, y los ambientes nao deben depender de los interruptores de prueba para poder verificarla.

RBAC
- Permisos exigidos de ejecución en cadena:
  - Propuestas: `CanProposeContractDeployment{ contract_id }`
  - Boletas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgación: `CanEnactGovernance`
  - Dirección del consejo (futuro): `CanManageParliament`Espacios de nombres protegidos
- El parámetro personalizado `gov_protected_namespaces` (matriz de cadenas JSON) habilita la puerta de admisión para implementar listados de espacios de nombres.
- Los clientes deben incluir chaves de metadatos de transacao para implementar que visan espacios de nombres protegidos:
  - `gov_namespace`: espacio de nombres alvo (por ejemplo, "aplicaciones")
  - `gov_contract_id`: ID de contrato lógico dentro del espacio de nombres
- `gov_manifest_approvers`: Matriz JSON opcional de ID de cuenta de validadores. Cuando un manifiesto de carril declara quórum mayor que um, la admisión requiere una autoridad da transacción más como cuentas listadas para satisfacer el quórum del manifiesto.
- Telemetria expoe contadores de admisión vía `governance_manifest_admission_total{result}` para que operadores distingam admitan bem-sucedidos de caminhos `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` e `runtime_hook_rejected`.
- Telemetria expoe o camino de cumplimiento vía `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para que los operadores auditen aprovacoes faltantes.
- Los carriles aplican una lista de espacios de nombres permitidos publicada en sus manifiestos. Cualquier transacción que defina `gov_namespace` debe fornecer `gov_contract_id`, y el espacio de nombres debe aparecer en el conjunto `protected_namespaces` del manifiesto. Envíos `RegisterSmartContractCode` sin metadatos sao rejeitadas quando a protecao esta habilitada.- Admisión impoe que existe una propuesta de gobierno Promulgada para o tupla `(namespace, contract_id, code_hash, abi_hash)`; caso contrario a validacao falha com um error NotPermitted.

Ganchos de actualización del tiempo de ejecución
- Manifiestos de carril que pueden declarar `hooks.runtime_upgrade` para obtener instrucciones de actualización en tiempo de ejecución (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos de gancho:
  - `allow` (bool, predeterminado `true`): cuando `false`, todas las instrucciones de actualización del tiempo de ejecución son rechazadas.
  - `require_metadata` (bool, predeterminado `false`): solicita una entrada de metadatos especificada por `metadata_key`.
  - `metadata_key` (cadena): nome da metadata aplicado pelo gancho. Predeterminado `gov_upgrade_id` cuando se requieren metadatos o una lista de permitidos.
  - `allowed_ids` (array de strings): lista permitida opcional de valores de metadatos (apos trim). Rejeita quando o valor fornecido nao esta lista.
- Cuando el gancho está presente, la admisión de fila aplica a política de metadatos antes de que una transacción entre en fila. Los metadatos ausentes, los valores en blanco o los foros de la lista de permitidos generan un error determinístico NotPermitted.
- Resultados de telemetria rastreia vía `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Las transacciones que satisfacen el gancho deben incluir metadatos `gov_upgrade_id=<value>` (ou a chave definida pelo manifesto) junto con quaisquer aprovacoes de validadores exigidas pelo quorum do manifesto.Punto final de conveniencia
- POST `/v2/gov/protected-namespaces` - aplica `gov_protected_namespaces` directamente no no.
  - Requisicao: { "espacios de nombres": ["aplicaciones", "sistema"]}
  - Respuesta: { "ok": true, "applied": 1 }
  - Notas: destinado a administrador/pruebas; Solicite el token de API configurado. Para producir, prefira enviar una transacción assinada con `SetParameter(Custom)`.CLI de ayuda
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Busca instancias de contrato para el espacio de nombres y conferencia que:
    - Torii armazena bytecode para cada `code_hash`, y su resumen Blake2b-32 corresponde a `code_hash`.
    - O manifesto armazenado em `/v2/contracts/code/{code_hash}` reporta `code_hash` e `abi_hash` corresponsales.
    - Existe una propuesta de gobierno promulgada para `(namespace, contract_id, code_hash, abi_hash)` derivada pelo mesmo hash de propuesta-id que o no usa.
  - Emite un relato JSON con `results[]` por contrato (issues, resumos de manifest/code/proposal) mais um resumo de uma linha a menos que suprimido (`--no-summary`).
  - Util para auditar espacios de nombres protegidos o verificar flujos de implementación controlados por el gobierno.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emite el esqueleto JSON de metadatos usados en implementaciones submétricas en espacios de nombres protegidos, incluido `gov_manifest_approvers` opcional para satisfacer las reglas del quórum del manifiesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]`: las sugerencias de bloqueo se activan cuando `min_bond_amount > 0`, y cualquier conjunto de sugerencias fornecido debe incluir `owner`, `amount` e `duration_blocks`.
  - Valida los identificadores de cuentas canónicas, canonicaliza las sugerencias de anulación de 32 bytes y combina las sugerencias en `public_inputs_json` (con `--public <path>` para anulaciones adicionales).- El anulador se deriva del compromiso de prueba (entrada pública) más `domain_tag`, `chain_id` e `election_id`; `--nullifier` se valida con la prueba cuando se suministra.
  - El resumen de una línea ahora expuesta `fingerprint=<hex>` determinístico derivado de `CastZkBallot` codificado junto con sugerencias decodificadas (`owner`, `amount`, `duration_blocks`, `direction` cuando fornecidos).
  - Como respuestas de CLI anotam `tx_instructions[]` con `payload_fingerprint_hex` más campos decodificados para que ferramentas downstream verifiquem o esqueleto sem reimplementar decodificacao Norito.
  - Fornecer Hints de Lock permite que o no emita eventos `LockCreated`/`LockExtended` para boletas ZK asim que o circuito expuser os mesmos valores.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` asignan los nombres de banderas ZK para paridad de script.
  - A saida de resumo espelha `vote --mode zk` ao incluir o dactilar da instrucción codificada e campos de votación legiveis (`owner`, `amount`, `duration_blocks`, `direction`), ofreciendo confirmacao rapida antes de assinar o esqueleto.Listado de instancias
- GET `/v2/gov/instances/{ns}` - lista de instancias de contrato activas para un espacio de nombres.
  - Parámetros de consulta:
    - `contains`: filtrado por subcadena de `contract_id` (distingue entre mayúsculas y minúsculas)
    - `hash_prefix`: filtrado por prefijo hexadecimal de `code_hash_hex` (minúsculas)
    - `offset` (predeterminado 0), `limit` (predeterminado 100, máx. 10_000)
    - `order`: um de `cid_asc` (predeterminado), `cid_desc`, `hash_asc`, `hash_desc`
  - Respuesta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) o `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Varredura de desbloquea (Operador/Auditoria)
- OBTENER `/v2/gov/unlocks/stats`
  - Respuesta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` reflete a altura de bloco mais recientes onde locks expirados foram varridos e persistentes. `expired_locks_now` y calculó el número de registros de bloqueo con `expiry_height <= height_current`.
- PUBLICACIÓN `/v2/gov/ballots/zk-v1`
  - Requisición (DTO estilo v1):
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
  - Aceita un JSON `BallotProof` directo y retorna un esqueleto `CastZkBallot`.
  - Requisición:
    {
      "autoridad": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "votación": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 para el contenedor ZK1 o H2*
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
    - El servidor mapeia `root_hint`/`owner`/`nullifier` es opcional para votar entre `public_inputs_json` y `CastZkBallot`.
    - Los bytes del sobre se vuelven a codificar como base64 para la carga útil de las instrucciones.
    - A resposta `reason` muda para `submitted transaction` quando Torii submete o ballot.
    - Este punto final está disponible cuando la función `zk-ballot` está habilitada.Camino de verificación CastZkBallot
- `CastZkBallot` decodifica a prueba base64 fornecida y rejeita payloads vazios ou malformados (`BallotRejected` com `invalid or empty proof`).
- El host resuelve una chave verificadora de la boleta a partir del referéndum (`vk_ballot`) o los valores predeterminados de gobierno y exige que el registro exista, esteja `Active` y contiene bytes en línea.
- Bytes de chave verificadora armazenados sao re-hasheados com `hash_vk`; qualquer desajuste de compromiso aborta a execucao antes da verificacao para proteger contra entradas de registro adulteradas (`BallotRejected` com `verifying key commitment mismatch`).
- Bytes de prueba sao despachados al backend registrados vía `zk::verify_backend`; transcricoes invalidas aparecen como `BallotRejected` com `invalid proof` e a instrucao falha de forma determinística.
- La prueba debe exponer un compromiso electoral y una raíz de elegibilidad como aportes públicos; la raíz debe coincidir con el `eligible_root` de la elección y el anulador derivado debe coincidir con cualquier sugerencia proporcionada.
- Provas bem-sucedidas emitem `BallotAccepted`; nullifiers duplicados, root de elegibilidade obsoletos ou regressao de lock continuam a produzir as razoes de rejeicao existentes descritos anteriormente neste documento.

## Mau comportamiento de validadores y consenso conjunto

### Flujo de corte y encarcelamientoEl consenso emite `Evidence` codificada en Norito cuando un validador viola el protocolo. Cada carga útil chega ao `EvidenceStore` en memoria y, se inédito, y materializado no mapa `consensus_evidence` respaldado por WSV. Registros más antiguos que `sumeragi.npos.reconfig.evidence_horizon_blocks` (predeterminado `7200` blocos) sao rejeitados para manter o arquivo limitado, mas a rejeicao e registrado para operadores. La evidencia dentro del horizonte también respeta `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el retraso de corte `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`); la gobernanza puede cancelar las sanciones con `CancelConsensusEvidencePenalty` antes de que se aplique la reducción.

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind`; os discriminantes sao estaveis e impostos pelo modelo de datos:

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

- **DoublePrepare/DoubleCommit** - el validador assinou hashes conflictivos para la misma tupla `(phase,height,view,epoch)`.
- **InvalidQc** - un agregador chismeó un certificado de confirmación cuya forma falta en comprobaciones determinísticas (por ejemplo, mapa de bits de firmantes vazio).
- **InvalidProposal** - un líder prop os un bloque que falta validacao estructural (por ejemplo, viola una regra de lock-chain).
- **Censura**: los recibos de envío firmados muestran una transacción que nunca se propuso ni se comprometió.

Operadores y herramientas pueden inspeccionar y retransmitir cargas útiles a través de:

- Torii: `GET /v2/sumeragi/evidence` e `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, e `... submit --evidence-hex <payload>`.La gobernanza debe tratar los bytes de evidencia como prueba canónica:

1. **Coletar o payload** antes de expirar. Archive los bytes Norito brutos junto con metadatos de altura/vista.
2. **Preparar a penalidade** embedando o payload em um referendum ou instrucao sudo (ej., `Unregister::peer`). Una ejecución revalida o payload; evidencia malformada o obsoleta y rechazada deterministamente.
3. **Agenda a topología de acompañamiento** para que el validador infrator nao possa retornar inmediatamente. Fluxos tipicos enfileiram `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com o roster atualizado.
4. **Auditar resultados** vía `/v2/sumeragi/evidence` e `/v2/sumeragi/status` para garantizar que el contador de evidencia avanza y que un gobierno aplica una remoción.

### Secuenciación de consenso conjunto

El conjunto de consenso garantiza que el conjunto de validadores de dicha finaliza el bloque de frontera antes de que el nuevo conjunto comience a proporcionar. La aplicación de tiempo de ejecución se registra mediante parámetros pareados:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` devem ser confirmados no **mesmo bloco**. `mode_activation_height` debe ser estritamente mayor que a altura do bloco que carregou a atualizacao, fornecendo ao menos um bloco de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el protector de configuración que impide las transferencias con retraso cero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`) retrasa la reducción del consenso para que la gobernanza pueda cancelar las sanciones antes de que se apliquen.```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- El tiempo de ejecución y los parámetros de exposición CLI organizados a través de `/v2/sumeragi/params` e `iroha --output-format text ops sumeragi params`, para que los operadores confirmen alturas de activación y listas de validadores.
- A automacao degobernanca deve siempre:
  1. Finalizar a decisao de remocao (ou reintegro) respaldada por evidencia.
  2. Enfileirar uma reconfiguracao de acompanhamento com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorear `/v2/sumeragi/status` ate `effective_consensus_mode` trocar na altura esperada.

Qualquer script que rotacione validadores ou aplique slashing **nao deve** tentar ativacao de lag zero ou omitir os parametros de hand-off; tais transacoes sao rejeitadas e deixam a rede no modo anterior.

## Superficies de telemetría- Métricas Prometheus exportación de actividad de gobierno:
  - `governance_proposals_status{status}` (calibre) rastreia contagios de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (contador) incrementa cuando la admisión de espacios de nombres protegidos permite o rejeita un despliegue.
  - `governance_manifest_activations_total{event}` (contador) registra insercoes de manifest (`event="manifest_inserted"`) y enlaces de espacio de nombres (`event="instance_bound"`).
- `/status` incluye un objeto `governance` que se muestra como contenedor de propuestas, relación total de espacios de nombres protegidos y lista activa de manifiestos recientes (espacio de nombres, identificación de contrato, código/hash ABI, altura del bloque, marca de tiempo de activación). Operadores podem consultar esse campo para confirmar que enactments atualizaram manifests e que gates de namespaces protegidos estao sendo aplicados.
- Una plantilla Grafana (`docs/source/grafana_governance_constraints.json`) y un runbook de telemetría en `telemetry.md` se muestran como ligar alertas para propuestas presas, activaciones de manifiestos ausentes, o avisos inesperados de espacios de nombres protegidos durante las actualizaciones de tiempo de ejecución.