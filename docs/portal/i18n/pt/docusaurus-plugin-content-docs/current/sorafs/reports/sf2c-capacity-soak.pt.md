---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatório de absorção de acumulação de capacidade SF-2c

Dados: 21/03/2026

## Escopo

Este relatorio registra os testes determinísticos de absorção de acumulação e pagamento de capacidade SoraFS
solicitados na trilha SF-2c do roadmap.

- **Soak multi-provider de 30 dias:** Executado por
  `capacity_fee_ledger_30_day_soak_deterministic` em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  O chicote instância cinco provedores, cobre 30 janelas de liquidação e
  valida que os totais do razão correspondem a uma projeção de referência
  calculado de forma independente. O teste emite um digest Blake3
  (`capacity_soak_digest=...`) para que um CI possa capturar e comparar o snapshot
  canônico.
- **Penalidades por subentrega:** Aplicadas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mesmo arquivo). O teste confirma que limites de strikes, cooldowns, slashes
  de garantias e contadores do razão permanecem determinísticos.

## Execução

Execute as validacoes de molho localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes completaram em menos de um segundo em um laptop padrão e não desligaram
luminárias externas.

## Observabilidade

Torii agora expõe snapshots de crédito de provedores junto a ledgers de taxas para que os dashboards
pode fazer gate em saldos baixos e pênaltis:

- REST: `GET /v1/sorafs/capacity/state` retorna entradas `credit_ledger[*]` que
  refletem os campos do razão selecionados no teste de imersão. Veja
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importação Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` plota os
  contadores de greves exportados, totais de ganhos e garantias presas para que o
  time on-call pode comparar as linhas de base de imersão com ambientes em produção.

## Acompanhamento

- Agendar execuções semanais de portão em CI para reexecutar o teste de molho (smoke-tier).
- Estender o painel Grafana com alvos de raspagem de Torii assim que as exportações de telemetria de produção entrarem em operação.