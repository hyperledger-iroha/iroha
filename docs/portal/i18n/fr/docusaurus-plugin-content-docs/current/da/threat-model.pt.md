---
lang: fr
direction: ltr
source: docs/portal/docs/da/threat-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Espelha `docs/source/da/threat_model.md`. Mantenha as duas versoes em
:::

# Modelo de ameacas de Data Availability da Sora Nexus

_Ultima revisao: 2026-01-19 -- Proxima revisao programada: 2026-04-19_

Cadencia de manutencao: Data Availability Working Group (<=90 dias). Cada revisao

deve aparecer em `status.md` com links para tickets de mitigacao ativos e
artefatos de simulacao.

## Proposito e escopo

O programa de Data Availability (DA) mantem transmissoes Taikai, blobs de lane
Nexus e artefatos de governanca recuperaveis sob falhas bizantinas, de rede e de
operadores. Este modelo de ameacas ancora o trabalho de engenharia para DA-1
(arquitetura e modelo de ameacas) e serve como baseline para tarefas DA
posteriores (DA-2 a DA-10).

Componentes no escopo:
- Extensao de ingestao DA do Torii e writers de metadata Norito.
- Arvores de armazenamento de blobs suportadas por SoraFS (tiers hot/cold) e
  politicas de replicacao.
- Commitments de blocos Nexus (wire formats, proofs, APIs de cliente leve).
- Hooks de enforcement PDP/PoTR especificos para payloads DA.
- Workflows de operadores (pinning, eviction, slashing) e pipelines de
  observabilidade.
- Aprovacoes de governanca que admitem ou removem operadores e conteudo DA.

Fora do escopo deste documento:
- Modelagem economica completa (capturada no workstream DA-7).
- Protocolos base SoraFS ja cobertos pelo modelo de ameacas SoraFS.
- Ergonomia de SDK de cliente alem de consideracoes de superficie de ameaca.

## Visao arquitetural

1. **Submissao:** Clientes submetem blobs via a API de ingestao DA do Torii. O
   node divide blobs, codifica manifests Norito (tipo de blob, lane, epoch, flags
   de codec), e armazena chunks no tier hot do SoraFS.
2. **Anuncio:** Pin intents e hints de replicacao propagam para provedores de
   storage via registry (SoraFS marketplace) com tags de politica que indicam
   metas de retencao hot/cold.
3. **Commitment:** Sequenciadores Nexus incluem commitments de blobs (CID + roots
   KZG opcionais) no bloco canonico. Clientes leves dependem do hash de
   commitment e da metadata anunciada para verificar availability.
4. **Replicacao:** Nodos de armazenamento puxam shares/chunks atribuidos, atendem
   desafios PDP/PoTR, e promovem dados entre tiers hot e cold conforme politica.
5. **Fetch:** Consumidores buscam dados via SoraFS ou gateways DA-aware,
   verificando proofs e emitindo pedidos de reparo quando replicas desaparecem.
6. **Governanca:** Parlamento e o comite de supervisao DA aprovam operadores,
   schedules de rent e escalacoes de enforcement. Artefatos de governanca sao
   armazenados pela mesma rota DA para garantir transparencia do processo.

## Ativos e responsaveis

Escala de impacto: **Critico** quebra seguranca/vivacidade do ledger; **Alto**

bloqueia backfill DA ou clientes; **Moderado** degrada qualidade mas permanece
recuperavel; **Baixo** efeito limitado.

| Ativo | Descricao | Integridade | Disponibilidade | Confidencialidade | Responsavel |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (chunks + manifests) | Blobs Taikai, lane e governanca armazenados em SoraFS | Critico | Critico | Moderado | DA WG / Storage Team |
| Manifests Norito DA | Metadata tipada descrevendo blobs | Critico | Alto | Moderado | Core Protocol WG |
| Commitments de bloco | CIDs + roots KZG dentro de blocos Nexus | Critico | Alto | Baixo | Core Protocol WG |
| Schedules PDP/PoTR | Cadencia de enforcement para replicas DA | Alto | Alto | Baixo | Storage Team |
| Registry de operadores | Provedores de storage aprovados e politicas | Alto | Alto | Baixo | Governance Council |
| Registros de rent e incentivos | Entradas ledger para rent DA e penalidades | Alto | Moderado | Baixo | Treasury WG |
| Dashboards de observabilidade | SLOs DA, profundidade de replicacao, alertas | Moderado | Alto | Baixo | SRE / Observability |
| Intents de reparo | Pedidos para reidratar chunks ausentes | Moderado | Moderado | Baixo | Storage Team |

## Adversarios e capacidades

| Ator | Capacidades | Motivacoes | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Submeter blobs malformados, replay de manifests antigos, tentar DoS no ingest. | Interromper broadcasts Taikai, injetar dados invalidos. | Sem chaves privilegiadas. |
| Nodo de armazenamento bizantino | Drop de replicas atribuidas, forjar proofs PDP/PoTR, coludir. | Reduzir retencao DA, evitar rent, reter dados. | Possui credenciais validas de operador. |
| Sequenciador comprometido | Omitir commitments, equivocar blocos, reordenar metadata de blobs. | Ocultar submissao DA, criar inconsistencia. | Limitado pela maioria de consenso. |
| Operador interno | Abusar acesso de governanca, manipular politicas de retencao, vazar credenciais. | Ganho economico, sabotagem. | Acesso a infraestrutura hot/cold. |
| Adversario de rede | Particionar nodes, atrasar replicacao, injetar trafego MITM. | Reduzir availability, degradar SLOs. | Nao quebra TLS mas pode dropar/atrasar links. |
| Atacante de observabilidade | Manipular dashboards/alertas, suprimir incidentes. | Ocultar outages DA. | Requer acesso ao pipeline de telemetria. |

## Fronteiras de confianca

- **Fronteira de ingress:** Cliente para extensao DA do Torii. Requer auth por
  request, rate limiting e validacao de payload.
- **Fronteira de replicacao:** Nodos de storage trocam chunks e proofs. Os nodes
  se autenticam mutuamente mas podem se comportar de forma bizantina.
- **Fronteira do ledger:** Dados de bloco commitados vs storage off-chain.
  Consenso garante integridade, mas availability requer enforcement off-chain.
- **Fronteira de governanca:** Decisoes Council/Parliament aprovando operadores,
  orcamentos e slashing. Falhas aqui impactam diretamente o deploy de DA.
- **Fronteira de observabilidade:** Coleta de metrics/logs exportada para
  dashboards/alert tooling. Tampering esconde outages ou ataques.

## Cenarios de ameaca e controles

### Ataques no caminho de ingestao

**Cenario:** Cliente malicioso submete payloads Norito malformados ou blobs
superdimensionados para exaurir recursos ou inserir metadata invalida.

**Controles**
- Validacao de schema Norito com negociacao estrita de versao; rejeitar flags
  desconhecidos.
- Rate limiting e autenticacao no endpoint de ingestao Torii.
- Limites de chunk size e encoding deterministico forcos pelo chunker SoraFS.
- Pipeline de admissao so persiste manifests apos checksum de integridade
  coincidir.
- Replay cache determinista (`ReplayCache`) rastreia janelas `(lane, epoch,
  sequence)`, persiste high-water marks em disco, e rejeita duplicados/replays
  obsoletos; harnesses de propriedade e fuzz cobrem fingerprints divergentes e
  envios fora de ordem. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuais**
- Torii ingest deve encadear o replay cache na admissao e persistir cursores de
  sequence durante reinicios.
- Schemas Norito DA agora possuem um fuzz harness dedicado
  (`fuzz/da_ingest_schema.rs`) para estressar invariantes de encode/decode; os
  dashboards de cobertura devem alertar se o target regredir.

### Retencao por withholding de replicacao

**Cenario:** Operadores de storage bizantinos aceitam pins mas dropam chunks,
passando desafios PDP/PoTR via respostas forjadas ou colusao.

**Controles**
- O schedule de desafios PDP/PoTR se estende a payloads DA com cobertura por
  epoch.
- Replicacao multi-source com thresholds de quorum; o orchestrator detecta shards
  faltantes e dispara reparo.
- Slashing de governanca vinculado a proofs falhas e replicas faltantes.
- Job de reconciliacao automatizado (`cargo xtask da-commitment-reconcile`) que
  compara receipts de ingestao com commitments DA (SignedBlockWire, `.norito` ou
  JSON), emite bundle JSON de evidencia para governanca, e falha em tickets
  faltantes ou divergentes para que Alertmanager pagine por omission/tampering.

**Lacunas residuais**
- O harness de simulacao em `integration_tests/src/da/pdp_potr.rs` (coberto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) exercita colusao e
  particao, validando que o schedule PDP/PoTR detecta comportamento bizantino
  de forma deterministica. Continue estendendo junto com DA-5 para cobrir novas
  superficies de proof.
- A politica de eviction cold-tier requer audit trail assinado para evitar drops
  encobertos.

### Manipulacao de commitments

**Cenario:** Sequenciador comprometido publica blocos omitindo ou alterando
commitments DA, causando falhas de fetch ou inconsistencias em clientes leves.

**Controles**
- Consenso cruza propostas de bloco com filas de submissao DA; peers rejeitam
  propostas sem commitments requeridos.
- Clientes leves verificam inclusion proofs antes de expor handles de fetch.
- Audit trail comparando receipts de submissao com commitments de bloco.
- Job de reconciliacao automatizado (`cargo xtask da-commitment-reconcile`) que
  compara receipts de ingestao com commitments DA (SignedBlockWire, `.norito` ou
  JSON), emite bundle JSON de evidencia para governanca, e falha em tickets
  faltantes ou divergentes para que Alertmanager pagine por omission/tampering.

**Lacunas residuais**
- Coberto pelo job de reconciliacao + hook Alertmanager; pacotes de governanca
  agora ingerem o bundle JSON de evidencia por default.

### Particao de rede e censura

**Cenario:** Adversario particiona a rede de replicacao, impedindo nodes de
obter chunks atribuidos ou responder a desafios PDP/PoTR.

**Controles**
- Requisitos de providers multi-region garantem caminhos de rede diversos.
- Janelas de desafio incluem jitter e fallback para canais de reparo fora de
  banda.
- Dashboards de observabilidade monitoram profundidade de replicacao, sucesso de
  desafios e latencia de fetch com thresholds de alerta.

**Lacunas residuais**
- Simulacoes de particao para eventos Taikai live ainda faltam; sao necessarios
  soak tests.
- Politica de reserva de banda de reparo ainda nao esta codificada.

### Abuso interno

**Cenario:** Operador com acesso ao registry manipula politicas de retencao,
whitelist de providers maliciosos, ou suprime alertas.

**Controles**
- Acoes de governanca requerem assinaturas multi-party e registros Norito
  notarizados.
- Mudancas de politica emitem eventos para monitoring e logs de arquivo.
- Pipeline de observabilidade aplica logs Norito append-only com hash chaining.
- A automacao de revisao trimestral (`cargo xtask da-privilege-audit`) percorre
  diretorios de manifest/replay (mais paths fornecidos por operadores), marca
  entradas faltantes/nao diretorio/world-writable, e emite bundle JSON assinado
  para dashboards de governanca.

**Lacunas residuais**
- Evidencia de tamper em dashboards requer snapshots assinados.

## Registro de riscos residuais

| Risco | Probabilidade | Impacto | Owner | Plano de mitigacao |
| --- | --- | --- | --- | --- |
| Replay de manifests DA antes do sequence cache DA-2 | Possivel | Moderado | Core Protocol WG | Implementar sequence cache + validacao de nonce em DA-2; adicionar testes de regressao. |
| Colusao PDP/PoTR quando >f nodes sao comprometidos | Improvavel | Alto | Storage Team | Derivar novo schedule de desafios com sampling cross-provider; validar via harness de simulacao. |
| Gap de auditoria de eviction cold-tier | Possivel | Alto | SRE / Storage Team | Anexar logs assinados e receipts on-chain para evictions; monitorar via dashboards. |
| Latencia de deteccao de omissao de sequenciador | Possivel | Alto | Core Protocol WG | `cargo xtask da-commitment-reconcile` noturno compara receipts vs commitments (SignedBlockWire/`.norito`/JSON) e pagina governanca em tickets faltantes ou divergentes. |
| Resiliencia a particao para streams Taikai live | Possivel | Critico | Networking TL | Executar drills de particao; reservar banda de reparo; documentar SOP de failover. |
| Deriva de privilegios de governanca | Improvavel | Alto | Governance Council | `cargo xtask da-privilege-audit` trimestral (dirs manifest/replay + paths extras) com JSON assinado + gate de dashboard; ancorar artefatos de auditoria on-chain. |

## Follow-ups requeridos

1. Publicar schemas Norito de ingestao DA e vetores de exemplo (carregado em
   DA-2).
2. Encadear o replay cache na ingestao DA do Torii e persistir cursores de
   sequence durante reinicios de nodes.
3. **Concluido (2026-02-05):** O harness de simulacao PDP/PoTR agora exercita
   colusao + particao com modelagem de backlog QoS; veja
   `integration_tests/src/da/pdp_potr.rs` (com tests em
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para a implementacao e
   resumos deterministas capturados abaixo.
4. **Concluido (2026-05-29):** `cargo xtask da-commitment-reconcile` compara
   receipts de ingestao com commitments DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, e esta ligado a
   Alertmanager/pacotes de governanca para alertas de omission/tampering
   (`xtask/src/da.rs`).
5. **Concluido (2026-05-29):** `cargo xtask da-privilege-audit` percorre o spool
   de manifest/replay (mais paths fornecidos por operadores), marca entradas
   faltantes/nao diretorio/world-writable, e produz bundle JSON assinado para
   dashboards/revisoes de governanca (`artifacts/da/privilege_audit.json`),
   fechando a lacuna de automacao de acesso.

**Onde olhar a seguir:**

- O replay cache e a persistencia de cursores aterrissaram em DA-2. Veja a
  implementacao em `crates/iroha_core/src/da/replay_cache.rs` (logica do cache)
  e a integracao Torii em `crates/iroha_torii/src/da/ingest.rs`, que encadeia checks de
  fingerprint via `/v1/da/ingest`.
- As simulacoes de streaming PDP/PoTR sao exercitadas via o harness proof-stream
  em `crates/sorafs_car/tests/sorafs_cli.rs`, cobrindo fluxos de requisicao
  PoR/PDP/PoTR e cenarios de falha animados no modelo de ameacas.
- Resultados de capacity e repair soak vivem em
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, enquanto a matriz de soak
  Sumeragi mais ampla e acompanhada em `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas incluidas). Esses artefatos capturam os drills de longa
  duracao referenciados no registro de riscos residuais.
- A automacao de reconciliacao + privilege-audit vive em
  `docs/automation/da/README.md` e nos comandos
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; use
  as saidas padrao sob `artifacts/da/` ao anexar evidencia a pacotes de
  governanca.

## Evidencia de simulacao e modelagem QoS (2026-02)

Para fechar o follow-up DA-1 #3, codificamos um harness de simulacao PDP/PoTR
determinista sob `integration_tests/src/da/pdp_potr.rs` (coberto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). O harness aloca nodes em
tres regioes, injeta particoes/colusao conforme as probabilidades do roadmap,
acompanha lateness PoTR, e alimenta um modelo de backlog de reparo que reflete o
orcamento de reparo do tier hot. Rodar o cenario default (12 epochs, 18 desafios
PDP + 2 janelas PoTR por epoch) produziu as seguintes metricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metrica | Valor | Notas |
| --- | --- | --- |
| Falhas PDP detectadas | 48 / 49 (98.0%) | Particoes ainda disparam deteccao; uma falha nao detectada vem de jitter honesto. |
| Latencia media de deteccao PDP | 0.0 epochs | Falhas surgem dentro do epoch de origem. |
| Falhas PoTR detectadas | 28 / 77 (36.4%) | Deteccao dispara quando um node perde >=2 janelas PoTR, deixando a maioria dos eventos no registro de riscos residuais. |
| Latencia media de deteccao PoTR | 2.0 epochs | Corresponde ao limiar de atraso de dois epochs embutido na escalacao de arquivo. |
| Pico da fila de reparo | 38 manifests | Backlog cresce quando particoes empilham mais rapido que quatro reparos disponiveis por epoch. |
| Latencia de resposta p95 | 30,068 ms | Reflete a janela de desafio de 30 s com jitter de +/-75 ms aplicado no sampling QoS. |
<!-- END_DA_SIM_TABLE -->

Esses outputs agora alimentam os prototipos de dashboard DA e satisfazem os
criterios de aceitacao de "simulation harness + QoS modelling" referenciados
no roadmap.

A automacao agora vive por tras de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que chama o harness compartilhado e emite Norito JSON para
`artifacts/da/threat_model_report.json` por default. Jobs noturnos consomem este
arquivo para atualizar as matrizes neste documento e alertar sobre deriva em

taxas de deteccao, filas de reparo ou samples QoS.

Para atualizar a tabela acima para docs, execute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, e reescreve esta secao via
`scripts/docs/render_da_threat_model_tables.py`. O espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) e atualizado no mesmo passo para que
as duas copias fiquem em sync.
