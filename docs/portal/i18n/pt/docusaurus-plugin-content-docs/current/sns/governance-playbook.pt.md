---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página espelha `docs/source/sns/governance_playbook.md` e agora serve como a
cópia canônica do portal. O arquivo fonte permanece para PRs de tradução.
:::

# Playbook de governança do Sora Name Service (SN-6)

**Status:** Redigido 2026-03-24 - referência viva para a prontidao SN-1/SN-6  
**Links para o roteiro:** SN-6 "Conformidade e resolução de disputas", SN-7 "Resolver e sincronização de gateway", política de endereco ADDR-1/ADDR-5  
**Pré-requisitos:** Esquema de registro em [`registry-schema.md`](./registry-schema.md), contrato da API do registrador em [`registrar-api.md`](./registrar-api.md), guia UX de enderecos em [`address-display-guidelines.md`](./address-display-guidelines.md), e regras de estrutura de contas em [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este manual descreve como os órgãos de governança do Sora Name Service (SNS)
adotem cartas, aprovem registros, escalem disputas e provem que os estados de
resolvedor e gateway permanecem em sincronia. Ele cumpre o requisito do roadmap de
que a CLI `sns governance ...`, manifestos Norito e artistas de auditoria
compartilhem uma referência única externa ao operador antes do N1 (lancamento
público).

## 1. Escopo e público

O documento se destina a:

- Membros do Conselho de Governança que votam em cartas, políticas de sufixo e
  resultados de disputa.
- Membros do conselho de tutores que emitem congelamentos de emergência e
  revisam reversos.
- Stewards de sufixo que operam filas do registrador, aprovam leiloes e gerenciam
  divisão de receitas.
- Operadores de resolvedor/gateway responsáveis pela propagação SoraDNS, atualizações
  GAR e guarda-corpos de telemetria.
- Equipes de conformidade, tesouraria e suporte que devem demonstrar que toda
  ação de governança deixou artefatos Norito auditáveis.

Ele cobre as fases beta fechada (N0), lançamento público (N1) e expansão (N2)
realizado em `roadmap.md`, vinculando cada fluxo de trabalho como evidências
necessárias, dashboards e caminhos de escalada.

## 2. Papéis e mapa de contato| Papel | Responsabilidades principais | Artefatos e telemetria principais | Escalação |
|-------|-----------------------------|-----------------------------------|-----------|
| Conselho de Governança | Redige e ratifica cartas, políticas de sufixo, vereditos de disputa e rotações de steward. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos do conselho guardados via `sns governance charter submit`. | Presidente do conselho + rastreador de agenda de governança. |
| Conselho de tutores | Emite congelamentos suaves/duros, cânones de emergência e revisões de 72 h. | Ticket Guardian emitido por `sns governance freeze`, manifestos de override registrados em `artifacts/sns/guardian/*`. | Guardião da Rotação de plantão (<=15 min ACK). |
| Administradores de sufixo | Operam filas do registrador, leiloes, niveis de preco e comunicacao com clientes; registrarem conformidades. | Políticas de steward em `SuffixPolicyV1`, folhas de referência de preço, reconhecimentos de steward armazenados junto com memorandos reguladores. | Lider do programa steward + PagerDuty por sufixo. |
| Operações de registro e cobrança | Operam endpoints `/v1/sns/*`, reconciliam pagamentos, emitem telemetria e mantêm snapshots de CLI. | API do registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, provas de pagamento arquivadas em `artifacts/sns/payments/*`. | Gerente de serviço do registrador e contato da tesouraria. |
| Operadores de resolução e gateway | Mantem SoraDNS, GAR e estado do gateway alinhados com eventos do registrador; transmitem métricas de transparência. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE resolver on-call + ponte ops do gateway. |
| Tesouraria e finanças | Aplicação de divisão de receita 70/30, carve-outs de encaminhamento, registros fiscais/tesouraria e atestados SLA. | Manifestos de acumulação de receita, exportações Stripe/tesouraria, apêndices KPI trimestrais em `docs/source/sns/regulatory/`. | Controlador financeiro + oficial de conformidade. |
| Ligação de conformidade e regulação | Acompanha obrigações globais (EU DSA, etc.), atualiza convênios KPI e registra divulgações. | Memos reguladores em `docs/source/sns/regulatory/`, decks de referência, entradas `ops/drill-log.md` para ensaios de mesa. | Líder do programa de conformidade. |
| Suporte / SRE plantão | Lida com incidentes (colisoes, drift de cobranca, quedas de resolver), coordena mensagens para clientes e é dono dos runbooks. | Modelos de incidente, `ops/drill-log.md`, evidências de laboratório, transcrições do Slack/war-room arquivadas em `incident/`. | Rotação plantão SNS + gestão SRE. |

## 3. Artefatos canônicos e fontes de dados| Artefato | Localização | Proposta |
|----------|------------|----------|
| Carta + adendos KPI | `docs/source/sns/governance_addenda/` | Cartas assinadas com controle de versão, covenants KPI e decisões de governança referenciadas por votos da CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estruturas Norito canônicas (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato do registrador | [`registrar-api.md`](./registrar-api.md) | Payloads REST/gRPC, métricas `sns_registrar_status_total` e expectativas de governança. |
| Guia UX de enderecos | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizações canônicas i105 (preferidas) e comprimidas (segunda melhor opção) refletidas por wallets/explorers. |
| Documentos SoraDNS/GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivação determinística de hosts, fluxo de coleta de transparência e regras de alerta. |
| Memorandos regulatórios | `docs/source/sns/regulatory/` | Notas de entrada por jurisdição (ex. EU DSA), agradecimentos do administrador, anexos de modelo. |
| Registro de perfuração | `ops/drill-log.md` | Registro de ensaios de caos e IR exigidos antes das saídas de fase. |
| Armazenamento de artefatos | `artifacts/sns/` | Provas de pagamento, guardião de tickets, diferenças de resolução, KPI de exportação e resposta de CLI assinada produzida por `sns governance ...`. |

Todas as ações de governança devem ser referenciadas pelo menos um artista na tabela
acima para que os auditores reconstruam o rastro de decisão em 24 horas.

## 4. Manual do ciclo de vida

### 4.1 Moços de carta e steward

| Etapa | Responsável | CLI / Evidência | Notas |
|-------|------------|-------|-------|
| Redigir adendos e deltas KPI | Relator do conselho + líder steward | Template Markdown armazenado em `docs/source/sns/governance_addenda/YY/` | Inclui IDs de KPI de convênio, ganchos de telemetria e condições de ativação. |
| Enviar proposta | Presidente do conselho | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produção `CharterMotionV1`) | A CLI emite o manifesto Norito salvo em `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto e reconhecimento guardião | Conselho + tutores | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Anexar atas hasheadas e provas de quorum. |
| Mordomo de aceitação | Programa de administrador | `sns governance steward-ack --proposal <id> --signature <file>` | Obrigatório antes de mudar políticas de sufixo; envelope de registrador em `artifacts/sns/governance/<id>/steward_ack.json`. |
| Ativação | Operações do registrador | Atualizar `SuffixPolicyV1`, atualizar caches do registrador, publicar nota em `status.md`. | Timestamp de ativação registrado em `sns_governance_activation_total`. |
| Registro de auditoria | Conformidade | Adicione entrada em `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` e nenhum log de perfuração se houver mesa. | Incluir referências a painéis de telemetria e diferenças políticas. |

### 4.2 Aprovações de registro, leilões e preços1. **Preflight:** O registrador consulta `SuffixPolicyV1` para confirmar o nível de
   preço, termos disponíveis e janelas de graça/redenção. Mantenha folhas de
   preço sincronizado com a tabela de níveis 3/4/5/6-9/10+ (nível base +
   coeficientes de sufixo) documentada no roadmap.
2. **Leiloes seal-bid:** Para pools premium, execute o ciclo 72 h commit /
   Revelação de 24 horas via `sns governance auction commit` / `... reveal`. Publicar um
   lista de commits (apenas hashes) em `artifacts/sns/auctions/<name>/commit.json`
   para que os auditores verifiquem a aleatoriedade.
3. **Verificação de pagamento:** Registradores validam `PaymentProofV1` contra a
   divisão de tesouraria (70% tesouraria / 30% steward com carve-out de encaminhamento 72 h, picos de erro do registrador, desvio de ARPU).### 4.4 Congelamentos, disputas e apelações

| Fase | Responsável | Ação e evidência | SLA |
|------|-------------|------------------|-----|
| Pedido de congelamento suave | Administrador / suporte | Abrir ticket `SNS-DF-<id>` com provas de pagamento, referência do vínculo de disputa e seletor(es) afetado(s). | <=4h da entrada. |
| Guardião de ingressos | Conselho guardião | `sns governance freeze --selector <i105> --reason <text> --until <ts>` produz `GuardianFreezeTicketV1`. Armazene o JSON do ticket em `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h execucao. |
| Ratificação do conselho | Conselho de governança | Aprovar ou rejeitar congelamentos, documentar decisão com link ao ticket Guardian e digest do bond de disputa. | Próxima sessão do conselho ou voto assincrono. |
| Painel de arbitragem | Conformidade + mordomo | Convocar painel de 7 jurados (conforme roadmap) com cedulas hasheadas via `sns governance dispute ballot`. Anexar recibos de voto anonimizados ao pacote de incidente. | Veredito <=7 dias após o depósito do título. |
| Apelação | Tutor + conselho | Apelações dobram o bond e repetem o processo de jurados; manifesto do registrador Norito `DisputeAppealV1` e referenciar ticket primário. | <=10 dias. |
| Descongelar e remediar | Registrador + operações de resolução | Executar `sns governance unfreeze --selector <i105> --ticket <id>`, atualizar status do registrador e propagar diffs GAR/resolver. | Ocorreu após o veredito. |

Canones de emergencia (congelamentos acionados por Guardian <=72 h) Segue o mesmo
fluxo, mas desativar revisão retroativa do conselho e uma nota de transparência em
`docs/source/sns/regulatory/`.

### 4.5 Propagação de resolvedor e gateway

1. **Hook de evento:** Cada evento de registro é emitido para o stream de eventos do
   resolvedor (`tools/soradns-resolver` SSE). Operações de resolução se inscrevem e
   registrar diferenças via o tailer de transparência
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Atualização de template GAR:** Gateways devem atualizar templates GAR
   referenciados por `canonical_gateway_suffix()` e reassinar a lista
   `host_pattern`. Armazene difere em `artifacts/sns/gar/<date>.patch`.
3. **Publicação do arquivo de zona:** Use o esqueleto do arquivo de zona descrito em `roadmap.md`
   (nome, ttl, cid, comprovante) e envio para Torii/SoraFS. Arquivar o JSON Norito em
   `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Cheque de transparência:** Execute `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantir que os alertas sigam verdes. Anexo a disse de texto do
   Prometheus ao relatorio semanal de transparência.
5. **Auditoria de gateway:** Registre amostras de cabeçalhos `Sora-*` (política de
   cache, CSP, digest GAR) e anexo-as ao log de governança para que operadores
   pode provar que o gateway funcionou o novo nome com os guardrails esperados.

## 5. Telemetria e relatos| Sinal | Fonte | Descrição / Ação |
|-------|-------|------------------|
| `sns_registrar_status_total{result,suffix}` | Manipuladores do registrador Torii | Contador de sucesso/erro para registros, renovações, congelamentos, transferências; alerta quando `result="error"` aumenta por sufixo. |
| `torii_request_duration_seconds{route="/v1/sns/*"}` | Métricas Torii | SLOs de latência para manipuladores de API; alimenta dashboards baseados em `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Etiqueta de transparência do resolvedor | Detecta provas obsoletas ou deriva de GAR; guarda-corpos definidos em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de governança | Contador incrementado quando um charter/addendum ativo; usado para reconciliar decisões do conselho vs adendos publicados. |
| Medidor `guardian_freeze_active` | Guardião CLI | Acompanha janelas de congelamento soft/hard por seletor; página SRE se o valor ficar `1` além do SLA declarado. |
| Painéis de anexos KPI | Finanças / Documentos | Rollups mensais publicados junto com memorandos reguladores; o portal os embute via [SNS KPI dashboard](./kpi-dashboard.md) para que stewards e reguladores acessem o mesmo visto Grafana. |

## 6. Requisitos de evidência e auditoria

| Ação | Evidência de arquivo | Armazenamento |
|------|----------------------|---------------|
| Mudança de carta / política | Manifesto Norito assinado, transcrição CLI, diferença de KPI, reconhecimento do administrador. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovação | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, prova de pagamento. | `artifacts/sns/payments/<tx>.json`, logs da API do registrador. |
| Leilão | Manifestos comprometem/revelam, semente de aleatoriedade, planilha de cálculo do vencedor. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket Guardian, hash de voto do conselho, URL de log de incidente, template de comunicação com cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagação de resolução | Diff zonefile/GAR, trecho JSONL do tailer, instantâneo Prometheus. | `artifacts/sns/resolver/<date>/` + relatórios de transparência. |
| Regulamentação de ingestão | Memorando de entrada, rastreador de prazos, reconhecimento de administrador, resumo de mudanças KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de portão de fase| Fase | Critérios de disseda | Pacote de evidências |
|------|--------------------|--------------------|
| N0 - Beta fechado | Esquema de registro SN-1/SN-2, CLI do registrador manual, drill Guardian completo. | Moção de carta + ACK steward, logs de simulação do registrador, relatorio de transparência do resolvedor, entrada em `ops/drill-log.md`. |
| N1 - Lancamento público | Leilões + níveis de preços fixos ativos para `.sora`/`.nexus`, autoatendimento de registrador, sincronização automática de resolução, dashboards de cobranca. | Diferença de folha de preço, resultados CI do registrador, anexo de pagamento/KPI, declaração de transparência, notas de ensaio de incidente. |
| N2 - Expansão | `.dao`, APIs de revendedor, portal de disputa, scorecards de steward, dashboards de análise. | Capturas do portal, métricas de disputa de SLA, exportações de scorecards de steward, carta de governança atualizada com políticas de revendedor. |

As saidas de fase desativar drills tabletop registrados (fluxo feliz de registro,
congelar, interrupção do resolvedor) com artistas anexados em `ops/drill-log.md`.

## 8. Resposta a incidentes e escalada

| Gatilho | Severidade | Dono imediato | Aços obrigatórios |
|--------|-----------|---------------|--------|
| Deriva de resolução/GAR ou provas obsoletas | 1º de setembro | SRE resolvedor + conselho guardião | Pagine o on-call do resolvedor, capture a saida do tailer, decida se deve congelar os nomes afetados, publique status a cada 30 min. |
| Queda de registrador, falha de cobrança, ou erros API generalizados | 1º de setembro | Gerente de plantão do registrador | Pare novos leiloes, mude para manual CLI, notifique stewards/tesouraria, anexe logs do Torii ao documento de incidente. |
| Disputa de nome único, incompatibilidade de pagamento ou escalonamento de cliente | 2 de setembro | Steward + líder de suporte | Colete provas de pagamento, determine se o congelamento é suave e necessário, responda ao solicitador dentro do SLA, registre o resultado no rastreador de disputa. |
| Achado de auditorias de conformidade | 2 de setembro | Contato de conformidade | Redigir plano de remediação, arquivar memorando em `docs/source/sns/regulatory/`, agendar sessão de conselho de acompanhamento. |
| Drill ou ensaio | 3 de setembro | PM do programa | Execute o cenário roteirizado de `ops/drill-log.md`, arquitetos de arquivo, marque lacunas como tarefas do roadmap. |

Todos os incidentes devem criar `incident/YYYY-MM-DD-sns-<slug>.md` com tabelas
de propriedade, registros de comandos e referencias como evidências produzidas ao longo
este manual.

## 9. Referências

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (seções SNS, DG, ADDR)

Mantenha este manual atualizado sempre que o texto das cartas, as superfícies
de CLI ou os contratos de telemetria mudando; as entradas do roadmap que
referencial `docs/source/sns/governance_playbook.md` deve sempre ser correspondente a
última revisão.