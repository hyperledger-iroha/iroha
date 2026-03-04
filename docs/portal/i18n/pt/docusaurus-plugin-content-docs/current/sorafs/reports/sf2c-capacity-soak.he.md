---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c55d6b54734958bcf65aec294ebbffaf73593639ef98cc7e370d2d0230f88ad8
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Relatorio de soak de acumulacao de capacidade SF-2c

Data: 2026-03-21

## Escopo

Este relatorio registra os testes deterministicos de soak de acumulacao e pagamento de capacidade SoraFS
solicitados na trilha SF-2c do roadmap.

- **Soak multi-provider de 30 dias:** Executado por
  `capacity_fee_ledger_30_day_soak_deterministic` em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  O harness instancia cinco providers, cobre 30 janelas de settlement e
  valida que os totais do ledger correspondam a uma projecao de referencia
  calculada de forma independente. O teste emite um digest Blake3
  (`capacity_soak_digest=...`) para que a CI possa capturar e comparar o snapshot
  canonico.
- **Penalidades por subentrega:** Aplicadas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mesmo arquivo). O teste confirma que limiares de strikes, cooldowns, slashes
  de collateral e contadores do ledger permanecem deterministicos.

## Execucao

Execute as validacoes de soak localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes completam em menos de um segundo em um laptop padrao e nao exigem
fixtures externas.

## Observabilidade

Torii agora expoe snapshots de credito de providers junto a fee ledgers para que os dashboards
possam fazer gate em saldos baixos e penalty strikes:

- REST: `GET /v1/sorafs/capacity/state` retorna entradas `credit_ledger[*]` que
  refletem os campos do ledger verificados no teste de soak. Veja
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importacao Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` plota os
  contadores de strikes exportados, totais de penalidades e collateral preso para que o
  time on-call possa comparar os baselines de soak com ambientes em producao.

## Follow-up

- Agendar execucoes semanais de gate em CI para reexecutar o teste de soak (smoke-tier).
- Estender o painel Grafana com alvos de scrape de Torii assim que as exportacoes de telemetry de producao entrarem em operacao.
