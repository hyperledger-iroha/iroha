---
lang: he
direction: rtl
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

סטטוס: rascunho/esboco para acompanhar as tarefas de implementacao de governanca. כפורמאס פודם mudar durante a implementacao. Determinismo e politica RBAC sao restricoes normativas; Torii pode assinar/submeter transacoes quando `authority` e `private_key` sao fornecidos, caso contrario os clientes constroem e submetem para `/transaction`.

Visao Geral
- נקודות הקצה של Todos OS retornam JSON. Para fluxos que produzem transacoes, as respostas incluem `tx_instructions` - um array de uma ou mais instrucoes esqueleto:
  - `wire_id`: זיהוי רישום עבור או טיפו דה הוראות
  - `payload_hex`: בתים של מטען Norito (hex)
- Se `authority` e `private_key` forem fornecidos (ou `private_key` em DTOs de ballots), Torii assina e submete a transacao e ainda retorna I108NI06X00.
- Caso contrario, clientes montam uma SignedTransaction usando sua Authority e chain_id, depois assinam e fazem POST para `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` retorna `GovernanceProposalResult` (נורמליזה קמפוס סטטוס/סוג), `ToriiClient.get_governance_referendum_typed` retorna `GovernanceReferendumResult`, Prometheus, Prometheus, Prometheus, Prometheus `ToriiClient.get_governance_locks_typed` retorna `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` retorna `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` retorna `GovernanceInstancesPage`, impondo de governance super de aempicaado README.
- Cliente Python leve (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` retornam bundles tipados `GovernanceInstructionDraft` (encapsulando o esqueleto `GovernanceInstancesPage` do Prometheus do `GovernanceInstancesPage` do `GovernanceInstructionDraft`) לנתח מדריך של JSON scripts quando compoem fluxos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` לחשוף עוזרים tipados para הצעות, משאל, נקודות, מנעולים, סטטיסטיקות ביטול נעילה, e agora `listGovernanceInstances(namespace, options)` mais os נקודות הקצה לעשות המועצה (Prometheus, I000NI, I000NI `governancePersistCouncil`, `getGovernanceCouncilAudit`) עבור לקוחות Node.js possam paginar `/v1/gov/instances/{ns}` וזרימות עבודה בשיתוף VRF junto do list existente de instancias de contrato.

נקודות קצה

- POST `/v1/gov/proposals/deploy-contract`
  - Requisicao (JSON):
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
  - תשובה (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validacao: os nos canonizam `abi_hash` para o `abi_version` fornecido e rejeitam divergencias. Para `abi_version = "v1"`, o valor esperado e `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.API de contratos (פריסה)
- POST `/v1/contracts/deploy`
  - Requisicao: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - רכיב: calcula `code_hash` a partir do corpo do programa IVM e `abi_hash` a partir do header `abi_version`, depois submete Prometheus (emanfesto) `RegisterSmartContractBytes` (בתים `.to` השלמות) עם שם `authority`.
  - תגובה: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - קבל את `/v1/contracts/code/{code_hash}` -> רטורנה או מניפסט ארמזנאדו
    - קבל `/v1/contracts/code-bytes/{code_hash}` -> retorna `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Requisicao: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - רכיב: deploya o bytecode fornecido e ativa imediatamente o mapeamento `(namespace, contract_id)` דרך `ActivateContractInstance`.
  - תגובה: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Servico de alias
- POST `/v1/aliases/voprf/evaluate`
  - Requisicao: { "blinded_element_hex": "..." }
  - תגובה: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete a implementacao do avaliador. ערך אמיתי: `blake2b512-mock`.
  - הערות: avaliador mock deterministico que aplica Blake2b512 com separacao de dominio `iroha.alias.voprf.mock.v1`. Destinado a tooling de teste at o pipeline VOPRF de producao ser integrado ao Iroha.
  - שגיאות: HTTP `400` עם קלט hex misformado. Torii retorna um envelope Norito `ValidationFail::QueryFailed::Conversion` com a mensagem de erro do מפענח.
- POST `/v1/aliases/resolve`
  - Requisicao: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - תגובה: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - הערות: בקש או זמן ריצה גשר ISO (`[iso_bridge.account_aliases]` em `iroha_config`). Torii נורמליזה כינויים removendo espacos e convertendo para maiusculas antes do lookup. Retorna 404 quando או alias esta ausente e 503 quando o runtime ISO bridge esta disabilitado.
- POST `/v1/aliases/resolve_index`
  - Requisicao: { "אינדקס": 0 }
  - תגובה: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: indices de alias sao atribuidos de forma deterministica pela ordem de configuracao (מבוסס 0). שירותים של לקוחות מטמון לא מקוונים לבניית אולמות אודיטוריה אירועי כינוי.

Limite de tamanho de codigo
- פרמטר מותאם אישית: `max_contract_code_bytes` (JSON u64)
  - Controla o tamanho maximo permitido (em bytes) para armazenamento de codigo de contrato on-chain.
  - ברירת מחדל: 16 MiB. אוסלו את `RegisterSmartContractBytes` quando o tamanho da imagem `.to` excede o limite com um erro de violacao de invariante.
  - Operadores podem ajustar דרך `SetParameter(Custom)` com `id = "max_contract_code_bytes"` e um מטען מטען numerico.- POST `/v1/gov/ballots/zk`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - תגובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות:
    - Quando OS כניסות publicos do circuito incluem `owner`, `amount` e `duration_blocks`, e a prova verifica contra a VK configurada, o no cria ou estende um lock de governanca para Icom 1020X `owner`. A direcao permanece oculta (`unknown`); כמות/תפוגה של apenas sao atualizados. הצבעות מחדש סאו מונוטוניות: כמות e expiry apenas aumentam (o no aplica max(amount, prev.amount) e max(expiry, prev.expiry)).
    - הצבעות מחדש ZK que tentem reduzir סכום או תפוגה sao rejeitados no servidor com diagnosticos `BallotRejected`.
    - A execucao do contrato deve chamar `ZK_VOTE_VERIFY_BALLOT` antes de enfileirar `SubmitBallot`; מארח impoem um latch de uma unica vez.

- POST `/v1/gov/ballots/plain`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "yetain|Nay"}: "Abs
  - תגובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }
  - הערות: הצבעות מחדש sao de extensao apenas - אום נובו הצבעה נאו pode reduzir סכום או תפוגה לעשות נעול קיימות. O `owner` deve igualar a Authority da transacao. Duracao minima e `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Requisicao: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito על-שרשרת (פיגום): פרסם אומה proposta deploy aprovada insere um `ContractManifest` minimo com chave `code_hash` com o `abi_hash` esperado e marca a proposta como. Se um manifesto ja existir para o `code_hash` com `abi_hash` diferente, o enactment e rejeitado.
  - הערות:
    - Para eleicoes ZK, os caminhos do contrato devem chamar `ZK_VOTE_VERIFY_TALLY` antes de executar `FinalizeElection`; os hosts impoem um latch de uso unico. `FinalizeReferendum` Rejeita referendos ZK ate que o tally da eleicao esteja finalizado.
    - O auto-fechamento em `h_end` emite אושר/נדחה apenas para referendos רגיל; Referendos ZK permanecem סגור אכלתי que um tally finalizado seja enviado e `FinalizeReferendum` seja executado.
    - כמו checagens de turnout usam apenas לאשר+לדחות; נמנע נאו קונטה פארה או שיעור הצבעה.- POST `/v1/gov/enact`
  - Requisicao: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105...?", "private_key": "...?" }
  - תגובה: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - הערות: Torii submete a transacao assinada quando `authority`/`private_key` sao fornecidos; caso contrario retorna um esqueleto para clientes assinarem e submeterem. תדמית מוקדמת ואופציונלי ומידע אינפורמטיבי.

- קבל את `/v1/gov/proposals/{id}`
  - נתיב `{id}`: id de proposta hex (64 תווים)
  - תגובה: { "נמצא": bool, "הצעה": { ... }? }

- קבל את `/v1/gov/locks/{rid}`
  - נתיב `{rid}`: string de id de referendum
  - תגובה: { "נמצא": bool, "referendum_id": "לפטר", "מנעולים": { ... }? }

- קבל `/v1/gov/council/current`
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notas: retorna o Council persistido quando presente; caso contrario deriva um fallback deterministico usando o asset de stake configurado e ספים (espelha a especificacao VRF ate que provas VRF em producao sejam persistidas on-chain).

- POST `/v1/gov/council/derive-vrf` (תכונה: gov_vrf)
  - Requisicao: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "רגיל|קטן", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: verifica a prova VRF de cada candidato contra o input canonico derivado de `chain_id`, `epoch` e do ultimo hash de bloco; ordena por bytes de saida desc com שובר שוויון; retorna os top `committee_size` membros. נאו מתמיד.
  - תגובה: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - הערות: רגיל = pk em G1, הוכחה em G2 (96 בתים). קטן = pk em G2, הוכחה em G1 (48 בתים). כניסות נפרדות לשלטון וכלל `chain_id`.

### ברירת מחדל de governanca (iroha_config `gov.*`)

O Council fallback usado pelo Torii quando nao existe ruster persistido e parametrizado via `iroha_config`:

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

עוקפים את המקבילות הסביבה:

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

`parliament_committee_size` limita o numero de membros fallback retornados quando nao ha Council persistido, `parliament_term_blocks` להגדיר o comprimento da epoca usado para derivacao de seed (`epoch = floor(height / term_blocks)`), `parliament_term_blocks` הגדר o comprimento da epoca usado para derivacao de seed (`epoch = floor(height / term_blocks)`), `epoch = floor(height / term_blocks)`, `parliament_term_blocks` הגדר o comprimento da epoca usado para derivacao de seed (`epoch = floor(height / term_blocks)`), `epoch = floor(height / term_blocks)`, `parliament_term_blocks` הגדר o comprimento da epoca usado para derivacao de seed (`epoch = floor(height / term_blocks)`), `epoch = floor(height / term_blocks)`, `parliament_term_blocks` minimas) no asset de elegibilidade, e `parliament_eligibility_asset_id` seleciona qual saldo de asset e escaneado ao construir o conjunto de candidatos.

A verificacao VK de governanca nao tem: a verificacao de ballot semper requer uma chave `Active` com bytes inline, e os ambientes nao devem depender de toggles de teste para polar a verificacao.

RBAC
- הרשאות Execucao על השרשרת:
  - הצעות: `CanProposeContractDeployment{ contract_id }`
  - קלפי: `CanSubmitGovernanceBallot{ referendum_id }`
  - חקיקה: `CanEnactGovernance`
  - הנהלת המועצה (futuro): `CanManageParliament`מרחבי שמות protegidos
- פרמטר מותאם אישית `gov_protected_namespaces` (מערך מיתרים של JSON) גישה לכניסה לפריסת מרחבי שמות.
- לקוחות הפיתוח כוללים את המטא-נתונים של טרנסאקאו עבור פריסת מרחבי שמות מוגנים:
  - `gov_namespace`: מרחב שמות alvo (לדוגמה, "אפליקציות")
  - `gov_contract_id`: מזהה חוזה logico dentro dospace
- `gov_manifest_approvers`: מערך JSON אופציונלי של מזהי חשבונות מאושרים. קוואנדו אום ליין מניפסט הצהרת קוורום מאיור que um, קבלה דורשת סמכות da transacao mais as contas listadas para satisfazer o quorum do Manifesto.
- Telemetria expoe contadores deadmission via `governance_manifest_admission_total{result}` para que operadores distingam admits bem-sucedidos de caminhos `missing_manifest`, `non_validator_authority`, `gov_manifest_approvers`, I01e0X, I01e `runtime_hook_rejected`.
- Telemetria expoe o caminho de enforcement via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) למען מפעילי הביקורת הביקורתית.
- Lanes aplicam a permitlist de namespaces publicada em seus manifests. Qualquer transacao que define `gov_namespace` deve fornecer `gov_contract_id`, e o namespace deve aparecer no conjunto `protected_namespaces` לעשות מניפסט. ההגשות `RegisterSmartContractCode` הן מטא נתונים וסגרות הגנה.
- קבלה impoe que exista uma proposta de governanca שנחקק para o tuple `(namespace, contract_id, code_hash, abi_hash)`; caso contrario a validacao falha com um erro NotPermitted.

הוקס לשדרוג זמן ריצה
- Manifests de lane podem declarar `hooks.runtime_upgrade` עבור הוראות שער לשדרוג זמן ריצה (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos לעשות הוק:
  - `allow` (bool, ברירת המחדל `true`): quando `false`, כמו הנחיות לשדרוג זמן ריצה.
  - `require_metadata` (bool, ברירת המחדל `false`): בקש קוד מפורט של מטא נתונים של `metadata_key`.
  - `metadata_key` (מחרוזת): שם מטא נתונים אפליקדה וו. ברירת מחדל `gov_upgrade_id` quando metadata e requerida ou ha have list.
  - `allowed_ids` (מערך של מחרוזות): רשימת היתרים אופציונלית של מטא-נתונים (apos trim). Rejeita quando o valor fornecido nao esta listado.
- Quando o hook esta presente, admission de fila aplica a politica de metadata antes de a transacao entrar na fila. Metadata ausente, valores em branco ou fora da permitlist geram um erro NotPermitted deterministico.
- תוצאות Telemetria rastreia דרך `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Transacoes que satisfazem o hook devem incluir metadata `gov_upgrade_id=<value>` (ou a chave definida pelo Manifesto) junto com quaisquer aprovacoes de validadores exigidas pelo quorum do Manifesto.

נקודת קצה נוחות
- POST `/v1/gov/protected-namespaces` - aplica `gov_protected_namespaces` diretamente no no.
  - Requisicao: { "מרחבי שמות": ["אפליקציות", "מערכת"] }
  - תגובה: { "בסדר": true, "applied": 1 }
  - הערות: destinado a admin/testing; בקש אסימון של תצורת API. Para producao, prefira enviar uma transacao assinada com `SetParameter(Custom)`.עוזרי CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Busca instancias de contrato para o namespace e confere que:
    - Torii armazena bytecode לקודמת `code_hash`, e seu digest Blake2b-32 corresponde ao `code_hash`.
    - O Manifesto Armazenado em `/v1/contracts/code/{code_hash}` reporta `code_hash` e `abi_hash` correspondents.
    - Existe uma proposta de governanca נחקק para `(namespace, contract_id, code_hash, abi_hash)` derivada pelo mesmo hashing de offer-id que o no usa.
  - Emite um relatorio JSON com `results[]` por contrato (בעיות, קורות חיים של מניפסט/קוד/הצעה) mais um resumo de uma linha a menos que supprimido (`--no-summary`).
  - השתמש במרחבי השמות המוגנים או בדוק את הפריסה של מערכות שליטה.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emite או JSON esqueleto של מטא נתונים בארה"ב או פריסות תת-מטרים במרחבי שמות, כולל אופציות `gov_manifest_approvers` לסיפוק חוקי מניפסט.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — רמזים לנעילה são obrigatórios quando `min_bond_amount > 0`, e qualquer conjunto de hints fornecido deve incluir `owner`, Prometheus I00000191900.
  - מאמת מזהי חשבונות קנוניים, מבצע קנוניזציה של רמזים לביטול של 32 בתים וממזג את הרמזים לתוך `public_inputs_json` (עם `--public <path>` לעקיפות נוספות).
  - המבטל נגזר מהתחייבות ההוכחה (קלט ציבורי) בתוספת `domain_tag`, `chain_id` ו-`election_id`; `--nullifier` מאומת כנגד ההוכחה כאשר היא מסופקת.
  - O resumo de uma linha agora exibe `fingerprint=<hex>` deterministico derivado do `CastZkBallot` codificado junto com hints decodificados (`owner`, `amount`, I000000208X, I000X `direction` quando fornecidos).
  - כתשובה של CLI anotam `tx_instructions[]` com `payload_fingerprint_hex` mais campos decodificados para que ferramentas downstream verifiquem o esqueleto sem reimplementar decodificacao Norito.
  - Fornecer hints de lock permite que o no emita eventos `LockCreated`/`LockExtended` para ballots ZK assim que o circuito expuser os mesmos valores.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - כינויים של OS `--lock-amount`/`--lock-duration-blocks` espelham os nomes de flags ZK para paridade de script.
  - A saida de resumo espelha `vote --mode zk` ao incluir o טביעת אצבע da instrucao codificada e campos de ballot legiveis (`owner`, `amount`, Prometheus, I0000002220X, I00000102220X, confirmacao rapida antes de assinar o esqueleto.Listagem de instancias
- קבל את `/v1/gov/instances/{ns}` - רשימה של מופעים נגדיים עבור מרחב שמות.
  - פרמטרים של שאילתה:
    - `contains`: filtra por substring de `contract_id` (תלוי רישיות)
    - `hash_prefix`: filtra por prefixo hex de `code_hash_hex` (אותיות קטנות)
    - `offset` (ברירת מחדל 0), `limit` (ברירת מחדל 100, מקסימום 10_000)
    - `order`: um de `cid_asc` (ברירת מחדל), `cid_desc`, `hash_asc`, `hash_desc`
  - תגובה: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) או `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Varredura de unlocks (Operador/Auditoria)
- קבל את `/v1/gov/unlocks/stats`
  - תגובה: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - הערות: `last_sweep_height` reflete a altura de bloco mais recente onde locks expirados foram varridos e persistidos. `expired_locks_now` e calculado ao varrer registros de lock com `expiry_height <= height_current`.
- POST `/v1/gov/ballots/zk-v1`
  - Requisicao (DTO estilo v1):
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
  - תגובה: { "בסדר": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (תכונה: `zk-ballot`)
  - עמודה על JSON `BallotProof` כתובה וחזרה על esqueleto `CastZkBallot`.
  - Requisicao:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "הצבעה": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 do container ZK1 ou H2*
        "root_hint": null, // מחרוזת hex אופציונלית של 32 בתים (שורש זכאות)
        "owner": null, // AccountId אופציונלי לבעלים
        "nullifier": null // מחרוזת hex אופציונלית של 32 בתים (רמז לבטל)
      }
    }
  - תגובה:
    {
      "בסדר": נכון,
      "מקובל": נכון,
      "reason": "בנה שלד עסקה",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - הערות:
    - O servidor mapeia `root_hint`/`owner`/`nullifier` אופציונאי לעשות הצבעה עבור `public_inputs_json` em `CastZkBallot`.
    - Os bytes עוטפים את הקוד מחדש כמו base64 עבור מטען מטען.
    - A resposta `reason` muda para `submitted transaction` quando Torii submet o the vote.
    - Este point end so esta disponivel quando o feature `zk-ballot` esta habilitado.Caminho de verificacao CastZkBallot
- `CastZkBallot` decodifica a prova base64 fornecida e rejeita payloads vazios ou malformados (`BallotRejected` com `invalid or empty proof`).
- המארחים פותרים את ההצבעה ומציאת משאל העם (`vk_ballot`) או את ברירת המחדל של הממשל e exige que o registro exista, esteja `Active` e contenha bytes inline.
- Bytes de chave verificadora armazenados sao re-hasheados com `hash_vk`; qualquer mismatch de commitment aborta a execucao antes da verificacao para proteger contra entradas de registro adulteradas (`BallotRejected` com `verifying key commitment mismatch`).
- Bytes de prova sao despachados ו-backend registrado דרך `zk::verify_backend`; transcricoes invalidas aparecem como `BallotRejected` com `invalid proof` e a instrucao falha de forma deterministica.
- על ההוכחה לחשוף התחייבות קלפי ושורש זכאות כתשומות ציבוריות; השורש חייב להתאים ל-`eligible_root` של הבחירות, והמבטל הנגזר חייב להתאים לכל רמז שסופק.
- Provas bem-sucedidas emitem `BallotAccepted`; nullifiers duplicados, שורשי de elegibilidade מיושנים או regressao de lock continuam a produzir כמו razoes de rejeicao existentes descritas anteriormente neste documento.

## Mau comportamento de validadores e consenso conjunto

### Fluxo de slashing e jailing

O consenso emite `Evidence` codificada em Norito quando um validador viola o protocolo. Cada payload chega ao `EvidenceStore` em memoria e, se inedito, e materializado no mapa `consensus_evidence` respaldado por WSV. Registros mais antigos que `sumeragi.npos.reconfig.evidence_horizon_blocks` (ברירת מחדל `7200` blocos) סאו rejeitados para manter o arquivo limitado, mas a rejeicao e registrada para operations. הראיות באופק מכבדות גם את `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) ואת עיכוב החיתוך `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`); ממשל יכול לבטל קנסות עם `CancelConsensusEvidencePenalty` לפני שהחתך חל.

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind`; os discriminantes sao estaveis e impostos pelo מודל נתונים:

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

- **DoublePrepare/DoubleCommit** - o validador assinou hashes conflitantes para o mesmo tuple `(phase,height,view,epoch)`.
- **InvalidQc** - um agregador gossiped um commit certificate cuja forma falha em checagens deterministicas (לדוגמה, מפת סיביות de signers vazio).
- **InvalidProposal** - um leader prop os um bloco que falha validacao estrutural (לדוגמה, viola a regra de locked-chain).
- **צנזורה** - קבלות הגשה חתומות מציגות עסקה שמעולם לא הוצעה/בוצעה.

מפעיל פודם כלי עבודה במיוחס עומסי שידור חוזרים באמצעות:

- Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, ו-`... submit --evidence-hex <payload>`.

A governanca deve tratar os bytes de evidence como prova canonica:1. **קולטר או מטען** לפני תפוגה. Arquive OS bytes Norito brutos junto com metadata de height/view.
2. **הכנת עונשין** embedando o payload em um referendum ou instrucao sudo (לדוגמה, `Unregister::peer`). A excucao re-valida o מטען; ראיות malformada ou stale e rejeitada deterministamente.
3. **סדר יום טופולוגיה דה אקומפנהמנטו** para que o validador infrator nao possa retornar imediatamente. Fluxos tipicos enfileiram `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com o roster atualizado.
4. **Auditar resultados** דרך `/v1/sumeragi/evidence` e `/v1/sumeragi/status` para garantir que o contador de evidence avancou e que a governanca aplicou a remocao.

### Sequenciamento de consenso conjunto

O consenso conjunto garante que o conjunto de validadores de saida finalize o bloco de fronteira antes que o novo conjunto comeca a propor. O aplica time run a regra דרך parametros paraeados:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` devem ser confirmados no **mesmo bloco**. `mode_activation_height` deve ser estritamente maior que a altura do bloco que carregou a atualizacao, fornecendo ao menos um bloco de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (ברירת מחדל `1`) e o guard de configuracao שמפריע למסירות com lag zero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (ברירת מחדל `259200`) מעכב את קיצוץ הקונצנזוס כך שהממשל יכול לבטל עונשים לפני שהם יחולו.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- זמן ריצה ופרמטרים של תערוכת CLI שבוצעו באמצעות `/v1/sumeragi/params` ו-`iroha --output-format text ops sumeragi params`, עבור מפעילי אישורים אופטימליים ורשימות תאימות.
- אאוטומקאו דה גוברננקה deve sempre:
  1. Finalizar a decisao de remocao (ou reintegro) respaldada por ראיות.
  2. Enfileirar uma reconfiguracao de acompanhamento com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorar `/v1/sumeragi/status` אכלו `effective_consensus_mode` טרוטות נוספות.

תסריט Qualquer que rotacione validadores ou aplique slashing **nao deve** tentar ativacao de lag zero או omitir os parametros de hand-off; tais transacoes sao rejeitadas e deixam a rede no modo anterior.

## שטחי טלמטריה- Metricas Prometheus ייצוא atividade de governanca:
  - `governance_proposals_status{status}` (מד) rastreia contagens de suggests por status.
  - `governance_protected_namespace_total{outcome}` (מונה) מגדילים את כניסת מרחבי השמות להגנה על הפריסה.
  - `governance_manifest_activations_total{event}` (מונה) registra insercoes de manifest (`event="manifest_inserted"`) e bindings de namespace (`event="instance_bound"`).
- `/status` כולל את אובייקט `governance` כחלק מההצעות, כולל מרחבי שמות אבטחים לרשימה האחרונה של המניפסט (מרחב שמות, מזהה חוזה, קוד/ABI hash, גובה בלוק, חותמת זמן הפעלה). Operadores podem consultar esse campo para confirmar que enactments atualizaram manifests e que Gates de namespaces protegidos estao sendo aplicados.
- אום תבנית Grafana (`docs/source/grafana_governance_constraints.json`) e o runbook de telemetria em `telemetry.md` mostram como ligar alertas para הצעות פרסות, adivacoes de manifest ausentes, ou rejeicoes inesperadas de run dutimerants upgrades decrete duspaces upgrades.