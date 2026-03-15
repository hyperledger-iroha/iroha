---
lang: pt
direction: ltr
source: docs/portal/docs/da/threat-model.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Reflete `docs/source/da/threat_model.md`. Gardez les deux versões sincronizadas
:::

# Modelo de ameaça à disponibilidade de dados Sora Nexus

_Revisão final: 19/01/2026 - Plano de revisão Prochaine: 19/04/2026_

Cadência de manutenção: Grupo de Trabalho de Disponibilidade de Dados (<=90 dias). Chaque
revisão doit apparaitre dans `status.md` com des liens vers les tickets de
ativos de mitigação e artefatos de simulação.

## Mas e perímetro

O programa Disponibilidade de Dados (DA) mantém as transmissões Taikai, os blobs
Nexus pista e os artefatos de governo recuperáveis sob as falhas
bizantinos, reseau et operaur. Este modelo de ameaças antes do trabalho
engenharia para DA-1 (arquitetura e modelo de ameaças) e sert de base
para os taches DA seguintes (DA-2 a DA-10).

Compostos no escopo:
- Extensão de ingestão DA Torii e gravadores de metadados Norito.
- Arbres de armazenamento de blobs incluídos em SoraFS (camadas quentes/frias) e políticas de
  replicação.
- Compromissos do bloco Nexus (formatos de fios, provas, APIs light-client).
- Ganchos de aplicação de PDP/PoTR específicos para cargas úteis DA.
- Operador de fluxos de trabalho (fixação, despejo, corte) e pipelines
  d'observabilidade.
- Aprovações de governo que admitem ou expulsam os operadores e
  conteúdo DA.

Fora do escopo deste documento:
- Modelização econômica completa (capturada no fluxo de trabalho DA-7).
- Os protocolos base SoraFS deixam de cobrir o modelo de ameaças SoraFS.
- Ergonomia dos clientes SDK além das considerações de superfície de ameaça.

## Vista do conjunto arquitetônico

1. **Envio:** Os clientes enviam blobs por meio da API de ingestão DA Torii.
   Le noeud decupe os blobs, codifique os manifestos Norito (tipo de blob, lane,
   época, sinalizadores de codec) e armazene os pedaços na camada quente SoraFS.
2. **Anúncio:** Intenções de pinos e dicas de replicação são propagadas para eles
   provedores via registro (mercado SoraFS) com tags de política aqui
   indique os objetivos de retenção quente/frio.
3. **Compromisso:** Os sequenciadores Nexus incluem compromissos de blobs (CID +
   racines KZG optionnelles) no bloco canônico. Os clientes leves se baseiam
   no hash de compromisso e nos metadados anunciados para verificação
   a disponibilidade.
4. **Replicação:** As noeuds de stockage cansam as ações/pedaços atribuídos,
   satisfaça os desafios do PDP/PoTR e promova os donnees entre níveis quentes
   e frio selon la politique.
5. **Fetch:** Os usuários recuperam os dados via SoraFS ou des gateways
   Ciente da DA, verifique as provas e emita as demandas de reparação quando
   des réplicas disparaissent.
6. **Governança:** Le Parlement et le comite de supervision DA approuvent les
   operadores, cronogramas de aluguel e escalações de execução. Os artefatos de
   a governança é mantida por meio do meme voie DA para garantir a transparência.

## Ativos e proprietários

Índice de impacto: **Crítica** casse la securite/vivacite du ledger; **Eleve**
bloquear o preenchimento de DA ou clientes; **Modere** degrada la qualidade mais reste
recuperável; **Fácil** limite de efeito.

| Ativo | Descrição | Integridade | Disponibilidade | Confidencialidade | Proprietário |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (pedaços + manifestos) | Blobs Taikai, pista, ações de governança em SoraFS | Crítica | Crítica | Modere | DA WG / Equipe de Armazenamento |
| Manifestos Norito DA | Tipo de metadados que desenha os blobs | Crítica | Onze | Modere | GT de Protocolo Central |
| Compromissos do bloco | CIDs + racines KZG nos blocos Nexus | Crítica | Onze | Faível | GT de Protocolo Central |
| Horários PDP/PoTR | Cadência de aplicação para réplicas DA | Onze | Onze | Faível | Equipe de armazenamento |
| Operador de registro | Fornecedores de armazenamento aprovado e político | Onze | Onze | Faível | Conselho de Governança |
| Registros de aluguel e incentivos | Registro de entradas para aluguel DA e penalidades | Onze | Modere | Faível | GT Tesouraria |
| Painéis de observação | SLOs DA, professor de replicação, alertas | Modere | Onze | Faível | SRE / Observabilidade |
| Intenções de reparação | Receitas para reidratar pedaços pequenos | Modere | Modere | Faível | Equipe de armazenamento |

## Adversários e capacitados| Ator | Capacidades | Motivações | Notas |
| --- | --- | --- | --- |
| Mal-intencionado ao cliente | Remova os blobs malformados, rejuvenesça os manifestos obsoletos, tente DoS na ingestão. | Perturbe as transmissões Taikai, injetando dados inválidos. | Privilégios Pas de cles. |
| Noeud de stockage byzantin | Conta-gotas de réplicas, falsificador de provas PDP/PoTR, conluio. | Reduire la retenção DA, eviter la rent, retenir les donnees. | Possuir credenciais válidas. |
| Comprometimento do sequenciador | Omitir compromissos, equivocar blocos, reordenar metadados. | Cacher des submitions DA, creer des incoherences. | Limite pela maioria do consenso. |
| Operador interno | Abusador do acesso ao governo, manipulador das políticas de retenção, vazando credenciais. | Ganho econômico, sabotagem. | Acesso a infra-estrutura quente/frio. |
| Reserva do adversário | Particiona os noeuds, retarda a replicação, injeta o tráfego MITM. | Reduza a disponibilidade e degrade os SLOs. | Não pode ser usado TLS, mas pode ser conta-gotas/ralentir les liens. |
| Observabilidade do Attaquant | Viole painéis/alertas, suprima incidentes. | Armazene interrupções no DA. | Necessita de acesso à telemetria do gasoduto. |

## Fronteiras de confiança

- **Entrada de fronteira:** Cliente na extensão DA Torii. Requer par de autenticação
  recepção, limitação de taxa e validação de carga útil.
- **Replicação de fronteira:** Noeuds de stockage echangent chunks et proofs. Les
  noeuds sont mutuellement autentica mais peuvent se comporter en bizantino.
- **Lista de fronteira:** Donnees de bloc commits versus estoque fora da cadeia. Le
  consenso garde l'integrite, mas l'disponibilidade requer aplicação
  fora da cadeia.
- **Governação fronteiriça:** Decisões que o Conselho/Parlamento aprova os operadores,
  orçamentos e cortes. Les bris ici impactam diretamente a implantação DA.
- **Observabilidade de fronteira:** Colete métricas/registros exportados para painéis e
  ferramentas de alerta. Le adulteração de interrupções ou ataques de cache.

## Cenários de ameaça e controles

### Ataques no caminho da ingestão

**Cenário:** Cliente mal-intencionado sob cargas úteis Norito malformadas ou desfeitas
blobs sobredimensionados para extrair recursos ou injetar metadados
invalidar.

**Controles**
- Validação do esquema Norito com negociação restrita de versão; rejeitar os
  sinaliza inconnus.
- Limitação de taxa e autenticação no ponto final de ingestão Torii.
- O tamanho do pedaço e a codificação determinam as forças do pedaço SoraFS.
- O pipeline de admissão não persiste nos manifestos que correspondem à soma de verificação.
- Determinação de cache de repetição (`ReplayCache`) adequada às janelas `(lane, época,
  sequência)`, persiste as marcas d'água no disco e rejeita as duplicatas
  et reproduz obsoletos; aproveita a propriedade e a fuzz protege as impressões digitais
  divergentes e submissões hors ordre. [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Lacunas residuais**
- Torii ingest depende do cache de repetição para a admissão e persiste os cursores
  de sequência a atravessa les redemarrages.
- Os esquemas Norito DA mantêm um fuzz chicote dedie
  (`fuzz/da_ingest_schema.rs`) para enfatizar as invariantes de codificação/decodificação; os
  dashboards de cobertura devem alertar se a regressão for cível.

### Retenção por retenção de replicação

**Cenário:** Os operadores de armazenamento bizantinos aceitam os pinos, mas os soltam
pedaços, passam pelos desafios PDP/PoTR por meio de respostas forjadas ou conluio.

**Controles**
- O cronograma de desafios PDP/PoTR é enviado para cargas úteis DA com cobertura por par
  época.
- Replicação multi-fonte com seus quorum; o orquestrador detectou os
  fragmentos manquants et declenche la reparation.
- Corte de governo mentira aux provas echoues et aux réplicas manquantes.
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) aqui
  compare os recibos de ingestão com os compromissos DA (SignedBlockWire,
  `.norito` ou JSON), emite um pacote JSON de evidência para o governo, e
  ecoe em tickets manquants/mismatches para que o Alertmanager possa pager.**Lacunas residuais**
- Le chicote de simulação em `integration_tests/src/da/pdp_potr.rs` (couvert
  par `integration_tests/tests/da/pdp_potr_simulation.rs`) exerce des cenários
  de conluio e partição, validando que o cronograma PDP/PoTR detecte o
  comportamento byzantin de facon determinista. Continue a escrever com DA-5
  para cobrir novas superfícies de prova.
- A política de despejo de nível frio exige uma trilha de auditoria assinada para prevenir
  as gotas furtifs.

### Manipulação de compromissos

**Cenário:** O sequenciador compromete a publicação dos blocos omitindo ou modificando os
compromissos DA, provocando falhas de busca ou incoerências light-client.

**Controles**
- O consenso verifica as propostas de bloco em relação aos arquivos de submissão
  DA; os pares rejeitam as propostas sem compromissos exigidos.
- Os clientes leves verificam as provas de inclusão antes de expor os identificadores
  de buscar.
- Trilha de auditoria comparando as receitas de ingestão de compromissos do bloco.
- Trabalho de reconciliação automatizado (`cargo xtask da-commitment-reconcile`) aqui
  compare os recibos de ingestão com os compromissos DA (SignedBlockWire,
  `.norito` ou JSON), emite um pacote JSON de evidência para o governo, e
  ecoe em tickets manquants ou mismatches para que o Alertmanager possa pager.

**Lacunas residuais**
- Couvert par le job de reconciliação + gancho Alertmanager; os pacotes de
  o governo não mantém o pacote JSON de evidência por padrão.

### Partição censura e censura

**Cenário:** O adversário particiona o recurso de replicação, executando-os
É necessário obter blocos atribuídos ou responder a desafios PDP/PoTR.

**Controles**
- Exigências de provedores multirregionais garantidos des chemins reseau divers.
- Febres de desafio com jitter e fallback em relação aos canais de reparação
  fora da banda.
- Painéis de observação que monitoram o nível de replicação,
  sucesso do desafio e latência de busca com seus alertas.

**Lacunas residuais**
- Simulações de partição para eventos Taikai ao vivo novamente;
  besoin de testes de imersão.
- Política de reserva de banda passante de reparação não codificada.

### Abuso interno

**Cenário:** O operador com acesso ao registro manipula as políticas de
retenção, lista branca de provedores mal-intencionados ou suprime alertas.

**Controles**
- Ações de governo que exigem assinaturas multipartidárias e registros
  notariais Norito.
- As alterações políticas emitem eventos em relação ao monitoramento e registros
  do arquivo.
- O pipeline de observação impõe os logs Norito apenas anexados com hash
  encadeamento.
- L'automatisation d'audit trimestriel (`cargo xtask da-privilege-audit`) parcourt
  os repertórios manifestos/replay (mais caminhos fornecidos pelos operadores), sinalizam os
  entradas manquantes/non-directory/world-writable, e emite um sinal JSON de pacote
  para painéis de governança.

**Lacunas residuais**
- A prevenção de violação do painel requer assinaturas de instantâneos.

## Registro de riscos residuais

| Risco | Probabilidade | Impacto | Proprietário | Plano de mitigação |
| --- | --- | --- | --- | --- |
| Repetição dos manifestos DA antes da chegada do cache de sequência DA-2 | Possível | Modere | GT de Protocolo Central | Cache de sequência do implementador + validação de nonce em DA-2; adicionar testes de regressão. |
| Conluio PDP/PoTR quando >f noeuds sont compromis | Peu provável | Onze | Equipe de armazenamento | Obtenha um novo cronograma de desafios com amostragem entre provedores; validador via chicote de simulação. |
| Lacuna de auditoria de despejo em nível frio | Possível | Onze | Equipe SRE/Armazenamento | Anexar registros de sinais e recibos na cadeia para despejos; monitor por meio de painéis. |
| Latência de detecção de omissão do sequenciador | Possível | Onze | GT de Protocolo Central | `cargo xtask da-commitment-reconcile` noturno compara recibos vs compromissos (SignedBlockWire/`.norito`/JSON) e página de gerenciamento de tickets manquants ou incompatíveis. |
| Partição de resiliência para transmissões ao vivo de Taikai | Possível | Crítica | Rede TL | Executor de exercícios de partição; reserve a banda passante de reparação; documentar SOP de failover. |
| Derive de privilégio de governo | Peu provável | Onze | Conselho de Governança | `cargo xtask da-privilege-audit` trimestre (dirs manifest/replay + caminhos extras) com sinal JSON + painel de controle; ancrer les artefatos de auditoria on-chain. |

## Requisitos de acompanhamento1. Publique os esquemas Norito de ingestão de DA e vetores de exemplo (portados em
   DA-2).
2. Ramifique o cache de repetição na ingestão de DA Torii e persista os cursores de
   sequência a atravessa les redemarrages.
3. **Termine (2026-02-05):** O chicote de simulação PDP/PoTR exerce manutenção
   conluio + partição com modelização de backlog QoS; ver
   `integration_tests/src/da/pdp_potr.rs` (testes sous
   `integration_tests/tests/da/pdp_potr_simulation.rs`) para implementação e
   les resume deterministes capturas ci-dessous.
4. **Término (2026-05-29):** `cargo xtask da-commitment-reconcile` compare arquivos
   recibos de ingestão de compromissos DA (SignedBlockWire/`.norito`/JSON), emet
   `artifacts/da/commitment_reconciliation.json`, e está ramificado a
   Alertmanager/pacotes de governança para alertas de omissão/adulteração
   (`xtask/src/da.rs`).
5. **Terminar (2026-05-29):** `cargo xtask da-privilege-audit` parcourt le spool
   manifesto/replay (mais caminhos fornecidos pelos operadores), sinal de entradas
   manquantes/non-directory/world-writable, e produz um pacote JSON assinado para
   painéis/revistas de governo (`artifacts/da/privilege_audit.json`),
   fermant la lacuna de automatização de acesso.

**Ou veja só:**

- O cache de repetição e a persistência dos cursores foram apagados no DA-2. Voir
  implementação em `crates/iroha_core/src/da/replay_cache.rs` (lógica de
  cache) e integração Torii em `crates/iroha_torii/src/da/ingest.rs`, aqui thread
  eles verificam a impressão digital via `/v2/da/ingest`.
- As simulações de streaming PDP/PoTR são exercidas por meio do chicote de prova-stream
  em `crates/sorafs_car/tests/sorafs_cli.rs`, cobre o fluxo de recepção
  PoR/PDP/PoTR e os cenários de fracasso animes no modelo de ameaças.
- Os resultados de capacidade e reparo absorvem vivent sous
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, tanto que a matriz de
  absorver Sumeragi plus large est suivie dans `docs/source/sumeragi_soak_matrix.md`
  (variantes localizadas inclusas). Esses artefatos capturam as brocas de longo prazo
  referências no registro de riscos residuais.
- Reconciliação de automatização + auditoria de privilégios vit dans
  `docs/automation/da/README.md` e os novos comandos
  `cargo xtask da-commitment-reconcile`/`cargo xtask da-privilege-audit`; utilizar
  as saídas por padrão sob `artifacts/da/` para o anexo de evidência aux
  pacotes de governo.

## Evidência de simulação e modelagem de QoS (2026-02)

Para encerrar o acompanhamento DA-1 # 3, vamos codificar um chicote de simulação
PDP/PoTR determinado sob `integration_tests/src/da/pdp_potr.rs` (couvert par
`integration_tests/tests/da/pdp_potr_simulation.rs`). Le arnês alloue des
noeuds sur trois regiões, injetam partições/conluio de acordo com as probabilidades de
roteiro, atender ao atraso do PoTR e apenas um modelo de atraso de reparação
que reflete o orçamento de reparação do nível quente. A execução do cenário par
defaut (12 épocas, 18 desafios PDP + 2 fenetres PoTR por época) para produzir les
métricas seguintes:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Métrica | Valor | Notas |
| --- | --- | --- |
| Detectados falhas de PDP | 48/49 (98,0%) | As partições diminuíram novamente a detecção; um único echec não detecta vient d'un jitter honete. |
| PDP significa latência de detecção | 0,0 épocas | Les echecs são sinalizados na época de origem. |
| Detectados falhas PoTR | 28/77 (36,4%) | A detecção diminui quando uma nova taxa >=2 fenômenos PoTR, libera a maioria dos eventos no registro de riscos residuais. |
| PoTR significa latência de detecção | Épocas 2.0 | Corresponde au seuil de lateness a deux epochs integre dans l'escalation d'archivage. |
| Pico na fila de reparos | 38 manifestos | O backlog aumenta quando as partições estão empiladas e certifique-se de que as quatro reparações estejam disponíveis por época. |
| Latência de resposta p95 | 30.068ms | Reflete a janela de desafio 30 s com jitter de +/- 75 ms aplicado à amostragem de QoS. |
<!-- END_DA_SIM_TABLE -->

Essas missões mantêm os protótipos do painel DA e satisfeitos
os critérios de aceitação "arnês de simulação + modelagem de QoS" são referenciados em
o roteiro.

A automação do traseiro
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
aqui chame o chicote partage e emet Norito JSON versão
`artifacts/da/threat_model_report.json` por padrão. Les jobs noturnos
consumir este arquivo para rafraichir as matrizes neste documento e alertar
sobre a derivação das taxas de detecção, das filas de reparação ou das amostras de QoS.

Para rafraichir la tabela ci-dessus para les docs, executor `make docs-da-threat-model`,
aqui invoco `cargo xtask da-threat-model-report`, regenerar
`docs/source/da/_generated/threat_model_report.json`, e reescrita esta seção
através de `scripts/docs/render_da_threat_model_tables.py`. Le espelho `docs/portal`
(`docs/portal/docs/da/threat-model.md`) está meu dia na passagem do meme para
que as duas cópias permanecem sincronizadas.