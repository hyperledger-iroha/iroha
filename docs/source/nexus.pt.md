---
lang: pt
direction: ltr
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

#! Iroha 3 - Sora Nexus Ledger: Especificacao tecnica de design

Este documento propoe a arquitetura do Sora Nexus Ledger para Iroha 3, evoluindo o Iroha 2 para um ledger global unico, logicamente unificado, organizado em torno de Data Spaces (DS). Data Spaces fornecem dominios fortes de privacidade ("private data spaces") e participacao aberta ("public data spaces"). O design preserva a composabilidade no ledger global enquanto garante isolamento estrito e confidencialidade para dados de DS privados, e introduz escalabilidade de disponibilidade de dados via erasure coding em Kura (armazenamento de blocos) e WSV (World State View).

O mesmo repositorio constroi Iroha 2 (redes auto-hospedadas) e Iroha 3 (SORA Nexus). A execucao e impulsionada pela Iroha Virtual Machine (IVM) e pela toolchain Kotodama compartilhada, entao contratos e artefatos de bytecode permanecem portaveis entre deployments auto-hospedados e o ledger global Nexus.

Objetivos
- Um ledger logico global composto por muitos validadores cooperativos e Data Spaces.
- Data Spaces privados para operacao permissionada (ex., CBDCs), com dados que nunca saem do DS privado.
- Data Spaces publicos com participacao aberta, acesso permissionless estilo Ethereum.
- Contratos inteligentes componiveis entre Data Spaces, sujeitos a permissoes explicitas para acesso a ativos de DS privados.
- Isolamento de performance para que atividade publica nao degrade transacoes internas de DS privados.
- Disponibilidade de dados em escala: Kura e WSV com erasure coding para suportar dados efetivamente ilimitados mantendo privados os dados de DS privados.

Nao-Objetivos (Fase inicial)
- Definir economia de tokens ou incentivos de validadores; agendamento e staking sao plugaveis.
- Introduzir uma nova versao de ABI ou ampliar superficies de syscalls/pointer-ABI; ABI v1 e fixo e as runtime upgrades nao mudam o ABI do host.

Terminologia
- Nexus Ledger: O ledger logico global formado ao compor blocos de Data Space (DS) em um historico unico e ordenado e um commitment de estado.
- Data Space (DS): Um dominio limitado de execucao e armazenamento com seus proprios validadores, governanca, classe de privacidade, politica de DA, quotas e politica de fees. Duas classes existem: DS publico e DS privado.
- Private Data Space: Validadores permissionados e controle de acesso; dados de transacao e estado nunca deixam o DS. Apenas commitments/metadata sao ancorados globalmente.
- Public Data Space: Participacao permissionless; dados completos e estado disponiveis publicamente.
- Data Space Manifest (DS Manifest): Um manifesto Norito-encodado que declara parametros de DS (validadores/chaves QC, classe de privacidade, politica ISI, parametros de DA, retencao, quotas, politica ZK, fees). O hash do manifesto e ancorado na chain nexus. A menos que indicado, quorum certificates de DS usam ML-DSA-87 (classe Dilithium5) como esquema de assinatura post-quantum padrao.
- Space Directory: Um contrato diretorio global on-chain que acompanha manifestos DS, versoes e eventos de governanca/rotacao para resolucao e auditorias.
- DSID: Um identificador global unico para um Data Space. Usado para namespacing de todos os objetos e referencias.
- Anchor: Um commitment criptografico de um bloco/header de DS incluido na chain nexus para ligar a historia do DS ao ledger global.
- Kura: Armazenamento de blocos Iroha. Aqui e estendido com armazenamento blob com erasure coding e commitments.
- WSV: World State View de Iroha. Aqui e estendido com segmentos de estado versionados, com snapshots e erasure coding.
- IVM: Iroha Virtual Machine para execucao de contratos inteligentes (bytecode Kotodama `.to`).
 - AIR: Algebraic Intermediate Representation. Uma visao algebrica de computacao para provas estilo STARK, descrevendo execucao como traces baseadas em campo com restricoes de transicao e fronteira.

Modelo de Data Spaces
- Identidade: `DataSpaceId (DSID)` identifica um DS e namespace tudo. DS pode ser instanciado em duas granularidades:
  - Domain-DS: `ds::domain::<domain_name>` - execucao e estado limitados a um dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execucao e estado limitados a uma definicao de asset.
  Ambas as formas coexistem; transacoes podem tocar multiplos DSID de forma atomica.
- Ciclo de vida do manifesto: criacao DS, atualizacoes (rotacao de chaves, mudancas de politica) e retirada sao registradas no Space Directory. Cada artefato por slot referencia o hash do manifesto mais recente.
- Classes: DS publico (participacao aberta, DA publica) e DS privado (permissionado, DA confidencial). Politicas hibridas sao possiveis via flags do manifesto.
- Politicas por DS: permissoes ISI, parametros DA `(k,m)`, criptografia, retencao, quotas (participacao min/max de tx por bloco), politica de provas ZK/otimistas, fees.
- Governanca: membresia DS e rotacao de validadores definidas pela secao de governanca do manifesto (propostas on-chain, multisig, ou governanca externa ancorada por transacoes e attestations nexus).

Gossip consciente de dataspace
- Lotes de gossip de transacoes agora carregam uma tag de plano (publico vs restrito) derivada do catalogo de lanes; lotes restritos sao unicast para os peers em linha da topologia de commit atual (respeitando `transaction_gossip_restricted_target_cap`) enquanto lotes publicos usam `transaction_gossip_public_target_cap` (defina `null` para broadcast). A selecao de alvos reembaralha na cadencia por plano definida por `transaction_gossip_public_target_reshuffle_ms` e `transaction_gossip_restricted_target_reshuffle_ms` (padrao: `transaction_gossip_period_ms`). Quando nao ha peers em linha na topologia de commit, operadores podem escolher recusar ou encaminhar payloads restritos ao overlay publico via `transaction_gossip_restricted_public_payload` (padrao `refuse`); a telemetria expoe tentativas de fallback, contagens de forward/drop e a politica configurada junto com selecoes de alvo por dataspace.
- Dataspaces desconhecidos sao re-enfileirados quando `transaction_gossip_drop_unknown_dataspace` esta habilitado; caso contrario caem para targeting restrito para evitar vazamentos.
- A validacao no recebimento descarta entradas cujas lanes/dataspaces nao concordam com o catalogo local, cuja tag de plano nao corresponde a visibilidade do dataspace derivado, ou cuja rota anunciada nao corresponde a decisao de roteamento re-derivada localmente.

Manifests de capacidades e UAID
- Contas universais: Cada participante recebe um UAID deterministico (`UniversalAccountId` em `crates/iroha_data_model/src/nexus/manifest.rs`) que abrange todos os dataspaces. Manifests de capacidades (`AssetPermissionManifest`) vinculam um UAID a um dataspace especifico, epochs de ativacao/expiracao e uma lista ordenada de regras allow/deny `ManifestEntry` que delimitam `dataspace`, `program_id`, `method`, `asset` e roles AMX opcionais. Regras deny sempre vencem; o avaliador emite `ManifestVerdict::Denied` com motivo de auditoria ou um grant `Allowed` com a metadata da allowance correspondente.
- Snapshots de portfolio UAID agora sao expostos via `GET /v1/accounts/{uaid}/portfolio` (ver `docs/source/torii/portfolio_api.md`), apoiados pelo agregador deterministico em `iroha_core::nexus::portfolio`.
- Allowances: Cada entrada allow carrega buckets `AllowanceWindow` deterministas (`PerSlot`, `PerMinute`, `PerDay`) mais um `max_amount` opcional. Hosts e SDKs consomem o mesmo payload Norito, entao a aplicacao permanece identica entre hardware e SDKs.
- Telemetria de auditoria: O Space Directory emite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) sempre que um manifesto muda de estado. A nova superficie `SpaceDirectoryEventFilter` permite que assinantes Torii/data-event monitorem atualizacoes, revogacoes e decisoes deny-wins sem plomeria customizada.

### Operacoes de manifesto UAID

Operacoes do Space Directory sao entregues em duas formas para que operadores escolham o
CLI embutido (para rollouts com scripts) ou envios Torii diretos (para
CI/CD automatizado). Ambos os caminhos aplicam a permissao `CanPublishSpaceDirectoryManifest{dataspace}`
 dentro do executor (`crates/iroha_core/src/smartcontracts/isi/space_directory.rs`)
e registram eventos de ciclo de vida no world state (`iroha_core::state::space_directory_manifests`).

#### Fluxo de trabalho CLI (`iroha app space-directory manifest ...`)

1. **Codificar JSON do manifesto** - converter rascunhos de politica em bytes Norito e emitir um
   hash reproduzivel antes da revisao:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

   O helper aceita `--json` (manifesto JSON bruto) ou `--manifest` (payload `.to` existente)
   e espelha a logica em
   `crates/iroha_cli/src/space_directory.rs::ManifestEncodeArgs`.

2. **Publicar/substituir manifestos** - enfileirar instrucoes `PublishSpaceDirectoryManifest`
   a partir de fontes Norito ou JSON:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

   `--reason` preenche `entries[*].notes` para registros que omitiram notas do operador.

3. **Expirar** manifestos que atingiram o fim de vida programado ou **revogar**
   UAIDs sob demanda. Ambos os comandos aceitam `--uaid uaid:<hex>` ou um digest
   hex de 64 caracteres (LSB=1) e o id numerico do dataspace:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **Produzir bundles de auditoria** - `manifest audit-bundle` escreve o JSON do manifesto,
   payload `.to`, hash, perfil de dataspace e metadata legivel por maquina em um
   diretorio de saida para que revisores de governanca baixem um unico arquivo:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

   O bundle incorpora hooks `SpaceDirectoryEvent` do perfil para provar que o
   dataspace expoe os webhooks de auditoria obrigatorios; ver `docs/space-directory.md`
   para o layout de campos e requisitos de evidencia.

#### APIs Torii

Operadores e SDKs podem executar as mesmas acoes via HTTPS. Torii aplica os
mesmos checks de permissao e assina transacoes em nome da autoridade fornecida
(as chaves privadas trafegam apenas em memoria dentro do handler seguro de Torii):

- `GET /v1/space-directory/uaids/{uaid}` - resolver os bindings atuais de dataspace
  para um UAID (enderecos normalizados, ids de dataspace, bindings de programa). Adicionar
  `address_format=compressed` para saida do Sora Name Service (IH58 preferido; compressed (`snx1`) e segunda melhor opcao Sora-only).
- `GET /v1/space-directory/uaids/{uaid}/portfolio` -
  agregador baseado em Norito que espelha `ToriiClient.getUaidPortfolio` para que wallets
  renderizem holdings universais sem raspar o estado por dataspace.
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}` - buscar o JSON do manifesto
  canonico, metadata de ciclo de vida e hash do manifesto para auditorias.
- `POST /v1/space-directory/manifests` - submeter manifestos novos ou de substituicao
  a partir de JSON (`authority`, `private_key`, `manifest`, `reason` opcional). Torii
  retorna `202 Accepted` quando a transacao entra na fila.
- `POST /v1/space-directory/manifests/revoke` - enfileirar revogacoes de emergencia
  com UAID, id de dataspace, epoch efetivo e reason opcional (espelha o layout do
  CLI).

O SDK JS (`javascript/iroha_js/src/toriiClient.js`) ja envolve essas superficies de leitura
via `ToriiClient.getUaidPortfolio`, `.getUaidBindings` e
`.getUaidManifests`; futuras releases Swift/Python reutilizam os mesmos payloads REST.
Referencie `docs/source/torii/portfolio_api.md` para esquemas completos de request/response e
`docs/space-directory.md` para o playbook de operador end-to-end.

Atualizacoes recentes de SDK/AMX
- **NX-11 (verificacao de relay cross-lane):** helpers de SDK agora validam os
  envelopes de relay de lane expostos por `/v1/sumeragi/status`. O cliente Rust envia
  helpers `iroha::nexus` para construir/verificar provas de relay e rejeitar tuplas
  duplicadas `(lane_id, dataspace_id, height)`, o binding Python expoe
  `verify_lane_relay_envelope_bytes`/`lane_settlement_hash`, e o SDK JS expoe
  `verifyLaneRelayEnvelope`/`laneRelayEnvelopeSample` para que operadores possam validar
  provas de transferencia cross-lane com hashes consistentes antes de encaminha-las.
  (crates/iroha/src/nexus.rs:1, python/iroha_python/iroha_python_rs/src/lib.rs:666, crates/iroha_js_host/src/lib.rs:640, javascript/iroha_js/src/nexus.js:1)
- **NX-17 (guardrails de budget AMX):** `ivm::analysis::enforce_amx_budget` estima
  o custo de execucao por dataspace e grupo usando o relatorio de analise estatica e
  aplica os budgets de 30 ms / 140 ms capturados aqui. O helper expoe violacoes claras
  para budgets por DS e grupo e e coberto por testes unitarios,
  tornando o budget de slots AMX deterministico para schedulers Nexus e tooling
  de SDK. (crates/ivm/src/analysis.rs:142, crates/ivm/src/analysis.rs:241)

Arquitetura de alto nivel
1) Camada de composicao global (Nexus Chain)
- Mantem um ordenamento canonico unico de blocos Nexus de 1 s que finalizam transacoes atomicas que abrangem um ou mais Data Spaces (DS). Cada transacao commitada atualiza o world state global unificado (vetor de raizes por DS).
- Contem metadata minima mais provas/QCs agregadas para garantir composabilidade, finality e deteccao de fraude (DSIDs tocados, raizes de estado por DS antes/depois, commitments DA, provas de validade por DS e o quorum certificate de DS usando ML-DSA-87). Nenhum dado privado e incluido.
- Consenso: Comite BFT global, em pipeline, de tamanho 22 (3f+1 com f=7), selecionado de um pool de ate ~200k validadores potenciais por um mecanismo VRF/stake por epoch. O comite nexus sequencia transacoes e finaliza o bloco em 1 s.

2) Camada Data Space (Publica/Privada)
- Executa fragmentos por DS de transacoes globais, atualiza WSV local do DS e produz artefatos de validade por bloco (provas por DS agregadas e commitments DA) que se agregam no bloco Nexus de 1 s.
- DS privados cifram dados em repouso e em voo entre validadores autorizados; apenas commitments e provas PQ de validade saem do DS.
- DS publicos exportam corpos de dados completos (via DA) e provas PQ de validade.

3) Transacoes atomicas cross-Data-Space (AMX)
- Modelo: Cada transacao de usuario pode tocar multiplos DS (ex., domain DS e um ou mais asset DS). Ela e commitada atomicamente em um bloco Nexus de 1 s ou aborta; nao ha efeitos parciais.
- Prepare-commit em 1 s: Para cada transacao candidata, os DS tocados executam em paralelo contra o mesmo snapshot (raizes DS de inicio do slot) e produzem provas PQ por DS (FASTPQ-ISI) e commitments DA. O comite nexus commit a transacao apenas se todas as provas DS requeridas verificarem e os certificados DA chegarem (alvo <=300 ms); caso contrario a transacao e reprogramada para o proximo slot.
- Consistencia: Conjuntos de leitura-escrita sao declarados; deteccao de conflitos ocorre no commit contra as raizes de inicio do slot. Execucao otimista sem locks por DS evita travamentos globais; atomicidade e imposta pela regra de commit nexus (tudo-ou-nada entre DS).
- Privacidade: DS privados exportam apenas provas/commitments ligados a raizes DS pre/post. Nenhum dado privado bruto sai do DS.

4) Disponibilidade de dados (DA) com erasure coding
- Kura armazena corpos de bloco e snapshots WSV como blobs com erasure coding. Blobs publicos sao amplamente shardados; blobs privados sao armazenados apenas nos validadores DS privados, com chunks cifrados.
- Commitments DA sao registrados em artefatos DS e blocos Nexus, permitindo amostragem e recuperacao sem revelar conteudos privados.

Estrutura de bloco e commit
- Artefato de prova de Data Space (por slot de 1 s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - DS privados exportam artefatos sem corpos de dados; DS publicos permitem recuperacao de corpos via DA.

- Bloco Nexus (cadencia de 1 s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacoes atomicas cross-DS com DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcao: finaliza todas as transacoes atomicas cujos artefatos DS requeridos verificam; atualiza o vetor de world state global de raizes DS em um passo.

Consenso e agendamento
- Consenso da Nexus Chain: BFT global em pipeline (classe Sumeragi) com um comite de 22 nos (3f+1 com f=7) mirando blocos de 1 s e finality de 1 s. Membros do comite sao selecionados por epoch via VRF/stake entre ~200k candidatos; a rotacao mantem descentralizacao e resistencia a censura.
- Consenso de Data Space: Cada DS executa seu proprio BFT entre seus validadores para produzir artefatos por slot (provas, commitments DA, DS QC). Comites lane-relay sao dimensionados em `3f+1` usando o `fault_tolerance` do dataspace e sao amostrados deterministicamente por epoch a partir do pool de validadores do dataspace usando a semente VRF do epoch vinculada a `(dataspace_id, lane_id)`. DS privados sao permissionados; DS publicos permitem liveness aberta sujeita a politicas anti-Sybil. O comite global nexus permanece inalterado.
- Agendamento de transacoes: Usuarios enviam transacoes atomicas declarando DSIDs tocados e conjuntos de leitura-escrita. DS executam em paralelo dentro do slot; o comite nexus inclui a transacao no bloco de 1 s se todos os artefatos DS verificarem e certificados DA forem pontuais (<=300 ms).
- Isolamento de performance: Cada DS possui mempools e execucao independentes. Quotas por DS limitam quantas transacoes tocando um DS podem ser confirmadas por bloco para evitar head-of-line blocking e proteger a latencia de DS privados.

Modelo de dados e namespacing
- IDs qualificados por DS: Todas as entidades (dominios, contas, assets, roles) sao qualificadas por `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globais: Uma referencia global e uma tupla `(dsid, object_id, version_hint)` e pode ser colocada on-chain na camada nexus ou em descritores AMX para uso cross-DS.
- Serializacao Norito: Todas as mensagens cross-DS (descritores AMX, provas) usam codecs Norito. Sem serde em caminhos de producao.

Contratos inteligentes e extensoes IVM
- Contexto de execucao: Adicionar `dsid` ao contexto de execucao IVM. Contratos Kotodama sempre executam dentro de um Data Space especifico.
- Primitivas atomicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitam uma transacao atomica multi-DS no host IVM.
  - `amx_touch(dsid, key)` declara intencao de leitura/escrita para deteccao de conflitos contra raizes de snapshot do slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operacao permitida apenas se a politica permitir e o handle for valido)
- Handles de assets e fees:
  - Operacoes de assets sao autorizadas pelas politicas ISI/roles do DS; fees sao pagas no token gas do DS. Tokens de capacidade opcionais e politicas mais ricas (multi-aprovador, rate-limits, geofencing) podem ser adicionadas depois sem mudar o modelo atomico.
- Determinismo: Todas as syscalls sao puras e deterministicas dadas as entradas e os conjuntos de leitura/escrita AMX declarados. Sem efeitos ocultos de tempo ou ambiente.

Provas de validade post-quantum (ISIs generalizados)
- FASTPQ-ISI (PQ, sem trusted setup): Um argumento baseado em hash e kernelizado que generaliza o design de transfer para todas as familias ISI enquanto visa provas sub-segundo para lotes em escala 20k em hardware classe GPU.
  - Perfil operacional:
    - Nos de producao constroem o prover via `fastpq_prover::Prover::canonical`, que agora sempre inicializa o backend de producao; o mock deterministico foi removido. (crates/fastpq_prover/src/proof.rs:126)
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permitem que operadores fixem a execucao CPU/GPU deterministica enquanto o hook observador registra triplas requested/resolved/backend para auditorias de frota. (crates/iroha_config/src/parameters/user.rs:1357, crates/irohad/src/main.rs:270, crates/irohad/src/main.rs:2192, crates/iroha_telemetry/src/metrics.rs:8887)
- Aritmetizacao:
  - AIR de atualizacao KV: Trata WSV como um mapa key-value tipado comprometido via Poseidon2-SMT. Cada ISI se expande em um pequeno conjunto de linhas read-check-write sobre chaves (contas, assets, roles, dominios, metadata, supply).
  - Restricoes com gate por opcode: Uma unica tabela AIR com colunas seletoras impoe regras por ISI (conservacao, contadores monotonos, permissoes, range checks, atualizacoes de metadata limitadas).
  - Argumentos de lookup: Tabelas transparentes, hash-committed, para permissoes/roles, precisao de assets e parametros de politica evitam constraints bit-heavy.
- Commitments de estado e atualizacoes:
  - Prova SMT agregada: Todas as chaves tocadas (pre/post) sao provadas contra `old_root`/`new_root` usando um frontier comprimido com siblings deduplicados.
  - Invariantes: Invariantes globais (ex., supply total por asset) sao impostas via igualdade de multisets entre linhas de efeitos e contadores rastreados.
- Sistema de provas:
  - Commitments polinomiais estilo FRI (DEEP-FRI) com alta aridade (8/16) e blow-up 8-16; hashes Poseidon2; transcript Fiat-Shamir com SHA-2/3.
  - Recursao opcional: agregacao recursiva local ao DS para comprimir micro-batches em uma prova por slot se necessario.
- Escopo e exemplos cobertos:
  - Assets: transfer, mint, burn, registrar/deregistrar definicoes de asset, set precision (limitado), set metadata.
  - Contas/Dominios: criar/remover, set key/threshold, adicionar/remover signatories (apenas estado; verificacoes de assinatura sao atestadas por validadores DS, nao provadas dentro do AIR).
  - Roles/Permissoes (ISI): conceder/revogar roles e permissoes; aplicadas por tabelas de lookup e checks de politica monotona.
  - Contratos/AMX: marcadores AMX begin/commit, mint/revoke de capacidades se habilitado; provados como transicoes de estado e contadores de politica.
- Checks fora do AIR para preservar latencia:
  - Assinaturas e criptografia pesada (ex., assinaturas ML-DSA de usuario) sao verificadas por validadores DS e atestadas no DS QC; a prova de validade cobre apenas consistencia de estado e conformidade de politica. Isso mantem as provas PQ rapidas.
- Metas de performance (ilustrativas, CPU 32-core + uma GPU moderna):
  - 20k ISIs mistos com pequeno toque de chaves (<=8 chaves/ISI): ~0.4-0.9 s para provar, ~150-450 KB de prova, ~5-15 ms para verificar.
  - ISIs mais pesados (mais chaves/constraints ricas): micro-batch (ex., 10x2k) + recursao para manter por slot <1 s.
- Configuracao de DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (assinaturas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (padrao; alternativas devem ser declaradas explicitamente)
- Fallbacks:
  - ISIs complexos/personalizados podem usar um STARK geral (`zk.policy = "stark_fri_general"`) com prova adiada e finality de 1 s via attestation QC + slashing em provas invalidas.
  - Opcoes nao-PQ (ex., Plonk com KZG) requerem trusted setup e nao sao mais suportadas no build padrao.

Introducao a AIR (para Nexus)
- Trace de execucao: Uma matriz com largura (colunas de registros) e comprimento (passos). Cada linha e um passo logico do processamento ISI; as colunas contem valores pre/post, seletores e flags.
- Restricoes:
  - Restricoes de transicao: impoem relacoes linha-a-linha (ex., post_balance = pre_balance - amount para uma linha de debito quando `sel_transfer = 1`).
  - Restricoes de fronteira: vinculam I/O publico (old_root/new_root, contadores) as primeiras/ultimas linhas.
  - Lookups/permutacoes: garantem membership e igualdades de multiset contra tabelas comprometidas (permissoes, parametros de asset) sem circuitos de bits pesados.
- Commitment e verificacao:
  - O prover compromete traces via codificacoes baseadas em hash e constroi polinomios de baixo grau validos se as restricoes se mantiverem.
  - O verificador checa baixo grau via FRI (baseado em hash, post-quantum) com algumas aberturas Merkle; o custo e logaritmico em passos.
- Exemplo (Transfer): registros incluem pre_balance, amount, post_balance, nonce e seletores. Restricoes impoem nao-negatividade/range, conservacao e monotonicidade de nonce, enquanto uma multi-prova SMT agregada liga folhas pre/post as raizes old/new.

Estabilidade de ABI (ABI v1)
- A superficie ABI v1 e fixa; nao se adicionam novos syscalls nem tipos pointer-ABI nesta versao.
- As runtime upgrades devem manter `abi_version = 1` com `added_syscalls`/`added_pointer_types` vazios.
- Os goldens de ABI (lista de syscalls, hash ABI, IDs de pointer type) permanecem fixos e nao devem mudar.

Modelo de privacidade
- Contencao de dados privados: corpos de transacao, diffs de estado e snapshots WSV para DS privados nunca saem do subconjunto de validadores privados.
- Exposicao publica: Apenas headers, commitments DA e provas PQ de validade sao exportadas.
- Provas ZK opcionais: DS privados podem produzir provas ZK (ex., saldo suficiente, politica satisfeita) permitindo acoes cross-DS sem revelar estado interno.
- Controle de acesso: A autorizacao e aplicada por politicas ISI/roles dentro do DS. Tokens de capacidade sao opcionais e podem ser introduzidos mais tarde se necessario.

Isolamento de performance e QoS
- Consenso, mempools e armazenamento separados por DS.
- Quotas de agendamento Nexus por DS para limitar o tempo de inclusao de anchors e evitar head-of-line blocking.
- Orcamentos de recursos de contrato por DS (compute/memory/IO), aplicados pelo host IVM. Contencao de DS publico nao pode consumir orcamentos de DS privado.
- Chamadas cross-DS assincronas evitam longas esperas sincronas dentro da execucao de DS privada.

Disponibilidade de dados e design de armazenamento
1) Erasure Coding
- Usar Reed-Solomon sistematico (ex., GF(2^16)) para erasure coding em nivel blob de blocos Kura e snapshots WSV: parametros `(k, m)` com `n = k + m` shards.
- Parametros padrao (propostos, DS publicos): `k=32, m=16` (n=48), permitindo recuperacao de ate 16 perdas de shard com ~1.5x de expansao. Para DS privados: `k=16, m=8` (n=24) dentro do conjunto permissionado. Ambos sao configuraveis por DS Manifest.
- Blobs publicos: Shards distribuidos entre muitos nos DA/validadores com checks de disponibilidade baseados em amostragem. Commitments DA em headers permitem verificacao de light clients.
- Blobs privados: Shards cifrados e distribuidos apenas entre validadores DS privados (ou custodios designados). A chain global carrega apenas commitments DA (sem localizacoes de shards ou chaves).

2) Commitments e amostragem
- Para cada blob: calcular uma raiz Merkle sobre shards e inclui-la em `*_da_commitment`. Manter PQ evitando commitments de curva eliptica.
- Attesters DA: attesters regionais amostrados por VRF (ex., 64 por regiao) emitem um certificado ML-DSA-87 atestando amostragem bem-sucedida de shards. Alvo de latencia de attestation DA <=300 ms. O comite nexus valida certificados em vez de buscar shards.

3) Integracao Kura
- Blocos armazenam corpos de transacao como blobs com erasure coding e commitments Merkle.
- Headers carregam commitments de blobs; corpos sao recuperados via rede DA para DS publicos e via canais privados para DS privados.

4) Integracao WSV
- Snapshotting WSV: Periodicamente, checkpoint de estado DS em snapshots chunked e erasure-coded com commitments registrados em headers. Entre snapshots, manter logs de mudanca. Snapshots publicos sao amplamente shardados; snapshots privados permanecem dentro de validadores privados.
- Acesso com prova: Contratos podem fornecer (ou solicitar) provas de estado (Merkle/Verkle) ancoradas por commitments de snapshot. DS privados podem fornecer attestations zero-knowledge em vez de provas brutas.

5) Retencao e pruning
- Sem pruning para DS publicos: reter todos os corpos Kura e snapshots WSV via DA (escalabilidade horizontal). DS privados podem definir retencao interna, mas commitments exportados permanecem imutaveis. A camada nexus retem todos os blocos Nexus e commitments de artefatos DS.

Redes e roles de nos
- Validadores globais: participam do consenso nexus, validam blocos Nexus e artefatos DS, executam checks DA para DS publicos.
- Validadores de Data Space: executam consenso DS, executam contratos, gerenciam Kura/WSV local, gerenciam DA para seu DS.
- Nos DA (opcionais): armazenam/publicam blobs publicos, facilitam amostragem. Para DS privados, nos DA ficam co-localizados com validadores ou custodios confiaveis.

Melhorias e consideracoes de sistema
- Desacoplamento sequencing/mempool: adotar um mempool DAG (ex., estilo Narwhal) alimentando um BFT em pipeline na camada nexus para reduzir latencia e melhorar throughput sem mudar o modelo logico.
- Quotas DS e fairness: quotas por DS por bloco e limites de peso para evitar head-of-line blocking e garantir latencia previsivel para DS privados.
- Attestation DS (PQ): Quorum certificates DS padrao usam ML-DSA-87 (classe Dilithium5). Isso e post-quantum e maior que assinaturas EC, mas aceitavel em um QC por slot. DS podem optar explicitamente por ML-DSA-65/44 (menor) ou assinaturas EC se declaradas no DS Manifest; DS publicos sao fortemente encorajados a manter ML-DSA-87.
- Attesters DA: Para DS publicos, usar attesters regionais amostrados por VRF que emitem certificados DA. O comite nexus valida certificados em vez de amostragem bruta de shards; DS privados mantem attestations DA internas.
- Recursao e provas por epoch: opcionalmente agregar varios micro-batches dentro de um DS em uma prova recursiva por slot/epoch para manter tamanho de prova e tempo de verificacao estaveis sob alta carga.
- Escalonamento por lanes (se necessario): se um comite global unico se tornar gargalo, introduzir K lanes de sequenciamento paralelo com merge deterministico. Isso preserva uma ordem global unica enquanto escala horizontalmente.
- Aceleracao deterministica: fornecer kernels SIMD/CUDA com feature-gate para hashing/FFT com fallback CPU bit-exato para preservar determinismo entre hardware.
- Limiares de ativacao de lanes (proposta): habilitar 2-4 lanes se (a) p95 finality excede 1.2 s por >3 minutos consecutivos, ou (b) ocupacao por bloco excede 85% por >5 minutos, ou (c) a taxa de tx entrante exigiria >1.2x capacidade de bloco em niveis sustentados. Lanes bucketizam transacoes deterministicamente por hash de DSID e fazem merge no bloco nexus.

Fees e economia (defaults iniciais)
- Unidade de gas: token de gas por DS com compute/IO medido; fees sao pagas no asset gas nativo do DS. Conversao entre DS e uma preocupacao de aplicacao.
- Prioridade de inclusao: round-robin entre DS com quotas por DS para preservar fairness e SLOs de 1 s; dentro de um DS, fee bidding pode desempatar.
- Futuro: mercado global de fees opcional ou politicas que minimizam MEV podem ser exploradas sem mudar atomicidade ou design de provas PQ.

Fluxo cross-Data-Space (Exemplo)
1) Um usuario envia uma transacao AMX que toca DS publico P e DS privado S: mover o asset X de S para beneficiario B cuja conta esta em P.
2) Dentro do slot, P e S executam seu fragmento contra o snapshot do slot. S verifica autorizacao e disponibilidade, atualiza seu estado interno e produz uma prova PQ de validade e commitment DA (nenhum dado privado vaza). P prepara a atualizacao de estado correspondente (ex., mint/burn/locking em P conforme a politica) e sua prova.
3) O comite nexus verifica ambas as provas DS e certificados DA; se ambas verificarem dentro do slot, a transacao e commitada atomicamente no bloco Nexus de 1 s, atualizando ambas as raizes DS no vetor de world state global.
4) Se alguma prova ou certificado DA estiver ausente/invalido, a transacao aborta (sem efeitos), e o cliente pode reenviar para o proximo slot. Nenhum dado privado sai de S em nenhum passo.

- Consideracoes de seguranca
- Execucao deterministica: Syscalls IVM permanecem deterministicas; resultados cross-DS sao dirigidos por commit AMX e finality, nao por relogio ou timing de rede.
- Controle de acesso: Permissoes ISI em DS privados restringem quem pode enviar transacoes e quais operacoes sao permitidas. Tokens de capacidade codificam direitos finos para uso cross-DS.
- Confidencialidade: Cifragem end-to-end para dados DS privados, shards erasure-coded armazenados apenas entre membros autorizados, provas ZK opcionais para attestations externas.
- Resistencia a DoS: Isolamento em camadas de mempool/consenso/armazenamento impede que congestao publica afete o progresso de DS privados.

Mudancas nos componentes Iroha
- iroha_data_model: Introduzir `DataSpaceId`, identificadores qualificados por DS, descritores AMX (conjuntos de leitura/escrita), tipos de prova/commitment DA. Serializacao apenas Norito.
- ivm: Manter a superficie ABI v1 fixa (sem novos syscalls nem tipos pointer-ABI); AMX/runtime upgrades usam primitivas v1 existentes; manter goldens ABI.
- iroha_core: Implementar scheduler nexus, Space Directory, roteamento/validacao AMX, verificacao de artefatos DS e enforcement de politicas para amostragem DA e quotas.
- Space Directory e loaders de manifestos: passar metadata de endpoints FMS (e outros descritores de servicos common-good) pelo parsing de manifestos DS para que nos auto-descubram endpoints locais ao ingressar em um Data Space.
- kura: Blob store com erasure coding, commitments, APIs de recuperacao respeitando politicas privadas/publicas.
- WSV: Snapshotting, chunking, commitments; APIs de prova; integracao com deteccao e verificacao de conflitos AMX.
- irohad: Roles de node, networking para DA, membership/autenticacao de DS privados, configuracao via `iroha_config` (sem toggles de env em caminhos de producao).

Configuracao e determinismo
- Todo comportamento de runtime configurado via `iroha_config` e repassado por constructores/hosts. Sem toggles de env em producao.
- Aceleracao de hardware (SIMD/NEON/METAL/CUDA) e opcional e feature-gated; fallbacks deterministas devem produzir resultados identicos entre hardware.
- - Post-Quantum padrao: Todos os DS devem usar provas de validade PQ (STARK/FRI) e ML-DSA-87 para QCs DS por padrao. Alternativas exigem declaracao explicita no DS Manifest e aprovacao de politica.

### Controle de ciclo de vida de lanes em runtime

- **Endpoint admin:** `POST /v1/nexus/lifecycle` (Torii) aceita um corpo Norito/JSON com `additions` (objetos completos `LaneConfig`) e `retire` (ids de lane) para adicionar ou remover lanes sem reinicio. Requisicoes sao gateadas em `nexus.enabled=true` e reutilizam a mesma view de configuracao/estado Nexus que a fila.
- **Comportamento:** Em sucesso o node aplica o plano de ciclo de vida a metadata WSV/Kura, reconstroi routing/limites/manifestos da fila e responde com `{ ok: true, lane_count: <u32> }`. Planos que falham validacao (ids de retire desconhecidos, aliases/ids duplicados, Nexus desabilitado) retornam `400 Bad Request` com `lane_lifecycle_error`.
- **Seguranca:** O handler usa o lock de view de estado compartilhado para evitar races com leitores enquanto atualiza catalogos; ainda assim, chamadores devem serializar updates de ciclo de vida externamente para evitar planos conflitantes.
- **TODO:** Rebalanceamento por lane de scheduler/DA/RBC, propagacao de validator-set e cleanup de armazenamento estao pendentes; manter requisicoes de ciclo de vida pouco frequentes ate que esses hooks cheguem.

Caminho de migracao (Iroha 2 -> Iroha 3)
1) Introduzir IDs qualificados por dataspace e composicao de bloco nexus/estado global no data model; adicionar feature flags para manter modos compativeis Iroha 2 durante a transicao.
2) Implementar backends Kura/WSV em erasure coding atras de feature flags, preservando backends atuais como defaults nas fases iniciais.
3) Manter a superficie ABI v1 fixa; implementar AMX sem novos syscalls/tipos pointer e atualizar testes/docs sem mudar ABI.
4) Entregar uma chain nexus minima com um DS publico e blocos de 1 s; depois adicionar o primeiro piloto DS privado exportando apenas provas/commitments.
5) Expandir para transacoes atomicas cross-DS completas (AMX) com provas FASTPQ-ISI locais ao DS e attesters DA; habilitar QCs ML-DSA-87 em todos os DS.

Estrategia de testes
- Testes unitarios para tipos de data model, roundtrips Norito, comportamentos de syscalls AMX e codificacao/decodificacao de provas.
- Testes IVM para fixar goldens de ABI v1 (lista de syscalls, hash ABI, IDs de pointer type).
- Testes de integracao para transacoes atomicas cross-DS (positivas/negativas), metas de latencia de attesters DA (<=300 ms) e isolamento de performance sob carga.
- Testes de seguranca para verificacao de DS QC (ML-DSA-87), deteccao de conflitos/semantica de abort e prevencao de vazamento de shards confidenciais.

### Ativos de telemetria e runbook NX-18

- **Grafana board:** `dashboards/grafana/nexus_lanes.json` agora exporta o dashboard "Nexus Lane Finality & Oracles" solicitado por NX-18. Paineis cobrem `histogram_quantile()` em `iroha_slot_duration_ms`, `iroha_da_quorum_ratio`, alertas de disponibilidade DA (`sumeragi_da_gate_block_total{reason="missing_local_data"}`), gauges de preco/staleness/TWAP/haircut de oraculos e o painel vivo `iroha_settlement_buffer_xor` para que operadores provem o slot de 1 s, DA e SLOs de tesouraria sem queries bespoke.
- **CI gate:** `scripts/telemetry/check_slot_duration.py` parseia snapshots Prometheus, imprime latencia p50/p95/p99 e aplica os thresholds NX-18 (p95 <= 1000 ms, p99 <= 1100 ms). O harness companheiro `scripts/telemetry/nx18_acceptance.py` valida quorum DA, staleness/TWAP/haircuts de oraculos, buffers de settlement e quantis de slot em uma passada (`--json-out` persiste evidencia), e ambos rodam dentro de `ci/check_nexus_lane_smoke.sh` para RCs.
- **Bundler de evidencia:** `scripts/telemetry/bundle_slot_artifacts.py` copia o snapshot de metricas + resumo JSON para `artifacts/nx18/` e emite `slot_bundle_manifest.json` com digests SHA-256, garantindo que cada RC envie exatamente os artefatos que dispararam o gate NX-18.
- **Automacao de release:** `scripts/run_release_pipeline.py` agora invoca `ci/check_nexus_lane_smoke.sh` (pular com `--skip-nexus-lane-smoke`) e copia `artifacts/nx18/` para a saida de release para que a evidencia NX-18 viaje com o bundle/imagem sem etapa manual.
- **Runbook:** `docs/source/runbooks/nexus_lane_finality.md` documenta o fluxo on-call (limiares, passos de incidente, captura de evidencia, drills de caos) que acompanha o dashboard, atendendo o bullet de "publicar dashboards/runbooks de operador" do NX-18.
- **Helpers de telemetria:** reutilizar `scripts/telemetry/compare_dashboards.py` existente para diff de dashboards exportados (evitando drift staging/prod) e `scripts/telemetry/check_nexus_audit_outcome.py` durante rehearsals routed-trace ou caos para que cada drill NX-18 arquive o payload `nexus.audit.outcome` correspondente.

Perguntas abertas (necessario esclarecimento)
1) Assinaturas de transacao: Decisao - usuarios finais sao livres para escolher qualquer algoritmo de assinatura que seu DS alvo anuncie (Ed25519, secp256k1, ML-DSA, etc.). Hosts devem impor flags de capacidade multisig/curve em manifestos, fornecer fallbacks deterministas e documentar implicacoes de latencia ao misturar algoritmos. Pendente: finalizar fluxo de negociacao de capacidades em Torii/SDKs e atualizar testes de admissao.
2) Economia de gas: Cada DS pode denominar gas em um token local, enquanto fees de settlement global sao pagas em SORA XOR. Pendente: definir o caminho padrao de conversao (DEX da lane publica vs outras fontes de liquidez), hooks de contabilidade do ledger e salvaguardas para DS que subsidiam ou precificam zero.
3) Attesters DA: Numero alvo por regiao e threshold (ex., 64 amostrados, 43-of-64 assinaturas ML-DSA-87) para cumprir <=300 ms mantendo durabilidade. Alguma regiao que devemos incluir no dia um?
4) Parametros DA padrao: Propusemos DS publico `k=32, m=16` e DS privado `k=16, m=8`. Deseja um perfil de redundancia mais alto (ex., `k=30, m=20`) para certas classes de DS?
5) Granularidade DS: Dominios e assets podem ser DS. Devemos suportar DS hierarquicos (domain DS como pai de asset DS) com heranca opcional de politicas, ou manter plano para v1?
6) ISIs pesadas: Para ISIs complexos que nao conseguem produzir provas sub-segundo, devemos (a) rejeitar, (b) dividir em passos atomicos menores entre blocos, ou (c) permitir inclusao atrasada com flags explicitas?
7) Conflitos cross-DS: O conjunto de leitura/escrita declarado pelo cliente e suficiente, ou o host deve inferir e expandir automaticamente por seguranca (ao custo de mais conflitos)?

Anexo: Conformidade com politicas do repositorio
- Norito e usado para todos os formatos on-wire e serializacao JSON via helpers Norito.
- ABI v1 apenas; sem toggles de runtime para politicas de ABI. Superficies de syscalls e pointer-types sao fixas e fixadas por testes golden.
- Determinismo preservado entre hardware; aceleracao opcional e gateada.
- Sem serde em caminhos de producao; sem configuracao baseada em ambiente em producao.
