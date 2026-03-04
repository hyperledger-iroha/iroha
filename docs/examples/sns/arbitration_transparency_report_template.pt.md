---
lang: pt
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-11-15T07:34:14.070551+00:00"
translation_last_reviewed: 2026-01-01
---

# Relatorio de transparencia de arbitragem SNS - <Mes YYYY>

- **Sufixo:** `<.sora / .nexus / .dao>`
- **Janela de reporte:** `<ISO start>` -> `<ISO end>`
- **Preparado por:** `<Council liaison>`
- **Artefatos de origem:** `cases.ndjson` SHA256 `<hash>`, export de dashboard `<filename>.json`

## 1. Resumo executivo

- Total de novos casos: `<count>`
- Casos fechados neste periodo: `<count>`
- Conformidade de SLA: `<ack %>` acknowledgement / `<resolution %>` decision
- Overrides do guardian emitidos: `<count>`
- Transferencias/reembolsos executados: `<count>`

## 2. Mix de casos

| Tipo de disputa | Novos casos | Casos fechados | Resolucao mediana (dias) |
|-----------------|-------------|---------------|--------------------------|
| Propriedade | 0 | 0 | 0 |
| Violacao de politica | 0 | 0 | 0 |
| Abuso | 0 | 0 | 0 |
| Faturamento | 0 | 0 | 0 |
| Outro | 0 | 0 | 0 |

## 3. Desempenho de SLA

| Prioridade | SLA de acuse | Cumprido | SLA de resolucao | Cumprido | Violacoes |
|------------|--------------|----------|------------------|----------|-----------|
| Urgente | <= 2 h | 0% | <= 72 h | 0% | 0 |
| Alta | <= 8 h | 0% | <= 10 d | 0% | 0 |
| Padrao | <= 24 h | 0% | <= 21 d | 0% | 0 |
| Info | <= 3 d | 0% | <= 30 d | 0% | 0 |

Descreva causas raiz de qualquer violacao e vincule tickets de remediacao.

## 4. Registro de casos

| ID do caso | Seletor | Prioridade | Status | Resultado | Notas |
|-----------|---------|-----------|--------|----------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Padrao | Fechado | Confirmado | `<summary>` |

Forneca notas de uma linha que referenciem fatos anonimizados ou links de votos publicos.
Sele onde necessario e mencione redacoes aplicadas.

## 5. Acoes e remedios

- **Congelamentos / liberacoes:** `<counts + case ids>`
- **Transferencias:** `<counts + assets moved>`
- **Ajustes de faturamento:** `<credits/debits>`
- **Follow-ups de politica:** `<tickets or RFCs opened>`

## 6. Apelacoes e overrides do guardian

Resuma apelacoes escaladas ao guardian board, incluindo timestamps e decisoes
(approve/deny). Vincule registros de `sns governance appeal` ou votos do council.

## 7. Itens pendentes

- `<Action item>` - Owner `<name>`, ETA `<date>`
- `<Action item>` - Owner `<name>`, ETA `<date>`

Anexe NDJSON, exports do Grafana e logs de CLI referenciados neste relatorio.
