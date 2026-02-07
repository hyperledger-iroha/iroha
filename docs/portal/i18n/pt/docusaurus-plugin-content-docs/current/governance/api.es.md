---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Estado: borrador/boceto para acompanhar as tarefas de implementação de governo. As formas podem mudar durante a implementação. O determinismo e a política RBAC são restrições normativas; Torii pode firmar/enviar transações quando for fornecido `authority` e `private_key`, contrariamente aos clientes que construíram e enviaram `/transaction`.

Resumo
- Todos os endpoints retornam JSON. Para fluxos que produzem transações, as respostas incluem `tx_instructions` - um conjunto de um ou mais instruções esqueléticas:
  - `wire_id`: identificador de registro para o tipo de instrução
  - `payload_hex`: bytes de carga útil Norito (hex)
- Se for fornecido `authority` e `private_key` (ou `private_key` em DTOs de cédulas), Torii firma e envia a transação e devolve `tx_instructions`.
- Pelo contrário, os clientes armam uma SignedTransaction usando autoridade e chain_id, depois firmam e fazem POST para `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` retorna `GovernanceProposalResult` (normaliza campos status/tipo), `ToriiClient.get_governance_referendum_typed` retorna `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` retorna `GovernanceTally`, `ToriiClient.get_governance_locks_typed` retorna `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` retorna `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` retorna `GovernanceInstancesPage`, imponiendo acesso tipado em toda a superfície de governo com exemplos de uso no README.
- Cliente ligero Python (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` devuelven pacotes tipados `GovernanceInstructionDraft` (envolvendo o esqueleto `tx_instructions` de Torii), evitando a análise JSON manual quando scripts componentes flujos Finalizar/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` expõe helpers tipados para propostas, referendos, contagens, bloqueios, estatísticas de desbloqueio, e agora `listGovernanceInstances(namespace, options)` mas os endpoints do conselho (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que os clientes Node.js possam paginar `/v1/gov/instances/{ns}` e conduzir fluxos respaldados por VRF junto com a lista de instâncias de contrato existentes.

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
      "autoridade": "ih58…?",
      "chave_privada": "...?"
    }
  - Resposta (JSON):
    { "ok": verdadeiro, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validação: os nodos canonizan `abi_hash` para o `abi_version` desde que e rechazan desajustes. Para `abi_version = "v1"`, o valor esperado é `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API de contratos (implantar)
-POSTO `/v1/contracts/deploy`
  - Solicitação: { "authority": "ih58...", "private_key": "...", "code_b64": "..." }
  - Comportamento: calcula `code_hash` do corpo do programa IVM e `abi_hash` do cabeçalho `abi_version`, depois envia `RegisterSmartContractCode` (manifica) e `RegisterSmartContractBytes` (bytes `.to` completo) em nome de `authority`.
  - Resposta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> retorna o manifesto armazenado
    - OBTER `/v1/contracts/code-bytes/{code_hash}` -> retornar `{ code_b64 }`
-POSTO `/v1/contracts/instance`
  - Solicitação: { "authority": "ih58...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamento: despligue o bytecode fornecido e ative imediatamente o mapa `(namespace, contract_id)` via `ActivateContractInstance`.
  - Resposta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Serviço de alias
-POSTO `/v1/aliases/voprf/evaluate`
  - Solicitação: { "blinded_element_hex": "..." }
  - Resposta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete a implementação do avaliador. Valor real: `blake2b512-mock`.
  - Notas: avaliador mock determinista que aplica Blake2b512 com separação de domínio `iroha.alias.voprf.mock.v1`. Projetado para ferramentas de teste até que o pipeline VOPRF de produção seja cabeado em Iroha.
  - Erros: HTTP `400` en input hex mal formado. Torii retorna um envelope Norito `ValidationFail::QueryFailed::Conversion` com a mensagem de erro do decodificador.
-POSTO `/v1/aliases/resolve`
  - Solicitação: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Resposta: { "alias": "GB82WEST12345698765432", "account_id": "ih58...", "index": 0, "source": "iso_bridge" }
  - Notas: requer o teste de ponte ISO em tempo de execução (`[iso_bridge.account_aliases]` e `iroha_config`). Torii normaliza alias eliminando espaços e passando por mayusculas antes da pesquisa. Devolve 404 quando o alias não existe e 503 quando a ponte ISO de tempo de execução está desativada.
-POSTO `/v1/aliases/resolve_index`
  - Solicitação: { "índice": 0 }
  - Resposta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "ih58...", "source": "iso_bridge" }
  - Notas: os índices de alias são atribuídos de forma determinista conforme a ordem de configuração (baseado em 0). Os clientes podem armazenar respostas offline para criar auditorias de eventos de atestação de alias.

Tope de tamanho de código
- Parâmetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Controle o tamanho máximo permitido (em bytes) para armazenamento de código de contrato on-chain.
  - Padrão: 16 MiB. Os nodos rechazan `RegisterSmartContractBytes` quando a imagem `.to` excede o topo com um erro de violação de invariante.
  - Os operadores podem ajustar o envio de `SetParameter(Custom)` com `id = "max_contract_code_bytes"` e uma carga útil numérica.

-POSTO `/v1/gov/ballots/zk`
  - Solicitação: { "autoridade": "ih58...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Resposta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas:
    - Quando as entradas públicas do circuito incluem `owner`, `amount` e `duration_blocks`, e a verificação verifica contra a configuração VK, o nó cria ou estende um bloqueio de governança para `election_id` com este `owner`. A direção permanece oculta (`unknown`); apenas se atualiza o valor/expiração. As rotações são monotônicas: quantidade e expiração só aumentam (o nó aplica max(amount, prev.amount) e max(expiry, prev.expiry)).
    - As revogações ZK que pretendem reduzir o valor ou a expiração são rechaçadas no lado do servidor com diagnóstico `BallotRejected`.
    - A execução do contrato deve ser chamada `ZK_VOTE_VERIFY_BALLOT` antes de colar `SubmitBallot`; os hosts impõem uma trava de uma só vez.

-POSTO `/v1/gov/ballots/plain`
  - Solicitação: { "autoridade": "ih58...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "ih58...", "amount": "1000", "duration_blocks": 6000, "direction": "Sim | Não | Abstenção" }
  - Resposta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas: as revogações são apenas de extensão - uma nova votação não pode reduzir o valor ou a expiração do bloqueio existente. El `owner` deve ser igual à autoridade da transação. A duração mínima é `conviction_step_blocks`.-POSTO `/v1/gov/finalize`
  - Solicitação: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "ih58…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito on-chain (e atual): promulgar uma proposta de implantação aprovada, inserir um `ContractManifest` mínimo com chave `code_hash` com o `abi_hash` esperado e marcar a proposta como Enacted. Se houver uma manifestação para o `code_hash` com um `abi_hash` diferente, a promulgação será rechazada.
  - Notas:
    - Para as eleições ZK, as regras do contrato devem ser chamadas `ZK_VOTE_VERIFY_TALLY` antes de executar `FinalizeElection`; os hosts impõem uma trava de uma só vez. `FinalizeReferendum` rechaza referendos ZK até que a contagem da eleição seja finalizada.
    - O fechamento automático em `h_end` emite Aprovado/Rejeitado apenas para referendos Simples; os referendos ZK permanecem fechados até que um registro finalizado seja enviado e executado `FinalizeReferendum`.
    - As comprovações de participação usam apenas aprovação+rejeição; abster-se de nenhuma conta para a participação.

-POSTO `/v1/gov/enact`
  - Solicitação: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "ih58…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii envia a transação firmada quando for fornecido `authority`/`private_key`; do contrário, devolva um esqueleto para que os clientes se firmem e invejem. A pré-imagem é opcional e atualmente informativa.

- OBTER `/v1/gov/proposals/{id}`
  - Caminho `{id}`: id da propriedade hexadecimal (64 caracteres)
  - Resposta: { "encontrado": bool, "proposta": { ... }? }

- OBTER `/v1/gov/locks/{rid}`
  - Caminho `{rid}`: string de id do referendo
  - Resposta: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTER `/v1/gov/council/current`
  - Resposta: { "época": N, "membros": [{ "account_id": "..." }, ...] }
  - Notas: devuelve el conselho persistido quando existe; do contrário, deriva um respaldo determinista usando o ativo de aposta configurado e umbrales (reflete a especificação VRF até que teste VRF ao vivo se persistir na cadeia).

- POST `/v1/gov/council/derive-vrf` (recurso: gov_vrf)
  - Solicitação: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: verifica a verificação VRF de cada candidato contra a entrada canônica derivada de `chain_id`, `epoch` e o farol do último hash de bloco; ordena por bytes de saída desc com critérios de desempate; devolva os principais membros `committee_size`. Não persista.
  - Resposta: { "época": N, "membros": [{ "account_id": "..." } ...], "total_candidates": M, "verificado": K }
  - Notas: Normal = pk em G1, prova em G2 (96 bytes). Pequeno = pk em G2, prova em G1 (48 bytes). As entradas estão separadas por domínio e incluem `chain_id`.

### Padrões de governo (iroha_config `gov.*`)

O conselho de respaldo usado por Torii quando não existe uma lista persistente é parametrizada via `iroha_config`:

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

Substituições de entorno equivalentes:

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

`parliament_committee_size` limita a quantidade de miembros de respaldo devueltos quando nenhum conselho persistido, `parliament_term_blocks` define a longitude de época usada para derivação de sementes (`epoch = floor(height / term_blocks)`), `parliament_min_stake` aplica o mínimo de participação (em unidades mínimas) sobre o ativo de elegibilidade, e `parliament_eligibility_asset_id` seleciona que o saldo de ativos será escaneado para construir o conjunto de candidatos.

A verificação de governança VK não é ignorada: a verificação de cédulas sempre requer uma chave selecionada `Active` com bytes inline, e os ambientes não dependem de alternâncias de teste para omitir a verificação.

RBAC
- A execução na cadeia requer permissões:
  - Propostas: `CanProposeContractDeployment{ contract_id }`
  - Cédulas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgação: `CanEnactGovernance`
  - Gestão do conselho (futuro): `CanManageParliament`Namespaces protegidos
- Parâmetro personalizado `gov_protected_namespaces` (array de strings JSON) habilita gateway de admissão para implantações em namespaces listados.
- Os clientes devem incluir chaves de metadados de transação para implantações direcionadas a namespaces protegidos:
  - `gov_namespace`: o objetivo do namespace (por exemplo, "apps")
  - `gov_contract_id`: o ID do contrato lógico dentro do namespace
- `gov_manifest_approvers`: array JSON opcional de IDs de contas de validadores. Quando um manifesto de via declara um quórum maior a um, a admissão requer a autoridade da transação, mas as contas convocadas para satisfazer o quórum do manifesto.
- A telemetria expõe contadores de admissão via `governance_manifest_admission_total{result}` para que operadores distintos admitam saídas de rotas `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected`, e `runtime_hook_rejected`.
- A telemetria expõe a rota de execução via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para que os operadores auditem aprovações faltantes.
- As faixas aplicam a lista de permissões de namespaces publicada em seus manifestos. Qualquer transação que o arquivo `gov_namespace` deve fornecer `gov_contract_id`, e o namespace deve aparecer no conjunto `protected_namespaces` do manifesto. Os envios `RegisterSmartContractCode` sem esses metadados serão rechazados quando a proteção estiver habilitada.
- A admissão impone que exista uma proposta de governança promulgada para a tupla `(namespace, contract_id, code_hash, abi_hash)`; ao contrário, a validação falhou com um erro NotPermitted.

Ganchos de atualização de tempo de execução
- Os manifestos de pista podem declarar `hooks.runtime_upgrade` para controlar instruções de atualização de tempo de execução (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos do gancho:
  - `allow` (bool, padrão `true`): quando é `false`, todas as instruções de atualização de tempo de execução são redefinidas.
  - `require_metadata` (bool, padrão `false`): exige a entrada de metadados especificada por `metadata_key`.
  - `metadata_key` (string): nome dos metadados aplicados pelo gancho. Padrão `gov_upgrade_id` quando é necessário metadados ou lista de permissões.
  - `allowed_ids` (array de strings): lista de permissões opcional de valores de metadados (tras trim). Rechaza cuando el valor fornecido não esta listado.
- Quando o gancho está presente, a admissão da cola aplica a política de metadados antes da transação entre a cola. Metadados ausentes, valores vazios ou valores fora da lista de permissões produziram um erro NotPermitted determinista.
- La telemetria rastrea resultados via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- As transações que completam o gancho devem incluir metadados `gov_upgrade_id=<value>` (ou a chave definida pelo manifesto) junto com qualquer aprovação de validadores exigida pelo quorum do manifesto.

Ponto final de conveniência
- POST `/v1/gov/protected-namespaces` - aplica `gov_protected_namespaces` diretamente no nodo.
  - Solicitação: { "namespaces": ["apps", "system"] }
  - Resposta: { "ok": true, "applied": 1 }
  - Notas: pensado para admin/testes; requer token API se estiver configurado. Para produção, prefira enviar uma transação firmada com `SetParameter(Custom)`.CLI de ajudantes
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Obtenha instâncias de contrato para o namespace e verifique se:
    - Torii armazena bytecode para cada `code_hash`, e seu resumo Blake2b-32 coincide com o `code_hash`.
    - O manifesto armazenado abaixo de `/v1/contracts/code/{code_hash}` reporta valores `code_hash` e `abi_hash` coincidentes.
    - Existe uma proposta de governança promulgada para `(namespace, contract_id, code_hash, abi_hash)` derivada do mesmo hashing de ID de proposta que usa o nodo.
  - Emite um relatório JSON com `results[]` por contrato (problemas, resumos de manifesto/código/proposta) mas um resumo de uma linha salva que é suprima (`--no-summary`).
  - Util para auditar namespaces protegidos ou verificar fluxos de implantação controlados por governo.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver ih58... --approver ih58...]`
  - Emite o esqueleto JSON de metadados usado para enviar implantações para namespaces protegidos, incluindo `gov_manifest_approvers` opcional para satisfazer as regras de quorum do manifesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner ih58... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — as dicas de bloqueio são obrigatórias quando `min_bond_amount > 0`, e qualquer conjunto de dicas fornecido deve incluir `owner`, `amount` e `duration_blocks`.
  - Valida IDs de contas canônicas, canoniza dicas de nulificador de 32 bytes e mescla as dicas em `public_inputs_json` (com `--public <path>` para substituições adicionais).
  - O anulador é derivado do compromisso de prova (entrada pública) mais `domain_tag`, `chain_id` e `election_id`; `--nullifier` é validado em relação à prova quando fornecido.
  - O resumo de uma linha agora expõe um determinista `fingerprint=<hex>` derivado de `CastZkBallot` codificado junto com dicas decodificadas (`owner`, `amount`, `duration_blocks`, `direction` quando se proporcionano).
  - As respostas CLI anotan `tx_instructions[]` com `payload_fingerprint_hex` mas campos decodificados para que as ferramentas downstream verifiquem o esqueleto sem reimplementar a decodificação Norito.
  - Provar as dicas de bloqueio permite que o nó emita eventos `LockCreated`/`LockExtended` para cédulas ZK uma vez que o circuito expongue os valores errados.
-`iroha app gov vote --mode plain --referendum-id <id> --owner ih58... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - O alias `--lock-amount`/`--lock-duration-blocks` reflete os nomes das bandeiras de ZK para paridade em scripts.
  - A saída do resumo refletido `vote --mode zk` inclui a impressão digital da instrução codificada e campos de cédula legíveis (`owner`, `amount`, `duration_blocks`, `direction`), oferecendo confirmação rápida antes de firmar o esqueleto.

Lista de instâncias
- GET `/v1/gov/instances/{ns}` - lista de instâncias de contrato ativas para um namespace.
  - Parâmetros de consulta:
    - `contains`: filtra por substring de `contract_id` (diferencia maiúsculas de minúsculas)
    - `hash_prefix`: filtra por prefijo hexadecimal de `code_hash_hex` (minúsculas)
    - `offset` (padrão 0), `limit` (padrão 100, máximo 10_000)
    - `order`: um de `cid_asc` (padrão), `cid_desc`, `hash_asc`, `hash_desc`
  - Resposta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Barrido de desbloqueio (Operador/Auditoria)
- OBTER `/v1/gov/unlocks/stats`
  - Resposta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` reflete a altura do bloco mais recente onde os bloqueios expirados foram barridos e persistentes. `expired_locks_now` é calculado escaneando registros de bloqueio com `expiry_height <= height_current`.
-POSTO `/v1/gov/ballots/zk-v1`
  - Solicitação (DTO estilo v1):
    {
      "autoridade": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "back-end": "halo2/ipa",
      "envelope_b64": "AAECAwQ=",
      "root_hint": "0x...64hex?",
      "proprietário": "ih58…?",
      "anulador": "blake2b32:...64hex?"
    }
  - Resposta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (recurso: `zk-ballot`)
  - Aceite um JSON `BallotProof` diretamente e desenvolva um esqueleto `CastZkBallot`.
  - Solicitação:
    {
      "autoridade": "ih58...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "votação": {
        "back-end": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 do conteúdo ZK1 ou H2*
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
    - O servidor mapeado `root_hint`/`owner`/`nullifier` é opcional desde a cédula até `public_inputs_json` para `CastZkBallot`.
    - Os bytes do envelope são recodificados como base64 para a carga útil da instrução.
    - A resposta `reason` muda para `submitted transaction` quando Torii envia a cédula.
    - Este endpoint só está disponível quando o recurso `zk-ballot` está habilitado.

Rota de verificação de CastZkBallot
- `CastZkBallot` decodifica a teste base64 provista e rechaza payloads vacios ou mal formados (`BallotRejected` com `invalid or empty proof`).
- O host resolve a chave selecionada da cédula do referendo (`vk_ballot`) ou padrões de governança e exige que o registro exista, como `Active`, e tenha bytes inline.
- Os bytes da chave selecionada são re-hashean com `hash_vk`; qualquer desajuste de compromisso abortar a execução antes de verificar para proteger contra entradas de registro adulteradas (`BallotRejected` com `verifying key commitment mismatch`).
- Os bytes da tentativa são enviados para o backend registrado via `zk::verify_backend`; transcrições invalidas aparecem como `BallotRejected` com `invalid proof` e a instrução falhou deterministamente.
- A prova deve expor um compromisso eleitoral e uma raiz de elegibilidade como contribuições públicas; a raiz deve corresponder ao `eligible_root` da eleição e o anulador derivado deve corresponder a qualquer dica fornecida.
- Testes exitosos emitidos `BallotAccepted`; nulificadores duplicados, raízes de elegibilidade antigas ou regressões de bloqueio continuam produzindo as razões de rechazo existentes descritas antes neste documento.

## Mala conduta de validadores e consenso conjunto

### Fluxo de corte e prisão

O consenso emite `Evidence` codificado em Norito quando um validador viola o protocolo. Cada carga útil chega ao `EvidenceStore` na memória e, se não for violada antes, é materializada no mapa `consensus_evidence` respaldado pelo WSV. Os registros anteriores a `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocos `7200` padrão) são rechaçados para que o arquivo permaneça acotado, mas o rechazo é registrado para os operadores. As evidências no horizonte também respeitam `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o atraso reduzido `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`); a governança pode cancelar penalidades com `CancelConsensusEvidencePenalty` antes que a redução seja aplicada.

As ofensas reconhecidas são mapeadas um a um em `EvidenceKind`; Os discriminantes são estabelecidos e foram reforçados pelo modelo de dados:

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

- **DoublePrepare/DoubleCommit** - o validador firma hashes em conflito com a mesma tupla `(phase,height,view,epoch)`.
- **InvalidQc** - um agregador fofoca um certificado de commit cuja forma falha em cheques deterministas (por exemplo, bitmap de firmantes vacio).
- **InvalidProposal** - um líder que propõe um bloco que falha na validação estrutural (por exemplo, romper a regra da cadeia bloqueada).
- **Censura** — recibos de envio assinados mostram uma transação que nunca foi proposta/comprometida.

Operadores e ferramentas podem inspecionar e retransmitir cargas úteis através de:

- Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count` e `... submit --evidence-hex <payload>`.

A governança deve tratar os bytes de evidência como prova canônica:1. **Recolha a carga útil** antes desse início. Arquiva os bytes Norito brutos junto com metadados de altura/visualização.
2. **Preparar a penalidade** incorporando a carga útil em um referendo ou instrução sudo (por exemplo, `Unregister::peer`). A execução revalida a carga útil; evidência mal formada ou rancia se rechaza deterministamente.
3. **Programe a topologia de acompanhamento** para que o validador infrator não possa reingressar imediatamente. Fluxos típicos incluem `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com a lista atualizada.
4. **Auditar resultados** via `/v1/sumeragi/evidence` e `/v1/sumeragi/status` para garantir que o contador de evidências avançou e que o governo aplique a remoção.

### Sequência de consenso conjunto

O conjunto de consenso garante que o conjunto de validadores salientes finalize o bloco de fronteira antes que o novo conjunto empregue um proponente. O tempo de execução impõe a regra por meio de parâmetros pareados:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` devem ser confirmados no **mismo bloco**. `mode_activation_height` deve ser estritamente maior que a altura do bloco que carrega a atualização, proporcionando al menos um bloco de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) é o protetor de configuração que evita transferências com atraso zero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`) atrasa a redução do consenso para que a governança possa cancelar as penalidades antes que elas sejam aplicadas.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- O tempo de execução e a CLI expõem parâmetros encenados via `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, para que os operadores confirmem alturas de ativação e listas de validadores.
- A automatização da governança sempre deve:
  1. Finalizar a decisão de remoção (ou reinstalação) respaldada por evidência.
  2. Selecione uma reconfiguração de seguimento com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitore `/v1/sumeragi/status` até que `effective_consensus_mode` mude para a altura esperada.

Qualquer script que valida automaticamente ou aplica slashing **não deve** tentar ativar com atraso zero ou omitir os parâmetros de entrega; Essas transações são rechazanadas e deixadas de lado no modo anterior.

## Superfícies de telemetria

- As métricas Prometheus exportam atividades de governo:
  - `governance_proposals_status{status}` (manômetro) rastreia conteúdos de propuestas por estado.
  - `governance_protected_namespace_total{outcome}` (counter) incrementa quando a admissão de namespaces protegidos permite o rechaza un deploy.
  - `governance_manifest_activations_total{event}` (contador) registra inserções de manifesto (`event="manifest_inserted"`) e ligações de namespace (`event="instance_bound"`).
- `/status` inclui um objeto `governance` que reflete os conteúdos de propostas, relata todos os namespaces protegidos e lista as ativações recentes do manifesto (namespace, ID do contrato, hash de código/ABI, altura do bloco, carimbo de data/hora de ativação). Os operadores podem consultar este campo para confirmar se as promulgações são atualizadas e se as portas dos namespaces protegidos se aplicam.
- Uma planta Grafana (`docs/source/grafana_governance_constraints.json`) e o runbook de telemetria em `telemetry.md` exibem como enviar alertas para propuestas atascadas, ativações de manifesto faltantes, ou rechazos inesperados de namespaces protegidos durante atualizações de tempo de execução.