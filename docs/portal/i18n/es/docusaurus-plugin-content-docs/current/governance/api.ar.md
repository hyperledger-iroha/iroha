---
lang: es
direction: ltr
source: docs/portal/docs/governance/api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: مسودة/تصور لمرافقة مهام تنفيذ الحوكمة. قد تتغير الصيغ اثناء التنفيذ. الحتمية وسياسة RBAC قيود معيارية؛ يمكن لتوريي توقيع/ارسال المعاملات عندما يتم توفير `authority` و`private_key`, y والا يبني العملاء المعاملة ويقدمونها الى `/transaction`.نظرة عامة
- جميع نقاط النهاية تعيد JSON. Para obtener más información, consulte el artículo `tx_instructions`:
  - `wire_id`: معرّف السجل لنوع التعليمة
  - `payload_hex`: Conectores Norito (hexadecimal)
- اذا تم توفير `authority` و `private_key` (او `private_key` في DTOs votes), يقوم Torii بالتوقيع والارسال Aquí está `tx_instructions`.
- Haga clic en la autoridad SignedTransaction y chain_id en la cuenta de usuario y en el POST `/transaction`.
- Actualización SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` y `GovernanceProposalResult` (estado/tipo), y `ToriiClient.get_governance_referendum_typed` y `GovernanceReferendumResult` y `ToriiClient.get_governance_tally_typed` `GovernanceTally`, `ToriiClient.get_governance_locks_typed`, `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed`, `GovernanceUnlockStats`, y `ToriiClient.list_governance_instances_typed`. `GovernanceInstancesPage`, presione y escriba el archivo README.
- Para Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` y `ToriiClient.enact_proposal`, escriba `GovernanceInstructionDraft` (descargado por `tx_instructions`) Torii), haga clic en el archivo JSON para finalizar/promulgar.- JavaScript (`@iroha/iroha-js`): `ToriiClient` Los ayudantes tecleados pueden desbloquear, desbloquear y bloquear. `listGovernanceInstances(namespace, options)` del consejo de نقاط نهاية (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) de Node.js Los dispositivos `/v2/gov/instances/{ns}` y los dispositivos VRF están conectados a dispositivos electrónicos.

نقاط النهاية

- PUBLICACIÓN `/v2/gov/proposals/deploy-contract`
  - الطلب (JSON):
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
  - الرد (JSON):
    { "ok": verdadero, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - التحقق: تقوم العقد بتوحيد `abi_hash` لنسخة `abi_version` المقدمة وترفض عدم التطابق. Aquí está `abi_version = "v1"`.API العقود (implementación)
- PUBLICACIÓN `/v2/contracts/deploy`
  - الطلب: { "autoridad": "i105...", "private_key": "...", "code_b64": "..." }
  - Contenido: `code_hash` de la versión IVM y `abi_hash` de la versión `abi_version` de la versión `abi_version`. `RegisterSmartContractCode` (manifiesto) y `RegisterSmartContractBytes` (بايتات `.to` كاملة) نيابة عن `authority`.
  - الرد: { "ok": verdadero, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - ذي صلة:
    - GET `/v2/contracts/code/{code_hash}` -> manifiesto de يعيد المخزن
    - OBTENER `/v2/contracts/code-bytes/{code_hash}` -> Siguiente `{ code_b64 }`
- PUBLICACIÓN `/v2/contracts/instance`
  - الطلب: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - السلوك: ينشر البايتات المقدمة ويُفعل فورا ربط `(namespace, contract_id)` عبر `ActivateContractInstance`.
  - الرد: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }خدمة الاسماء المستعارة
- PUBLICACIÓN `/v2/aliases/voprf/evaluate`
  - الطلب: { "blinded_element_hex": "..." }
  - Contenido: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` يعكس تنفيذ المقيم. القيمة الحالية: `blake2b512-mock`.
  - ملاحظات: مقيم mock حتمي يطبق Blake2b512 مع فصل مجال `iroha.alias.voprf.mock.v1`. Los dispositivos de seguridad están conectados a VOPRF como Iroha.
  - Contenido: HTTP `400` Es un código hexadecimal. Hay Torii y Norito `ValidationFail::QueryFailed::Conversion` para un decodificador.
- PUBLICACIÓN `/v2/aliases/resolve`
  - الطلب: { "alias": "GB82 OESTE 1234 5698 7654 32" }
  - الرد: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - ملاحظات: يتطلب تشغيل Puente ISO (`[iso_bridge.account_aliases]` في `iroha_config`). يقوم Torii بتطبيع الاسماء عبر ازالة الفراغات وتحويلها الى احرف كبيرة قبل البحث. Hay 404 opciones de tiempo de ejecución en el puente ISO.
- PUBLICACIÓN `/v2/aliases/resolve_index`
  - الطلب: { "índice": 0 }
  - الرد: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - ملاحظات: مؤشرات الاسماء تعين بشكل حتمي حسب ترتيب التكوين (basado en 0). يمكن للعملاء تخزين الردود fuera de línea لبناء مسارات تدقيق لاحداث atestación الخاصة بالاسماء.حد حجم الكود
- Contenido del archivo: `max_contract_code_bytes` (JSON u64)
  - يتحكم في الحد الاقصى المسموح (بالبايت) لتخزين كود العقود على السلسلة.
  - Memoria: 16 MiB. ترفض العقد `RegisterSmartContractBytes` عندما يتجاوز حجم صورة `.to` الحد مع خطا انتهاك invariante.
  - مكن للمشغلين التعديل عبر `SetParameter(Custom)` مع `id = "max_contract_code_bytes"` وحمولة رقمية.

- PUBLICACIÓN `/v2/gov/ballots/zk`
  - الطلب: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - الرد: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - ملاحظات:
    - عندما تضمن المدخلات العامة للدائرة `owner` و`amount` و`duration_blocks`، وتتحقق البرهان من VK المضبوطة، Asegúrese de que la cerradura esté cerrada desde `election_id` hasta `owner`. تبقى الاتجاهات مخفية (`unknown`)؛ ويتم تحديث monto/vencimiento فقط. اعادات التصويت monótono: cantidad y vencimiento تزداد فقط (تطبق العقدة max(cantidad, cantidad anterior) y max(expiración, vencimiento anterior)).
    - اعادات التصويت ZK التي تحاول تقليل cantidad y vencimiento يتم رفضها من جهة الخادم مع تشخيصات `BallotRejected`.
    - يجب على تنفيذ العقد استدعاء `ZK_VOTE_VERIFY_BALLOT` قبل ادراج `SubmitBallot`; ويفرض المضيفون pestillo لمرة واحدة.- PUBLICACIÓN `/v2/gov/ballots/plain`
  - الطلب: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sí|No|Abstain" }
  - الرد: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }
  - ملاحظات: اعادات التصويت تمتد فقط - لا يمكن لbalot جديد تقليل monto او vencimiento لقفل موجود. يجب ان يساوي `owner` سلطة المعاملة. الحد الادنى للمدة هو `conviction_step_blocks`.- PUBLICACIÓN `/v2/gov/finalize`
  - الطلب: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - الاثر على السلسلة (الهيكل الحالي): تنفيذ اقتراح desplegar معتمد يدرج `ContractManifest` ادنى بمفتاح `code_hash` مع `abi_hash` المتوقع ويضع الاقتراح بحالة Promulgado. Hay un manifiesto que incluye `code_hash` y `abi_hash`, que es un error.
  - ملاحظات:
    - لانتخابات ZK, يجب على مسارات العقد استدعاء `ZK_VOTE_VERIFY_TALLY` قبل تنفيذ `FinalizeElection`; ويفرض المضيفون pestillo لمرة واحدة. يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء tally للانتخابات.
    - الاغلاق التلقائي عند `h_end` يصدر Aprobado/Rechazado فقط للاستفتاءات Normal؛ تبقى استفتاءات ZK Closed حتى يتم ارسال tally منته ويجري تنفيذ `FinalizeReferendum`.
    - فحوصات participación تستخدم aprobar+rechazar فقط؛ abstenerse لا يحتسب ضمن participación.- PUBLICACIÓN `/v2/gov/enact`
  - الطلب: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - ملاحظات: يقدم Torii المعاملة الموقعة عندما تتوفر `authority`/`private_key`; والا يعيد هيكلا لتوقيع العملاء وارساله. الـ preimagen اختيارية وحاليا معلوماتية.

- OBTENER `/v2/gov/proposals/{id}`
  - Texto `{id}`: Texto hexadecimal (64 caracteres)
  - الرد: { "encontrado": bool, "propuesta": {...}? }

- OBTENER `/v2/gov/locks/{rid}`
  - Mensaje `{rid}`: cadena de mensajes de texto
  - الرد: { "found": bool, "referendum_id": "rid", "locks": {...}? }

- OBTENER `/v2/gov/council/current`
  - الرد: { "época": N, "miembros": [{ "account_id": "..." }, ...] }
  - ملاحظات: يعيد consejo المحفوظ اذا كان موجودا؛ والا يشتق بديلا حتميا باستخدام اصل share المضبوط والعتبات (يعكس مواصفات VRF حتى تثبت ادلة VRF الحية على السلسلة).- PUBLICACIÓN `/v2/gov/council/derive-vrf` (característica: gov_vrf)
  - الطلب: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Pequeño", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - السلوك: يتحقق من برهان VRF لكل مرشح مقابل المدخل القانوني المشتق من `chain_id` و`epoch` ومنارة اخر hash للبلوك؛ يرتب حسب bytes الخرج desc مع كاسرات تعادل؛ Aquí está `committee_size` desde aquí. لا يتم الحفظ.
  - الرد: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Configuración: Normal = pk a G1, prueba a G2 (96 bytes). Pequeño = pk para G2, prueba para G1 (48 bytes). Utilice el cable de alimentación `chain_id`.

### افتراضات الحوكمة (iroha_config `gov.*`)

يتم ضبط consejo الاحتياطي الذي يستخدمه Torii عندما لا يوجد lista محفوظ عبر `iroha_config`:

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

بدائل البيئة المكافئة:

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

`parliament_committee_size` يحد عدد اعضاء respaldo المعادين عندما لا يوجد consejo محفوظ، و`parliament_term_blocks` يحدد طول الحقبة المستخدمة لاشتقاق semilla (`epoch = floor(height / term_blocks)`), و`parliament_min_stake` يفرض الحد الادنى من estaca (بوحدات صغرى) على اصل الاهلية, و`parliament_eligibility_asset_id` يحدد اي رصيد اصل يتم مسحه عند بناء مجموعة المرشحين.

تحقق VK للحوكمة بلا bypass: التحقق من ballots يتطلب دائما مفتاح تحقق `Active` ببايتات inline, y ولا يجب ان تعتمد البيئات على تبديلات اختبارية لتخطي التحقق.RBAC
- التنفيذ على السلسلة يتطلب صلاحيات:
  - Propuestas: `CanProposeContractDeployment{ contract_id }`
  - Boletas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgación: `CanEnactGovernance`
  - Dirección del consejo (مستقبلا): `CanManageParliament`

Espacios de nombres
- El comando `gov_protected_namespaces` (matriz JSON de cadenas) permite implementar espacios de nombres en la puerta.
- Para implementar metadatos en la implementación de espacios de nombres:
  - `gov_namespace`: espacio de nombres الهدف (مثلا "aplicaciones")
  - `gov_contract_id`: Espacio de nombres disponible en el espacio de nombres
- `gov_manifest_approvers`: matriz JSON que contiene ID de cuenta. عندما يعلن manifiesto لمسار ما quorum اكبر من واحد، يتطلب admisión سلطة المعاملة بالاضافة الى الحسابات المدرجة لتلبية quorum الخاص بالmanifest.
- تكشف التليمترية عدادات admisión عبر `governance_manifest_admission_total{result}` لتمييز القبولات الناجحة عن مسارات `missing_manifest` و`non_validator_authority` `quorum_rejected`, `protected_namespace_rejected` y `runtime_hook_rejected`.
- تكشف التليمترية مسار aplicación de la ley عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) حتى يتمكن المشغلون من تدقيق الموافقات المفقودة.
- Lista de permitidos de espacios de nombres y manifiestos. Aquí está el `gov_namespace` y el `gov_contract_id` y el espacio de nombres del `protected_namespaces` en el manifiesto. يتم رفض ارسال `RegisterSmartContractCode` بدون هذه metadatos عندما تكون الحماية مفعلة.
- يفرض admisión وجود اقتراح حوكمة Promulgado للتركيبة `(namespace, contract_id, code_hash, abi_hash)`؛ والا تفشل عملية التحقق بخطا NotPermitted.Ganchos en tiempo de ejecución
- يمكن لـ manifests الخاصة بالمسار اعلان `hooks.runtime_upgrade` لبوابة تعليمات ترقية runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- حقول gancho:
  - `allow` (bool, الافتراضي `true`): عندما تكون `false` يتم رفض جميع تعليمات ترقية runtime.
  - `require_metadata` (bool, الافتراضي `false`): يتطلب مدخل metadata المحدد بواسطة `metadata_key`.
  - `metadata_key` (cadena): اسم metadata الذي يفرضه gancho. Aquí `gov_upgrade_id` incluye metadatos y una lista de permitidos.
  - `allowed_ids` (matriz de cadenas): lista blanca de metadatos (sin recortar). يرفض عندما لا تكون القيمة المقدمة مدرجة.
- عندما يكون gancho موجودا، يفرض admisión في الطابور سياسة metadatos قبل دخول المعاملة للطابور. metadatos de la lista de permitidos y de la lista de permitidos no permitidos.
- تتابع التليمترية النتائج عبر `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- المعاملات التي تفي بالhook يجب ان تتضمن metadatos `gov_upgrade_id=<value>` (او المفتاح المحدد في manifest) مع اي موافقات مدققين مطلوبة بواسطة quorum الخاص بالmanifest.

Punto final للراحة
- POST `/v2/gov/protected-namespaces` - يطبق `gov_protected_namespaces` مباشرة على العقدة.
  - الطلب: { "espacios de nombres": ["aplicaciones", "sistema"] }
  - الرد: { "ok": verdadero, "aplicado": 1 }
  - ملاحظات: مخصص للادارة/الاختبار؛ Este token API es un token API. Asegúrese de que el dispositivo esté conectado a `SetParameter(Custom)`.CLI مساعدات
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - يجلب مثيلات العقود للnamespace y يتحقق من:
    - Torii es un código de bytes para `code_hash` y un resumen de Blake2b-32 para `code_hash`.
    - manifiesto المخزن تحت `/v2/contracts/code/{code_hash}` يبلغ بقيم `code_hash` و`abi_hash` متطابقة.
    - يوجد اقتراح حوكمة promulgado للتركيبة `(namespace, contract_id, code_hash, abi_hash)` مشتق بنفس hash propuesta-id الذي تستخدمه العقدة.
  - يخرج تقرير JSON يحتوي `results[]` لكل عقد (problemas, manifiesto/código/propuesta de manifiesto) بالاضافة الى ملخص سطر واحد الا اذا تم تعطيله (`--no-summary`).
  - Permite implementar espacios de nombres y realizar implementaciones.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Para implementar metadatos JSON y desplegar espacios de nombres, utilice el manifiesto de quórum `gov_manifest_approvers`.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — تلميحات القفل مطلوبة عندما يكون `min_bond_amount > 0`, وأي مجموعة تلميحات مقدمة يجب أن تضمن `owner`, `amount` y `duration_blocks`.
  - Valida los identificadores de cuentas canónicas, canonicaliza las sugerencias de anulación de 32 bytes y combina las sugerencias en `public_inputs_json` (con `--public <path>` para anulaciones adicionales).
  - El anulador se deriva del compromiso de prueba (entrada pública) más `domain_tag`, `chain_id` e `election_id`; `--nullifier` se valida con la prueba cuando se suministra.- الملخص في سطر واحد يعرض الان `fingerprint=<hex>` حتميا مشتقا من `CastZkBallot` المشفر مع sugerencias المفككة (`owner`, `amount`, `duration_blocks`, `direction`.
  - Haga clic en CLI para conectar `tx_instructions[]` o `payload_fingerprint_hex`. Esta es la versión Norito.
  - توفير sugerencias للـ bloqueo يسمح للعقدة باصدار احداث `LockCreated`/`LockExtended` للـ ZK boletas بمجرد ان تكشف الدائرة القيم نفسها.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Los alias `--lock-amount`/`--lock-duration-blocks` son banderas y etiquetas ZK.
  - ناتج الملخص يعكس `vote --mode zk` باضافة huella digital للتعليمة المشفرة وحقول boleta المقروءة (`owner`, `amount`, `duration_blocks`, `direction`), لتاكيد سريع قبل توقيع الهيكل.قائمة المثيلات
- GET `/v2/gov/instances/{ns}` - يسرد مثيلات العقود النشطة لnamespace.
  - Parámetros de consulta:
    - `contains`: subcadena diferente de `contract_id` (distingue entre mayúsculas y minúsculas)
    - `hash_prefix`: texto hexadecimal en `code_hash_hex` (minúsculas)
    - `offset` (predeterminado 0), `limit` (predeterminado 100, máx. 10_000)
    - `order`: compatible con `cid_asc` (predeterminado), `cid_desc`, `hash_asc`, `hash_desc`
  - الرد: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) y `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

مسح desbloquea (المشغل/التدقيق)
- OBTENER `/v2/gov/unlocks/stats`
  - الرد: { "height_current": H, "expired_locks_now": n, "referendo_with_expired": m, "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` يعكس اخر ارتفاع بلوك تم فيه مسح locks منتهية وتخزينها. `expired_locks_now` يحسب عبر مسح سجلات lock ذات `expiry_height <= height_current`.
- PUBLICACIÓN `/v2/gov/ballots/zk-v1`
  - الطلب (DTO نمط v1):
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
  - الرد: { "ok": verdadero, "aceptado": verdadero, "tx_instructions": [{...}] }- PUBLICACIÓN `/v2/gov/ballots/zk-v1/ballot-proof` (característica: `zk-ballot`)
  - El archivo JSON `BallotProof` se actualiza y se ejecuta en `CastZkBallot`.
  - الطلب:
    {
      "autoridad": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "clave_privada": "...?",
      "election_id": "ref-1",
      "votación": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 para ZK1 y H2*
        "root_hint": null, // cadena hexadecimal de 32 bytes opcional (raíz de elegibilidad)
        "propietario": nulo, // AccountId اختياري عندما تلتزم الدائرة بـ propietario
        "nullifier": null // cadena hexadecimal de 32 bytes opcional (pista de anulación)
      }
    }
  - الرد:
    {
      "bien": cierto,
      "aceptado": verdadero,
      "razón": "construir esqueleto de transacción",
      "tx_instrucciones": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - ملاحظات:
    - يقوم الخادم بربط `root_hint`/`owner`/`nullifier` الاختيارية من ballot الى `public_inputs_json` لـ `CastZkBallot`.
    - Necesita bytes para el sobre en base64 y la carga útil.
    - تتغير `reason` الى `submitted transaction` عندما يقدم Torii boleta.
    - Este punto final tiene la función `zk-ballot`.مسار التحقق من CastZkBallot
- `CastZkBallot` es una base de datos base64 y una base de datos (`BallotRejected` o `invalid or empty proof`).
- المضيف يحل مفتاح التحقق للballot من referendum (`vk_ballot`) او افتراضات الحوكمة ويتطلب ان يكون السجل موجودا Hay `Active` y bytes en línea.
- bytes de hash de `hash_vk`; اي عدم تطابق للالتزام يوقف التنفيذ قبل التحقق للحماية من ادخالات سجل معبث بها (`BallotRejected` مع `verifying key commitment mismatch`).
- bytes del backend del servidor `zk::verify_backend`; Conecte el conector `BallotRejected` al `invalid proof` y conecte el dispositivo.
- La prueba debe exponer un compromiso electoral y una raíz de elegibilidad como aportes públicos; la raíz debe coincidir con el `eligible_root` de la elección y el anulador derivado debe coincidir con cualquier sugerencia proporcionada.
- البراهين الناجحة تصدر `BallotAccepted`; anuladores المكررة او جذور اهلية قديمة او تراجعات lock تستمر في انتاج اسباب الرفض المذكورة سابقا في هذا المستند.

## سوء سلوك المدققين والتوافق المشترك

### سير عمل corte y encarcelamientoيصدر الاجماع `Evidence` مرمزا بـ Norito عندما ينتهك مدقق البروتوكول. Utilice el controlador `EvidenceStore` para conectar el controlador `consensus_evidence` al controlador `consensus_evidence`. WSV. يتم رفض السجلات الاقدم من `sumeragi.npos.reconfig.evidence_horizon_blocks` (الافتراضي `7200` بلوك) كي يبقى الارشيف محدودا، لكن الرفض يسجل للمشغلين. La evidencia dentro del horizonte también respeta `sumeragi.npos.reconfig.activation_lag_blocks` (predeterminado `1`) y el retraso de corte `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`); la gobernanza puede cancelar las sanciones con `CancelConsensusEvidencePenalty` antes de que se aplique la reducción.

الانتهاكات المعترف بها تقابل واحدا لواحد مع `EvidenceKind`; المميزات ثابتة ويفرضها نموذج البيانات:

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

- **DoublePrepare/DoubleCommit** - Los hashes de la tupla `(phase,height,view,epoch)`.
- **InvalidQc** - قام مجمع ببث commit certificado شكله يفشل الفحوص الحتمية (مثلا bitmap موقعين فارغ).
- **Propuesta no válida** - قدم قائد بلوكا يفشل التحقق البنيوي (مثلا يكسر قاعدة cadena cerrada).
- **Censura**: los recibos de envío firmados muestran una transacción que nunca se propuso ni se comprometió.

يمكن للمشغلين والادوات فحص واعادة بث الحمولات عبر:

- Torii: `GET /v2/sumeragi/evidence` y `GET /v2/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, y `... submit --evidence-hex <payload>`.

يجب على الحوكمة اعتبار bytes الخاصة بالevidence دليلا قانونيا:1. **جمع الحمولة** قبل ان تتقادم. ارشفة bytes Norito contiene metadatos de altura/vista.
2. **تجهيز العقوبة** عبر تضمين الحمولة في referendum او تعليمة sudo (مثل `Unregister::peer`). تعيد عملية التنفيذ التحقق من الحمولة؛ evidencia المشوهة او القديمة ترفض حتميا.
3. **جدولة طوبولوجيا المتابعة** حتى لا يتمكن المدقق المخالف من العودة فورا. Los nombres de las listas son `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)`.
4. **تدقيق النتائج** عبر `/v2/sumeragi/evidence` و`/v2/sumeragi/status` لضمان ان عداد evidencia تقدم وان الحوكمة طبقت الازالة.

### تسلسل الاجماع المشترك

يضمن الاجماع المشترك ان تقوم مجموعة المدققين الخارجة بانهاء بلوك الحد قبل ان تبدا المجموعة الجديدة بالاقتراح. يفرض الـ runtime القاعدة عبر معاملات مزدوجة:

- يجب ان يتم الالتزام بـ `SumeragiParameter::NextMode` و `SumeragiParameter::ModeActivationHeight` في **نفس البلوك**. Hay un `mode_activation_height` que está conectado a una computadora y que tiene un retraso.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) هو guardia تهيئة يمنع traspasos بدون retraso:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (predeterminado `259200`) retrasa la reducción del consenso para que la gobernanza pueda cancelar las sanciones antes de que se apliquen.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```- El tiempo de ejecución y la CLI en etapas de `/v2/sumeragi/params` y `iroha --output-format text ops sumeragi params` son compatibles con los sistemas operativos. وقوائم المدققين.
- يجب ان تقوم اتمتة الحوكمة دائما بما يلي:
  1. انهاء قرار الازالة (او الاستعادة) المدعوم بـ evidencia.
  2. جدولة اعادة تهيئة متابعة مع `mode_activation_height = h_current + activation_lag_blocks`.
  3. مراقبة `/v2/sumeragi/status` حتى يتبدل `effective_consensus_mode` عند الارتفاع المتوقع.

اي سكربت يدير تدوير المدققين او يطبق slashing **يجب الا** يحاول تفعيل بدون lag او يحذف معاملات hand-off؛ يتم رفض تلك المعاملات وتترك الشبكة على الوضع السابق.

## اسطح التليمترية

- Nombre del usuario Prometheus:
  - `governance_proposals_status{status}` (medidor) يتتبع تعداد المقترحات حسب الحالة.
  - `governance_protected_namespace_total{outcome}` (contador) يزيد عندما يسمح او يرفض admisión لنشر ضمن espacios de nombres محمية.
  - `governance_manifest_activations_total{event}` (contador) يسجل ادخالات manifiesto (`event="manifest_inserted"`) y espacio de nombres (`event="instance_bound"`).
- `/status` muestra el `governance` muestra los espacios de nombres y muestra el manifiesto الاخيرة (espacio de nombres, ID de contrato, código/hash ABI, altura del bloque, marca de tiempo de activación). يمكن للمشغلين استطلاع هذا الحقل لتاكيد ان promulgaciones, manifiestos y espacios de nombres de nombres.
- قالب Grafana (`docs/source/grafana_governance_constraints.json`) y runbook التليمترية في `telemetry.md` يوضحان كيفية ربط التنبيهات للمقترحات العالقة، وتفعيلات manifest المفقودة، او رفضات namespaces المحمية غير المتوقعة اثناء ترقيات runtime.