---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Cette page reflete `docs/source/sns/governance_playbook.md` et sert maintenant de
cópia canônica do portal. A fonte do arquivo persiste para os PRs de tradução.
:::

# Manual de governança do Sora Name Service (SN-6)

**Estatuto:** Redige 2026-03-24 - referência vivante para a preparação SN-1/SN-6  
**Liens du roadmap:** SN-6 "Conformidade e resolução de disputas", SN-7 "Resolver e sincronização de gateway", política de endereços ADDR-1/ADDR-5  
**Pré-requisitos:** Esquema de registro em [`registry-schema.md`](./registry-schema.md), contrato de API do registrador em [`registrar-api.md`](./registrar-api.md), guia UX de endereços em [`address-display-guidelines.md`](./address-display-guidelines.md), e regras de estrutura de contas em [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este manual decrita comentários sobre os órgãos de governança do Sora Name Service (SNS)
adotar des chartes, aprovar registros, escalar litígios, et
Provei que os estados do resolvedor e do gateway permanecem sincronizados. Satisfeito
A exigência do roteiro de acordo com a CLI `sns governance ...`, os manifestos
Norito e os artefatos de auditoria compartilham uma referência de operação exclusiva
avant N1 (lanceamento público).

## 1. Porta e público

O documento é cível:

- Membros do Conselho de Governo que votam nas cartas, nas políticas de
  sufixo e resultados de litígio.
- Membros do conselho dos guardiões que emitem géis de urgência e examinadores
  as reversões.
- Administradores de sufixo que gerenciam os arquivos do registrador, aprovam os preenchimentos
  e gere as partes da receita.
- Operadores resolvedores/gateway responsáveis pela propagação SoraDNS, des mises a
  jour GAR, et des garde-fous de telemetria.
- Equipes de conformidade, tesouraria e suporte que devem demonstrar o que cada um
  ação de governança e liberação de artefatos Norito auditáveis.

Ele cobre as fases de beta fechado (N0), lançamento público (N1) e expansão (N2)
enumerados em `roadmap.md` e dependentes de cada fluxo de trabalho com pré-requisitos,
painéis e velocidades de escalada.

## 2. Funções e cartão de contato| Função | Responsabilidades principais | Artefatos e princípios de telemetria | Escalada |
|------|--------------|-------------------------|---------|
| Conselho de governo | Redige e ratifique as cartas, políticas de sufixo, veredictos de litígio e rotações de administradores. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, boletins de aconselhamento de ações via `sns governance charter submit`. | Presidente do conselho + suivi du docket de gouvernance. |
| Conselho dos guardiões | Emet des gels soft/hard, canons d'urgence, et revues 72 h. | Tickets Guardian emis par `sns governance freeze`, manifestos d'override consignes sous `artifacts/sns/guardian/*`. | Guardião rotativo de plantão (<=15 min ACK). |
| Administradores de sufixo | Gerenciar os arquivos do registrador, as vagas, os níveis de preço e o cliente de comunicação; reconnaissent les conformites. | Políticas de steward em `SuffixPolicyV1`, fichas de referência de preço, agradecimentos de steward stockes e cote des memos reglementaires. | Líder do programa steward + PagerDuty por sufixo. |
| Registrador de operações e faturação | Abre os endpoints `/v1/sns/*`, reconcilia os pagamentos, emite a telemetria e mantém os snapshots CLI. | Registrador de API ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, pedidos de pagamento arquivados sob `artifacts/sns/payments/*`. | Duty manager du registrador et liaison tresorerie. |
| Operadores resolvedores e gateway | Maintiennent SoraDNS, GAR et l'etat du gateway aligne avec les evenements du registrar; difunde as métricas de transparência. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | Resolvedor SRE de plantão + gateway de operações de ponte. |
| Tesouro e finanças | Solicita a repartição 70/30, as exclusões de encaminhamento, os depósitos fiscais/tresorerie e os atestados SLA. | Manifestos de acumulação de receitas, exportações Stripe/tresorerie, apêndices KPI trimestriels sous `docs/source/sns/regulatory/`. | Controlador financeiro + conformite responsável. |
| Ligação conforme e regulamentada | Cumprir as obrigações globais (EU DSA, etc.), cumprir os KPI dos convênios e depositar as divulgações. | Memorandos regulamentados em `docs/source/sns/regulatory/`, decks de referência, entradas `ops/drill-log.md` para ensaios de mesa. | Liderar o programa de conformidade. |
| Suporte / SRE de plantão | Gere incidentes (colisões, derivação de fatura, painéis de resolução), coordena o cliente de comunicação e possui runbooks. | Modelos de incidente, `ops/drill-log.md`, testes de trabalho na cena, transcrições de arquivos do Slack/sala de guerra sob `incident/`. | Rotação de plantão SNS + SRE de gestão. |

## 3. Artefatos canônicos e fontes de dados| Artefato | Colocação | Objetivo |
|----------|------------|---------|
| Gráfico + adendos KPI | `docs/source/sns/governance_addenda/` | Signatários de gráficos com controle de versão, KPI de convênios e decisões de governança referenciadas por votos CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estruturas canônicas Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato do registrador | [`registrar-api.md`](./registrar-api.md) | Cargas REST/gRPC, métricas `sns_registrar_status_total` e atenção aos ganchos de governança. |
| Guia UX de endereços | [`address-display-guidelines.md`](./address-display-guidelines.md) | Rendus canonices I105 (preferir) e compressas (segunda escolha) reproduzidos por carteiras/exploradores. |
| Documentos SoraDNS/GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | A derivação determina os hosts, o fluxo de trabalho do tamanho da transparência e as regras de alerta. |
| Memorandos regulamentares | `docs/source/sns/regulatory/` | Notes d'accueil par juridiction (ex. EU DSA), administrador de reconhecimentos, anexos de modelo. |
| Diário de perfuração | `ops/drill-log.md` | Journal des ensaios caos e IR requis avant sorties de phase. |
| Estoque de artefatos | `artifacts/sns/` | Solicitações de pagamento, guardião de tickets, resolvedor de diferenças, exporta KPI e sortie CLI signee produzido por `sns governance ...`. |

Todas as ações de governo devem ser referenciadas em menos de um artefato
quadro ci-dessus para que os auditores possam reconstruir o traço de
decisão em 24 horas.

## 4. Manual de ciclo de vida

### 4.1 Moções de charte et steward

| Étape | Proprietário | CLI/Préuve | Notas |
|-------|---------|----------|-------|
| Redimensionar o adendo e os deltas KPI | Relator du conseil + administrador principal | Modelo Markdown estoque sob `docs/source/sns/governance_addenda/YY/` | Inclui IDs de KPI de convênio, ganchos de telemetria e condições de ativação. |
| Soumettre a proposta | Presidente do Conselho | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produto `CharterMotionV1`) | A CLI emitiu um manifesto Norito armazenado sob `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto e reconhecimento guardião | Conselho + tutores | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Junte os hashs dos minutos e as previsões de quorum. |
| Comissário de aceitação | Administrador do programa | `sns governance steward-ack --proposal <id> --signature <file>` | Requis avant changement des politiques de suffixe; registre o envelope sob `artifacts/sns/governance/<id>/steward_ack.json`. |
| Ativação | Operações do registrador | Entre no dia `SuffixPolicyV1`, rafraichir les caches du registrar, publique uma nota em `status.md`. | Registro de data e hora de ativação em `sns_governance_activation_total`. |
| Jornal de auditoria | Conformidade | Adicione uma entrada a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` e um diário de perfuração e efeito de mesa. | Inclui referências a painéis de telemetria e diferenças políticas. |

### 4.2 Aprovações de registro, registro e preço1. **Preflight:** O registrador interroge `SuffixPolicyV1` para confirmar o nível
   de preço, os termos disponíveis e as janelas de graça/redenção. Cuide deles
   fichas de preços sincronizadas com a tabela de níveis 3/4/5/6-9/10+ (niveau
   de base + coeficientes de sufixo) documentado no roteiro.
2. **Encheres seal-bid:** Para os pools premium, executa o ciclo de 72 h commit /
   Revelação de 24 horas via `sns governance auction commit` / `... reveal`. Publicar a lista
   des commits (hashes únicos) sob `artifacts/sns/auctions/<name>/commit.json`
   para que os auditores possam verificar o aleatório.
3. **Verificação de pagamento:** Registros válidos `PaymentProofV1` por relatório
   aux repartitions de tresorerie (70% tresorerie / 30% steward com carve-out de
   encaminhamento <=10%). Armazene o JSON Norito sob `artifacts/sns/payments/<tx>.json`
   e ele está na resposta do registrador (`RevenueAccrualEventV1`).
4. **Gancho de governo:** Anexador `GovernanceHookV1` para nomes premium/protegidos
   referindo-se às ids da proposta do conselho e às assinaturas do administrador.
   Les ganchos manquants declinados `sns_err_governance_missing`.
5. **Ativação + resolução de sincronização:** Uma vez que Torii emite o evento de registro,
   diminua o tamanho da transparência do resolvedor para confirmar que o novo
   etat GAR/zone s'est propage (ver 4.5).
6. **Cliente de divulgação:** Mettre a jour le ledger oriente client (wallet/explorer)
   via les fixtures compartilhadas em [`address-display-guidelines.md`](./address-display-guidelines.md),
   com certeza que os resultados I105 e as compactações correspondentes aos guias de cópia/QR.

### 4.3 Renovações, faturação e reconciliação de tesouros- **Workflow de renovação:** Os registradores aplicam a janela de graça de
  30 dias + a janela de resgate de 60 dias especificada em `SuffixPolicyV1`.
  Apres 60 jours, la sequencia de reouverture holandesa (7 jours, frais 10x
  decroissant de 15%/dia) é desativado automaticamente via `sns governance reopen`.
- **Repartition des revenus:** Chaque renouvellement ou transfert cree un
  `RevenueAccrualEventV1`. As exportações de tresorerie (CSV/Parquet) devem ser realizadas
  reconciliar ces evenements quotidiennement; joindre les preuves a
  `artifacts/sns/treasury/<date>.json`.
- **Separações de referência:** As percentagens de opções de referência são suivis
  par sufixo en ajoutant `referral_share` a la politique steward. Os registradores
  emetent le split final et stockent les manifestes de reference a cote de la
  pré-pagamento.
- **Cadência de relatórios:** As finanças publicam os anexos mensais dos KPI
  (registros, renovações, ARPU, utilização de litígios/títulos) sous
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Os painéis são fornecidos
  toque nas tabelas de memes exportadas para que os chefes Grafana
  correspondente aux preuves du ledger.
- **Mensuelle Revue KPI:** O checkpoint du premier mardi associe le lead finance,
  le steward de service e o programa PM. Abrir o [painel SNS KPI](./kpi-dashboard.md)
  (incorporar portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exportar tabelas de taxa de transferência + receita do registrador, consignar os deltas
  no anexo, e junte os artefatos ao memorando. Diminuir um incidente si la
  revista trouve des violações SLA (fenetres de freeze >72 h, pics d'erreurs du
  registrador, derivar ARPU).

### 4.4 Géis, litígios e apelos

| Fase | Proprietário | Ação e prevenção | SLA |
|-------|-------------|------------------|-----|
| Demanda de gel suave | Administrador / suporte | Deposite um ticket `SNS-DF-<id>` com pré-pagamento, referência de título de litígio e seletor(es) afetado(s). | <=4 horas após a entrada. |
| Guardião de ingressos | Conselho guardião | Produto `sns governance freeze --selector <I105> --reason <text> --until <ts>` `GuardianFreezeTicketV1`. Armazene o JSON do ticket sob `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h de execução. |
| Ratificação do conselho | Conselho de governo | Ao aprovar ou rejeitar os géis, documente a decisão com garantia sobre o guardião do tíquete e o resumo do título de litígio. | Prochaine session du conseil ou vote asynchrone. |
| Painel de arbitragem | Conformista + mordomo | Convocar um painel de 7 jurados (selon roadmap) com hashes de boletins via `sns governance dispute ballot`. Junte-se ao recus de voto anonimamente no pacote de incidentes. | Veredicto <= 7 dias após o depósito do título. |
| Apelo | Guardião + conselho | Os apelos dobram o vínculo e repetem o processo dos jurados; registre o manifesto Norito `DisputeAppealV1` e referencie o ticket principal. | <=10 horas. |
| Degel e remediação | Registrador + resolvedor de operações | O executor `sns governance unfreeze --selector <I105> --ticket <id>`, atualizou o status do registrador e propaga as diferenças GAR/resolver. | Imediatamente após o veredicto. |Les canons d'urgence (gels declenches par Guardian <=72 h) suivent le meme flux
mais exigir uma revista retroativa do conselho e uma nota de transparência sob
`docs/source/sns/regulatory/`.

### 4.5 Resolvedor de propagação e gateway

1. **Hook d'evenement:** Cada evento de registro emet ver o fluxo de eventos
   resolvedor (`tools/soradns-resolver` SSE). As operações resolvidas foram canceladas e
   registre as diferenças através do tailer de transparência
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Mise a jour du template GAR:** Os gateways devem ser mettre a jour les templates
   Referências GAR par `canonical_gateway_suffix()` e re-assinar a lista
   `host_pattern`. Armazene as diferenças em `artifacts/sns/gar/<date>.patch`.
3. **Publicação do arquivo de zona:** Utilize o esquema do arquivo de zona descrito em
   `roadmap.md` (nome, ttl, cid, prova) e o botão para Torii/SoraFS. Arquivador
   le JSON Norito como `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Verificação de transparência:** Executor `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantir que os alertas permaneçam verdes. Joindre la sortie texto
   Prometheus com relação à transparência hebdomadaire.
5. **Gateway de auditoria:** Registrar echantillons d'en-tetes `Sora-*` (cache de política,
   CSP, digest GAR) e os membros do jornal de governo depois que eles
   Os operadores podem provar que o gateway atende o novo nome com eles
   garde-fous prevus.

## 5. Telemetria e relatórios

| Sinal | Fonte | Descrição / Ação |
|--------|--------|-----------|
| `sns_registrar_status_total{result,suffix}` | Registrador de gerentes Torii | Compteur succes/erreur pour registros, renovações, géis, transferências; alerta quando `result="error"` aumenta por sufixo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métricas Torii | SLO de latência para API de manipuladores; alimentos dos painéis são emitidos por `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Resolvedor de transparência | Detecte des preuves perimees ou des deriva GAR; garde-fous definido em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | Governança CLI | Compteur incremente a chaque activação de charte/addendum; utilize para reconciliar as decisões do conselho vs adendos públicos. |
| Medidor `guardian_freeze_active` | Guardião CLI | Adapte as janelas de gel macio/duro por seleção; página SRE se o valor restante `1` au-dela du SLA declare. |
| Painéis de anexos KPI | Finanças / Documentos | Rollups mensais publicados com memorandos regulamentares; o portal é inteiro via [painel SNS KPI](./kpi-dashboard.md) para que administradores e reguladores aceitem o meme vue Grafana. |

## 6. Exigências de evidência e auditoria| Ação | Evidência de um arquivador | Estoque |
|--------|----------|----------|
| Mudança de gráfico / política | Manifesto Norito assinatura, transcrição CLI, diferença KPI, administrador de confirmação. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Inscrição / renovação | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, pré-pagamento. | `artifacts/sns/payments/<tx>.json`, registra API do registrador. |
| Enchere | Manifestos cometer/revelar, grão de aleatoire, tabela de cálculo do gagnant. | `artifacts/sns/auctions/<name>/`. |
| Gel / degel | Guardião do ticket, hash do voto do conselho, URL do registro do incidente, modelo do cliente de comunicação. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Resolvedor de propagação | Diff zonefile/GAR, extração JSONL do cliente, instantâneo Prometheus. | `artifacts/sns/resolver/<date>/` + relatórios de transparência. |
| Regulador de admissão | Memorando de início, rastreador de prazos, administrador de reconhecimento, currículo de alterações KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de portão de fase

| Fase | Critérios de surtida | Pacote de evidências |
|-------|--------------------|------------------|
| N0 - Beta fechado | Esquema de registro SN-1/SN-2, manual do registrador CLI, guardião de treinamento completo. | Movimento de gráfico + administrador ACK, logs de simulação do registrador, relatório de resolução de transparência, entrada em `ops/drill-log.md`. |
| N1 - Lanceamento público | Pacotes + níveis de correção de preços ativos para `.sora`/`.nexus`, autoatendimento de registrador, resolvedor de sincronização automática, fabricação de painéis. | Comparação de folha de preço, resultados CI do registrador, anexo de pagamento/KPI, seleção de transparência, notas de incidente de ensaio. |
| N2 - Expansão | `.dao`, revendedor de APIs, portal de litígio, administrador de scorecards, dashboards analíticos. | Captura a tela do portal, mede o SLA do litígio, exporta o administrador dos scorecards, o gráfico de governança atualizado com o revendedor político. |

As saídas de fase que exigem o registro de brocas de mesa (parcours registre
happy path, gel, panne resolve) com artefatos anexados em `ops/drill-log.md`.

## 8. Resposta a incidentes e escalada| Declarador | Severita | Proprietário imediato | Ações obrigatórias |
|------------|----------|-----------------------|----------------------|
| Derive resolver/GAR ou preuves perimees | 1º de setembro | Resolvedor SRE + guardião do conselho | Pager o resolvedor de plantão, capturar a saída final, decidir se os nomes são afetados etre geles, postar um status todos os 30 min. |
| Panne registrar, echec de facturation, ou erros API generalizados | 1º de setembro | Gerente de serviço do registrador | Arreter os novos cheios, baixá-los no CLI manuel, notifier stewards/tresorerie, juntar os logs Torii ao documento de incidente. |
| Litígio sobre um nome único, incompatibilidade de pagamento ou escalonamento de cliente | 2 de setembro | Administrador + suporte de liderança | Colete as prévias de pagamento, determine se um gel macio é necessário, responda ao demandante no SLA, consigne o resultado no rastreador de litígio. |
| Estado de auditoria de conformidade | 2 de setembro | Ligação conforme | Rediger um plano de remediação, depor um memorando sob `docs/source/sns/regulatory/`, planejar uma sessão de conselho de suivi. |
| Drill ou ensaio | 3 de setembro | Programa PM | Execute o script de cenário a partir de `ops/drill-log.md`, arquive os artefatos, etiquete as lacunas como as tabelas do roteiro. |

Todos os incidentes devem ser criados `incident/YYYY-MM-DD-sns-<slug>.md` com des
tabelas próprias, registros de comandos e referências de referências
produzido ao longo deste manual.

## 9. Referências

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (seções SNS, DG, ADDR)

Leia este manual agora, sempre que o texto dos gráficos, as superfícies CLI
ou os contratos de telemetria alterados; as entradas do roteiro que são referenciadas
`docs/source/sns/governance_playbook.md` deve corresponder sempre à la
última revisão.