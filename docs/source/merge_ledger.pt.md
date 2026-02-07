---
lang: pt
direction: ltr
source: docs/source/merge_ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44f1c681730f1c94d9d00e8f829a0134374ce6cb29f21727a27685e096f0da40
source_last_modified: "2026-01-18T05:31:56.955438+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Merge Ledger Design – Finalidade da pista e redução global

Esta nota finaliza o design do livro-razão de mesclagem para o Milestone 5. Ela explica o
política de bloco não vazio, semântica de mesclagem de controle de qualidade entre pistas e fluxo de trabalho de finalidade
que vincula a execução em nível de pista ao compromisso global do estado mundial.

O design estende a arquitetura Nexus descrita em `nexus.md`. Termos como
"lane block", "lane QC", "merge dica" e "merge ledger" herdam seus
definição desse documento; esta nota se concentra em regras comportamentais e
orientação de implementação que deve ser aplicada pelo tempo de execução, armazenamento e WSV
camadas.

## 1. Política de bloqueio não vazio

**Regra (OBRIGATÓRIA):** Um proponente de pista emite um bloco somente quando o bloco contém pelo menos
pelo menos um fragmento de transação executada, gatilho baseado em tempo ou determinístico
atualização de artefato (por exemplo, acúmulo de artefato DA). Blocos vazios são proibidos.

**Implicações:**

- Slot keep-alive: quando nenhuma transação atende sua janela determinística de commit,
a pista não emite nenhum bloqueio e simplesmente avança para o próximo slot. O livro de mesclagem
permanece na dica anterior para essa pista.
- Disparo em lote: gatilhos em segundo plano que não produzem transição de estado (por exemplo,
cron que reafirma invariantes) são considerados vazios e DEVEM ser ignorados ou
empacotado com outro trabalho antes de produzir um bloco.
- Telemetria: `pipeline_detached_merged` e tratamento de métricas de acompanhamento ignorado
slots explicitamente – os operadores podem distinguir "sem trabalho" de "pipeline paralisado".
- Replay: o armazenamento em bloco não insere espaços reservados sintéticos vazios. O Kura
o loop de repetição simplesmente observa o mesmo hash pai para slots consecutivos, se não
bloco foi emitido.

**Verificação canônica:** Durante a proposta e validação do bloco, `ValidBlock::commit`
afirma que o `StateBlock` associado carrega pelo menos uma sobreposição confirmada
(delta, artefato, gatilho). Isso se alinha com a proteção `StateBlock::is_empty`
isso já garante que as gravações não operacionais sejam eliminadas. A aplicação acontece antes
assinaturas são solicitadas para que os comitês nunca votem em cargas vazias.

## 2. Semântica de mesclagem de controle de qualidade entre pistas

Cada bloco de pista `B_i` finalizado por seu comitê produz:

- `lane_state_root_i`: Compromisso Poseidon2-SMT sobre raízes estaduais por DS tocadas
no bloco.
- `merge_hint_root_i`: candidato contínuo para o razão de mesclagem (`tag =
"iroha:merge:candidato:v1\0"`).
- `lane_qc_i`: assinaturas agregadas do comitê de pista ao longo do
  pré-imagem de votação de execução (hash de bloco, `parent_state_root`,
  `post_state_root`, altura/visualização/época, chain_id e tag de modo).

Os nós de mesclagem coletam as dicas mais recentes `{(B_i, lane_qc_i, merge_hint_root_i)}` para
todas as pistas `i ∈ [0, K)`.

**Mesclar entrada (DEVE):**

```
MergeLedgerEntry {
    epoch_id: u64,
    lane_tips: [Hash32; K],
    merge_hint_root: [Hash32; K],
    global_state_root: Hash32,
    merge_qc: QuorumCertificate,
}
```- `lane_tips[i]` é o hash da pista que bloqueia os selos de entrada de mesclagem para a pista
  `i`. Se uma pista não emitiu nenhum bloco desde a entrada de mesclagem anterior, esse valor será
  repetido.
- `merge_hint_root[i]` é o `merge_hint_root` da pista correspondente
  bloco. É repetido quando `lane_tips[i]` se repete.
- `global_state_root` é igual a `ReduceMergeHints(merge_hint_root[0..K-1])`, um
  Dobra Poseidon2 com tag de separação de domínio
  `"iroha:merge:reduce:v1\0"`. A redução é determinística e DEVE
  reconstruir o mesmo valor entre pares.
- `merge_qc` é um certificado de quorum BFT do comitê de fusão sobre o
  entrada serializada.

**Mesclar carga útil de controle de qualidade (OBRIGATÓRIA):**

Os membros do comitê de fusão assinam um resumo determinístico:

```
merge_qc_digest = blake2b32(
    "iroha:merge:qc:v1\0" ||
    chain_id ||
    norito(MergeLedgerSignPayload {
        view,
        epoch_id,
        lane_tips,
        merge_hint_roots,
        global_state_root,
    })
)
```

- `view` é a visualização do comitê de fusão derivada das dicas de pista (máx.
  `view_change_index` através dos cabeçalhos da pista selados pela entrada).
- `chain_id` é a string do identificador de cadeia configurada (UTF-8 bytes).
- A carga útil usa a codificação Norito com a ordem dos campos mostrada acima.

O resumo resultante é armazenado em `merge_qc.message_digest` e é a mensagem
verificado por assinaturas BLS.

**Mesclar construção de controle de qualidade (DEVE):**

- A lista do comitê de mesclagem é o conjunto atual de validadores de topologia de commit.
- Quórum necessário = `commit_quorum_from_len(roster_len)`.
- `merge_qc.signers_bitmap` codifica os índices validadores participantes (LSB primeiro)
  em ordem de topologia de confirmação.
- `merge_qc.aggregate_signature` é o agregado normal do BLS para o resumo
  acima.

**Validação (OBRIGATÓRIA):**

1. Verifique cada `lane_qc_i` em relação a `lane_tips[i]` e confirme os cabeçalhos do bloco
   inclua o `merge_hint_root_i` correspondente.
2. Certifique-se de que nenhum `lane_qc_i` aponte para um `Invalid` ou bloco não executado. O
   a política não vazia acima garante que o cabeçalho inclua sobreposições de estado.
3. Recalcule `ReduceMergeHints` e compare com `global_state_root`.
4. Recalcule o resumo do QC de mesclagem e verifique o bitmap do signatário, o limite de quorum,
   e assinatura agregada na lista de topologias de commit.

**Observabilidade:** Os nós de mesclagem emitem contadores Prometheus para
`merge_entry_lane_repeats_total{i}` para destacar pistas que pularam slots para
visibilidade operacional.

## 3. Fluxo de trabalho de finalidade

### 3.1 Finalidade no nível da pista

1. As transações são agendadas por via em slots determinísticos.
2. O executor aplica sobreposições em `StateBlock`, produzindo deltas e
artefatos.
3. Após a validação, o comitê de pista assina a pré-imagem de votação de execução que
   vincula o hash do bloco, raízes de estado e altura/visualização/época. A tupla
   `(block_hash, lane_qc_i, merge_hint_root_i)` é considerado o final da pista.
4. Clientes leves PODEM tratar a ponta da pista como final para provas limitadas pelo DS, mas
deve registrar o `merge_hint_root` associado para reconciliar com o razão de mesclagem
mais tarde.Os comitês de pista são por espaço de dados e não substituem o commit global
topologia. O tamanho do comitê é fixado em `3f+1`, onde `f` vem do
catálogo de espaço para dados (`fault_tolerance`). O pool de validadores é o dataspace
validadores (manifestos de governança de pista para pistas gerenciadas pelo administrador ou vias públicas
registros de piquetagem para pistas eleitas por estaca). A participação no comitê é
amostrado deterministicamente uma vez por época usando a semente da época VRF ligada com
`dataspace_id` e `lane_id`. Se o pool for menor que `3f+1`, finalidade da pista
faz uma pausa até que o quorum seja restaurado (a recuperação de emergência é tratada separadamente).

### 3.2 Finalidade do Merge-Ledger

1. O comitê de mesclagem coleta as dicas de pista mais recentes, verifica cada `lane_qc_i` e
constrói o `MergeLedgerEntry` conforme definido acima.
2. Após verificação da redução determinística, o comitê de fusão assina o
entrada (`merge_qc`).
3. Os nós acrescentam a entrada ao log do razão de mesclagem e a persistem junto com o
referências de blocos de pista.
4. `global_state_root` torna-se o compromisso oficial do Estado mundial para o
época/slot. Nós completos atualizam seus metadados de ponto de verificação WSV para espelhar isso
valor; a repetição determinística deve reproduzir a mesma redução.

### 3.3 WSV e integração de armazenamento

- `State::commit_merge_entry` registra as raízes de estado por pista e o
  final `global_state_root`, conectando a execução da pista com a soma de verificação global.
- Kura persiste `MergeLedgerEntry` adjacente aos artefatos do bloco de pista para que um
  a repetição pode reconstruir sequências de finalização global e em nível de pista.
- Quando uma pista pula uma vaga, o armazenamento simplesmente retém a ponta anterior; não
  entradas de mesclagem de espaço reservado são criadas até que pelo menos uma pista produza um novo
  bloco.
- Superfícies API (Torii, telemetria) expõem dicas de pista e a mesclagem mais recente
  entrada para que operadores e clientes possam conciliar visualizações globais e por faixa.

## 4. Notas de implementação- `crates/iroha_core/src/state.rs`: `State::commit_merge_entry` valida o
  redução e conecta os metadados da pista/globais ao estado mundial, para que as consultas
  e os observadores podem acessar as dicas de mesclagem e o hash global oficial.
- `crates/iroha_core/src/kura.rs`: `Kura::store_block_with_merge_entry` enfileiramentos
  o bloco e persiste a entrada de mesclagem associada em uma única etapa, revertendo
  o bloco na memória quando o acréscimo falha, então o armazenamento nunca registra um bloco
  sem seus metadados de vedação. O log do livro-razão de mesclagem é removido em etapa de bloqueio
  com a altura do bloco validada durante a recuperação de inicialização e armazenado em cache na memória
  com uma janela limitada (`kura.merge_ledger_cache_capacity`, padrão 256) para
  evite o crescimento ilimitado em nós de longa duração. A recuperação trunca parcial ou
  entradas finais do livro-razão de mesclagem superdimensionadas e anexar entradas rejeitadas acima do
  proteção de tamanho máximo de carga útil para limitar as alocações.
- `crates/iroha_core/src/block.rs`: validação de bloco rejeita blocos sem
  pontos de entrada (transações externas ou gatilhos de tempo) e sem determinística
  artefatos como pacotes DA (`BlockValidationError::EmptyBlock`), garantindo
  a política não vazia é aplicada antes que as assinaturas sejam solicitadas e transportadas
  no livro de mesclagem.
- O auxiliar de redução determinística reside no serviço de mesclagem: `reduce_merge_hint_roots`
  (`crates/iroha_core/src/merge.rs`) implementa a dobra Poseidon2 descrita acima.
  Os ganchos de aceleração de hardware continuam sendo um trabalho futuro, mas o caminho escalar agora impõe
  a redução canônica deterministicamente.
- Integração de telemetria: expondo repetições de mesclagem por pista e o
  O medidor `global_state_root` permanece rastreado no backlog de observabilidade para que o
  o trabalho do painel pode ser fornecido junto com a implementação do serviço de mesclagem.
- Testes de componentes cruzados: a cobertura de repetição dourada para a redução de mesclagem é
  rastreado com o backlog de teste de integração para garantir mudanças futuras
  `reduce_merge_hint_roots` mantém as raízes registradas estáveis.