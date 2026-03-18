---
lang: pt
direction: ltr
source: docs/portal/docs/governance/api.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Situação: черновик/набросок для сопровождения задач по реализации governança. Os formulários podem ser exibidos na sua conta. Determinação e política RBAC являются normas de organização; Torii pode ser fornecido/transformado por `authority` e `private_key`, em clientes Verifique e instale em `/transaction`.

Óбзор
- Todos os endpoints geram JSON. Para um carro, você precisa de uma transação, abra o conector `tx_instructions` - um número maior ou menor instruções de instruções:
  - `wire_id`: реестровый идентификатор типа инструкции
  - `payload_hex`: carga útil Norito (hex)
- Если `authority` e `private_key` предоставлены (ou `private_key` em cédulas DTO), Torii подписывает и отправляет transação e você precisará de `tx_instructions`.
- Seus clientes usam SignedTransaction com sua autoridade e chain_id, que foram enviados e POST em `/transaction`.
- SDK de abertura:
- Python (`iroha_python`): `ToriiClient.get_governance_proposal_typed` corresponde a `GovernanceProposalResult` (definido por status/tipo), `ToriiClient.get_governance_referendum_typed` corresponde a `GovernanceReferendumResult`, `ToriiClient.get_governance_tally_typed` substitui `GovernanceTally`, `ToriiClient.get_governance_locks_typed` altera `GovernanceLocksResult`, `ToriiClient.get_governance_unlock_stats_typed` altera `GovernanceUnlockStats`, e `ToriiClient.list_governance_instances_typed` é compatível com `GovernanceInstancesPage`, verifique o tipo de fornecimento para sua implementação de governança com exemplos publicado no README.
- Cliente Python legível (`iroha_torii_client`): `ToriiClient.finalize_referendum` e `ToriiClient.enact_proposal` fornecem o tipo de pacotes `GovernanceInstructionDraft` (оборачивают Torii esqueleto `tx_instructions`), избегая ручного JSON-парсинга при сборке Finalize/Enact fluxos.
- JavaScript (`@iroha/iroha-js`): `ToriiClient` fornece tipos de ajudantes para propostas, referendos, contagens, bloqueios, estatísticas de desbloqueio, etc. `listGovernanceInstances(namespace, options)` além de endpoints do conselho (`getGovernanceCouncilCurrent`, `governanceDeriveCouncilVrf`, `governancePersistCouncil`, `getGovernanceCouncilAudit`), esses clientes Node.js podem ser configurados para `/v1/gov/instances/{ns}` e Fluxos de trabalho apoiados por VRF são criados em instâncias de contrato de espectro confiáveis.

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
  - Validação: ноды канонизируют `abi_hash` para заданного `abi_version` e отвергают несовпадения. Para `abi_version = "v1"` é definido como - `hex::encode(ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1))`.

API de contratos (implantação)
-POSTO `/v1/contracts/deploy`
  - Solicitação: { "autoridade": "i105...", "private_key": "...", "code_b64": "..." }
  - Comportamento: selecione `code_hash` para os programas IVM e `abi_hash` para definir `abi_version`, затем отправляет `RegisterSmartContractCode` (manifest) e `RegisterSmartContractBytes` (полные `.to` байты) от имени `authority`.
  - Resposta: { "ok": true, "code_hash_hex": "...", "abi_hash_hex": "..." }
  - Relacionado:
    - GET `/v1/contracts/code/{code_hash}` -> возвращает сохраненный manifesto
    - GET `/v1/contracts/code-bytes/{code_hash}` -> obter `{ code_b64 }`
-POSTO `/v1/contracts/instance`
  - Solicitação: { "autoridade": "i105...", "private_key": "...", "namespace": "apps", "contract_id": "calc.v1", "code_b64": "..." }
  - Comportamento: деплоит предоставленный bytecode e сразу активирует маппинг `(namespace, contract_id)` через `ActivateContractInstance`.
  - Resposta: { "ok": true, "namespace": "apps", "contract_id": "calc.v1", "code_hash_hex": "...", "abi_hash_hex": "..." }Serviço de alias
-POSTO `/v1/aliases/voprf/evaluate`
  - Solicitação: { "blinded_element_hex": "..." }
  - Resposta: { "evaluated_element_hex": "...128hex", "backend": "blake2b512-mock" }
    - `backend` отражает реализацию оценщика. Código de segurança: `blake2b512-mock`.
  - Notas: детерминированный mock оценщик, применяющий Blake2b512 com separação de domínio `iroha.alias.voprf.mock.v1`. Para ferramentas de teste, o pipeline VOPRF de produção não pode ser usado em Iroha.
  - Erros: HTTP `400` при некорректном hex вводе. Torii возвращает Norito envelope `ValidationFail::QueryFailed::Conversion` com um decodificador.
-POSTO `/v1/aliases/resolve`
  - Solicitação: { "alias": "GB82 WEST 1234 5698 7654 32" }
  - Resposta: { "alias": "GB82WEST12345698765432", "account_id": "i105...", "index": 0, "source": "iso_bridge" }
  - Notas: teste de tempo de execução da ponte ISO (`[iso_bridge.account_aliases]` em `iroha_config`). Torii alias normalizado, удаляя пробелы и приводя к верхнему регистру. Selecione 404 para o alias de отсутствии e 503, que permite o tempo de execução da ponte ISO.
-POSTO `/v1/aliases/resolve_index`
  - Solicitação: { "índice": 0 }
  - Resposta: { "index": 0, "alias": "GB82WEST12345698765432", "account_id": "i105...", "source": "iso_bridge" }
  - Notas: индексы alias назначаются детерминированно по порядку конфигурации (baseado em 0). Os clientes podem usar o recurso off-line para monitorar a trilha de auditoria através do alias de atestação.

Limite de tamanho do código
- Parâmetro personalizado: `max_contract_code_bytes` (JSON u64)
  - Управляет максимальным допустимым размером (no байтах) on-chain хранения кода контрактов.
  - Padrão: 16 MiB. Ноды отклоняют `RegisterSmartContractBytes`, когда размер `.to` изображения превышает лимит, с ошибкой violação invariável.
  - O operador pode usar a carga útil `SetParameter(Custom)` com `id = "max_contract_code_bytes"` e carga útil.

-POSTO `/v1/gov/ballots/zk`
  - Solicitação: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "election_id": "e1", "proof_b64": "...", "public": {...} }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas:
    - Когда публичные входы схемы включают `owner`, `amount` e `duration_blocks`, e prova de verificação com настроенным VK, нода создает или продлевает bloqueio de governança para `election_id` com этим `owner`. Abrir tela (`unknown`); обновляются только quantidade/validade. A tabela de valores padrão: quantidade e expiração é usada (não é definida como max(amount, prev.amount) e max(expiry, prev.expiry)).
    - ZK re-votos, пытающиеся уменьшить valor ou expiração, отклоняются сервером с диагностикой `BallotRejected`.
    - O contrato de transferência é feito por meio de `ZK_VOTE_VERIFY_BALLOT` para a postagem `SubmitBallot`; хосты impor trava одноразовый.

-POSTO `/v1/gov/ballots/plain`
  - Solicitação: { "autoridade": "i105...", "private_key": "...?", "chain_id": "...", "referendum_id": "r1", "owner": "i105...", "amount": "1000", "duration_blocks": 6000, "direction": "Sim|Não|Abstenção" }
  - Resposta: { "ok": verdadeiro, "aceito": verdadeiro, "tx_instructions": [{...}] }
  - Notas: re-votos только на расширение - новый cédula не может уменьшить quantidade или expiração существующего bloqueio. `owner` é fornecido com transações de autoridade. Минимальная длительность - `conviction_step_blocks`.

-POSTO `/v1/gov/finalize`
  - Solicitação: { "referendum_id": "r1", "proposal_id": "...64hex", "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...FinalizeReferendum", "payload_hex": "..." }] }
  - Efeito na cadeia (andaime atual): promulgar proposta de implantação утвержденного вставляет минимальный `ContractManifest`, привязанный к `code_hash`, с ожидаемым `abi_hash` e proposta proposta como promulgada. Este manifesto foi criado para `code_hash` com o mesmo `abi_hash`, promulgação отклоняется.
  - Notas:
    - Para as eleições ZK, o contrato foi transferido de `ZK_VOTE_VERIFY_TALLY` para `FinalizeElection`; хосты impor trava одноразовый. `FinalizeReferendum` отклоняет ZK-референдумы, пока tally выборов не финализирован.
    - Автозакрытие на `h_end` эмитит Aprovado/Rejeitado только для Plain-референдумов; ZK-референдумы остаются Fechado, пока не будет отправлен финализированный registro e выполнен `FinalizeReferendum`.
    - A participação de Проверки используют только aprovar+rejeitar; abster-se de não participar.-POSTO `/v1/gov/enact`
  - Solicitação: { "proposal_id": "...64hex", "preimage_hash": "...64hex?", "window": { "lower": 0, "upper": 0 }?, "authority": "i105…?", "private_key": "...?" }
  - Resposta: { "ok": true, "tx_instructions": [{ "wire_id": "...EnactReferendum", "payload_hex": "..." }] }
  - Notas: Torii отправляет подписанную транзакцию при наличии `authority`/`private_key`; Você precisa de um esqueleto para um cliente e um cliente. Preimage опционален и сейчас носит информационный характер.

- OBTER `/v1/gov/proposals/{id}`
  - Caminho `{id}`: ID da proposta hexadecimal (64 caracteres)
  - Resposta: { "encontrado": bool, "proposta": { ... }? }

- OBTER `/v1/gov/locks/{rid}`
  - Caminho `{rid}`: string de identificação do referendo
  - Resposta: { "encontrado": bool, "referendum_id": "rid", "locks": { ... }? }

- OBTER `/v1/gov/council/current`
  - Resposta: { "época": N, "membros": [{ "account_id": "..." }, ...] }
  - Notas: возвращает conselho persistente при наличии; иначе деривирует детерминированный fallback, используя настроенный ativos de participação e limites (definir VRF específico para isso, por exemplo, provas de VRF не будут сохранены on-chain).

- POST `/v1/gov/council/derive-vrf` (recurso: gov_vrf)
  - Solicitação: { "committee_size": 21, "época": 123? , "candidatos": [{ "account_id": "...", "variant": "Normal|Small", "pk_b64": "...", "proof_b64": "..." }, ...] }
  - Comportamento: provar prova VRF каждого кандидата на каноническом entrada, полученном из `chain_id`, `epoch` и beacon последнего bloco hash; сортирует по bytes de saída desc с desempates; возвращает top `committee_size` участников. Não há necessidade.
  - Resposta: { "época": N, "membros": [{ "account_id": "..." } ...], "total_candidates": M, "verificado": K }
  - Notas: Normal = pk em G1, prova em G2 (96 bytes). Pequeno = pk em G2, prova em G1 (48 bytes). Entradas доменно разделены и включают `chain_id`.

### Padrões de governança (iroha_config `gov.*`)

Conselho substituto, используемый Torii при отсутствии lista persistente, параметризуется через `iroha_config`:

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

Эквивалентные переменные окружения:

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

`parliament_committee_size` ограничивает число fallback членов при отсутствии conselho, `parliament_term_blocks` задает длину эпохи для derivação semente (`epoch = floor(height / term_blocks)`), `parliament_min_stake` representa uma participação mínima (em uma edição mínima) em um ativo de elegibilidade, e `parliament_eligibility_asset_id` é considerado, como A verificação de ativos é garantida por meio de um candidato.

Governança Verificação VK sem bypass: проверка Ballots всегда требует `Active` verificando chave com bytes embutidos, e окружения не должны полагаться на test alterna, чтобы пропускать проверку.

RBAC
- On-chain выполнение требует разрешений:
  - Propostas: `CanProposeContractDeployment{ contract_id }`
  - Cédulas: `CanSubmitGovernanceBallot{ referendum_id }`
  - Promulgação: `CanEnactGovernance`
  - Gestão do conselho (futuro): `CanManageParliament`

Namespaces protegidos
- O parâmetro personalizado `gov_protected_namespaces` (matriz JSON de strings) fornece controle de admissão para implantações em namespaces configurados.
- Os clientes fornecem chaves de metadados para implantações, armazenadas em namespaces protegidos:
  - `gov_namespace`: namespace principal (por exemplo, "apps")
  - `gov_contract_id`: ID do contrato de lógica no namespace
- `gov_manifest_approvers`: IDs de contas de matriz JSON opcionais validados. Когда lane manifest задает quorum > 1, admissão требует autoridade транзакции плюс перечисленные аккаунты для удовлетворения quorum манифеста.
- Телеметрия предоставляет contadores de admissão через `governance_manifest_admission_total{result}`, чтобы операторы различали успешные admite от `missing_manifest`, `non_validator_authority`, `quorum_rejected`, `protected_namespace_rejected` e `runtime_hook_rejected`.
- Телеметрия показывает caminho de aplicação через `governance_manifest_quorum_total{outcome}` (значения `satisfied` / `rejected`), позволяя операторам аудировать aprovações desnecessárias.
- As pistas impõem lista de permissões de namespace, опубликованный em manifestos. Любая транзакция, устанавливающая `gov_namespace`, должна предоставить `gov_contract_id`, um namespace должен присутствовать no conjunto `protected_namespaces` манифеста. Os envios `RegisterSmartContractCode` não incluem esses metadados, que foram ativados.
- Admissão требует, чтобы существовал Proposta de governança promulgada para tupla `(namespace, contract_id, code_hash, abi_hash)`; иначе валидация завершается ошибкой NotPermitted.Ganchos de atualização de tempo de execução
- Os manifestos de pista podem ser usados `hooks.runtime_upgrade` para a instrução de atualização de tempo de execução do portão (`ProposeRuntimeUpgrade`, `ActivateRuntimeUpgrade`, `CancelRuntimeUpgrade`).
- Gancho:
  - `allow` (bool, padrão `true`): como `false`, suas instruções de atualização de tempo de execução foram ativadas.
  - `require_metadata` (bool, padrão `false`): требует metadados de entrada, указанную `metadata_key`.
  - `metadata_key` (string): seus metadados, gancho imposto. Padrão `gov_upgrade_id`, contém metadados ou lista de permissões.
  - `allowed_ids` (matriz de strings): lista de permissões opcional para metadados (por trim). Отклоняет, когда предоставленное значение не входит в список.
- Когда hook присутствует, admissão очереди impor metadados политику до попадания транзакции в очередь. Ao exibir metadados, coloque uma senha ou uma lista de permissões para determinar NotPermitted.
- Os resultados obtidos pela telemetria são `governance_manifest_hook_total{hook="runtime_upgrade", outcome="allowed|rejected"}`.
- Транзакции, удовлетворяющие hook, должны включать metadata `gov_upgrade_id=<value>` (или ключ, определенный manifest) вместе с любыми aprovações do validador, quorum manifesto требуемыми.

Ponto final de conveniência
- POST `/v1/gov/protected-namespaces` - применяет `gov_protected_namespaces` напрямую на ноде.
  - Solicitação: { "namespaces": ["apps", "system"] }
  - Resposta: { "ok": verdadeiro, "aplicado": 1 }
  - Notas: предназначен для admin/testing; Obtenha o token de API para configuração. Na produção, a transação é solicitada com `SetParameter(Custom)`.

Ajudantes CLI
-`iroha --output-format text app gov deploy audit --namespace apps [--contains calc --hash-prefix deadbeef]`
  - Use instâncias de contrato para namespace e prove, aqui:
    - Torii хранит bytecode para o caso `code_hash`, e também Blake2b-32 digest соответствует `code_hash`.
    - O manifesto em `/v1/contracts/code/{code_hash}` é compatível com `code_hash` e `abi_hash`.
    - Существует proposta de governança promulgada para `(namespace, contract_id, code_hash, abi_hash)`, деривированный тем же hashing de id de proposta, что использует нода.
  - Você pode criar JSON com `results[]` para o contrato (problemas, manifesto/código/resumos de propostas) e resumo detalhado, mas não há necessidade de usá-lo (`--no-summary`).
  - É possível auditar namespaces protegidos ou testar fluxos de trabalho de implantação controlados por governança.
-`iroha app gov deploy-meta --namespace apps --contract-id calc.v1 [--approver i105... --approver i105...]`
  - Verifique os metadados do esqueleto JSON para implantação em namespaces protegidos, usando `gov_manifest_approvers` para quorum quorum organizado.
- `iroha app gov vote --mode zk --referendum-id <id> --proof-b64 <b64> [--owner i105... --nullifier <32-byte-hex> --lock-amount <u128> --lock-duration-blocks <u64> --direction <Aye|Nay|Abstain>]` — dicas de bloqueio обязательны при `min_bond_amount > 0`, e mais dicas de bloqueio должен включать `owner`, `amount` e `duration_blocks`.
  - Valida IDs de contas canônicas, canoniza dicas de nulificador de 32 bytes e mescla as dicas em `public_inputs_json` (com `--public <path>` para substituições adicionais).
  - O anulador é derivado do compromisso de prova (entrada pública) mais `domain_tag`, `chain_id` e `election_id`; `--nullifier` é validado em relação à prova quando fornecido.
  - Resumo do resumo da configuração do código `fingerprint=<hex>` e do código `CastZkBallot` no momento dicas de decodificação (`owner`, `amount`, `duration_blocks`, `direction` por conta própria).
  - CLI abre a anotação `tx_instructions[]` do `payload_fingerprint_hex` e do conjunto de decodificadores, usando ferramentas downstream para fornecer esqueleto sem problemas повторной реализации decodificação Norito.
  - Предоставление dicas de bloqueio позволяет ноде эмитить `LockCreated`/`LockExtended` para cédulas ZK, когда схема раскрывает те же значения.
-`iroha app gov vote --mode plain --referendum-id <id> --owner i105... --amount <u128> --duration-blocks <u64> --direction <Aye|Nay|Abstain>`
  - Алиасы `--lock-amount`/`--lock-duration-blocks` отражают имена ZK флагов para paridade no скриптах.
  - Resumo вывод отражает `vote --mode zk`, включая impressão digital закодированной инструкции и читаемые cédula поля (`owner`, `amount`, `duration_blocks`, `direction`), este é o esqueleto que você pode usar.

Listagem de Instâncias
- GET `/v1/gov/instances/{ns}` - especifica as instâncias do contrato ativo para o namespace.
  - Parâmetros de consulta:
    - `contains`: filtro para substring `contract_id` (diferencia maiúsculas de minúsculas)
    - `hash_prefix`: filtro com prefixo hexadecimal `code_hash_hex` (minúsculas)
    - `offset` (padrão 0), `limit` (padrão 100, máximo 10_000)
    - `order`: `cid_asc` (padrão), `cid_desc`, `hash_asc`, `hash_desc`
  - Resposta: { "namespace": "ns", "instances": [{ "contract_id": "...", "code_hash_hex": "..." }, ...], "total": N, "offset": n, "limit": m }
  - Auxiliar SDK: `ToriiClient.listGovernanceInstances("apps", { contains: "calc", limit: 5 })` (JavaScript) ou `ToriiClient.list_governance_instances_typed("apps", ...)` (Python).Desbloquear varredura (operador/auditoria)
- OBTER `/v1/gov/unlocks/stats`
  - Resposta: { "height_current": H, "expired_locks_now": n, "referenda_with_expired": m, "last_sweep_height": S }
  - Notas: `last_sweep_height` отражает последний altura do bloco, quando os bloqueios expirados são varridos e persistem. `expired_locks_now` pode ser usado para verificar registros de bloqueio em `expiry_height <= height_current`.
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
  - Crie `BallotProof` JSON e crie o esqueleto `CastZkBallot`.
  - Solicitação:
    {
      "autoridade": "i105...",
      "chain_id": "00000000-0000-0000-0000-000000000000",
      "chave_privada": "...?",
      "election_id": "ref-1",
      "votação": {
        "back-end": "halo2/ipa",
        "envelope_bytes": "AAECAwQ=", // conteúdo base64 ZK1 ou H2*
        "root_hint": null, // string hexadecimal opcional de 32 bytes (raiz de elegibilidade)
        "owner": null, // opcional AccountId когда circuito фиксирует proprietário
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
    - Mapa de servidor opcional `root_hint`/`owner`/`nullifier` de votação em `public_inputs_json` para `CastZkBallot`.
    - Bytes de envelope são codificados em base64 para instruções de carga útil.
    - `reason` corresponde a `submitted transaction`, e Torii отправляет cédula.
    - Este endpoint é fornecido com o recurso `zk-ballot`.

Caminho de verificação CastZkBallot
- `CastZkBallot` decodifica a prova base64 e libera cargas úteis (`BallotRejected` com `invalid or empty proof`).
- Хост разрешает chave de verificação de votação do referendo (`vk_ballot`) ou padrões de governança e требует, чтобы запись существовала, была `Active` и содержала bytes embutidos.
- Сохраненные bytes de chave de verificação повторно хешируются с `hash_vk`; любое несовпадение comprometimento останавливает выполнение до проверки для защиты от подмененных entradas de registro (`BallotRejected` с `verifying key commitment mismatch`).
- Bytes de prova отправляются no backend зарегистрированный через `zk::verify_backend`; transcrições inválidas são fornecidas para `BallotRejected` com `invalid proof` e instruções para determinar o padrão.
- A prova deve expor um compromisso eleitoral e uma raiz de elegibilidade como contribuições públicas; a raiz deve corresponder ao `eligible_root` da eleição e o anulador derivado deve corresponder a qualquer dica fornecida.
- Provas Успешные эмитят `BallotAccepted`; повторные nulificadores, устаревшие raízes de elegibilidade ou регрессии bloqueio продолжают давать существующие причины отказа, описанные ранее.

## Não permita que você valide e concorde corretamente

### Corte e prisão

Консенсус эмитит Norito-codificado `Evidence` при нарушениях протокола валидатором. A carga útil é armazenada na memória `EvidenceStore` e, exceto no novo, é materializada em `consensus_evidence` apoiado por WSV. Записи старше `sumeragi.npos.reconfig.evidence_horizon_blocks` (blocos `7200` padrão) отклоняются, чтобы архив оставался ограниченным, но отказ registro para operadores. As evidências no horizonte também respeitam `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) e o atraso reduzido `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`); a governança pode cancelar penalidades com `CancelConsensusEvidencePenalty` antes que a redução seja aplicada.

Распознанные нарушения отображаются один-к-одному на `EvidenceKind`; Definições de dados estáveis e de segurança no modelo de dados:

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

- **DoublePrepare/DoubleCommit** - valida o arquivo de conexão para o `(phase,height,view,epoch)`.
- **InvalidQc** - агрегатор разослал commit Certificate, чья форма не проходит детерминированные проверки (por exemplo, пустой signer bitmap).
- **InvalidProposal** - лидер предложил блок, который проваливает структурную валидацию (por exemplo, нарушает regra de cadeia bloqueada).
- **Censura** — recibos de envio assinados mostram uma transação que nunca foi proposta/comprometida.Operadores e instrumentos podem produzir e distribuir cargas úteis corretamente:

- Torii: `GET /v1/sumeragi/evidence` e `GET /v1/sumeragi/evidence/count`.
-CLI: `iroha ops sumeragi evidence list`, `... count`, `... submit --evidence-hex <payload>`.

Governança должна рассматривать bytes de evidência как каноническое доказательство:

1. **Carga útil útil** para a operação. O arquivo Norito bytes contém metadados de altura/visualização.
2. **Подготовить штраф** встроив payload no referendo ou instrução sudo (por exemplo, `Unregister::peer`). A capacidade de validar a carga útil; malformado ou evidência obsoleta отклоняется детерминированно.
3. **Faça o acompanhamento de acompanhamento** чтобы нарушивший валидатор не смог сразу вернуться. Типовые потоки ставят `SetParameter(Sumeragi::NextMode)` e `SetParameter(Sumeragi::ModeActivationHeight)` com lista completa.
4. **Аудит результатов** через `/v1/sumeragi/evidence` e `/v1/sumeragi/status`, чтобы убедиться, что счетчик evidência вырос и governança выполнила удаление.

### Последовательность consenso conjunto

Consenso conjunto garantido, что исходный набор валидаторов финализирует пограничный блок до того, как новый набор начнет предлагать. O tempo de execução impõe parâmetros de parâmetro:

- `SumeragiParameter::NextMode` e `SumeragiParameter::ModeActivationHeight` foram confirmados em **том же блоке**. `mode_activation_height` é um bloco maior para você, que é mais confiável, com valor mínimo de uso блок лаг.
- `sumeragi.npos.reconfig.activation_lag_blocks` (padrão `1`) - Esta configuração de guarda, который предотвращает hand-off без лагов:
- `sumeragi.npos.reconfig.slashing_delay_blocks` (padrão `259200`) atrasa a redução do consenso para que a governança possa cancelar as penalidades antes que elas sejam aplicadas.

```rust
use iroha_config::parameters::defaults::sumeragi::npos::RECONFIG_ACTIVATION_LAG_BLOCKS;
assert_eq!(RECONFIG_ACTIVATION_LAG_BLOCKS, 1);
```

- Runtime e CLI показывают staged параметры через `/v1/sumeragi/params` e `iroha --output-format text ops sumeragi params`, чтобы операторы подтвердили alturas de ativação e escala de validação.
- A governação automática é necessária:
  1. Финализировать решение об исключении (ou восстановлении), поддержанное evidência.
  2. Após a reconfiguração de acompanhamento com `mode_activation_height = h_current + activation_lag_blocks`.
  3. Monitore `/v1/sumeragi/status` para configurar `effective_consensus_mode` em sua tela.

Любой скрипт, который ротирует валидаторов ou применяет slashing, **не должен** пытаться ativação de atraso zero ou опускать hand-off parâmetros; Essa transação é aberta e estabelecida na configuração anterior.

## Superfícies de telemetria

- Governança ativa da métrica Prometheus:
  - `governance_proposals_status{status}` (manômetro) отслеживает количество propostas по статусу.
  - `governance_protected_namespace_total{outcome}` (contador) usado para permitir/rejeitar admissão em namespaces protegidos.
  - `governance_manifest_activations_total{event}` (contador) фиксирует вставки manifesto (`event="manifest_inserted"`) e ligações de namespace (`event="instance_bound"`).
- `/status` включает объект `governance`, который зеркалит счетчики propostas, сообщает totais em namespaces protegidos e перечисляет não há ativações de manifesto (namespace, ID do contrato, hash de código/ABI, altura do bloco, carimbo de data/hora de ativação). Os operadores podem usar este post, quais são os убедиться, quais as promulgações, os manifestos e os portões de namespace protegidos.
- Modelo Grafana (`docs/source/grafana_governance_constraints.json`) e runbook de telemetria em `telemetry.md` показывают, как настроить оповещения о застрявших propostas, Propor ativações de manifesto ou não abrir namespaces protegidos em atualizações de tempo de execução.