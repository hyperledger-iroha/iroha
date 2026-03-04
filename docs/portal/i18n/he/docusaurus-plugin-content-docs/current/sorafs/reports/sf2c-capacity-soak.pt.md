---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatorio de soak de acumulacao de capacidade SF-2c

נתונים: 21-03-2026

## אסקופו

Este relatorio registra os testes deterministicos de soak de acumulacao e pagamento de capacidade SoraFS
הצעות לטרילה SF-2c לעשות מפת דרכים.

- **Saak multi-provider de 30 dias:** Executado por
  `capacity_fee_ledger_30_day_soak_deterministic` em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  O רתמה instancia cinco ספקים, cobre 30 janelas de settlement e
  valida que os totais do financial correspondam a uma projecao de referencia
  calculada de forma independente. O teste emite um digest Blake3
  (`capacity_soak_digest=...`) עבור CI possa capturar e comparar או תמונת מצב
  canonico.
- **Penalidades por subentrega:** Aplicadas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mesmo arquivo). O teste confirma que limiares de strikes, cooldowns, slashes
  de collateral e contadores לעשות פנקס חשבונות קבוע לקבוע.

## Execucao

בצע כ-validacoes de soak localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes completam em menos de um segundo em um laptop padrao e nao exigem
מתקנים חיצוניים.

## תצפית

Torii תצלומי מצב של agora expoe de credito de providers junto a fee books para que os dashboards
possam fazer gate em saldos baixos e penalty strikes:

- מנוחה: `GET /v1/sorafs/capacity/state` retorna entradas `credit_ledger[*]` que
  refletem os campos do book en verificados no teste de soak. Veja
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importacao Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` plota os
  contadores de strikes exportados, totais de penalidades e collateral preso para que o
  זמן כוננות יכול להשוות בין קווי היסוד של ספיגה com ambientes עם producao.

## מעקב

- סדר יום execucoes semanais de gate em CI para reexecutar o teste de soak (עשן שכבת).
- Estender o painel Grafana com alvos de scrape de Torii assim que as exportacoes de telemetry de producao entrarem em operacao.