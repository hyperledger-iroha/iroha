---
lang: pt
direction: ltr
source: docs/source/fraud_monitoring_system.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7c8262bacbb15b83bd70c824990e4948832418b59f184bca353eee899e44f4d4
source_last_modified: "2026-01-03T18:07:57.676991+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sistema de monitoramento de fraude

Este documento captura o design de referência para a capacidade compartilhada de monitoramento de fraudes que acompanhará o livro-razão principal. O objetivo é fornecer aos prestadores de serviços de pagamento (PSP) sinais de risco de alta qualidade para cada transação, mantendo ao mesmo tempo a custódia, a privacidade e as decisões políticas sob o controle de operadores designados fora do mecanismo de liquidação.

## Metas e critérios de sucesso
- Forneça avaliações de risco de fraude em tempo real (<120 ms 95p, <40 ms mediana) para cada pagamento que toca no mecanismo de liquidação.
- Preservar a privacidade do usuário, garantindo que o serviço central nunca processe informações de identificação pessoal (PII) e apenas ingira identificadores pseudônimos e telemetria comportamental.
- Suporta ambientes multi-PSP onde cada provedor mantém autonomia operacional, mas pode consultar inteligência compartilhada.
- Adapte-se continuamente a novos padrões de ataque por meio de modelos supervisionados e não supervisionados, sem introduzir comportamento de registro não determinístico.
- Fornecer rastros de decisões auditáveis ​​para reguladores e revisores independentes sem expor carteiras ou contrapartes sensíveis.

## Escopo
- **No escopo:** Pontuação de risco de transação, análise comportamental, correlação entre PSP, alerta de anomalias, ganchos de governança e APIs de integração de PSP.
- **Fora do escopo:** Aplicação direta (permanece como responsabilidade do PSP), triagem de sanções (tratada por pipelines de conformidade existentes) e prova de identidade (o gerenciamento de alias cobre isso).

## Requisitos Funcionais
1. **API de pontuação de transação**: API síncrona que os PSPs chamam antes de encaminhar um pagamento para o mecanismo de liquidação, retornando uma pontuação de risco, veredicto categórico e recursos de raciocínio.
2. **Ingestão de eventos**: fluxo de resultados de liquidação, eventos do ciclo de vida da carteira, impressões digitais de dispositivos e feedback de fraude no nível PSP para aprendizado contínuo.
3. **Gerenciamento do ciclo de vida do modelo**: modelos versionados com treinamento off-line, implantação shadow, implementação gradual e suporte para reversão. A heurística determinística de fallback deve existir para cada recurso.
4. **Círculo de feedback**: Os PSPs devem ser capazes de enviar casos de fraude confirmados, falsos positivos e notas de correção. O sistema alinha o feedback com recursos de risco e atualiza análises.
5. **Controles de privacidade**: Todos os dados armazenados e transmitidos devem ser baseados em alias. Qualquer solicitação contendo metadados de identidade brutos é rejeitada e registrada.
6. **Relatórios de governança**: exportações programadas de métricas agregadas (detecções por PSP, tipologias, latência de resposta) além de APIs de investigação ad hoc para auditores autorizados.
7. **Resiliência**: implantação ativa-ativa em pelo menos duas instalações com esgotamento e repetição automáticos de filas. Se o serviço for degradado, os PSPs recorrem às regras locais sem bloquear o livro-razão.## Requisitos Não Funcionais
- **Determinismo e consistência**: as pontuações de risco orientam as decisões do PSP, mas não alteram a execução do razão. As confirmações do razão permanecem determinísticas entre os nós.
- **Escalabilidade**: Sustente ≥10 mil avaliações de risco por segundo com escalabilidade horizontal e particionamento de mensagens codificadas por identificadores de pseudocarteira.
- **Observabilidade**: exponha métricas (`fraud.scoring_latency_ms`, `fraud.risk_score_distribution`, `fraud.api_error_rate`, `fraud.model_version_active`) e logs estruturados para cada chamada de pontuação.
- **Segurança**: TLS mútuo entre PSPs e o serviço central, módulos de segurança de hardware para assinatura de envelopes de resposta, trilhas de auditoria invioláveis.
- **Conformidade**: Alinhe-se aos requisitos de AML/CFT, forneça períodos de retenção configuráveis ​​e integre-se a fluxos de trabalho de preservação de evidências.

## Visão geral da arquitetura
1. **Camada de gateway de API**
  - Recebe solicitações de pontuação e feedback por meio de APIs HTTP/JSON autenticadas.
   - Executa validação de esquema usando codecs Norito e impõe limites de taxa por ID PSP.

2. **Serviço de agregação de recursos**
   - Une solicitações recebidas com agregados históricos (velocidade, padrões geoespaciais, uso de dispositivos) armazenados em um armazenamento de recursos de série temporal.
   - Suporta janelas de recursos configuráveis ​​(minutos, horas, dias) usando funções de agregação determinísticas.

3. **Motor de risco**
   - Executa o pipeline do modelo ativo (conjunto de árvores com gradiente aumentado, detectores de anomalias, regras).
   - Inclui um conjunto de regras determinísticas de fallback para garantir respostas limitadas quando as pontuações do modelo não estão disponíveis.
   - Emite envelopes `FraudAssessment` com partitura, banda, recursos de contribuição e versão do modelo.## Modelos de pontuação e heurísticas
- **Escala e faixas de pontuação**: as pontuações de risco são normalizadas para 0–1.000. As bandas são definidas como: `0–249` (baixo), `250–549` (médio), `550–749` (alto), `750+` (crítico). As faixas são mapeadas para ações recomendadas para PSPs (aprovação automática, intensificação, fila para revisão, recusa automática), mas a fiscalização permanece específica do PSP.
- **Conjunto Modelo**:
  - Árvores de decisão aprimoradas por gradiente ingerem recursos estruturados, como quantidade, alias/velocidade do dispositivo, categoria do comerciante, força de autenticação, nível de confiança do PSP e recursos gráficos entre carteiras.
  - Um detector de anomalias baseado em autoencoder é executado em vetores comportamentais com janelas de tempo (cadência de gasto por alias, comutação de dispositivos, entropia temporal). As pontuações são calibradas em relação à atividade recente do PSP para limitar o desvio.
  - As regras de políticas determinísticas são executadas primeiro; seus resultados alimentam os modelos estatísticos como recursos binários/contínuos para que o conjunto possa aprender interações.
- **Heurísticas de fallback**: quando a execução do modelo falha, a camada determinística ainda produz uma pontuação limitada agregando penalidades de regras. Cada regra contribui com um peso configurável, somado e fixado na escala de 0 a 1.000, garantindo latência e explicabilidade no pior caso.
- **Orçamento de latência**: metas de pipeline de pontuação <20 ms para gateway de API + validação, <30 ms para agregação de recursos (servidos de caches na memória com write-behind para armazenamentos persistentes) e <40 ms para avaliação de conjunto. O substituto determinístico retorna em <10 ms se a inferência de ML exceder seu orçamento, garantindo que o P95 geral permaneça abaixo de 120 ms.
 - **Orçamento de latência**: metas de pipeline de pontuação <20 ms para gateway de API + validação, <30 ms para agregação de recursos (servidos de caches na memória com write-behind para armazenamentos persistentes) e <40 ms para avaliação de conjunto. O substituto determinístico retorna em <10 ms se a inferência de ML exceder seu orçamento, garantindo que o P95 geral permaneça abaixo de 120 ms.## Design de cache de recursos na memória
- **Layout de fragmentos**: os armazenamentos de recursos são fragmentados por hash de alias de 64 bits em fragmentos `N = 256`. Cada fragmento possui:
  - Um buffer de anel sem bloqueio para deltas de transações recentes (janelas de 5 min + 1 hora) armazenados como struct-of-arrays para maximizar a localidade da linha de cache.
  - Uma árvore Fenwick compactada (baldes de 16 bits compactados) para manter agregados de 24 horas/7 dias sem recomputação completa.
  - Um mapa hash hop-scotch mapeando contrapartes → estatísticas contínuas (contagem, soma, variação, último carimbo de data e hora) limitadas a 1.024 entradas por alias.
- **Residência de memória**: Hot shards permanecem na RAM. Para um universo de 50 milhões de alias com 1% ativo na última hora, a residência do cache é de aproximadamente 500 mil aliases. Com aproximadamente 320 B por alias de metadados ativos, o conjunto de trabalho é de aproximadamente 160 MB – pequeno o suficiente para cache L3 em servidores modernos.
- **Simultaneidade**: os leitores emprestam referências imutáveis ​​por meio de recuperação baseada em época; os escritores anexam deltas e atualizam agregados usando comparar e trocar. Isso evita a contenção de mutex e mantém caminhos ativos para duas operações atômicas + perseguição de ponteiro limitado.
- **Pré-busca**: o trabalhador de pontuação emite dicas manuais `prefetch_read` para o próximo fragmento de alias assim que a validação da solicitação for concluída, ocultando a latência da memória principal (~80 ns) por trás da agregação de recursos.
- **Write-behind Log**: um WAL por fragmento agrupa deltas a cada 50 ms (ou 4 KB) e libera para o armazenamento durável. Os pontos de verificação são executados a cada 5 minutos para manter os limites de recuperação rígidos.

### Análise teórica da latência (servidor Intel Ice Lake classe, 3,1 GHz)
- **Pesquisa de shard + pré-busca**: 1 perda de cache (~80 ns) mais cálculo de hash (<10 ns).
- **Iteração do buffer de anel (32 entradas)**: 32 × 2 cargas = 64 cargas; com linhas de cache de 32 B e acesso sequencial permanece em L1 → ~20 ns.
- **Atualizações Fenwick (log₂ 2048 ≈ 11 etapas)**: 11 saltos de ponteiro; assumindo metade L1, metade L2 atinge → ~30 ns.
- **Teste de mapa hop-scotch (fator de carga 0,75, 2 testes)**: 2 linhas de cache, ~2 × 15 ns.
- **Montagem de recurso de modelo**: 150 operações escalares (<0,1 ns cada) → ~15 ns.Somando isso, resulta em ~ 160 ns de computação e ~ 120 ns de paralisações de memória por solicitação (~ 0,28 µs). Com quatro trabalhadores de agregação simultâneos por núcleo, o estágio atende facilmente ao orçamento de 30 ms, mesmo sob carga intermitente; a implantação real deve registrar histogramas para validação (via `fraud.feature_cache_lookup_ms`).
- **Janelas de recursos e agregação**:
  - Janelas de curto prazo (5 minutos e 1 hora) e de longo prazo (24 horas e 7 dias) rastreiam a velocidade de gastos, a reutilização do dispositivo e os graus do gráfico de alias.
  - Os recursos gráficos (por exemplo, dispositivos compartilhados entre aliases, distribuição repentina, novas contrapartes em clusters de alto risco) dependem de resumos compactados regularmente para que as consultas permaneçam abaixo de um milissegundo.
  - A heurística de localização compara geobuckets grosseiros com o comportamento histórico, sinalizando saltos improváveis ​​(por exemplo, vários locais distantes em minutos) usando um incremento de risco limitado baseado em Haversine.
  - Os detectores de formato de fluxo mantêm histogramas contínuos de valores de entrada/saída e contrapartes para detectar assinaturas de mistura/tumbling (fan-in rápido seguido por fan-out semelhante, sequências de salto cíclicas, intermediários de curta duração).
- **Catálogo de regras (não exaustivo)**:
  - **Quebra de velocidade**: série rápida de transferências de alto valor que excedem os limites por alias ou por dispositivo.
  - **Anomalia do gráfico de alias**: o alias interage com um cluster vinculado a casos de fraude confirmados ou padrões de mule conhecidos.
  - **Reutilização de dispositivos**: Impressão digital de dispositivo compartilhada entre aliases pertencentes a diferentes grupos de usuários PSP sem vinculação prévia.
  - **Primeira vez de alto valor**: Novo alias tentando valores acima do corredor de integração típico do PSP.
  - **Rebaixamento de autenticação**: a transação usa fatores mais fracos do que a linha de base da conta (por exemplo, fallback de biometria para PIN) sem justificativa declarada pelo PSP.
  - **Padrão de mistura/tumbling**: o Alias ​​participa de altas cadeias de fan-in/fan-out com tempo fortemente acoplado, quantidades repetidas de ida e volta ou fluxos circulares em vários aliases em janelas curtas. A regra aumenta a pontuação usando picos de centralidade do gráfico e detectores de formato de fluxo; casos graves são fixados na banda `high` mesmo antes da saída ML.
  - **Acerto na lista negra de transações**: o pseudônimo ou contraparte aparece no feed da lista negra compartilhada com curadoria por meio de votação de governança na cadeia ou autoridade delegada com controles `sudo` (por exemplo, ordens regulatórias, fraude confirmada). A pontuação fixa-se à banda `critical` e emite o código de razão `BLACKLIST_MATCH`; Os PSPs devem registrar substituições para auditoria.
  - **Incompatibilidade de assinatura de sandbox**: PSP envia avaliação gerada com assinatura de modelo desatualizada; a pontuação aumenta para `critical` e os gatilhos de gancho de auditoria.
- **Códigos de Motivo**: Cada avaliação inclui códigos de motivo legíveis por máquina classificados por peso de contribuição (por exemplo, `VELOCITY_BREACH`, `NEW_DEVICE`, `GRAPH_HIGH_RISK`, `AUTH_DOWNGRADE`). Os PSPs podem divulgá-los para operadoras ou carteiras para mensagens dos usuários.- **Governança do modelo**: A calibração e a definição de limites seguem manuais documentados – curvas ROC/PR revisadas trimestralmente, backtesting contra fraudes rotuladas e modelos desafiadores executados na sombra até ficarem estáveis. Qualquer atualização de limite requer aprovação dupla (operações fraudulentas + risco independente).

## Fluxo de lista negra originado pela governança
- **Autoria em cadeia**: as entradas da lista negra são introduzidas através do subsistema de governança (`iroha_core::smartcontracts::isi::governance`) como um ISI `BlacklistProposal` que lista aliases, ids de PSP ou impressões digitais de dispositivos a serem bloqueados. As partes interessadas votam usando o processo eleitoral padrão; uma vez atingido o quorum, a cadeia emite um registro `GovernanceEvent::BlacklistUpdated` contendo as adições/remoções aprovadas mais um `blacklist_epoch` que aumenta monotonicamente.
- **Caminho sudo delegado**: Ações de emergência podem ser executadas por meio da instrução `sudo::Execute`, que emite o mesmo evento `BlacklistUpdated`, mas sinaliza a alteração como `origin = Sudo`. Isto reflete a história da cadeia com proveniência explícita para que os auditores possam distinguir votos de consenso de intervenções delegadas.
- **Canal de distribuição**: o serviço de ponte FMS assina o fluxo `LedgerEvent` (codificado em Norito) e observa eventos `BlacklistUpdated`. Cada evento é validado em relação à prova Merkle de governança e verificado com a assinatura do bloco antes de ser aplicado. Os eventos são idempotentes; o FMS mantém o `blacklist_epoch` mais recente para evitar replays.
- **Aplicativo dentro do FMS**: depois que uma atualização é aceita, as entradas são gravadas no armazenamento de regras determinísticas (apoiado por armazenamento somente anexado com logs de auditoria). O mecanismo de pontuação recarrega a lista negra em 30 segundos, garantindo que as avaliações subsequentes acionem a regra `BLACKLIST_MATCH` e fixem-na em `critical`.
- **Auditoria e reversão**: a governança pode votar para remover entradas por meio do mesmo pipeline. O FMS mantém instantâneos históricos marcados com `blacklist_epoch` para que os operadores possam responder a perguntas forenses ou reproduzir decisões anteriores durante as investigações.

4. **Plataforma de aprendizagem e análise**
   - Recebe eventos de fraude confirmados, resultados de liquidação e feedback do PSP por meio de um livro-razão somente anexado (por exemplo, armazenamento de objetos Kafka +).
   - Fornece notebooks/trabalhos off-line para cientistas de dados treinarem novamente modelos. Os artefatos do modelo são versionados e assinados antes da promoção.

5. **Portal de Governança**
   - Interface restrita para auditores revisarem tendências, pesquisar avaliações históricas e exportar relatórios de incidentes.
   - Implementa verificações de políticas para que os investigadores não possam detalhar as PII sem a cooperação do PSP.

6. **Adaptadores de integração**
   - SDKs leves para PSPs (Rust, Kotlin, Swift, TypeScript) implementando as solicitações/respostas Norito e cache local.
   - Gancho do mecanismo de liquidação (dentro de `iroha_core`) que regista referências de avaliação de risco quando os PSP encaminham transações pós-verificação.## Fluxo de dados
1. O PSP autentica-se no gateway API e envia um `RiskQuery` contendo:
   - Identificadores de alias para pagador/beneficiário, ID do dispositivo com hash, valor da transação, categoria, balde grosso de geolocalização, sinalizadores de confiança do PSP e metadados de sessões recentes.
2. O gateway valida a carga útil, enriquece com metadados PSP (nível de licença, SLA) e filas para agregação de recursos.
3. O serviço de recursos extrai os agregados mais recentes, constrói o vetor do modelo e o envia ao mecanismo de risco.
4. O mecanismo de risco avalia a solicitação, anexa códigos de razão determinísticos, assina o `FraudAssessment` e devolve-o ao PSP.
5. O PSP combina a avaliação com suas políticas locais para aprovar, recusar ou intensificar a autenticação da transação.
6. O resultado (aprovado/recusado, fraude confirmada/falso positivo) é enviado de forma assíncrona para a plataforma de aprendizagem para melhoria contínua.
7. Os processos diários em lote acumulam métricas para relatórios de governança e enviam alertas de políticas (por exemplo, casos crescentes de engenharia social) para painéis de controle do PSP.

## Integração com componentes Iroha
- **Ganchos de host principais**: a admissão de transação agora impõe os metadados `fraud_assessment_band` sempre que `fraud_monitoring.enabled` e `required_minimum_band` são definidos. O host rejeita transações que faltam no campo ou que carregam uma banda abaixo do mínimo configurado e emite um aviso determinístico quando `missing_assessment_grace_secs` é diferente de zero (janela de carência programada para remoção no marco FM-204 assim que o verificador remoto estiver conectado). As avaliações também devem incluir `fraud_assessment_score_bps`; o host compara a pontuação com a faixa declarada (0–249 ➜ baixa, 250–549 ➜ média, 550–749 ➜ alta, 750+ ➜ crítica, com valores de ponto base suportados até 10.000). Quando `fraud_monitoring.attesters` é configurado, as transações devem anexar um `fraud_assessment_envelope` (base64) codificado em Norito e um `fraud_assessment_digest` (hex) correspondente. O daemon decodifica deterministicamente o envelope, verifica a assinatura Ed25519 em relação ao registro do atestador, recalcula o resumo sobre a carga útil não assinada e rejeita incompatibilidades para que apenas as avaliações atestadas cheguem ao consenso.
- **Configuração**: adicione entradas de configuração em `iroha_config::fraud_monitoring` para os terminais de serviço de risco, tempos limite e faixas de avaliação necessárias. Os padrões desabilitam a aplicação para o desenvolvimento local.| Chave | Tipo | Padrão | Notas |
  | --- | --- | --- | --- |
  | `enabled` | bool | `false` | Chave mestre para verificações de admissão; sem `required_minimum_band` o host registra um aviso e ignora a aplicação. |
  | `service_endpoints` | matriz | `[]` | Lista ordenada de URLs base de serviços fraudulentos. As duplicatas são removidas de forma determinística; reservado para o próximo verificador. |
  | `connect_timeout_ms` | duração | `500` | Milissegundos antes das tentativas de conexão serem abortadas; valores zero retornam ao padrão. |
  | `request_timeout_ms` | duração | `1500` | Milissegundos para aguardar uma resposta do serviço de risco. |
  | `missing_assessment_grace_secs` | duração | `0` | Janela de tolerância permitindo avaliações faltantes; valores diferentes de zero acionam um substituto determinístico que registra e permite a transação. |
  | `required_minimum_band` | enumeração (`low`, `medium`, `high`, `critical`) | `null` | Quando definidas, as transações devem anexar uma avaliação igual ou superior a esta faixa de severidade; valores mais baixos são rejeitados. Defina como `null` para desabilitar o gate mesmo se `enabled` for verdadeiro. |
  | `attesters` | matriz | `[]` | Registro opcional de mecanismos de atestado. Quando preenchidos, os envelopes devem ser assinados por uma das chaves listadas e incluir um resumo correspondente. |

- **Validação**: testes de unidade em `crates/iroha_core/tests/fraud_monitoring.rs` cobrem caminhos de banda desativados, ausentes e insuficientes; `integration_tests::fraud_monitoring_requires_assessment_bands` exercita o fluxo de avaliação simulada de ponta a ponta.

- **Telemetria**: `iroha_telemetry` exporta coletores voltados para PSP capturando contagens de avaliação (`fraud_psp_assessments_total{tenant,band,lane,subnet}`), metadados ausentes (`fraud_psp_missing_assessment_total{tenant,lane,subnet,cause}`), histogramas de latência (`fraud_psp_latency_ms{tenant,lane,subnet}`), distribuições de pontuação (`fraud_psp_score_bps{tenant,band,lane,subnet}`), cargas úteis inválidas (`fraud_psp_invalid_metadata_total{tenant,field,lane,subnet}`), resultados de atestado (`fraud_psp_attestation_total{tenant,engine,lane,subnet,status}`) e incompatibilidades de resultados (`fraud_psp_outcome_mismatch_total{tenant,direction,lane,subnet}`). As chaves de metadados esperadas em cada transação são `fraud_assessment_band`, `fraud_assessment_tenant`, `fraud_assessment_score_bps`, `fraud_assessment_latency_ms`, o par envelope/resumo do atestador (`fraud_assessment_envelope`, `fraud_assessment_digest`) e o pós-incidente Sinalizador `fraud_assessment_disposition` (valores: `approved`, `declined`, `manual_review`, `confirmed_fraud`, `false_positive`, `chargeback`, `loss`).
- **Esquema Norito**: Defina os tipos Norito para `RiskQuery`, `FraudAssessment` e relatórios de governança. Forneça testes de ida e volta para garantir a estabilidade do codec.

## Privacidade e minimização de dados
- Aliases, IDs de dispositivos com hash e intervalos de geolocalização grosseira formam todo o plano de dados compartilhado com o serviço central.
- Os PSP mantêm o mapeamento de pseudónimos para identidades reais; nenhum mapeamento desse tipo sai de seu perímetro.
- Os modelos de risco operam apenas em sinais comportamentais pseudônimos mais o contexto enviado pelo PSP (categoria do comerciante, canal, força de autenticação).
- As exportações de auditoria são agregadas (por exemplo, contagens por PSP por dia). Qualquer detalhamento requer controle duplo e desanonimização do lado do PSP.## Operações e implantação
- Implementar a plataforma de pontuação como um subsistema dedicado gerido por um operador designado, distinto dos operadores dos nós do banco central.
- Fornece ambientes azuis/verdes: `fraud-scoring-prod`, `fraud-scoring-shadow`, `fraud-lab`.
- Implementar verificações de integridade automatizadas (latência de API, backlog de mensagens, sucesso de carregamento de modelo). Se as verificações de integridade falharem, os SDKs do PSP mudam automaticamente para o modo somente local e notificam os operadores.
- Manter buckets de retenção: armazenamento quente (30 dias no feature store), armazenamento quente (1 ano no armazenamento de objetos), arquivo frio (5 anos compactado).

## Coletores e painéis de telemetria

### Coletores obrigatórios

- **Prometheus scrape**: habilite `/metrics` em cada validador executando o perfil de integração PSP para que a série `fraud_psp_*` seja exportada. Os rótulos padrão incluem os IDs de espaço reservado `subnet="global"` e `lane` para que os painéis possam dinamizar assim que o roteamento de várias sub-redes for enviado.
- **Totais de avaliações**: `fraud_psp_assessments_total{tenant,band}` conta avaliações aceitas por faixa de gravidade; os alertas disparam se um inquilino parar de reportar por 5 minutos.
- **Metadados ausentes**: `fraud_psp_missing_assessment_total{tenant,cause}` distingue rejeições definitivas (`cause="missing"`) de permissões de período de carência (`cause="grace"`). Transações de gate que caem repetidamente no intervalo de carência.
- **Histograma de latência**: `fraud_psp_latency_ms_bucket` rastreia a latência de pontuação relatada pelo PSP. Alvo 20% da média dos últimos 30 dias.
- **Metadados inválidos**: `fraud_psp_invalid_metadata_total{field}` sinaliza regressões de carga útil do PSP (por exemplo, IDs de locatário ausentes, disposições malformadas) para que as atualizações do SDK possam ser implementadas rapidamente.
- **Status do atestado**: `fraud_psp_attestation_total{tenant,engine,status}` confirma que os envelopes estão sendo assinados e os resumos correspondem. Alerte se `status!="verified"` aumentar para qualquer locatário ou mecanismo.

### Cobertura do painel

- **Visão geral executiva**: gráfico de áreas empilhadas de `fraud_psp_assessments_total` por banda por locatário, juntamente com uma tabela que resume a latência P95 e contagens de incompatibilidade.
- **Operações**: painéis de histograma para `fraud_psp_latency_ms` e `fraud_psp_score_bps` com comparação semana após semana, além de contadores de estatísticas únicas para `fraud_psp_missing_assessment_total` divididos por `cause`.
- **Monitoramento de risco**: gráfico de barras de `fraud_psp_outcome_mismatch_total` por locatário, tabela detalhada listando casos `fraud_assessment_disposition=confirmed_fraud` recentes em que `band` era `low` ou `medium`.
- **Regras de alerta**:
  - `rate(fraud_psp_missing_assessment_total{cause="missing"}[5m]) > 0` → alerta de paging (admissão rejeitando tráfego PSP).
  - `histogram_quantile(0.95, sum(rate(fraud_psp_latency_ms_bucket[10m])) by (le,tenant)) > 150` → violação de SLO de latência.
  - `sum by (tenant) (rate(fraud_psp_outcome_mismatch_total{direction="missed_fraud"}[1h])) > 0.01` → desvio do modelo/lacuna política.

### Expectativas de failover- Os SDKs PSP devem manter dois endpoints de pontuação ativos e fazer failover dentro de 15 segundos após a detecção de erros de transporte ou picos de latência >200 ms. O razão tolera tráfego de tolerância por no máximo `fraud_monitoring.missing_assessment_grace_secs`; os operadores devem manter o botão em <=30 segundos na produção.
- Os validadores registram `fraud_psp_missing_assessment_total{cause="grace"}` durante o fallback; se um inquilino permanecer em carência por mais de 5 minutos, o PSP deverá mudar para a revisão manual e abrir um incidente Sev2 com a equipe de operações de fraude compartilhada.
- As implantações ativo-ativo devem demonstrar drenagem/repetição de fila durante exercícios de recuperação de desastres. As métricas de repetição devem manter `fraud_psp_latency_ms` P99 abaixo de 400 ms para a janela de repetição.

## Lista de verificação de compartilhamento de dados PSP

1. **Telemetria**: exponha as chaves de metadados listadas acima para cada transação entregue ao razão; os identificadores de locatário devem ser pseudônimos e ter como escopo o contrato PSP.
2. **Anonimização**: confirme se os hashes dos dispositivos, identificadores de alias e disposições são pseudonimizados antes de sair do perímetro do PSP; nenhuma PII pode ser incorporada nos metadados Norito.
3. **Relatórios de latência**: preencha `fraud_assessment_latency_ms` com tempo de ponta a ponta (gateway para PSP) para que as regressões de SLA apareçam imediatamente.
4. **Reconciliação de resultados**: atualize `fraud_assessment_disposition` assim que os casos de fraude forem confirmados (por exemplo, estorno publicado) para manter as métricas de incompatibilidade precisas.
5. **Exercícios de failover**: ensaie trimestralmente usando a lista de verificação compartilhada — verifique o failover automático de endpoint, garanta o registro da janela de carência e anexe notas de detalhamento à tarefa de acompanhamento arquivada por `scripts/ci/schedule_fraud_scoring.sh`.
6. **Validação do painel**: as equipes de operações do PSP devem revisar os painéis Prometheus após a integração e após cada exercício da equipe vermelha para confirmar que as métricas estão fluindo com os rótulos de locatário esperados.

## Considerações de segurança
- Todas as respostas são assinadas com chaves apoiadas por hardware; Os PSPs validam as assinaturas antes de confiar nas pontuações.
- Limite de taxa por alias/dispositivo para mitigar ataques de sondagem com o objetivo de aprender os limites do modelo.
- Incorpore marcas d'água nas avaliações para rastrear respostas vazadas sem revelar publicamente a identidade do PSP.
- Realizar exercícios trimestrais da equipe vermelha em coordenação com o GT de Segurança (Marco 0) e alimentar as descobertas nas atualizações do roteiro.## Fases de Implementação
1. **Fase 0 – Fundações**
   - Finalizar esquemas Norito, andaimes PSP SDK, fiação de configuração e stub de verificação do lado do razão.
   - Construir mecanismo de regras determinísticas cobrindo verificações de risco obrigatórias (velocidade, velocidade por par de alias, reutilização de dispositivos).
2. **Fase 1 – MVP de pontuação central**
   - Implantar armazenamento de recursos, serviço de pontuação e painéis de telemetria.
   - Integrar pontuação em tempo real com uma coorte PSP limitada; capturar métricas de latência e qualidade.
3. **Fase 2 – Análise Avançada**
   - Introduzir detecção de anomalias, análise de links baseada em gráficos e limites adaptativos.
   - Lançar portal de governança e pipelines de relatórios em lote.
4. **Fase 3 – Aprendizagem Contínua e Automação**
   - Automatize pipelines de treinamento/validação de modelos, adicione implantações canário e expanda a cobertura do SDK.
   - Alinhe-se com acordos de compartilhamento de dados entre jurisdições e conecte-se a futuras pontes de múltiplas sub-redes.

## Perguntas abertas
- Que entidade reguladora irá licenciar o operador do serviço antifraude e como são partilhadas as responsabilidades de supervisão?
- Como os PSPs expõem os fluxos de desafios do usuário final e, ao mesmo tempo, mantêm uma experiência do usuário consistente entre os provedores?
- Que tecnologias que melhoram a privacidade (por exemplo, enclaves seguros, agregação homomórfica) devem ser priorizadas quando o serviço de base estiver estável?