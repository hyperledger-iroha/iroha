---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/deal-engine.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: מנוע עסקה
כותרת: Motor de acordos da SoraFS
sidebar_label: Motor de acordos
תיאור: Visao geral do motor de acordos SF-8, integracao com Torii e superficies de telemetria.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/deal_engine.md`. Mantenha ambos os locais alinhados enquanto a documentacao alternative permanecer ativa.
:::

# Motor de acordos da SoraFS

O מסלול לעשות מפת הדרכים SF-8 introduz o Motor de acordos da SoraFS, fornecendo
contabilidade deterministica para acordos de armazenamento e recuperacao entre
לקוחות e provedores. Os acordos sao descritos com os loadloads Norito
definidos em `crates/sorafs_manifest/src/deal.rs`, cobrindo termos do acordo,
בלוקוויו דה איגרות חוב, micropagamentos probabilisticos ו registros de liquidacao.

O worker embutido da SoraFS (`sorafs_node::NodeHandle`) agora instancia um
`DealEngine` עבור תהליך תהליך הנודו. O מנוע:

- valida e registra acordos usando `DealTermsV1`;
- acumula cobrancas denominadas em XOR quando o uso de replicacao e reportado;
- avalia janelas de micropagamento probabilistico usando amostragem deterministica
  baseada em BLAKE3; ה
- produz תמונת מצב של Ledger e Payloads de liquidacao adequados para publicacao
  דה גוברננקה.

Testes unitarios cobrem validacao, selecao de micropagamentos e fluxos de liquidacao para
que מפעיל את possam exercitar כמו APIs com confianca. Liquidacoes agora emitem
מטענים של גוברננקה `DealSettlementV1`, קונקטנדו ניהול וצינור דה
publicacao SF-12, e atualizam a series OpenTelemetry `sorafs.node.deal_*`
(`deal_settlements_total`, `deal_expected_charge_nano`, `deal_client_debit_nano`,
`deal_outstanding_nano`, `deal_bond_slash_nano`, `deal_publish_total`) עבור לוחות מחוונים לעשות Torii e
aplicacao de SLOs. Os itens seguintes focam na automatizacao de slashing iniciada por
auditores e na coordenacao de semanticas de cancelamento com a politica de governanca.

טלמטריה דה אוסו אגורה טמבם alimenta o conjunto de metricas `sorafs.node.micropayment_*`:
`micropayment_charge_nano`, `micropayment_credit_generated_nano`,
`micropayment_credit_applied_nano`, `micropayment_credit_carry_nano`,
`micropayment_outstanding_nano`, e os contadores de tickets
(`micropayment_tickets_processed_total`, `micropayment_tickets_won_total`,
`micropayment_tickets_duplicate_total`). Esses totais expoem o fluxo de loteria
probabilistica para que operadores possam correlacionar ganhos de micropagamento e
העברה דה קרדיטו com resultados de liquidacao.

## Integracao com Torii

Torii נקודות קצה של חשיפה לגילוי נקודות קצה למטרות הוכחות דיווחים לשימוש וטיפול
חיווט ciclo de vida do acordo sem sob medida:- `POST /v1/sorafs/deal/usage` aceita telemetria `DealUsageReport` e retorna
  resultados deterministas de contabilidade (`UsageOutcome`).
- `POST /v1/sorafs/deal/settle` סיים את ג'אנלה, העברה
  `DealSettlementRecord` resultante junto com um `DealSettlementV1` em base64
  pronto para publicacao no DAG de governanca.
- O feed `/v1/events/sse` do Torii agora transmite registros `SorafsGatewayEvent::DealUsage`
  resumindo cada envio de uso (תקופה, שעות GiB, Medidos, Contadores de Tickets,
  cobrancas deterministas), registros `SorafsGatewayEvent::DealSettlement`
  que incluem o canonico do Ledger de liquidacao mais o digest/tamanho/base64
  BLAKE3 לעשות ארטפטו דה גוברננקה עם דיסקו, e alertas `SorafsGatewayEvent::ProofHealth`
  semper que limiares PDP/PoTR sao excedidos (פרודור, ג'אנלה, estado de strike/cooldown,
  valor da penalidade). צריכת מסננים למטרות חדשות
  telemetria, liquidacoes או alertas de saude de proofs sem polling.

נקודות הקצה של Ambos OS משתתפות לעשות את Framework de cotas da SoraFS דרך a nova Janela
`torii.sorafs.quota.deal_telemetry`, מותר להפעיל התאמה אישית
permitida por deploy.