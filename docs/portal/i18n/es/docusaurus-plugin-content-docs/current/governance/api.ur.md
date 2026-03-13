---
lang: es
direction: ltr
source: docs/portal/docs/governance/api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

حالت: گورننس نفاذی کاموں کے ساتھ چلنے والا ڈرافٹ/اسکیچ۔ عملدرآمد کے دوران ساختیں بدل سکتی ہیں۔ Determinismo اور RBAC پالیسی معیاری پابندیاں ہیں؛ جب `authority` اور `private_key` فراہم ہوں تو Torii ٹرانزیکشن سائن/سبمٹ کر سکتا ہے، ورنہ کلائنٹس بنا کر `/transaction` پر سبمٹ کرتے ہیں۔جائزہ
- تمام puntos finales JSON y کرتے ہیں۔ ٹرانزیکشن بنانے والے فلو کے لئے جوابات میں `tx_instructions` شامل ہوتے ہیں - ایک یا زیادہ esqueletos de instrucciones کی matriz:
  - `wire_id`: instrucción ٹائپ کا identificador de registro
  - `payload_hex`: Bytes de carga útil Norito (hexadecimal)
- اگر `authority` اور `private_key` (یا DTO electorales میں `private_key`) فراہم ہوں تو Torii ٹرانزیکشن سائن اور سبمٹ کرتا ہے اور پھر بھی `tx_instructions` واپس کرتا ہے۔
- ورنہ کلائنٹس اپنی Authority اور chain_id کے ساتھ SignedTransaction بناتے ہیں، پھر سائن کر کے `/transaction` پر POST کرتے ہیں۔
- Cobertura del SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` y کرتا ہے (campos de estado/tipo کو normalizar کرتا ہے), `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` واپس کرتا ہے، `ToriiClient.get_governance_tally_typed` `GovernanceTally` واپس کرتا ہے، `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` واپس کرتا ہے، superficie de gobernanza پر acceso escrito ملتا ہے اور README میں ejemplos de uso دیے گئے ہیں۔
- Cliente ligero Python (`iroha_torii_client`): paquetes `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal` mecanografiados `GovernanceInstructionDraft` y paquetes (Torii کی `tx_instructions` esqueleto کو wrap کرتے ہوئے), تاکہ scripts Finalizar/Promulgar flujos بناتے وقت análisis JSON manual سے بچ سکیں۔- JavaScript (`@iroha/iroha-js`): `ToriiClient` propuestas, referendos, recuentos, bloqueos, estadísticas de desbloqueo کے لئے ayudantes escritos دیتا ہے، اور اب `listGovernanceInstances(namespace, options)` کے ساتھ puntos finales del consejo (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) بھی دیتا ہے تاکہ Clientes Node.js `/v2/gov/instances/{ns}` کو paginar کر سکیں اور Flujos de trabajo respaldados por VRF کو موجودہ Listado de instancias de contrato کے ساتھ چلا سکیں۔

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
  - Validación: nodos فراہم کردہ `abi_version` کے لئے `abi_hash` کو canonicalizar کرتے ہیں اور falta de coincidencia پر rechazar کرتے ہیں۔ `abi_version = "v1"` کے لئے متوقع valor `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ہے۔API de contratos (implementación)
- PUBLICACIÓN `/v2/contracts/deploy`
  - Solicitud: { "autoridad": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamiento: IVM پروگرام باڈی سے `code_hash` اور header `abi_version` سے `abi_hash` نکالتا ہے، پھر `RegisterSmartContractCode` (manifiesto) اور `RegisterSmartContractBytes` (مکمل `.to` bytes) `authority` کی طرف سے سبمٹ کرتا ہے۔
  - Respuesta: { "ok": verdadero, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - OBTENER `/v2/contracts/code/{code_hash}` -> ذخیرہ شدہ manifiesto واپس کرتا ہے
    - OBTENER `/v2/contracts/code-bytes/{code_hash}` -> `{ code_b64 }` واپس کرتا ہے
- PUBLICACIÓN `/v2/contracts/instance`
  - Solicitud: { "autoridad": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamiento: implementación de código de bytes فراہم کردہ کرتا ہے اور `ActivateContractInstance` کے ذریعے `(namespace, contract_id)` mapeo فوراً فعال کرتا ہے۔
  - Respuesta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Servicio de alias
- PUBLICACIÓN `/v2/aliases/voprf/evaluate`
  - Solicitud: { "blinded_element_hex": "..." }
  - Respuesta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - Implementación del evaluador `backend` کو ظاہر کرتا ہے۔ Valor mínimo: `blake2b512-mock`۔
  - Notas: evaluador simulado determinista جو Blake2b512 کو separación de dominios `iroha.alias.voprf.mock.v1` کے ساتھ apply کرتا ہے۔ یہ herramientas de prueba کے لئے ہے جب تک producción VOPRF tubería Iroha میں alambre نہ ہو جائے۔
  - Errores: entrada hexadecimal con formato incorrecto en HTTP `400`۔ Torii Norito `ValidationFail::QueryFailed::Conversion` Mensaje de error del sobre y del decodificador واپس کرتا ہے۔
- PUBLICACIÓN `/v2/aliases/resolve`
  - Solicitud: { "alias": "GB82 OESTE 1234 5698 7654 32" }
  - Respuesta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: puesta en escena del tiempo de ejecución del puente ISO درکار ہے (`[iso_bridge.account_aliases]` en `iroha_config`) ۔ Torii espacios en blanco ہٹا کر اور mayúsculas بنا کر búsqueda کرتا ہے۔ alias نہ ہو تو 404 اور Tiempo de ejecución del puente ISO بند ہو تو 503 دیتا ہے۔
- PUBLICACIÓN `/v2/aliases/resolve_index`
  - Solicitud: { "índice": 0 }
  - Respuesta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: orden de configuración de índices de alias کے مطابق determinista طریقے سے asignar ہوتے ہیں (basado en 0)۔ کلائنٹس caché fuera de línea کر کے eventos de atestación de alias کے pistas de auditoría بنا سکتے ہیں۔Código Tamaño Tapa
- Parámetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Almacenamiento de código de contrato en cadena کے لئے زیادہ سے زیادہ سائز (bytes میں) کنٹرول کرتا ہے۔
  - Predeterminado: 16 MiB۔ Imagen `.to` حد سے بڑی ہو تو nodos `RegisterSmartContractBytes` کو error de violación invariante کے ساتھ rechazar کرتے ہیں۔
  - Operadores `SetParameter(Custom)` کے ذریعے `id = "max_contract_code_bytes"` اور carga útil numérica دے کر ایڈجسٹ کر سکتے ہیں۔

- PUBLICACIÓN `/v2/gov/ballots/zk`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas:
    - Circuito de entradas públicas `owner`, `amount`, `duration_blocks` Prueba configurada VK Verificar nodo `election_id` کے لئے bloqueo de gobernanza بناتا یا بڑھاتا ہے۔ dirección چھپی رہتی ہے (`unknown`); صرف monto/vencimiento اپڈیٹ ہوتے ہیں۔ re-votos monótonos ہیں: cantidad اور vencimiento صرف بڑھتے ہیں (nodo max(cantidad, cantidad anterior) اور max(vencimiento, vencimiento anterior) لگاتا ہے)۔
    - ZK vuelve a votar جو cantidad یا vencimiento کم کرنے کی کوشش کریں diagnóstico `BallotRejected` del lado del servidor کے ساتھ rechazar ہوتے ہیں۔
    - Ejecución de contrato کو `SubmitBallot` en cola کرنے سے پہلے `ZK_VOTE_VERIFY_BALLOT` کال کرنا لازم ہے؛ los hosts ejecutan el pestillo de un solo disparo کرتے ہیں۔- PUBLICACIÓN `/v2/gov/ballots/plain`
  - Solicitud: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sí|No|Abstenerse" }
  - Respuesta: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - Notas: nuevas votaciones solo se extienden ہیں - نیا boleta موجودہ bloqueo کا cantidad یا vencimiento کم نہیں کر سکتا۔ `owner` کو autoridad de transacción کے برابر ہونا چاہئے۔ کم از کم مدت `conviction_step_blocks` ہے۔- PUBLICACIÓN `/v2/gov/finalize`
  - Solicitud: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efecto en cadena (andamio actual): منظور شدہ implementar propuesta کو promulgar کرنے سے `code_hash` con clave mínima `ContractManifest` شامل ہوتا ہے جس میں متوقع `abi_hash` ہوتا ہے اور propuesta Promulgada ہو جاتا ہے۔ اگر `code_hash` کے لئے مختلف `abi_hash` والا manifiesto پہلے سے ہو تو promulgación rechazar ہوتا ہے۔
  - Notas:
    - Elecciones ZK کے لئے rutas de contrato کو `FinalizeElection` سے پہلے `ZK_VOTE_VERIFY_TALLY` کال کرنا لازمی ہے؛ los hosts ejecutan el pestillo de un solo disparo کرتے ہیں۔ `FinalizeReferendum` Referendos ZK کو اس وقت تک rechazar کرتا ہے جب تک recuento electoral finalizado نہ ہو جائے۔
    - Cierre automático `h_end` پر صرف Referendos simples کے لئے Aprobado/Rechazado emit کرتا ہے؛ Referendos ZK Cerrado رہتے ہیں جب تک recuento finalizado enviar نہ ہو اور `FinalizeReferendum` ejecutar نہ ہو۔
    - Controles de participación صرف aprobar+rechazar استعمال کرتی ہیں؛ abstenerse de participar میں شمار نہیں ہوتا۔- PUBLICACIÓN `/v2/gov/enact`
  - Solicitud: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Respuesta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: جب `authority`/`private_key` فراہم ہوں تو Torii transacción firmada سبمٹ کرتا ہے؛ ورنہ وہ esqueleto واپس کرتا ہے جسے کلائنٹ سائن اور سبمٹ کرے۔ preimagen اختیاری اور فی الحال معلوماتی ہے۔

- OBTENER `/v2/gov/proposals/{id}`
  - Ruta `{id}`: ID de propuesta hexadecimal (64 caracteres)
  - Respuesta: {"encontrado": bool, "propuesta": {...}? }

- OBTENER `/v2/gov/locks/{rid}`
  - Ruta `{rid}`: cadena de identificación del referéndum
  - Respuesta: { "found": bool, "referendum_id": "rid", "locks": {...}? }

- OBTENER `/v2/gov/council/current`
  - Respuesta: { "época": N, "miembros": [{ "account_id": "..." }, ...] }
  - Notas: Consejo de موجودہ موجود ہو تو واپس کرتا ہے، ورنہ activo de participación configurado اور umbrales استعمال کر کے derivación de reserva determinista کرتا ہے (especificaciones VRF کو اس وقت تک reflejar کرتا ہے جب تک pruebas VRF en vivo en cadena persisten نہ ہوں)۔- PUBLICACIÓN `/v2/gov/council/derive-vrf` (característica: gov_vrf)
  - Solicitud: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Pequeño", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamiento: ہر امیدوار کا prueba VRF `chain_id`, `epoch` اور تازہ ترین bloque de baliza hash سے مشتق entrada canónica کے خلاف verificar کرتا ہے؛ bytes de salida کو desc ترتیب میں desempates کے ساتھ sort کرتا ہے؛ miembros principales de `committee_size` واپس کرتا ہے۔ Persistir نہیں کرتا۔
  - Respuesta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk en G1, prueba en G2 (96 bytes). Pequeño = pk en G2, prueba en G1 (48 bytes). Entradas separadas por dominio ہیں اور `chain_id` شامل ہے۔

### Valores predeterminados de gobernanza (iroha_config `gov.*`)

Torii جب کوئی lista persistente نہ پائے تو consejo de respaldo `iroha_config` کے ذریعے parametrizar کیا جاتا ہے:

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

Anulaciones de entorno equivalentes:

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

`parliament_committee_size` miembros alternativos کی تعداد کو límite کرتا ہے جب consejo persistir نہ ہو، `parliament_term_blocks` longitud de época definir کرتا ہے جو derivación de semillas کے لئے استعمال ہوتا ہے (`epoch = floor(height / term_blocks)`), `parliament_min_stake` activo de elegibilidad پر کم از کم participación (unidades más pequeñas) hacer cumplir کرتا ہے، اور `parliament_eligibility_asset_id` منتخب کرتا ہے کہ candidatos بناتے وقت کس balance de activos کو escaneo کیا جائے۔Verificación de Gobernanza VK کا کوئی bypass نہیں: verificación de boleta ہمیشہ `Active` clave de verificación اور bytes en línea کا تقاضا کرتا ہے، اور entornos کو alterna solo de prueba پر انحصار نہیں کرنا چاہئے۔

RBAC
- Ejecución en cadena کے لئے permisos درکار ہیں:
  - Propuestas: `CanProposeContractDeployment{ contract_id }`
  - Boletas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgación: `CanEnactGovernance`
  - Dirección del consejo (futuro): `CanManageParliament`Espacios de nombres protegidos
- El parámetro personalizado `gov_protected_namespaces` (matriz JSON de cadenas) enumera los espacios de nombres میں implementar کے لئے puerta de admisión فعال کرتا ہے۔
- Los clientes utilizan espacios de nombres protegidos para implementar claves de metadatos de transacciones para implementar:
  - `gov_namespace`: espacio de nombres de destino (nombre: "aplicaciones")
  - `gov_contract_id`: espacio de nombres کے اندر ID de contrato lógico
- `gov_manifest_approvers`: matriz JSON opcional de ID de cuenta del validador۔ جب carril quórum manifiesto > 1 declarar کرے تو admisión کے لئے autoridad de transacción اور cuentas listadas دونوں درکار ہوتے ہیں تاکہ quórum manifiesto پورا ہو سکے۔
- Telemetría `governance_manifest_admission_total{result}` کے ذریعے contadores de admisión holísticos دکھاتی ہے تاکہ operadores کامیاب admite کو `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, y `runtime_hook_rejected` سے فرق کر سکیں۔
- Telemetría `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) کے ذریعے ruta de cumplimiento دکھاتی ہے تاکہ aprobaciones faltantes کا auditoría ہو سکے۔
- Lanes اپنے manifiesta میں شائع شدہ lista de espacios de nombres permitidos کو hacer cumplir کرتے ہیں۔ Este es el nombre `gov_namespace` y el espacio de nombres y el manifiesto `protected_namespaces`. سیٹ میں ہونا چاہئے۔ `RegisterSmartContractCode` envíos بغیر metadatos کے، جب protección habilitar ہو، rechazar ہوتے ہیں۔
- Admisión اس بات کو hacer cumplir کرتا ہے کہ `(namespace, contract_id, code_hash, abi_hash)` کے لئے Propuesta de gobernanza promulgada موجود ہو؛ ورنہ validación Error no permitido کے ساتھ falla ہوتا ہے۔Ganchos de actualización en tiempo de ejecución
- Los manifiestos de carril `hooks.runtime_upgrade` declaran کر سکتے ہیں تاکہ instrucciones de actualización del tiempo de ejecución (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) کو gate کیا جا سکے۔
- Campos de gancho:
  - `allow` (bool, predeterminado `true`): جب `false` ہو تو تمام las instrucciones de actualización del tiempo de ejecución rechazan ہو جاتے ہیں۔
  - `require_metadata` (bool, predeterminado `false`): `metadata_key` کے مطابق entrada de metadatos درکار ہے۔
  - `metadata_key` (cadena): gancho کا nombre de metadatos obligatorio۔ Predeterminado `gov_upgrade_id` Se requieren metadatos y lista de permitidos Más
  - `allowed_ids` (matriz de cadenas): valores de metadatos کی lista de permitidos opcional (recortar کے بعد)۔ اگر دیا گیا valor فہرست میں نہ ہو تو rechazar۔
- Gancho موجود ہو تو admisión de cola ٹرانزیکشن کے cola میں جانے سے پہلے política de metadatos aplicar کرتا ہے۔ Faltan metadatos, valores, lista de permitidos, valores deterministas, error no permitido دیتی ہیں۔
- Telemetría `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` کے ذریعے seguimiento de resultados کرتی ہے۔
- Hook کی تسکین کرنے والی ٹرانزیکشنز کو metadatos `gov_upgrade_id=<value>` (یا clave definida por manifiesto) شامل کرنا ہوگا، ساتھ ہی quórum manifiesto کے مطابق validadores کی aprobaciones بھی درکار ہیں۔Punto final de conveniencia
- POST `/v2/gov/protected-namespaces` - `gov_protected_namespaces` کو براہ راست nodo پر aplicar کرتا ہے۔
  - Solicitud: { "espacios de nombres": ["aplicaciones", "sistema"]}
  - Respuesta: { "ok": verdadero, "aplicado": 1 }
  - Notas: administrador/pruebas کے لئے ہے؛ Cómo configurar el token API درکار ہوگا۔ producción کے لئے `SetParameter(Custom)` کے ساتھ transacción firmada ترجیح دیں۔Ayudantes de CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - espacio de nombres کے instancias de contrato buscar کرتا ہے اور verificación cruzada کرتا ہے کہ:
    - Torii ہر `code_hash` کے لئے bytecode ذخیرہ کرتا ہے، اور اس کا Blake2b-32 digest `code_hash` سے match کرتا ہے۔
    - `/v2/contracts/code/{code_hash}` میں موجود manifiesto que coincide con los valores `code_hash` اور `abi_hash` رپورٹ کرتا ہے۔
    - `(namespace, contract_id, code_hash, abi_hash)` کے لئے propuesta de gobernanza promulgada موجود ہے جو اسی hash de ID de propuesta سے derivar ہوتا ہے جو nodo استعمال کرتا ہے۔
  - `results[]` کے ساتھ JSON رپورٹ دیتا ہے (problemas, resúmenes de manifiesto/código/propuesta) اور ایک لائن کا خلاصہ (اگر `--no-summary` نہ ہو)۔
  - Espacios de nombres protegidos کے auditoría یا flujos de trabajo de implementación controlados por la gobernanza کی تصدیق کے لئے مفید۔
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Espacios de nombres protegidos implementación کے لئے esqueleto de metadatos JSON دیتا ہے، جس میں opcional `gov_manifest_approvers` شامل ہیں تاکہ reglas de quórum manifiesto پوری ہوں۔
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` ہونے پر sugerencias de bloqueo لازم ہیں، اور فراہم کیے گئے کسی بھی sugerencias سیٹ میں `owner`, `amount` y `duration_blocks` شامل ہونا ضروری ہے۔
  - Valida los identificadores de cuentas canónicas, canonicaliza las sugerencias de anulación de 32 bytes y combina las sugerencias en `public_inputs_json` (con `--public <path>` para anulaciones adicionales).- El anulador se deriva del compromiso de prueba (entrada pública) más `domain_tag`, `chain_id` e `election_id`; `--nullifier` se valida con la prueba cuando se suministra.
  - ایک لائن خلاصہ اب determinista `fingerprint=<hex>` دکھاتا ہے جو codificado `CastZkBallot` سے deriva ہوتا ہے، ساتھ sugerencias decodificadas (`owner`, `amount`, `duration_blocks`, `direction` جب فراہم ہوں)۔
  - Respuestas CLI `tx_instructions[]` کو `payload_fingerprint_hex` اور campos decodificados کے ساتھ anotar کرتے ہیں تاکہ esqueleto de herramientas posteriores کو بغیر Norito decodificación کے verificar کر سکے۔
  - Sugerencias de bloqueo دینے سے nodo ZK boletas کے لئے `LockCreated`/`LockExtended` eventos emiten کر سکتا ہے جب circuito y valores exponen کرے۔
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` alias Banderas ZK کے ناموں کو mirror کرتے ہیں تاکہ scripting parity ہو۔
  - Salida de resumen `vote --mode zk` con huella digital de instrucción codificada y campos de votación legibles (`owner`, `amount`, `duration_blocks`, `direction`) جس سے firma سے پہلے فوری تصدیق ہو جاتی ہے۔Listado de instancias
- GET `/v2/gov/instances/{ns}` - espacio de nombres کے لئے instancias de contrato activas کی فہرست۔
  - Parámetros de consulta:
    - `contains`: `contract_id` کی subcadena کے مطابق filtro (distingue entre mayúsculas y minúsculas)
    - `hash_prefix`: `code_hash_hex` کے prefijo hexadecimal کے مطابق filtro (minúsculas)
    - `offset` (predeterminado 0), `limit` (predeterminado 100, máx. 10_000)
    - `order`: `cid_asc` (predeterminado), `cid_desc`, `hash_asc`, `hash_desc`
  - Respuesta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Ayudante de SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) یا `ToriiClient.list_governance_instances_typed("apps", ...)` (Python) ۔Desbloquear barrido (Operador/Auditoría)
- OBTENER `/v2/gov/unlocks/stats`
  - Respuesta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` سب سے حالیہ altura del bloque دکھاتا ہے جہاں barrido de cerraduras caducadas اور persist کئے گئے۔ `expired_locks_now` ان bloquear registros کو escaneo کر کے نکلتا ہے جن میں `expiry_height <= height_current` ہو۔
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
  - `BallotProof` JSON براہ راست قبول کر کے `CastZkBallot` esqueleto y کرتا ہے۔
  - Solicitud:
    {
      "autoridad": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "votación": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // Contenedor ZK1 یا H2* en base64
        "root_hint": null, // cadena hexadecimal de 32 bytes opcional (raíz de elegibilidad)
        "propietario": nulo, // ID de cuenta opcional y confirmación del propietario del circuito
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
    - Servidor opcional `root_hint`/`owner`/`nullifier` کو papeleta سے `CastZkBallot` کے `public_inputs_json` میں mapa کرتا ہے۔
    - Bytes de sobre کو carga útil de instrucciones کے لئے base64 میں دوبارہ codificar کیا جاتا ہے۔
    - جب Torii envío de boleta کرتا ہے تو `reason` بدل کر `submitted transaction` ہو جاتا ہے۔
    - یہ punto final صرف تب دستیاب ہے جب Función `zk-ballot` habilitada ہو۔Ruta de verificación de CastZkBallot
- `CastZkBallot` فراہم کردہ decodificación de prueba base64 کرتا ہے اور خالی یا خراب cargas útiles کو rechazar کرتا ہے (`BallotRejected` con `invalid or empty proof`).
- Referéndum anfitrión (`vk_ballot`) یا valores predeterminados de gobernanza سے resolución de clave de verificación de boleta کرتا ہے اور تقاضا کرتا ہے کہ registro موجود ہو، `Active` ہو، اور en línea bytes رکھتا ہو۔
- Bytes de clave de verificación almacenados کو `hash_vk` کے ساتھ دوبارہ hash کیا جاتا ہے؛ falta de coincidencia de compromiso ہو تو verificación سے پہلے ejecución روک دیا جاتا ہے تاکہ entradas de registro manipuladas سے بچا جا سکے (`BallotRejected` con `verifying key commitment mismatch`).
- Bytes de prueba `zk::verify_backend` کے ذریعے backend registrado کو despacho ہوتے ہیں؛ las transcripciones no válidas `BallotRejected` con `invalid proof` کے ساتھ ظاہر ہوتے ہیں اور la instrucción falla de manera determinista ہوتی ہے۔
- La prueba debe exponer un compromiso electoral y una raíz de elegibilidad como aportes públicos; la raíz debe coincidir con el `eligible_root` de la elección y el anulador derivado debe coincidir con cualquier sugerencia proporcionada.
- Las pruebas exitosas `BallotAccepted` emiten کرتے ہیں؛ anuladores duplicados, raíces de elegibilidad obsoletas, regresiones de bloqueo پھر بھی پہلے بیان کردہ motivos de rechazo دیتے ہیں۔

## Mal comportamiento del validador y consenso conjunto

### Flujo de trabajo de reducción y encarcelamientoEl validador de consenso پروٹوکول کی خلاف ورزی کرے تو Norito codificado por `Evidence` emite کرتا ہے۔ Carga útil en memoria `EvidenceStore` میں آتا ہے اور اگر پہلے نہ دیکھا گیا ہو تو Mapa `consensus_evidence` respaldado por WSV میں materialize ہو جاتا ہے۔ `sumeragi.npos.reconfig.evidence_horizon_blocks` (bloques `7200` predeterminados) سے پرانے ریکارڈ rechazar ہو جاتے ہیں تاکہ archivo limitado رہے، مگر rechazo کو operadores کے لئے registro کیا جاتا ہے۔ La evidencia dentro del horizonte también respeta `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el retraso de corte `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`); la gobernanza puede cancelar las sanciones con `CancelConsensusEvidencePenalty` antes de que se aplique la reducción.

Delitos reconocidos `EvidenceKind` سے mapa uno a uno ہوتے ہیں؛ discriminantes مستحکم ہیں اور modelo de datos کے ذریعے hacer cumplir ہوتے ہیں:

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

- **DoublePrepare/DoubleCommit** - validador نے اسی `(phase,height,view,epoch)` کے لئے متضاد hashes پر دستخط کئے۔
- **InvalidQc** - agregador نے ایسا cometer chismes de certificados کیا جس کی شکل comprobaciones deterministas میں fallar ہو (مثلا خالی mapa de bits del firmante) ۔
- **InvalidProposal** - líder نے ایسا بلاک propone کیا جو validación estructural میں falla ہو (مثلا regla de cadena cerrada توڑے)۔
- **Censura**: los recibos de envío firmados muestran una transacción que nunca se propuso ni se comprometió.

Los operadores deben inspeccionar las cargas útiles de las herramientas y retransmitirlas:- Torii: `GET /v2/sumeragi/evidence` y `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, y `... submit --evidence-hex <payload>`.

Gobernanza کو bytes de evidencia کو prueba canónica کے طور پر tratar کرنا چاہئے:

1. **La carga útil جمع کریں** اس کے expira ہونے سے پہلے۔ raw Norito bytes کو altura/ver metadatos کے ساتھ archivo کریں۔
2. **Etapa de penalización کریں** carga útil کو referéndum یا instrucción sudo میں incrustar کر کے (مثلا `Unregister::peer`)۔ Carga útil de ejecución کو دوبارہ validar کرتا ہے؛ evidencia mal formada یا obsoleta rechazar deterministamente ہوتی ہے۔
3. **Programa de topología de seguimiento کریں** تاکہ validador infractor فوراً واپس نہ آ سکے۔ عام fluye میں `SetParameter(Sumeragi::NextMode)` اور `SetParameter(Sumeragi::ModeActivationHeight)` lista actualizada کے ساتھ cola کئے جاتے ہیں۔
4. **نتائج auditoría کریں** `/v2/sumeragi/evidence` اور `/v2/sumeragi/status` کے ذریعے تاکہ evidencia contador بڑھے اور gobernanza نے eliminación نافذ کیا ہو۔

### Secuenciación por consenso conjunto

Consenso conjunto اس بات کی ضمانت دیتا ہے کہ validador saliente establecer bloque de límites finalizar کرے اس سے پہلے کہ نیا establecer proponer شروع کرے۔ Tiempo de ejecución جوڑی شدہ parámetros کے ذریعے یہ regla de aplicación کرتا ہے:- `SumeragiParameter::NextMode` اور `SumeragiParameter::ModeActivationHeight` کو **اسی بلاک** میں commit ہونا چاہیے۔ `mode_activation_height` کو actualización لانے والے بلاک کی altura سے estrictamente بڑا ہونا چاہیے، تاکہ کم از کم ایک بلاک retraso ملے۔
- `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) protección de configuración ہے جو transferencias sin demora کو روکتا ہے:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`) retrasa la reducción del consenso para que la gobernanza pueda cancelar las sanciones antes de que se apliquen.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Tiempo de ejecución, parámetros preparados de CLI, `/v2/sumeragi/params`, `iroha --output-format text ops sumeragi params`, ajustes de alturas de activación de operadores y listas de validadores. کر سکیں۔
- Automatización de la gobernanza کو ہمیشہ:
  1. evidencia پر مبنی remoción (یا reintegración) فیصلہ finalizar کرنا چاہیے۔
  2. `mode_activation_height = h_current + activation_lag_blocks` کے ساتھ cola de reconfiguración de seguimiento کرنا چاہیے۔
  3. `/v2/sumeragi/status` کی نگرانی کرنی چاہیے جب تک `effective_consensus_mode` متوقع altura پر interruptor نہ ہو جائے۔

جو بھی validadores de secuencias de comandos rotar کرے یا corte aplicar کرے اسے **activación con retardo cero** یا parámetros de transferencia کو omitir نہیں کرنا چاہیے؛ ایسی transacciones rechazadas ہو جاتی ہیں اور نیٹ ورک پچھلے modo میں رہتا ہے۔

## Superficies de telemetría- Exportación de actividad de gobernanza de métricas Prometheus کرتے ہیں:
  - Propuestas `governance_proposals_status{status}` (medidor) کی گنتی estado کے حساب سے pista کرتا ہے۔
  - `governance_protected_namespace_total{outcome}` (contador) اس وقت incrementar ہوتا ہے جب espacios de nombres protegidos کی admisión implementar کو permitir یا rechazar کرے۔
  - `governance_manifest_activations_total{event}` (contador) inserciones de manifiesto (`event="manifest_inserted"`) y enlaces de espacio de nombres (`event="instance_bound"`) registro کرتا ہے۔
- `/status` objeto `governance` y recuentos de propuestas y reflejo del informe de totales de espacios de nombres protegidos y activaciones de manifiesto recientes (espacio de nombres, ID de contrato, código/hash ABI, altura de bloque, activación marca de tiempo) lista کرتا ہے۔ Operadores اس campo کو encuesta کر کے تصدیق کر سکتے ہیں کہ promulgaciones نے manifiestos اپڈیٹ کئے اور puertas de espacios de nombres protegidos نافذ ہیں۔
- Plantilla Grafana (`docs/source/grafana_governance_constraints.json`) اور `telemetry.md` Runbook de telemetría دکھاتا ہے کہ propuestas estancadas, activaciones de manifiesto faltantes, یا actualizaciones de tiempo de ejecución کے دوران rechazos inesperados de espacios de nombres protegidos کے لئے alertas کیسے cable کئے جائیں۔