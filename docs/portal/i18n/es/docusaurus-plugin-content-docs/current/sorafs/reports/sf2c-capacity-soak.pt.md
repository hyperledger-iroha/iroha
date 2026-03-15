---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatorio de remojo de acumulacao de capacidade SF-2c

Datos: 2026-03-21

##escopo

Este relatorio registra los testes determinísticos de remojo de acumulación y pago de capacidade SoraFS
Solicitados na trilha SF-2c en la hoja de ruta.

- **Remojo multiproveedor de 30 días:** Ejecutado por
  `capacity_fee_ledger_30_day_soak_deterministic` en
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  O arnés cinco instancias proveedores, cobre 30 janelas de asentamiento e
  valida que os totais do ledger corresponden a un proyecto de referencia
  calculada de forma independiente. O teste emite un resumen Blake3
  (`capacity_soak_digest=...`) para que un CI pueda capturar y comparar la instantánea
  canónico.
- **Penalidades por subentrega:** Aplicadas por
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (meso archivo). O teste confirma que limiares de strikes, cooldowns, slashes
  de colateral y contadores do ledger permanecenm determinísticos.

## Ejecución

Ejecutar como validacoes de remojo localmente com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes completem em less de um segundo em um laptop padrao e nao exigem
accesorios externos.

## Observabilidade

Torii ahora expone instantáneas de crédito de proveedores junto a libros de contabilidad de tarifas para que os paneles
possam fazer gate em saldos bajos y strikes de penalización:- REST: `GET /v1/sorafs/capacity/state` retorna entradas `credit_ledger[*]` que
  Refleje los campos del libro mayor verificados sin prueba de remojo. Veja
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importacao Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` sistema operativo de trama
  contadores de huelgas exportados, totais de penalidades e colateral preso para que o
  El tiempo de guardia permite comparar las líneas base de remojo con los ambientes en producción.

## Seguimiento

- Agendar ejecuciones semanales de puerta en CI para reejecutar la prueba de remojo (nivel de humo).
- Estender o Painel Grafana con alvos de scrape de Torii asim que as exportacoes de telemetry de producao entrarem em operacao.