---
lang: he
direction: rtl
source: docs/portal/docs/nexus/nexus-spec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-spec
title: Especificacao tecnica da Sora Nexus
description: Espelho completo de `docs/source/nexus.md`, cobrindo a arquitetura e as restricoes de design para o ledger Iroha 3 (Sora Nexus).
---

:::note Fonte canonica
Esta pagina espelha `docs/source/nexus.md`. Mantenha ambas as copias alinhadas ate que o backlog de traducao chegue ao portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificacao tecnica de design

Este documento propoe a arquitetura do Sora Nexus Ledger para Iroha 3, evoluindo o Iroha 2 para um ledger global unico e logicamente unificado organizado em torno de Data Spaces (DS). Data Spaces fornecem dominios fortes de privacidade ("private data spaces") e participacao aberta ("public data spaces"). O design preserva a composabilidade ao longo do ledger global enquanto garante isolamento estrito e confidencialidade para dados de private-DS, e introduz escala de disponibilidade de dados via codificacao de apagamento em Kura (block storage) e WSV (World State View).

O mesmo repositorio compila tanto Iroha 2 (redes auto-hospedadas) quanto Iroha 3 (SORA Nexus). A execucao e impulsionada pela Iroha Virtual Machine (IVM) compartilhada e pela toolchain Kotodama, de modo que contratos e artefatos de bytecode permanecem portaveis entre deployments auto-hospedados e o ledger global do Nexus.

Objetivos
- Um ledger logico global composto por muitos validadores cooperantes e Data Spaces.
- Data Spaces privados para operacao permissionada (ex., CBDCs), com dados que nunca saem do DS privado.
- Data Spaces publicos com participacao aberta, acesso sem permissao estilo Ethereum.
- Contratos inteligentes composaveis entre Data Spaces, sujeitos a permissoes explicitas para acesso a ativos de private-DS.
- Isolamento de desempenho para que a atividade publica nao degrade transacoes internas de private-DS.
- Disponibilidade de dados em escala: Kura e WSV com codificacao de apagamento para suportar dados efetivamente ilimitados mantendo dados de private-DS privados.

Nao objetivos (fase inicial)
- Definir economia de token ou incentivos de validadores; politicas de scheduling e staking sao pluggable.
- Introduzir uma nova versao de ABI; mudancas visam ABI v1 com extensoes explicitas de syscalls e pointer-ABI conforme a politica de IVM.

Terminologia
- Nexus Ledger: O ledger logico global formado ao compor blocos de Data Space (DS) em uma historia ordenada unica e um compromisso de estado.
- Data Space (DS): Dominio delimitado de execucao e armazenamento com seus proprios validadores, governanca, classe de privacidade, politica de DA, quotas e politica de taxas. Existem duas classes: public DS e private DS.
- Private Data Space: Validadores permissionados e controle de acesso; dados de transacao e estado nunca saem do DS. Apenas compromissos/metadados sao ancorados globalmente.
- Public Data Space: Participacao sem permissao; dados completos e estado sao publicos.
- Data Space Manifest (DS Manifest): Manifest codificado em Norito que declara parametros DS (validadores/chaves QC, classe de privacidade, politica ISI, parametros DA, retencao, quotas, politica ZK, taxas). O hash do manifest e ancorado na cadeia nexus. Salvo override, certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) como esquema de assinatura post-quantum por padrao.
- Space Directory: Contrato de diretorio global on-chain que rastreia manifests DS, versoes e eventos de governanca/rotacao para resolucao e auditorias.
- DSID: Identificador globalmente unico para um Data Space. Usado para namespacing de todos os objetos e referencias.
- Anchor: Compromisso criptografico de um bloco/header DS incluido na cadeia nexus para vincular a historia do DS ao ledger global.
- Kura: Armazenamento de blocos Iroha. Estendido aqui com armazenamento de blobs codificados com apagamento e compromissos.
- WSV: Iroha World State View. Estendido aqui com segmentos de estado versionados, com snapshots, e codificados com apagamento.
- IVM: Iroha Virtual Machine para execucao de contratos inteligentes (bytecode Kotodama `.to`).
  - AIR: Algebraic Intermediate Representation. Visao algebrica de computacao para provas estilo STARK, descrevendo a execucao como tracos baseados em campos com restricoes de transicao e fronteira.

Modelo de Data Spaces
- Identidade: `DataSpaceId (DSID)` identifica um DS e faz namespacing de tudo. DS podem ser instanciados em duas granularidades:
  - Domain-DS: `ds::domain::<domain_name>` - execucao e estado delimitados a um dominio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execucao e estado delimitados a uma definicao de ativo unica.
  Ambas as formas coexistem; transacoes podem tocar multiplos DSIDs de forma atomica.
- Ciclo de vida do manifest: criacao de DS, atualizacoes (rotacao de chaves, mudancas de politica) e aposentadoria sao registradas no Space Directory. Cada artefato DS por slot referencia o hash do manifest mais recente.
- Classes: Public DS (participacao aberta, DA publica) e Private DS (permissionado, DA confidencial). Politicas hibridas sao possiveis via flags do manifest.
- Politicas por DS: permissoes ISI, parametros DA `(k,m)`, criptografia, retencao, quotas (participacao min/max de tx por bloco), politica de prova ZK/otimista, taxas.
- Governanca: membership DS e rotacao de validadores definida pela secao de governanca do manifest (propostas on-chain, multisig ou governanca externa ancorada por transacoes nexus e atestacoes).

Manifests de capacidades e UAID
- Contas universais: cada participante recebe um UAID deterministico (`UniversalAccountId` em `crates/iroha_data_model/src/nexus/manifest.rs`) que abrange todos os dataspaces. Manifests de capacidades (`AssetPermissionManifest`) vinculam um UAID a um dataspace especifico, epocas de ativacao/expiracao e uma lista ordenada de regras allow/deny `ManifestEntry` que delimitam `dataspace`, `program_id`, `method`, `asset` e roles AMX opcionais. Regras deny sempre ganham; o avaliador emite `ManifestVerdict::Denied` com uma razao de auditoria ou um grant `Allowed` com a metadata de allowance correspondente.
- Allowances: cada entrada allow carrega buckets deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mais um `max_amount` opcional. Hosts e SDKs consomem o mesmo payload Norito, entao a aplicacao permanece identica entre hardware e SDK.
- Telemetria de auditoria: Space Directory transmite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) sempre que um manifest muda de estado. A nova superficie `SpaceDirectoryEventFilter` permite que assinantes Torii/data-event monitorem atualizacoes de manifest UAID, revogacoes e decisoes deny-wins sem plumbing personalizado.

Para evidencia operacional end-to-end, notas de migracao de SDK e checklists de publicacao de manifest, espelhe esta secao com a Universal Account Guide (`docs/source/universal_accounts_guide.md`). Mantenha ambos os documentos alinhados sempre que a politica ou as ferramentas UAID mudarem.

Arquitetura de alto nivel
1) Camada de composicao global (Nexus Chain)
- Mantem uma unica ordenacao canonica de blocos Nexus de 1 segundo que finalizam transacoes atomicas abrangendo um ou mais Data Spaces (DS). Cada transacao committed atualiza o world state global unificado (vetor de roots por DS).
- Contem metadados minimos mais provas/QCs agregados para garantir composabilidade, finalidade e deteccao de fraude (DSIDs tocados, roots de estado por DS antes/depois, compromissos DA, provas de validade por DS, e o certificado de quorum DS usando ML-DSA-87). Nenhum dado privado e incluido.
- Consenso: comite BFT global em pipeline de tamanho 22 (3f+1 com f=7), selecionado de um pool de ate ~200k validadores potenciais por um mecanismo de VRF/stake por epoca. O comite nexus sequencia transacoes e finaliza o bloco em 1s.

2) Camada de Data Space (Public/Private)
- Executa fragmentos por DS de transacoes globais, atualiza WSV local do DS e produz artefatos de validade por bloco (provas por DS agregadas e compromissos DA) que se acumulam no bloco Nexus de 1 segundo.
- Private DS criptografam dados em repouso e em transito entre validadores autorizados; apenas compromissos e provas de validade PQ saem do DS.
- Public DS exportam corpos completos de dados (via DA) e provas de validade PQ.

3) Transacoes atomicas cross-Data-Space (AMX)
- Modelo: cada transacao de usuario pode tocar multiplos DS (ex., domain DS e um ou mais asset DS). Ela e committed atomicamente em um unico bloco Nexus ou aborta; nao ha efeitos parciais.
- Prepare-Commit dentro de 1s: para cada transacao candidata, DS tocados executam em paralelo contra o mesmo snapshot (roots DS no inicio do slot) e produzem provas de validade PQ por DS (FASTPQ-ISI) e compromissos DA. O comite nexus commita a transacao apenas se todas as provas DS exigidas verificarem e os certificados DA chegarem a tempo (alvo <=300 ms); caso contrario, a transacao e reprogramada para o proximo slot.
- Consistencia: conjuntos de leitura/escrita sao declarados; deteccao de conflitos ocorre no commit contra roots do inicio do slot. Execucao otimista sem locks por DS evita stalls globais; atomicidade e imposta pela regra de commit nexus (tudo ou nada entre DS).
- Privacidade: private DS exportam apenas provas/compromissos vinculados aos roots DS pre/post. Nenhum dado privado cru sai do DS.

4) Disponibilidade de dados (DA) com codificacao de apagamento
- Kura armazena corpos de blocos e snapshots WSV como blobs codificados com apagamento. Blobs publicos sao amplamente shardados; blobs privados sao armazenados apenas em validadores private-DS, com chunks criptografados.
- Compromissos DA sao registrados tanto em artefatos DS quanto em blocos Nexus, possibilitando amostragem e garantias de recuperacao sem revelar conteudo privado.

Estrutura de bloco e commit
- Artefato de prova de Data Space (por slot de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefatos sem corpos de dados; public DS permite recuperacao de corpos via DA.

- Nexus Block (cadencia de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transacoes atomicas cross-DS com DSIDs tocados), ds_artifacts[], nexus_qc.
  - Funcao: finaliza todas as transacoes atomicas cujos artefatos DS requeridos verificam; atualiza o vetor de roots DS do world state global em um passo.

Consenso e scheduling
- Consenso da Nexus Chain: BFT global em pipeline (classe Sumeragi) com comite de 22 nos (3f+1 com f=7) mirando blocos de 1s e finalidade de 1s. Membros do comite sao selecionados por epocas via VRF/stake entre ~200k candidatos; a rotacao mantem descentralizacao e resistencia a censura.
- Consenso de Data Space: cada DS executa seu proprio BFT entre validadores para produzir artefatos por slot (provas, compromissos DA, DS QC). Comites lane-relay sao dimensionados em `3f+1` usando `fault_tolerance` do dataspace e sao amostrados de forma deterministica por epoca a partir do pool de validadores do dataspace usando a seed VRF ligada a `(dataspace_id, lane_id)`. Private DS sao permissionados; public DS permitem liveness aberta sujeita a politicas anti-Sybil. O comite global nexus permanece inalterado.
- Scheduling de transacoes: usuarios submetem transacoes atomicas declarando DSIDs tocados e conjuntos de leitura/escrita. DS executam em paralelo dentro do slot; o comite nexus inclui a transacao no bloco de 1s se todos os artefatos DS verificarem e os certificados DA forem pontuais (<=300 ms).
- Isolamento de desempenho: cada DS tem mempools e execucao independentes. Quotas por DS limitam quantas transacoes tocando um DS podem ser committed por bloco para evitar head-of-line blocking e proteger a latencia de private DS.

Modelo de dados e namespacing
- IDs qualificados por DS: todas as entidades (dominios, contas, ativos, roles) sao qualificadas por `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referencias globais: uma referencia global e uma tupla `(dsid, object_id, version_hint)` e pode ser colocada on-chain na camada nexus ou em descritores AMX para uso cross-DS.
- Serializacao Norito: todas as mensagens cross-DS (descritores AMX, provas) usam codecs Norito. Sem uso de serde em caminhos de producao.

Contratos inteligentes e extensoes IVM
- Contexto de execucao: adicionar `dsid` ao contexto de execucao IVM. Contratos Kotodama sempre executam dentro de um Data Space especifico.
- Primitivas atomicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitam uma transacao atomica multi-DS no host IVM.
  - `amx_touch(dsid, key)` declara intencao de leitura/escrita para deteccao de conflitos contra roots snapshot do slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> result (operacao permitida apenas se a politica permitir e o handle for valido)
- Asset handles e taxas:
  - Operacoes de ativos sao autorizadas pelas politicas ISI/role do DS; taxas sao pagas no token de gas do DS. Capability tokens opcionais e politicas mais ricas (multi-approver, rate-limits, geofencing) podem ser adicionados depois sem mudar o modelo atomico.
- Determinismo: todas as novas syscalls sao puras e deterministicas dadas as entradas e os conjuntos de leitura/escrita AMX declarados. Sem efeitos ocultos de tempo ou ambiente.

Provas de validade post-quantum (ISI generalizados)
- FASTPQ-ISI (PQ, sem trusted setup): um argumento hash-based que generaliza o design de transfer para todas as familias ISI enquanto mira prova sub-segundo para lotes em escala 20k em hardware classe GPU.
  - Perfil operacional:
    - Nos de producao constroem o prover via `fastpq_prover::Prover::canonical`, que agora sempre inicializa o backend de producao; o mock deterministico foi removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permitem que operadores fixem execucao CPU/GPU de forma deterministica enquanto o observer hook registra triplas solicitadas/resolvidas/backend para auditorias de frota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetizacao:
  - KV-Update AIR: trata WSV como um mapa key-value tipado comprometido via Poseidon2-SMT. Cada ISI se expande para um pequeno conjunto de linhas read-check-write sobre chaves (contas, ativos, roles, dominios, metadata, supply).
  - Restricoes com gates de opcode: uma unica tabela AIR com colunas seletoras impone regras por ISI (conservacao, contadores monotonic, permissoes, range checks, atualizacoes de metadata limitadas).
  - Argumentos de lookup: tabelas transparentes comprometidas por hash para permissoes/roles, precisao de ativos e parametros de politica evitam restricoes bitwise pesadas.
- Compromissos e atualizacoes de estado:
  - Prova SMT agregada: todas as chaves tocadas (pre/post) sao provadas contra `old_root`/`new_root` usando um frontier comprimido com siblings deduplicados.
  - Invariantes: invariantes globais (ex., supply total por ativo) sao impostas via igualdade de multiconjuntos entre linhas de efeito e contadores rastreados.
- Sistema de prova:
  - Compromissos polinomiais estilo FRI (DEEP-FRI) com alta aridade (8/16) e blow-up 8-16; hashes Poseidon2; transcript Fiat-Shamir com SHA-2/3.
  - Recursao opcional: agregacao recursiva local DS para comprimir micro-lotes em uma prova por slot se necessario.
- Escopo e exemplos cobertos:
  - Ativos: transfer, mint, burn, register/unregister asset definitions, set precision (limitado), set metadata.
  - Contas/Dominios: create/remove, set key/threshold, add/remove signatories (apenas estado; verificacoes de assinatura sao atestadas por validadores DS, nao provadas dentro do AIR).
  - Roles/Permissoes (ISI): grant/revoke roles e permissoes; impostas por tabelas de lookup e checks de politica monotonic.
  - Contratos/AMX: marcadores begin/commit AMX, capability mint/revoke se habilitado; provados como transicoes de estado e contadores de politica.
- Checks fora do AIR para preservar latencia:
  - Assinaturas e criptografia pesada (ex., assinaturas ML-DSA de usuario) sao verificadas por validadores DS e atestadas no DS QC; a prova de validade cobre apenas consistencia de estado e conformidade de politica. Isso mantem provas PQ e rapidas.
- Metas de desempenho (ilustrativas, CPU 32 cores + uma GPU moderna):
  - 20k ISIs mistos com key-touch pequeno (<=8 chaves/ISI): ~0.4-0.9 s de prova, ~150-450 KB de prova, ~5-15 ms de verificacao.
  - ISIs mais pesadas (mais chaves/contrastes ricos): micro-lote (ex., 10x2k) + recursao para manter <1 s por slot.
- Configuracao do DS Manifest:
  - `zk.policy = "fastpq_isi"`
  - `zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  - `state.commitment = "smt_poseidon2"`
  - `zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (assinaturas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (padrao; alternativas devem ser declaradas explicitamente)
- Fallbacks:
  - ISIs complexas/personalizadas podem usar um STARK geral (`zk.policy = "stark_fri_general"`) com prova adiada e finalidade de 1 s via atestacao QC + slashing em provas invalidas.
  - Opcoes nao PQ (ex., Plonk com KZG) exigem trusted setup e nao sao mais suportadas no build padrao.

Introducao ao AIR (para Nexus)
- Traco de execucao: matriz com largura (colunas de registros) e comprimento (passos). Cada linha e um passo logico do processamento ISI; colunas armazenam valores pre/post, seletores e flags.
- Restricoes:
  - Restricoes de transicao: impoem relacoes de linha a linha (ex., post_balance = pre_balance - amount para uma linha de debito quando `sel_transfer = 1`).
  - Restricoes de fronteira: ligam I/O publico (old_root/new_root, contadores) as primeiras/ultimas linhas.
  - Lookups/permutations: garantem membership e igualdade de multiconjuntos contra tabelas comprometidas (permissoes, parametros de ativos) sem circuitos pesados de bits.
- Compromisso e verificacao:
  - O prover compromete traces via codificacoes hash e constroi polinomios de baixo grau que sao validos se as restricoes forem satisfeitas.
  - O verifier checa baixo grau via FRI (hash-based, post-quantum) com poucas aberturas Merkle; o custo e logaritmico nos passos.
- Exemplo (Transfer): registros incluem pre_balance, amount, post_balance, nonce e seletores. Restricoes impem nao negatividade/intervalo, conservacao e monotonicidade de nonce, enquanto uma multiprova SMT agregada liga folhas pre/post a roots old/new.

Evolucao de ABI e syscalls (ABI v1)
- Syscalls a adicionar (nomes ilustrativos):
  - `SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos pointer-ABI a adicionar:
  - `PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Atualizacoes necessarias:
  - Adicionar a `ivm::syscalls::abi_syscall_list()` (manter ordenacao), gate por politica.
  - Mapear numeros desconhecidos para `VMError::UnknownSyscall` nos hosts.
  - Atualizar testes: syscall list golden, ABI hash, pointer type ID goldens e policy tests.
  - Docs: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de privacidade
- Contencao de dados privados: corpos de transacao, diffs de estado e snapshots WSV de private DS nunca deixam o subconjunto privado de validadores.
- Exposicao publica: apenas headers, compromissos DA e provas de validade PQ sao exportados.
- Provas ZK opcionais: private DS podem produzir provas ZK (ex., saldo suficiente, politica satisfeita) habilitando acoes cross-DS sem revelar estado interno.
- Controle de acesso: autorizacao e imposta por politicas ISI/role dentro do DS. Capability tokens sao opcionais e podem ser introduzidos depois.

Isolamento de desempenho e QoS
- Consenso, mempools e armazenamento separados por DS.
- Quotas de scheduling nexus por DS para limitar o tempo de inclusao de anchors e evitar head-of-line blocking.
- Orcamentos de recursos de contrato por DS (compute/memory/IO), impostos pelo host IVM. Contencao em public DS nao pode consumir orcamentos de private DS.
- Chamadas cross-DS assincronas evitam esperas sincronas longas dentro da execucao private-DS.

Disponibilidade de dados e design de armazenamento
1) Codificacao de apagamento
- Usar Reed-Solomon sistematico (ex., GF(2^16)) para codificacao de apagamento em nivel de blob de blocos Kura e snapshots WSV: parametros `(k, m)` com `n = k + m` shards.
- Parametros padrao (propostos, public DS): `k=32, m=16` (n=48), permitindo recuperacao de ate 16 shards perdidos com ~1.5x de expansao. Para private DS: `k=16, m=8` (n=24) dentro do conjunto permissionado. Ambos configuraveis por DS Manifest.
- Blobs publicos: shards distribuidos por muitos nos DA/validadores com checagens de disponibilidade por amostragem. Compromissos DA nos headers permitem verificacao por light clients.
- Blobs privados: shards criptografados e distribuidos apenas entre validadores private-DS (ou custodios designados). A cadeia global carrega apenas compromissos DA (sem localizacoes de shards ou chaves).

2) Compromissos e amostragem
- Para cada blob: calcular uma raiz Merkle sobre shards e inclui-la em `*_da_commitment`. Manter PQ evitando compromissos de curva eliptica.
- DA Attesters: attesters regionais amostrados por VRF (ex., 64 por regiao) emitem um certificado ML-DSA-87 atestando amostragem bem sucedida de shards. Meta de latencia de atestacao DA <=300 ms. O comite nexus valida certificados em vez de buscar shards.

3) Integracao com Kura
- Blocos armazenam corpos de transacao como blobs codificados com apagamento e compromissos Merkle.
- Headers carregam compromissos de blob; corpos sao recuperaveis via rede DA para public DS e via canais privados para private DS.

4) Integracao com WSV
- Snapshots WSV: periodicamente checkpoint do estado DS em snapshots chunked e codificados com apagamento com compromissos registrados em headers. Entre snapshots, mantem change logs. Snapshots publicos sao amplamente shardados; snapshots privados permanecem dentro de validadores privados.
- Acesso com prova: contratos podem fornecer (ou solicitar) provas de estado (Merkle/Verkle) ancoradas por compromissos de snapshot. Private DS podem fornecer atestacoes zero-knowledge em vez de provas cruas.

5) Retencao e pruning
- Sem pruning para public DS: reter todos os corpos Kura e snapshots WSV via DA (escalabilidade horizontal). Private DS podem definir retencao interna, mas compromissos exportados permanecem imutaveis. A camada nexus retem todos os blocos Nexus e compromissos de artefatos DS.

Rede e papeis de nos
- Validadores globais: participam do consenso nexus, validam blocos Nexus e artefatos DS, realizam checagens DA para public DS.
- Validadores de Data Space: executam consenso DS, executam contratos, gerenciam Kura/WSV local, lidam com DA para seu DS.
- Nos DA (opcional): armazenam/publicam blobs publicos, facilitam amostragem. Para private DS, nos DA sao co-localizados com validadores ou custodios confiaveis.

Melhorias e consideracoes de sistema
- Desacoplamento sequencing/mempool: adotar um mempool DAG (ex., estilo Narwhal) alimentando um BFT em pipeline na camada nexus para reduzir latencia e melhorar throughput sem mudar o modelo logico.
- Quotas DS e fairness: quotas por DS por bloco e caps de peso para evitar head-of-line blocking e garantir latencia previsivel para private DS.
- Atestacao DS (PQ): certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) por padrao. E post-quantum e maior que assinaturas EC, mas aceitavel com um QC por slot. DS podem optar explicitamente por ML-DSA-65/44 (menores) ou assinaturas EC se declarado no DS Manifest; public DS sao fortemente encorajados a manter ML-DSA-87.
- DA attesters: para public DS, usar attesters regionais amostrados por VRF que emitem certificados DA. O comite nexus valida certificados em vez de amostragem bruta de shards; private DS mantem atestacoes DA internas.
- Recursao e provas por epoca: opcionalmente agregar varios micro-lotes dentro de um DS em uma prova recursiva por slot/epoca para manter tamanho de prova e tempo de verificacao estaveis sob alta carga.
- Escalonamento de lanes (se necessario): se um unico comite global virar gargalo, introduzir K lanes de sequenciamento paralelas com merge deterministico. Isso preserva uma unica ordem global enquanto escala horizontalmente.
- Aceleracao deterministica: fornecer kernels SIMD/CUDA com feature flags para hashing/FFT com fallback CPU bit-exato para preservar determinismo cross-hardware.
- Limiares de ativacao de lanes (proposta): habilitar 2-4 lanes se (a) a finalidade p95 exceder 1.2 s por >3 minutos consecutivos, ou (b) a ocupacao por bloco exceder 85% por >5 minutos, ou (c) a taxa de entrada de tx exigiria >1.2x a capacidade do bloco em niveis sustentados. Lanes fazem bucket de transacoes de forma deterministica por hash de DSID e se fazem merge no bloco nexus.

Taxas e economia (padroes iniciais)
- Unidade de gas: token de gas por DS com compute/IO medido; taxas sao pagas no ativo de gas nativo do DS. Conversao entre DS e responsabilidade da aplicacao.
- Prioridade de inclusao: round-robin entre DS com quotas por DS para preservar fairness e SLOs de 1s; dentro de um DS, fee bidding pode desempatar.
- Futuro: mercado global de taxas ou politicas que minimizem MEV podem ser explorados sem mudar a atomicidade ou o design de provas PQ.

Fluxo cross-Data-Space (exemplo)
1) Um usuario envia uma transacao AMX tocando public DS P e private DS S: mover o ativo X de S para o beneficiario B cuja conta esta em P.
2) Dentro do slot, P e S executam seu fragmento contra o snapshot do slot. S verifica autorizacao e disponibilidade, atualiza seu estado interno e produz uma prova de validade PQ e compromisso DA (nenhum dado privado vaza). P prepara a atualizacao de estado correspondente (ex., mint/burn/locking em P conforme politica) e sua prova.
3) O comite nexus verifica ambas as provas DS e certificados DA; se ambas verificarem dentro do slot, a transacao e committed atomicamente no bloco Nexus de 1s, atualizando ambos roots DS no vetor de world state global.
4) Se qualquer prova ou certificado DA estiver faltando/invalido, a transacao aborta (sem efeitos) e o cliente pode reenviar para o proximo slot. Nenhum dado privado sai de S em nenhum passo.

- Consideracoes de seguranca
- Execucao deterministica: syscalls IVM permanecem deterministicas; resultados cross-DS sao guiados por commit AMX e finalizacao, nao por relogio ou timing de rede.
- Controle de acesso: permissoes ISI em private DS restringem quem pode submeter transacoes e quais operacoes sao permitidas. Capability tokens codificam direitos finos para uso cross-DS.
- Confidencialidade: criptografia end-to-end para dados private-DS, shards codificados com apagamento armazenados apenas entre membros autorizados, provas ZK opcionais para atestacoes externas.
- Resistencia a DoS: isolamento em mempool/consenso/armazenamento impede que congestionamento publico impacte o progresso de private DS.

Mudancas nos componentes Iroha
- iroha_data_model: introduzir `DataSpaceId`, IDs qualificados por DS, descritores AMX (conjuntos leitura/escrita), tipos de prova/compromissos DA. Serializacao somente Norito.
- ivm: adicionar syscalls e tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e provas DA; atualizar testes/docs ABI conforme politica v1.
