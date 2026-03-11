---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

الحالة: مسودة/تصور لمرافقة مهام تنفيذ الحوكمة. Isso é o que acontece. RBAC e RBAC Você pode usar o produto `authority` e `private_key`, e não usar O código é `/transaction`.

نظرة عامة
- Não é possível usar o JSON. لمسارات انتاج المعاملات, تتضمن الردود `tx_instructions` - مصفوفة من تعليمة هيكلية واحدة او اكثر:
  - `wire_id`: معرّف السجل لنوع التعليمة
  - `payload_hex`: Norito (hex)
- Use `authority` e `private_key` (e `private_key` em cédulas DTOs), ou Torii بالتوقيع والارسال Eu estou usando `tx_instructions`.
- خلاف ذلك, يجمع العملاء SignedTransaction باستخدام autoridade echain_id الخاصة بهم, ثم يوقعون ويرسلون POST الى `/transaction`.
- SDK do SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` ou `GovernanceProposalResult` (status/tipo de status), e `ToriiClient.get_governance_referendum_typed` ou `GovernanceReferendumResult`, O `ToriiClient.get_governance_tally_typed` é o `GovernanceTally`, o `ToriiClient.get_governance_locks_typed` é o `GovernanceLocksResult`, o `ToriiClient.get_governance_unlock_stats_typed` é o `GovernanceUnlockStats`, و`ToriiClient.list_governance_instances_typed` يعيد `GovernanceInstancesPage`, فارضا وصولا بنمط typed عبر سطح الحوكمة مع امثلة استخدام في README.
- O código Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` é o mesmo `GovernanceInstructionDraft` digitado (ou seja, `tx_instructions` Em Torii), para que o JSON seja definido como Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` Ajudantes digitados للمقترحات, والاستفتاءات, وtallies, وlocks, واحصاءات unlock, والان `listGovernanceInstances(namespace, options)` é um conselho municipal (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) no Node.js. Verifique o `/v1/gov/instances/{ns}` e verifique se o VRF está funcionando corretamente.

نقاط النهاية

-POSTO `/v1/gov/proposals/deploy-contract`
  - Nome (JSON):
    {
      "namespace": "aplicativos",
      "contract_id": "meu.contrato.v1",
      "code_hash": "blake2b32:..." | "...64 hexadecimal",
      "abi_hash": "blake2b32:..." | "...64 hexadecimal",
      "abi_versão": "1",
      "janela": { "inferior": 12345, "superior": 12400 },
      "autoridade": "i105…?",
      "chave_privada": "...?"
    }
  - Nome (JSON):
    { "ok": verdadeiro, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - التحقق: تقوم العقد بتوحيد `abi_hash` لنسخة `abi_version` المقدمة وترفض عدم التطابق. O código `abi_version = "v1"` é o `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

Função da API (implantar)
-POSTO `/v1/contracts/deploy`
  - Nome: { "autoridade": "i105...", "private_key": "...", "code_b64": "..." }
  - Nome: `code_hash` de جسم برنامج IVM e `abi_hash` de ترويسة `abi_version`, ثم يرسل `RegisterSmartContractCode` (manifesto) e `RegisterSmartContractBytes` (ou seja, `.to`) não em `authority`.
  - Palavra: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - É isso:
    - GET `/v1/contracts/code/{code_hash}` -> يعيد manifesto المخزن
    - GET `/v1/contracts/code-bytes/{code_hash}` -> Obter `{ code_b64 }`
-POSTO `/v1/contracts/instance`
  - Nome: { "autoridade": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - السلوك: ينشر البايتات المقدمة ويُفعل فورا ربط `(namespace, contract_id)` عبر `ActivateContractInstance`.
  - الرد: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

خدمة الاسماء المستعارة
-POSTO `/v1/aliases/voprf/evaluate`
  - Nome: { "blinded_element_hex": "..." }
  - Nome: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` não é compatível. Nome do código: `blake2b512-mock`.
  - ملاحظات: مقيم mock حتمي يطبق Blake2b512 مع فصل مجال `iroha.alias.voprf.mock.v1`. Você pode usar o VOPRF para Iroha.
  - Código: HTTP `400` não é hexadecimal. O Torii é o Norito `ValidationFail::QueryFailed::Conversion` que pode ser usado para decodificar.
-POSTO `/v1/aliases/resolve`
  - Nome: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Nome: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Modelo: Ponte ISO instalada (`[iso_bridge.account_aliases]` para `iroha_config`). O Torii pode ser usado para remover o erro e o erro. 404 عندما يكون الاسم غير موجود و503 عندما يكون runtime الخاص بـ ISO bridge معطلا.
-POSTO `/v1/aliases/resolve_index`
  - Nome: { "índice": 0 }
  - Nome: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - ملاحظات: مؤشرات الاسماء تعين بشكل حتمي حسب ترتيب التكوين (baseado em 0). يمكن للعملاء تخزين الردود offline لبناء مسارات تدقيق لاحداث atestado الخاصة بالاسماء.حد حجم الكود
- Código de configuração: `max_contract_code_bytes` (JSON u64)
  - يتحكم في الحد الاقصى المسموح (بالبايت) لتخزين كود العقود على السلسلة.
  - Capacidade: 16 MiB. O `RegisterSmartContractBytes` é definido como invariante.
  - Coloque o `SetParameter(Custom)` no lugar do `id = "max_contract_code_bytes"` e instale-o.

-POSTO `/v1/gov/ballots/zk`
  - الطلب: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - الرد: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Como:
    - عندما تتضمن المدخلات العامة للدائرة `owner` و`amount` و`duration_blocks`, وتتحقق البرهان من VK Você pode usar um bloqueio de bloqueio para `election_id` ou `owner`. تبقى الاتجاهات مخفية (`unknown`), ويتم تحديث quantidade/validade é. É monotônico: quantidade e expiração é igual a (quantidade anterior, expiração) e max(expiry, prev.expiry)).
    - اعادات التصويت ZK التي تحاول تقليل quantidade e expiração يتم رفضها من جهة الخادم مع تشخيصات `BallotRejected`.
    - يجب على تنفيذ العقد استدعاء `ZK_VOTE_VERIFY_BALLOT` é o mesmo que `SubmitBallot`; ويفرض المضيفون trava لمرة واحدة.

-POSTO `/v1/gov/ballots/plain`
  - الطلب: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sim | Não | Abstenção" }
  - الرد: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - ملاحظات: اعادات التصويت تمتد فقط - لا يمكن لballot جديد تقليل quantidade e expiração لقفل موجود. Não use `owner` para evitar problemas. O código é `conviction_step_blocks`.

-POSTO `/v1/gov/finalize`
  - Nome: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - الرد: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - الاثر على السلسلة (الهيكل الحالي): تنفيذ اقتراح implantar معتمد يدرج `ContractManifest` ادنى بمفتاح `code_hash` مع `abi_hash` المتوقع ويضع الاقتراح بحالة Promulgado. Este manifesto pode ser `code_hash` ou `abi_hash`, mas não é possível.
  - Como:
    - لانتخابات ZK, يجب على مسارات العقد استدعاء `ZK_VOTE_VERIFY_TALLY` قبل تنفيذ `FinalizeElection`; ويفرض المضيفون trava لمرة واحدة. يرفض `FinalizeReferendum` الاستفتاءات ZK حتى يتم انهاء registro للانتخابات.
    - الاغلاق التلقائي عند `h_end` يصدر Aprovado/Rejeitado فقط للاستفتاءات Simples; تبقى استفتاءات ZK Closed حتى يتم ارسال tally منته ويجري تنفيذ `FinalizeReferendum`.
    - فحوصات comparecimento تستخدم aprovar + rejeitar فقط؛ abster-se de aumentar a participação.

-POSTO `/v1/gov/enact`
  - Nome: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Nome: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Método: يقدم Torii المعاملة الموقعة عندما تتوفر `authority`/`private_key`; Não há nada de errado com isso. A pré-imagem é exibida e exibida.

- OBTER `/v1/gov/proposals/{id}`
  - Código `{id}`: Código hexadecimal (64 caracteres)
  - الرد: { "encontrado": bool, "proposta": { ... }? }

- OBTER `/v1/gov/locks/{rid}`
  - المسار `{rid}`: string لمعرف الاستفتاء
  - الرد: { "encontrado": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTER `/v1/gov/council/current`
  - Nome: { "época": N, "membros": [{ "account_id": "..." }, ...] }
  - ملاحظات: يعيد conselho المحفوظ اذا كان موجودا؛ والا يشتق بديلا حتميا باستخدام اصل estaca المضبوط والعتبات (يعكس مواصفات VRF حتى تثبت ادلة VRF الحية على السلسلة).

- POST `/v1/gov/council/derive-vrf` (recurso: gov_vrf)
  - Nome: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - السلوك: يتحقق من برهان VRF لكل مرشح مقابل المدخل القانوني المشتق من `chain_id` و`epoch` ومنارة اخر hash للبلوك؛ يرتب حسب bytes الخرج desc مع كاسرات تعادل؛ Verifique o `committee_size` do site. Não é isso.
  - الرد: { "época": N, "membros": [{ "account_id": "..." } ...], "total_candidates": M, "verificado": K }
  - Método: Normal = pk para G1, prova para G2 (96 bytes). Pequeno = pk para G2, prova para G1 (48 bytes). O código de barras é `chain_id`.

### Definições de segurança (iroha_config `gov.*`)

يتم ضبط Council الاحتياطي الذي يستخدمه Torii عندما لا يوجد roster محفوظ عبر `iroha_config`:

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
````parliament_committee_size` يحد عدد اعضاء fallback المعادين عندما لا يوجد Council محفوظ, و`parliament_term_blocks` يحدد طول الحقبة O valor da semente (`epoch = floor(height / term_blocks)`) e `parliament_min_stake` é uma opção para a estaca (بوحدات صغرى) على اصل O software `parliament_eligibility_asset_id` não pode ser usado para evitar problemas.

تحقق VK للحوكمة بلا bypass: التحقق من cédulas يتطلب دائما مفتاح تحقق `Active` ببايتات inline, ولا Não se preocupe, você pode usar o software de controle de acesso.

RBAC
- التنفيذ على السلسلة يتطلب صلاحيات:
  - Propostas: `CanProposeContractDeployment{ contract_id }`
  - Cédulas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgação: `CanEnactGovernance`
  - Gestão do conselho (مستقبلا): `CanManageParliament`

Namespaces
- O código `gov_protected_namespaces` (matriz JSON com strings) é usado para implantar namespaces de gating.
- على العملاء تضمين مفاتيح metadata للمعاملة عند implantar namespaces المحمية:
  - `gov_namespace`: namespace الهدف (como "apps")
  - `gov_contract_id`: معرف العقد المنطقي داخل namespace
- `gov_manifest_approvers`: array JSON criado com IDs de conta. عندما يعلن manifest لمسار ما quorum اكبر من واحد, يتطلب admissão سلطة المعاملة بالاضافة الى الحسابات المدرجة لتلبية quorum الخاص بالmanifest.
- تكشف التليمترية عدادات admissão عبر `governance_manifest_admission_total{result}` لتمييز القبولات الناجحة عن مسارات `missing_manifest` e`non_validator_authority` e`quorum_rejected` e`protected_namespace_rejected` e`runtime_hook_rejected`.
- تكشف التليمترية مسار fiscalização عبر `governance_manifest_quorum_total{outcome}` (القيم `satisfied` / `rejected`) حتى يتمكن المشغلون تدقيق الموافقات المفقودة.
- تفرض المسارات lista de permissões para namespaces e manifestos. O `gov_namespace` é definido como `gov_contract_id` e o namespace do `protected_namespaces` é o manifesto. A configuração `RegisterSmartContractCode` contém metadados que não podem ser usados.
- يفرض admissão وجود اقتراح حوكمة Promulgado للتركيبة `(namespace, contract_id, code_hash, abi_hash)`؛ Não é permitido usar NotPermitted.

Ganchos em tempo de execução
- يمكن لـ manifests الخاصة بالمسار اعلان `hooks.runtime_upgrade` لبوابة تعليمات ترقية runtime (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Gancho:
  - `allow` (bool, `true`): O `false` é usado para reduzir o tempo de execução.
  - `require_metadata` (bool, código `false`): gera metadados de valor igual a `metadata_key`.
  - `metadata_key` (string): É um gancho de metadados. O `gov_upgrade_id` contém metadados e lista de permissões.
  - `allowed_ids` (array com strings): lista de permissões contém metadados (ou trim). يرفض عندما لا تكون القيمة المقدمة مدرجة.
- عندما يكون hook موجودا, يفرض admissão في الطابور سياسة metadados قبل دخول المعاملة للطابور. metadata A lista de permissões é definida como NotPermitted.
- Verifique o código `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- المعاملات التي تفي بالhook يجب ان تتضمن metadados `gov_upgrade_id=<value>` (او المفتاح المحدد في manifesto) مع اي موافقات مدققين مطلوبة بواسطة quorum الخاص بالmanifest.

Endpoint para
- POST `/v1/gov/protected-namespaces` - يطبق `gov_protected_namespaces` مباشرة على العقدة.
  - Nome: { "namespaces": ["apps", "sistema"] }
  - الرد: { "ok": verdadeiro, "aplicado": 1 }
  - ملاحظات: مخصص للادارة/الاختبار؛ O token de API não está disponível. Para obter mais informações, consulte `SetParameter(Custom)`.CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - يجلب مثيلات العقود للnamespace ويتحقق من:
    - Torii é um bytecode para `code_hash` e um resumo Blake2b-32 de `code_hash`.
    - o manifesto contém `/v1/contracts/code/{code_hash}` يبلغ بقيم `code_hash` e `abi_hash` متطابقة.
    - يوجد اقتراح حوكمة promulgado للتركيبة `(namespace, contract_id, code_hash, abi_hash)` مشتق بنفس hashing proposto-id الذي تستخدمه العقدة.
  - Use JSON como `results[]` para definir (problemas, como manifesto/código/proposta) بالاضافة الى ملخص سطر واحد الا اذا Não use (`--no-summary`).
  - مفيد لتدقيق namespaces المحمية او التحقق من تدفقات deploy الخاضعة للحوكمة.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Use JSON para metadados para implementar os namespaces de implantação do `gov_manifest_approvers`. O quórum do manifesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0`, e o número de telefone que você usa é o `min_bond_amount > 0`; Use `owner`, `amount` e `duration_blocks`.
  - Valida IDs de contas canônicas, canoniza dicas de nulificador de 32 bytes e mescla as dicas em `public_inputs_json` (com `--public <path>` para substituições adicionais).
  - O anulador é derivado do compromisso de prova (entrada pública) mais `domain_tag`, `chain_id` e `election_id`; `--nullifier` é validado em relação à prova quando fornecido.
  - الملخص في سطر واحد يعرض الان `fingerprint=<hex>` حتميا مشتقا من `CastZkBallot` المشفر مع dicas المففككة (`owner`, `amount`, `duration_blocks`, `direction`).
  - ردود CLI تضع تعليقات على `tx_instructions[]` ou `payload_fingerprint_hex` بالاضافة الى حقول مفكوكة كي تتحقق O código de segurança do dispositivo é Norito.
  - Dicas de bloqueio para bloqueio يسمح للعقدة باصدار احداث `LockCreated`/`LockExtended` para cédulas ZK بمجرد ان تكشف Não há problema.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Aliases `--lock-amount`/`--lock-duration-blocks` تعكس اسماء flags الخاصة بـ ZK لتحقيق تماثل السكربتات.
  - ناتج الملخص يعكس `vote --mode zk` باضافة impressão digital للتعليمة المشفرة وحقول cédula المقروءة (`owner`, `amount`, `duration_blocks`, `direction`).

قائمة المثيلات
- GET `/v1/gov/instances/{ns}` - يسرد مثيلات العقود النشطة لnamespace.
  - Parâmetros de consulta:
    - `contains`: Substring de substring de `contract_id` (diferencia maiúsculas de minúsculas)
    - `hash_prefix`: تصفية بحسب بادئة hex para `code_hash_hex` (minúsculas)
    - `offset` (padrão 0), `limit` (padrão 100, máximo 10_000)
    - `order`: é igual a `cid_asc` (padrão), `cid_desc`, `hash_asc`, `hash_desc`
  - الرد: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) e `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

مسح desbloqueia (المشغل/التدقيق)
- OBTER `/v1/gov/unlocks/stats`
  - الرد: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - ملاحظات: `last_sweep_height` يعكس اخر ارتفاع بلوك تم فيه مسح locks منتهية وتخزينها. `expired_locks_now` é um bloqueio de bloqueio de `expiry_height <= height_current`.
-POSTO `/v1/gov/ballots/zk-v1`
  - الطلب (DTO نمط v1):
    {
      "autoridade": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "back-end": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "proprietário": "i105…?",
      "anulador": "blake2b32:...64hex?"
    }
  - الرد: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (recurso: `zk-ballot`)
  - O JSON `BallotProof` é processado e o `CastZkBallot`.
  - الطلب:
    {
      "autoridade": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "votação": {
        "back-end": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 para ZK1 e H2*
        "root_hint": null, // string hexadecimal opcional de 32 bytes (raiz de elegibilidade)
        "owner": null, // AccountId
        "nullifier": null // string hexadecimal opcional de 32 bytes (dica do anulador)
      }
    }
  - الرد:
    {
      "ok": verdade,
      "aceito": verdadeiro,
      "reason": "construir esqueleto da transação",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Como:
    - يقوم الخادم بربط `root_hint`/`owner`/`nullifier` الاختيارية cédula de `public_inputs_json` para `CastZkBallot`.
    - Você precisa de bytes no envelope para base64 no payload.
    - تتغير `reason` ou `submitted transaction` عندما يقدم Torii cédula.
    - Este ponto de extremidade é definido como recurso `zk-ballot`.

مسار التحقق من CastZkBallot
- `CastZkBallot` é um arquivo base64 que pode ser usado como base64 (`BallotRejected` em vez de `invalid or empty proof`).
- المضيف يحل مفتاح التحقق لللللللللللللمن referendo (`vk_ballot`) او افتراضات الحوكمة ويتطلب ان يكون السجل موجودا O `Active` armazena bytes em linha.
- bytes de hashing para `hash_vk`; Isso é algo que você pode fazer para não perder dinheiro (`BallotRejected` em vez de `verifying key commitment mismatch`).
- bytes do backend do backend do `zk::verify_backend`; Verifique se o `BallotRejected` está no `invalid proof` e se está funcionando.
- A prova deve expor um compromisso eleitoral e uma raiz de elegibilidade como contribuições públicas; a raiz deve corresponder ao `eligible_root` da eleição e o anulador derivado deve corresponder a qualquer dica fornecida.
- Número de telefone `BallotAccepted`; nullifiers são bloqueados e bloqueados por um bloqueio de segurança. É isso.

## سوء سلوك المدققين والتوافق المشترك

### سير عمل cortar e encarcerar

Verifique se `Evidence` é compatível com Norito. Verifique se o `EvidenceStore` está no banco de dados e se você está usando o `consensus_evidence` Nome do WSV. Você pode usar o `sumeragi.npos.reconfig.evidence_horizon_blocks` (`7200` `7200`) para obter mais informações Você não pode fazer isso. As evidências no horizonte também respeitam `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o atraso reduzido `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`); a governança pode cancelar penalidades com `CancelConsensusEvidencePenalty` antes que a redução seja aplicada.

O número de telefone é o mesmo que o `EvidenceKind`; Nome de usuário e número de telefone:

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

- **DoublePrepare/DoubleCommit** - Os hashes e os hashes são usados na tupla `(phase,height,view,epoch)`.
- **InvalidQc** - قام مجمع ببث commit certificado شكله يفشل الفحوص الحتمية (مثلا bitmap موقعين فارغ).
- **InvalidProposal** - قدم قائد بلوكا يفشل التحقق البنيوي (مثلا يكسر قاعدة cadeia bloqueada).
- **Censura** — recibos de envio assinados mostram uma transação que nunca foi proposta/comprometida.

يمكن للمشغلين والادوات فحص واعادة بث الحمولات عبر:

- Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, e`... submit --evidence-hex <payload>`.

يجب على الحوكمة اعتبار bytes الخاصة بالevidence دليلا قانونيا:

1. **جمع الحمولة** قبل ان تتقادم. Os bytes Norito são definidos como metadados por altura/visualização.
2. **تجهيز العقوبة** عبر تضمين الحمولة no referendo e تعليمة sudo (exemplo `Unregister::peer`). تعيد عملية التنفيذ التحقق من الحمولة؛ evidência المشوهة او القديمة ترفض حتميا.
3. **جدولة طوبولوجيا المتابعة** حتى لا يتمكن المدقق المخالف من العودة فورا. A lista de verificação é `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com lista de tarefas.
4. **تدقيق النتائج** عبر `/v1/sumeragi/evidence` e `/v1/sumeragi/status` لضمان ان عداد evidências تقدم وان الحوكمة طبقت الازالة.

### تسلسل الاجماع المشترك

يضمن الاجماع المشترك ان تقوم مجموعة المدققين الخارجة بانهاء بلوك الحد قبل ان تبدا المجموعة Não há problema. يفرض الـ runtime القاعدة عبر معاملات مزدوجة:- Não há nenhum problema em `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` em **نفس البلوك**. Use o `mode_activation_height` para obter mais informações sobre o produto Não há atraso.
- `sumeragi.npos.reconfig.activation_lag_blocks` (الافتراضي `1`) e guarda تهيئة يمنع hand-offs بدون lag:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`) atrasa a redução do consenso para que a governança possa cancelar as penalidades antes que elas sejam aplicadas.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- يعرض الـ runtime وCLI المعاملات staged عبر `/v1/sumeragi/params` و`iroha --output-format text ops sumeragi params` حتى يتمكن المشغلون من التحقق من ارتفاعات التفعيل وقوائم المدققين.
- يجب ان تقوم اتمتة الحوكمة دائما بما يلي:
  1. انهاء قرار الازالة (او الاستعادة) nenhuma evidência.
  2. Verifique se o produto está em `mode_activation_height = h_current + activation_lag_blocks`.
  3. A chave `/v1/sumeragi/status` deve ser instalada no `effective_consensus_mode`.

Você pode cortar o slashing **يجب الا** يحاول تفعيل بدون lag e معاملات hand-off; يتم رفض تلك المعاملات e تترك الشبكة على الوضع السابق.

## اسطح التليمترية

- Verifique o tamanho do Prometheus:
  - `governance_proposals_status{status}` (manômetro) يتتبع تعداد المقترحات حسب الحالة.
  - `governance_protected_namespace_total{outcome}` (counter) يزيد عندما يسمح او يرفض admissão لنشر ضمن namespaces محمية.
  - `governance_manifest_activations_total{event}` (counter) é um manifesto (`event="manifest_inserted"`) e um namespace (`event="instance_bound"`).
- `/status` يتضمن كائنا `governance` يعكس تعداد المقترحات, ويبلغ عن اجمالي namespaces المحمية, ويسرد Exibe valores de manifesto (namespace, ID do contrato, código/hash ABI, altura do bloco, carimbo de data/hora de ativação). Não há nenhuma lei sobre o assunto, nem decretos, manifestos e espaços para nome que podem ser usados.
- Grafana (`docs/source/grafana_governance_constraints.json`) e runbook para `telemetry.md`. Você pode usar o manifesto e definir namespaces para o tempo de execução.