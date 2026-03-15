---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77a3a111d5dc132351c92b586389766a2d183c8bb60aed68b18032e10421c92
source_last_modified: "2025-12-04T07:55:53.675646+00:00"
translation_last_reviewed: 2026-01-01
---

Status: rascunho/esboco para acompanhar as tarefas de implementacao de governanca. As formas podem mudar durante a implementacao. Determinismo e politica RBAC sao restricoes normativas; Torii pode assinar/submeter transacoes quando `authority` e `private_key` sao fornecidos, caso contrario os clientes constroem e submetem para `/transaction`.

Visao geral
- Todos os endpoints retornam JSON. Para fluxos que produzem transacoes, as respostas incluem `tx_instructions` - um array de uma ou mais instrucoes esqueleto:
  - `wire_id`: identificador de registro para o tipo de instrucao
  - `payload_hex`: bytes de payload Norito (hex)
- Se `authority` e `private_key` forem fornecidos (ou `private_key` em DTOs de ballots), Torii assina e submete a transacao e ainda retorna `tx_instructions`.
- Caso contrario, clientes montam uma SignedTransaction usando sua authority e chain_id, depois assinam e fazem POST para `/transaction`.
- Cobertura de SDK:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` retorna `GovernanceProposalResult` (normaliza campos status/kind), `ToriiClient.get_governance_referendum_typed` retorna `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` retorna `GovernanceTally`, `ToriiClient.get_governance_locks_typed` retorna `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` retorna `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` retorna `GovernanceInstancesPage`, impondo acesso tipado em toda a superficie de governanca com exemplos de uso no README.
- Cliente Python leve (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` retornam bundles tipados `GovernanceInstructionDraft` (encapsulando o esqueleto `tx_instructions` do Torii), evitando parse manual de JSON quando scripts compoem fluxos Finalize/Enact.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` expone helpers tipados para proposals, referenda, tallies, locks, unlock stats, e agora `listGovernanceInstances(namespace, options)` mais os endpoints do council (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`) para que clientes Node.js possam paginar `/v1/gov/instances/{ns}` e conduzir workflows com VRF junto do listing existente de instancias de contrato.

Endpoints

- POST `/v1/gov/proposals/deploy-contract`
  - Requisicao (JSON):
    {
      "namespace": "apps",
      "contract_id": "my.contract.v1",
      "code_hash": "blake2b32:..." | "...64hex",
      "abi_hash": "blake2b32:..." | "...64hex",
      "abi_version": "1",
      "window": { "lower": 12345, "upper": 12400 },
      "authority": "i105…?",
      "private_key": "...?"
    }
  - Resposta (JSON):
    { "ok": true, "proposal_id": "...64hex", "tx_instructions": [{ "wire_id": "...", "payload_hex": "..." }] }
  - Validacao: os nos canonizam `abi_hash` para o `abi_version` fornecido e rejeitam divergencias. Para `abi_version = "v1"`, o valor esperado e `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API de contratos (deploy)
- POST `/v1/contracts/deploy`
  - Requisicao: { "authority": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamento: calcula `code_hash` a partir do corpo do programa IVM e `abi_hash` a partir do header `abi_version`, depois submete `RegisterSmartContractCode` (manifesto) e `RegisterSmartContractBytes` (bytes `.to` completos) em nome de `authority`.
  - Resposta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> retorna o manifesto armazenado
    - GET `/v1/contracts/code-bytes/{code_hash}` -> retorna `{ code_b64 }`
- POST `/v1/contracts/instance`
  - Requisicao: { "authority": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamento: deploya o bytecode fornecido e ativa imediatamente o mapeamento `(namespace, contract_id)` via `ActivateContractInstance`.
  - Resposta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }

Servico de alias
- POST `/v1/aliases/voprf/evaluate`
  - Requisicao: { "blinded_element_hex": "..." }
  - Resposta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` reflete a implementacao do avaliador. Valor atual: `blake2b512-mock`.
  - Notas: avaliador mock deterministico que aplica Blake2b512 com separacao de dominio `iroha.alias.voprf.mock.v1`. Destinado a tooling de teste ate o pipeline VOPRF de producao ser integrado ao Iroha.
  - Erros: HTTP `400` em input hex malformado. Torii retorna um envelope Norito `ValidationFail::QueryFailed::Conversion` com a mensagem de erro do decoder.
- POST `/v1/aliases/resolve`
  - Requisicao: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Resposta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: requer o runtime ISO bridge staging (`[iso_bridge.account_aliases]` em `iroha_config`). Torii normaliza aliases removendo espacos e convertendo para maiusculas antes do lookup. Retorna 404 quando o alias esta ausente e 503 quando o runtime ISO bridge esta desabilitado.
- POST `/v1/aliases/resolve_index`
  - Requisicao: { "index": 0 }
  - Resposta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: indices de alias sao atribuidos de forma deterministica pela ordem de configuracao (0-based). Clientes podem cachear respostas offline para construir trilhas de auditoria de eventos de atestacao de alias.

Limite de tamanho de codigo
- Parametro custom: `max_contract_code_bytes` (JSON u64)
  - Controla o tamanho maximo permitido (em bytes) para armazenamento de codigo de contrato on-chain.
  - Default: 16 MiB. Os nos rejeitam `RegisterSmartContractBytes` quando o tamanho da imagem `.to` excede o limite com um erro de violacao de invariante.
  - Operadores podem ajustar via `SetParameter(Custom)` com `id = "max_contract_code_bytes"` e um payload numerico.

- POST `/v1/gov/ballots/zk`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Resposta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas:
    - Quando os inputs publicos do circuito incluem `owner`, `amount` e `duration_blocks`, e a prova verifica contra a VK configurada, o no cria ou estende um lock de governanca para `election_id` com esse `owner`. A direcao permanece oculta (`unknown`); apenas amount/expiry sao atualizados. Re-votes sao monotonic: amount e expiry apenas aumentam (o no aplica max(amount, prev.amount) e max(expiry, prev.expiry)).
    - Re-votes ZK que tentem reduzir amount ou expiry sao rejeitados no servidor com diagnosticos `BallotRejected`.
    - A execucao do contrato deve chamar `ZK_VOTE_VERIFY_BALLOT` antes de enfileirar `SubmitBallot`; hosts impoem um latch de uma unica vez.

- POST `/v1/gov/ballots/plain`
  - Requisicao: { "authority": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Aye|Nay|Abstain" }
  - Resposta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }
  - Notas: re-votes sao de extensao apenas - um novo ballot nao pode reduzir amount ou expiry do lock existente. O `owner` deve igualar a authority da transacao. Duracao minima e `conviction_step_blocks`.

- POST `/v1/gov/finalize`
  - Requisicao: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito on-chain (scaffold atual): promulgar uma proposta de deploy aprovada insere um `ContractManifest` minimo com chave `code_hash` com o `abi_hash` esperado e marca a proposta como Enacted. Se um manifesto ja existir para o `code_hash` com `abi_hash` diferente, o enactment e rejeitado.
  - Notas:
    - Para eleicoes ZK, os caminhos do contrato devem chamar `ZK_VOTE_VERIFY_TALLY` antes de executar `FinalizeElection`; os hosts impoem um latch de uso unico. `FinalizeReferendum` rejeita referendos ZK ate que o tally da eleicao esteja finalizado.
    - O auto-fechamento em `h_end` emite Approved/Rejected apenas para referendos Plain; referendos ZK permanecem Closed ate que um tally finalizado seja enviado e `FinalizeReferendum` seja executado.
    - As checagens de turnout usam apenas approve+reject; abstain nao conta para o turnout.

- POST `/v1/gov/enact`
  - Requisicao: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii submete a transacao assinada quando `authority`/`private_key` sao fornecidos; caso contrario retorna um esqueleto para clientes assinarem e submeterem. A preimage e opcional e hoje informativa.

- GET `/v1/gov/proposals/{id}`
  - Path `{id}`: id de proposta hex (64 chars)
  - Resposta: { "found": bool, "proposal": { ... }? }

- GET `/v1/gov/locks/{rid}`
  - Path `{rid}`: string de id de referendum
  - Resposta: { "found": bool, "referendum_id": "rid", "locks": { ... }? }

- GET `/v1/gov/council/current`
  - Resposta: { "epoch": N, "members": [{ "account_id": "..." }, ...] }
  - Notas: retorna o council persistido quando presente; caso contrario deriva um fallback deterministico usando o asset de stake configurado e thresholds (espelha a especificacao VRF ate que provas VRF em producao sejam persistidas on-chain).

- POST `/v1/gov/council/derive-vrf` (feature: gov_vrf)
  - Requisicao: { "committee_size": 21, "epoch": 123? , "candidates": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: verifica a prova VRF de cada candidato contra o input canonico derivado de `chain_id`, `epoch` e do ultimo hash de bloco; ordena por bytes de saida desc com tiebreakers; retorna os top `committee_size` membros. Nao persiste.
  - Resposta: { "epoch": N, "members": [{ "account_id": "..." } ...], "total_candidates": M, "verified": K }
  - Notas: Normal = pk em G1, proof em G2 (96 bytes). Small = pk em G2, proof em G1 (48 bytes). Inputs sao separados por dominio e incluem `chain_id`.

### Defaults de governanca (iroha_config `gov.*`)

O council fallback usado pelo Torii quando nao existe roster persistido e parametrizado via `iroha_config`:

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

Overrides de ambiente equivalentes:

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

`parliament_committee_size` limita o numero de membros fallback retornados quando nao ha council persistido, `parliament_term_blocks` define o comprimento da epoca usado para derivacao de seed (`epoch = floor(height / term_blocks)`), `parliament_min_stake` aplica o minimo de stake (em unidades minimas) no asset de elegibilidade, e `parliament_eligibility_asset_id` seleciona qual saldo de asset e escaneado ao construir o conjunto de candidatos.

A verificacao VK de governanca nao tem bypass: a verificacao de ballot sempre requer uma chave `Active` com bytes inline, e os ambientes nao devem depender de toggles de teste para pular a verificacao.

RBAC
- Execucao on-chain exige permissoes:
  - Proposals: `CanProposeContractDeployment{ contract_id }`
  - Ballots: `CanSubmitGovernanceBallot{ referendum_id }`
  - Enactment: `CanEnactGovernance`
  - Council management (futuro): `CanManageParliament`

Namespaces protegidos
- Parametro custom `gov_protected_namespaces` (JSON array de strings) habilita admission gating para deploys em namespaces listados.
- Clientes devem incluir chaves de metadata de transacao para deploys que visam namespaces protegidos:
  - `gov_namespace`: namespace alvo (ex., "apps")
  - `gov_contract_id`: contract id logico dentro do namespace
- `gov_manifest_approvers`: JSON array opcional de account IDs de validadores. Quando um lane manifest declara quorum maior que um, admission requer a authority da transacao mais as contas listadas para satisfazer o quorum do manifesto.
- Telemetria expoe contadores de admission via `governance_manifest_admission_total{result}` para que operadores distingam admits bem-sucedidos de caminhos `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` e `runtime_hook_rejected`.
- Telemetria expoe o caminho de enforcement via `governance_manifest_quorum_total{outcome}` (valores `satisfied` / `rejected`) para que operadores auditem aprovacoes faltantes.
- Lanes aplicam a allowlist de namespaces publicada em seus manifests. Qualquer transacao que define `gov_namespace` deve fornecer `gov_contract_id`, e o namespace deve aparecer no conjunto `protected_namespaces` do manifesto. Submissoes `RegisterSmartContractCode` sem essa metadata sao rejeitadas quando a protecao esta habilitada.
- Admission impoe que exista uma proposta de governanca Enacted para o tuple `(namespace, contract_id, code_hash, abi_hash)`; caso contrario a validacao falha com um erro NotPermitted.

Hooks de runtime upgrade
- Manifests de lane podem declarar `hooks.runtime_upgrade` para gatear instrucoes de runtime upgrade (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Campos do hook:
  - `allow` (bool, default `true`): quando `false`, todas as instrucoes de runtime upgrade sao rejeitadas.
  - `require_metadata` (bool, default `false`): requer a entrada de metadata especificada por `metadata_key`.
  - `metadata_key` (string): nome da metadata aplicada pelo hook. Default `gov_upgrade_id` quando metadata e requerida ou ha allowlist.
  - `allowed_ids` (array de strings): allowlist opcional de valores de metadata (apos trim). Rejeita quando o valor fornecido nao esta listado.
- Quando o hook esta presente, admission de fila aplica a politica de metadata antes de a transacao entrar na fila. Metadata ausente, valores em branco ou fora da allowlist geram um erro NotPermitted deterministico.
- Telemetria rastreia outcomes via `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Transacoes que satisfazem o hook devem incluir metadata `gov_upgrade_id=<value>` (ou a chave definida pelo manifesto) junto com quaisquer aprovacoes de validadores exigidas pelo quorum do manifesto.

Endpoint de conveniencia
- POST `/v1/gov/protected-namespaces` - aplica `gov_protected_namespaces` diretamente no no.
  - Requisicao: { "namespaces": ["apps", "system"] }
  - Resposta: { "ok": true, "applied": 1 }
  - Notas: destinado a admin/testing; requer token de API se configurado. Para producao, prefira enviar uma transacao assinada com `SetParameter(Custom)`.

Helpers CLI
- `iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Busca instancias de contrato para o namespace e confere que:
    - Torii armazena bytecode para cada `code_hash`, e seu digest Blake2b-32 corresponde ao `code_hash`.
    - O manifesto armazenado em `/v1/contracts/code/{code_hash}` reporta `code_hash` e `abi_hash` correspondentes.
    - Existe uma proposta de governanca enacted para `(namespace, contract_id, code_hash, abi_hash)` derivada pelo mesmo hashing de proposal-id que o no usa.
  - Emite um relatorio JSON com `results[]` por contrato (issues, resumos de manifest/code/proposal) mais um resumo de uma linha a menos que suprimido (`--no-summary`).
  - Util para auditar namespaces protegidos ou verificar fluxos de deploy controlados por governanca.
- `iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Emite o esqueleto JSON de metadata usado ao submeter deployments em namespaces protegidos, incluindo `gov_manifest_approvers` opcionais para satisfazer regras de quorum do manifesto.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — lock hints são obrigatórios quando `min_bond_amount > 0`, e qualquer conjunto de hints fornecido deve incluir `owner`, `amount` e `duration_blocks`.
  - Validates canonical account ids, canonicalizes 32-byte nullifier hints, and merges the hints into `public_inputs_json` (with `--public <path>` for additional overrides).
  - The nullifier is derived from the proof commitment (public input) plus `domain_tag`, `chain_id`, and `election_id`; `--nullifier` is validated against the proof when supplied.
  - O resumo de uma linha agora exibe `fingerprint=<hex>` deterministico derivado do `CastZkBallot` codificado junto com hints decodificados (`owner`, `amount`, `duration_blocks`, `direction` quando fornecidos).
  - As respostas da CLI anotam `tx_instructions[]` com `payload_fingerprint_hex` mais campos decodificados para que ferramentas downstream verifiquem o esqueleto sem reimplementar decodificacao Norito.
  - Fornecer hints de lock permite que o no emita eventos `LockCreated`/`LockExtended` para ballots ZK assim que o circuito expuser os mesmos valores.
- `iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Os aliases `--lock-amount`/`--lock-duration-blocks` espelham os nomes de flags ZK para paridade de script.
  - A saida de resumo espelha `vote --mode zk` ao incluir o fingerprint da instrucao codificada e campos de ballot legiveis (`owner`, `amount`, `duration_blocks`, `direction`), oferecendo confirmacao rapida antes de assinar o esqueleto.

Listagem de instancias
- GET `/v1/gov/instances/{ns}` - lista instancias de contrato ativas para um namespace.
  - Query params:
    - `contains`: filtra por substring de `contract_id` (case-sensitive)
    - `hash_prefix`: filtra por prefixo hex de `code_hash_hex` (lowercase)
    - `offset` (default 0), `limit` (default 100, max 10_000)
    - `order`: um de `cid_asc` (default), `cid_desc`, `hash_asc`, `hash_desc`
  - Resposta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Helper SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).

Varredura de unlocks (Operador/Auditoria)
- GET `/v1/gov/unlocks/stats`
  - Resposta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` reflete a altura de bloco mais recente onde locks expirados foram varridos e persistidos. `expired_locks_now` e calculado ao varrer registros de lock com `expiry_height <= height_current`.
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
      "owner": "i105…?",
      "nullifier": "blake2b32:...64hex?"
    }
  - Resposta: { "ok": true, "accepted": true, "tx_instructions": [{...}] }

- POST `/v1/gov/ballots/zk-v1/ballot-proof` (feature: `zk-ballot`)
  - Aceita um JSON `BallotProof` direto e retorna um esqueleto `CastZkBallot`.
  - Requisicao:
    {
      "authority": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "private_key": "...?",
      "election_id": "ref-1",
      "ballot": {
        "backend": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=",   // base64 do container ZK1 ou H2*
        "root_hint": null,                // optional 32-byte hex string (eligibility root)
        "owner": null,                    // AccountId opcional quando o circuito compromete owner
        "nullifier": null                 // optional 32-byte hex string (nullifier hint)
      }
    }
  - Resposta:
    {
      "ok": true,
      "accepted": true,
      "reason": "build transaction skeleton",
      "tx_instructions": [
        { "wire_id": "CastZkBallot", "payload_hex": "..." }
      ]
    }
  - Notas:
    - O servidor mapeia `root_hint`/`owner`/`nullifier` opcionais do ballot para `public_inputs_json` em `CastZkBallot`.
    - Os bytes do envelope sao re-encodados como base64 para o payload da instrucao.
    - A resposta `reason` muda para `submitted transaction` quando Torii submete o ballot.
    - Este endpoint so esta disponivel quando o feature `zk-ballot` esta habilitado.

Caminho de verificacao CastZkBallot
- `CastZkBallot` decodifica a prova base64 fornecida e rejeita payloads vazios ou malformados (`BallotRejected` com `invalid or empty proof`).
- O host resolve a chave verificadora do ballot a partir do referendum (`vk_ballot`) ou defaults de governanca e exige que o registro exista, esteja `Active` e contenha bytes inline.
- Bytes de chave verificadora armazenados sao re-hasheados com `hash_vk`; qualquer mismatch de commitment aborta a execucao antes da verificacao para proteger contra entradas de registro adulteradas (`BallotRejected` com `verifying key commitment mismatch`).
- Bytes de prova sao despachados ao backend registrado via `zk::verify_backend`; transcricoes invalidas aparecem como `BallotRejected` com `invalid proof` e a instrucao falha de forma deterministica.
- The proof must expose a ballot commitment and eligibility root as public inputs; the root must match the election’s `eligible_root`, and the derived nullifier must match any provided hint.
- Provas bem-sucedidas emitem `BallotAccepted`; nullifiers duplicados, roots de elegibilidade obsoletos ou regressao de lock continuam a produzir as razoes de rejeicao existentes descritas anteriormente neste documento.

## Mau comportamento de validadores e consenso conjunto

### Fluxo de slashing e jailing

O consenso emite `Evidence` codificada em Norito quando um validador viola o protocolo. Cada payload chega ao `EvidenceStore` em memoria e, se inedito, e materializado no mapa `consensus_evidence` respaldado por WSV. Registros mais antigos que `sumeragi.npos.reconfig.evidence_horizon_blocks` (default `7200` blocos) sao rejeitados para manter o arquivo limitado, mas a rejeicao e registrada para operadores. Evidence within the horizon also respects `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) and the slashing delay `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`); governance can cancel penalties with `CancelConsensusEvidencePenalty` before slashing applies.

Ofensas reconhecidas mapeiam um-para-um para `EvidenceKind`; os discriminantes sao estaveis e impostos pelo data model:

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
- **InvalidQc** - um agregador gossiped um commit certificate cuja forma falha em checagens deterministicas (ex., bitmap de signers vazio).
- **InvalidProposal** - um leader prop os um bloco que falha validacao estrutural (ex., viola a regra de locked-chain).
- **Censorship** — signed submission receipts show a transaction that was never proposed/committed.

Operadores e tooling podem inspecionar e re-broadcast payloads via:

- Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
- CLI: `iroha ops sumeragi evidence list`, `... count`, e `... submit --evidence-hex <payload>`.

A governanca deve tratar os bytes de evidence como prova canonica:

1. **Coletar o payload** antes de expirar. Arquive os bytes Norito brutos junto com metadata de height/view.
2. **Preparar a penalidade** embedando o payload em um referendum ou instrucao sudo (ex., `Unregister::peer`). A execucao re-valida o payload; evidence malformada ou stale e rejeitada deterministamente.
3. **Agendar a topologia de acompanhamento** para que o validador infrator nao possa retornar imediatamente. Fluxos tipicos enfileiram `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com o roster atualizado.
4. **Auditar resultados** via `/v1/sumeragi/evidence` e `/v1/sumeragi/status` para garantir que o contador de evidence avancou e que a governanca aplicou a remocao.

### Sequenciamento de consenso conjunto

O consenso conjunto garante que o conjunto de validadores de saida finalize o bloco de fronteira antes que o novo conjunto comeca a propor. O runtime aplica a regra via parametros pareados:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` devem ser confirmados no **mesmo bloco**. `mode_activation_height` deve ser estritamente maior que a altura do bloco que carregou a atualizacao, fornecendo ao menos um bloco de lag.
- `sumeragi.npos.reconfig.activation_lag_blocks` (default `1`) e o guard de configuracao que impede hand-offs com lag zero:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (default `259200`) delays consensus slashing so governance can cancel penalties before they apply.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- O runtime e a CLI expoem parametros staged via `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, para que operadores confirmem alturas de ativacao e rosters de validadores.
- A automacao de governanca deve sempre:
  1. Finalizar a decisao de remocao (ou reintegro) respaldada por evidence.
  2. Enfileirar uma reconfiguracao de acompanhamento com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitorar `/v1/sumeragi/status` ate `effective_consensus_mode` trocar na altura esperada.

Qualquer script que rotacione validadores ou aplique slashing **nao deve** tentar ativacao de lag zero ou omitir os parametros de hand-off; tais transacoes sao rejeitadas e deixam a rede no modo anterior.

## Superficies de telemetria

- Metricas Prometheus exportam atividade de governanca:
  - `governance_proposals_status{status}` (gauge) rastreia contagens de proposals por status.
  - `governance_protected_namespace_total{outcome}` (counter) incrementa quando admission de namespaces protegidos permite ou rejeita um deploy.
  - `governance_manifest_activations_total{event}` (counter) registra insercoes de manifest (`event="manifest_inserted"`) e bindings de namespace (`event="instance_bound"`).
- `/status` inclui um objeto `governance` que espelha as contagens de proposals, relata totais de namespaces protegidos e lista ativacoes recentes de manifest (namespace, contract id, code/ABI hash, block height, activation timestamp). Operadores podem consultar esse campo para confirmar que enactments atualizaram manifests e que gates de namespaces protegidos estao sendo aplicados.
- Um template Grafana (`docs/source/grafana_governance_constraints.json`) e o runbook de telemetria em `telemetry.md` mostram como ligar alertas para proposals presas, ativacoes de manifest ausentes, ou rejeicoes inesperadas de namespaces protegidos durante upgrades de runtime.