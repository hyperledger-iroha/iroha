---
lang: pt
direction: ltr
source: docs/portal/docs/sns/governance-playbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fonte canônica
Esta página reflete `docs/source/sns/governance_playbook.md` e agora sirve como
a cópia canônica do portal. O arquivo fonte persiste para PRs de tradução.
:::

# Manual de governança do Serviço de Nomes Sora (SN-6)

**Estado:** Borrador 2026-03-24 - referência viva para a preparação SN-1/SN-6  
**Links do roteiro:** SN-6 "Conformidade e resolução de disputas", SN-7 "Resolver e sincronização de gateway", políticas de direção ADDR-1/ADDR-5  
**Pré-requisitos:** Esquema de registro em [`registry-schema.md`](./registry-schema.md), contrato de API de registrador em [`registrar-api.md`](./registrar-api.md), guia UX de direções em [`address-display-guidelines.md`](./address-display-guidelines.md), e regras de estrutura de contas em [`docs/account_structure.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/account_structure.md).

Este manual descreve como os corpos de governança do Sora Name Service (SNS)
adotar cartas, registrar registros, escalar disputas e verificar quais estados de
resolvedor e gateway permanecem sincronizados. Cumprir o requisito do roteiro de
que o CLI `sns governance ...`, os manifestos Norito e os artefatos de
auditórios compartilham uma única referência operativa antes de N1 (lançamento
público).

## 1. Alcance e audiência

O documento foi direcionado a:

- Miembros do Conselho de Governo que votaram cartas, políticas de sufijos e
  resultados de disputas.
- Miembros da junta de guardiões que emitem congelamentos de emergência e
  revisadas reversões.
- Administradores de sufijos que operam colas de registrador, aprueban subastas y
  gerente de departamentos de ingressos.
- Operadores de resolução/gateway responsáveis pela propagação SoraDNS,
  atualizações GAR e guarda-corpos de telemetria.
- Equipamentos de cumprimento, tesouraria e suporte que devem ser demonstrados em todos
  ação de governança de artefatos Norito auditáveis.

Cubre as fases de beta fechada (N0), lançamento público (N1) e expansão (N2)
contratado em `roadmap.md`, vinculando cada fluxo de trabalho com a evidência,
 painéis e rotas de escalação necessárias.

## 2. Funções e mapa de contatos| Rolo | Responsabilidades principais | Artefatos e telemetria principais | Escalação |
|------|------------------------------------------|--------------------------|------------|
| Conselho de Governo | Redigir e ratificar cartas, políticas de sufijos, veredictos de disputa e rotações de administradores. | `docs/source/sns/governance_addenda/`, `artifacts/sns/governance/*`, votos do conselho almacenados via `sns governance charter submit`. | Presidência do conselho + rastreador de agenda de governo. |
| Junta de Guardiões | Emite congelamentos suaves/duros, cânones de emergência e revisões de 72 h. | Tickets de guardião emitidos por `sns governance freeze`, manifestos de substituição em `artifacts/sns/guardian/*`. | Guardia de plantão (<=15 min ACK). |
| Comissários de sufijo | Operar colas de registrador, subastas, níveis de preço e comunicação com clientes; reconhecer cumprimentos. | Políticas de steward em `SuffixPolicyV1`, hojas de referência de preços, acusações de steward junto com memorandos reguladores. | Lider do programa de stewards + PagerDuty por sufijo. |
| Operações de registro e faturação | Opera endpoints `/v2/sns/*`, reconcilia pagamentos, emite telemetria e mantém snapshots de CLI. | API do registrador ([`registrar-api.md`](./registrar-api.md)), métricas `sns_registrar_status_total`, testes de pagamento em `artifacts/sns/payments/*`. | Gerente de tarefas do registrador e link de tesoreria. |
| Operadores de resolução e gateway | Manter SoraDNS, GAR e estado do gateway alinhado com eventos do registrador; transmitem métricas de transparência. | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md), `dashboards/alerts/soradns_transparency_rules.yml`. | SRE do resolvedor de plantão + ponte de operações do gateway. |
| Tesouraria e Finanças | Aplica o regulamento 70/30, exclusões de especificações, declarações fiscais/tesoreria e atestados de SLA. | Manifestações de acumulação de ingressos, exportações Stripe/tesoreria, apêndices KPI trimestrais em `docs/source/sns/regulatory/`. | Controlador de finanças + oficial de cumprimento. |
| Link de Cumprimento e Regulamentação | Rastrear obrigações globais (EU DSA, etc.), atualizar KPI de convênios e apresentar divulgações. | Memos reguladores em `docs/source/sns/regulatory/`, decks de referência, entradas `ops/drill-log.md` para ensaios de mesa. | Líder do programa de cumprimento. |
| Soporte / SRE Atendimento | Maneja incidentes (colisões, desvios de fatura, falhas de resolução), coordenação de mensagens para clientes e é duelo de runbooks. | Plantas de incidente, `ops/drill-log.md`, evidências de laboratório, transcrições Slack/war-room em `incident/`. | Rotação de plantão SNS + gestão SRE. |

## 3. Artefatos canônicos e fontes de dados| Artefato | Localização | Proposta |
|----------|----------|--------|
| Carta + anexos KPI | `docs/source/sns/governance_addenda/` | Cartas firmadas com controle de versão, KPI de convênios e decisões de governança referenciadas por votos de CLI. |
| Esquema de registro | [`registry-schema.md`](./registry-schema.md) | Estruturas canônicas Norito (`NameRecordV1`, `SuffixPolicyV1`, `RevenueAccrualEventV1`). |
| Contrato do registrador | [`registrar-api.md`](./registrar-api.md) | Cargas REST/gRPC, métricas `sns_registrar_status_total` e gancho de expectativas de governança. |
| Guia UX de direções | [`address-display-guidelines.md`](./address-display-guidelines.md) | Renderizados canônicos I105 (preferido) e comprimidos (segunda melhor opção) refletidos por carteiras/exploradores. |
| Documentos SoraDNS/GAR | [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md), [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md) | Derivação determinística de hosts, fluxo de controle de transparência e regras de alertas. |
| Memorandos regulatórios | `docs/source/sns/regulatory/` | Notas de ingresso jurisdiccional (p. ej., EU DSA), acusações de steward, anexos plantilla. |
| Registro de perfuração | `ops/drill-log.md` | Registro de ensaios de caos e IR exigidos antes de sair de fase. |
| Almacen de artefatos | `artifacts/sns/` | Testes de pagamento, tickets de guardião, diferenças de resolução, exportações de KPI e saída firmada de `sns governance ...`. |

Todas as ações de governo devem ser referenciadas, menos um artefato no
 tabela anterior para que os auditores reconstruam o rastro de decisão em 24
 horas.

## 4. Manual do ciclo de vida

### 4.1 Moções de carta e mordomo

| Passo | Dueno | CLI / Evidência | Notas |
|------|-------|----------------|-------|
| Editar adendo e deltas KPI | Relator do conselho + líder do administrador | Plantilla Markdown em `docs/source/sns/governance_addenda/YY/` | Inclui IDs de KPI de convênio, ganchos de telemetria e condições de ativação. |
| Apresentar proposta | Presidência do Conselho | `sns governance charter submit --input SN-CH-YYYY-NN.md` (produzir `CharterMotionV1`) | A CLI emite manifestações para Norito e `artifacts/sns/governance/<id>/charter_motion.json`. |
| Voto e reconhecimento de guardiões | Conselho + guardiões | `sns governance ballot cast --proposal <id>` e `sns governance guardian-ack --proposal <id>` | Adicionar minutos com hash e testes de quorum. |
| Aceitação de mordomo | Programa de administradores | `sns governance steward-ack --proposal <id> --signature <file>` | Requerido antes de mudar políticas de sufijos; salve em `artifacts/sns/governance/<id>/steward_ack.json`. |
| Ativação | Operações do registrador | Atualizar `SuffixPolicyV1`, atualizar caches do registrador, publicar nota em `status.md`. | Timestamp de ativação em `sns_governance_activation_total`. |
| Registro de auditoria | Cumprimento | Agregar entrada a `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md` e perfurar log se hubo tabletop. | Incluir referências a painéis de telemetria e diferenças políticas. |

### 4.2 Registros, subastas e aprovações de preços1. **Preflight:** O registrador consulta `SuffixPolicyV1` para confirmar o nível de
   precio, terminos disponibles y ventanas de gracia/redencion. Mantener hojas de
   preços sincronizados com a tabela de níveis 3/4/5/6-9/10+ (nível base +
   coeficientes de sufijo) documentados no roteiro.
2. **Subastas seal-bid:** Para pools premium, ejecutar o ciclo 72 h commit /
   Revelação de 24 horas via `sns governance auction commit` / `... reveal`. Publicar o
   lista de commits (hashes individuais) em `artifacts/sns/auctions/<name>/commit.json`
   para que os auditores verifiquem a randomização.
3. **Verificação de pagamento:** Os registradores validam `PaymentProofV1` contra o
   reparto de tesoreria (70% tesoreria / 30% steward con carve-out de referidos <=10%).
   Guardar o JSON Norito em `artifacts/sns/payments/<tx>.json` e ativá-lo
   resposta do registrador (`RevenueAccrualEventV1`).
4. **Gancho de governo:** Adjuntar `GovernanceHookV1` para nomes premium/protegidos
   com referências a ids de proposta de conselho e firmas de steward. Ganchos
   faltantes resultam em `sns_err_governance_missing`.
5. **Ativação + sincronização do resolvedor:** Uma vez que Torii emite o evento de
   registro, disparar o tailer de transparência do resolvedor para confirmar
   que o novo estado GAR/zone é anunciado (ver 4.5).
6. **Divulgação ao cliente:** Atualizar o razão orientado ao cliente
   (wallet/explorer) via los fixtures compartidos en
   [`address-display-guidelines.md`](./address-display-guidelines.md), garantindo
   que os renderizados I105 e comprimidos coincidem com o guia de cópia/QR.

### 4.3 Renovações, faturação e reconciliação de tesouraria- **Flujo de renovacion:** Os registradores aplicam as janelas de graça de
  30 dias + resgate de 60 dias especificados em `SuffixPolicyV1`. Depois de 60
  dias, la secuencia de reapertura holandesa (7 dias, tarifa 10x decadendo 15%/dia)
  é ativado automaticamente via `sns governance reopen`.
- **Repartição de ingressos:** Cada renovação ou transferência cria um
  `RevenueAccrualEventV1`. As exportações de tesouraria (CSV/Parquet) devem
  reconciliar esses eventos diariamente; adjuntar pruebas en
  `artifacts/sns/treasury/<date>.json`.
- **Separações de termos:** Porcentagens de referências opcionais são rastreadas por
  sufijo agregando `referral_share` à política de steward. Os registradores
  emite a divisão final e guarda os manifestos de referência junto à tentativa de pagamento.
- **Cadência de relatórios:** Finanzas publica anexos KPI mensais (registros,
  renovaciones, ARPU, uso de disputas/bond) en
  `docs/source/sns/regulatory/<suffix>/YYYY-MM.md`. Os painéis devem ser usados
  das tabelas mismas exportadas para que os números de Grafana coincidam com
  a evidência do razão.
- **Revisão KPI mensal:** El checkpoint del primer martes empareja al lider de
  finanças, administrador de turno e PM do programa. Abra o [painel SNS KPI](./kpi-dashboard.md)
  (incorporar o portal de `sns-kpis` / `dashboards/grafana/sns_suffix_analytics.json`),
  exportar as tabelas de rendimento e receita do registrador, registrador deltas en
  o anexo e adjuntar os artefatos ao memorando. Disparar um incidente se a revisão
  encontro brechas SLA (ventanas de congelamento >72 h, picos de erro do registrador,
  desvio de ARPU).

### 4.4 Congelações, disputas e apelações

| Fase | Dueno | Ação e evidência | SLA |
|-------|-------|-------------------|-----|
| Solicitação de congelamento suave | Mordomo / suporte | Apresentar ticket `SNS-DF-<id>` com testes de pagamento, referência de título de disputa e seletor(es) afetado(s). | <=4 horas desde o ingresso. |
| Bilhete do guardião | Junta de guardiões | `sns governance freeze --selector <I105> --reason <text> --until <ts>` produz `GuardianFreezeTicketV1`. Guarde o JSON do ticket em `artifacts/sns/guardian/<id>.json`. | <=30 min ACK, <=2 h de ejeção. |
| Ratificação do conselho | Conselho de governança | Aprovar ou rechazar congelamentos, documentar decisão enlazada ao bilhete do guardião e resumo do vínculo de disputa. | Próxima sessão do conselho o voto assíncrono. |
| Painel de arbitragem | Cumplimiento + mordomo | Convocar painel de 7 jurados (segun roadmap) com boletas hasheadas via `sns governance dispute ballot`. Adjuntar recibos de voto anonimizados ao pacote de incidente. | Veredicto <=7 dias após o depósito do título. |
| Apelação | Guardiões + conselhos | As apelações duplicam o vínculo e repetem o processo de jurados; registrar manifiesto Norito `DisputeAppealV1` y referenciar ticket primario. | <=10 dias. |
| Descongelar e remediar | Registrador + operações de resolução | Execute `sns governance unfreeze --selector <I105> --ticket <id>`, atualize o estado do registrador e propague diffs GAR/resolver. | Imediatamente após o veredicto. |Os cânones de emergência (congelamentos ativados por guardiões <=72 h) seguem
o mesmo fluxo, mas requer revisão retroativa do conselho e uma nota de
transparência em `docs/source/sns/regulatory/`.

### 4.5 Propagação de resolvedor e gateway

1. **Hook de evento:** Cada evento de registro emite al stream de eventos del
   resolvedor (`tools/soradns-resolver` SSE). As operações de resolução são inscritas e
   registrar diferenças via el tailer de transparência
   (`scripts/telemetry/run_soradns_transparency_tail.sh`).
2. **Atualização de planta GAR:** Gateways devem atualizar plantas GAR
   referenciadas por `canonical_gateway_suffix()` e reafirmar a lista
   `host_pattern`. Guardar diferenças em `artifacts/sns/gar/<date>.patch`.
3. **Publicação do arquivo de zona:** Usar o esqueleto do arquivo de zona descrito em
   `roadmap.md` (nome, ttl, cid, prova) e empurre-o para Torii/SoraFS. Arquivar o
   JSON Norito e `artifacts/sns/zonefiles/<name>/<version>.json`.
4. **Cheque de transparência:** Executar `promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml`
   para garantir que os alertas permaneçam verdes. Adjuntar a saída de texto
   de Prometheus ao relatório de transparência semanal.
5. **Auditoria de gateway:** Registrador de tabelas de cabeçalhos `Sora-*` (política de
   cache, CSP, resumo de GAR) e adjunto ao log de governança para que
   Os operadores podem provar que o gateway sirvio é o novo nome com os
   guarda-corpos previstos.

## 5. Telemetria e relatórios

| Senal | Fonte | Descrição / Ação |
|--------|--------|-----------|
| `sns_registrar_status_total{result,suffix}` | Manipuladores de registradores Torii | Contador de saída/erro para registros, renovações, congelamentos, transferências; alerta quando `result="error"` é substituído por sufixo. |
| `torii_request_duration_seconds{route="/v2/sns/*"}` | Métricas de Torii | SLOs de latência para API de manipuladores; alimenta painéis baseados em `torii_norito_rpc_observability.json`. |
| `soradns_bundle_proof_age_seconds` e `soradns_bundle_cid_drift_total` | Etiqueta de transparência do resolvedor | Detecta testes obsoletos ou desvios de GAR; guarda-corpos definidos em `dashboards/alerts/soradns_transparency_rules.yml`. |
| `sns_governance_activation_total` | CLI de governança | Contador incrementado quando um alvará/adendo é ativado; se usa para reconciliar decisões do conselho vs adendos publicados. |
| Medidor `guardian_freeze_active` | Guardião CLI | Rastrear janelas de congelamento suave/duro por seletor; paginar a SRE se o valor cair em `1` além do SLA declarado. |
| Painéis anexos de KPI | Finanças / Documentos | Rollups mensais publicados junto com memorandos regulatórios; o portal é incorporado via [painel SNS KPI](./kpi-dashboard.md) para que administradores e reguladores acessem a mesma vista de Grafana. |

## 6. Requisitos de evidência e auditoria| Ação | Evidência e arquivamento | Almacén |
|--------|----------------------|--------|
| Mudança de carta / política | Manifestado Norito firmado, transcrição CLI, KPI diff, acusação de administrador. | `artifacts/sns/governance/<proposal-id>/` + `docs/source/sns/governance_addenda/`. |
| Registro / renovação | Carga útil `RegisterNameRequestV1`, `RevenueAccrualEventV1`, teste de pagamento. | `artifacts/sns/payments/<tx>.json`, logs de API do registrador. |
| Subasta | Manifestos commit/reveal, semilla de aleatoriedad, planilha de cálculo de ganador. | `artifacts/sns/auctions/<name>/`. |
| Congelar / descongelar | Ticket de guardião, hash de voto do conselho, URL de registro de incidente, planta de comunicação ao cliente. | `artifacts/sns/guardian/<ticket>/`, `incident/<date>-sns-*.md`. |
| Propagação de resolução | Zonefile/GAR diff, extração JSONL do tailer, instantâneo Prometheus. | `artifacts/sns/resolver/<date>/` + relatórios de transparência. |
| Regulamentação de ingresso | Memorando de entrada, rastreador de prazos, aviso de administrador, resumo de mudanças KPI. | `docs/source/sns/regulatory/<jurisdiction>/<cycle>.md`. |

## 7. Checklist de portões de fase

| Fase | Critérios de saída | Pacote de evidências |
|-------|--------------------|--------------------|
| N0 - Beta cerrada | Esquema de registro SN-1/SN-2, CLI do registrador manual, broca de guardião completado. | Moção de carta + ACK do administrador, logs de simulação do registrador, relatório de transparência do resolvedor, entrada em `ops/drill-log.md`. |
| N1 - Lançamento público | Subastas + níveis de preço fijo ativos para `.sora`/`.nexus`, autoatendimento de registrador, sincronização automática de resolução, painéis de faturação. | Comparação de horas de preços, resultados CI do registrador, anexo de pagamentos/KPI, saída de alfaiataria de transparência, notas de ensaio de incidente. |
| N2 - Expansão | `.dao`, APIs de revendedor, portal de disputas, scorecards de steward, dashboards de análise. | Capturas do portal, métricas de SLA de disputas, exportações de scorecards de steward, carta de governo atualizada com políticas de revendedor. |

Las salidas de fase requerem brocas de mesa registradas (registro happy path,
congelar, interrupção do resolvedor) com artefatos adjuntos em `ops/drill-log.md`.

## 8. Resposta a incidentes e escalada| Gatilho | Severidade | Dueno imediato | Ações obrigatórias |
|--------|----------|----------------|----------------------|
| Deriva de resolução/GAR ou testes obsoletos | 1º de setembro | Resolver SRE + junta de guardiões | Paginar on-call del resolvedor, capturar saída do tailer, decidir se congelar nomes afetados, publicar estado a cada 30 min. |
| Caida de registrador, falha de fatura ou erros API generalizados | 1º de setembro | Gerente de serviço do registrador | Detener novas subastas, alterar o manual CLI, notificar stewards/tesoreria, adicionar logs de Torii ao documento de incidente. |
| Disputa de nome único, incompatibilidade de pagamento ou escalação de cliente | 2 de setembro | Comissário + líder de apoio | Recopilar testes de pagamento, decidir se há falta de congelamento suave, responder ao solicitador dentro do SLA, resultado do registrador no rastreador de disputa. |
| Sala de auditoria de cumprimento | 2 de setembro | Link de cumprimento | Redactar plano de remediação, arquivar memorando em `docs/source/sns/regulatory/`, agendar sessão de conselho de acompanhamento. |
| Broca ou ensaio | 3 de setembro | PM do programa | Execute o cenário guiado de `ops/drill-log.md`, arquivar artefatos, marcar lacunas como tarefas do roteiro. |

Todos os incidentes devem ser criados `incident/YYYY-MM-DD-sns-<slug>.md` com tabelas
de propriedade, registros de comandos e referências à evidência produzida neste
manual.

## 9. Referências

- [`registry-schema.md`](./registry-schema.md)
- [`registrar-api.md`](./registrar-api.md)
- [`address-display-guidelines.md`](./address-display-guidelines.md)
- [`docs/account_structure.md`](../../../account_structure.md)
- [`docs/source/soradns/deterministic_hosts.md`](../../../source/soradns/deterministic_hosts.md)
- [`docs/source/reports/soradns_transparency.md`](../../../source/reports/soradns_transparency.md)
-`ops/drill-log.md`
- `roadmap.md` (seções SNS, DG, ADDR)

Mantenha este manual atualizado quando o texto das cartas muda, as
superfícies de CLI ou contratos de telemetria; as entradas do roteiro que
referencial `docs/source/sns/governance_playbook.md` deve coincidir sempre com
a revisão mais recente.