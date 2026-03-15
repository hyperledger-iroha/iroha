---
lang: pt
direction: ltr
source: docs/source/global_feature_matrix.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6a406b7656a87bb1469444db1cc2d2d5922f16660b53cc7eaef5b838199127e8
source_last_modified: "2026-01-23T20:16:38.056405+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Matriz Global de Recursos

Legenda: `◉` totalmente implementado · `○` praticamente implementado · `▲` parcialmente implementado · Implementação de `△` recém iniciada · `✖︎` não iniciada

## Consenso e networking

| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Suporte K/r para vários coletores e ganhos de certificado de primeiro commit | ◉ | Seleção determinística de coletor, distribuição redundante, parâmetros K/r na cadeia e aceitação do primeiro certificado de confirmação válido enviado com testes. | status.md:255; status.md:314 |
| Backoff do marcapasso, piso RTT, jitter determinístico | ◉ | Temporizadores configuráveis ​​com banda de jitter conectados por meio de configuração, telemetria e documentos. | status.md:251 |
| NEW_VIEW controle de controle de qualidade e maior controle de qualidade | ◉ | O fluxo de controle carrega NEW_VIEW/Evidence, o QC mais alto adota monotonicamente, o aperto de mão protege a impressão digital computada. | status.md:210 |
| rastreamento de evidências de disponibilidade (consultivo) | ◉ | Evidência de disponibilidade emitida e rastreada; commit não determina a disponibilidade na v1. | status.md:mais recente |
| Transmissão confiável (transporte de carga útil DA) | ◉ | O fluxo de mensagens RBC (Init/Chunk/Ready/Deliver) é ativado quando `da_enabled=true` como um caminho de transporte/recuperação; a evidência de disponibilidade é rastreada (aconselhamento) enquanto o commit prossegue de forma independente. | status.md:mais recente |
| Confirmar ligação de raiz de estado QC | ◉ | Confirmar QCs carregam `parent_state_root`/`post_state_root`; não há porta de controle de qualidade de execução separada. | status.md:mais recente |
| Propagação de evidências e endpoints de auditoria | ◉ | ControlFlow::Evidence, endpoints de evidência Torii e testes negativos foram obtidos. | status.md:176; status.md:760-761 |
| Telemetria RBC, métricas de prontidão/entregues | ◉ | Endpoints `/v2/sumeragi/rbc*` e contadores/histograma de telemetria disponíveis para operadores. | status.md:283-284; status.md:772 |
| Anúncio de parâmetro de consenso e verificação de topologia | ◉ | Os nós transmitem `(collectors_k, redundant_send_r)` e validam a igualdade entre pares. | status.md:255 |
| Rotação permitida baseada em PRF | ◉ | A seleção de líder/coletor permitido usa semente PRF + altura/visão sobre a lista canônica; a rotação prev-hash continua sendo um auxiliar legado. | status.md:mais recente |

## Pipeline, Kura e Estado| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Limites de pista de quarentena e telemetria | ◉ | Botões de configuração, manipulação determinística de overflow e contadores de telemetria implementados. | status.md:263 |
| Botão de pool de trabalhadores de pipeline | ◉ | `[pipeline].workers` encadeado por meio de inicialização de estado com testes de análise de ambiente. | status.md:264 |
| Faixa de consulta de instantâneo (cursores armazenados/efêmeros) | ◉ | Modo cursor armazenado com integração Torii e bloqueio de pools de trabalhadores. | status.md:265; status.md:371; status.md:501 |
| Sidecars estáticos de recuperação de impressão digital DAG | ◉ | Sidecars armazenados no Kura, validados na inicialização, avisos emitidos em caso de incompatibilidades. | status.md:106; status.md:349 |
| Endurecimento de decodificação de hash da loja de blocos Kura | ◉ | As leituras de hash mudaram para manipulação bruta de 32 bytes com testes de ida e volta independentes de Norito. | status.md:608; status.md:668 |
| Telemetria adaptativa Norito para codecs | ◉ | Métricas de seleção AoS vs NCB adicionadas a Norito. | status.md:156 |
| Instantâneo de consultas WSV via Torii | ◉ | A pista de consulta de snapshot Torii usa pool de trabalhadores de bloqueio e semântica determinística. | status.md:501 |
| Acionar encadeamento de execução por chamada | ◉ | Cadeia de gatilhos de dados imediatamente após a execução da chamada com ordem determinística. | status.md:668 |

## Norito Serialização e ferramentas

| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Migração JSON Norito (área de trabalho) | ◉ | Serde retirado da produção; inventário + guarda-corpos mantêm o espaço de trabalho apenas Norito. | status.md:112; status.md:124 |
| Lista de negações de Serde e proteções de CI | ◉ | Os fluxos de trabalho/scripts do Guard evitam o novo uso direto do Serde no espaço de trabalho. | status.md:218 |
| Norito codec goldens e testes AoS/NCB | ◉ | Goldens AoS/NCB, testes de truncamento e sincronização de documentos adicionados. | status.md:140-147; status.md:149-150; status.md:332; status.md:666 |
| Ferramentas de matriz de recursos Norito | ◉ | `scripts/run_norito_feature_matrix.sh` suporta testes de fumaça downstream; CI cobre combos pack-seq/struct. | status.md:146; status.md:152 |
| Ligações de linguagem Norito (Python/Java) | ◉ | Codecs Python e Java Norito mantidos com scripts de sincronização. | status.md:74; status.md:81 |
| Norito Classificadores estruturais SIMD Estágio 1 | ◉ | Classificadores NEON/AVX2 estágio 1 com cross-arch goldens e testes de corpora randomizados. | status.md:241 |

## Governança e atualizações de tempo de execução| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Admissão de atualização em tempo de execução (gate ABI) | ◉ | Conjunto de ABI ativo aplicado na admissão com erros e testes estruturados. | status.md:196 |
| Controle de implantação de namespace protegido | ▲ | Implantar requisitos de metadados e portas conectadas; política/UX ainda em evolução. | status.md:171 |
| Terminais de leitura de governança Torii | ◉ | `/v2/gov/*` lê APIs roteadas com testes de roteador. | status.md:212 |
| Ciclo de vida e eventos do registro da chave de verificação | ◉ | Registro/atualização/descontinuação de VK, eventos, filtros CLI e semântica de retenção implementados. | status.md:236-239; status.md:595; status.md:603 |

## Infraestrutura de conhecimento zero

| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| APIs de armazenamento de anexos | ◉ | Terminais de anexo `POST/GET/LIST/DELETE` com ids e testes determinísticos. | status.md:231 |
| Trabalhador de prova de antecedentes e relatório TTL | ▲ | Esboço do provador atrás do sinalizador de recurso; TTL GC e botões de configuração conectados; pipeline completo pendente. | status.md:212; status.md:233 |
| Vinculação de hash de envelope no CoreHost | ◉ | Verifique os hashes de envelope vinculados ao CoreHost e expostos por meio de pulsos de auditoria. | status.md:250 |
| Controle de histórico de raiz blindado | ◉ | Instantâneos de raiz encadeados no CoreHost com histórico limitado e configuração de raiz vazia. | status.md:303 |
| Execução de votação ZK e bloqueios de governança | ○ | Derivação de nulificador, atualizações de bloqueio, alternadores de verificação implementados; ciclo de vida completo ainda em maturação. | status.md:126-128; status.md:194-195 |
| Anexo de prova pré-verificação e desduplicação | ◉ | A sanidade da tag de back-end, a desduplicação e os registros de prova persistiram na pré-execução. | status.md:348; status.md:602 |
| Ponto final de busca de prova ZK Torii | ◉ | `/v2/zk/proof/{backend}/{hash}` expõe registros de prova (status, altura, vk_ref/commitment). | status.md:94 |

## Integração IVM e Kotodama| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| CoreHost syscall → ponte ISI | ○ | Decodificação de ponteiro TLV e enfileiramento de syscall operacional; lacunas de cobertura/testes de paridade planeados. | status.md:299-307; status.md:477-486 |
| Construtores de ponteiro e componentes de domínio | ◉ | Os integrados Kotodama emitem TLVs e SCALLs Norito digitados, com testes e documentos IR/e2e. | status.md:299-301 |
| Validação estrita do Pointer-ABI e sincronização de documentos | ◉ | Política TLV aplicada no host/IVM com testes dourados e documentos gerados. | status.md:227; status.md:317; status.md:344; status.md:366; status.md:527 |
| Controle de syscall ZK via CoreHost | ◉ | As filas por operação bloqueiam os envelopes verificados e impõem a correspondência de hash antes da execução do ISI. | crates/iroha_core/src/smartcontracts/ivm/host.rs:213; crates/iroha_core/src/smartcontracts/ivm/host.rs:279 |
| Kotodama ponteiro-documentos e gramática ABI | ◉ | Gramática/documentos sincronizados com construtores ativos e mapeamentos SCALL. | status.md:299-301 |
| Mecanismo orientado por esquema ISO 20022 e ponte Torii | ◉ | Esquemas canônicos ISO 20022 incorporados, análise XML determinística e API `/v2/iso20022/status/{MsgId}` exposta. | status.md:65-70 |

## Aceleração de Hardware

| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Testes de paridade de cauda/desalinhamento SIMD | ◉ | Testes de paridade randomizados garantem que as operações vetoriais SIMD correspondam à semântica escalar para alinhamento arbitrário. | status.md:243 |
| Fallback e autotestes de Metal/CUDA | ◉ | Os back-ends da GPU executam autotestes dourados e voltam para escalar/SIMD em caso de incompatibilidade; suítes de paridade cobrem SHA-256/Keccak/AES. | status.md:244-246 |

## Tempo de rede e modos de consenso

| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Serviço de tempo de rede (NTS) | ✖︎ | O design existe em `new_pipeline.md`; implementação ainda não monitorada nas atualizações de status. | novo_pipeline.md |
| Modo de consenso PoS nomeado | ✖︎ | Nexus documenta os modos de conjunto fechado e NPoS; implementação principal pendente. | novo_pipeline.md; nexo.md |

## Roteiro do razão Nexus| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Andaime de contrato do Diretório Espacial | ✖︎ | Contrato de registro global para manifestos/governança do DS ainda não implementado. | nexo.md |
| Formato e ciclo de vida do manifesto do Data Space | ✖︎ | O esquema de manifesto Norito, o controle de versão e o fluxo de governança permanecem no roteiro. | nexo.md |
| Governança do DS e rotação de validadores | ✖︎ | Procedimentos on-chain para adesão/rotação ao DS ainda em fase de design. | nexo.md |
| Ancoragem Cross-DS e composição de bloco Nexus | ✖︎ | Camada de composição e compromissos de ancoragem delineados mas não implementados. | nexo.md |
| Armazenamento com código de eliminação Kura/WSV | ✖︎ | Armazenamento de blob/instantâneo codificado para eliminação para DS público/privado ainda não criado. | nexo.md |
| Política de prova ZK/otimista por DS | ✖︎ | Requisitos e aplicação de prova por DS não rastreados no código. | nexo.md |
| Isolamento de taxas/cotas por espaço de dados | ✖︎ | As cotas específicas do DS e os mecanismos de política de taxas continuam sendo trabalhos futuros. | nexo.md |

## Caos e injeção de falhas

| Recurso | Estado | Notas | Evidência |
|--------|--------|-------|----------|
| Orquestração de caosnet Izanami | ○ | A carga de trabalho Izanami agora impulsiona definição de ativos, metadados, NFT e receitas de repetição de gatilho com cobertura de unidade para os novos caminhos. | crates/izanami/src/instructions.rs; crates/izanami/src/instructions.rs#tests |