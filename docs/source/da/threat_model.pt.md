---
lang: pt
direction: ltr
source: docs/source/da/threat_model.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0bff91e735291e82d0d50b5dad4dfbf2b57af68f2f7067760add5da81fc7f554
source_last_modified: "2026-01-19T07:28:06.298292+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Modelo de ameaça à disponibilidade de dados Sora Nexus

_Última revisão: 19/01/2026 — Próxima revisão agendada: 19/04/2026_

Cadência de manutenção: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). Toda revisão deve
aparecem em `status.md` com links para tickets de mitigação ativos e artefatos de simulação.

## Objetivo e Escopo

O programa Data Availability (DA) mantém transmissões Taikai, blobs de pista Nexus e
artefatos de governança recuperáveis sob falhas bizantinas, de rede e de operadora.
Este modelo de ameaça ancora o trabalho de engenharia para DA-1 (arquitetura e modelo de ameaça)
e serve como linha de base para tarefas DA downstream (DA-2 a DA-10).

Componentes no escopo:
- Extensão de ingestão Torii DA e gravadores de metadados Norito.
- Árvores de armazenamento de blob apoiadas por SoraFS (camadas quentes/frias) e políticas de replicação.
- Compromissos de bloco Nexus (formatos de transmissão, provas, APIs de cliente leve).
- Ganchos de aplicação de PDP/PoTR específicos para cargas DA.
- Fluxos de trabalho do operador (fixação, despejo, corte) e pipelines de observabilidade.
- Aprovações de governança que admitem ou expulsam operadores e conteúdo de DA.

Fora do escopo deste documento:
- Modelagem econômica completa (capturada no fluxo de trabalho DA-7).
- Protocolos base SoraFS já cobertos pelo modelo de ameaça SoraFS.
- Ergonomia do Client SDK além das considerações de superfície de ameaça.

## Visão Geral da Arquitetura

1. **Envio:** os clientes enviam blobs por meio da API de ingestão Torii DA. O nó
   pedaços de blobs, codifica manifestos Norito (tipo de blob, faixa, época, sinalizadores de codec),
   e armazena pedaços na camada SoraFS quente.
2. **Anúncio:** intenções de fixação e dicas de replicação se propagam para o armazenamento
   provedores por meio do registro (mercado SoraFS) com tags de política que
   indicar metas de retenção quente/frio.
3. **Compromisso:** Os sequenciadores Nexus incluem compromissos de blob (CID + KZG opcional
   raízes) no bloco canônico. Os clientes Light contam com o hash de compromisso e
   metadados anunciados para verificar a disponibilidade.
4. **Replicação:** Nós de armazenamento extraem compartilhamentos/pedaços atribuídos, satisfazem PDP/PoTR
   desafios e promover dados entre níveis quentes e frios por política.
5. **Busca:** Os consumidores buscam dados por meio de gateways SoraFS ou compatíveis com DA, verificando
   provas e levantando solicitações de reparo quando as réplicas desaparecem.
6. **Governança:** O Parlamento e o comitê de supervisão do DA aprovam operadores,
   cronogramas de aluguel e escalonamentos de fiscalização. Os artefatos de governança são armazenados
   através do mesmo caminho DA para garantir a transparência do processo. Os parâmetros de aluguel
   rastreados no DA-7 são registrados em `docs/source/da/rent_policy.md`, portanto, as auditorias
   e as revisões de aplicação podem fazer referência aos valores exatos de XOR aplicados por blob.

## Ativos e Proprietários

Escala de impacto: **Crítico** quebra a segurança/vivacidade do livro-razão; **Alto** bloqueia DA
preenchimento ou clientes; **Moderado** degrada a qualidade, mas permanece recuperável;
**Baixo** efeito limitado.| Ativo | Descrição | Integridade | Disponibilidade | Confidencialidade | Proprietário |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Taikai, pista, blobs de governança armazenados em SoraFS | Crítico | Crítico | Moderado | DA WG / Equipe de Armazenamento |
| Manifestos DA Norito | Metadados digitados que descrevem blobs | Crítico | Alto | Moderado | GT de Protocolo Central |
| Bloquear compromissos | CIDs + raízes KZG dentro de blocos Nexus | Crítico | Alto | Baixo | GT de Protocolo Central |
| Cronogramas PDP/PoTR | Cadência de aplicação para réplicas DA | Alto | Alto | Baixo | Equipe de armazenamento |
| Cadastro de operadores | Provedores e políticas de armazenamento aprovados | Alto | Alto | Baixo | Conselho de Governança |
| Registros de aluguel e incentivos | Lançamentos contábeis para aluguel e multas de DA | Alto | Moderado | Baixo | GT Tesouraria |
| Painéis de observabilidade | SLOs DA, profundidade de replicação, alertas | Moderado | Alto | Baixo | SRE / Observabilidade |
| Intenções de reparação | Pedidos para reidratar pedaços perdidos | Moderado | Moderado | Baixo | Equipe de armazenamento |

## Adversários e Capacidades

| Ator | Capacidades | Motivações | Notas |
| --- | --- | --- | --- |
| Cliente malicioso | Envie blobs malformados, reproduza manifestos obsoletos, tente DoS na ingestão. | Interrompa as transmissões do Taikai, injete dados inválidos. | Sem chaves privilegiadas. |
| Nó de armazenamento bizantino | Abandone réplicas atribuídas, forje provas PDP/PoTR, conspire com outras pessoas. | Reduza a retenção de DA, evite aluguel, mantenha dados como reféns. | Possui credenciais de operador válidas. |
| Sequenciador comprometido | Omita compromissos, equivoque-se em blocos, reordene metadados de blob. | Oculte envios de DA, crie inconsistência. | Limitado pela maioria de consenso. |
| Operador interno | Abusar do acesso à governança, adulterar políticas de retenção, vazar credenciais. | Ganho econômico, sabotagem. | Acesso à infraestrutura de nível quente/frio. |
| Adversário da rede | Nós de partição, atrasam a replicação, injetam tráfego MITM. | Reduza a disponibilidade e degrade os SLOs. | Não é possível quebrar o TLS, mas pode interromper/retardar links. |
| Atacante de observabilidade | Adulterar painéis/alertas, suprimir incidentes. | Ocultar interrupções do DA. | Requer acesso ao pipeline de telemetria. |

## Limites de confiança

- **Limite de entrada:** Cliente para extensão DA Torii. Requer autenticação em nível de solicitação,
  limitação de taxa e validação de carga útil.
- **Limite de replicação:** Nós de armazenamento trocando pedaços e provas. Nós são
  mutuamente autenticados, mas podem se comportar como bizantinos.
- **Limite do razão:** Dados de bloco confirmados versus armazenamento fora da cadeia. Guardas de consenso
  integridade, mas a disponibilidade requer aplicação fora da cadeia.
- **Limite de governação:** Decisões do Conselho/Parlamento que aprovam operadores,
  orçamentos e cortes. As quebras aqui impactam diretamente a implantação do DA.
- **Limite de observabilidade:** Coleta de métricas/logs exportada para painéis/alertas
  ferramentas. A adulteração esconde interrupções ou ataques.

## Cenários e controles de ameaças

### Ingerir ataques de caminho**Cenário:** cliente malicioso envia cargas Norito malformadas ou superdimensionadas
blobs para esgotar recursos ou contrabandear metadados inválidos.

**Controles**
- Validação de esquema Norito com negociação rigorosa de versão; rejeitar bandeiras desconhecidas.
- Limitação de taxa e autenticação no endpoint de ingestão Torii.
- Limites de tamanho de bloco e codificação determinística imposta pelo chunker SoraFS.
- O pipeline de admissão só persiste nos manifestos após a soma de verificação de integridade corresponder.
- Cache de repetição determinístico (`ReplayCache`) rastreia janelas `(lane, epoch, sequence)`, persiste marcas d’água altas no disco e rejeita duplicatas/replays obsoletos; os chicotes de propriedade e fuzz cobrem impressões digitais divergentes e envios fora de ordem.【crates/iroha_core/src/da/replay_cache.rs:1】【fuzz/da_replay_cache.rs:1】【crates/iroha_torii/src/da/ingest.rs:1】

**Lacunas residuais**
- A ingestão Torii deve encadear o cache de repetição na admissão e persistir os cursores de sequência nas reinicializações.
- Os esquemas DA Norito agora possuem um chicote de fuzz dedicado (`fuzz/da_ingest_schema.rs`) para enfatizar invariantes de codificação/decodificação; os painéis de cobertura devem alertar se a meta regredir.

### Retenção de replicação

**Cenário:** Operadores de armazenamento bizantinos aceitam atribuições de pinos, mas descartam pedaços,
superar os desafios do PDP/PoTR através de respostas forjadas ou conluio.

**Controles**
- O cronograma de desafio PDP/PoTR se estende às cargas DA com cobertura por época.
- Replicação multifonte com limites de quorum; buscar orquestrador detecta
  fragmentos perdidos e reparo de gatilhos.
- Redução da governação associada a provas falhadas e réplicas em falta.

**Lacunas residuais**
- Chicote de simulação em `integration_tests/src/da/pdp_potr.rs` (coberto por
  `integration_tests/tests/da/pdp_potr_simulation.rs`) agora exerce conluio
  e cenários de partição, validando que o cronograma PDP/PoTR detecta
  Comportamento bizantino deterministicamente. Continue estendendo-o ao lado do DA-5 para
  cobrir novas superfícies de prova.
- A política de despejo de nível frio exige uma trilha de auditoria assinada para evitar quedas encobertas.

### Adulteração de compromisso

**Cenário:** Sequenciador comprometido publica blocos omitindo ou alterando DA
compromissos, causando falhas de busca ou inconsistências de clientes leves.

**Controles**
- Consenso cruza propostas de blocos com filas de submissão de DA; colegas rejeitam
  propostas que faltam aos compromissos exigidos.
- Os clientes Light verificam as provas de inclusão de compromisso antes de exibir identificadores de busca.
- Trilha de auditoria comparando recebimentos de submissões com compromissos de bloco.
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) compara
  ingerir recibos com compromissos DA (SignedBlockWire, `.norito` ou JSON),
  emite um pacote de evidências JSON para governança e falha ao perder ou
  tickets incompatíveis para que o Alertmanager possa paginar sobre omissão/adulteração.

**Lacunas residuais**
- Coberto pelo trabalho de reconciliação + gancho Alertmanager; pacotes de governança agora
  ingerir o pacote de evidências JSON por padrão.

### Partição de rede e censura**Cenário:** O adversário particiona a rede de replicação, impedindo que os nós
obter pedaços atribuídos ou responder aos desafios do PDP/PoTR.

**Controles**
- Os requisitos do provedor multirregional garantem diversos caminhos de rede.
- As janelas de desafio incluem jitter e fallback para canais de reparo fora de banda.
- Painéis de observabilidade monitoram a profundidade da replicação, desafiam o sucesso e
  buscar latência com limites de alerta.

**Lacunas residuais**
- Ainda faltam simulações de partição para eventos ao vivo de Taikai; preciso de testes de imersão.
- Reparar a política de reserva de largura de banda ainda não codificada.

### Abuso interno

**Cenário:** Operador com acesso ao registro manipula políticas de retenção,
coloca provedores maliciosos na lista de permissões ou suprime alertas.

**Controles**
- As ações de governança exigem assinaturas multipartidárias e registros autenticados Norito.
- Mudanças de política emitem eventos para monitoramento e arquivamento de logs.
- O pipeline de observabilidade impõe logs Norito somente anexados com encadeamento de hash.
- Caminhadas de automação de revisão de acesso trimestral (`cargo xtask da-privilege-audit`)
  os diretórios de manifesto/replay do DA (mais caminhos fornecidos pelo operador), sinalizadores
  entradas ausentes/não-diretório/graváveis mundialmente e emite um pacote JSON assinado
  para painéis de governança.

**Lacunas residuais**
- A evidência de violação do painel requer instantâneos assinados.

## Registro de Risco Residual

| Risco | Probabilidade | Impacto | Proprietário | Plano de Mitigação |
| --- | --- | --- | --- | --- |
| Reprodução de manifestos DA antes que o cache da sequência DA-2 chegue | Possível | Moderado | GT de Protocolo Central | Implementar cache de sequência + validação de nonce em DA-2; adicione testes de regressão. |
| Conluio PDP/PoTR quando >f nós comprometem | Improvável | Alto | Equipe de armazenamento | Obtenha um novo cronograma de desafios com amostragem entre fornecedores; validar via chicote de simulação. |
| Lacuna na auditoria de despejo nas camadas frias | Possível | Alto | Equipe SRE/Armazenamento | Anexe registros de auditoria assinados e recibos na rede para despejos; monitorar por meio de painéis. |
| Latência de detecção de omissão do sequenciador | Possível | Alto | GT de Protocolo Central | Nightly `cargo xtask da-commitment-reconcile` compara recibos versus compromissos (SignedBlockWire/`.norito`/JSON) e governança de páginas em tickets ausentes ou incompatíveis. |
| Resiliência de partição para transmissões ao vivo do Taikai | Possível | Crítico | Rede TL | Executar exercícios de partição; reserva de largura de banda de reparo; SOP de failover de documento. |
| Desvio de privilégio de governança | Improvável | Alto | Conselho de Governança | Execução `cargo xtask da-privilege-audit` trimestral (diretórios de manifesto/reprodução + caminhos extras) com JSON assinado + portão do painel; artefatos de auditoria âncora na cadeia. |

## Acompanhamentos necessários1. Publicar esquemas Norito de ingestão de DA e vetores de exemplo (transportados para DA-2).
2. Passe o cache de reprodução por meio da ingestão de DA Torii e persista os cursores de sequência nas reinicializações do nó.
3. **Concluído (05/02/2026):** O chicote de simulação PDP/PoTR agora exercita cenários de conluio + partição com modelagem de backlog de QoS; consulte [`integration_tests/src/da/pdp_potr.rs`](/integration_tests/src/da/pdp_potr.rs) (com testes em `integration_tests/tests/da/pdp_potr_simulation.rs`) para a implementação e resumos determinísticos capturados abaixo.
4. **Concluído (2026-05-29):** `cargo xtask da-commitment-reconcile` compara recibos de ingestão com compromissos DA (SignedBlockWire/`.norito`/JSON), emite `artifacts/da/commitment_reconciliation.json` e é conectado a pacotes Alertmanager/governance para alertas de omissão/adulteração (`xtask/src/da.rs`).
5. **Concluído (2026-05-29):** `cargo xtask da-privilege-audit` percorre o spool de manifesto/reprodução (mais caminhos fornecidos pelo operador), sinaliza entradas ausentes/não-diretório/graváveis ​​mundialmente e produz um pacote JSON assinado para painéis/revisões de governança (`artifacts/da/privilege_audit.json`), fechando a lacuna de automação de revisão de acesso.

**Onde procurar em seguida:**

- O cache de repetição do DA e a persistência do cursor chegaram ao DA-2. Veja o
  implementação em `crates/iroha_core/src/da/replay_cache.rs` (lógica de cache) e
  a integração Torii em `crates/iroha_torii/src/da/ingest.rs`, que encadeia o
  verificações de impressões digitais por meio de `/v1/da/ingest`.
- Simulações de streaming PDP/PoTR são exercidas por meio do equipamento de fluxo de prova em
  `crates/sorafs_car/tests/sorafs_cli.rs`, cobrindo fluxos de solicitação PoR/PDP/PoTR
  e cenários de falha animados no modelo de ameaça.
- Os resultados de absorção de capacidade e reparo permanecem abaixo
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, enquanto o mais amplo
  A matriz de imersão Sumeragi é rastreada em `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas incluídas). Esses artefatos capturam os exercícios de longa duração
  referenciado no registro de risco residual.
- Reconciliação + automação de auditoria de privilégios reside em
  `docs/automation/da/README.md` e o novo `cargo xtask da-commitment-reconcile`
  /`cargo xtask da-privilege-audit` comandos; use as saídas padrão em
  `artifacts/da/` ao anexar evidências a pacotes de governança.

## Evidência de simulação e modelagem de QoS (2026-02)

Para encerrar o acompanhamento nº 3 do DA-1, codificamos uma simulação determinística de PDP/PoTR
chicote sob `integration_tests/src/da/pdp_potr.rs` (coberto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). O arnês
aloca nós em três regiões, injeta partições/conluio de acordo com
as probabilidades do roteiro, rastreia o atraso do PoTR e alimenta um backlog de reparos
modelo que reflete o orçamento de reparos de nível quente. Executando o cenário padrão
(12 épocas, 18 desafios PDP + 2 janelas PoTR por época) produziu o
seguintes métricas:<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Falhas de PDP detectadas | 48/49 (98,0%) | As partições ainda acionam a detecção; uma única falha não detectada vem de um tremor honesto. |
| PDP significa latência de detecção | 0,0 épocas | As falhas surgem na época de origem. |
| Falhas PoTR detectadas | 28/77 (36,4%) | A detecção é acionada quando um nó perde ≥2 janelas PoTR, deixando a maioria dos eventos no registro de risco residual. |
| PoTR significa latência de detecção | Épocas 2.0 | Corresponde ao limite de atraso de duas épocas incorporado ao escalonamento de arquivamento. |
| Pico na fila de reparos | 38 manifestos | O backlog aumenta quando as partições se acumulam mais rápido do que os quatro reparos disponíveis por época. |
| Latência de resposta p95 | 30.068ms | Espelha a janela de desafio de 30 s com o jitter de ±75 ms aplicado para amostragem de QoS. |
<!-- END_DA_SIM_TABLE -->

Essas saídas agora impulsionam os protótipos do painel DA e satisfazem a “simulação
aproveitamento + modelagem de QoS” referenciados no roteiro.

A automação agora vive por trás do `cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`, que chama o chicote compartilhado e
emite Norito JSON para `artifacts/da/threat_model_report.json` por padrão. Todas as noites
jobs consomem este arquivo para atualizar as matrizes neste documento e para alertar sobre
desvio nas taxas de detecção, filas de reparo ou amostras de QoS.

Para atualizar a tabela acima para documentos, execute `make docs-da-threat-model`, que
invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json` e reescreve esta seção
através de `scripts/docs/render_da_threat_model_tables.py`. O espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) é atualizado na mesma passagem para que ambos
as cópias permanecem sincronizadas.