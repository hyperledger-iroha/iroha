---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: especificação do nexo
título: Especificação técnica da Sora Nexus
descrição: Espelho completo de `docs/source/nexus.md`, cobrindo a arquitetura e as restrições de design para o ledger Iroha 3 (Sora Nexus).
---

:::nota Fonte canônica
Esta página espelha `docs/source/nexus.md`. Mantenha ambas as cópias homologadas até que o backlog de tradução chegue ao portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificação técnica de design

Este documento propõe a arquitetura do Sora Nexus Ledger para Iroha 3, evoluindo o Iroha 2 para um ledger global único e logicamente unificado organizado em torno de Data Spaces (DS). Os Espaços de Dados fornecem domínios fortes de privacidade (“espaços de dados privados”) e participação aberta (“espaços de dados públicos”). O design preserva a composição ao longo do ledger global enquanto garante isolamento estrito e confidencialidade para dados de private-DS, e introduz escala de disponibilidade de dados via codificação de desligamento em Kura (block storage) e WSV (World State View).

O mesmo repositório compila tanto Iroha 2 (redes auto-hospedadas) quanto Iroha 3 (SORA Nexus). A execução e impulsionada pela Máquina Virtual Iroha (IVM) compartilhada e pela cadeia de ferramentas Kotodama, de modo que contratos e contratos de bytecode permanecem portáveis ​​entre implantações auto-hospedadas e o ledger global do Nexus.

Objetivos
- Um ledger lógico global composto por muitos validadores cooperantes e Data Spaces.
- Data Spaces privados para operação permissionada (ex., CBDCs), com dados que nunca saem do DS privado.
- Data Spaces públicos com participação aberta, acesso sem permissão estilo Ethereum.
- Contratos inteligentes compostos entre Data Spaces, sujeitos a permissões explícitas para acesso a ativos de private-DS.
- Isolamento de desempenho para que a atividade pública não degrade transações internas de DS privado.
- Disponibilidade de dados em escala: Kura e WSV com codificação de apagamento para suportar dados efetivamente ilimitados mantendo dados de private-DS privados.

Não objetivos (fase inicial)
- Definir economia de token ou incentivos de validadores; políticas de agendamento e piquetagem são conectáveis.
- Introduzir uma nova versão da ABI; mudanças visam ABI v1 com extensões explícitas de syscalls e pointer-ABI conforme a política de IVM.

Terminologia
- Nexus Ledger: O ledger lógico global formado ao composto por blocos de Data Space (DS) em uma história ordenada única e um compromisso de estado.
- Espaço de Dados (DS): Domínio delimitado de execução e armazenamento com seus próprios validadores, governança, classe de privacidade, política de DA, cotas e política de taxas. Existem duas classes: DS público e DS privado.
- Espaço Privado de Dados: Validadores permissionados e controle de acesso; dados de transação e estado nunca saem do DS. Apenas compromissos/metados são ancorados globalmente.
- Espaço Público de Dados: Participação sem permissão; dados completos e estado são públicos.
- Data Space Manifest (DS Manifest): Manifesto codificado em Norito que declara parâmetros DS (validadores/chaves QC, classe de privacidade, política ISI, parâmetros DA, retencao, quotas, política ZK, taxas). O hash do manifesto e ancorado na cadeia Nexus. Salvo override, certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) como esquema de assinatura post-quantum por padrão.
- Diretório Espacial: Contrato de diretorio global on-chain que rastreia manifestos DS, versoes e eventos de governança/rotacao para resolução e auditorias.
- DSID: Identificador globalmente único para um Data Space. Usado para namespace de todos os objetos e referências.
- Âncora: Compromisso criptográfico de um bloco/cabeçalho DS incluído na cadeia nexus para vincular a história do DS ao ledger global.
- Kura: Armazenamento de blocos Iroha. Estendido aqui com armazenamento de blobs codificados com desligamento e compromissos.
- WSV: Iroha Visão do Estado Mundial. Estendido aqui com segmentos de estado versionados, com snapshots, e codificados com apagamento.
- IVM: Máquina Virtual Iroha para execução de contratos inteligentes (bytecode Kotodama `.to`).
  - AIR: Representação Algébrica Intermediária. Visão algébrica de computação para provas estilo STARK, descrevendo a execução como caminhos baseados em campos com restrições de transição e fronteira.Modelo de Espaços de Dados
- Identidade: `DataSpaceId (DSID)` identifica um DS e faz namespace de tudo. O DS pode ser instanciado em duas granularidades:
  - Domínio-DS: `ds::domain::<domain_name>` - execução e estado delimitados a um domínio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - execução e estado delimitados a uma definição de ativo único.
  Ambas as formas coexistem; transações podem tocar múltiplos DSIDs de forma atômica.
- Ciclo de vida do manifesto: criação de DS, atualizações (rotação de chaves, mudanças de política) e transferência são registradas no Space Directory. Cada artefacto DS por slot referencia o hash do manifesto mais recente.
- Turmas: DS Público (participação aberta, DA pública) e DS Privado (permissionado, DA confidencial). Políticas hibridas são possiveis via bandeiras se manifestam.
- Políticas por DS: permissões ISI, parâmetros DA `(k,m)`, criptografia, retenção, cotas (participação min/max de tx por bloco), política de prova ZK/otimista, taxas.
- Governança: adesão DS e rotação de validadores definidos pela estação de governança do manifesto (propostas on-chain, multisig ou governança externa ancorada por transações nexus e atestações).

Manifestos de Capacidades e UAID
- Contas universais: cada participante recebe um UAID determinístico (`UniversalAccountId` em `crates/iroha_data_model/src/nexus/manifest.rs`) que abrange todos os espaços de dados. Manifestos de Capacidades (`AssetPermissionManifest`) vinculam um UAID a um dataspace específico, épocas de ativação/expiração e uma lista ordenada de regras permit/deny `ManifestEntry` que delimitam `dataspace`, `program_id`, `method`, `asset` e funções AMX variadas. Regras negam sempre ganham; o avaliador emite `ManifestVerdict::Denied` com uma razão de auditoria ou uma concessão `Allowed` com os metadados de permissão correspondentes.
- Abonos: cada entrada permite carrega buckets deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mais um `max_amount` opcional. Hosts e SDKs consomem o mesmo payload Norito, então a aplicação permanece idêntica entre hardware e SDK.
- Telemetria de auditoria: Space Directory transmite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) sempre que um manifesto muda de estado. A nova superfície `SpaceDirectoryEventFilter` permite que os assinantes Torii/data-event monitorem atualizações de manifesto UAID, revogações e decisões deny-wins sem encanamento personalizado.

Para evidenciar operacional ponta a ponta, notas de migração de SDK e checklists de publicação de manifesto, espele esta seção com o Universal Account Guide (`docs/source/universal_accounts_guide.md`). Mantenha ambos os documentos alinhados sempre que a política ou as ferramentas da UAID mudarem.

Arquitetura de alto nível
1) Camada de composição global (Nexus Chain)
- Mantem uma ordenação canônica única de blocos Nexus de 1 segundo que finalizam transações atômicas abrangendo um ou mais Espaços de Dados (DS). Cada transação comprometida atualiza o estado mundial global unificado (vetor de raízes por DS).
- Contem metadados mínimos mais provas/QCs agregados para garantir composabilidade, específicas e detecção de fraude (DSIDs tocados, raízes de estado por DS antes/depois, compromissos DA, provas de validade por DS, e o certificado de quorum DS usando ML-DSA-87). Nenhum dado privado e incluído.
- Consenso: comitê BFT global em pipeline de tamanho 22 (3f+1 com f=7), selecionado de um pool de até ~200k validadores potenciais para um mecanismo de VRF/stake por época. O comite nexus transações sequenciais e finaliza o bloco em 1s.

2) Camada de Espaço de Dados (Público/Privado)
- Executa fragmentos por DS de transações globais, atualiza WSV local do DS e produz artefatos de validade por bloco (provas por DS agregadas e compromissos DA) que se acumulam no bloco Nexus de 1 segundo.
- DS privado criptografam dados em segurança e em trânsito entre validadores autorizados; apenas compromissos e provas de validade PQ saem do DS.
- DS público exportam corpos completos de dados (via DA) e provas de validade PQ.3) Transações Atômicas Cross-Data-Space (AMX)
- Modelo: cada transação de usuário pode tocar múltiplos DS (ex., domínio DS e um ou mais ativo DS). Ela é comprometida atomicamente em um único bloco Nexus ou aborta; não há efeitos parciais.
- Prepare-Commit dentro de 1s: para cada transação candidata, DS tocados executados em paralelo contra o mesmo snapshot (roots DS no inicio do slot) e produzindo provas de validade PQ por DS (FASTPQ-ISI) e compromissos DA. O comite nexus comita a transacao apenas se todas as provas DS Ocorrem verificarem e os certificados DA chegarem a tempo (alvo <=300 ms); caso contrário, a transação é reprogramada para o slot próximo.
- Consistência: conjuntos de leitura/escrita são declaradas; a detecção de conflitos ocorre no commit contraroots do inicio do slot. Execução otimista sem bloqueios por DS evita travamentos globais; atomicidade e imposta pela regra de commit Nexus (tudo ou nada entre DS).
- Privacidade: private DS exportam apenas provas/compromissos garantidos às raízes DS pré/pós. Nenhum dado privado cru sai do DS.

4) Disponibilidade de dados (DA) com codificação de apagamento
- Kura armazena corpos de blocos e snapshots WSV como blobs codificados com apagamento. Blobs públicos são amplamente fragmentados; blobs privados são armazenados apenas em validadores private-DS, com pedaços criptografados.
- Compromissos DA são registrados tanto em artefatos DS quanto em blocos Nexus, possibilitando amostragem e garantias de recuperação sem revelar conteúdo privado.

Estrutura de bloco e commit
- Artefato de prova de Data Space (por slot de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefatos sem corpos de dados; DS público permite recuperação de corpos via DA.

- Bloco Nexus (cadência de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transações atômicas cross-DS com DSIDs tocados), ds_artifacts[], nexus_qc.
  - Função: finaliza todas as transações atômicas dos artistas DS exigem verificação; atualiza o vetor de raízes DS do estado mundial global em um passo.

Consenso e agendamento
- Consenso da Nexus Chain: BFT global em pipeline (classe Sumeragi) com comitê de 22 nos (3f+1 com f=7) mirando blocos de 1s e especificamente de 1s. Os membros do comite são selecionados por épocas via VRF/stake entre ~200k candidatos; a rotação mantém descentralização e resistência à censura.
- Consenso de Data Space: cada DS executa seu próprio BFT entre validadores para produzir artefatos por slot (provas, compromissos DA, DS QC). Os comitês lane-relay são dimensionados em `3f+1` usando `fault_tolerance` do dataspace e são amostrados de forma determinística por época a partir do pool de validadores do dataspace usando uma semente VRF ligada a `(dataspace_id, lane_id)`. DS privados são permissionados; public DS permite liveness aberta a políticas anti-Sybil. O comitê global nexus permanece inalterado.
- Agendamento de transações: os usuários submetem transações atômicas declarando DSIDs tocados e conjuntos de leitura/escrita. DS executado em paralelo dentro do slot; o comite nexus inclui a transação no bloco de 1s se todos os artistas DS verificarem e os certificados DA fores pontuais (<=300 ms).
- Isolamento de desempenho: cada DS tem membros e execução independente. As cotas por DS limitam quantas transações tocando em um DS podem ser comprometidas por bloco para evitar bloqueio de cabeçalho e proteger a latência de DS privado.

Modelo de dados e namespace
- IDs formadas por DS: todas as entidades (domínios, contas, atividades, papéis) são comprometidas por `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referências globais: uma referência global e uma tupla `(dsid, object_id, version_hint)` e pode ser colocada on-chain na camada nexus ou em descritores AMX para uso cross-DS.
- Serializacao Norito: todas as mensagens cross-DS (descritores AMX, experimentam) usam codecs Norito. Sem uso de serde em caminhos de produção.Contratos inteligentes e extensos IVM
- Contexto de execução: adição `dsid` ao contexto de execução IVM. Os contratos Kotodama sempre serão executados dentro de um espaço de dados específico.
- Primitivas atômicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimitam uma transação atômica multi-DS no host IVM.
  - `amx_touch(dsid, key)` declara intenção de leitura/escrita para detecção de conflitos contra raízes snapshot do slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> resultado (operação permitida apenas se a política permitir e o identificador para validação)
- Gestão de ativos e taxas:
  - Operações de ativos são autorizadas pelas políticas ISI/papel do DS; taxas são pagas no token de gás do DS. Os tokens de capacidade adicionais e políticas mais ricas (multi-approver, rate-limits, geofencing) podem ser adicionados depois de sem mudar o modelo atômico.
- Determinismo: todos os novos syscalls são puros e determinísticos dados como entradas e os conjuntos de leitura/escrita AMX declarados. Sem efeitos ocultos de tempo ou ambiente.Provas de validade pós-quântica (ISI generalizadas)
- FASTPQ-ISI (PQ, sem configuração confiável): um argumento baseado em hash que generaliza o design de transferência para todas as famílias ISI enquanto mira prova sub-segundo para lotes em escala 20k em hardware classe GPU.
  - Perfil operacional:
    - Nos de produção construímos ou provamos via `fastpq_prover::Prover::canonical`, que agora sempre inicializa o backend de produção; o mock determinístico foi removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permitem que os operadores fixem a execução de CPU/GPU de forma determinística enquanto o observer hook registra triplas solicitadas/resolvidas/backend para auditorias de frota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetização:
  - KV-Update AIR: trata WSV como um mapa de valor-chave tipado comprometido via Poseidon2-SMT. Cada ISI se expande para um pequeno conjunto de linhas read-check-write sobre chaves (contas, ativos, funções, domínios, metadados, fornecimento).
  - Restrições com portas de opcode: uma tabela única AIR com colunas seletoras impone regras por ISI (conservação, contadores monotônicos, permissões, verificações de intervalo, atualizações de metadados limitadas).
  - Argumentos de pesquisa: tabelas transparentes comprometidas por hash para permissões/funções, precisão de ativos e parâmetros de política evitam restrições bit a bit pesadas.
- Compromissos e atualizações de estado:
  - Prova SMT agregada: todas as chaves tocadas (pré/pós) são provadas contra `old_root`/`new_root` usando uma fronteira compactada com irmãos deduplicados.
  - Invariantes: invariantes globais (ex., fornecimento total por ativo) são impostas via igualdade de multiconjuntos entre linhas de efeito e contadores rastreados.
- Sistema de prova:
  - Compromissos polinomiais estilo FRI (DEEP-FRI) com alta aridade (8/16) e blow-up 8-16; hashes Poseidon2; transcrição Fiat-Shamir com SHA-2/3.
  - Recursão opcional: agregação recursiva local DS para comprimir micro-lotes em uma prova por slot se necessário.
- Escopo e exemplos cobertos:
  - Ativos: transferir, cunhar, queimar, registrar/cancelar registro de definições de ativos, definir precisão (limitado), definir metadados.
  - Contas/Domínios: criar/remover, definir chave/limite, adicionar/remover signatários (apenas estado; verificações de assinatura são atestadas por validadores DS, não provadas dentro do AIR).
  - Funções/Permissões (ISI): conceder/revogar funções e permissões; impostas por tabelas de pesquisa e verificações de política monotônica.
  - Contratos/AMX: marcadores start/commit AMX, capacidade mint/revoke se habilitado; provados como transições de estado e contadores de política.
- Verificações fora do AIR para preservar a latência:
  - Assinaturas e criptografia pesada (ex., assinaturas ML-DSA de usuário) são verificadas por validadores DS e atestados no DS QC; a prova de validade cobre apenas consistência de estado e conformidade de política. Isso mantem testes PQ e rápidos.
- Metas de desempenho (ilustrativas, CPU 32 núcleos + uma GPU moderna):
  - 20k ISIs mistos com key-touch pequeno (<=8 chaves/ISI): ~0,4-0,9 s de prova, ~150-450 KB de prova, ~5-15 ms de verificação.
  - ISIs mais pesados ​​(mais chaves/contrastes ricos): micro-lote (ex., 10x2k) + recursão para manter <1 s por slot.
- Configuração do Manifesto DS:
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (assinaturas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (padrão; alternativas devem ser declaradas explicitamente)
- Subsídios:
  - ISIs complexos/personalizados podem usar um STARK geral (`zk.policy = "stark_fri_general"`) com prova adiada e específica de 1 s via atestacao QC + slashing em provas invalidas.
  - Opcoes não PQ (ex., Plonk com KZG) desativar configuração confiável e não são mais suportados no build padrão.Introdução ao AIR (para Nexus)
- Traço de execução: matriz com largura (colunas de registros) e comprimento (passos). Cada linha e um passo lógico do processamento ISI; colunas armazenam valores pre/post, selectores e flags.
- Restrições:
  - Restrições de transição: impoem relações de linha a linha (ex., post_balance = pre_balance - amount para uma linha de débito quando `sel_transfer = 1`).
  - Restrições de fronteira: ligam I/O público (old_root/new_root, contadores) as primeiras/ultimas linhas.
  - Pesquisas/permutações: garantem associação e igualdade de multiconjuntos contra tabelas comprometidas (permissões, parâmetros de ativos) sem circuitos pesados ​​de bits.
- Compromisso e verificação:
  - O provador compromete traços via codificações hash e constrói polinômios de baixo grau que são válidos se as restrições forem satisfeitas.
  - O verificador checa baixo grau via FRI (hash-based, post-quantum) com poucas aberturas Merkle; o custo e logarítmico nos passos.
- Exemplo (Transferência): registros incluem pre_balance, amount, post_balance, nonce e seletores. Restrições impem não negatividade/intervalo, conservação e monotonicidade de nonce, enquanto uma multiprova SMT agregada liga folhas pré/pós a raízes antigas/novas.

Evolução de ABI e syscalls (ABI v1)
- Syscalls a adicionar (nomes ilustrativos):
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos pointer-ABI a adicionar:
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Atualizações necessárias:
  - Adicionar a `ivm::syscalls::abi_syscall_list()` (manter ordenação), portão por política.
  - Mapear números desconhecidos para `VMError::UnknownSyscall` nos hosts.
  - Atualizar testes: syscall list golden, ABI hash, ponteiro tipo ID goldens e testes de política.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de privacidade
- Contenção de dados privados: corpos de transação, diffs de estado e snapshots WSV de private DS nunca deixa o subconjunto privado de validadores.
- Exposição pública: apenas cabeçalhos, compromissos DA e provas de validade PQ são exportados.
- Provas ZK fornecidas: private DS podem produzir provas ZK (ex., saldo suficiente, política satisfeita) habilitando ações cross-DS sem revelar estado interno.
- Controle de acesso: autorização e imposta por política ISI/função dentro do DS. Os tokens de capacidade são específicos e podem ser introduzidos depois.

Isolamento de desempenho e QoS
- Consenso, mempools e armazenamento separados por DS.
- Cotas de agendamento de nexo por DS para limitar o tempo de inclusão de âncoras e evitar bloqueio de head-of-line.
- Orçamentos de recursos de contrato por DS (compute/memory/IO), impostos pelo host IVM. Contenção em DS público não pode consumir orcamentos de DS privado.
- Chamadas cross-DS assincronas evitam esperas sincronas mais longas dentro da execução private-DS.

Disponibilidade de dados e design de armazenamento
1) Codificação de desligamento
- Usar Reed-Solomon sistemático (ex., GF(2^16)) para codificação de desligamento em nível de blob de blocos Kura e snapshots WSV: parâmetros `(k, m)` com `n = k + m` shards.
- Parâmetros padrão (propostos, DS público): `k=32, m=16` (n=48), permitindo recuperação de até 16 shards perdidos com ~1,5x de expansão. Para DS privado: `k=16, m=8` (n=24) dentro do conjunto permissionado. Ambos configuráveis ​​pelo DS Manifest.
- Blobs públicos: shards distribuídos por muitos nos DA/validadores com verificações de disponibilidade por amostragem. Compromissos DA nos cabeçalhos permitem verificação por clientes leves.
- Blobs privados: shards criptografados e distribuídos apenas entre validadores privados-DS (ou custódios designados). A cadeia global carrega apenas compromissos DA (sem localização de shards ou chaves).

2) Compromissos e amostragem
- Para cada blob: calcule uma raiz Merkle sobre shards e inclua-la em `*_da_commitment`. Manter PQ evitando compromissos de curva elíptica.
- Atestadores DA: atestadores regionais amostrados por VRF (ex., 64 por região) emitem um certificado ML-DSA-87 atestando amostragem bem sucedida de shards. Meta de latência de atestado DA <=300 ms. O comitê nexus valida certificados em vez de buscar shards.

3) Integração com Kura
- Blocos armazenam corpos de transação como blobs codificados com cancelamento e compromissos Merkle.
- Headers carregam compromissos de blob; corpos são recuperáveis ​​via rede DA para DS público e via canais privados para DS privado.4) Integração com WSV
- Snapshots WSV: periodicamente checkpoint do estado DS em snapshots chunked e codificados com apagamento com compromissos registrados em headers. Entre instantâneos, mantenha logs de alterações. Snapshots públicos são amplamente fragmentados; snapshots privados permanecem dentro de validadores privados.
- Acesso com prova: contratos podem fornecer (ou solicitar) provas de estado (Merkle/Verkle) ancoradas por compromissos de snapshot. O DS privado pode fornecer atestados de conhecimento zero em vez de provas cruas.

5) Retenção e poda
- Sem poda para DS público: reter todos os corpos Kura e snapshots WSV via DA (escalabilidade horizontal). A DS privada pode definir a retenção interna, mas os compromissos exportados permanecem imutáveis. A camada nexus retem todos os blocos Nexus e compromissos de atores DS.

Rede e papéis de nós
- Validadores globais: participam do consenso nexus, validam blocos Nexus e artistas DS, realizando verificações DA para público DS.
- Validadores de Data Space: executam consenso DS, executam contratos, gerenciam Kura/WSV local, lidam com DA para seu DS.
- Nos DA (opcional): armazenam/publicam blobs públicos, facilitam amostras. Para DS privado, nossos DA são co-localizados com validadores ou custos confiáveis.

Melhorias e considerações de sistema
- Desacoplar sequenciamento/mempool: adotar um mempool DAG (ex., estilo Narwhal) alimentando um BFT em pipeline na camada nexus para reduzir a latência e melhorar o throughput sem mudar o modelo lógico.
- Cotas DS e justiça: cotas por DS por bloco e limites de peso para evitar bloqueio de linha e garantir latência previsível para DS privado.
- Atestação DS (PQ): certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) por padrão. E pós-quântico e maior que assinaturas EC, mas aceitavel com um QC por slot. O DS pode optar explicitamente por ML-DSA-65/44 (menores) ou assinaturas EC se declaradas no Manifesto DS; público DS são fortemente encorajados a manter ML-DSA-87.
- Atestadores DA: para DS público, utilize atestadores regionais amostrados por VRF que emitem certificados DA. O comite nexus valida certificados em vez de amostragem brutal de shards; private DS mantem atestações DA internas.
- Recursão e provas por epoca: opcionalmente adicionar vários micro-lotes dentro de um DS em uma prova recursiva por slot/epoca para manter tamanho de prova e tempo de verificação estaveis sob alta carga.
- Escalonamento de pistas (se necessário): se um único comitê global muda gargalo, introduza K pistas de sequenciamento paralelas com mesclagem determinística. Isso preserva uma ordem global única enquanto escala horizontalmente.
- Aceleração determinística: fornecer kernels SIMD/CUDA com feature flags para hashing/FFT com fallback CPU bit-exato para preservar determinismo cross-hardware.
- Limiares de ativação de pistas (proposta): habilitar 2-4 pistas se (a) a específica p95 ultrapassar 1,2 s por >3 minutos consecutivos, ou (b) a ocupação por bloco exceder 85% por >5 minutos, ou (c) a taxa de entrada de tx exigida >1,2x a capacidade do bloco em níveis sustentados. As pistas fazem bucket de transações de forma determinística por hash de DSID e se fazem merge no bloco nexus.

Taxas e economia (padrões iniciais)
- Unidade de gás: token de gás por DS com computar/IO medido; taxas são pagas no ativo de gás nativo do DS. Conversa entre DS e responsabilidade da aplicação.
- Prioridade de inclusão: round-robin entre DS com cotas por DS para preservar a justiça e SLOs de 1s; dentro de um DS, a licitação de taxas pode desempatar.
- Futuro: o mercado global de taxas ou políticas que minimizam o MEV podem ser explorados sem mudar a atomicidade ou o design de provas PQ.Fluxo cross-Data-Space (exemplo)
1) Um usuário envia uma transação AMX tocando público DS P e privado DS S: move o ativo X de S para o beneficiário B cuja conta está em P.
2) Dentro do slot, P e S executam seu fragmento contra o snapshot do slot. S verifica autorização e disponibilidade, atualiza seu estado interno e produz uma prova de validade PQ e compromisso DA (nenhum dado privado vaza). P prepara a atualização de estado correspondente (ex., mint/burn/locking em P conforme política) e sua prova.
3) O comite nexus verifica ambas as provas DS e certificados DA; se ambos verificarem dentro do slot, uma transação e cometida atomicamente no bloco Nexus de 1s, atualizando ambas as raízes DS no vetor de estado mundial global.
4) Se qualquer prova ou certificado DA estiver faltando/invalido, a transação aborta (sem efeitos) e o cliente poderá reenviar para o slot próximo. Nenhum dado privado sai de S em nenhum passo.

- Considerações de segurança
- Execução determinística: syscalls IVM permanecem determinísticas; resultados cross-DS são guiados por commit AMX e finalização, não por relógio ou timing de rede.
- Controle de acesso: permissões ISI em privado DS restringem quem pode submeter transações e quais operações são permitidas. Tokens de capacidade codificam direitos finos para uso cross-DS.
- Confidencialidade: criptografia end-to-end para dados private-DS, shards codificados com exclusão armazenados apenas entre membros autorizados, provas ZK adicionais para atestados externos.
- Resistência a DoS: isolamento em mempool/consenso/armazenamento impede que o congestionamento público impacte o progresso do DS privado.

Mudanças nos componentes Iroha
- iroha_data_model: insira `DataSpaceId`, IDs complicadas por DS, descritores AMX (conjuntos de leitura/escrita), tipos de prova/compromissos DA. Serializacao somente Norito.
- ivm: adicionar syscalls e tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e provas DA; atualizar testes/docs ABI conforme política v1.