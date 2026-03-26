---
lang: pt
direction: ltr
source: docs/source/iroha_2_whitepaper.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a4e8824c128b9f2a34262a5c9bc09f6b2cd790a0561aa083fa18a987accd7004
source_last_modified: "2026-01-22T15:59:09.647697+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#Iroha v2.0

Hyperledger Iroha v2 é um livro-razão distribuído determinístico e tolerante a falhas bizantinas que enfatiza um
arquitetura modular, padrões fortes e APIs acessíveis. A plataforma é enviada como um conjunto de caixas de ferrugem
que podem ser incorporados em implantações personalizadas ou usados em conjunto para operar uma rede blockchain de produção.

---

## 1. Visão geral

Iroha 2 continua a filosofia de design introduzida com Iroha 1: fornecer uma coleção com curadoria de
recursos prontos para uso para que as operadoras possam montar uma rede sem escrever grandes quantidades de
código. A versão v2 consolida o ambiente de execução, o pipeline de consenso e o modelo de dados em um
único espaço de trabalho coeso.

A linha v2 é voltada para organizações que desejam operar seus próprios permissionados ou consorciados
blockchains. Cada implantação executa sua própria rede de consenso, mantém governança independente e pode personalizar
configuração, dados de gênese e cadência de atualização sem depender de terceiros. O espaço de trabalho compartilhado
permite que várias redes independentes sejam construídas exatamente na mesma base de código enquanto escolhe os recursos e
políticas que correspondam aos seus casos de uso.

Tanto Iroha 2 quanto SORA Nexus (Iroha 3) executam a mesma máquina virtual Iroha (IVM). Os desenvolvedores podem criar Kotodama
contrata uma vez e os implanta em redes auto-hospedadas ou no livro-razão global Nexus sem recompilar ou
bifurcando o ambiente de execução.
### 1.1 Relacionamento com o ecossistema Hyperledger

Os componentes Iroha são projetados para interoperar com outros projetos Hyperledger. Consenso, modelo de dados e
as caixas de serialização podem ser reutilizadas em pilhas compostas ou junto com implantações Fabric, Sawtooth e Besu.
Ferramentas comuns – como codecs Norito e manifestos de governança – ajudam a manter as interfaces consistentes em todo o
ecossistema enquanto permite que Iroha forneça uma implementação padrão opinativa.

### 1.2 Bibliotecas cliente e SDKs

Para garantir experiências móveis e web de primeira classe, o projeto publica SDKs mantidos:

- `IrohaSwift` para clientes iOS e macOS, integrando aceleração Metal/NEON por trás de substitutos determinísticos.
- `iroha_js` para aplicativos JavaScript e TypeScript, incluindo construtores Kaigi e auxiliares Norito.
- `iroha_python` para integrações Python, com suporte a HTTP, WebSocket e telemetria.
- `iroha_cli` para administração e scripts orientados por terminal.

linguagens e plataformas.

### 1.3 Princípios de design- **Determinismo primeiro:** Cada nó executa os mesmos caminhos de código e produz os mesmos resultados com o mesmo
  entradas. Os caminhos SIMD/CUDA/NEON são controlados por recursos e recorrem a implementações escalares determinísticas.
**Módulos combináveis:** Rede, consenso, execução, telemetria e armazenamento, cada um ao vivo em módulos dedicados
  caixas para que os incorporadores possam adotar subconjuntos sem carregar a pilha inteira.
- **Configuração explícita:** Os botões comportamentais são exibidos por meio de `iroha_config`; alternadores de ambiente são
  limitado às conveniências do desenvolvedor.
- **Padrões seguros:** Codecs canônicos, aplicação estrita de ABI de ponteiro e manifestos versionados tornam
  atualizações entre redes previsíveis.

## 2. Arquitetura da plataforma

### 2.1 Composição do nó

Um nó Iroha executa vários serviços cooperantes:

- **Torii (`iroha_torii`)** expõe APIs HTTP/WebSocket para transações, consultas, eventos de streaming e
  telemetria (pontos finais `/v1/...`).
- **Core (`iroha_core`)** coordena validação, consenso, execução, governança e gestão de estado.
- **Sumeragi (`iroha_core::sumeragi`)** implementa o pipeline de consenso pronto para NPoS com alterações de visualização,
  disponibilidade confiável de dados de transmissão e certificados de confirmação. Veja o
  [Guia de consenso Sumeragi](./sumeragi.md) para obter detalhes.
- **Kura (`iroha_core::kura`)** persiste blocos canônicos, sidecars de recuperação e metadados de testemunha no disco.
- **World State View (`iroha_core::state`)** armazena o instantâneo oficial na memória usado para validação
  e consultas.
- **Máquina virtual Iroha (`ivm`)** executa o bytecode Kotodama (`.to`) e impõe a política ABI do ponteiro.
- **Norito (`crates/norito`)** fornece serialização binária e JSON determinística para cada tipo on-wire.
- **Telemetria (`iroha_telemetry`)** exporta métricas Prometheus, registro estruturado e eventos de streaming.
- **P2P (`iroha_p2p`)** gerencia fofoca, topologia e conexões seguras entre pares.

### 2.2 Rede e topologia

Os pares Iroha mantêm uma topologia ordenada derivada do estado confirmado. Cada rodada de consenso seleciona um líder,
validadores de conjunto de validação, cauda de proxy e conjunto B. As transações são fofocadas usando mensagens codificadas em Norito
antes que o líder os agrupe em uma proposta. A transmissão confiável garante que bloqueie e suporte
as evidências chegam a todos os pares honestos, garantindo a disponibilidade de dados mesmo em situações de turbulência na rede. Ver alterações girar
liderança quando os prazos são perdidos e os certificados de commit garantem que cada bloco commitado carregue o
conjunto de assinatura canônica usado por todos os pares.

### 2.3 Criptografia

A caixa `iroha_crypto` permite gerenciamento de chaves, hashing e verificação de assinatura:- Ed25519 é o esquema de chave do validador padrão.
- Os back-ends opcionais incluem Secp256k1, TC26 GOST, BLS (para atestados agregados) e auxiliares ML-DSA.
- Canais de streaming emparelham identidades Ed25519 com HPKE baseado em Kyber para proteger sessões de streaming Norito.
- Todas as rotinas de hashing usam implementações determinísticas (SHA-2, SHA-3, Blake2, Poseidon2) com espaço de trabalho
  auditorias documentadas em `docs/source/crypto/dependency_audits.md`.

### 2.4 Streaming e pontes de aplicativos

- **Streaming Norito (`iroha_core::streaming`, `norito::streaming`)** fornece mídia criptografada e determinística
  e canais de dados com instantâneos de sessão, rotação de chaves HPKE e ganchos de telemetria. Conferência Kaigi e
  transferências de evidências confidenciais usam esta via.
- **Connect bridge (`connect_norito_bridge`)** expõe uma superfície C ABI que alimenta SDKs de plataforma
  (Swift, Kotlin/Android) enquanto reutiliza os clientes Rust nos bastidores.
- **Ponte ISO 20022 (`iroha_torii::iso20022_bridge`)** converte mensagens de pagamento regulamentadas em Norito
  transações, permitindo a interoperabilidade com fluxos de trabalho financeiros sem ignorar o consenso ou a validação.
- Todas as pontes preservam cargas úteis Norito determinísticas para que os sistemas downstream possam verificar as transições de estado.

## 3. Modelo de dados

A caixa `iroha_data_model` define todos os objetos contábeis, instruções, consultas e eventos. Destaques:

- **Domains, accounts, and assets** use canonical Katakana i105 account ids and canonical Base58 asset ids. Account aliases are separate on-chain
  bindings in `name@dataspace` / `name@domain.dataspace` form that resolve to Katakana i105 account ids, and asset aliases are separate on-chain bindings in `name#dataspace` / `name#domain.dataspace` form that resolve to canonical Base58 asset ids. Metadata is deterministic (`Metadata` map). Numeric assets support fixed-point
  operations; NFTs carry arbitrary structured metadata.

- **Funções e permissões** usam tokens enumerados por Norito que mapeiam diretamente para verificações do executor.
- **Gatilhos** (baseados em tempo, baseados em blocos ou orientados por predicados) emitem transações determinísticas por meio da cadeia
  executor.
- **Eventos** são transmitidos via Torii e espelham transições de estado confirmadas, incluindo fluxos confidenciais e
  ações de governança.
- **Transações, blocos e manifestos** são codificados em Norito (`SignedTransaction`, `SignedBlockWire`) com
  cabeçalhos de versão explícitos, garantindo decodificação extensível para frente.
- **A customização** acontece através do modelo de dados do executor: os operadores podem cadastrar instruções customizadas,
  permissões e parâmetros, preservando o determinismo.
- **Repositórios (`RepoInstruction`)** permitem agrupar planos de atualização determinísticos (executores, manifestos e
  ativos) para que implementações em várias etapas possam ser gerenciadas na cadeia com aprovação de governança.
- **Artefatos de consenso** — como certificados de confirmação e listas de testemunhas — residem no modelo de dados e
  faça uma viagem de ida e volta através de testes de ouro para garantir a compatibilidade entre `iroha_core`, Torii e SDKs.
- **Registros e eventos confidenciais** capturam descritores de ativos protegidos, chaves de verificador, compromissos,
  anuladores e cargas úteis de eventos (`ConfidentialEvent::{Shielded,Transferred,Unshielded}`) para fluxos confidenciais
  permanecem auditáveis sem vazar dados de texto simples.

## 4. Ciclo de vida da transação1. **Admissão:** Torii decodifica a carga útil Norito, verifica assinaturas, TTL e limites de tamanho e, em seguida, enfileira o
   transação localmente.
2. **Fofoca:** A transação se propaga pela topologia; pares desduplicam por hash e repetem a admissão
   verificações.
3. **Seleção:** O líder atual extrai transações do conjunto pendente e realiza validação sem estado.
4. **Simulação com estado:** As transações candidatas são executadas dentro de um `StateBlock` transitório, invocando IVM ou
   instruções integradas. Conflitos ou violações de regras são eliminados de forma determinística.
5. **Materialização de gatilhos:** Os gatilhos programados devidos na rodada são convertidos em transações internas
   e validado usando o mesmo pipeline.
6. **Selagem da proposta:** Quando os limites do bloco são atingidos ou os tempos limite expiram, o líder emite um sinal codificado Norito
   Mensagem `BlockCreated`.
7. **Validação:** Os pares no conjunto de validação executam novamente verificações sem estado/com estado. Sinal de pares bem sucedidos
   Mensagens `BlockSigned` e encaminhá-las para o conjunto de coletores determinísticos.
8. **Commit:** Um coletor monta um certificado de commit depois de coletar o conjunto de assinaturas canônicas,
   transmite `BlockCommitted` e finaliza o bloco localmente.
9. **Aplicação:** Todos os pares registram o bloco no Kura, aplicam atualizações de estado, emitem telemetria/eventos, eliminam
   transações confirmadas do mempool e alternar funções de topologia.

Os caminhos de recuperação usam transmissão determinística para retransmitir blocos ausentes e visualizar as alterações alternando a liderança
quando os prazos expiram. Sidecars e telemetria fornecem insights de diagnóstico sem alterar os resultados de consenso.

## 5. Contratos inteligentes e execução

Os contratos inteligentes são executados na máquina virtual Iroha (IVM):

- **Kotodama** compila fontes `.ko` de alto nível em bytecode `.to` determinístico.
- **Aplicação de ABI de ponteiro** garante que os contratos interajam com a memória do host por meio de tipos de ponteiro validados.
  As superfícies Syscall são descritas em `ivm/docs/syscalls.md`; a lista ABI é hash e versionada.
- **Syscalls e hosts** cobrem acesso ao estado do razão, agendamento de gatilhos, primitivas confidenciais, mídia Kaigi
  fluxos e aleatoriedade determinística.
- **Executor integrado** continua a oferecer suporte a Instruções Especiais (ISI) Iroha para ativos, contas, permissões,
  e operações de governança. Executores personalizados podem estender o conjunto de instruções enquanto respeitam os esquemas Norito.
- **Recursos confidenciais** — incluindo transferências protegidas e registros de verificador — são expostos via executor
  instruções e validadas por anfitriões com compromissos Poseidon.

## 6. Armazenamento e persistência- **Kura block store** grava cada bloco finalizado como uma carga útil `SignedBlockWire` com um cabeçalho Norito, mantendo
  cabeçalhos canônicos, transações, certificados de confirmação e dados de testemunha juntos.
- **World State View** mantém o estado oficial na memória para consultas rápidas. Instantâneos determinísticos e
  sidecars de pipeline (`pipeline/sidecars.norito` + `pipeline/sidecars.index`) suportam recuperação e auditorias.
- **A classificação por níveis de estado** permite o particionamento quente/frio para grandes implantações, preservando a capacidade determinística
  validação.
- **Sincronizar e reproduzir** carrega blocos confirmados de volta ao estado usando as mesmas regras de validação. Determinístico
  a transmissão garante que os pares possam recuperar dados perdidos dos vizinhos sem depender de armazenamento confiável.

## 7. Governança e economia

- Parâmetros on-chain (`SetParameter`) controlam temporizadores de consenso, limites de mempool, botões de telemetria, faixas de taxas,
  e sinalizadores de recursos. Os manifestos Genesis gerados por `kagami` instalam a configuração inicial.
- As instruções **Kaigi** gerenciam sessões colaborativas (criar/entrar/sair/encerrar) e alimentar streaming Norito
  telemetria para casos de uso de conferência.
- **Hijiri** fornece reputação determinística de pares e contas, integrando-se com consenso, admissão
  políticas e multiplicadores de taxas (matemática de ponto fixo Q16). Manifestos de evidências, pontos de verificação e reputação
  os registros são comprometidos na cadeia e os perfis dos observadores regem a proveniência do recebimento.
- **Modo NPoS** (quando ativado) usa janelas eleitorais apoiadas por VRF e comitês ponderados por estacas, preservando
  padrões de configuração determinísticos.
- **Registros confidenciais** regem chaves de verificação de conhecimento zero, ciclos de vida de prova e compromissos para
  fluxos blindados.

## 8. Experiência do cliente e ferramentas

- **API Torii** oferece interfaces REST e WebSocket para transações, consultas, fluxos de eventos, telemetria e
  pontos finais de governação. As projeções JSON são derivadas de esquemas Norito.
- **Ferramentas CLI** (`iroha_cli`, `iroha_monitor`) abrangem administração, painéis de pares ativos e pipeline
  inspeção.
- **Ferramentas Genesis** (`kagami`) gera manifestos codificados em Norito, material de chave do validador e configuração
  modelos.
- **SDKs** (Swift, JS/TS, Python) fornecem acesso idiomático a instruções, consultas, gatilhos e telemetria.
- **Scripts e ganchos CI** dentro do `scripts/` automatizam a validação do painel, a regeneração do codec e a fumaça
  testes.

## 9. Desempenho, resiliência e roteiro- O pipeline atual tem como meta **20.000 tps** com tempos de bloqueio de **2–3 segundos** em rede favorável
  condições, apoiadas por verificação de assinatura de lote e programação determinística.
- **Telemetria** expõe métricas Prometheus para temporizadores de consenso, ocupação de mempool, integridade de propagação de bloco,
  Uso de Kaigi e atualizações de reputação de Hijiri.
- **Recursos de resiliência** incluem disponibilidade determinística de dados, sidecars de recuperação, rotação de topologia e
  limites de visualização/alteração configuráveis.
- Os marcos futuros do roteiro (consulte `roadmap.md`) continuam a trabalhar nos espaços de dados Nexus, com confidencialidade aprimorada
  ferramentas e aceleração de hardware mais ampla, preservando resultados determinísticos.

## 10. Operações e implantação

- **Artefatos:** fluxos de trabalho Dockerfiles, Nix flake e `cargo` suportam compilações reproduzíveis. `kagami` emite
  manifestos do genesis, chaves de validação e configurações de exemplo para implantações com permissão e NPoS.
- **Redes auto-hospedadas:** as operadoras gerenciam seus próprios conjuntos de pares, regras de admissão e cadência de atualização. O
  O espaço de trabalho suporta muitas redes Iroha 2 independentes coexistindo sem coordenação, compartilhando apenas o
  código ascendente.
- **Ciclo de vida da configuração:** `iroha_config` resolve usuário → real → camadas padrão, garantindo que cada botão seja
  explícito e controlado por versão. As alterações no tempo de execução fluem através das instruções `SetParameter`.
- **Observabilidade:** `iroha_telemetry` exporta métricas Prometheus, logs estruturados e dados de painel verificados
  por scripts CI (`ci/check_swift_dashboards.sh`, `scripts/render_swift_dashboards.sh`,
  `scripts/check_swift_dashboard_data.py`). Eventos de streaming, consenso e Hijiri estão disponíveis em
  WebSocket e `scripts/sumeragi_backpressure_log_scraper.py` correlacionam a contrapressão do marcapasso com
  telemetria para solução de problemas.
- **Testes:** `cargo test --workspace`, testes de integração (`integration_tests/`), conjuntos de SDK de linguagem e
  As luminárias douradas Norito protegem o determinismo. Pointer ABI, listas de syscall e manifestos de governança têm
  testes de ouro dedicados.
- **Recuperação:** sidecars Kura, reprodução determinística e sincronização de transmissão permitem que os nós recuperem o estado do disco
  ou colegas. Os pontos de verificação e manifestos de governança Hijiri fornecem instantâneos auditáveis ​​para conformidade.

# Glossário

Para a terminologia referenciada neste documento, consulte o glossário de todo o projeto em
.