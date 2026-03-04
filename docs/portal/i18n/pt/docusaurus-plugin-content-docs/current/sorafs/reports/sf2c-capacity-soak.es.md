---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Informe de absorção de acumulação de capacidade SF-2c

Data: 21/03/2026

## Alcance

Este relatório registra os testes determinísticos de absorção de acumulação de capacidade SoraFS e pagamentos
solicitados abaixo da estrada SF-2c.

- **Soak multi-provedor de 30 dias:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` pt
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  El chicote instância cinco provedores, abarca 30 janelas de liquidação e
  valida que os totais do razão coincidem com uma projeção de referência
  cálculo de forma independente. A tentativa de emitir um resumo Blake3
  (`capacity_soak_digest=...`) para que o CI possa capturar e comparar o snapshot
  canônico.
- **Penalizaciones por subentrega:** Impuestas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mismo arquivo). A tentativa confirma que os guarda-chuvas de golpes, cooldowns,
  barras de garantia e contadores do razão permanecem determinísticos.

## Execução

Execute as validações de imersão localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes são realizados em menos de um segundo em um laptop padrão e não exigem
luminárias externas.

## Observabilidade

Torii agora exponha snapshots de crédito de provedores junto com registros de taxas para os painéis
puedan gatear sobre saldos baixos e pênaltis:

- REST: `GET /v1/sorafs/capacity/state` devolve entradas `credit_ledger[*]` que
  reflita sobre os campos do razão selecionado na tentativa de imersão. Ver
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importação de Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` gráficos
  contadores de greves exportados, totais de penalizações e garantias para que o
  O equipamento on-call pode comparar as linhas de base de imersão com ambientes ao vivo.

## Seguimento

- Programar ejecuciones de gate semanales en CI para reejecutar a tentativa de molho (smoke-tier).
- Estenda a mesa de Grafana com objetivos de raspar de Torii quando exportar
  a telemetria de produção está disponível.