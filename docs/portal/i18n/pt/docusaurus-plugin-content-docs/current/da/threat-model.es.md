---
lang: pt
direction: ltr
source: docs/portal/docs/da/threat-model.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflexo `docs/source/da/threat_model.md`. Mantenha ambas as versões em
:::

# Modelo de amenazas de Data Availability de Sora Nexus

_Revisão final: 19/01/2026 -- Revisão Proxima programada: 19/04/2026_

Cadência de manutenção: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). Cada
revisão deve aparecer em `status.md` com links para tickets de mitigação de ativos
e artefatos de simulação.

## Propósito e alcance

O programa de Disponibilidade de Dados (DA) mantém transmissões Taikai, blobs de
pista Nexus e artefatos de governo recuperáveis antes de caírem bizantinas,
de rede e de operadores. Este modelo de amenas ancla o trabalho de engenharia
para DA-1 (arquitetura e modelo de amenazas) e sirve como linha de base para tareas
DA posteriores (DA-2 a DA-10).

Componentes dentro do alcance:
- Extensão de ingestão DA em Torii e escritores de metadados Norito.
- Arboles de armazenamento de blobs respaldados por SoraFS (camadas quente/frio) e
  políticas de replicação.
- Compromissos de blocos Nexus (formatos de fios, provas, APIs de cliente ligero).
- Hooks de aplicação PDP/PoTR específicos para payloads DA.
- Fluxos de operadores (pinning, eviction, slashing) e pipelines de
  observabilidade.
- Aprovações de governo que admitem ou expulsam operadores e conteúdo DA.

Fora de alcance para este documento:
- Modelado economicamente completo (capturado no fluxo de trabalho DA-7).
- Protocolos baseados em SoraFS e cobertos pelo modelo de ameaças de SoraFS.
- Ergonomia de SDK de clientes, mas com considerações de superfície de
  amenaza.

## Panorama arquitetônico

1. **Envio:** Os clientes enviam blobs por meio da API de ingestão DA de Torii. El
   nodo trocea blobs, codifica manifestos Norito (tipo de blob, lane, epoch,
   flags de codec), e armazena pedaços na camada quente de SoraFS.
2. **Anúncio:** Intents de pin e dicas de replicação são propagadas aos fornecedores
   de armazenamento via registro (SoraFS marketplace) com tags de política
   que indica objetivos de retenção de calor/frio.
3. **Compromisso:** Os sequenciadores Nexus incluem compromissos de blob (CID +
   root KZG opcional) no bloco canônico. Clientes leves dependem do
   hash de compromisso e metadados anunciados para verificar a disponibilidade.
4. **Replicação:** Nodos de armazenamento baixam ações/pedaços atribuídos,
   satisfazer desafios PDP/PoTR e promover dados entre níveis quentes e frios segun
   política.
5. **Recuperação:** Consumidores recuperam dados via SoraFS ou gateways DA-aware,
   verificando provas e levantando solicitações de reparação quando desaparece
   réplicas.
6. **Governança:** Parlamento e comitê de supervisão DA aprueban operadores,
   horários de aluguel e escalações de execução. Artefatos de governança se
   armazene pela misma ruta DA para garantir a transparência do processo.

## Ativos e responsáveis

Escala de impacto: **Critico** rompe seguridad/vivacidad del ledger; **Alto**
bloquear preenchimento DA o clientes; **Moderado** qualidade degradada, mas
recuperável; **Bajo** efeito limitado.

| Ativo | Descrição | Integridade | Disponibilidade | Confidencialidade | Responsável |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Blobs Taikai, pista e governo almacenados em SoraFS | Crítico | Crítico | Moderado | DA WG / Equipe de Armazenamento |
| Manifestos Norito DA | Metadados típicos que descrevem blobs | Crítico | alto | Moderado | GT de Protocolo Central |
| Compromissos de bloco | CIDs + raízes KZG dentro de blocos Nexus | Crítico | alto | Baixo | GT de Protocolo Central |
| Horários PDP/PoTR | Cadência de aplicação para réplicas DA | alto | alto | Baixo | Equipe de armazenamento |
| Registro de operadores | Provedores de armazenamento aprovados e políticos | alto | alto | Baixo | Conselho de Governança |
| Registros de aluguel e incentivos | Registro de entradas para aluguel e lucro DA | alto | Moderado | Baixo | GT Tesouraria |
| Painéis de observação | SLOs DA, profundidade de replicação, alertas | Moderado | alto | Baixo | SRE / Observabilidade |
| Intenções de reparação | Solicitações para rehidratar pedaços faltantes | Moderado | Moderado | Baixo | Equipe de armazenamento |

## Adversários e capacidades| Ator | Capacidades | Motivações | Notas |
| --- | --- | --- | --- |
| Cliente fraudulento | Enviar blobs malformados, replays de manifestos obsoletos, tentar DoS e ingerir. | Interrumpir transmite Taikai, inyectar dados inválidos. | Sem chaves privilegiadas. |
| Nodo de armazenamento bizantino | Soltar réplicas atribuídas, forjar provas PDP/PoTR, coludir com outras. | Reduzir a retenção DA, evitar aluguel, reter dados como rehens. | Obtenha credenciais válidas do operador. |
| Sequenciador comprometido | Omitir compromissos, blocos equívocos, reordenar metadados de blobs. | Ocultar envios DA, criar inconsistência. | Limitado pela prefeitura de consenso. |
| Operador interno | Abusar do acesso ao governo, manipular políticas de retenção, filtrar credenciais. | Ganância econômica, sabotagem. | Acesso a infraestructura quente/frio. |
| Adversário de vermelho | Particionar nós, atrasar a replicação, iniciar o tráfego MITM. | Reduzir a disponibilidade e degradar SLOs. | Não é possível romper TLS, mas é possível soltar/ralentizar links. |
| Atacante de observabilidade | Manipular dashboards/alertas, suprimir incidentes. | Ocultar caidas DA. | Requer acesso ao pipeline de telemetria. |

## Fronteiras de confiança

- **Fronteira de entrada:** Cliente na extensão DA de Torii. Requer autorização por
  solicitação, limitação de taxa e validação de carga útil.
- **Frontera de replicacion:** Nodos de armazenamento intercambian chunks y
  provas. Os nodos são autenticados mutuamente, mas podem se comportar de forma
  bizantina.
- **Fronteira do razão:** Dados de bloqueio comprometidos vs almacenamiento
  fora da cadeia. O consenso protege a integridade, mas requer disponibilidade
  aplicação fora da cadeia.
- **Fronteira de governo:** Decisões do Conselho/Parlamento que são aprovadas
  operadores, pressupostos e cortes. Fallas aqui impactam diretamente o
  despliegue DA.
- **Fronteira de observação:** Coleção de métricas/logs exportados para
  painéis/ferramentas de alerta. A manipulação oculta interrupções ou ataques.

## Cenários de ameaças e controles

### Ataques na rota de ingestão

**Cenário:** Cliente malicioso envia payloads Norito malformados o blobs
sobredimensionados para acumular recursos ou contrabandear metadados invalidados.

**Controles**
- Validação do esquema Norito com negociação restrita de versões; rechazar
  sinalizadores desconhecidos.
- Limitação de taxa e autenticação no endpoint de ingestão Torii.
- Limites de tamanho do bloco e codificação determinados pelo bloco SoraFS.
- O pipeline de admissão só persiste manifesta após coincidir com a soma de verificação de
  integridade.
- Cache de replay determinista (`ReplayCache`) rastrea ventanas `(lane, época,
  sequência)`, persistir marcas d’água na discoteca, e rechaza duplicados/replays
  obsoletos; aproveitamentos de propiedad y fuzz cubren impressões digitais divergentes y
  envio fora da ordem. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Brechas residuais**
- Torii ingest deve atualizar o cache de repetição na entrada e persistir os cursores
  de sequência a traves de reinicios.
- Os esquemas Norito DA agora têm um fuzz chicote dedicado
  (`fuzz/da_ingest_schema.rs`) para definir invariantes de codificação/decodificação; eles
  painéis de cobertura devem ser alertados se o alvo retornar.

### Retenção por retenção de replicação

**Cenário:** Operadores de armazenamento bizantinos aceitam atribuições de
pin pero sueltan chunks, pasando desafios PDP/PoTR via respuestas forjadas o
conluio.

**Controles**
- O cronograma de desafios PDP/PoTR se estende às cargas DA com cobertura por
  época.
- Replicação multifonte com limites de quorum; el fetch orquestrador detecta
  fragmentos faltantes e dispara reparação.
- Slashing de gobernanza garantido a provas falidas e réplicas faltantes.
- Trabalho de reconciliação automática (`cargo xtask da-commitment-reconcile`)
  comparar recibos de ingestão com compromissos DA (SignedBlockWire, `.norito` o
  JSON), emite um pacote JSON de evidência para governo e falha nos tickets anteriores
  faltantes ou não coincidentes para a página do Alertmanager por omissão/adulteração.**Brechas residuais**
- El chicote de simulação em `integration_tests/src/da/pdp_potr.rs` (cubierto
  por `integration_tests/tests/da/pdp_potr_simulation.rs`) agora ejetado
  cenários de conluio e participação, validando que o cronograma PDP/PoTR detecta
  um comportamento bizantino de forma determinista. Siga estendendo-o junto a
  DA-5 para descobrir novas superfícies de prova.
- A política de despejo da camada fria exige uma trilha de auditoria firmada para
  prevenir gotas encubiertos.

### Manipulação de compromissos

**Cenário:** Seqüenciador comprometido publicamente com bloqueios omitindo ou alterando
comprometimentos DA, causando falhas de busca ou inconsistências em clientes leves.

**Controles**
- El consenso cruza propostas de blocos com colas de envio DA; pares rechazan
  propostas sem compromissos exigidos.
- Clientes verificam levemente as provas de inclusão antes do expositor lidar com a busca.
- Trilha de auditoria comparando recibos de envio com compromissos de bloqueio.
- Trabalho de reconciliação automática (`cargo xtask da-commitment-reconcile`)
  comparar recibos de ingestão com compromissos DA (SignedBlockWire, `.norito` o
  JSON), emite um pacote JSON de evidência para governo e falha nos tickets anteriores
  faltantes ou não coincidentes para a página do Alertmanager por omissão/adulteração.

**Brechas residuais**
- Cubierto pelo trabalho de reconciliação + gancho do Alertmanager; os pacotes de
  governo agora injere o pacote JSON de evidência por defeito.

### Partição vermelha e censura

**Cenário:** O adversário particiona a rede de replicação, evitando que nós
obter pedaços atribuídos para responder aos desafios PDP/PoTR.

**Controles**
- Requisitos de provedores multi-regiões garantem caminhos de redes diversas.
- Ventanas de desafio incluem jitter e fallback em canais de reparação fuera
  de banda.
- Painéis de observação monitorados com profundidade de replicação, saída de
  desafios e latência de busca com guarda-chuvas de alerta.

**Brechas residuais**
- Faltam simulações de participação para eventos ao vivo de Taikai; você precisa
  testes de imersão.
- A política de reserva de âncora de banda de reparação não está codificada.

### Abuso interno

**Cenário:** Operador com acesso ao registro manipula políticas de retenção,
whitelistea provadores maliciosos ou alertas de surpresa.

**Controles**
- Ações de governo exigem firmas multipartidárias e registros Norito
  notarizados.
- Mudanças políticas emitem eventos para monitoramento e registros de arquivo.
- O pipeline de aplicação de observação registra Norito somente acréscimo com encadeamento de hash.
- A automatização de revisões de acesso trimestral
  (`cargo xtask da-privilege-audit`) recorre a diretórios de manifesto/replay
  (mas paths provistos por operadores), marca entradas faltantes/no diretório/
  gravável mundialmente e emite um pacote JSON firmado para painéis de controle.

**Brechas residuais**
- A evidência de violação nos painéis requer instantâneos firmados.

## Registro de riscos residuais

| Riesgo | Probabilidade | Impacto | Responsável | Plano de mitigação |
| --- | --- | --- | --- | --- |
| Repetir os manifestos DA antes de aterrissar o cache da sequência DA-2 | Possível | Moderado | GT de Protocolo Central | Implementar cache de sequência + validação de nonce em DA-2; agregar testes de regressão. |
| Conluio PDP/PoTR quando >f nodos se comprometem | Improvável | alto | Equipe de armazenamento | Derivar novo calendário de desafios com mapa cross-provider; validar via chicote de simulação. |
| Brecha de auditório e despejo de camada fria | Possível | alto | Equipe SRE/Armazenamento | Adjuntar registros firmados e recibos on-chain para despejos; monitorar via dashboards. |
| Latência de detecção de omissão de sequenciador | Possível | alto | GT de Protocolo Central | `cargo xtask da-commitment-reconcile` noturno compara recibos vs compromissos (SignedBlockWire/`.norito`/JSON) e página de governo antes de tickets faltantes ou não coincidentes. |
| Resiliência à partição para streams ao vivo de Taikai | Possível | Crítico | Rede TL | Executar exercícios de partição; reservar ancho de banda de reparação; documentar SOP de failover. |
| Deriva de privilégios de governo | Improvável | alto | Conselho de Governança | `cargo xtask da-privilege-audit` trimestral (dirs manifest/replay + paths extra) com JSON firmado + portão do painel; anunciar artefatos de auditoria on-chain. |

## Acompanhamentos necessários1. Publicar esquemas Norito de ingestão DA e vetores de exemplo (se vai para DA-2).
2. Abra o cache de repetição na ingestão DA de Torii e mantenha os cursores de
   sequência a traves de reinicios de nodos.
3. **Concluído (2026-02-05):** El chicote de simulação PDP/PoTR agora ejercita
   cenários de conluio + participação com modelagem de backlog QoS; ver
   `integration_tests/src/da/pdp_potr.rs` (con testes em
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para implementação e
   os currículos deterministas capturados abaixo.
4. **Concluído (2026-05-29):** `cargo xtask da-commitment-reconcile` comparar
   recibos de ingestão contra compromissos DA (SignedBlockWire/`.norito`/JSON),
   emite `artifacts/da/commitment_reconciliation.json`, e está conectado a
   Alertmanager/pacotes de governança para alertas de omissão/adulteração
   (`xtask/src/da.rs`).
5. **Concluído (2026-05-29):** `cargo xtask da-privilege-audit` retorna o carretel
   de manifest/replay (mas caminhos fornecidos pelos operadores), marca entradas
   faltantes/no diretório/world-writable, e produz um pacote JSON firmado para
   painéis/revisões de governo (`artifacts/da/privilege_audit.json`),
   cerrando a brecha de automatização de acesso.

**Onde mirar despues:**

- O cache de repetição e a persistência dos cursores foram aterrados no DA-2. Veja
  implementação em `crates/iroha_core/src/da/replay_cache.rs` (lógica de cache)
  e a integração Torii em `crates/iroha_torii/src/da/ingest.rs`, que completa os
  testes de impressão digital através de `/v1/da/ingest`.
- As simulações de streaming PDP/PoTR são executadas por meio do chicote proof-stream
  em `crates/sorafs_car/tests/sorafs_cli.rs`, recebendo fluxos de solicitação
  PoR/PDP/PoTR e cenários de falhas animadas no modelo de ameaças.
- Os resultados de capacidade e reparação absorvem viven en
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, enquanto a matriz de
  absorver Sumeragi mas amplia se rastrea en `docs/source/sumeragi_soak_matrix.md`
  (com variantes localizadas). Esses artefatos capturaram as brocas largas
  duração referenciada no registro de riscos residuais.
- A automatização de reconciliação + auditoria de privilégios vive em
  `docs/automation/da/README.md` e os novos comandos
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit`; usar
  saídas por defeito abaixo `artifacts/da/` ao lado da evidência de pacotes
  de governança.

## Evidência de simulação e QoS modelado (2026-02)

Para encerrar o acompanhamento DA-1 #3, codificamos um chicote de simulação PDP/PoTR

determinista baixo `integration_tests/src/da/pdp_potr.rs` (cubierto por
`integration_tests/tests/da/pdp_potr_simulation.rs`). El arnês atribui nodos

em três regiões, inyecta particiones/conluio após as probabilidades do
roadmap, rastrea tardanza PoTR, e alimenta um modelo de backlog de reparação
que reflete o pressuposto de reparação da camada quente. Executar o cenário
por defeito (12 épocas, 18 desafios PDP + 2 janelas PoTR por época) produto
as seguintes métricas:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Fallas PDP bloqueadas | 48/49 (98,0%) | Las particiones aun deteccion disparan; Uma única falha não detectada é o jitter honesto. |
| Latência média de detecção PDP | 0,0 épocas | Las fallas aparecem dentro da época de origem. |
| Fallas PoTR bloqueadas | 28/77 (36,4%) | A detecção é ativada quando um nó perfura >=2 janelas PoTR, deixando a maioria dos eventos no registro de riscos residuais. |
| Latência média de detecção PoTR | Épocas 2.0 | Coincide com o umbral de atraso de duas épocas incorporadas na escalada de arquivo. |
| Pico de cola de reparação | 38 manifestos | O backlog é díspar quando as partições são apiladas mais rapidamente do que as quatro reparações disponíveis por época. |
| Latência de resposta p95 | 30.068ms | Reflita a janela do desafio de 30 s com jitter de +/-75 ms aplicado para mostrar QoS. |
<!-- END_DA_SIM_TABLE -->

Esses resultados agora alimentam os protótipos de painéis DA e satisfazem
os critérios de aceitação de "chinês de simulação + modelagem de QoS" referidos

no roteiro.

A automatização agora vive depois de
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
que chama o chicote compartilhado e emite Norito JSON a
`artifacts/da/threat_model_report.json` por defeito. Empregos noturnos consumidos
este arquivo para atualizar as matrizes neste documento e alertar antecipadamente

em tarefas de detecção, colas de reparação ou exibições de QoS.

Para atualizar a tabela inicial para documentos, execute `make docs-da-threat-model`,
que invoca `cargo xtask da-threat-model-report`, regenera
`docs/source/da/_generated/threat_model_report.json`, e reescrever esta seção
através de `scripts/docs/render_da_threat_model_tables.py`. O espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) é atualizado no mesmo passo para que

ambas as cópias são mantidas sincronizadas.