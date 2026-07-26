---
lang: pt
direction: ltr
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 Chicote SLO

A linha de lançamento Iroha 3 carrega SLOs explícitos para os caminhos críticos Nexus:

- duração do slot de finalidade (cadência NX‑18)
- verificação de provas (certificados de confirmação, atestados JDG, provas de ponte)
- manipulação de endpoint de prova (proxy de caminho Axum por meio de latência de verificação)
- taxas e caminhos de staking (fluxos de pagador/patrocinador e títulos/slash)

## Orçamentos

Os orçamentos ficam em `benchmarks/i3/slo_budgets.json` e são mapeados diretamente para a bancada
cenários no conjunto I3. Os objetivos são metas p99 por chamada:

- Taxa/staking: 50 ms por chamada (`fee_payer`, `fee_sponsor`, `staking_bond`, `staking_slash`)
- Confirmação de certificado/JDG/verificação de ponte: 80ms (`commit_cert_verify`, `jdg_attestation_verify`,
  `bridge_proof_verify`)
- Confirmar montagem do certificado: 80ms (`commit_cert_assembly`)
- Agendador de acesso: 50ms (`access_scheduler`)
Proxy de endpoint de prova: 120 ms (`torii_proof_endpoint`)

Dicas de taxa de gravação (`burn_rate_fast`/`burn_rate_slow`) codificam 14.4/6.0
proporções de múltiplas janelas para alertas de paginação versus alertas de tickets.

## Arnês

Execute o chicote via `cargo xtask i3-slo-harness`:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

Saídas:

- `bench_report.json|csv|md` — resultados brutos do conjunto de bancada I3 (git hash + cenários)
- `slo_report.json|md` — Avaliação de SLO com proporção de aprovação/reprovação/orçamento por meta

O chicote consome o arquivo de orçamentos e aplica `benchmarks/i3/slo_thresholds.json`
durante a corrida de bancada para falhar rapidamente quando um alvo regride.

## Telemetria e painéis

- Finalidade: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- Verificação do comprovante: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Os painéis iniciais Grafana residem em `dashboards/grafana/i3_slo.json`. Prometheus
alertas de taxa de queima são fornecidos em `dashboards/alerts/i3_slo_burn.yml` com o
orçamentos acima incorporados (finalidade 2s, verificação de prova 80ms, proxy de endpoint de prova
120ms).

## Notas operacionais

- Passe o arnês nas noites; publicar `artifacts/i3_slo/<stamp>/slo_report.md`
  juntamente com os artefatos de bancada para evidências de governança.
- Se um orçamento falhar, use a redução de referência para identificar o cenário e, em seguida, analise
  no painel/alerta Grafana correspondente para correlacionar com métricas ao vivo.
- Os SLOs de endpoint de prova usam a latência de verificação como um proxy para evitar por rota
  explosão de cardinalidade; a meta de referência (120 ms) corresponde à retenção/DoS
  proteções na API de prova.