---
lang: pt
direction: ltr
source: docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 06dcd662ffb2b22a13d9cd5418f5d2e8e64a4cdadb71a054488ce75b1eb96188
source_last_modified: "2025-11-07T11:07:58.852037+00:00"
translation_last_reviewed: 2026-01-01
---

# Relatorio stage-gate SNNet-10 (T?_ -> T?_)

> Substitua cada placeholder (itens entre < >) antes do envio. Mantenha os cabecalhos de secao para que a automacao de governance possa fazer parse do arquivo.

## 1. Metadados

| Campo | Valor |
|-------|-------|
| Promocao | `<T0->T1 ou T1->T2>` |
| Janela de reporte | `<YYYY-MM-DD -> YYYY-MM-DD>` |
| Relays no escopo | `<count + IDs ou "ver apendice A">` |
| Contato principal | `<name/email/Matrix handle>` |
| Arquivo de envio | `<snnet10-stage-gate-YYYYMMDD.tar.zst>` |
| Arquivo SHA-256 | `<sha256:...>` |

## 2. Resumo de metricas

| Metrica | Observado | Limite | Passa? | Fonte |
|--------|----------|--------|-------|--------|
| Ratio de sucesso de circuitos | `<0.000>` | >=0.95 | N / Y | `reports/metrics-report.json` |
| Ratio de brownout de fetch | `<0.000>` | <=0.01 | N / Y | `reports/metrics-report.json` |
| Variancia do mix GAR | `<+0.0%>` | <=+/-10% | N / Y | `reports/metrics-report.json` |
| PoW p95 em segundos | `<0.0 s>` | <=3 s | N / Y | `telemetry/pow_window.json` |
| Latencia p95 | `<0 ms>` | <200 ms | N / Y | `telemetry/latency_window.json` |
| Ratio PQ (media) | `<0.00>` | >= target | N / Y | `telemetry/pq_summary.json` |

**Narrativa:** `<summaries of anomalies, mitigations, overrides>`

## 3. Registro de drill e incidentes

| Timestamp (UTC) | Regiao | Tipo | Alert ID | Resumo de mitigacao |
|-----------------|--------|------|----------|---------------------|
| `<YYYY-MM-DD HH:MM>` | `<region>` | `Brownout drill` | `<alert://...>` | `<restored anon-guard-pq in 3m12s>` |

## 4. Anexos e hashes

| Artefato | Caminho | SHA-256 |
|----------|--------|---------|
| Snapshot de metricas | `reports/metrics-window.json` | `<sha256>` |
| Relatorio de metricas | `reports/metrics-report.json` | `<sha256>` |
| Transcricoes de guard rotation | `evidence/guard_rotation/*.log` | `<sha256>` |
| Manifests de exit bonding | `evidence/exit_bonds/*.to` | `<sha256>` |
| Logs de drill | `evidence/drills/*.md` | `<sha256>` |
| MASQUE readiness (T1->T2) | `reports/masque-readiness.md` | `<sha256 or n/a>` |
| Plano de rollback (T1->T2) | `reports/downgrade_plan.md` | `<sha256 or n/a>` |

## 5. Aprovacoes

| Funcao | Nome | Assinado (S/N) | Notas |
|--------|------|---------------|-------|
| Networking TL | `<name>` | N / Y | `<comments>` |
| Governance rep | `<name>` | N / Y | `<comments>` |
| SRE delegate | `<name>` | N / Y | `<comments>` |

## Apendice A - Lista de relays

```
- relay-id-001 (AS64496, region=SJC)
- relay-id-002 (AS64497, region=NRT)
...
```

## Apendice B - Resumos de incidentes

```
<Detailed context for any incidents or overrides referenced above.>
```
