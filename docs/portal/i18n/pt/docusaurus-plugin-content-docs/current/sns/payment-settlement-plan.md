---
id: payment-settlement-plan
lang: pt
direction: ltr
source: docs/portal/docs/sns/payment-settlement-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Plano de pagamento e liquidacao SNS

> Fonte canonica: [`docs/source/sns/payment_settlement_plan.md`](../../../source/sns/payment_settlement_plan.md).

A tarefa do roadmap **SN-5 -- Payment & Settlement Service** introduz uma camada
de pagamento determinista para o Sora Name Service. Cada registro, renovacao ou
reembolso deve emitir um payload Norito estruturado para que tesouraria, stewards
e governanca possam reproduzir os fluxos financeiros sem planilhas. Esta pagina
condensa a especificacao para o publico do portal.

## Modelo de receita

- A taxa base (`gross_fee`) deriva da matriz de precos do registrar.
- Tesouraria recebe `gross_fee x 0.70`, stewards recebem o restante menos
  bonuses de referral (limitados a 10 %).
- Holdbacks opcionais permitem a governanca pausar pagamentos a stewards durante
  disputas.
- Bundles de settlement expoem um bloco `ledger_projection` com ISIs `Transfer`
  concretos para que a automacao poste movimentos XOR direto no Torii.

## Servicos e automacao

| Componente | Objetivo | Evidencia |
|------------|----------|-----------|
| `sns_settlementd` | Aplica politica, assina bundles, expone `/v1/sns/settlements`. | Bundle JSON + hash. |
| Settlement queue & writer | Fila idempotente + submitter do ledger conduzido por `iroha_cli app sns settlement ledger`. | Manifesto de bundle hash <-> tx hash. |
| Reconciliation job | Diff diario + demonstrativo mensal sob `docs/source/sns/reports/`. | Markdown + JSON digest. |
| Refund desk | Reembolsos aprovados por governanca via `/settlements/{id}/refund`. | `RefundRecordV1` + ticket. |

Os helpers de CI espelham esses fluxos:

```bash
# Quote & ledger projection
iroha_cli app sns settlement quote --selector makoto.sora --term-years 1 --pricing hot-tier-a

# Emit transfers for automation/pipeline
iroha_cli app sns settlement ledger --bundle artifacts/sns/settlements/2026-05/makoto.sora.json

# Produce a reconciliation statement
iroha_cli app sns settlement reconcile --period 2026-05 --out docs/source/sns/reports/settlement_202605.md
```

## Observabilidade e reporting

- Dashboards: `dashboards/grafana/sns_payment_settlement.json` para totais de
  tesouraria vs stewards, pagamentos de referral, profundidade de fila e
  latencia de reembolso.
- Alertas: `dashboards/alerts/sns_payment_settlement_rules.yml` monitora idade
  pendente, falhas de reconciliacao e deriva do ledger.
- Demonstrativos: digests diarios (`settlement_YYYYMMDD.{json,md}`) entram em
  relatorios mensais (`settlement_YYYYMM.md`) que sao enviados ao Git e ao
  storage de objetos de governanca (`s3://sora-governance/sns/settlements/<period>/`).
- Pacotes de governanca agrupam dashboards, logs de CLI e aprovacoes antes do
  sign-off do council.

## Checklist de rollout

1. Prototipar helpers de quote + ledger e capturar um bundle de staging.
2. Lancar `sns_settlementd` com queue + writer, conectar dashboards e executar
   testes de alertas (`promtool test rules ...`).
3. Entregar o helper de reembolso mais o template de demonstrativo mensal; espelhar
   artefatos em `docs/portal/docs/sns/reports/`.
4. Executar um ensaio com parceiros (mes completo de settlements) e capturar o
   voto de governanca marcando SN-5 como concluido.

Consulte o documento fonte para as definicoes exatas de esquema, questoes
abertas e futuras emendas.
