---
lang: he
direction: rtl
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

מקום: borrador/boceto para acompanar las tareas de implementacion de gobernanza. Las formas pueden cambiar durante la implementacion. El determinismo y la politica RBAC son restricciones normativas; Torii puede firmar/enviar transacciones cuando se proporcionan `authority` y `private_key`, de lo contrario los clientes construyen y envian a `/transaction`.

קורות חיים
- Todos los pointpoints devuelven JSON. Para flujos que produce transacciones, las respuestas incluyen `tx_instructions` - un arreglo de una o mas instrucciones esqueleto:
  - `wire_id`: זיהוי רישום עבור אל טיפו דה הוראות
  - `payload_hex`: בתים של מטען Norito (hex)
- Si se proporcionan `authority` y `private_key` (o `private_key` en DTOs de ballots), Torii firma y envia la transaccion y aun devuelve I108NI00X.
- De lo contrario, los clientes arman una SignedTransaction usando su Authority y chain_id, luego firman y hasen POST a `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` devuelve `GovernanceProposalResult` (מצב/סוג normaliza campos), `ToriiClient.get_governance_referendum_typed` devuelve `GovernanceReferendumResult`, I100NI4300 `GovernanceTally`, `ToriiClient.get_governance_locks_typed` devuelve `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` devuelve `GovernanceUnlockStats`, y `ToriiClient.list_governance_instances_typed` devuelve I0101 en toda la superficie de gobernanza con ejemplos de uso en README.
- Cliente ligero Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal` devuelven bundles tipados `GovernanceInstructionDraft` (envolviendo el esqueleto Prometheus I105520X), evitando parseo JSON ידני קבצי סקריפטים מרכיבים flujos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` לחשוף עוזרים טיפים להצעות, משאל בחירות, תוצאות, מנעולים, סטטיסטיקות ביטול נעילה, y ahora `listGovernanceInstances(namespace, options)` mas los נקודות הקצה של המועצה (Prometheus, I000000590X, I010000590X, I00000050X, I000000050X, `listGovernanceInstances(namespace, options)` `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que clientes Node.js puedan paginar `/v2/gov/instances/{ns}` y conducir flujos respaldados por VRF Junto con el Listado de instancias de contrato existente.

נקודות קצה

- POST `/v2/gov/proposals/deploy-contract`
  - Solicitud (JSON):
    {
      "namespace": "אפליקציות",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "i105...?",
      "private_key": "...?"
    }
  - תגובה (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - תוקף: los nodos canonizan `abi_hash` para el `abi_version` provisto y rechazan desajustes. Para `abi_version = "v1"`, el valor esperado es `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contratos (פריסה)
- POST `/v2/contracts/deploy`
  - Solicitud: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - רכיב: calcula `code_hash` del cuerpo del programa IVM y `abi_hash` del header `abi_version`, luego envia `RegisterSmartContractCode`) 0y01estoNI (בתים `.to` השלמות) בכתובת `authority`.
  - תגובה: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - קבל `/v2/contracts/code/{code_hash}` -> devuelve el manifiesto almacenado
    - קבל `/v2/contracts/code-bytes/{code_hash}` -> devuelve `{ code_b64 }`
- POST `/v2/contracts/instance`
  - Solicitud: { "authority": "i105...", "private_key": "...", "namespace": "אפליקציות", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: despliega el bytecode provisto y active de inmediato el mapeo `(namespace, contract_id)` דרך `ActivateContractInstance`.
  - תגובה: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Servicio de alias
- POST `/v2/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - תגובה: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` רפלג'ה ליישום מעריך. ערך בפועל: `blake2b512-mock`.
  - הערות: evaluador mock determinista que aplica Blake2b512 con separacion de dominio `iroha.alias.voprf.mock.v1`. Disenado para tooling de prueba hasta que el pipeline VOPRF de produccion este cableado en Iroha.
  - שגיאות: HTTP `400` ב-Input hex mal formatado. Torii devuelve un envelope Norito `ValidationFail::QueryFailed::Conversion` con el mensaje de error del decoder.
- POST `/v2/aliases/resolve`
  - Solicitud: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - תגובה: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - הערות: דרושות גישור ISO בזמן ריצה (`[iso_bridge.account_aliases]` ו-`iroha_config`). Torii נורמליזה כינוי eliminando espacios y pasando ו-mausculas antes de lookup. Devuelve 404 cuando el alias no existe y 503 cuando el runtime ISO bridge esta dishabilitado.
- POST `/v2/aliases/resolve_index`
  - בקשה: { "אינדקס": 0 }
  - תגובה: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: los indices de alias se assignan de forma determinista segun el orden de configuracion (מבוסס 0). כל לקוחות מאמינים ב-cachear respuestas לא מקוונים עבור בניית אודיטוריה של אירועי כינוי.

Tope de tamano de codigo
- פרמטר מותאם אישית: `max_contract_code_bytes` (JSON u64)
  - Controla el tamano maximo permitido (en bytes) para almacenamiento de codigo de contrato on-chain.
  - ברירת מחדל: 16 MiB. Los nodos rechazan `RegisterSmartContractBytes` cuando la imagen `.to` excede el tope con un error de violacion de invariante.
  - Los operadores pueden ajustar enviando `SetParameter(Custom)` con `id = "max_contract_code_bytes"` y un payload numerico.- POST `/v2/gov/ballots/zk`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - תשובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות:
    - Cuando las entradas publicas del circuito incluyen `owner`, `amount` y `duration_blocks`, y la prueba verifica contra la VK configurada, el nodo crea o para extiende un bloqueo 01020X `owner`. La direccion permanece oculta (`unknown`); סכום/תפוגה בודד. Las revotaciones son monotonic: כמות y expiry solo aumentan (el nodo aplica max(amount, prev.amount) y max(expiry, prev.expiry)).
    - Las revotaciones ZK que intenten reducir סכום o expire se rechazan del lado del servidor con diagnosticos `BallotRejected`.
    - La ejecucion del contrato debe llamar `ZK_VOTE_VERIFY_BALLOT` antes de encolar `SubmitBallot`; los hosts imponen un latch de una sola vez.

- POST `/v2/gov/ballots/plain`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "directionay"}: "Aye|tainN
  - תשובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas: las revotaciones son de solo extension - un nuevo ballot no puede reducir el amount o expiry del bloqueo existente. El `owner` debe igualar la Authority de la transaccion. La duracion minima es `conviction_step_blocks`.

- POST `/v2/gov/finalize`
  - Solicitud: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efecto on-chain (andamiaje actual): פרסם את הפריסה של הוספה ל-`ContractManifest` minimo con clave `code_hash` con el `abi_hash` esperado y marca la propuesta como Enacted. Si ya existe un manifiesto para el `code_hash` con un `abi_hash` distinto, la promulgacion se rechaza.
  - הערות:
    - Para elecciones ZK, las rutas del contrato deben llamar `ZK_VOTE_VERIFY_TALLY` antes de ejecutar `FinalizeElection`; los hosts imponen un latch de una sola vez. `FinalizeReferendum` rechaza referendos ZK hasta que el tally de la elccion este finalizado.
    - El cierre automatico en `h_end` emite אושר/נדחה סולו עבור רפרנטים מישור; los referendos ZK permanecen סגורה יש לראות את סיכום הסיכום y seejecute `FinalizeReferendum`.
    - Las comprobaciones de turnout usan solo approve+reject; נמנע ללא התאמה לשיעור ההצבעה.- POST `/v2/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - הערות: Torii envia la transaccion firmada cuando se proporcionan `authority`/`private_key`; de lo contrario devuelve un esqueleto para que los clientes firmen y envien. La preimage es optional y actualmente informativa.

- קבל את `/v2/gov/proposals/{id}`
  - נתיב `{id}`: id de propuesta hex (64 תווים)
  - תגובה: { "נמצא": bool, "הצעה": { ... }? }

- קבל את `/v2/gov/locks/{rid}`
  - נתיב `{rid}`: string de referendum id
  - תגובה: { "נמצא": bool, "referendum_id": "לפטר", "מנעולים": { ... }? }

- קבל `/v2/gov/council/current`
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notas: devuelve el Council persistido cuando existe; de lo contrario deriva un respaldo determinista usando el asset de stake configurado y umbrales (reflja la especificacion VRF hasta que pruebas VRF en vivo se persistan on-chain).

- POST `/v2/gov/council/derive-vrf` (תכונה: gov_vrf)
  - Solicitud: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "רגיל|קטן", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: verifica la prueba VRF de cada candidato contra el input canonico derivado de `chain_id`, `epoch` y el beacon del ultimo hash de bloque; ordena por bytes de salida desc con breakers; devuelve los top `committee_size` miembros. לא מתמיד.
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - הערות: Normal = pk en G1, proof en G2 (96 בתים). Small = pk en G2, proof en G1 (48 בתים). Los inputs estan separados por dominio e incluyen `chain_id`.

### ברירות מחדל de gobernanza (iroha_config `gov.*`)

El Council de respaldo usado por Torii cuando no existe un roster persistido se parametriza via `iroha_config`:

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

עוקפים את ה-entorno equivalentes:

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

`parliament_committee_size` limita la cantidad de miembros de respaldo devueltos cuando no hay Council persistido, `parliament_term_blocks` define la longitud de epoca usada para derivacion de seed (Prometheus), I001NI stake de (en unidades minimas) sobre el asset de elegibilidad, y `parliament_eligibility_asset_id` selecciona que balance de asset se escanea al construir el conjunto de candidatos.

La verificacion VK de gobernanza no tiene bypass: la verificacion de ballots siempre requiere una clave verificadora `Active` con bytes inline, y los entornos no deben depender de toggles de prueba para omitir verificacion.

RBAC
- La ejecucion על השרשרת דורש הרשאות:
  - הצעות: `CanProposeContractDeployment{ contract_id }`
  - קלפי: `CanSubmitGovernanceBallot{ referendum_id }`
  - חקיקה: `CanEnactGovernance`
  - הנהלת המועצה (futuro): `CanManageParliament`מרחבי שמות protegidos
- פרמטר מותאם אישית `gov_protected_namespaces` (מערך מיתרים של JSON) גישה לכניסה לפריסת מרחבי שמות.
- Los clientes deben inclluir claves de metadata de transaccion עבור פריסת מערכות מידע ומרחבי שמות:
  - `gov_namespace`: el namespace objetivo (ej., "אפליקציות")
  - `gov_contract_id`: מזהה חוזה לוגיקה דנטרו של מרחב השמות
- `gov_manifest_approvers`: מערך JSON אופציונלי של מזהי חשבונות מאושרים. Cuando un manifiesto de lane declara un quorum mayor a uno, קבלה מחייבת רשות דה לה טרנסאקציון mas las cuentas listadas para satisfacer el quorum del manifiesto.
- La telemetria expone contadores de admission via `governance_manifest_admission_total{result}` para que operadores distingan admits exitosos de rutas `missing_manifest`, `non_validator_authority`, `quorum_rejected`, I101NI500, Prometheus, `runtime_hook_rejected`.
- La telemetria expone la ruta de enforcement via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) עבור מפעילי ביקורת אפרובאציוניים.
- Los lanes aplican el permitlist de namespaces publicado en sus manifests. Cualquier transaccion que fije `gov_namespace` debe proporcionar `gov_contract_id`, y el namespace debe aparecer en el set `protected_namespaces` del manifiesto. Los envios `RegisterSmartContractCode` esta metadata se rechazan cuando la proteccion esta habilitada.
- La admission impone que exista una propuesta de gobernanza Enacted para el tuple `(namespace, contract_id, code_hash, abi_hash)`; de lo contrario la validacion falla con un error NotPermitted.

הוקס לשדרוג זמן ריצה
- Los manifests de lane pueden declarar `hooks.runtime_upgrade` עבור הוראות שליטה לשדרוג זמן ריצה (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos del Hook:
  - `allow` (bool, ברירת המחדל `true`): cuando es `false`, ראה מחדש את ההוראות לשדרוג זמן ריצה.
  - `require_metadata` (bool, ברירת המחדל `false`): exige la entrada de metadata especificada por `metadata_key`.
  - `metadata_key` (מחרוזת): אפליקציית מטא-נתונים עבור אל הוק. ברירת מחדל `gov_upgrade_id` יש צורך במטא נתונים לרשימת ההיתרים.
  - `allowed_ids` (מערך של מחרוזות): רשימת אישורים אופציונלית של מטא-נתונים (קצץ תזמורת). Rechaza cuando el valor provisto no esta listado.
- Cuando el hook esta presente, כניסה de la cola aplica la politica de metadata antes de que la transaccion entre a la cola. מטא-נתונים נפלו, תוצאות חילופיות או ערכיות של רשימת ההיתרים נוצרה ב-NotPermitted שגיאה.
- La telemetria rastrea resultados דרך `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Las transacciones que cumplen el hook deben incluir metadata `gov_upgrade_id=<value>` (o la clave definida por el manifiesto) junto con cualquier aprobacion de validadores requerida por el quorum del manifiesto.נקודת קצה נוחות
- POST `/v2/gov/protected-namespaces` - אפליקציית `gov_protected_namespaces` directamente en el nodo.
  - בקשה: { "מרחבי שמות": ["אפליקציות", "מערכת"] }
  - תגובה: { "בסדר": true, "applied": 1 }
  - הערות: pensado para admin/testing; דורש אסימון API עבור הגדרות אלה. Para produccion, prefiera enviar una transaccion firmada con `SetParameter(Custom)`.

עוזרי CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Obtiene instancias de contrato para el namespace y verifica que:
    - Torii almacena bytecode para cada `code_hash`, y su digest Blake2b-32 coincide con el `code_hash`.
    - El manifiesto almacenado bajo `/v2/contracts/code/{code_hash}` reporta valores `code_hash` y `abi_hash` מקריים.
    - Existe una propuesta de gobernanza נחקק para `(namespace, contract_id, code_hash, abi_hash)` derivada por el mismo hashing de offer-id que usa el nodo.
  - Emite un reporte JSON con `results[]` por contrato (בעיות, קורות חיים של מניפסט/קוד/הצעה) mas un resumen de una linea salvo que se supprima (`--no-summary`).
  - השתמש במרחבי השמות המוגנים או בדוק את פעולות ההפעלה של בקרה.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - אבטחת JSON של מטא נתונים בארה"ב לכל פריסות מרחבי שמות, כולל `gov_manifest_approvers` אופציונליים לשביעות רצונם של מניין המניפסט.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — los lock רמזים son obligatorios cuando `min_bond_amount > 0`, y cualquier conjunto de hints proporcionado debe incluir `owner`, `/v2/contracts/instance`.
  - מאמת מזהי חשבונות קנוניים, מבצע קנוניזציה של רמזים לביטול של 32 בתים וממזג את הרמזים לתוך `public_inputs_json` (עם `--public <path>` לעקיפות נוספות).
  - המבטל נגזר מהתחייבות ההוכחה (קלט ציבורי) בתוספת `domain_tag`, `chain_id` ו-`election_id`; `--nullifier` מאומת כנגד ההוכחה כאשר היא מסופקת.
  - El resumen de una linea ahora expone un `fingerprint=<hex>` determinista derivado del `CastZkBallot` codificado junto con hints decodificados (`owner`, `amount`, `amount`, I000000208X, I00000208X0, I `direction` cuando se proporcionan).
  - Las respuestas CLI anotan `tx_instructions[]` con `payload_fingerprint_hex` mas campos decodificados para que herramientas downstream verifiquen el esqueleto sin reimplementar decodificacion Norito.
  - הוכח רמזים דה בלוקו היתרי que el nodo emita eventos `LockCreated`/`LockExtended` לקלפי ZK una vez que el circuito exponga los mismos valores.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` reflejan los nombres de flags de ZK para paridad en scripts.
  - La salida de resumen refleja `vote --mode zk` כולל טביעת אצבע של קוד ההוראות וקריאת ההצבעה (`owner`, `amount`, Prometheus, Prometheus, I01000022120X, I0100002200X, I010000220X, I010000219X, rapida antes de firmar el esqueleto.Listado de instancias
- קבל `/v2/gov/instances/{ns}` - רשימה של מופעים נגדיים להפעלת מרחב שמות.
  - פרמטרים של שאילתה:
    - `contains`: filtra por substring de `contract_id` (תלוי רישיות)
    - `hash_prefix`: filtra por prefijo hex de `code_hash_hex` (אותיות קטנות)
    - `offset` (ברירת מחדל 0), `limit` (ברירת מחדל 100, מקסימום 10_000)
    - `order`: uno de `cid_asc` (ברירת מחדל), `cid_desc`, `hash_asc`, `hash_desc`
  - תגובה: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) או `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Barrido de unlocks (Operador/Auditoria)
- קבל את `/v2/gov/unlocks/stats`
  - תגובה: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - הערות: `last_sweep_height` refleja el block height mas reciente donde los locks expirados fueron barridos y persistidos. `expired_locks_now` חשב את רישומי המנעול עם `expiry_height <= height_current`.
- POST `/v2/gov/ballots/zk-v1`
  - Solicitud (DTO estilo v1):
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "backend": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "owner": "i105...?",
      "nullifier": "blake2b32:...64hex?"
    }
  - תשובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v2/gov/ballots/zk-v1/ballot-proof` (תכונה: `zk-ballot`)
  - Acepta un JSON `BallotProof` director y devuelve un esqueleto `CastZkBallot`.
  - שידול:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "הצבעה": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 de contenedor ZK1 o H2*
        "root_hint": null, // מחרוזת hex אופציונלית של 32 בתים (שורש זכאות)
        "owner": null, // AccountId אופציונלי cuando el circuito compromete בעלים
        "nullifier": null // מחרוזת hex אופציונלית של 32 בתים (רמז לבטל)
      }
    }
  - תשובה:
    {
      "בסדר": נכון,
      "מקובל": נכון,
      "reason": "בנה שלד עסקה",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - הערות:
    - El servidor mapea `root_hint`/`owner`/`nullifier` אופציונליים לבחירה ב-`public_inputs_json` עבור `CastZkBallot`.
    - Los bytes del envelope se re-encodan como base64 para el payload de la instruccion.
    - La respuesta `reason` cambia a `submitted transaction` cuando Torii envia el vote.
    - נקודת קצה בודדת ניתנת לשימוש עם תכונה `zk-ballot`.Ruta de verificacion de CastZkBallot
- `CastZkBallot` decodifica la prueba base64 provista y rechaza payloads vacios o mal formados (`BallotRejected` con `invalid or empty proof`).
- El host resuelve la clave verificadora del ballot desde el משאל העם (`vk_ballot`) או ברירות מחדל של gobernanza y requiere que el registro exista, sea `Active`, y lleve bytes inline.
- Los bytes de la clave verificadora almacenada se re-hashean con `hash_vk`; cualquier desajuste de compromiso aborta la ejecucion antes de verificar para proteger contra entradas de registro adulteradas (`BallotRejected` con `verifying key commitment mismatch`).
- Los bytes de la prueba se despachan al backend registrado דרך `zk::verify_backend`; transcripciones invalidas aparecen como `BallotRejected` con `invalid proof` y la instruccion falla deterministamente.
- על ההוכחה לחשוף התחייבות קלפי ושורש זכאות כתשומות ציבוריות; השורש חייב להתאים ל-`eligible_root` של הבחירות, והמבטל הנגזר חייב להתאים לכל רמז שסופק.
- Pruebas exitosas emiten `BallotAccepted`; מבטל דופליקציות, שורשים דה elegibilidad viejos או regresiones de lock siguen produciendo las razones de rechazo existentes descritas antes en este documento.

## Mala conducta de validadores y consenso conjunto

### חיתוך בכלא

קונסנסו emit `Evidence` codificada en Norito cuando unvalidador viola el protocolo. Cada payload llega al `EvidenceStore` en memoria y, si לא ראה אנטס, se materializa en el mapa `consensus_evidence` respaldado por WSV. Los registros anteriores a `sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת המחדל של `7200` בלוקס) ראה את הרשימה עבור ארכיון קבוע, אבל יש לך אפשרות להירשם עבור מפעילים. הראיות באופק מכבדות גם את `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) ואת עיכוב החיתוך `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`); ממשל יכול לבטל קנסות עם `CancelConsensusEvidencePenalty` לפני שהחתך חל.

Las ofensas reconocidas se mapean uno a uno a `EvidenceKind`; los discriminantes son estables y estan reforzados por el modelo de data:

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

- **DoublePrepare/DoubleCommit** - el validador firmo hashes en conflicto para el mismo tuple `(phase,height,view,epoch)`.
- **InvalidQc** - un agregador gossipeo un commit certificate cuya forma falla chequeos deterministas (ej., bitmap de firmantes vacio).
- **InvalidProposal** - un lider propuso un bloque que falla la validacion estructural (ej., rompe la regla de locked-chain).
- **צנזורה** - קבלות הגשה חתומות מציגות עסקה שמעולם לא הוצעה/בוצעה.

מפעילים ומכשירים בודקים בדיקת עומסי שידור חוזרים ומשדרים:

- Torii: `GET /v2/sumeragi/evidence` y `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, y `... submit --evidence-hex <payload>`.

La gobernanza debe tratar los bytes de evidence como prueba canonica:1. **Recolectar el payload** antes de que caduque. ארכיון בתים Norito crudos junto con metadata de height/view.
2. **Preparar la penalidad** embebiendo el payload en un referendum o instruccion sudo (ej., `Unregister::peer`). La ejecucion re-valida el מטען; ראיות mal formada o rancia se rechaza deterministamente.
3. **Programar la topologia de seguimiento** para que el validador infractor no pueda reingresar de inmediato. Flujos tipicos encolan `SetParameter(Sumeragi::NextMode)` y `SetParameter(Sumeragi::ModeActivationHeight)` עם הסגל בפועל.
4. **Auditar resultados** דרך `/v2/sumeragi/evidence` y `/v2/sumeragi/status` para asegurar que el contador de evidence avanzo y que la gobernanza aplico la remocion.

### Secuenciacion de consenso conjunto

El consenso conjunto garantiza que el conjunto de validadores saliente finalice el bloque de frontera antes de que el nuevo conjunto empiece a proponer. Eltime run time impone la regla via parametros paraeados:

- `SumeragiParameter::NextMode` y `SumeragiParameter::ModeActivationHeight` deben confirmarse en el **mismo bloque**. `mode_activation_height` debe ser estrictamente mayor que la altura del bloque que cargo el update, proporcionando al menos un bloque de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) הוא השמירה על ההגדרות של מסירות מוקדמות עם המשך:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`) מעכב את קיצוץ הקונצנזוס כך שהממשל יכול לבטל עונשים לפני שהם יחולו.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- זמן ריצה עם פרמטרים של CLI מבוצעים באמצעות `/v2/sumeragi/params` y `iroha --output-format text ops sumeragi params`, עבור מפעילים מאשרים את ההפעלה ואת רשימות האימות.
- La automatizacion de gobernanza siempre debe:
  1. Finalizar la decision de remocion (o reinstallacion) respaldada por evidence.
  2. Encolar una reconfiguracion de seguimiento con `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorear `/v2/sumeragi/status` יש גישה ל-`effective_consensus_mode`.

תסריט מדוייק que rote validadores o aplique slashing **ללא תכונה** הפעלה של כוונה להחלפת הפרמטרים של המסירה; esas transacciones se rechazan y dejan la red en el modo previo.

## שטחי טלמטריה- Las metricas Prometheus ייצוא פעילות גופנית:
  - `governance_proposals_status{status}` (מד) rastrea conteos de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (מונה) מגדילים את כניסת מרחבי השמות להגנה על פריסה.
  - `governance_manifest_activations_total{event}` (counter) registra inserciones de manifest (`event="manifest_inserted"`) y bindings de namespace (`event="instance_bound"`).
- `/status` כולל את `governance` עבור מסמכים נוספים, דיווחים הכוללים של מרחבי השמות והרשימה הרשמית של המניפסטים (מרחב שמות, מזהה חוזה, קוד/ABI hash, גובה בלוק, חותמת זמן הפעלה). Los operadores pueden consultar este campo para confirmar que las promulgaciones actualizaron manifests y que los gates de namespaces protegidos se aplican.
- Una plantilla Grafana (`docs/source/grafana_governance_constraints.json`) y el runbook de telemetria en `telemetry.md` muestran como כבלים אזעקות לשירותי התקדמות, הפעלה של מניפסט faltantes, או rechazos de name despaceranteadosgis.