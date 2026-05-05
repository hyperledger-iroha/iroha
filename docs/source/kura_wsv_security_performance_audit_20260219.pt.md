<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/source/kura_wsv_security_performance_audit_20260219.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 194721ce71f5593cc9e4df6313c6e3aa85c5c3dc0e3efe4a28d0ded968c0584a
source_last_modified: "2026-02-19T08:31:06.766140+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Auditoria de segurança e desempenho Kura / WSV (19/02/2026)

## Escopo

Esta auditoria abrangeu:

- Persistência Kura e caminhos orçamentários: `crates/iroha_core/src/kura.rs`
- Caminhos de consulta/confirmação de estado/WSV de produção: `crates/iroha_core/src/state.rs`
- Superfícies de host simuladas IVM WSV (escopo de teste/desenvolvimento): `crates/ivm/src/mock_wsv.rs`

Fora do escopo: caixas não relacionadas e repetições de benchmark de sistema completo.

## Resumo de risco

- Crítico: 0
- Alto: 4
- Médio: 6
- Baixo: 2

## Descobertas (ordenadas por gravidade)

### Alto

1. **O gravador Kura entra em pânico com falhas de E/S (risco de disponibilidade do nó)**
- Componente: Kura
Tipo: Segurança (DoS), Confiabilidade
- Detalhe: o loop do gravador entra em pânico com erros de acréscimo/index/fsync em vez de retornar erros recuperáveis, portanto, falhas transitórias no disco podem encerrar o processo do nó.
- Evidência:
  -`crates/iroha_core/src/kura.rs:1697`
  -`crates/iroha_core/src/kura.rs:1724`
  -`crates/iroha_core/src/kura.rs:1845`
  -`crates/iroha_core/src/kura.rs:1854`
  -`crates/iroha_core/src/kura.rs:1860`
- Impacto: carga remota + pressão do disco local podem induzir loops de travamento/reinicialização.2. **O despejo de Kura faz reescrita completa de dados/índices sob `block_store` mutex**
- Componente: Kura
Tipo: Desempenho, Disponibilidade
- Detalhe: `evict_block_bodies` reescreve `blocks.data` e `blocks.index` por meio de arquivos temporários enquanto mantém o bloqueio `block_store`.
- Evidência:
  Aquisição de bloqueio: `crates/iroha_core/src/kura.rs:834`
  - Loops de reescrita completa: `crates/iroha_core/src/kura.rs:921`, `crates/iroha_core/src/kura.rs:942`
  - Substituição/sincronização atômica: `crates/iroha_core/src/kura.rs:956`, `crates/iroha_core/src/kura.rs:960`
- Impacto: eventos de despejo podem paralisar gravações/leituras por períodos prolongados em históricos grandes.

3. **A confirmação do estado mantém `view_lock` grosseiro em trabalhos pesados de confirmação**
Componente: Produção WSV
Tipo: Desempenho, Disponibilidade
- Detalhe: o commit do bloco contém `view_lock` exclusivo ao confirmar transações, hashes de bloco e estado mundial, criando fome de leitor sob blocos pesados.
- Evidência:
  - O bloqueio começa: `crates/iroha_core/src/state.rs:17456`
  Trabalhe dentro da fechadura: `crates/iroha_core/src/state.rs:17466`, `crates/iroha_core/src/state.rs:17476`, `crates/iroha_core/src/state.rs:17483`
- Impacto: confirmações pesadas e sustentadas podem degradar a capacidade de resposta da consulta/consenso.4. **IVM Aliases de administrador JSON permitem mutações privilegiadas sem verificações de chamador (host de teste/desenvolvimento)**
- Componente: host simulado IVM WSV
- Tipo: Segurança (escalonamento de privilégios em ambientes de teste/desenvolvimento)
- Detalhe: os manipuladores de alias JSON roteiam diretamente para métodos de função/permissão/mutação de pares que não exigem tokens de permissão no escopo do chamador.
- Evidência:
  - Aliases de administrador: `crates/ivm/src/mock_wsv.rs:4274`, `crates/ivm/src/mock_wsv.rs:4371`, `crates/ivm/src/mock_wsv.rs:4448`
  - Mutadores não bloqueados: `crates/ivm/src/mock_wsv.rs:1035`, `crates/ivm/src/mock_wsv.rs:1055`, `crates/ivm/src/mock_wsv.rs:855`
  - Nota de escopo nos documentos do arquivo (intenção de teste/desenvolvimento): `crates/ivm/src/mock_wsv.rs:295`
- Impacto: contratos/ferramentas de teste podem se elevar e invalidar suposições de segurança em equipamentos de integração.

### Médio

5. **As verificações de orçamento do Kura recodificam blocos pendentes em cada enfileiramento (O(n) por gravação)**
- Componente: Kura
Tipo: Desempenho
- Detalhe: cada enfileiramento recalcula bytes de fila pendentes iterando blocos pendentes e serializando cada um por meio do caminho canônico de tamanho de fio.
- Evidência:
  - Verificação de fila: `crates/iroha_core/src/kura.rs:2509`
  - Caminho de codificação por bloco: `crates/iroha_core/src/kura.rs:2194`, `crates/iroha_core/src/kura.rs:2525`
  - Chamado na verificação de orçamento na fila: `crates/iroha_core/src/kura.rs:2580`, `crates/iroha_core/src/kura.rs:2050`
- Impacto: degradação da taxa de transferência de gravação no backlog.6. **As verificações de orçamento do Kura realizam leituras repetidas de metadados de armazenamento de blocos por enfileiramento**
- Componente: Kura
Tipo: Desempenho
- Detalhe: cada verificação lê a contagem de índices duráveis e comprimentos de arquivos enquanto bloqueia `block_store`.
- Evidência:
  -`crates/iroha_core/src/kura.rs:2538`
  -`crates/iroha_core/src/kura.rs:2548`
  -`crates/iroha_core/src/kura.rs:2575`
- Impacto: sobrecarga de E/S/bloqueio evitável no caminho de enfileiramento ativo.

7. **A remoção do Kura é acionada diretamente do caminho do orçamento da fila**
- Componente: Kura
Tipo: Desempenho, Disponibilidade
- Detalhe: o caminho do enfileiramento pode chamar o despejo de forma síncrona antes de aceitar novos blocos.
- Evidência:
  - Enfileirar cadeia de chamadas: `crates/iroha_core/src/kura.rs:2050`
  - Chamada de despejo em linha: `crates/iroha_core/src/kura.rs:2603`
- Impacto: picos de latência final na ingestão de transações/blocos quando próximo do orçamento.

8. **`State::view` pode retornar sem adquirir bloqueio grosseiro sob contenção**
Componente: Produção WSV
- Tipo: compensação consistência/desempenho
- Detalhe: na contenção de bloqueio de gravação, o fallback `try_read` retorna a visualização sem proteção grosseira por design.
- Evidência:
  -`crates/iroha_core/src/state.rs:14543`
  -`crates/iroha_core/src/state.rs:14545`
  -`crates/iroha_core/src/state.rs:18301`
- Impacto: maior vivacidade, mas os chamadores devem tolerar uma atomicidade de componentes cruzados mais fraca sob contenção.9. **`apply_without_execution` usa `expect` rígido no avanço do cursor DA**
Componente: Produção WSV
Tipo: Segurança (DoS via panic-on-invariant-break), Confiabilidade
- Detalhe: o bloco confirmado aplica path panics se os invariantes de avanço do cursor DA falharem.
- Evidência:
  -`crates/iroha_core/src/state.rs:17621`
  -`crates/iroha_core/src/state.rs:17625`
- Impacto: bugs latentes de validação/indexação podem se tornar falhas que eliminam nós.

10. **IVM O syscall de publicação TLV não possui um limite explícito de tamanho de envelope antes da alocação (host de teste/desenvolvimento)**
- Componente: host simulado IVM WSV
Tipo: Segurança (memória DoS), Desempenho
- Detalhe: lê o comprimento do cabeçalho e depois aloca/copia a carga útil TLV completa sem um limite de nível de host neste caminho.
- Evidência:
  -`crates/ivm/src/mock_wsv.rs:3750`
  -`crates/ivm/src/mock_wsv.rs:3755`
  -`crates/ivm/src/mock_wsv.rs:3759`
- Impacto: cargas de teste maliciosas podem forçar grandes alocações.

### Baixo

11. **O canal de notificação Kura é ilimitado (`std::sync::mpsc::channel`)**
- Componente: Kura
Tipo: Desempenho/Higiene de memória
- Detalhe: o canal de notificação pode acumular eventos de ativação redundantes durante pressão sustentada do produtor.
- Evidência:
  -`crates/iroha_core/src/kura.rs:552`
- Impacto: o risco de crescimento de memória é baixo por tamanho de evento, mas pode ser evitado.12. **A fila secundária do pipeline é ilimitada na memória até o esgotamento do gravador**
- Componente: Kura
Tipo: Desempenho/Higiene de memória
- Detalhe: a fila sidecar `push_back` não possui limite/contrapressão explícita.
- Evidência:
  -`crates/iroha_core/src/kura.rs:104`
  -`crates/iroha_core/src/kura.rs:3427`
- Impacto: crescimento potencial da memória durante atrasos prolongados na gravação.

## Cobertura e lacunas de teste existentes

###Kura

- Cobertura existente:
  - comportamento do orçamento de armazenamento: `store_block_rejects_when_budget_exceeded`, `store_block_rejects_when_pending_blocks_exceed_budget`, `store_block_evicts_when_block_exceeds_budget` (`crates/iroha_core/src/kura.rs:6820`, `crates/iroha_core/src/kura.rs:6949`, `crates/iroha_core/src/kura.rs:6984`)
  - correção de despejo e reidratação: `evict_block_bodies_does_not_truncate_unpersisted`, `evicted_block_rehydrates_from_da_store` (`crates/iroha_core/src/kura.rs:8040`, `crates/iroha_core/src/kura.rs:8126`)
- Lacunas:
  - sem cobertura de injeção de falhas para tratamento de falhas de anexação/index/fsync sem pânico
  - nenhum teste de regressão de desempenho para grandes filas pendentes e custo de verificação de orçamento de enfileiramento
  - nenhum teste de latência de despejo de longo histórico sob contenção de bloqueio

### Produção WSV

- Cobertura existente:
  - comportamento de contenção de fallback: `state_view_returns_when_view_lock_held` (`crates/iroha_core/src/state.rs:18293`)
  - segurança de ordem de bloqueio em torno do back-end em camadas: `state_commit_does_not_hold_tiered_backend_while_waiting_for_view_lock` (`crates/iroha_core/src/state.rs:18321`)
- Lacunas:
  - nenhum teste de contenção quantitativo afirmando o tempo máximo de espera de commit aceitável sob commits mundiais pesados
  - nenhum teste de regressão para manipulação sem pânico se os invariantes de avanço do cursor DA quebrarem inesperadamente

### IVM Host simulado WSV- Cobertura existente:
  - semântica do analisador JSON de permissão e análise de pares (`crates/ivm/src/mock_wsv.rs:5234`, `crates/ivm/src/mock_wsv.rs:5332`)
  - testes de fumaça do syscall em torno da decodificação TLV e da decodificação JSON (`crates/ivm/src/mock_wsv.rs:5962`, `crates/ivm/src/mock_wsv.rs:6078`)
- Lacunas:
  - sem testes de rejeição de alias de administrador não autorizados
  - sem testes de rejeição de envelope TLV superdimensionados em `INPUT_PUBLISH_TLV`
  - sem testes de benchmark/guardrail em torno do custo do clone de ponto de verificação/restauração

## Plano de remediação priorizado

### Fase 1 (endurecimento de alto impacto)

1. Substitua as ramificações `panic!` do gravador Kura por propagação de erro recuperável + sinalização de saúde degradada.
- Arquivos de destino: `crates/iroha_core/src/kura.rs`
- Aceitação:
  - falhas de anexação/index/fsync injetadas não entrem em pânico
  - erros surgem por meio de telemetria/registro e o gravador permanece controlável

2. Adicione verificações de envelope limitado para publicação TLV de host simulado IVM e caminhos de envelope JSON.
- Arquivos de destino: `crates/ivm/src/mock_wsv.rs`
- Aceitação:
  - cargas superdimensionadas são rejeitadas antes do processamento com muita alocação
  - novos testes cobrem casos superdimensionados de TLV e JSON

3. Aplique verificações explícitas de permissão do chamador para aliases de administrador JSON (ou aliases de portão atrás de sinalizadores de recursos estritos somente para teste e documente claramente).
- Arquivos de destino: `crates/ivm/src/mock_wsv.rs`
- Aceitação:
  - o chamador não autorizado não pode alterar a função/permissão/estado do par por meio de aliases

### Fase 2 (desempenho de caminho ativo)4. Torne a contabilidade orçamentária do Kura incremental.
- Substitua a recomputação completa da fila pendente por enfileiramento por contadores mantidos atualizados em enfileiramento/persistência/descarte.
- Aceitação:
  - custo de enfileiramento próximo a O(1) para cálculo de bytes pendentes
  - o benchmark de regressão mostra latência estável à medida que a profundidade pendente aumenta

5. Reduza o tempo de espera do bloqueio de despejo.
- Opções: compactação segmentada, cópia fragmentada com limites de liberação de bloqueio ou modo de manutenção em segundo plano com bloqueio limitado de primeiro plano.
- Aceitação:
  - a latência de despejo com histórico extenso diminui e as operações em primeiro plano permanecem responsivas

6. Encurte a seção crítica grosseira `view_lock` sempre que possível.
- Avalie a divisão de fases de confirmação ou a captura instantânea de deltas preparados para minimizar janelas de espera exclusivas.
- Aceitação:
  - métricas de contenção demonstram tempo de espera reduzido de 99p sob commits de blocos pesados

### Fase 3 (Guarda-corpos operacionais)

7. Introduzir sinalização de esteira limitada/coalescida para o escritor Kura e contrapressão/limites de fila lateral.
8. Expanda os painéis de telemetria para:
- Distribuições de espera/retenção `view_lock`
- duração da remoção e bytes recuperados por execução
- latência da fila de verificação de orçamento

## Adições de teste sugeridas1. `kura_writer_io_failures_do_not_panic` (unidade, injeção de falha)
2. `kura_budget_check_scales_with_pending_depth` (regressão de desempenho)
3. `kura_eviction_does_not_block_reads_beyond_threshold` (integração/desempenho)
4. `state_commit_view_lock_hold_under_heavy_world_commit` (regressão de contenção)
5. `state_apply_without_execution_handles_da_cursor_error_without_panic` (resiliência)
6. `mock_wsv_admin_alias_requires_permissions` (regressão de segurança)
7. `mock_wsv_input_publish_tlv_rejects_oversize` (proteção DoS)
8. `mock_wsv_checkpoint_restore_cost_regression` (benchmark de desempenho)

## Notas sobre escopo e confiança

- As descobertas para `crates/iroha_core/src/kura.rs` e `crates/iroha_core/src/state.rs` são descobertas do caminho de produção.
- As descobertas para `crates/ivm/src/mock_wsv.rs` têm escopo explicitamente de teste/desenvolvimento do host, de acordo com a documentação em nível de arquivo.
- Nenhuma alteração de versão da ABI é exigida por esta auditoria em si.