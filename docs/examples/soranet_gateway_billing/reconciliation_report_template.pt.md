---
lang: pt
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-11-21T12:24:49.353535+00:00"
translation_last_reviewed: 2026-01-01
---

# Reconciliacao de faturamento do gateway SoraGlobal

- **Janela:** `<from>/<to>`
- **Tenant:** `<tenant-id>`
- **Versao do catalogo:** `<catalog-version>`
- **Snapshot de uso:** `<path or hash>`
- **Guardrails:** soft cap `<soft-cap-xor> XOR`, hard cap `<hard-cap-xor> XOR`, limiar de alerta `<alert-threshold>%`
- **Pagador -> Tesouraria:** `<payer>` -> `<treasury>` em `<asset-definition>`
- **Total devido:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Checagens de line items
- [ ] Entradas de uso cobrem apenas ids de medidor do catalogo e regioes de faturamento validas
- [ ] Unidades de quantidade batem com definicoes do catalogo (requests, GiB, ms, etc.)
- [ ] Multiplicadores de regiao e tiers de desconto aplicados conforme catalogo
- [ ] Exports CSV/Parquet batem com os line items da fatura JSON

## Avaliacao de guardrails
- [ ] Limiar de alerta do soft cap atingido? `<yes/no>` (anexe evidencia do alerta se yes)
- [ ] Hard cap excedido? `<yes/no>` (se yes, anexe aprovacao de override)
- [ ] Piso minimo de fatura satisfeito

## Projecao de ledger
- [ ] Total do lote de transferencias igual a `total_micros` na fatura
- [ ] Definicao do asset coincide com a moeda de faturamento
- [ ] Contas do pagador e tesouraria coincidem com tenant e operador registrado
- [ ] Artefatos Norito/JSON anexados para replay de auditoria

## Notas de disputa/ajuste
- Variacao observada: `<variance detail>`
- Ajuste proposto: `<delta and rationale>`
- Evidencias de suporte: `<logs/dashboards/alerts>`

## Aprovacoes
- Analista de faturamento: `<name + signature>`
- Revisor de tesouraria: `<name + signature>`
- Hash do pacote de governance: `<hash/reference>`
