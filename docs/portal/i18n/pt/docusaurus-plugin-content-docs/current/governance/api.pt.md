---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Status: rascunho/esboco para acompanhar as tarefas de implementação de governança. As formas podem mudar durante a implementação. Determinismo e política RBAC são restrições normativas; Torii pode variar/submeter transações quando `authority` e `private_key` são fornecidos, caso contrário aos clientes constroem e submetem para `/transaction`.

Visão geral
- Todos os endpoints retornam JSON. Para fluxos que produzem transações, as respostas incluem `tx_instructions` - um array de uma ou mais instruções esqueleto:
  - `wire_id`: identificador de registro para o tipo de instrução
  - `payload_hex`: bytes de carga útil Norito (hex)
- Se `authority` e `private_key` forem fornecidos (ou `private_key` em DTOs de cédulas), Torii assina e submete a transação e ainda retorna `tx_instructions`.
- Caso contrário, os clientes montam uma SignedTransaction usando sua autoridade e chain_id, depois assinam e fazem POST para `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` retorna `GovernanceProposalResult` (normaliza campos status/kind), `ToriiClient.get_governance_referendum_typed` retorna `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` retorna `GovernanceTally`, `ToriiClient.get_governance_locks_typed` retorna `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` retorna `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` retorna `GovernanceInstancesPage`, impondo acesso tipado em toda a superfície de governança com exemplos de uso no README.
- Cliente Python leve (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` retornam bundles tipados `GovernanceInstructionDraft` (encapsulando o esqueleto `tx_instructions` do Torii), evitando parse manual de JSON quando scripts compõem fluxos Finalizar/Aprovar.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` expõe helpers tipados para propostas, referendos, contagens, bloqueios, estatísticas de desbloqueio, e agora `listGovernanceInstances(namespace, options)` mais os endpoints do conselho (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que os clientes Node.js possam paginar `/v1/gov/instances/{ns}` e direcionar fluxos de trabalho com VRF junto com a listagem existente de instâncias de contrato.

Pontos finais

-POSTO `/v1/gov/proposals/deploy-contract`
  - Requisição (JSON):
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
  - Validação: os nos canonizam `abi_hash` para o `abi_version` fornecido e rejeitam divergências. Para `abi_version = "v1"`, o valor esperado e `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API de contratos (implantar)
-POSTO `/v1/contracts/deploy`
  - Requisição: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamento: calcula `code_hash` a partir do corpo do programa IVM e `abi_hash` a partir do cabeçalho `abi_version`, depois submete `RegisterSmartContractCode` (manifesto) e `RegisterSmartContractBytes` (bytes `.to` completo) em nome de `authority`.
  - Resposta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> retorna o manifesto armazenado
    - GET `/v1/contracts/code-bytes/{code_hash}` -> retorna `{ code_b64 }`
-POSTO `/v1/contracts/instance`
  - Requisição: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamento: implanta o bytecode fornecido e ativa imediatamente o mapeamento `(namespace, contract_id)` via `ActivateContractInstance`.
  - Resposta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Serviço de alias
-POSTO `/v1/aliases/voprf/evaluate`
  - Requisição: { "blinded_element_hex": "..." }
  - Resposta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete a implementação do avaliador. Valor atual: `blake2b512-mock`.
  - Notas: avaliado mock determinístico que aplica Blake2b512 com separação de domínio `iroha.alias.voprf.mock.v1`. Destinado a um ferramental de teste ao pipeline VOPRF de produção ser integrado ao Iroha.
  - Erros: HTTP `400` em entrada hexadecimal malformada. Torii retorna um envelope Norito `ValidationFail::QueryFailed::Conversion` com uma mensagem de erro do decodificador.
-POSTO `/v1/aliases/resolve`
  - Requisição: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Resposta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: requer o runtime ISO bridge staging (`[iso_bridge.account_aliases]` em `iroha_config`). Torii normaliza aliases removendo espaços e convertendo para maiusculas antes de fazer a pesquisa. Retorno 404 quando o alias está ausente e 503 quando o runtime ISO bridge está desabilitado.
-POSTO `/v1/aliases/resolve_index`
  - Requisição: { "index": 0 }
  - Resposta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: índices de alias são atribuídos de forma determinística pela ordem de configuração (base 0). Clientes armazenam respostas offline para construir trilhas de auditoria de eventos de atestação de alias.

Limite de tamanho do código
- Parâmetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Controle o tamanho máximo permitido (em bytes) para armazenamento de código de contrato on-chain.
  - Padrão: 16 MiB. Os nos rejeitam `RegisterSmartContractBytes` quando o tamanho da imagem `.to` excede o limite com um erro de violação de invariante.
  - Os operadores podem ser ajustados via `SetParameter(Custom)` com `id = "max_contract_code_bytes"` e um payload numérico.

-POSTO `/v1/gov/ballots/zk`
  - Requisição: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas:
    - Quando os inputs públicos do circuito incluem `owner`, `amount` e `duration_blocks`, e a prova verifica contra a VK configurada, o no cria ou estende um lock de governança para `election_id` com esse `owner`. A direção permanece oculta (`unknown`); apenas valor/validade são atualizados. Re-votos são monotônicos: amount e expiry apenas aumentam (ou no aplica max(amount, prev.amount) e max(expiry, prev.expiry)).
    - Re-votos ZK que tentam reduzir quantidade ou expiração são rejeitados no servidor com diagnósticos `BallotRejected`.
    - A execução do contrato deve chamar `ZK_VOTE_VERIFY_BALLOT` antes de arquivar `SubmitBallot`; hosts impoem uma trava de uma única vez.

-POSTO `/v1/gov/ballots/plain`
  - Requisição: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sim|Não|Abstenção" }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas: revotações são de extensão apenas - uma nova votação não pode reduzir valor ou expiração do bloqueio existente. O `owner` deve igualar a autoridade da transação. Duração mínima e `conviction_step_blocks`.

-POSTO `/v1/gov/finalize`
  - Requisição: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito on-chain (scaffold atual): promulgar uma proposta de implantação aprovada inserir um `ContractManifest` minimo com chave `code_hash` com o `abi_hash` esperado e marcar a proposta como Enacted. Se um manifesto já existir para o `code_hash` com `abi_hash` diferente, o promulgação e rejeitado.
  - Notas:
    - Para eleicoes ZK, os caminhos do contrato devem chamar `ZK_VOTE_VERIFY_TALLY` antes de executar `FinalizeElection`; os hosts impõem uma trava de uso único. `FinalizeReferendum` rejeita referendos ZK até que o registro da eleição esteja finalizado.
    - O auto-fechamento em `h_end` emite Aprovado/Rejeitado apenas para referendos Simples; referendos ZK permanecem fechados até que um registro finalizado seja enviado e `FinalizeReferendum` seja executado.
    - As verificações de participação usam apenas aprovar+rejeitar; abster-se não conta para o comparecimento.-POSTO `/v1/gov/enact`
  - Requisição: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii submete a transação assinada quando `authority`/`private_key` são fornecidos; caso contrário, retorna um esqueleto para clientes, submeterem e submeterem. A pré-imagem e opcional e hoje informativa.

- OBTER `/v1/gov/proposals/{id}`
  - Caminho `{id}`: id da proposta hex (64 caracteres)
  - Resposta: { "encontrado": bool, "proposta": { ... }? }

- OBTER `/v1/gov/locks/{rid}`
  - Caminho `{rid}`: string de id de referendo
  - Resposta: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTER `/v1/gov/council/current`
  - Resposta: { "época": N, "membros": [{ "account_id": "..." }, ...] }
  - Notas: retorna o conselho persistido quando presente; caso contrário, deriva um fallback determinístico usando o ativo de participação configurado e limites (espelha a especificação VRF até que testes VRF em produção sejam persistentes on-chain).

- POST `/v1/gov/council/derive-vrf` (recurso: gov_vrf)
  - Requisição: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: verifica a prova VRF de cada candidato contra a entrada canônica derivada de `chain_id`, `epoch` e do último hash de bloco; ordena por bytes de saida desc com tiebreakers; retorna os principais `committee_size` membros. Não persista.
  - Resposta: { "época": N, "membros": [{ "account_id": "..." } ...], "total_candidates": M, "verificado": K }
  - Notas: Normal = pk em G1, prova em G2 (96 bytes). Pequeno = pk em G2, prova em G1 (48 bytes). As entradas são separadas por domínio e incluem `chain_id`.

### Padrões de governança (iroha_config `gov.*`)

O conselho fallback usado pelo Torii quando não existe roster persistido e parametrizado via `iroha_config`:

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

`parliament_committee_size` limita o número de membros fallback retornados quando não há conselho persistido, `parliament_term_blocks` define o comprimento da época usada para derivacao de seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` aplica o mínimo de participação (em unidades mínimas) no ativo de elegibilidade, e `parliament_eligibility_asset_id` selecione qual saldo de ativos e escaneado para construir o conjunto de candidatos.

A verificação de governança VK não tem bypass: a verificação de votação sempre requer uma chave `Active` com bytes inline, e os ambientes não devem depender de toggles de teste para pular a verificação.

RBAC
- Execução on-chain exige permissões:
  - Propostas: `CanProposeContractDeployment{ contract_id }`
  - Cédulas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgação: `CanEnactGovernance`
  - Gestão do conselho (futuro): `CanManageParliament`

Namespaces protegidos
- Parâmetro custom `gov_protected_namespaces` (JSON array de strings) habilita o gate de admissão para implantações em namespaces listados.
- Os clientes devem incluir chaves de metadados de transação para implantar que visam namespaces protegidos:
  - `gov_namespace`: namespace alvo (ex., "apps")
  - `gov_contract_id`: id do contrato lógico dentro do namespace
- `gov_manifest_approvers`: array JSON opcional de IDs de contas de validadores. Quando um manifesto declara quorum maior que um, a admissão requer autoridade da transação mais as contas envolvidas para satisfazer o quorum do manifesto.
- Telemetria expõe contadores de admissão via `governance_manifest_admission_total{result}` para que operadores distingam admitidos bem-sucedidos dos caminhos `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` e `runtime_hook_rejected`.
- Telemetria expõe o caminho de fiscalização via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para que operadores auditem aprovações faltantes.
- Lanes aplicam a lista de permissões de namespaces publicados em seus manifestos. Qualquer transação que defina `gov_namespace` deverá fornecer `gov_contract_id`, e o namespace deverá aparecer no conjunto `protected_namespaces` do manifesto. Submissões `RegisterSmartContractCode` sem esses metadados são rejeitadas quando a proteção está habilitada.
- Admissão impoe que exista uma proposta de governanca Promulgada para a tupla `(namespace, contract_id, code_hash, abi_hash)`; caso contrário, uma falha de validação com um erro NotPermitted.Ganchos de atualização de tempo de execução
- Manifestos de pista podem declarar `hooks.runtime_upgrade` para obter instruções de atualização de tempo de execução (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos do gancho:
  - `allow` (bool, padrão `true`): quando `false`, todas as instruções de atualização de tempo de execução são rejeitadas.
  - `require_metadata` (bool, padrão `false`): requer uma entrada de metadados especificada por `metadata_key`.
  - `metadata_key` (string): nome do metadado aplicado pelo hook. Padrão `gov_upgrade_id` quando metadados são exigidos ou possuem lista de permissões.
  - `allowed_ids` (array de strings): lista de permissões opcional de valores de metadados (após trim). Rejeita quando o valor fornecido não está listado.
- Quando o gancho está presente, a admissão de fila aplica-se à política de metadados antes de uma transação entrar na fila. Metadados ausentes, valores em branco ou fora da lista de permissões geram um erro NotPermitted determinístico.
- Resultados de telemetria rastreia via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- As transações que satisfazem o gancho devem incluir metadados `gov_upgrade_id=<value>` (ou a chave definida pelo manifesto) junto com quaisquer aprovações de validadores aplicáveis ​​pelo quorum do manifesto.

Ponto final de conveniência
- POST `/v1/gov/protected-namespaces` - aplica `gov_protected_namespaces` diretamente no no.
  - Requisição: { "namespaces": ["apps", "system"] }
  - Resposta: { "ok": verdadeiro, "aplicado": 1 }
  - Notas: destinadas a administração/teste; requer token de API configurado. Para produção, prefira enviar uma transação assinada com `SetParameter(Custom)`.

CLI de ajudantes
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Busque instâncias de contrato para o namespace e confira que:
    - Torii armazena bytecode para cada `code_hash`, e seu resumo Blake2b-32 corresponde ao `code_hash`.
    - O manifesto armazenado em `/v1/contracts/code/{code_hash}` reporta `code_hash` e `abi_hash` correspondente.
    - Existe uma proposta de governança promulgada para `(namespace, contract_id, code_hash, abi_hash)` derivada pelo mesmo hashing de propostas-id que o no usa.
  - Emite um relatorio JSON com `results[]` por contrato (issues, resumos de manifest/code/proposal) mais um resumo de uma linha a menos que suprimido (`--no-summary`).
  - Util para auditar namespaces protegidos ou verificar fluxos de implantação controlados por governança.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emite o esqueleto JSON de metadados usados ao submeter implantações em namespaces protegidos, incluindo `gov_manifest_approvers` questionário para satisfação de regras de quorum do manifesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — dicas de bloqueio são obrigatórias quando `min_bond_amount > 0`, e qualquer conjunto de dicas fornecido deve incluir `owner`, `amount` e `duration_blocks`.
  - Valida IDs de contas canônicas, canoniza dicas de nulificador de 32 bytes e mescla as dicas em `public_inputs_json` (com `--public <path>` para substituições adicionais).
  - O anulador é derivado do compromisso de prova (entrada pública) mais `domain_tag`, `chain_id` e `election_id`; `--nullifier` é validado em relação à prova quando fornecido.
  - O resumo de uma linha agora exibe `fingerprint=<hex>` determinístico derivado do `CastZkBallot` codificado junto com dicas decodificadas (`owner`, `amount`, `duration_blocks`, `direction` quando fornecido).
  - As respostas da CLI anotam `tx_instructions[]` com `payload_fingerprint_hex` mais campos decodificados para que ferramentas downstream verifiquem o esqueleto sem reimplementar a decodificação Norito.
  - Fornecer dicas de bloqueio permite que o não emita eventos `LockCreated`/`LockExtended` para votos ZK assim que o circuito expõe os mesmos valores.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Os aliases `--lock-amount`/`--lock-duration-blocks` espelham os nomes de flags ZK para paridade de script.
  - A saida de resumo `vote --mode zk` ao incluir a impressão digital da instrução codificada e campos de votação legíveis (`owner`, `amount`, `duration_blocks`, `direction`), proporcionando confirmação rápida antes de selecionar o esqueleto.Lista de instâncias
- GET `/v1/gov/instances/{ns}` - lista de instâncias de contrato ativas para um namespace.
  - Parâmetros de consulta:
    - `contains`: filtra por substring de `contract_id` (diferencia maiúsculas de minúsculas)
    - `hash_prefix`: filtro por prefixo hexadecimal de `code_hash_hex` (minúsculas)
    - `offset` (padrão 0), `limit` (padrão 100, máximo 10_000)
    - `order`: um de `cid_asc` (padrão), `cid_desc`, `hash_asc`, `hash_desc`
  - Resposta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Varredura de desbloqueios (Operador/Auditoria)
- OBTER `/v1/gov/unlocks/stats`
  - Resposta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` reflete a altura do bloco mais recente onde locks expirados foram varridos e persistidos. `expired_locks_now` e calculando ao varrer registros de lock com `expiry_height <= height_current`.
-POSTO `/v1/gov/ballots/zk-v1`
  - Requisição (DTO estilo v1):
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
  - Aceita um JSON `BallotProof` direto e retorna um esqueleto `CastZkBallot`.
  - Requisição:
    {
      "autoridade": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "votação": {
        "back-end": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 do container ZK1 ou H2*
        "root_hint": null, // string hexadecimal opcional de 32 bytes (raiz de elegibilidade)
        "owner": null, // AccountId opcional quando o circuito compromete o proprietário
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
    - O servidor mapeia `root_hint`/`owner`/`nullifier` questionário do ballot para `public_inputs_json` em `CastZkBallot`.
    - Os bytes do envelope são recodificados como base64 para o payload da instrução.
    - A resposta `reason` muda para `submitted transaction` quando Torii submete o voto.
    - Este endpoint está disponível quando o recurso `zk-ballot` está habilitado.

Caminho de verificação CastZkBallot
- `CastZkBallot` decodifica a prova base64 encontradas e rejeita payloads vazios ou malformados (`BallotRejected` com `invalid or empty proof`).
- O host resolve a chave selecionada do voto a partir do referendo (`vk_ballot`) ou padrões de governança e exige que o registro exista, utilize `Active` e contenha bytes inline.
- Bytes de chave selecionada armazenados são re-hasheados com `hash_vk`; qualquer incompatibilidade de compromisso aborta a execução antes da verificação para proteger contra entradas de registro adulteradas (`BallotRejected` com `verifying key commitment mismatch`).
- Bytes de prova são despachados ao backend registrados via `zk::verify_backend`; transcrições inválidas aparecem como `BallotRejected` com `invalid proof` e uma instrução falha de forma determinística.
- A prova deve expor um compromisso eleitoral e uma raiz de elegibilidade como contribuições públicas; a raiz deve corresponder ao `eligible_root` da eleição e o anulador derivado deve corresponder a qualquer dica fornecida.
- Provas bem-sucedidas emitem `BallotAccepted`; anuladores duplicados, raízes de elegibilidade obsoletos ou regressão de bloqueio continuam a produzir as razões de rejeição existentes descritas anteriormente neste documento.

## Mau comportamento de validadores e consenso conjunto

### Fluxo de corte e prisãoO consenso emite `Evidence` codificado em Norito quando um validador viola o protocolo. Cada payload chega ao `EvidenceStore` em memória e, ineditamente, e materializado no mapa `consensus_evidence` respaldado por WSV. Registros mais antigos que `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` blocos) são rejeitados para manter o arquivo limitado, mas a rejeicao e registrada para operadores. As evidências no horizonte também respeitam `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o atraso reduzido `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`); a governança pode cancelar penalidades com `CancelConsensusEvidencePenalty` antes que a redução seja aplicada.

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind`; os discriminantes são estaveis e impostos pelo modelo de dados:

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

- **DoublePrepare/DoubleCommit** - o validador assinado hashes conflitantes para a mesma tupla `(phase,height,view,epoch)`.
- **InvalidQc** - um agregador fofocado um certificado de commit cuja forma falha em verificações determinísticas (ex., bitmap de signers vazio).
- **InvalidProposal** - um líder prop os um bloco que falha na validação estrutural (ex., viola a regra de blockchain).
- **Censura** — recibos de envio assinados mostram uma transação que nunca foi proposta/comprometida.

Operadores e ferramentas podem funcionar e retransmitir payloads via:

-Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count` e `... submit --evidence-hex <payload>`.

A governança deve tratar os bytes de evidência como prova canônica:

1. **Colete o payload** antes de expirar. Arquive os bytes Norito brutos junto com metadados de altura/visualização.
2. **Preparar a deliberação** incorporando o payload em um referendo ou instrução sudo (ex., `Unregister::peer`). A execução revalidada do payload; evidência malformada ou obsoleta e rejeitada deterministicamente.
3. **Agendar a topologia de acompanhamento** para que o validador infrator não possa retornar imediatamente. Fluxos típicos enfileiram `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com o roster atualizado.
4. **Auditar resultados** via `/v1/sumeragi/evidence` e `/v1/sumeragi/status` para garantir que o contador de evidências avançou e que a governança aplicou a remoção.

### Sequenciamento de consenso conjunto

O consenso conjunto garante que o conjunto de validadores de saida finalize o bloco de fronteira antes que o novo conjunto chegue a proporção. O runtime aplica-se a regra via parâmetros pareados:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` serão confirmados no **mesmo bloco**. `mode_activation_height` deve ser maior que a altura do bloco que carregou a atualização, fornecendo ao menos um bloco de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o protetor de configuração que impede hand-offs com lag zero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`) atrasa a redução do consenso para que a governança possa cancelar as penalidades antes que elas sejam aplicadas.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- O runtime e a CLI expoem parâmetros staged via `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, para que os operadores confirmem alturas de ativação e listas de validadores.
- A automação de governança deve sempre:
  1. Finalizar a decisão de remoção (ou reintegro) respaldada por evidência.
  2. Enfileirar uma reconfiguração de acompanhamento com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorar `/v1/sumeragi/status` e `effective_consensus_mode` trocarte na altura esperada.

Qualquer script que rotacione validadores ou aplique slashing **não deve** tentar ativar o lag zero ou omitir os parâmetros de hand-off; tais transações são rejeitadas e deixam a rede no modo anterior.

## Superfícies de telemetria

- Métricas Prometheus exportam atividade de governança:
  - `governance_proposals_status{status}` (manômetro) rastreia contagens de propostas por status.
  - `governance_protected_namespace_total{outcome}` (contador) incrementa quando a admissão de namespaces protegidos permite ou rejeita um deploy.
  - `governance_manifest_activations_total{event}` (counter) registra inserções de manifesto (`event="manifest_inserted"`) e ligações de namespace (`event="instance_bound"`).
- `/status` inclui um objeto `governance` que reflete as contagens de propostas, relacionadas a todos os namespaces protegidos e listas ativadas recentes de manifesto (namespace, id do contrato, hash de código/ABI, altura do bloco, carimbo de data e hora de ativação). Os operadores consultam esse campo para confirmar se as promulgações atualizaram os manifestos podem e se as portas dos namespaces protegidos estão sendo aplicadas.
- Um template Grafana (`docs/source/grafana_governance_constraints.json`) e o runbook de telemetria em `telemetry.md` mostram como ligar alertas para propostas presas, ativações de manifestos ausentes, ou rejeições inesperadas de namespaces protegidos durante atualizações de tempo de execução.