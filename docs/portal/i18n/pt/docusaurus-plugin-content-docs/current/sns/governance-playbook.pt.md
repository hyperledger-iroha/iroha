---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sns/governance_playbook.md` e agora serve como a
copia canonica do portal. O arquivo fonte permanece para PRs de traducao.
:::

# Playbook de governanca do Sora Name Service (SN-6)

**Status:** Redigido 2026-03-24 - referencia viva para a prontidao SN-1/SN-6  
**Links do roadmap:** SN-6 "Compliance & Dispute Resolution", SN-7 "Resolver & Gateway Sync", politica de endereco ADDR-1/ADDR-5  
**Pre-requisitos:** Esquema do registro em [`registry-schema.md`](./registry-schema.md), contrato da API do registrar em [`registrar-api.md`](./registrar-api.md), guia UX de enderecos em [`address-display-guidelines.md`](./address-display-guidelines.md), e regras de estrutura de contas em [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este playbook descreve como os corpos de governanca do Sora Name Service (SNS)
adotam cartas, aprovam registros, escalam disputas e provam que os estados de
resolver e gateway permanecem em sincronia. Ele cumpre o requisito do roadmap de
que a CLI `sns governance ...`, manifestos Norito e artefatos de auditoria
compartilhem uma unica referencia voltada ao operador antes do N1 (lancamento
publico).

## 1. Escopo e publico

O documento se destina a:

- Membros do Conselho de Governanca que votam em cartas, politicas de sufixo e
  resultados de disputa.
- Membros do conselho de guardians que emitem congelamentos de emergencia e
  revisam reversoes.
- Stewards de sufixo que operam filas do registrar, aprovam leiloes e gerenciam
  divisao de receitas.
- Operadores de resolver/gateway responsaveis pela propagacao SoraDNS, atualizacoes
  GAR e guardrails de telemetria.
- Equipes de conformidade, tesouraria e suporte que devem demonstrar que toda
  acao de governanca deixou artefatos Norito auditaveis.

Ele cobre as fases beta fechada (N0), lancamento publico (N1) e expansao (N2)
listadas em `roadmap.md`, vinculando cada fluxo de trabalho as evidencias
necessarias, dashboards e caminhos de escalonamento.

## 2. Papeis e mapa de contato

| Papel | Responsabilidades principais | Artefatos e telemetria principais | Escalacao |
|-------|------------------------------|-----------------------------------|-----------|
| Conselho de Governanca | Redige e ratifica cartas, politicas de sufixo, vereditos de disputa e rotacoes de steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos do conselho armazenados via `sns governance charter submit`. | Presidente do conselho + rastreador de agenda de governanca. |
| Conselho de guardians | Emite congelamentos soft/hard, canones de emergencia e revisoes de 72 h. | Tickets guardian emitidos por `sns governance freeze`, manifestos de override registrados em `artifacts/sns/guardian/*`. | Rotacao guardian on-call (<=15 min ACK). |
| Stewards de sufixo | Operam filas do registrar, leiloes, niveis de preco e comunicacao com clientes; reconhecem conformidades. | Politicas de steward em `SuffixPolicyV1`, folhas de referencia de preco, acknowledgements de steward armazenados junto a memos regulatorios. | Lider do programa steward + PagerDuty por sufixo. |
| Operacoes de registrar e cobranca | Operam endpoints `/v1/sns/*`, reconciliam pagamentos, emitem telemetria e mantem snapshots de CLI. | API do registrar ([`registrar-api.md`](./registrar-api.md)), metricas `sns_registrar_status_total`, provas de pagamento arquivadas em `artifacts/sns/payments/*`. | Duty manager do registrar e liaison da tesouraria. |
| Operadores de resolver e gateway | Mantem SoraDNS, GAR e estado do gateway alinhados com eventos do registrar; transmitem metricas de transparencia. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE resolver on-call + ponte ops do gateway. |
| Tesouraria e financas | Aplica divisao de receita 70/30, carve-outs de referral, registros fiscais/tesouraria e atestacoes SLA. | Manifestos de acumulacao de receita, exports Stripe/tesouraria, apendices KPI trimestrais em `docs/source/sns/regulatory/`. | Controller financeiro + oficial de conformidade. |
| Liaison de conformidade e regulacao | Acompanha obrigacoes globais (EU DSA, etc.), atualiza covenants KPI e registra divulgacoes. | Memos regulatorios em `docs/source/sns/regulatory/`, decks de referencia, entradas `ops/drill-log.md` para ensaios tabletop. | Lider do programa de conformidade. |
| Suporte / SRE on-call | Lida com incidentes (colisoes, drift de cobranca, quedas de resolver), coordena mensagens a clientes e e dono dos runbooks. | Templates de incidente, `ops/drill-log.md`, evidencia de laboratorio, transcricoes Slack/war-room arquivadas em `incident/`. | Rotacao on-call SNS + gestao SRE. |

## 3. Artefatos canonicos e fontes de dados

| Artefato | Localizacao | Proposito |
|----------|------------|----------|
| Carta + addenda KPI | `docs/source/sns/governance_addenda/` | Cartas assinadas com controle de versao, covenants KPI e decisoes de governanca referenciadas por votos da CLI. |
| Esquema do registro | [`registry-schema.md`](./registry-schema.md) | Estruturas Norito canonicas (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato do registrar | [`registrar-api.md`](./registrar-api.md) | Payloads REST/gRPC, metricas `sns_registrar_status_total` e expectativas de governance hook. |
| Guia UX de enderecos | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizacoes canonicas IH58 (preferido) e comprimidas (segunda melhor opcao) refletidas por wallets/explorers. |
| Docs SoraDNS / GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivacao deterministica de hosts, fluxo do tailer de transparencia e regras de alerta. |
| Memos regulatorios | `docs/source/sns/regulatory/` | Notas de entrada por jurisdicao (ex. EU DSA), acknowledgements de steward, anexos de template. |
| Drill log | `ops/drill-log.md` | Registro de ensaios de caos e IR requeridos antes de saidas de fase. |
| Armazenamento de artefatos | `artifacts/sns/` | Provas de pagamento, tickets guardian, diffs de resolver, exports KPI e saida de CLI assinada produzida por `sns governance ...`. |

Todas as acoes de governanca devem referenciar pelo menos um artefato na tabela
acima para que auditores reconstruam o rastro de decisao em 24 horas.

## 4. Playbooks de ciclo de vida

### 4.1 Mocoes de carta e steward

| Etapa | Responsavel | CLI / Evidencia | Notas |
|-------|-------------|-----------------|-------|
| Redigir addendum e deltas KPI | Relator do conselho + lider steward | Template Markdown armazenado em `docs/source/sns/governance_addenda/YY/` | Incluir IDs de covenant KPI, hooks de telemetria e condicoes de ativacao. |
| Enviar proposta | Presidente do conselho | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produz `CharterMotionV1`) | A CLI emite manifesto Norito salvo em `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto e acknowledgement guardian | Conselho + guardians | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Anexar atas hasheadas e provas de quorum. |
| Aceitacao steward | Programa de steward | `sns governance steward-ack --proposal <id> --signature <file>` | Obrigatorio antes de mudar politicas de sufixo; registrar envelope em `artifacts/sns/governance/<id>/steward_ack.json`. |
| Ativacao | Ops do registrar | Atualizar `SuffixPolicyV1`, atualizar caches do registrar, publicar nota em `status.md`. | Timestamp de ativacao registrado em `sns_governance_activation_total`. |
| Log de auditoria | Conformidade | Adicionar entrada em `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` e no drill log se houver tabletop. | Incluir referencias a dashboards de telemetria e diffs de politica. |

### 4.2 Aprovacoes de registro, leilao e preco

1. **Preflight:** O registrar consulta `SuffixPolicyV1` para confirmar nivel de
   preco, termos disponiveis e janelas de graca/redencao. Mantenha folhas de
   preco sincronizadas com a tabela de niveis 3/4/5/6-9/10+ (nivel base +
   coeficientes de sufixo) documentada no roadmap.
2. **Leiloes sealed-bid:** Para pools premium, execute o ciclo 72 h commit /
   24 h reveal via `sns governance auction commit` / `... reveal`. Publique a
   lista de commits (apenas hashes) em `artifacts/sns/auctions/<name>/commit.json`
   para que auditores verifiquem a aleatoriedade.
3. **Verificacao de pagamento:** Registrars validam `PaymentProofV1` contra a
   divisao de tesouraria (70% tesouraria / 30% steward com carve-out de referral <=10%).
   Armazene o JSON Norito em `artifacts/sns/payments/<tx>.json` e vincule-o na
   resposta do registrar (`RevenueAccrualEventV1`).
4. **Hook de governanca:** Anexe `GovernanceHookV1` para nomes premium/guarded com
   referencia a ids de proposta do conselho e assinaturas de steward. Hooks
   ausentes resultam em `sns_err_governance_missing`.
5. **Ativacao + sync do resolver:** Assim que Torii emitir o evento de registro,
   acione o tailer de transparencia do resolver para confirmar que o novo estado
   GAR/zone se propagou (veja 4.5).
6. **Divulgacao ao cliente:** Atualize o ledger voltado ao cliente (wallet/explorer)
   via os fixtures compartilhados em [`address-display-guidelines.md`](./address-display-guidelines.md),
   garantindo que renderizacoes IH58 e comprimidas correspondam a orientacoes de copy/QR.

### 4.3 Renovacoes, cobranca e reconciliacao da tesouraria

- **Fluxo de renovacao:** Registrars aplicam a janela de graca de 30 dias + janela
  de redencao de 60 dias especificadas em `SuffixPolicyV1`. Apos 60 dias, a
  sequencia de reabertura holandesa (7 dias, taxa 10x decaindo 15%/dia) e
  acionada automaticamente via `sns governance reopen`.
- **Divisao de receita:** Cada renovacao ou transferencia cria um
  `RevenueAccrualEventV1`. Exports de tesouraria (CSV/Parquet) devem reconciliar
  esses eventos diariamente; anexe provas em `artifacts/sns/treasury/<date>.json`.
- **Carve-outs de referral:** Percentuais de referral opcionais sao rastreados por
  sufixo ao adicionar `referral_share` a politica de steward. Registrars emitem a
  divisao final e armazenam manifestos de referral ao lado da prova de pagamento.
- **Cadencia de relatorios:** Financas publica anexos KPI mensais (registros,
  renovacoes, ARPU, uso de disputas/bond) em `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`.
  Dashboards devem puxar das mesmas tabelas exportadas para que os numeros de
  Grafana batam com as evidencias do ledger.
- **Revisao KPI mensal:** O checkpoint da primeira terca-feira junta o lider de
  financas, steward de plantao e PM do programa. Abra o [SNS KPI dashboard](./kpi-dashboard.md)
  (embed do portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exporte as tabelas de throughput + receita do registrar, registre deltas no
  anexo e anexe os artefatos ao memo. Acione um incidente se a revisao encontrar
  quebras de SLA (janelas de freeze >72 h, picos de erro do registrar, drift de ARPU).

### 4.4 Congelamentos, disputas e apelacoes

| Fase | Responsavel | Acao e evidencia | SLA |
|------|-------------|------------------|-----|
| Pedido de freeze soft | Steward / suporte | Abrir ticket `SNS-DF-<id>` com provas de pagamento, referencia do bond de disputa e seletor(es) afetados. | <=4 h da entrada. |
| Ticket guardian | Conselho guardian | `sns governance freeze --selector <IH58> --reason <text> --until <ts>` produz `GuardianFreezeTicketV1`. Armazene o JSON do ticket em `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h execucao. |
| Ratificacao do conselho | Conselho de governanca | Aprovar ou rejeitar congelamentos, documentar decisao com link ao ticket guardian e digest do bond de disputa. | Proxima sessao do conselho ou voto assincrono. |
| Painel de arbitragem | Conformidade + steward | Convocar painel de 7 jurados (conforme roadmap) com cedulas hasheadas via `sns governance dispute ballot`. Anexar recibos de voto anonimizados ao pacote de incidente. | Veredito <=7 dias apos deposito do bond. |
| Apelacao | Guardian + conselho | Apelacoes dobram o bond e repetem o processo de jurados; registrar manifesto Norito `DisputeAppealV1` e referenciar ticket primario. | <=10 dias. |
| Descongelar e remediar | Registrar + ops de resolver | Executar `sns governance unfreeze --selector <IH58> --ticket <id>`, atualizar status do registrar e propagar diffs GAR/resolver. | Imediatamente apos o veredito. |

Canones de emergencia (congelamentos acionados por guardian <=72 h) seguem o mesmo
fluxo, mas exigem revisao retroativa do conselho e uma nota de transparencia em
`docs/source/sns/regulatory/`.

### 4.5 Propagacao de resolver e gateway

1. **Hook de evento:** Cada evento de registro emite para o stream de eventos do
   resolver (`tools/soradns-resolver` SSE). Ops de resolver se inscrevem e
   registram diffs via o tailer de transparencia
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Atualizacao de template GAR:** Gateways devem atualizar templates GAR
   referenciados por `canonical_gateway_suffix()` e re-assinar a lista
   `host_pattern`. Armazene diffs em `artifacts/sns/gar/<date>.patch`.
3. **Publicacao de zonefile:** Use o skeleton de zonefile descrito em `roadmap.md`
   (name, ttl, cid, proof) e envie para Torii/SoraFS. Arquive o JSON Norito em
   `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Cheque de transparencia:** Execute `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantir que os alertas sigam verdes. Anexe a saida de texto do
   Prometheus ao relatorio semanal de transparencia.
5. **Auditoria de gateway:** Registre amostras de headers `Sora-*` (politica de
   cache, CSP, digest GAR) e anexe-as ao log de governanca para que operadores
   possam provar que o gateway serviu o novo nome com os guardrails esperados.

## 5. Telemetria e relatorios

| Sinal | Fonte | Descricao / Acao |
|-------|-------|------------------|
| `sns_registrar_status_total{result,suffix}` | Handlers do registrar Torii | Contador de sucesso/erro para registros, renovacoes, congelamentos, transferencias; alerta quando `result="error"` aumenta por sufixo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Metricas Torii | SLOs de latencia para handlers de API; alimenta dashboards baseados em `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Tailer de transparencia do resolver | Detecta provas obsoletas ou drift de GAR; guardrails definidos em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de governanca | Contador incrementado quando um charter/addendum ativa; usado para reconciliar decisoes do conselho vs addenda publicadas. |
| `guardian_freeze_active` gauge | CLI guardian | Acompanha janelas de freeze soft/hard por seletor; pagine SRE se o valor ficar `1` alem do SLA declarado. |
| Dashboards de anexos KPI | Financas / Docs | Rollups mensais publicados junto a memos regulatorios; o portal os embute via [SNS KPI dashboard](./kpi-dashboard.md) para que stewards e reguladores acessem a mesma visao Grafana. |

## 6. Requisitos de evidencia e auditoria

| Acao | Evidencia a arquivar | Armazenamento |
|------|----------------------|---------------|
| Mudanca de carta / politica | Manifesto Norito assinado, transcript CLI, diff de KPI, acknowledgement de steward. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovacao | Payload `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prova de pagamento. | `artifacts/sns/payments/<tx>.json`, logs da API do registrar. |
| Leilao | Manifestos commit/reveal, semente de aleatoriedade, planilha de calculo do vencedor. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket guardian, hash de voto do conselho, URL de log de incidente, template de comunicacao com cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagacao de resolver | Diff zonefile/GAR, trecho JSONL do tailer, snapshot Prometheus. | `artifacts/sns/resolver/<date>/` + relatorios de transparencia. |
| Intake regulatorio | Memo de intake, tracker de deadlines, acknowledgement de steward, resumo de mudancas KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de gate de fase

| Fase | Criterios de saida | Bundle de evidencia |
|------|--------------------|--------------------|
| N0 - Beta fechada | Esquema de registro SN-1/SN-2, CLI de registrar manual, drill guardian completo. | Motion de carta + ACK steward, logs de dry-run do registrar, relatorio de transparencia do resolver, entrada em `ops/drill-log.md`. |
| N1 - Lancamento publico | Leiloes + tiers de preco fixo ativos para `.sora`/`.nexus`, registrar self-service, auto-sync de resolver, dashboards de cobranca. | Diff de folha de preco, resultados CI do registrar, anexo de pagamento/KPI, saida do tailer de transparencia, notas de ensaio de incidente. |
| N2 - Expansao | `.dao`, APIs de reseller, portal de disputa, scorecards de steward, dashboards de analitica. | Capturas do portal, metricas SLA de disputa, exports de scorecards de steward, carta de governanca atualizada com politicas de reseller. |

As saidas de fase exigem drills tabletop registrados (fluxo feliz de registro,
freeze, outage de resolver) com artefatos anexados em `ops/drill-log.md`.

## 8. Resposta a incidentes e escalonamento

| Gatilho | Severidade | Dono imediato | Acoes obrigatorias |
|---------|-----------|---------------|-------------------|
| Drift de resolver/GAR ou provas obsoletas | Sev 1 | SRE resolver + conselho guardian | Pagine o on-call do resolver, capture a saida do tailer, decida se deve congelar os nomes afetados, publique status a cada 30 min. |
| Queda de registrar, falha de cobranca, ou erros API generalizados | Sev 1 | Duty manager do registrar | Pare novos leiloes, mude para CLI manual, notifique stewards/tesouraria, anexe logs do Torii ao doc de incidente. |
| Disputa de nome unico, mismatch de pagamento, ou escalonamento de cliente | Sev 2 | Steward + lider de suporte | Colete provas de pagamento, determine se freeze soft e necessario, responda ao solicitante dentro do SLA, registre o resultado no tracker de disputa. |
| Achado de auditoria de conformidade | Sev 2 | Liaison de conformidade | Redigir plano de remediacao, arquivar memo em `docs/source/sns/regulatory/`, agendar sessao de conselho de acompanhamento. |
| Drill ou ensaio | Sev 3 | PM do programa | Execute o cenario roteirizado de `ops/drill-log.md`, arquive artefatos, marque gaps como tarefas do roadmap. |

Todos os incidentes devem criar `incident/YYYY-MM-DD-sns-<slug>.md` com tabelas
de ownership, logs de comandos e referencias as evidencias produzidas ao longo
deste playbook.

## 9. Referencias

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
- `ops/drill-log.md`
- `roadmap.md` (secoes SNS, DG, ADDR)

Mantenha este playbook atualizado sempre que o texto das cartas, as superficies
de CLI ou os contratos de telemetria mudarem; as entradas do roadmap que
referenciam `docs/source/sns/governance_playbook.md` devem sempre corresponder a
ultima revisao.
