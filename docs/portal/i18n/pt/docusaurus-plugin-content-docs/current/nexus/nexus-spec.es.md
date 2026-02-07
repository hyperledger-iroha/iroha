---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: especificação do nexo
título: Especificação técnica de Sora Nexus
description: Espejo completo de `docs/source/nexus.md`, que cobre a arquitetura e as restrições de design para o razão Iroha 3 (Sora Nexus).
---

:::nota Fonte canônica
Esta página reflete `docs/source/nexus.md`. Mantenha ambas as cópias alinhadas até que o backlog de tradução chegue ao portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificação técnica de design

Este documento propõe a arquitetura do Sora Nexus Ledger para Iroha 3, evoluindo Iroha 2 para um ledger global único e logicamente unificado organizado ao redor de Data Spaces (DS). Os Espaços de Dados provam domínios fuertes de privacidad ("espaços de dados privados") e participação aberta ("espaços de dados públicos"). O projeto preserva a composição através do razão global enquanto garante isolamento estrito e confidencialidade para os dados de DS privados, e introduz escalação de disponibilidade de dados via codificação de borrado em Kura (armazenamento em bloco) e WSV (World State View).

O mesmo repositório compila tanto Iroha 2 (redes autoalojadas) quanto Iroha 3 (SORA Nexus). A execução é impulsionada pela Máquina Virtual Iroha (IVM) compartilhada e pelo conjunto de ferramentas de Kotodama, porque os contratos e artefatos de bytecode permanecem portáteis entre despliegues autoalojados e o razão global de Nexus.

Objetivos
- Um livro-razão lógico global composto por muitos validadores cooperantes e espaços de dados.
- Espaços de dados privados para operação com permissões (p. ej., CBDC), com dados que nunca são vendidos no DS privado.
- Data Spaces públicos com participação aberta, acesso sem permissão no estilo Ethereum.
- Contratos inteligentes composables entre Data Spaces, sujeitos a permissões explícitas para acesso a ativos de private-DS.
- Isolamento de rendimiento para que a atividade pública não prejudique as transações internas de private-DS.
- Disponibilidade de dados em escala: Kura e WSV com codificação de borrado para suporte de dados efetivamente ilimitados, mantendo os dados de privados-DS privados.

Sem objetivos (fase inicial)
- Definir economia de token ou incentivos de validadores; as políticas de agendamento e piquetagem são enchufáveis.
- Apresentar uma nova versão da ABI; As mudanças foram feitas para ABI v1 com extensões explícitas de syscalls e ponteiro-ABI de acordo com a política de IVM.Terminologia
- Nexus Ledger: O ledger lógico global formado pelos blocos componentes do Data Space (DS) em uma história ordenada e um compromisso de estado.
- Data Space (DS): Dominio acotado de ejecucion y almacenamiento con sus propios validadores, gobernanza, clase de privacidad, política de DA, cotas e política de tarifas. Existem duas classes: DS público e DS privado.
- Espaço de Dados Privado: Validadores com permissões e controle de acesso; os dados de transação e o estado nunca são vendidos no DS. Apenas se anulam compromissos/metadados globalmente.
- Espaço de Dados Públicos: Participacion sin permisos; os dados completos e o estado são públicos.
- Data Space Manifest (DS Manifest): Manifesto codificado com Norito que declara parâmetros DS (validadores/llaves QC, classe de privacidad, política ISI, parâmetros DA, retencion, cuotas, política ZK, tarifas). O hash do manifesto é anulado na cadeia de nexo. Salvo que se anule, os certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) como esquema de firma pós-cuantico por defeito.
- Diretório Espacial: Contrato de diretorio global on-chain que rastrea manifestos DS, versões e eventos de gobernanza/rotacion para resolução de problemas e auditorias.
- DSID: Identificador globalmente único para um espaço de dados. É usado para namespace de todos os objetos e referências.
- Âncora: Compromisso criptográfico de um bloco/cabeçalho DS incluído no nexo de cadeia para vincular o histórico DS ao razão global.
- Kura: Armazenamento de blocos de Iroha. Estenda-se aqui com armazenamento de blobs codificados com borrados e compromissos.
- WSV: Iroha Visão do Estado Mundial. Se estende aqui com segmentos de estado versionados, com snapshots e codificados com borrado.
- IVM: Máquina Virtual Iroha para execução de contratos inteligentes (bytecode Kotodama `.to`).
  - AIR: Representação Algébrica Intermediária. Vista algébrica do cálculo para testes no estilo STARK, descrevendo a execução como trazidas baseadas em campos com restrições de transição e fronteira.

Modelo de Espaços de Dados
- Identidade: `DataSpaceId (DSID)` identifica um DS e o namespace de todo. Los DS podem ser instanciados em duas granularidades:
  - Domínio-DS: `ds::domain::<domain_name>` - execução e estado acotados em um domínio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - ejeção e estado acotados a uma definição de ativo único.
  Ambas formas coexistem; as transações podem gerar múltiplos DSID de forma atômica.
- Ciclo de vida do manifesto: a criação do DS, atualizações (rotação de folhas, mudanças políticas) e retirada são registradas no Diretório Espacial. Cada artefato DS por slot refere-se ao hash do manifesto mais recente.
- Aulas: DS Público (participação aberta, DA publica) e DS Privado (permisionado, DA confidencial). Políticas híbridas são possíveis através de bandeiras do manifesto.
- Políticas por DS: permissões ISI, parâmetros DA `(k,m)`, cifrado, retenção, cotas (min/max participação de tx por bloco), política de testes ZK/otimistas, tarifas.
- Governança: membro DS e rotação de validadores definidos pela seção de governança do manifesto (propostas on-chain, multisig ou governança externa anclada por transações nexo e atestados).

Manifestos de capacidades e UAID
- Contas universais: cada participante recebe um UAID determinístico (`UniversalAccountId` e `crates/iroha_data_model/src/nexus/manifest.rs`) que abarca todos os espaços de dados. Os manifestos de capacidades (`AssetPermissionManifest`) vinculam um UAID a um espaço de dados específico, períodos de ativação/expiração e uma lista ordenada de regulamentos permitir/negar `ManifestEntry` que inclui `dataspace`, `program_id`, `method`, `asset` e funções AMX opcionais. Las regilas negam sempre o ganho; o avaliador emite `ManifestVerdict::Denied` com uma razão de auditoria ou uma concessão `Allowed` com os metadados de permissão coincidentes.
- Permissões: cada entrada permite lleva buckets deterministas `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mas um `max_amount` opcional. Hosts e SDK consomem a mesma carga útil Norito, portanto a aplicação permanece idêntica entre hardware e SDK.
- Telemetria de auditoria: Space Directory emite `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) quando um manifesto de mudança de estado. A nova superfície `SpaceDirectoryEventFilter` permite que os assinantes de Torii/data-event monitorem atualizações de manifesto UAID, revogações e decisões de negação de vitórias sem encanamento personalizado.Para evidenciar operação de ponta a ponta, notas de migração de SDK e listas de verificação de publicação de manifestos, observe esta seção com o Guia de Conta Universal (`docs/source/universal_accounts_guide.md`). Mantenha ambos os documentos alinhados quando a política ou as ferramentas da UAID mudam.

Arquitetura de alto nível
1) Capa de composição global (Cadeia Nexus)
- Mantenha uma ordem canônica única de blocos Nexus de 1 segundo para finalizar transações atômicas que abarcam um ou mais Data Spaces (DS). Cada transação confirmada atualiza o estado mundial unificado global (vetor de raízes por DS).
- Contém metadados mínimos, mas testes/QC agregados para garantir a composição, finalização e detecção de fraude (DSIDs tocados, raízes de estado por DS antes/depois, compromissos DA, testes de validade por DS, e o certificado de quorum DS usando ML-DSA-87). Não são incluídos dados privados.
- Consenso: comitê BFT global com pipeline de tamanho 22 (3f+1 com f=7), selecionado de um pool de até ~200k validadores potenciais através de um mecanismo VRF/stake por épocas. O comite Nexus ordena as transações e finaliza o bloco em 1s.

2) Capa de Espaço de Dados (Público/Privado)
- Ejeta fragmentos por DS de transações globais, atualiza o WSV local do DS e produz artefatos de validade por bloco (testes agregados por DS e compromissos DA) que são agregados no bloco Nexus de 1 segundo.
- DS privado dados cifran em repouso e em trânsito entre validadores autorizados; apenas compromissos e testes de validade PQ salen del DS.
- Public DS exporta dados completos (via DA) e testes de validade PQ.

3) Transações Atômicas entre Espaço de Dados (AMX)
- Modelo: cada transação de usuário pode realizar múltiplos DS (p. ej., domínio DS e um ou mais ativo DS). Se a forma atômica for confirmada em um único bloco Nexus ou abortada; sem efeitos parciais.
- Prepare-Commit dentro de 1s: para cada transação candidata, os toques DS são executados paralelamente ao mesmo snapshot (roots DS no início do slot) e produzem testes de validade PQ por DS (FASTPQ-ISI) e compromissos DA. O comite nexus confirma a transação somente se todas as verificações DS exigidas forem verificadas e os certificados DA llegan a tiempo (objetivo <=300 ms); Caso contrário, a transação será reprogramada para o slot seguinte.
- Consistência: os conjuntos de leitura/escritura são declarados; a detecção de conflitos ocorre no commit contra as raízes do início do slot. A execução otimista sem bloqueios do DS evita bloqueios globais; a atomicidade é imposta pela regra de commit nexus (todo ou nada entre DS).
- Privacidade: DS privado exporta apenas testes/compromissos ligados a raízes DS pré/pós. Nenhum dado de venda privada cruda del DS.

4) Disponibilidade de dados (DA) com codificação de borrado
- Kura armazena blocos e instantâneos WSV como blobs codificados com borrado. Os blobs públicos são fragmentados ampliamente; Os blobs privados são armazenados apenas dentro dos validadores private-DS, com pedaços cifrados.
- Os compromissos DA são registrados tanto em artefatos DS como em blocos Nexus, habilitando museu e garantias de recuperação sem revelar conteúdo privado.

Estrutura de blocos e commit
- Artefato de teste de Data Space (por slot de 1s, por DS)
  - Campos: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Private-DS exporta artefatos sem corpos de dados; DS público permite recuperação de corpos via DA.

- Bloco Nexus (cadência de 1s)
  - Campos: block_number, parent_hash, slot_time, tx_list (transações atômicas cross-DS com DSIDs tocados), ds_artifacts[], nexus_qc.
  - Função: finaliza todas as transações atômicas dos artefatos que o DS exige verificação; atualiza o vetor de raízes DS do estado mundial global em um único passo.Consenso e agendamento
- Consenso de Nexus Chain: BFT global com pipeline (classe Sumeragi) com comitê de 22 nós (3f+1 com f=7) apuntando blocos de 1s e finalização de 1s. Os membros do comitê foram selecionados por épocas via VRF/stake entre ~200 mil candidatos; a rotação mantém a descentralização e a resistência à censura.
- Consenso de Data Space: cada DS executa seu próprio BFT entre seus validadores para produzir artefatos por slot (testes, compromissos DA, DS QC). Os comitês lane-relay são dimensionados em `3f+1` usando a configuração `fault_tolerance` do espaço de dados e são mostrados de forma determinista por época a partir do pool de validadores do espaço de dados usando a semente de época VRF ligada a `(dataspace_id, lane_id)`. DS privados são autorizados; DS público permite vivacidade aberta sujeta a política anti-Sybil. El comitê nexo global sem mudança.
- Agendamento de transações: os usuários enviam transações atômicas declarando DSIDs tocados e conjuntos de leitura/escritura. O DS é executado paralelamente dentro do slot; o comite nexus inclui a transação no bloco de 1s se todos os artefatos DS forem verificados e os certificados DA forem pontuais (<=300 ms).
- Isolamento de rendimiento: cada DS tem membros e execução independente. As cotas do DS limitam quantas transações um DS pode ser confirmado por bloqueio para evitar bloqueio de cabeçalho e proteger a latência do DS privado.

Modelo de dados e namespace
- IDs qualificados por DS: todas as entidades (domínios, contas, ativos, funções) são qualificadas por `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referências globais: uma referência global é uma tupla `(dsid, object_id, version_hint)` e pode ser colocada on-chain na capa nexus ou nos descritores AMX para uso cross-DS.
- Serializacion Norito: todas as mensagens cross-DS (descritores AMX, testes) usam codecs Norito. Não se usa caminhos de produção.

Contratos inteligentes e extensões de IVM
- Contexto de execução: adicionar `dsid` ao contexto de execução de IVM. Os contratos Kotodama são sempre executados dentro de um espaço de dados específico.
- Primitivas atômicas cross-DS:
  - `amx_begin()` / `amx_commit()` delimita uma transação atômica multi-DS no host IVM.
  - `amx_touch(dsid, key)` declara intenção de leitura/escritura para detecção de conflitos contra root snapshot do slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> resultado (operação permitida somente se a política permitir e o identificador for válido)
- Tratamento de ativos e tarifas:
  - As operações de ativos são autorizadas pelas políticas ISI/rol del DS; as tarifas são pagas no token de gás do DS. Os tokens de capacidade opcionais e políticas mais ricas (multi-aprovador, limites de taxa, geofencing) podem agregar mais adiante sem alterar o modelo atômico.
- Determinismo: todos os novos syscalls são puros e deterministas dados nas entradas e nos conjuntos de leitura/escritura AMX declarados. Sem efeitos ocultos de tempo ou entorno.Testes de validade pós-cuanticas (ISI generalizados)
- FASTPQ-ISI (PQ, sem configuração confiável): um argumento baseado em hash que generaliza o projeto de transferência para todas as famílias ISI enquanto tenta testar sub-segundo para lotes de escala 20k em hardware de classe GPU.
  - Perfil operativo:
    - Os nós de produção constroem o teste via `fastpq_prover::Prover::canonical`, que agora sempre inicializa o backend de produção; o mock determinista foi removido. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permitem que os operadores executem CPU/GPU de forma determinista enquanto o observador hook registra triplos solicitados/resueltos/backend para auditorias de flota. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetização:
  - KV-Update AIR: trata WSV como um mapa de valor-chave tipado comprometido via Poseidon2-SMT. Cada ISI se expande um conjunto pequeno de filas read-check-write sobre claves (contas, ativos, papéis, domínios, metadados, fornecimento).
  - Restrições com portas de opcode: uma única tabela AIR com colunas seletoras impostas regras por ISI (conservação, contadores monotônicos, permissões, verificações de intervalo, atualizações de metadados selecionadas).
  - Argumentos de pesquisa: tabelas transparentes comprometidas por hash para permissões/funções, precisões de ativos e parâmetros de política evitam restrições pesadas bit a bit.
- Compromissos e atualizações de estado:
  - Teste SMT agregado: todas as chaves tocadas (pré/pós) são testadas contra `old_root`/`new_root` usando uma fronteira compactada com irmãos desduplicados.
  - Invariantes: invariantes globais (p. ej., fornecimento total por ativo) são impostos via igualdade de multiconjuntos entre filas de efeito e contadores rastreados.
- Sistema de teste:
  - Compromisos polinomiales estilo FRI (DEEP-FRI) com alta aridad (8/16) e blow-up 8-16; hashes Poseidon2; transcrição Fiat-Shamir com SHA-2/3.
  - Recursão opcional: agregação recursiva local ao DS para comprimir microlotes em uma tentativa de slot se necessário.
- Alcance e exemplos cubiertos:
  - Ativos: transferir, cunhar, queimar, registrar/cancelar definições de ativos, definir precisão (acotado), definir metadados.
  - Contas/Domínios: criar/remover, definir chave/limite, adicionar/remover signatários (só estado; as verificações de firma são atestadas por validadores DS, não se experimentam dentro do AIR).
  - Funções/Permissões (ISI): conceder/revogar funções e permissões; impostos por tabelas de pesquisa e verificações de política monotônica.
  - Contratos/AMX: marcadores start/commit AMX, capacidade mint/revoke se estiver habilitado; é testado como transições de estado e contadores de política.
- Verifica a força do AR para preservar a latência:
  - Firmas e criptografia pesada (p. ej., firmas ML-DSA de usuários) são verificadas por validadores DS e são atestadas no DS QC; a tentativa de validade é apenas consistência de estado e cumprimento de política. Isso mantém testes PQ e rápidos.
- Objetivos de desempenho (ilustrativos, CPU de 32 núcleos + uma GPU moderna):
  - 20k ISI mixtas com key-touch pequeno (<=8 claves/ISI): ~0,4-0,9 s de teste, ~150-450 KB de teste, ~5-15 ms de verificação.
  - ISI mas pesadas (mas claves/constraints ricas): micro-lotes (p. ej., 10x2k) + recursão para manter por slot <1 s.
- Configuração do Manifesto DS:
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (firmas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (por defeito; alternativas devem ser declaradas explicitamente)
- Subsídios:
  - ISI completos/personalizados podem usar um STARK geral (`zk.policy = "stark_fri_general"`) com teste de lesão e finalização de 1 s via atestado QC + corte em testes inválidos.
  - As opções no PQ (p. ej., Plonk com KZG) exigem configuração confiável e não são suportadas na compilação com defeito.Introdução ao AIR (para Nexus)
- Traza de ejecucion: matriz con ancho (colunas de registros) e longitude (passos). Cada fila é um passo lógico do processamento ISI; as colunas contêm valores pré/pós, seletores e sinalizadores.
- Restrições:
  - Restrições de transição: impor relações fila a fila (p. ej., post_balance = pre_balance - amount para uma fila de débito quando `sel_transfer = 1`).
  - Restrições de fronteira: vinculação E/S pública (old_root/new_root, contadores) à primeira/última fila.
  - Pesquisas/permutações: garantem membros e igualdades de multiconjuntos contra tabelas comprometidas (permissões, parâmetros de ativos) sem circuitos pesados ​​de bits.
- Compromisso e verificação:
  - O provador compromete-se a trazer codificações hash e a construir polinômios de baixo grau que sejam válidos se as restrições forem cumpridas.
  - El verificador comprueba bajo grado via FRI (baseado em hash, pós-cuantico) com aberturas ocasionais Merkle; o custo é logarítmico nos passos.
- Exemplo (Transferência): os registros incluem pre_balance, amount, post_balance, nonce e seletores. Las restrições não impõem negatividade/rango, conservação e monotonicidad de nonce, enquanto uma multiprueba SMT agregada vincula hojas pré/pós às raízes antigas/novas.

Evolução de ABI e syscalls (ABI v1)
- Syscalls para agregar (nomes ilustrativos):
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos Pointer-ABI para agregar:
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Atualizações necessárias:
  - Agregar a `ivm::syscalls::abi_syscall_list()` (manter ordem), gatear por política.
  - Mapear números desconhecidos para `VMError::UnknownSyscall` em hosts.
  - Atualizar testes: syscall list golden, ABI hash, ponteiro tipo ID goldens e testes de política.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de privacidade
- Contenção de dados privados: cuerpos de transação, diferenças de estado e instantâneos WSV de DS privado nunca vendem o subconjunto privado de validadores.
- Exposição pública: cabeçalhos individuais, compromissos DA e testes de validade PQ se exportam.
- Testes ZK opcionais: DS privado pode produzir testes ZK (p. ej., equilíbrio suficiente, política cumplida) habilitando ações cross-DS sem revelar estado interno.
- Controle de acesso: a autorização é imposta pela política ISI/rol dentro do DS. Os tokens de capacidade são opcionais e podem ser introduzidos mais adelante.

Isolamento de desempenho e QoS
- Consenso, mempools e armazenamento separados por DS.
- Cuotas de agendamento de nexo por DS para limitar o tempo de inclusão de âncoras e evitar bloqueio de cabeçalho.
- Pré-requisitos de recursos de contrato por DS (compute/memory/IO), impostos pelo host IVM. A contenção em DS público não pode consumir pressupostos de DS privado.
- Llamadas cross-DS assincronas evitam esperas sincronas largas dentro da ejecução private-DS.

Disponibilidade de dados e distribuição de armazenamento
1) Codificação de borrado
- Usar Reed-Solomon sistemático (p. ej., GF(2^16)) para codificação de borrado no nível de blob de blocos Kura e snapshots WSV: parâmetros `(k, m)` com fragmentos `n = k + m`.
- Parâmetros defeituosos (propostos, DS público): `k=32, m=16` (n=48), permitindo recuperação de até 16 fragmentos perdidos com expansão de ~1,5x. Para DS particular: `k=16, m=8` (n=24) dentro do conjunto permitido. Ambos configuráveis ​​pelo DS Manifest.
- Blobs públicos: shards distribuídos através de muitos nodos DA/validadores com verificações de disponibilidade por muestreo. Os compromissos DA nos cabeçalhos permitem a verificação por clientes leves.
- Blobs privados: shards cifrados e distribuídos apenas entre validadores privados-DS (ou custódios designados). A cadeia global só levará compromissos DA (sem ubicaciones de shards ni llaves).

2) Compromissos e museu
- Para cada blob: calcule uma raiz Merkle sobre shards e inclua-o em `*_da_commitment`. Mantener PQ evitando comprometimentos da curva elíptica.
- Atestadores DA: atestadores regionais mostrados por VRF (p. ej., 64 por região) emitem um certificado ML-DSA-87 atestando um show existente de shards. Objetivo de latência de atestação DA <=300 ms. O comitê Nexus valida certificados em vez de extrair fragmentos.3) Integração com Kura
- Os blocos armazenam custos de transação como blobs codificados com borrado com compromissos Merkle.
- Os cabeçalhos levam a comprometimentos de blob; os corpos são recuperados via rede DA para DS público e via canais privados para DS privado.

4) Integração com WSV
- Snapshots WSV: periodicamente se faz checkpoint do estado DS em snapshots por pedaços codificados com borrado com comprometimentos registrados em cabeçalhos. Entre os snapshots, são mantidos logs de alterações. Os instantâneos públicos são fragmentados amplamente; Os snapshots privados permanecem dentro dos validadores privados.
- Acesso com verificações: os contratos podem fornecer (ou solicitar) verificações de estado (Merkle/Verkle) canceladas por compromissos de snapshot. O DS privado pode fornecer atestados de conhecimento cero em vez de testes brutos.

5) Retenção e poda
- Sem poda para DS público: retener todos os cuerpos Kura e instantâneos WSV via DA (escalado horizontal). O DS privado pode definir a retenção interna, mas os compromissos exportados permanecem imutáveis. La capa nexus retém todos os blocos Nexus e os compromissos de artefatos DS.

Red e papéis de nós
- Validadores globais: participam do consenso nexus, validam blocos Nexus e artefatos DS, realizam verificações DA para DS público.
- Validadores de Data Space: ejecutan consenso DS, ejecutan contratos, gestionan Kura/WSV local, manejan DA para su DS.
- Nodos DA (opcional): almacenan/publican blobs publicos, facilitan muestreo. Para DS privado, os nodos DA são ubicanos com validadores ou custos confiáveis.

Melhores e considerações sobre o nível do sistema
- Desacoplar sequência/mempool: adote um mempool DAG (p. ej., estilo Narwhal) que alimenta um BFT com pipeline no capa nexus para reduzir a latência e melhorar a taxa de transferência sem alterar o modelo lógico.
- Cotas DS e justiça: cotas por DS por bloco e limites máximos de peso para evitar bloqueio de linha e garantir latência predecível para DS privado.
- Atestado DS (PQ): os certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) por defeito. É pós-cuantico e mas grande que firmas EC mas são aceitáveis ​​com um QC por slot. O DS pode optar explicitamente por ML-DSA-65/44 (mas pequeno) ou empresas EC se forem declaradas no DS Manifest; recomendamos manter ML-DSA-87 para DS público.
- Atestadores DA: para DS público, use atestadores regionais mostrados por VRF que emitem certificados DA. O comitê nexus valida certificados em vez de exibição de fragmentos brutos; private DS mantienen atestaciones DA internacionais.
- Recursão e testes por época: opcionalmente, adicione vários microlotes dentro de um DS em um teste recursivo por slot/epoca para manter o tempo de teste e tempo de verificação estabelecidos em alta carga.
- Escalado de pistas (se necessário): se um comitê global único se tornar um copo de garrafa, introduza K pistas de sequência paralelamente com uma fusão determinista. Isto preserva uma ordem global única enquanto escala horizontalmente.
- Aceleração determinista: prove kernels SIMD/CUDA com feature flags para hashing/FFT com fallback CPU bit-exacto para preservar o determinismo cross-hardware.
- Guarda-chuvas de ativação de pistas (proposta): habilitar 2-4 pistas se (a) p95 de finalização exceder 1,2 s por >3 minutos consecutivos, o (b) a ocupação por bloco exceder 85% por >5 minutos, ou (c) a tasa entrada de tx requeriria >1,2x a capacidade de bloqueio em níveis sustentados. Las lanes agrupam transações de forma determinista por hash de DSID e se fundem no bloco nexus.

Tarifas e economia (valores iniciais)
- Unidade de gás: token de gás por DS com computação/IO medido; as tarifas são pagas no ativo de gás nativo do DS. A conversão entre DS é de responsabilidade da aplicação.
- Prioridade de inclusão: round-robin entre DS com cotas por DS para preservar a justiça e SLOs de 1s; dentro de um DS, a taxa de licitação pode ser cancelada.
- Futuro: você poderá explorar um mercado global de tarifas ou políticas que minimizem o MEV sem mudar a atomicidade ou o design de testes PQ.Flujo cross-Data-Space (exemplo)
1) Um usuário envia uma transação AMX que toca DS P público e DS S privado: move o ativo X de S para o beneficiário B, cuja conta está em P.
2) Dentro do slot, P e S ejecutam seu fragmento contra o instantâneo do slot. Ao verificar autorização e disponibilidade, atualizar seu estado interno e produzir uma tentativa de validade PQ e compromisso DA (sem filtrar dados privados). P prepara la actualizacion de estado correspondente (p. ej., mint/burn/locking en P segun politica) y su prueba.
3) O comite nexus verifica ambos os testes DS e certificados DA; se ambas forem verificadas dentro do slot, a transação será confirmada atomicamente no bloco Nexus de 1s, atualizando ambas as raízes DS no vetor de estado mundial global.
4) Se algum certificado DA estiver faltando ou for inválido, a transação será abortada (sem efeitos) e o cliente poderá ser reenviado para o slot seguinte. Ningun dato privado sale de S en ningun paso.

- Considerações de segurança
- Execução determinista: as syscalls IVM permanecem deterministas; os resultados cross-DS ditam AMX commit e finalização, sem o relógio ou o timing do red.
- Controle de acesso: as permissões ISI em DS privado restringem quem pode enviar transações e que operações são permitidas. Os tokens de capacidade codificam direitos de grano fino para uso cross-DS.
- Confidencialidade: cifrado ponta a ponta para dados privados-DS, shards codificados com borrado armazenado apenas entre membros autorizados, testes ZK opcionais para atestados externos.
- Resistência a DoS: o isolamento em mempool/consenso/almacenamiento evita que o congestionamento público afete o progresso do DS privado.

Mudanças nos componentes do Iroha
- iroha_data_model: introdução `DataSpaceId`, IDs qualificados por DS, descritores AMX (conjuntos de leitura/escritura), tipos de teste/compromissos DA. Serialização solo Norito.
- ivm: agrega syscalls e tipos pointer-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e testa DA; atualizar testes/documentos da ABI após a política v1.