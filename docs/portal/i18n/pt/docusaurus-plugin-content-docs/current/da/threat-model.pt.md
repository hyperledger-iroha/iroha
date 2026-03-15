---
lang: pt
direction: ltr
source: docs/portal/docs/da/threat-model.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Espelha `docs/source/da/threat_model.md`. Mantenha as duas versoes em
:::

# Modelo de ameacas de Data Availability da Sora Nexus

_Revisão final: 19/01/2026 -- Revisão programada: 19/04/2026_

Cadência de manutenção: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). Cada revisão

deve aparecer em `status.md` com links para tickets de mitigação ativa e
artefatos de simulação.

## Propósito e escopo

O programa de Disponibilidade de Dados (DA) mantém transmissões de Taikai, blobs de lane
Nexus e arquitetos de governança recuperáveis sob falhas bizantinas, de rede e de
operadores. Este modelo de ameacas ancora o trabalho de engenharia para DA-1
(arquitetura e modelo de ameacas) e serve como linha de base para tarefas DA
posteriores (DA-2 a DA-10).

Componentes no escopo:
- Extensão de ingestão DA do Torii e gravadores de metadados Norito.
- Árvores de armazenamento de blobs suportadas por SoraFS (camadas quente/frio) e
  políticas de replicação.
- Compromissos de blocos Nexus (formatos de fios, provas, APIs de nível de cliente).
- Hooks de aplicação PDP/PoTR específicos para payloads DA.
- Workflows de operadores (pinning, eviction, slashing) e pipelines de
  observabilidade.
- Aprovações de governança que admitem ou removem operadores e conteúdo DA.

Fora do escopo deste documento:
- Modelagem econômica completa (capturada no fluxo de trabalho DA-7).
- Protocolos base SoraFS e cobertores pelo modelo de ameacas SoraFS.
- Ergonomia de SDK de cliente além de considerações de superfície de ameaca.

##Visão arquitetônica

1. **Submissão:** Clientes enviam blobs via API de ingestão DA do Torii. Ó
   node divide blobs, codifica manifestos Norito (tipo de blob, lane, epoch, flags
   de codec), e armazena pedaços no nível quente do SoraFS.
2. **Anúncio:** Pin intents e dicas de replicação de propagação para provedores de
   armazenamento via registro (SoraFS marketplace) com tags de política que indicam
   metas de retenção quente/frio.
3. **Compromisso:** Sequenciadores Nexus incluem compromissos de blobs (CID + raízes
   KZG adicional) no bloco canônico. Clientes leves dependem do hash de
   compromisso e dos metadados anunciados para verificar a disponibilidade.
4. **Replicacao:** Nodos de armazenamento puxam share/chunks atribuídos, atendem
   PDP/PoTR, desafios e promovem dados entre níveis quentes e frios conforme política.
5. **Buscar:** Consumidores buscam dados via SoraFS ou gateways DA-aware,
   verificando provas e emitindo pedidos de reparo quando as réplicas desaparecerem.
6. **Governanca:** Parlamento e o comitê de supervisão DA aprovam operadores,
   horários de aluguel e escalações de execução. Artefatos de governança de são
   armazenados pela rota DA para garantir a mesma transparência do processo.

## Ativos e responsáveis

Escala de impacto: **Critico** quebra segurança/vivacidade do ledger; **Alto**

bloquear backfill DA ou clientes; **Moderado** degrada qualidade mas permanece
recuperável; **Baixo** efeito limitado.

| Ativo | Descrição | Integridade | Disponibilidade | Confidencialidade | Responsável |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Blobs Taikai, lane e governança armazenados em SoraFS | Crítico | Crítico | Moderado | DA WG / Equipe de Armazenamento |
| Manifestos Norito DA | Metadados tipados descrevendo blobs | Crítico | alto | Moderado | GT de Protocolo Central |
| Compromissos do bloco | CIDs + raízes KZG dentro de blocos Nexus | Crítico | alto | Baixa | GT de Protocolo Central |
| Horários PDP/PoTR | Cadência de aplicação para réplicas DA | alto | alto | Baixa | Equipe de armazenamento |
| Registro de operadores | Fornecedores de armazenamento aprovados e políticos | alto | alto | Baixa | Conselho de Governança |
| Registros de aluguel e incentivos | Registro de entradas para aluguel DA e deliberações | alto | Moderado | Baixa | GT Tesouraria |
| Dashboards de observabilidade | SLOs DA, profundidade de replicação, alertas | Moderado | alto | Baixa | SRE / Observabilidade |
| Intenções de reparo | Pedidos para reidratar pedaços ausentes | Moderado | Moderado | Baixa | Equipe de armazenamento |

## Adversários e Capacidades| Ator | Capacidades | Motivações | Notas |
| --- | --- | --- | --- |
| Cliente fraudulento | Submeter blobs malformados, replay de manifestos antigos, tente DoS no ingest. | Interromper transmite Taikai, injetar dados inválidos. | Sem chaves privilegiadas. |
| Nodo de armazenamento bizantino | Descarte as réplicas atribuídas, forjar provas PDP/PoTR, coludir. | Reduzir a retenção DA, evitar aluguel, reter dados. | Possui credenciais válidas de operador. |
| Sequenciador comprometido | Omitir compromissos, blocos equívocos, reordenar metadados de blobs. | Ocultar submissão DA, criar inconsistência. | Limitado pela maioria de consenso. |
| Operador interno | Abusar do acesso de governança, manipular políticas de retenção, vazar credenciais. | Ganho econômico, sabotagem. | Acesso a infraestrutura quente/frio. |
| Adversário de rede | Particionar nós, atrasar replicação, injetar tráfego MITM. | Reduzir a disponibilidade e degradar SLOs. | Não quebre o TLS, mas você pode descartar/atrasar links. |
| Atacante de observabilidade | Manipular dashboards/alertas, suprimir incidentes. | Interrupções ocultas DA. | Solicite acesso ao pipeline de telemetria. |

##Fronteiras de confiança

- **Fronteira de ingresso:** Cliente para extensão DA do Torii. Solicitar autenticação por
  request, rate limiting e validação de payload.
- **Fronteira de replicação:** Nodos de armazenamento trocam pedaços e provas. Os nós
  se autenticam mutuamente, mas podem se comportar de forma bizantina.
- **Fronteira do ledger:** Dados de bloco comprometidos vs armazenamento off-chain.
  O consenso garante integridade, mas a disponibilidade requer aplicação fora da cadeia.
- **Fronteira de governança:** Decisões do Conselho/Parlamento aprovando operadores,
  orcamentos e slashing. Falhas aqui impactam diretamente o deploy de DA.
- **Fronteira de observabilidade:** Coleta de métricas/logs exportada para
  painéis/ferramentas de alerta. A adulteração esconde interrupções ou ataques.

## Cenários de ameaca e controles

### Ataques no caminho da ingestão

**Cenário:** Cliente malicioso submete payloads Norito malformados ou blobs
superdimensionados para examinar recursos ou inserir metadados inválidos.

**Controles**
- Validação do esquema Norito com negociação estrita de versão; rejeitar bandeiras
  desconhecidos.
- Rate limiting e autenticação no endpoint de ingestão Torii.
- Limites de tamanho do chunk e codificação determinística forcos pelo chunker SoraFS.
- Pipeline de admissão para persistir manifestos após checksum de integridade
  coincidir.
- Replay cache determinista (`ReplayCache`) rastreia janelas `(lane, epoch,
  sequência)`, persistem marcas d'água em disco, e duplicados/replays
  obsoletos; chicotes de propriedade e fuzz cobrem impressões digitais divergentes e
  envios fora de ordem. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuais**
- Torii ingest deve encadear o replay cache na admissão e persistir os cursores de
  sequência durante os reinícios.
- Esquemas Norito DA agora possuem um fuzz chicote dedicado
  (`fuzz/da_ingest_schema.rs`) para estressar invariantes de codificação/decodificação; sistema operacional
  dashboards de cobertura devem alertar se o alvo se regredir.

### Retenção por retenção de replicação

**Cenário:** Operadores de armazenamento bizantinos aceitam pins mas dropam chunks,
passando desafios PDP/PoTR via respostas forjadas ou colusão.

**Controles**
- O cronograma de desafios PDP/PoTR se estende a payloads DA com cobertura por
  época.
- Replicação multi-source com limiares de quorum; o orquestrador detecta fragmentos
  faltantes e dispara reparo.
- Corte de governança vinculado a provas de falhas e réplicas faltantes.
- Job de reconciliação automática (`cargo xtask da-commitment-reconcile`) que
  comparar recibos de ingestão com compromissos DA (SignedBlockWire, `.norito` ou
  JSON), emite bundle JSON de evidência para governança, e falha em tickets
  faltantes ou divergentes para a página do Alertmanager por omissão/adulteração.

**Lacunas residuais**
- O chicote de simulação em `integration_tests/src/da/pdp_potr.rs` (coberto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) exercício de colusão e
  particao, validando que o cronograma PDP/PoTR detecta comportamento bizantino
  de forma determinística. Continue estendendo junto com DA-5 para cobrir novas
  superfícies de prova.
- Uma política de despejo cold-tier requer trilha de auditoria assinada para evitar quedas
  encobertos.

### Manipulação de compromissos**Cenário:** Sequenciador comprometido com blocos públicos omitindo ou alterando
compromissos DA, causando falhas de busca ou inconsistências nos níveis dos clientes.

**Controles**
- Consenso cruza propostas de bloco com filas de submissão DA; pares rejeitam
  propostas sem compromissos exigidos.
- Os clientes precisam verificar as provas de inclusão antes de exportar os identificadores de busca.
- Trilha de auditoria comparando recibos de submissão com compromissos de bloco.
- Job de reconciliação automatizada (`cargo xtask da-commitment-reconcile`) que
  comparar recibos de ingestão com compromissos DA (SignedBlockWire, `.norito` ou
  JSON), emite bundle JSON de evidência para governança, e falha em tickets
  faltantes ou divergentes para a página do Alertmanager por omissão/adulteração.

**Lacunas residuais**
- Coberto pelo job de reconciliação + hook Alertmanager; pacotes de governança
  agora insira o pacote JSON de evidência por padrão.

### Partição de rede e censura

**Cenário:** Adversário participa da rede de replicação, impedindo nós de
obter pedaços atribuídos ou responder aos desafios PDP/PoTR.

**Controles**
- Requisitos de provedores multirregionais garantindo caminhos de rede diversos.
- Janelas de desafio incluem jitter e fallback para canais de reparo fora de
  banda.
- Dashboards de observabilidade monitoram profundidade de replicação, sucesso de
  desafios e latência de busca com limites de alerta.

**Lacunas residuais**
- Simulações de participação para eventos Taikai live ainda faltam; são necessários
  testes de imersão.
- Política de reserva de banda de reparo ainda não está codificada.

### Abuso interno

**Cenário:** Operador com acesso ao registro manipula políticas de retenção,
whitelist de provedores maliciosos, ou alertas surpresa.

**Controles**
- Ações de governança requerem assinaturas multipartidárias e registros Norito
  notarizados.
- Mudanças de política emitem eventos para monitoramento e logs de arquivo.
- Pipeline de observabilidade aplica logs Norito somente acréscimo com encadeamento de hash.
- A automação de revisão trimestral (`cargo xtask da-privilege-audit`) percorre
  diretórios de manifest/replay (mais caminhos fornecidos pelos operadores), marca
  entradas faltantes/nao diretório/world-writable, e emite bundle JSON assinado
  para dashboards de governança.

**Lacunas residuais**
- Evidência de adulteração em dashboards requer snapshots assinados.

## Registro de riscos residuais

| Risco | Probabilidade | Impacto | Proprietário | Plano de mitigação |
| --- | --- | --- | --- | --- |
| Replay de manifestos DA antes do cache de sequência DA-2 | Possível | Moderado | GT de Protocolo Central | Implementar cache de sequência + validação de nonce em DA-2; adicionar testes de regressão. |
| Colusão PDP/PoTR quando >f nós são comprometidos | Melhorar | alto | Equipe de armazenamento | Derivar novo cronograma de desafios com amostragem cross-provedor; validar via chicote de simulação. |
| Lacuna de auditorias de despejo cold-tier | Possível | alto | Equipe SRE/Armazenamento | Anexar registra contratos assinados e recibos on-chain para despejos; Monitore via dashboards. |
| Latência de detecção de omissão de sequenciador | Possível | alto | GT de Protocolo Central | `cargo xtask da-commitment-reconcile` noturno compara recibos vs compromissos (SignedBlockWire/`.norito`/JSON) e página governanca em tickets faltantes ou divergentes. |
| Resiliência a participação para transmissões ao vivo de Taikai | Possível | Crítico | Rede TL | Executar treinos de partida; reservar banda de reparo; documentar SOP de failover. |
| Deriva de privilégios de governança | Melhorar | alto | Conselho de Governança | `cargo xtask da-privilege-audit` trimestral (dirs manifest/replay + paths extras) com JSON recebido + gate de dashboard; ancorar artistas de auditoria on-chain. |

## Acompanhamentos necessários1. Publicar esquemas Norito de ingestão DA e vetores de exemplo (carregados em
   DA-2).
2. Encadeie o cache de repetição na ingestão DA do Torii e persista os cursores de
   sequência durante reinícios de nós.
3. **Concluído (2026-02-05):** O chicote de simulação PDP/PoTR agora exercita
   colaboração + participação com modelagem de backlog QoS; veja
   `integration_tests/src/da/pdp_potr.rs` (com testes em
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para a implementação e
   resumos deterministas capturados abaixo.
4. **Concluído (2026-05-29):** `cargo xtask da-commitment-reconcile` comparar
   recibos de ingestão com compromissos DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, e está ligado a
   Alertmanager/pacotes de governança para alertas de omissão/adulteração
   (`xtask/src/da.rs`).
5. **Concluído (2026-05-29):** `cargo xtask da-privilege-audit` percorre o spool
   de manifest/replay (mais caminhos fornecidos pelos operadores), marca entradas
   faltantes/nao diretorio/world-writable, e produz pacote JSON assinado para
   dashboards/revisões de governança (`artifacts/da/privilege_audit.json`),
   fechando a lacuna de automação de acesso.

**Onde olhar a seguir:**

- O replay cache e a persistência de cursores aterrissaram em DA-2. Veja a
  implementação em `crates/iroha_core/src/da/replay_cache.rs` (lógica do cache)
  e a integração Torii em `crates/iroha_torii/src/da/ingest.rs`, que encadeia verificações de
  impressão digital via `/v2/da/ingest`.
- As simulações de streaming PDP/PoTR são exercitadas via o aproveitamento proof-stream
  em `crates/sorafs_car/tests/sorafs_cli.rs`, cobrindo fluxos de requisição
  PoR/PDP/PoTR e cenários de falhas animadas no modelo de ameacas.
- Resultados de capacidade e reparação absorver vivem em
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, enquanto a matriz de imersão
  Sumeragi mais amplo e acompanhado em `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas incluídas). Esses artistas capturaram os exercícios de longa duração
  duração referenciada no registro de riscos residuais.
- A automação de reconciliação + auditoria de privilégio vive em
  `docs/automation/da/README.md` e nossos comandos
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit`; usar
  as saidas padrão sob `artifacts/da/` ao fixação evidência a pacotes de pacotes
  governança.

## Evidência de simulação e modelagem QoS (2026-02)

Para fechar o acompanhamento DA-1 #3, codificamos um chicote de simulação PDP/PoTR
determinista sob `integration_tests/src/da/pdp_potr.rs` (coberto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). O aproveite os nós aloca em
três regiões, injetar particoes/colusao conforme as probabilidades do roadmap,
acompanha lateness PoTR, e alimenta um modelo de backlog de reparo que reflete o
orcamento de reparo do tier hot. Rodar o cenário padrão (12 épocas, 18 desafios
PDP + 2 janelas PoTR por época) produzidas com as seguintes métricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Falhas PDP bloqueadas | 48/49 (98,0%) | Partições ainda disparam detecção; uma falha não bloqueada vem de jitter honesto. |
| Latência média de detecção PDP | 0,0 épocas | Falhas surgem dentro da época de origem. |
| Falhas PoTR bloqueadas | 28/77 (36,4%) | Detecção dispara quando um nó perde >=2 janelas PoTR, deixando a maioria dos eventos no registro de riscos residuais. |
| Latência média de detecção PoTR | Épocas 2.0 | Corresponde aos limites de atraso de duas épocas embutidos na escalada de arquivo. |
| Pico da fila de reparo | 38 manifestos | O backlog cresce quando as partículas se acumulam mais rapidamente que quatro reparos disponíveis por época. |
| Latência de resposta p95 | 30.068ms | Reflete a janela do desafio de 30 s com jitter de +/-75 ms aplicado no sampling QoS. |
<!-- END_DA_SIM_TABLE -->

Essas saídas agora alimentam os protótipos de dashboard DA e satisfazem os
critérios de aceitação de "simulation chicote + QoS modelling" referenciados
nenhum roteiro.

A automação agora vive por trás de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que chama o chicote compartilhado e emite Norito JSON para
`artifacts/da/threat_model_report.json` por padrão. Trabalhos noturnos consomem este
arquivo para atualizar as matrizes neste documento e alertar sobre deriva em

taxas de detecção, filas de reparo ou amostras de QoS.

Para atualizar a tabela acima para docs, execute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, e reescreva esta estação via
`scripts/docs/render_da_threat_model_tables.py`. O espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) e atualize no mesmo passo para que
as duas cópias ficam em sincronia.