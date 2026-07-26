---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-spec.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: especificação do nexo
título: Especificação técnica de Sora Nexus
descrição: Espelho completo de `docs/source/nexus.md`, cobrindo a arquitetura e as restrições de concepção para o livro razão Iroha 3 (Sora Nexus).
---

:::nota Fonte canônica
Esta página representa `docs/source/nexus.md`. Certifique-se de que as duas cópias estejam alinhadas até que o atraso na tradução chegue ao portal.
:::

#! Iroha 3 - Sora Nexus Ledger: Especificação técnica de concepção

Este documento propõe a arquitetura do Sora Nexus Ledger para Iroha 3, evoluindo rapidamente Iroha 2 para um ledger global exclusivo e logístico unificado que organiza automaticamente seus espaços de dados (DS). Les Data Spaces fornecem domínios de confidencialidade fortes ("espaços de dados privados") e uma participação ouverte ("espaços de dados públicos"). O design preserva a compatibilidade através do livro-razão global, garantindo isolamento estrito e confidencialidade de dados privados-DS, e introduz a disponibilidade de dados por meio do código de apagamento em Kura (armazenamento em bloco) e WSV (World State View).

Le meme depot construiu Iroha 2 (reseaux auto-heberges) e Iroha 3 (SORA Nexus). A execução é garantida pela máquina virtual Iroha (IVM) compartilhada e pela cadeia de ferramentas Kotodama, para que os contratos e artefatos de bytecode permaneçam portáteis entre as implantações de armazenamento automático e o livro-razão global Nexus.

Objetivos
- Um livro-razão lógico global composto por vários validadores cooperantes e espaços de dados.
- Des Data Spaces privados para uma operação autorizada (p. ex., CBDC), com donnees qui ne quittent jamais le DS prive.
- Os Data Spaces são públicos com participação aberta, acesso sem permissão do tipo Ethereum.
- Os contratos inteligentes que podem ser compostos entre os espaços de dados, além das permissões explícitas para acesso a ativos privados-DS.
- Isolamento de desempenho para que a atividade pública não degrade as transações internas privadas-DS.
- Disponibilite des donnees a grande echelle: Kura et WSV com código de apagamento para apoiar os donnees effections illimitees tout en gardant les donnees private-DS privees.

Não-objetivos (fase inicial)
- Definir a economia de token ou as incitações de validadores; As políticas de agendamento e piquetagem são conectáveis.
- Apresentar uma nova versão da ABI; as alterações atuais da ABI v1 com extensões explícitas de syscalls e ponteiro-ABI de acordo com a política IVM.Terminologia
- Nexus Ledger: O ledger lógico global forma composto por blocos de espaço de dados (DS) em uma história ordenada única e um engajamento de estado.
- Espaço de dados (DS): domínio de execução e armazenamento suportado por validadores próprios, governança, classe de confidencialidade, política DA, cotas e política de frais. Duas classes existentes: DS público e DS privado.
- Espaço de dados privado: permissões de validação e controle de acesso; les donnees de transaction et d'etat ne quittent jamais le DS. Seuls des engagements/metadonnees são ancres globalement.
- Espaço Público de Dados: Participação sem autorização; les donnees completes et l'etat sont publics.
- Manifesto de Espaço de Dados (Manifesto DS): Código de manifesto em Norito que declara os parâmetros DS (validadores/cles QC, classe de confidencialidade, política ISI, parâmetros DA, retenção, cotas, política ZK, frais). O hash do manifesto está inserido na cadeia de nexo. Sem substituição, os certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) como esquema de assinatura pós-quantita por padrão.
- Diretório de Espaço: Contrato de diretório global on-chain que rastreia os manifestos DS, versões e eventos de governança/rotação para resolução e auditorias.
- DSID: Identificador global exclusivo para um espaço de dados. Utilize para o namespace de todos os objetos e referências.
- Âncora: Engajamento criptográfico de um bloco/cabeçalho DS incluído no nexo da cadeia para a história do DS no livro-razão global.
- Kura: Estoque de blocos Iroha. Continue aqui com armazenamento de blobs, código de exclusão e compromissos.
- WSV: Iroha Visão do Estado Mundial. Eu estou aqui com versões de segmentos de estado, com instantâneos e códigos por eliminação.
- IVM: Máquina Virtual Iroha para execução de contratos inteligentes (bytecode Kotodama `.to`).
  - AIR: Representação Algébrica Intermediária. Vue algebrique du calcul pour des preuves type STARK, decrivant l'execution como des traces sur des champs com contraintes de transaction et de frontiere.

Modelo de Espaços de Dados
- Identidade: `DataSpaceId (DSID)` identifica um DS e todo o namespace. Les DS podem ter instâncias com dois granulares:
  - Domínio-DS: `ds::domain::<domain_name>` - execução e escopo de um domínio.
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>` - a execução e o escopo do estado têm uma definição de ação exclusiva.
  Les deux formas coexistentes; as transações podem tocar mais DSID de maneira atômica.
- Ciclo de vida do manifesto: criação DS, mises a jour (rotation de cles, changements de politique) et retrait enregistres no Space Directory. Cada artefato DS por slot faz referência ao hash do manifesto mais recente.
- Classes: DS Público (participação ouverte, DA publique) e DS Privado (permissionne, DA confidencial). As políticas híbridas são possíveis através das bandeiras de manifesto.
- Políticas por DS: permissões ISI, parâmetros DA `(k,m)`, chiffrement, retenção, cotas (parte min/max de tx par bloc), política de preuve ZK/optimiste, frais.
- Governança: a governança dos membros DS e a rotação dos validadores são definidas pela seção governança do manifesto (proposições on-chain, multisig, ou governança externa ancree por transações nexo e atestados).

Manifestos de capacidade e UAID
- Contas universais: cada participante recupera um UAID determinado (`UniversalAccountId` e `crates/iroha_data_model/src/nexus/manifest.rs`) que cobre todos os espaços de dados. Os manifestos de capacidade (`AssetPermissionManifest`) referem-se a um UAID em um espaço de dados específico, às épocas de ativação/expiração e uma lista de regras permitir/negar `ManifestEntry` que nasceu `dataspace`, `program_id`, `method`, `asset` e as funções opcionais AMX. Les regles negam todos os dias; O avaliador emet `ManifestVerdict::Denied` com uma razão de auditoria ou uma concessão `Allowed` com os metadados de subsídio correspondentes.
- Subsídios: cada entrada permite o transporte de baldes determinados `AllowanceWindow` (`PerSlot`, `PerMinute`, `PerDay`) mais uma opção `max_amount`. Os hosts e o SDK usam a carga útil do meme Norito, fazendo com que o aplicativo seja idêntico ao hardware e ao SDK.
- Telemetria de auditoria: Diretório Espacial difuso `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` (`crates/iroha_data_model/src/events/data/space_directory.rs`) sempre que houve uma mudança de estado manifesta. A nova superfície `SpaceDirectoryEventFilter` permite que Torii/data-event monitore as mises no dia do manifesto UAID, as revogações e as decisões de negação de vitórias sem encanamento sob medida.Para testar a operação de ponta a ponta, as notas de migração do SDK e as listas de verificação de publicação do manifesto, leia esta seção com o Guia de Conta Universal (`docs/source/universal_accounts_guide.md`). Gardez les dois documentos alinhados quando a política ou as ferramentas do UAID foram alteradas.

Arquitetura de alto nível
1) Sofá de composição global (Cadeia Nexus)
- Mantenha uma ordem canônica única de blocos Nexus de 1 segundo que finaliza transações atômicas cobrindo um ou mais Data Spaces (DS). Cada commit de transação atingiu a atual unificação global do estado mundial (vetor de raízes por DS).
- Contém des metadonnees mínimos mais des preuves/QC agreges para garantir a composabilidade, o final e a detecção de fraude (DSIDs tocam, raízes de estado por DS avant/apres, engajamentos DA, preuves de validite par DS, et le certificado de quorum DS com ML-DSA-87). Aucune donnee privee não está incluído.
- Consenso: comitê BFT global pipeline de taille 22 (3f+1 com f=7), seleção parmi um pool de jusqu'a ~200k validadores potenciais através de um mecanismo VRF/stake por épocas. O comitê sequencia as transações e finaliza o bloco em 1s.

2) Espaço de dados Couche (público/privado)
- Execute os fragmentos por DS de transações globais, com um dia o WSV local do DS e produza artefatos de validade por bloco (previas por DS agregados e compromissos DA) que são montados no bloco Nexus por 1 segundo.
- Les Private DS chiffrent les donnees au repos et en transit entre validadores autorizados; seusls les engagements et preuves de validite PQ quittent le DS.
- Les Public DS exportam les corps completos de donnees (via DA) et les preuves de validite PQ.

3) Transações atômicas entre espaço de dados (AMX)
- Modelo: cada usuário de transação pode tocar mais DS (p. ex., domínio DS e um ou mais ativos DS). Ela está comprometida atomicamente em um bloco Nexus exclusivo ou ela deseja; nenhum efeito parcial.
- Prepare-Commit em 1s: para cada candidato a transação, o DS executa em paralelo com o instantâneo do meme (roots DS na estreia do slot) e produz testes de validação de PQ por DS (FASTPQ-ISI) e compromissos DA. O comitê nexus confirma a transação apenas se todos os pré-requisitos DS forem verificados e se os certificados DA chegarem a tempo (objetivo <=300 ms); Sinon la transaction est replanejada para o slot seguinte.
- Consistência: les ensembles palestra/ecriture sont declara; a detecção de conflitos em vez de confirmar as raízes da estreia do slot. A execução otimizada sem bloqueios por DS evita travamentos globais; l'atomicite é imposta pela regra de commit nexus (tout ou rien entre DS).
- Confidencialidade: o DS Privado exporta exclusivamente pré-compromissos/engajamentos nas raízes do DS pré/pós. Aucune donnee privee brute ne quitte le DS.

4) Disponibilite des donnees (DA) com código de eliminação
- Kura armazena o corpo de blocos e instantâneos WSV como os blobs e um código de apagamento. Os blobs públicos são muito fragmentados; os blobs privados são armazenados exclusivamente nos validadores privados-DS, com pedaços chiffres.
- Os compromissos DA são registrados nos artefatos DS e nos blocos Nexus, permitindo garantias de amostragem e recuperação sem revelação de conteúdo privado.

Estrutura de bloco e commit
- Artefato de teste de espaço de dados (por slot de 1s, por DS)
  - Campeões: dsid, slot, pre_state_root, post_state_root, ds_tx_set_hash, kura_da_commitment, wsv_da_commitment, manifest_hash, ds_qc (ML-DSA-87), ds_validity_proof (FASTPQ-ISI).
  - Les Private-DS exporta artefatos sem corpo de donos; les Public DS permite a recuperação via DA.

- Bloco Nexus (cadência 1s)
  - Campeões: block_number, parent_hash, slot_time, tx_list (transações atômicas entre DS com toques de DSIDs), ds_artifacts[], nexus_qc.
  - Função: finalizar todas as transações atômicas com os artefatos DS requeridos verificient; conheci hoje o vetor de raízes DS do estado mundial global em uma etapa.Consenso e agendamento
- Cadeia Consenso Nexus: pipeline global BFT (classe Sumeragi) com um comitê de 22 noeus (3f+1 com f=7) visualizando blocos de 1s e um final 1s. Os membros do comitê são selecionados por épocas via VRF/stake parmi ~200 mil candidatos; a rotação mantém a descentralização e a resistência à censura.
- Espaço de dados de consenso: cada DS executa seu próprio BFT entre ses validadores para produzir artefatos por slot (preuves, engagements DA, DS QC). Os comitês lane-relay são dimensionados para `3f+1` e usam o parâmetro `fault_tolerance` do espaço de dados e são números de forma determinados por época a partir do pool de validadores do espaço de dados e usam a semente VRF como `(dataspace_id, lane_id)`. Les Private DS são autorizados; les Public DS permite a vivacidade ouverte sous politiques anti-Sybil. O comite global nexus reste inchange.
- Agendamento de transações: os usuários usam transações atômicas declarando os DSIDs tocados e os conjuntos de leitura/escrita. O DS é executado em paralelo no slot; o comite nexus inclui a transação no bloco 1s se todos os artefatos DS forem verificados e se os certificados DA estiverem na hora (<=300 ms).
- Isolamento de desempenho: cada DS em mempools e uma execução independente. As cotas por DS geradas pela combinação de transações que tocam um DS podem ser comprometidas por bloco para evitar o bloqueio de cabeçalho e proteger a latência do DS privado.

Modelo de dados e namespace
- IDs qualificados por DS: todos os entidades (domínios, contas, ativos, funções) são qualificados por `dsid`. Exemplo: `ds::<domain>::account`, `ds::<domain>::asset#precision`.
- Referências globais: uma referência global é uma tupla `(dsid, object_id, version_hint)` e pode ser colocada on-chain no sofá nexus ou nos descritores AMX para uso cross-DS.
- Serialização Norito: todas as mensagens cross-DS (descritores AMX, anteriores) utilizam os codecs Norito. Pas de serde en produção.

Contratos inteligentes e extensões IVM
- Contexto de execução: adicionar `dsid` ao contexto de execução IVM. Os contratos Kotodama são executados continuamente em um espaço de dados específico.
- Primitivos atômicos cross-DS:
  - `amx_begin()` / `amx_commit()` delimita uma transação atômica multi-DS no host IVM.
  - `amx_touch(dsid, key)` declara uma intenção de leitura/gravação para detecção de conflitos contra instantâneos de raízes no slot.
  - `verify_space_proof(dsid, proof, statement)` -> bool
  - `use_asset_handle(handle, op, amount)` -> resultado (a operação é permitida apenas se a política for autorizada e se o identificador for válido)
- Identificadores de ativos e frais:
  - As operações de ação são autorizadas pelas políticas ISI/funções do DS; Os frais são pagos em token de gás do DS. As opções de tokens de capacidade e as políticas mais riquezas (aprovação múltipla, limites de taxa, delimitação geográfica) podem ser adicionadas mais tarde sem alterar o modelo atômico.
- Determinismo: todos os novos syscalls são puros e determinísticos dados as entradas e os conjuntos de leitura/gravação que o AMX declara. Pas d'effets caches de tempo ou ambiente.Preuves de validite post-quantiques (ISI generaliza)
- FASTPQ-ISI (PQ, sem configuração confiável): um argumento baseado em hash que generaliza a transferência de design para todas as famílias ISI e visa um teste no segundo para os lotes na GPU de classe de hardware de 20k.
  - Perfil operacional:
    - As noeuds de produção constroem o teste via `fastpq_prover::Prover::canonical`, que inicializa desormais sempre o backend de produção; le mock determinista ete se aposenta. [crates/fastpq_prover/src/proof.rs:126]
    - `zk.fastpq.execution_mode` (config) e `irohad --fastpq-execution-mode` permitem que os operadores de figura executem CPU/GPU de maneira que determinem que o gancho do observador registre as triplas demandas/resoluções/backend para auditorias de flotte. [crates/iroha_config/src/parameters/user.rs:1357] [crates/irohad/src/main.rs:270] [crates/irohad/src/main.rs:2192] [crates/iroha_telemetry/src/metrics.rs:8887]
- Aritmetização:
  - KV-Update AIR: trate WSV como um tipo de valor-chave de mapa, engajado via Poseidon2-SMT. Cada ISI contém um pequeno conjunto de linhas de leitura-verificação-gravação sobre os itens (comptes, ativos, funções, domínios, metadados, fornecimento).
  - Contraintes gatees par opcode: uma única tabela AIR com colunas selecteurs impõem des regles par ISI (conservação, computadores monotônicos, permissões, verificações de intervalo, erros diários de suporte de metadados).
  - Argumentos de pesquisa: tabelas transparentes envolvidas por hash para permissões/funções, precisões de atividades e parâmetros políticos, evitando restrições bit a bit.
- Engajamentos e mises a jour d'etat:
  - Preuve SMT agregee: todos os itens tocados (pré/pós) são provados contra `old_root`/`new_root` e utilizam uma fronteira compactada com irmãos desduplicados.
  - Invariantes: invariantes globaux (p. ex., fornecimento total por ativo) são impostos via égalité de multiensembles entre linhas de efeito e compteurs suivis.
- Sistema de teste:
  - Engajamentos estilo polinômio FRI (DEEP-FRI) com forte arite (8/16) e blow-up 8-16; hashes Poseidon2; transcrição Fiat-Shamir com SHA-2/3.
  - Opções de recursão: agregação de localidade recursiva DS para compactar microlotes em um teste por slot, se necessário.
- Portée et exemples couverts:
  - Ativos: transferir, cunhar, queimar, registrar/cancelar registro de definições de ativos, definir precisão (transportada), definir metadados.
  - Comptes/Domains: criar/remover, definir chave/limite, adicionar/remover signatários (etat uniquement; as verificações de assinaturas são atestadas pelos validadores DS, não provadas no AIR).
  - Funções/Permissões (ISI): conceder/revogar funções e permissões; impõe através de tabelas de pesquisa e verificações de política monotônica.
  - Contratos/AMX: marqueurs iniciam/confirmam AMX, capacidade mint/revoke si active; prova como transições de estado e compteurs de politique.
- Verifica fora do AIR para preservar a latência:
  - Assinaturas e criptografia de alta qualidade (p. ex., assinaturas utilizadas pelo usuário ML-DSA) são verificadas pelos validadores DS e atestados no DS QC; a preuve de validite cobre apenas a coerência de estado e a conformidade política. Cela garde des preuves PQ et rapides.
- Cibles de performance (ilustrativo, CPU 32 núcleos + uma GPU moderna):
  - 20k ISI mixagens com key-touch petit (<=8 células/ISI): ~0,4-0,9 s de teste, ~150-450 KB de teste, ~5-15 ms de verificação.
  - ISI plus lourdes (mais riquezas de cles/contraintes): microlote (p. ex., 10x2k) + recursão para garder <1 s par slot.
- Configuração do Manifesto DS:
  -`zk.policy = "fastpq_isi"`
  -`zk.hash = "poseidon2"`, `zk.fri = { blowup: 8|16, arity: 8|16 }`
  -`state.commitment = "smt_poseidon2"`
  -`zk.recursion = { none | local }`
  - `attestation.signatures_in_proof = false` (assinaturas verificadas por DS QC)
  - `attestation.qc_signature = "ml_dsa_87"` (par padrão; alternativas devem ser declaradas explicitamente)
- Subsídios:
  - Complexos/personalizados ISI podem usar um STARK geral (`zk.policy = "stark_fri_general"`) com pré-diferenciação e finalização de 1 s via atestado QC + corte em testes inválidos.
  - Opções não PQ (p. ex., Plonk com KZG) exigem configuração confiável e não são mais suportadas na compilação por padrão.Introdução AR (para Nexus)
- Traço de execução: matriz com grandes (colunas de registro) e longo (etapas). Cada linha é uma etapa lógica do tratamento ISI; as colunas contêm valores pré/pós, seletores e bandeiras.
- Contra-indicações:
  - Contraintes de transição: impõe relações de linha a linha (p. ex., post_balance = pre_balance - valor para uma linha de débito quando `sel_transfer = 1`).
  - Contraintes de frontiere: lient I/O public (old_root/new_root, compteurs) a la premier/derniere ligne.
  - Pesquisas/permutações: garantem a aparência e a igualdade de multiensembles contra tabelas engajadas (permissões, parâmetros de ação) sem circuitos altos de bits.
- Engajamento e verificação:
  - Prove envolver os rastreamentos por meio de codificações hash e construa os polinômios com um grau fraco de validade se houver restrições.
  - O verificador verifica o grau de falha via FRI (baseado em hash, pós-quantique) com algumas aberturas Merkle; le cout é logarítmico em etapas.
- Exemplo (Transferência): os registros incluem pre_balance, amount, post_balance, nonce et selecteurs. As contraintes impostas não-negatividade/intervalo, conservação e monotonicidade de nonce, tandis qu'une multi-preuve SMT agregam mentiras les feuilles pré/pós aux raízes antigas/novas.

Evolution ABI e syscalls (ABI v1)
- Syscalls um ajouter (noms illustratifs):
  -`SYS_AMX_BEGIN`, `SYS_AMX_TOUCH`, `SYS_AMX_COMMIT`, `SYS_VERIFY_SPACE_PROOF`, `SYS_USE_ASSET_HANDLE`.
- Tipos pointer-ABI e ajouter:
  -`PointerType::DataSpaceId`, `PointerType::AmxDescriptor`, `PointerType::AssetHandle`, `PointerType::ProofBlob`.
- Mises a jour exige:
  - Adicionar a `ivm::syscalls::abi_syscall_list()` (garder l'ordre), portão por política.
  - Mapeie números desconhecidos para `VMError::UnknownSyscall` nos hosts.
  - Faça alguns testes: syscall list golden, ABI hash, ponteiro tipo ID goldens, testes de política.
  - Documentos: `crates/ivm/docs/syscalls.md`, `status.md`, `roadmap.md`.

Modelo de confidencialidade
- Contenção de donnees privees: les corps de transaction, diffs d'etat et snapshots WSV des private DS ne quittent jamais le sub-conjunto de validadores privados.
- Exposição pública: seus cabeçalhos, compromissos DA e preuves de validite PQ são exportações.
- Preuves ZK optionnelles: les Private DS pode produzir des preuves ZK (p. ex., solde suffisant, politique satisfaite) permitindo ações entre DS sem revelar o estado interno.
- Controle de acesso: a autorização é imposta pelas políticas ISI/funções no DS. Os tokens de capacidade são opcionais e podem ser adicionados mais tarde.

Isolamento de desempenho e QoS
- Consenso, mempools e stockage separa por DS.
- Cotas de agendamento de nexo por DS para suportar o tempo de inclusão de âncoras e evitar o bloqueio de cabeçalho.
- Orçamentos de recursos de contratos por DS (computação/memória/IO), impostos por host IVM. A contenção do DS público não pode consumir os orçamentos do DS privado.
- Chama cross-DS assíncronos evitando longos períodos de sincronização na execução do private-DS.

Disponibilite des donnees et design de stockage
1) Código de eliminação
- Use a sistemática Reed-Solomon (p. ex., GF (2 ^ 16)) para apagar o código no nível do blob de blocos Kura e instantâneos WSV: parâmetros `(k, m)` com fragmentos `n = k + m`.
- Parâmetros padrão (proposta, DS público): `k=32, m=16` (n=48), permitindo a recuperação de apenas 16 fragmentos perdidos com ~1,5x de expansão. Para DS privado: `k=16, m=8` (n=24) na permissão do conjunto. Os dois são configuráveis ​​pelo DS Manifest.
- Blobs públicos: os shards são distribuídos por meio de nobreux noeuds DA/validateurs com verificações de disponibilidade por amostragem. Os compromissos DA nos cabeçalhos permitem a verificação por clientes leves.
- Blobs privados: shards chiffres e distribues unicamente entre validadores privados-DS (ou custodiantes designados). A cadeia global não é compatível com compromissos DA (sem colocação de fragmentos de nicles).

2) Engajamentos e amostragem
- Para cada blob: calcule uma linha Merkle nos shards e inclua-a em `*_da_commitment`. Restaure o PQ e evite os compromissos com um curso elíptico.
- Atestadores DA: atestadores regionais de echantillonnes par VRF (p. ex., 64 por região) emitem um certificado ML-DSA-87 atestam uma amostragem reutilizável de shards. Objeto de latência de atestado DA <=300 ms. O comite nexus valida os certificados em vez de retirar os shards.3) Integração Kura
- Os blocos armazenam o corpo de transações como blobs e código de apagamento com compromissos de Merkle.
- Os cabeçalhos pressagiam engajamentos de blob; les corps são recuperáveis ​​via le reseau DA para DS público e via des canaux prives para DS privado.

4) Integração WSV
- Snapshots WSV: periodicidade, ponto de verificação do estado DS e pedaços de snapshots e códigos apagados com compromissos registrados nos cabeçalhos. Entre os snapshots, os logs de alterações são mantidos. Os instantâneos públicos são fragmentos maiores; os instantâneos privados permanecem nos validadores privados.
- Acces porteur de preuves: les contrats peuvent fournir (ou demander) des preuves d'etat (Merkle/Verkle) ancrees par des engagements de snapshot. Les Private DS pode fornecer atestados de conhecimento zero em vez de testes brutos.

5) Retenção e poda
- Passos de poda para DS público: conservar todo o corpo Kura e instantâneos WSV via DA (escalabilidade horizontal). Les Private DS podem definir uma retenção interna, mas os compromissos de exportação permanecem imuáveis. La couche nexus conserva todos os blocos Nexus e os compromissos dos artefatos DS.

Reseau e papéis de noeuds
- Validadores globais: participantes do nexo de consenso, validam os blocos Nexus e os artefatos DS, efetuam verificações DA para DS público.
- Validadores de espaço de dados: executa o consenso DS, executa os contratos, gera Kura/WSV local, gera DA para seu DS.
- Noeuds DA (opcional): armazena/publica blobs públicos, facilita a amostragem. Para DS privado, os noeuds DA são co-localizados com validadores ou custodiantes de confiança.

Melhorias e considerações do sistema
- Sequenciamento de desacoplamento/mempool: adota um DAG de mempool (p. ex., estilo Narwhal) alimentando um pipeline BFT no sofá nexus para reduzir a latência e melhorar o rendimento sem alterar o modelo lógico.
- Cotas DS e justiça: cotas por DS por bloco e limites máximos de peso para evitar o bloqueio de cabeçalho e garantir uma latência previsível para DS privado.
- Atestado DS (PQ): os certificados de quorum DS usam ML-DSA-87 (classe Dilithium5) por padrão. É pós-quantique e mais volumoso que as assinaturas EC mais aceitáveis ​​em um slot de QC por par. O DS pode explicitamente optar pelo ML-DSA-65/44 (mais petits) ou pelas assinaturas CE que você declara no Manifesto do DS; les Public DS é fortement incentiva um jardineiro ML-DSA-87.
- Atestadores DA: para DS público, utilize os atestadores regionais por VRF que emitem certificados DA. O comite nexus valida os certificados em vez da amostragem bruta dos shards; les Private DS jardinaram les atestados DA internos.
- Recursão e testes por época: opção de agregar mais microlotes em um DS em um teste recursivo por slot/época para garantir o máximo de testes e tempos de verificação estáveis ​​sob carga elevada.
- Escalonamento de pistas (se necessário): se um comitê global único tiver um resultado, introduza K pistas de sequenciamento paralelas com uma fusão determinada. Cela preservará uma ordem global única em horizontalidade crescente.
- Determinação de aceleração: fornece kernels SIMD/CUDA gates par feature para hashing/FFT com um CPU substituto bit-exato para preservar o determinismo entre hardware.
- Seusils d'activation des lanes (proposição): activer 2-4 lanes si (a) la finalite p95 depasse 1,2 s pendente >3 minutos consecutivos, ou (b) l'ocupation par bloc depasse 85% pendente >5 minutos, ou (c) le debit entrant de tx requerrait >1,2x la capacite de bloco a des niveaux soutenus. As faixas agrupam as transações de maneira determinada pelo hash do DSID e se fundem no bloco Nexus.

Frais et economie (valores iniciais)
- Unidade de gás: token de gás por DS com medidores de computação/IO; les frais sont payes en actif de gas natif du DS. A conversão entre DS é um aplicativo de preocupação.
- Prioridade de inclusão: round-robin entre DS com cotas por DS para preservar a justiça e os SLOs 1s; em um DS, a mise antes das taxas pode partir.
- Futuro: uma marcha global de taxas ou de políticas minimizadas MEV pode ser explorada sem alterar a atomicidade e o design de preuves PQ.Fluxo de trabalho entre espaços de dados (exemplo)
1) Um usuário usa uma transação AMX tocando um DS público P e um DS S privado: substitui o ativo X de S para o beneficiário B, não a conta está em P.
2) No slot, P et S executam seu fragmento contra o instantâneo do slot. Verifique a autorização e a disponibilidade, se o dia estiver em estado interno, e produza uma tentativa de validação de PQ e um noivado DA (nenhum privilégio não foi vazado). P prepare la mise a jour d'etat correspondente (p. ex., mint/burn/locking dans P selon la politique) et sa preuve.
3) Le comite nexus verifica os dois preuves DS e os certificados DA; se os dois forem verificados no slot, a transação for confirmada atomicamente no bloco Nexus de 1s, colocando um dia as duas raízes DS no vetor estado mundial global.
4) Se uma verificação ou um certificado DA for inválido/inválido, a transação será cancelada (algum efeito) e o cliente poderá ser reiniciado para o slot seguinte. Aucune donnee privee ne quitte S a aucun moment.

- Considerações de segurança
- Execução determinista: os syscalls IVM restantes deterministas; Os resultados do cross-DS são ditados pelo commit e final do AMX, não pelo relógio ou pelo registro de tempo.
- Controle de acesso: as permissões ISI no DS privado são restritas e podem ser submetidas a transações e operações autorizadas. Os tokens de capacidade codificam os direitos para uso entre DS.
- Confidencialidade: confidencialidade de ponta a ponta para dados privados-DS, fragmentos e código de apagamento armazenados exclusivamente entre membros autorizados, pré-opções ZK para atestados externos.
- Resistance DoS: o isolamento do nível mempool/consensus/stockage impede o congestionamento público de impactar a progressão do DS privado.

Alterações dos componentes Iroha
- iroha_data_model: introdução `DataSpaceId`, IDs qualificados por DS, descritores AMX (conjuntos palestra/escrita), tipos de preuve/engagement DA. Serialização Norito exclusiva.
- ivm: adiciona syscalls e tipos de ponteiro-ABI para AMX (`amx_begin`, `amx_commit`, `amx_touch`) e testa DA; leia os testes/documentos ABI de acordo com a política v1.