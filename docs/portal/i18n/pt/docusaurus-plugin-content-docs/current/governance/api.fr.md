---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Estatuto: brouillon/esquisse para acompanhar as tabelas de implementação da governança. As formas podem ser alteradas durante a implementação. O determinismo e a política RBAC são contraintes normativos; Torii pode assinar/soumettre transações quando `authority` e `private_key` são fornecidos, desde que os clientes construam e sejam substituídos por `/transaction`.

Apercu
- Todos os endpoints enviados pelo JSON. Para o fluxo que produz transações, as respostas incluem `tx_instructions` - um quadro de uma ou mais instruções de instruções:
  - `wire_id`: identificador de registro para o tipo de instrução
  - `payload_hex`: bytes de carga útil Norito (hex)
- Si `authority` e `private_key` são fornecidos (ou `private_key` no DTO das cédulas), Torii assina e envia a transação e o reenvio quando meme `tx_instructions`.
- Sinon, os clientes montam uma SignedTransaction com sua autoridade e chain_id, depois assinam e POST versão `/transaction`.
- SDK de cobertura:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` reenvio `GovernanceProposalResult` (normalizar o status/tipo dos campeonatos), `ToriiClient.get_governance_referendum_typed` reenvio `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` reenvio `GovernanceTally`, `ToriiClient.get_governance_locks_typed` reenvio `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` reenvio `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` reenvio `GovernanceInstancesPage`, impondo um tipo de acesso em toute a superfície de governança com exemplos de uso no README.
- Texto Python do cliente (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` envia pacotes do tipo `GovernanceInstructionDraft` (que encapsula o esqueleto `tx_instructions` de Torii), evitando a análise JSON manuel quando os scripts compõem o fluxo Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` expõe os tipos de ajudantes para propostas, referendos, contagens, bloqueios, estatísticas de desbloqueio e manutenção de `listGovernanceInstances(namespace, options)` mais o conselho de endpoints (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que os clientes Node.js possam acessar a página `/v1/gov/instances/{ns}` e pilotar os fluxos de trabalho VRF em paralelo à listagem de instâncias de contratos existentes.

Pontos finais

-POSTO `/v1/gov/proposals/deploy-contract`
  - Requete (JSON):
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
  - Validação: les noeuds canonisent `abi_hash` pour l'`abi_version` fourni et rejettent les incoerences. Para `abi_version = "v1"`, o valor do atendimento é `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

Contratos de API (implantação)
-POSTO `/v1/contracts/deploy`
  - Requete: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamento: calcule `code_hash` a partir do corpo do programa IVM e `abi_hash` a partir de `abi_version`, depois soumet `RegisterSmartContractCode` (manifesto) e `RegisterSmartContractBytes` (bytes `.to` completos) para `authority`.
  - Resposta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Mentira:
    - GET `/v1/contracts/code/{code_hash}` -> reenviar o manifesto em estoque
    - OBTER `/v1/contracts/code-bytes/{code_hash}` -> reenvio `{ code_b64 }`
-POSTO `/v1/contracts/instance`
  - Requete: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamento: implemente o bytecode fornecido e ative imediatamente o mapeamento `(namespace, contract_id)` via `ActivateContractInstance`.
  - Resposta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Serviço de alias
-POSTO `/v1/aliases/voprf/evaluate`
  -Requete: { "blinded_element_hex": "..." }
  - Resposta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete a implementação do avaliador. Valor atual: `blake2b512-mock`.
  - Notas: avaliador simulado determinista que aplica Blake2b512 com separação do domínio `iroha.alias.voprf.mock.v1`. Antes de iniciar o teste, basta que o pipeline VOPRF de produção dependa de Iroha.
  - Erros: HTTP `400` na entrada hexadecimal de formato incorreto. Torii envia um envelope Norito `ValidationFail::QueryFailed::Conversion` com mensagem de erro do decodificador.
-POSTO `/v1/aliases/resolve`
  - Requete: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Resposta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: requer o teste de ponte ISO em tempo de execução (`[iso_bridge.account_aliases]` e `iroha_config`). Torii normaliza o alias, retirando os espaços e colocando letras maiúsculas antes da pesquisa. Retorne 404 se o alias estiver ausente e 503 se a ponte ISO de tempo de execução estiver desativada.
-POSTO `/v1/aliases/resolve_index`
  - Requete: { "index": 0 }
  - Resposta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: os índices de alias são atribuídos deterministicamente de acordo com a ordem de configuração (baseado em 0). Os clientes podem colocar um cache fora da linha para construir pistas de auditoria para eventos de atestado de alias.

Cap de taille de código
- Parâmetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Controle a quantidade máxima autorizada (em bytes) para armazenar o código do contrato on-chain.
  - Padrão: 16 MiB. As noeus rejeitaram `RegisterSmartContractBytes` quando a cauda da imagem `.to` deixou a tampa com um erro de invariante.
  - Os operadores podem ser ajustados via `SetParameter(Custom)` com `id = "max_contract_code_bytes"` e um número de carga útil.

-POSTO `/v1/gov/ballots/zk`
  - Requete: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas:
    - Quando as entradas públicas do circuito incluem `owner`, `amount` e `duration_blocks`, e que você verifique previamente a configuração do VK, ele criará ou estabelecerá uma versão de governo para `election_id` com ce `owner`. La direção resto cachee (`unknown`); seu valor/expiração não foi atualizado. Os re-votos são monótonos: amount et expiration ne font qu'augmenter (le noeud applique max(amount, prev.amount) et max(expiry, prev.expiry)).
    - Os re-votos ZK que tentam reduzir o valor ou a expiração são rejeitados pelo servidor com o diagnóstico `BallotRejected`.
    - A execução do contrato deve ser feita pelo `ZK_VOTE_VERIFY_BALLOT` antes do arquivo `SubmitBallot`; Os hosts impõem uma trava apenas uma vez.

-POSTO `/v1/gov/ballots/plain`
  - Requete: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sim|Não|Abstenção" }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas: les re-votos são em extensão única - uma nova cédula não pode reduzir o valor ou o vencimento do verrou existente. Le `owner` iguala a autoridade da transação. A duração mínima é `conviction_step_blocks`.-POSTO `/v1/gov/finalize`
  - Requete: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito on-chain (scaffold atual): executar uma proposta de implantação aprovada, inserir um `ContractManifest` mínimo cle `code_hash` com l'`abi_hash` atender e marcar a proposta promulgada. Se um manifesto já existir para o `code_hash` com um `abi_hash` diferente, a promulgação será rejeitada.
  - Notas:
    - Para as eleições ZK, os caminhos do contrato devem chamar `ZK_VOTE_VERIFY_TALLY` antes de executar `FinalizeElection`; Os hosts impõem um bloqueio de uso exclusivo. `FinalizeReferendum` rejeitou os referendos ZK até que a contagem não seja finalizada.
    - La cloture automatique a `h_end` emet Approved/Rejected only for les referendums Plain; les referendums ZK restent Closed jusqu'a ce qu'un tally finalize soit soumis et que `FinalizeReferendum` soit execute.
    - Les verificações de comparecimento utilizando apenas aprovação + rejeição; abster-se de não contar com a participação.

-POSTO `/v1/gov/enact`
  - Requete: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii é o signatário da transação quando `authority`/`private_key` é fornecido; sem reenviar uma planilha para assinatura e envio do cliente. A pré-imagem é opcional e informativa para o instante.

- OBTER `/v1/gov/proposals/{id}`
  - Caminho `{id}`: id de proposição hexadecimal (64 caracteres)
  - Resposta: { "encontrado": bool, "proposta": { ... }? }

- OBTER `/v1/gov/locks/{rid}`
  - Caminho `{rid}`: string id de referendo
  - Resposta: { "encontrado": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTER `/v1/gov/council/current`
  - Resposta: { "época": N, "membros": [{ "account_id": "..." }, ...] }
  - Notas: o reenvio do conselho persiste se estiver presente; você não deriva um substituto determinado com a configuração do ativo de aposta e os seus próprios (o espelho da especificação VRF é exatamente o mesmo que os testes de VRF em directo são persistentes na cadeia).

- POST `/v1/gov/council/derive-vrf` (recurso: gov_vrf)
  - Requete: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: verifique o VRF anterior de cada candidato com a entrada canônica derivada de `chain_id`, `epoch` e o farol do último hash de bloco; tente par bytes de sortie desc com desempates; reenvie os principais membros `committee_size`. Não persista.
  - Resposta: { "época": N, "membros": [{ "account_id": "..." } ...], "total_candidates": M, "verificado": K }
  - Notas: Normal = pk em G1, prova em G2 (96 bytes). Pequeno = pk em G2, prova em G1 (48 bytes). As entradas são separadas por domínio e incluem `chain_id`.

### Padrões de governo (iroha_config `gov.*`)

O substituto do conselho utiliza par Torii quando alguma lista persiste e não existe é o parâmetro via `iroha_config`:

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

Substitui equivalentes de ambiente:

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

`parliament_committee_size` limita o número de membros substitutos enviados quando algum conselho não persiste, `parliament_term_blocks` define o tempo de época utilizado para a derivação de sementes (`epoch = floor(height / term_blocks)`), `parliament_min_stake` impõe o mínimo de participação (em unidades mínimas) em o ativo elegível, e a seleção `parliament_eligibility_asset_id` que a solda do ativo é escaneada durante a construção do conjunto de candidatos.

A verificação de governança VK não é ignorada: a verificação da cédula requer sempre um cle `Active` com bytes inline, e os ambientes não devem ser pressionados nos botões de teste para salvar a verificação.

RBAC
- A execução on-chain requer permissões:
  - Propostas: `CanProposeContractDeployment{ contract_id }`
  - Cédulas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgação: `CanEnactGovernance`
  - Gestão do conselho (futuro): `CanManageParliament`Protegidos de namespaces
- Parâmetro personalizado `gov_protected_namespaces` (matriz JSON de strings) ativo para acesso às implantações nas listas de namespaces.
- Os clientes devem incluir metadados de transação para implantações em namespaces protegidos:
  - `gov_namespace`: o namespace cible (ex., "apps")
  - `gov_contract_id`: ID da lógica de contrato no namespace
- `gov_manifest_approvers`: opção de array JSON para IDs de conta de validadores. Quando um manifesto de via declara um quorum > 1, a admissão requer a autoridade da transação mais as listas de contas para satisfazer o quorum do manifesto.
- A telemetria expõe os compteurs de admissão via `governance_manifest_admission_total{result}` para que os operadores distingam as admissões reussis des chemins `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` et `runtime_hook_rejected`.
- A telemetria expõe o caminho de execução via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para auditar as aprovações manquantes.
- As pistas aplicam a lista de permissões de namespaces publicados em seus manifestos. Toda transação que fixa `gov_namespace` é fornecida por `gov_contract_id`, e o namespace é exibido no conjunto `protected_namespaces` do manifesto. Os depósitos `RegisterSmartContractCode` sem esses metadados são rejeitados quando a proteção está ativa.
- A admissão impõe uma proposta de governo promulgada que existe para a tupla `(namespace, contract_id, code_hash, abi_hash)`; a validação foi repetida com um erro NotPermitted.

Ganchos de atualização de tempo de execução
- Os manifestos da pista podem ser declarados `hooks.runtime_upgrade` para obter instruções de atualização em tempo de execução (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Champs du gancho:
  - `allow` (bool, padrão `true`): quando `false`, todas as instruções de atualização de tempo de execução foram rejeitadas.
  - `require_metadata` (bool, padrão `false`): exige a entrada de metadados especificados por `metadata_key`.
  - `metadata_key` (string): nome do aplicativo de metadados pelo gancho. Padrão `gov_upgrade_id` quando os metadados são necessários ou quando uma lista de permissões está presente.
  - `allowed_ids` (matriz de strings): lista de permissões de opções de metadados de valores (após corte). Rejeite quando o valor da compra não for listado.
- Quando o gancho estiver presente, a entrada do arquivo aplica os metadados políticos antes da entrada da transação no arquivo. Metadados perdidos, valores de vídeo ou fora da lista de permissões produziram um erro notpermitido determinado.
- A telemetria rastreia os resultados via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- As transações que satisfazem o gancho devem incluir os metadados `gov_upgrade_id=<value>` (ou o código definido pelo manifesto) e mais as aprovações de validadores exigidas pelo quorum do manifesto.

Endpoint de commodities
- POST `/v1/gov/protected-namespaces` - aplique `gov_protected_namespaces` diretamente no noeud.
  - Requete: { "namespaces": ["apps", "system"] }
  - Resposta: { "ok": verdadeiro, "aplicado": 1 }
  - Notas: destino a l'admin/testing; requer uma API de token para configurar. Para a produção, prefira um signatário da transação com `SetParameter(Custom)`.CLI de ajudantes
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Recupere as instâncias de contrato para o namespace e verifique se:
    - Torii armazena o bytecode para cada `code_hash`, e seu resumo Blake2b-32 corresponde a `code_hash`.
    - O manifesto stocke sous `/v1/contracts/code/{code_hash}` reporta os valores `code_hash` e `abi_hash` correspondentes.
    - Existe uma proposta de governo promulgada para `(namespace, contract_id, code_hash, abi_hash)` derivada do meme hashing do ID da proposta que o noeud utiliza.
  - Classifique um relatório JSON com `results[]` por contrato (problemas, currículos de manifesto/código/proposta) mais um currículo em uma linha segura de supressão (`--no-summary`).
  - Útil para auditar namespaces protegidos ou verificar fluxos de trabalho de implantação de controles de governança.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emita o modelo JSON de metadados utilizado durante implantações em namespaces protegidos, incluindo opções `gov_manifest_approvers` para satisfazer as regras de quorum do manifesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — as dicas de bloqueio são necessárias antes de `min_bond_amount > 0`, e todo o conjunto de dicas fornecidas inclui `owner`, `amount` e `duration_blocks`.
  - Valida IDs de contas canônicas, canoniza dicas de nulificador de 32 bytes e mescla as dicas em `public_inputs_json` (com `--public <path>` para substituições adicionais).
  - O anulador é derivado do compromisso de prova (entrada pública) mais `domain_tag`, `chain_id` e `election_id`; `--nullifier` é validado em relação à prova quando fornecido.
  - O resumo em uma linha expõe mantendo um `fingerprint=<hex>` determinado pela codificação `CastZkBallot`, assim como as dicas decodificadas (`owner`, `amount`, `duration_blocks`, `direction` si quatronis).
  - As respostas CLI anotadas `tx_instructions[]` com `payload_fingerprint_hex` mais descodificações de campeões para que as ferramentas downstream verifiquem o esquema sem reimplementar a decodificação Norito.
  - Fornecer dicas de bloqueio para permitir a ocorrência de eventos `LockCreated`/`LockExtended` para as cédulas ZK uma vez que o circuito expõe os valores dos memes.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - O alias `--lock-amount`/`--lock-duration-blocks` reflete os nomes dos sinalizadores ZK para a parte do script.
  - O resumo da triagem reflete `vote --mode zk`, incluindo a impressão digital da instrução codificada e os campos de votação disponíveis (`owner`, `amount`, `duration_blocks`, `direction`), para uma confirmação rápida antes da assinatura do esqueleto.

Listagem de instâncias
- GET `/v1/gov/instances/{ns}` - lista as instâncias de contrato ativas para um namespace.
  - Parâmetros de consulta:
    - `contains`: filtro por sub-cadeia de `contract_id` (diferencia maiúsculas de minúsculas)
    - `hash_prefix`: filtro par prefixo hexadecimal de `code_hash_hex` (minúsculas)
    - `offset` (padrão 0), `limit` (padrão 100, máximo 10_000)
    - `order`: um dos `cid_asc` (padrão), `cid_desc`, `hash_asc`, `hash_desc`
  - Resposta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - SDK auxiliar: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Balayage d'unlocks (Operador/Auditoria)
- OBTER `/v1/gov/unlocks/stats`
  - Resposta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` reflete a altivez do bloco mais recente ou os bloqueios expiram e persistem. `expired_locks_now` calcula e verifica os registros de bloqueio com `expiry_height <= height_current`.
-POSTO `/v1/gov/ballots/zk-v1`
  - Requete (estilo DTO v1):
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
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }- POST `/v1/gov/ballots/zk-v1/ballot-proof` (recurso: `zk-ballot`)
  - Aceite um JSON `BallotProof` direto e envie um modelo `CastZkBallot`.
  -Requete:
    {
      "autoridade": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "votação": {
        "back-end": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // base64 do conteúdo ZK1 ou H2*
        "root_hint": null, // string hexadecimal opcional de 32 bytes (raiz de elegibilidade)
        "owner": null, // Opção AccountId se o proprietário do commit do circuito
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
    - O servidor mapa `root_hint`/`owner`/`nullifier` opções de votação vers `public_inputs_json` para `CastZkBallot`.
    - Os bytes do envelope são recodificados em base64 para a carga útil da instrução.
    - A resposta `reason` passa para `submitted transaction` quando Torii é votada.
    - Este endpoint está disponível apenas se o recurso `zk-ballot` estiver ativo.

Parcurso de verificação CastZkBallot
- `CastZkBallot` decodifica a versão base64 anterior e rejeita as cargas úteis de forma incorreta (`BallotRejected` com `invalid or empty proof`).
- O host retorna a chave de verificação da cédula após o referendo (`vk_ballot`) ou os padrões de governo e exige que o registro exista, como `Active` e transporte de bytes inline.
- Os bytes do código de verificação são re-hashes com `hash_vk`; toda incompatibilidade de compromisso impede a execução antes da verificação para proteger as entradas do registro corrompido (`BallotRejected` com `verifying key commitment mismatch`).
- Os bytes de teste são despachados no backend registrado via `zk::verify_backend`; as transcrições são inválidas remontadas em `BallotRejected` com `invalid proof` e a instrução ecoa determinística.
- A prova deve expor um compromisso eleitoral e uma raiz de elegibilidade como contribuições públicas; a raiz deve corresponder ao `eligible_root` da eleição e o anulador derivado deve corresponder a qualquer dica fornecida.
- Les preuves reussies emettent `BallotAccepted`; nulificadores duplicados, raízes de períodos de elegibilidade ou regressões de bloqueio continuam a produzir as razões de rejeição existentes descritas mais acima neste documento.

## Mau canal de validação e consenso conjunto

### Fluxo de trabalho de corte e prisão

O consenso emet `Evidence` codifica em Norito para validar o protocolo violado. Cada carga útil chega em `EvidenceStore` na memória e, se não editada, é materializada no mapa `consensus_evidence` encontrado no WSV. Os registros mais antigos que `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocos `7200` padrão) foram rejeitados para manter o arquivo carregado, mas a rejeição é registrada para os operadores. As evidências no horizonte também respeitam `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o atraso reduzido `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`); a governança pode cancelar penalidades com `CancelConsensusEvidencePenalty` antes que a redução seja aplicada.

Les ofensas reconnuas se mappent un-a-un sur `EvidenceKind`; Os discriminantes são estáveis ​​e impõem o modelo de dados par:

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

- **DoublePrepare/DoubleCommit** - valida o sinal de hashes em conflito para a tupla meme `(phase,height,view,epoch)`.
- **InvalidQc** - um agregador de fofocas e um certificado de commit não são reproduzidos em verificações determinadas (ex., bitmap de signataires vide).
- **InvalidProposal** - um líder propõe um bloco que ecoa a estrutura de validação (ex., viola a regra da cadeia bloqueada).
- **Censura** — recibos de envio assinados mostram uma transação que nunca foi proposta/comprometida.

Os operadores e a utilização podem inspecionar e retransmitir as cargas úteis por meio de:

- Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count` e `... submit --evidence-hex <payload>`.

O governo deve trair os bytes de evidência como preuve canonique:1. **Colecione a carga útil** antes que ela expire. Arquive os bytes Norito brutos com altura/visualização de metadados.
2. **Prepare a penalidade** e embarque a carga útil em um referendo ou em uma instrução sudo (ex., `Unregister::peer`). A execução revalida a carga útil; evidências mal formadas ou obsoletas são rejeitadas pela determinação.
3. **Planifier la topologia de suivi** para que o validador fautif não possa retornar imediatamente. Os fluxos típicos incluem `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com a lista do dia.
4. **Auditar os resultados** via `/v1/sumeragi/evidence` e `/v1/sumeragi/status` para confirmar que o contador comprovou antecipadamente e que o governo aplicou o retrait.

### Sequenciamento do consenso conjunto

A junta de consenso garante que o conjunto de validadores finalize o bloco de fronteira antes que o novo conjunto inicie um proponente. O tempo de execução impõe a regra por meio dos parâmetros do aplicativo:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` devem ser confirmados no **bloco meme**. `mode_activation_height` doit etre strictement superior à la hauteur du bloc qui a porte la mise a jour, donnant au menos un bloc de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) é a configuração que executa as transferências com atraso zero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`) atrasa a redução do consenso para que a governança possa cancelar as penalidades antes que elas sejam aplicadas.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- O tempo de execução e a CLI expõem os parâmetros preparados por meio de `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, para que os operadores confirmem as taxas de ativação e as listas de validadores.
- L'automatisation de gouvernance doit toujours:
  1. Finalizar a decisão de retrait (ou reintegração) apoiada por provas.
  2. Insira uma reconfiguração de suivi com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Vigilante `/v1/sumeragi/status` apenas aquele `effective_consensus_mode` basculante à alta presença.

Todo script que faz a validação dos validadores ou aplica uma barra **ne doit pass** tente uma ativação com atraso zero ou omita os parâmetros de entrega; essas transações são rejeitadas e abandonadas no modo precedente.

## Superfícies de telemetria

- As métricas Prometheus exportam a atividade de governo:
  - `governance_proposals_status{status}` (medidor) adequado aos compradores de propostas por status.
  - `governance_protected_namespace_total{outcome}` (contador) aumenta quando a admissão de namespaces protegidos é aceita ou rejeitada na implantação.
  - `governance_manifest_activations_total{event}` (contador) registra as inserções de manifesto (`event="manifest_inserted"`) e as ligações de namespace (`event="instance_bound"`).
- `/status` inclui um objeto `governance` que reflete os compradores de propostas, relata todos os namespaces protegidos e lista as ativações recentes de manifesto (namespace, ID do contrato, hash de código/ABI, altura do bloco, carimbo de data/hora de ativação). Os operadores podem procurar este campo para confirmar que as promulgações foram impostas aos manifestos e que as portas dos namespaces protegidos são impostas.
- Um modelo Grafana (`docs/source/grafana_governance_constraints.json`) e o runbook de telemetria em `telemetry.md` montam comentários para enviar alertas para propostas bloqueadas, ativações de manifesto deficientes ou rejeitos desatendidos de namespaces protegidos durante atualizações em tempo de execução.