---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

حالت: گورننس نفاذی کاموں کے ساتھ چلنے والا ڈرافٹ/اسکیچ۔ عملدرآمد کے دوران ساختیں بدل سکتی ہیں۔ Determinismo no RBAC پالیسی معیاری پابندیاں ہیں؛ جب `authority` ou `private_key` فراہم ہوں تو Torii ٹرانزیکشن سائن/سبمٹ کر سکتا ہے، O cartão de crédito `/transaction` é um cartão de crédito

جائزہ
- Os endpoints JSON são definidos como padrão ٹرانزیکشن بنانے والے فلو کے لئے جوابات میں `tx_instructions` شامل ہوتے ہیں - ایک یا زیادہ instrução esqueletos کی array:
  - `wire_id`: instrução ٹائپ کا identificador de registro
  - `payload_hex`: Norito bytes de carga útil (hex)
- اگر `authority` اور `private_key` (یا ballot DTOs میں `private_key`) فراہم ہوں تو Torii ٹرانزیکشن Você pode usar o cartão de crédito `tx_instructions` e o cartão de crédito `tx_instructions`
- ورنہ کلائنٹس اپنی autoridade e chain_id کے ساتھ SignedTransaction بناتے ہیں, پھر سائن کر کے `/transaction` پر POST کرتے ہیں۔
- Cobertura SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` `GovernanceProposalResult` واپس کرتا ہے (campos de status/tipo کو normalizar کرتا ہے), `ToriiClient.get_governance_referendum_typed` `GovernanceReferendumResult` واپس کرتا ہے, `ToriiClient.get_governance_tally_typed` `GovernanceTally` واپس کرتا ہے, `ToriiClient.get_governance_locks_typed` `GovernanceLocksResult` واپس کرتا ہے، `ToriiClient.get_governance_unlock_stats_typed` `GovernanceUnlockStats` واپس کرتا ہے, اور `ToriiClient.list_governance_instances_typed` `GovernanceInstancesPage` واپس کرتا ہے, جس سے پورے superfície de governança پر acesso digitado ملتا ہے اور README میں exemplos de uso دیے گئے ہیں۔
- Cliente leve Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` digitado pacotes `GovernanceInstructionDraft` واپس کرتے ہیں (Torii کی Esqueleto `tx_instructions` کو wrap کرتے ہوئے), تاکہ scripts Finalizar/Aprovar fluxos بناتے وقت análise JSON manual سے بچ سکیں۔
- JavaScript (`@iroha/iroha-js`): propostas `ToriiClient`, referendos, contagens, bloqueios, estatísticas de desbloqueio کے لئے ajudantes digitados دیتا ہے, اور اب `listGovernanceInstances(namespace, options)` کے ساتھ endpoints do conselho (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) Você pode usar clientes Node.js `/v1/gov/instances/{ns}` para paginar سکیں اور fluxos de trabalho apoiados por VRF کو موجودہ listagem de instâncias de contrato

Pontos finais

-POSTO `/v1/gov/proposals/deploy-contract`
  - Solicitação (JSON):
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
  - Resposta (JSON):
    { "ok": verdadeiro, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validação: nós فراہم کردہ `abi_version` کے لئے `abi_hash` کو canonizar کرتے ہیں اور incompatibilidade پر rejeitar کرتے ہیں۔ `abi_version = "v1"` کے لئے متوقع valor `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))` ہے۔

API de contratos (implantação)
-POSTO `/v1/contracts/deploy`
  - Solicitação: { "autoridade": "i105...", "private_key": "...", "code_b64": "..." }
  - Behavior: IVM پروگرام باڈی سے `code_hash` اور header `abi_version` سے `abi_hash` نکالتا ہے، پھر `RegisterSmartContractCode` (manifesto) e `RegisterSmartContractBytes` (مکمل `.to` bytes) `authority` کی طرف سے سبمٹ کرتا ہے۔
  - Resposta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> ذخیرہ شدہ manifest واپس کرتا ہے
    - GET `/v1/contracts/code-bytes/{code_hash}` -> `{ code_b64 }` واپس کرتا ہے
-POSTO `/v1/contracts/instance`
  - Solicitação: { "autoridade": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamento: فراہم کردہ bytecode deploy کرتا ہے اور `ActivateContractInstance` کے ذریعے Mapeamento `(namespace, contract_id)` فوراً فعال کرتا ہے۔
  - Resposta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Serviço de alias
-POSTO `/v1/aliases/voprf/evaluate`
  - Solicitação: { "blinded_element_hex": "..." }
  - Resposta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - Implementação do avaliador `backend` کو ظاہر کرتا ہے۔ Valor do valor: `blake2b512-mock`۔
  - Notas: avaliador simulado determinístico جو Blake2b512 کو separação de domínio `iroha.alias.voprf.mock.v1` کے ساتھ aplicar کرتا ہے۔ یہ ferramentas de teste کے لئے ہے جب تک pipeline VOPRF de produção Iroha میں fio نہ ہو جائے۔
  - Erros: entrada hexadecimal malformada پر HTTP `400`۔ Torii Norito `ValidationFail::QueryFailed::Conversion` envelope اور mensagem de erro do decodificador واپس کرتا ہے۔
-POSTO `/v1/aliases/resolve`
  - Solicitação: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Resposta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: teste de tempo de execução da ponte ISO درکار ہے (`[iso_bridge.account_aliases]` em `iroha_config`)۔ Torii espaço em branco ہٹا کر اور maiúsculo بنا کر pesquisa کرتا ہے۔ alias نہ ہو تو 404 Tempo de execução da ponte ISO بند ہو تو 503 دیتا ہے۔
-POSTO `/v1/aliases/resolve_index`
  - Solicitação: { "índice": 0 }
  - Resposta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: ordem de configuração dos índices de alias کے مطابق determinístico طریقے سے atribuir ہوتے ہیں (baseado em 0)۔ کلائنٹس cache offline کر کے eventos de atestado de alias کے trilhas de auditoria بنا سکتے ہیں۔

Limite de tamanho do código
- Parâmetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Armazenamento de código de contrato na cadeia
  - Padrão: 16 MiB۔ جب imagem `.to` حد سے بڑی ہو تو nós `RegisterSmartContractBytes` کو erro de violação invariante کے ساتھ rejeitar کرتے ہیں۔
  - Operadores `SetParameter(Custom)` کے ذریعے `id = "max_contract_code_bytes"` اور carga útil numérica دے کر ایڈجسٹ کر سکتے ہیں۔

-POSTO `/v1/gov/ballots/zk`
  - Solicitação: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas:
    - جب circuito کے entradas públicas میں `owner`, `amount`, `duration_blocks` شامل ہوں اور prova configurada VK کے خلاف verificar ہو جائے تو nó `election_id` کے لئے bloqueio de governança بناتا یا بڑھاتا ہے۔ direção چھپی رہتی ہے (`unknown`); صرف valor/validade اپڈیٹ ہوتے ہیں۔ re-votos monotônicos ہیں: quantidade اور expiração صرف بڑھتے ہیں (node ​​max(amount, prev.amount) اور max(expiry, prev.expiry) لگاتا ہے)۔
    - ZK re-votos جو valor یا expiração کم کرنے کی کوشش کریں diagnóstico `BallotRejected` do lado do servidor کے ساتھ rejeitar ہوتے ہیں۔
    - Execução de contrato کو `SubmitBallot` enfileiramento کرنے سے پہلے `ZK_VOTE_VERIFY_BALLOT` کال کرنا لازم ہے؛ hospeda a trava one-shot impor کرتے ہیں۔

-POSTO `/v1/gov/ballots/plain`
  - Solicitação: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sim|Não|Abstenção" }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas: revotações estendidas apenas ہیں - نیا cédula موجودہ bloqueio کا valor یا expiração کم نہیں کر سکتا۔ `owner` کو autoridade de transação کے برابر ہونا چاہئے۔ کم از کم مدت `conviction_step_blocks` ہے۔

-POSTO `/v1/gov/finalize`
  - Solicitação: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito na cadeia (andaime atual): منظور شدہ proposta de implantação کو promulgar کرنے سے `code_hash` com chave mínima `ContractManifest` شامل ہوتا ہے جس میں متوقع `abi_hash` ہوتا ہے اور proposta promulgada ہو جاتا ہے۔ اگر `code_hash` کے لئے مختلف `abi_hash` e manifesto پہلے سے ہو تو rejeição de promulgação ہوتا ہے۔
  - Notas:
    - Eleições ZK کے لئے caminhos de contrato کو `FinalizeElection` سے پہلے `ZK_VOTE_VERIFY_TALLY` کال کرنا لازمی ہے؛ hospeda a trava one-shot impor کرتے ہیں۔ `FinalizeReferendum` Referendos ZK کو اس وقت تک rejeitar کرتا ہے جب تک contagem eleitoral finalizada نہ ہو جائے۔
    - Fechamento automático `h_end` پر صرف Referendos simples کے لئے Emissão aprovada/rejeitada کرتا ہے؛ Referendos ZK Fechados رہتے ہیں جب تک envio de contagem finalizado نہ ہو اور `FinalizeReferendum` executar نہ ہو۔
    - Verificações de participação صرف aprovar + rejeitar استعمال کرتی ہیں؛ abster-se de comparecer میں شمار نہیں ہوتا۔-POSTO `/v1/gov/enact`
  - Solicitação: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: جب `authority`/`private_key` فراہم ہوں تو Torii transação assinada سبمٹ کرتا ہے؛ Um esqueleto e um esqueleto são um esqueleto que pode ser construído pré-imagem

- OBTER `/v1/gov/proposals/{id}`
  - Caminho `{id}`: ID da proposta hexadecimal (64 caracteres)
  - Resposta: { "encontrado": bool, "proposta": { ... }? }

- OBTER `/v1/gov/locks/{rid}`
  - Caminho `{rid}`: string de identificação do referendo
  - Resposta: { "encontrado": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTER `/v1/gov/council/current`
  - Resposta: { "época": N, "membros": [{ "account_id": "..." }, ...] }
  - Notas: Conselho موجودہ موجود ہو تو واپس کرتا ہے، ورنہ ativo de aposta configurado اور limites استعمال کر کے derivação determinística de fallback کرتا ہے (especificações VRF کو اس وقت تک refletir کرتا ہے جب تک provas VRF ao vivo na cadeia persistem نہ ہوں)۔

- POST `/v1/gov/council/derive-vrf` (recurso: gov_vrf)
  - Solicitação: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: ہر امیدوار کا Prova VRF `chain_id`, `epoch` اور تازہ ترین bloco hash beacon سے مشتق entrada canônica کے خلاف verificar کرتا ہے؛ bytes de saída کو desc ترتیب میں desempates کے ساتھ sort کرتا ہے؛ principais membros `committee_size` واپس کرتا ہے۔ Persistir
  - Resposta: { "época": N, "membros": [{ "account_id": "..." } ...], "total_candidates": M, "verificado": K }
  - Notas: Normal = pk em G1, prova em G2 (96 bytes). Pequeno = pk em G2, prova em G1 (48 bytes). Entradas separadas por domínio ہیں اور `chain_id` شامل ہے۔

### Padrões de governança (iroha_config `gov.*`)

Torii جب کوئی lista persistente نہ پائے تو conselho de fallback `iroha_config` کے ذریعے parametrizar کیا جاتا ہے:

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

Substituições de ambiente equivalentes:

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

`parliament_committee_size` membros substitutos کی تعداد کو limite کرتا ہے جب conselho persistir نہ ہو, `parliament_term_blocks` comprimento da época definir کرتا ہے جو derivação de sementes کے لئے استعمال ہوتا ہے (`epoch = floor(height / term_blocks)`), `parliament_min_stake` ativo de elegibilidade پر کم از کم participação (unidades menores) impor کرتا ہے, اور `parliament_eligibility_asset_id` منتخب کرتا ہے کہ candidatos بناتے وقت کس saldo de ativos کو scan کیا جائے۔

Verificação de governança VK کا کوئی ignorar نہیں: verificação de votação ہمیشہ `Active` chave de verificação اور bytes inline کا تقاضا کرتا ہے, اور ambientes کو alterna somente teste پر انحصار نہیں کرنا چاہئے۔

RBAC
- Execução on-chain کے لئے permissões درکار ہیں:
  - Propostas: `CanProposeContractDeployment{ contract_id }`
  - Cédulas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgação: `CanEnactGovernance`
  - Gestão do conselho (futuro): `CanManageParliament`

Namespaces protegidos
- Parâmetro personalizado `gov_protected_namespaces` (matriz JSON de strings) namespaces listados میں implantar کے لئے gate de admissão فعال کرتا ہے۔
- Clientes کو namespaces protegidos پر implantar کے لئے chaves de metadados de transação شامل کرنی ہوں گی:
  - `gov_namespace`: namespace de destino (como: "apps")
  - `gov_contract_id`: namespace کے اندر ID do contrato lógico
- `gov_manifest_approvers`: matriz JSON opcional de IDs de contas validadoras۔ جب quorum de manifesto de pista > 1 declaração کرے تو admissão کے لئے autoridade de transação اور contas listadas دونوں درکار ہوتے ہیں تاکہ quorum de manifesto پورا ہو سکے۔
- Telemetria `governance_manifest_admission_total{result}` کے ذریعے contadores de admissão holísticos دکھاتی ہے تاکہ operadores کامیاب admite کو `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, e `runtime_hook_rejected` são de alta qualidade
- Telemetria `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) کے ذریعے caminho de aplicação دکھاتی ہے تاکہ aprovações faltantes کا auditoria ہو سکے۔
- Lanes اپنے manifestos میں شائع شدہ lista de permissões de namespace کو impor کرتے ہیں۔ O nome do `gov_namespace` é o `gov_contract_id`, o namespace e o manifesto `protected_namespaces` سیٹ میں ہونا چاہئے۔ `RegisterSmartContractCode` submissões بغیر metadados کے, جب proteção ativar ہو, rejeitar ہوتے ہیں۔
- Admissão اس بات کو fazer cumprir کرتا ہے کہ `(namespace, contract_id, code_hash, abi_hash)` کے لئے Proposta de governança promulgada موجود ہو؛ Erro de validação NotPermitted کے ساتھ falha ہوتا ہے۔Ganchos de atualização de tempo de execução
- Manifestos de pista `hooks.runtime_upgrade` declaram کر سکتے ہیں تاکہ instruções de atualização de tempo de execução (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`) کو gate کیا جا سکے۔
- Campos de gancho:
  - `allow` (bool, padrão `true`): جب `false` ہو تو تمام instruções de atualização de tempo de execução rejeitadas ہو جاتے ہیں۔
  - `require_metadata` (bool, padrão `false`): `metadata_key` کے مطابق entrada de metadados درکار ہے۔
  - `metadata_key` (string): gancho کا nome de metadados aplicados۔ Padrão `gov_upgrade_id` جب metadados necessários ہو یا lista de permissões موجود ہو۔
  - `allowed_ids` (matriz de strings): valores de metadados کی lista de permissões opcional (trim کے بعد)۔ اگر دیا گیا valor فہرست میں نہ ہو تو rejeitar۔
- Hook موجود ہو تو admissão de fila ٹرانزیکشن کے fila میں جانے سے پہلے política de metadados impor کرتا ہے۔ Metadados ausentes, valores não permitidos, lista de permissões, valores determinísticos, erro NotPerMITido, erros determinísticos
- Telemetria `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}` کے ذریعے faixa de resultados کرتی ہے۔
- Hook کی تسکین کرنے والی ٹرانزیکشنز کو metadados `gov_upgrade_id=<value>` (یا chave definida pelo manifesto) شامل کرنا ہوگا, ساتھ ہی quorum de manifesto کے مطابق validadores کی aprovações بھی درکار ہیں۔

Ponto final de conveniência
- POST `/v1/gov/protected-namespaces` - `gov_protected_namespaces` کو براہ راست node پر apply کرتا ہے۔
  - Solicitação: { "namespaces": ["apps", "system"] }
  - Resposta: { "ok": verdadeiro, "aplicado": 1 }
  - Notas: admin/testes کے لئے ہے؛ Configurar e configurar token de API produção کے لئے `SetParameter(Custom)` کے ساتھ transação assinada ترجیح دیں۔

Ajudantes CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - namespace کے instâncias de contrato busca کرتا ہے اور verificação cruzada کرتا ہے کہ:
    - Torii ہر `code_hash` کے لئے bytecode ذخیرہ کرتا ہے، اور اس کا Blake2b-32 digest `code_hash` سے match کرتا ہے۔
    - `/v1/contracts/code/{code_hash}` میں موجود manifesto correspondente `code_hash` e `abi_hash` valores رپورٹ کرتا ہے۔
    - `(namespace, contract_id, code_hash, abi_hash)` کے لئے proposta de governança promulgada موجود ہے جو اسی hashing de id de proposta سے deriva ہوتا ہے جو nó استعمال کرتا ہے۔
  - `results[]` کے ساتھ JSON رپورٹ دیتا ہے (problemas, manifesto/código/resumos de propostas) اور ایک لائن کا خلاصہ (اگر `--no-summary` نہ ہو)۔
  - Namespaces protegidos کے auditoria یا fluxos de trabalho de implantação controlados por governança کی تصدیق کے لئے مفید۔
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Namespaces protegidos میں implantação کے لئے esqueleto de metadados JSON دیتا ہے، جس میں opcional `gov_manifest_approvers` شامل ہیں تاکہ regras de quorum de manifesto پوری ہوں۔
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — `min_bond_amount > 0` ہونے پر dicas de bloqueio لازم ہیں, اور فراہم کیے گئے کسی بھی dicas سیٹ میں `owner`, `amount` e `duration_blocks` são de alta qualidade
  - Valida IDs de contas canônicas, canoniza dicas de nulificador de 32 bytes e mescla as dicas em `public_inputs_json` (com `--public <path>` para substituições adicionais).
  - O anulador é derivado do compromisso de prova (entrada pública) mais `domain_tag`, `chain_id` e `election_id`; `--nullifier` é validado em relação à prova quando fornecido.
  - ایک لائن خلاصہ اب `fingerprint=<hex>` determinístico دکھاتا ہے جو `CastZkBallot` codificado سے derivar ہوتا ہے, ساتھ dicas decodificadas (`owner`, `amount`, `duration_blocks`, `direction` جب فراہم ہوں)۔
  - Respostas CLI `tx_instructions[]` کو `payload_fingerprint_hex` اور campos decodificados کے ساتھ anotar کرتے ہیں تاکہ esqueleto de ferramentas downstream کو بغیر Norito decodificação کے verificar کر سکے۔
  - Dicas de bloqueio دینے سے cédulas ZK do nó کے لئے `LockCreated` / `LockExtended` eventos emitem کر سکتا ہے جب circuito وہی valores expõem کرے۔
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - `--lock-amount`/`--lock-duration-blocks` aliases ZK flags کے ناموں کو espelho کرتے ہیں تاکہ paridade de script ہو۔
  - Saída de resumo `vote --mode zk` کی طرح impressão digital de instrução codificada e campos de votação legíveis (`owner`, `amount`, `duration_blocks`, `direction`) شامل کرتا ہے، جس سے assinatura سے پہلے فوری تصدیق ہو جاتی ہے۔

Listagem de Instâncias
- GET `/v1/gov/instances/{ns}` - namespace کے لئے instâncias de contrato ativas کی فہرست۔
  - Parâmetros de consulta:
    - `contains`: `contract_id` کی substring کے مطابق filtro (diferencia maiúsculas de minúsculas)
    - `hash_prefix`: `code_hash_hex` کے prefixo hexadecimal کے مطابق filtro (minúsculas)
    - `offset` (padrão 0), `limit` (padrão 100, máximo 10_000)
    - `order`: `cid_asc` (padrão), `cid_desc`, `hash_asc`, `hash_desc`
  - Resposta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Auxiliar SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python)۔Desbloquear varredura (operador/auditoria)
- OBTER `/v1/gov/unlocks/stats`
  - Resposta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` سب سے حالیہ altura do bloco دکھاتا ہے جہاں varredura de bloqueios expirados اور persist کئے گئے۔ `expired_locks_now` ان lock records کو scan کر کے نکلتا ہے جن میں `expiry_height <= height_current` ہو۔
-POSTO `/v1/gov/ballots/zk-v1`
  - Solicitação (DTO estilo v1):
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
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (recurso: `zk-ballot`)
  - `BallotProof` JSON براہ راست قبول کر کے Esqueleto `CastZkBallot` واپس کرتا ہے۔
  - Solicitação:
    {
      "autoridade": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "votação": {
        "back-end": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // contêiner ZK1 e H2* em base64
        "root_hint": null, // string hexadecimal opcional de 32 bytes (raiz de elegibilidade)
        "proprietário": null, // opcional AccountId ou commit do proprietário do circuito کرے
        "nullifier": null // string hexadecimal opcional de 32 bytes (dica do anulador)
      }
    }
  - Resposta:
    {
      "ok": verdade,
      "aceito": verdadeiro,
      "reason": "construir esqueleto da transação",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notas:
    - Servidor opcional `root_hint`/`owner`/`nullifier` کو cédula سے `CastZkBallot` کے `public_inputs_json` میں mapa کرتا ہے۔
    - Bytes de envelope کو carga útil de instrução کے لئے base64 میں دوبارہ codificação کیا جاتا ہے۔
    - جب Torii votação enviada کرتا ہے تو `reason` بدل کر `submitted transaction` ہو جاتا ہے۔
    - یہ endpoint صرف تب دستیاب ہے جب `zk-ballot` recurso habilitado ہو۔

Caminho de verificação CastZkBallot
- `CastZkBallot` فراہم کردہ decodificação de prova base64 کرتا ہے اور خالی یا خراب cargas úteis کو rejeitar کرتا ہے (`BallotRejected` com `invalid or empty proof`).
- Referendo anfitrião (`vk_ballot`) یا padrões de governança سے votação verificando resolução de chave کرتا ہے اور تقاضا کرتا ہے کہ registro موجود ہو, `Active` ہو، Os bytes inline são usados
- Bytes de chave de verificação armazenados کو `hash_vk` کے ساتھ دوبارہ hash کیا جاتا ہے؛ incompatibilidade de compromisso ہو تو verificação سے پہلے execução روک دیا جاتا ہے تاکہ entradas de registro adulteradas سے بچا جا سکے (`BallotRejected` com `verifying key commitment mismatch`).
- Bytes de prova `zk::verify_backend` کے ذریعے backend registrado کو despacho ہوتے ہیں؛ transcrições inválidas `BallotRejected` com `invalid proof` کے ساتھ ظاہر ہوتے ہیں اور instrução falha deterministicamente ہوتی ہے۔
- A prova deve expor um compromisso eleitoral e uma raiz de elegibilidade como contribuições públicas; a raiz deve corresponder ao `eligible_root` da eleição e o anulador derivado deve corresponder a qualquer dica fornecida.
- Provas bem-sucedidas `BallotAccepted` emitem کرتے ہیں؛ anuladores duplicados, raízes de elegibilidade obsoletas, یا bloquear regressões پھر بھی پہلے بیان کردہ motivos de rejeição دیتے ہیں۔

## Mau comportamento do validador e consenso conjunto

### Corte ou encarceramento do fluxo de trabalho

Consenso جب کوئی validador پروٹوکول کی خلاف ورزی کرے تو Norito codificado `Evidence` emite کرتا ہے۔ ہر carga útil na memória `EvidenceStore` میں آتا ہے اور اگر پہلے نہ دیکھا گیا ہو تو Mapa `consensus_evidence` apoiado por WSV میں materializar ہو جاتا ہے۔ `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocos `7200` padrão) سے پرانے ریکارڈ rejeitar ہو جاتے ہیں تاکہ arquivo limitado رہے, مگر rejeição کو operadores کے لئے log کیا جاتا ہے۔ As evidências no horizonte também respeitam `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o atraso reduzido `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`); a governança pode cancelar penalidades com `CancelConsensusEvidencePenalty` antes que a redução seja aplicada.

Ofensas reconhecidas `EvidenceKind` سے mapa um para um ہوتے ہیں؛ discriminantes مستحکم ہیں اور modelo de dados کے ذریعے impor ہوتے ہیں:

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
```- **DoublePrepare/DoubleCommit** - validador `(phase,height,view,epoch)` کے لئے متضاد hashes پر دستخط کئے۔
- **InvalidQc** - agregador نے ایسا commit certificado fofoca کیا جس کی شکل verificações determinísticas میں falha ہو ​​(مثلا خالی signer bitmap)۔
- **InvalidProposal** - líder نے ایسا بلاک propor کیا جو validação estrutural میں falhar ہو (مثلا regra de cadeia bloqueada توڑے)۔
- **Censura** — recibos de envio assinados mostram uma transação que nunca foi proposta/comprometida.

Operadores e cargas úteis de ferramentas inspecionam e retransmitem o seguinte:

- Torii: `GET /v1/sumeragi/evidence` ou `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, ou `... submit --evidence-hex <payload>`.

Governança کو bytes de evidência کو prova canônica کے طور پر tratar کرنا چاہئے:

1. **Carga útil جمع کریں** اس کے expirar ہونے سے پہلے۔ bytes brutos Norito کو metadados de altura/visualização کے ساتھ arquivo کریں۔
2. **Estágio de penalidade کریں** carga útil کو referendo یا instrução sudo میں incorporar کر کے (مثلا `Unregister::peer`)۔ Carga útil de execução کو دوبارہ validar کرتا ہے؛ malformado یا evidência obsoleta rejeitada deterministicamente ہوتی ہے۔
3. **Cronograma de topologia de acompanhamento کریں** تاکہ validador ofensivo فوراً واپس نہ آ سکے۔ Os fluxos são `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` lista atualizada کے ساتھ fila کئے جاتے ہیں۔
4. **نتائج auditoria کریں** `/v1/sumeragi/evidence` e `/v1/sumeragi/status` کے ذریعے تاکہ contador de evidências بڑھے اور governança نے remoção نافذ کیا ہو۔

### Sequenciamento de consenso conjunto

Consenso conjunto اس بات کی ضمانت دیتا ہے کہ validador de saída definir limite de bloco finalizar کرے اس سے پہلے کہ نیا definir propor شروع کرے۔ Tempo de execução جوڑی شدہ parâmetros کے ذریعے یہ regra impor کرتا ہے:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` کو **اسی بلاک** میں commit ہونا چاہیے۔ `mode_activation_height` کو atualizar لانے والے بلاک کی altura سے estritamente بڑا ہونا چاہیے, تاکہ کم از کم ایک بلاک lag ملے۔
- Proteção de configuração `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) ہے جو transferências de atraso zero کو روکتا ہے:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`) atrasa a redução do consenso para que a governança possa cancelar as penalidades antes que elas sejam aplicadas.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Tempo de execução اور Parâmetros de estágio CLI کو `/v1/sumeragi/params` اور `iroha --output-format text ops sumeragi params` کے ذریعے ظاہر کرتے ہیں, تاکہ alturas de ativação de operadores اور listas de validadores کی تصدیق کر سکیں۔
- Automação de governança کو ہمیشہ:
  1. evidência پر مبنی remoção (یا reintegração) فیصلہ finalizar کرنا چاہیے۔
  2. `mode_activation_height = h_current + activation_lag_blocks` کے ساتھ fila de reconfiguração de acompanhamento کرنا چاہیے۔
  3. `/v1/sumeragi/status` کی نگرانی کرنی چاہیے جب تک `effective_consensus_mode` متوقع interruptor de altura پر نہ ہو جائے۔

جو بھی validadores de script giram کرے یا slashing apply کرے اسے **ativação com atraso zero** یا parâmetros de transferência کو omitem نہیں کرنا چاہیے؛ ایسی transações rejeitadas ہو جاتی ہیں اور نیٹ ورک پچھلے modo میں رہتا ہے۔

## Superfícies de telemetria

- Exportação de atividade de governança de métricas Prometheus کرتے ہیں:
  - Propostas `governance_proposals_status{status}` (medidor) کی گنتی status کے حساب سے faixa کرتا ہے۔
  - `governance_protected_namespace_total{outcome}` (contador) اس وقت incremento ہوتا ہے جب namespaces protegidos کی admissão, implantação کو permitir یا rejeitar کرے۔
  - `governance_manifest_activations_total{event}` (contador) inserções de manifesto (`event="manifest_inserted"`) e vinculações de namespace (`event="instance_bound"`) registro کرتا ہے۔
- `/status` میں `governance` objeto شامل ہے جو contagens de propostas کو refletir کرتا ہے، relatório de totais de namespace protegido کرتا ہے، اور ativações de manifesto recentes (namespace, ID do contrato, código/hash ABI, altura do bloco, ativação timestamp) lista کرتا ہے۔ Operadores اس campo کو enquete کر کے تصدیق کر سکتے ہیں کہ promulgações نے manifestos اپڈیٹ کئے اور portas de namespace protegidas نافذ ہیں۔
- Modelo Grafana (`docs/source/grafana_governance_constraints.json`) e `telemetry.md` میں runbook de telemetria دکھاتا ہے کہ propostas travadas, ativações de manifesto ausentes, atualizações de tempo de execução e rejeições inesperadas de namespace protegido کے لئے alertas کیسے fio کئے جائیں۔