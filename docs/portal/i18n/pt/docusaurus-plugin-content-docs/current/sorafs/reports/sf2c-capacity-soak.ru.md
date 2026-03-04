---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Deixe de molho SF-2c

Data: 21/03/2026

##Oblado

Isso foi feito para determinar a determinação dos testes embebendo a tinta SoraFS e выплат,
Coloque no cartão SF-2c.

- **30 dias de imersão multi-provedor:** Запускается
  `capacity_fee_ledger_30_day_soak_deterministic` em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Harness создает пять provedores, охватывает 30 окон liquidação e
  prove que este livro-razão é fornecido com um valor inestimável
  proекцией. Teste o Blake3 digest (`capacity_soak_digest=...`), CI
  Você pode capturar e capturar instantâneos canônicos.
- **Штрафы за недопоставку:** Обеспечиваются
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (isso é verdade). Тест подтверждает, что пороги strikes, cooldowns, slashes
  garantias e registros contábeis são determinados.

## Aprovação

Запустите проверки mergulhe локально:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes são necessários para o mês em segundo lugar no padrão, sem necessidade e sem necessidade
luminárias.

## Наблюдаемость

Torii теперь показывает instantâneos provedores de crédito вместе с taxas, чтобы
painéis de controle têm portão com equilíbrio e penalidades:

- REST: `GET /v1/sorafs/capacity/state` é definido como `credit_ledger[*]`,
  Depois de abrir o livro-razão, verifique no teste de imersão. Sim.
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importar Grafana: `dashboards/grafana/sorafs_capacity_penalties.json` estrutura
  экспортированные счетчики greves, суммы штрафов e залог garantia, чтобы
  дежурная команда могла сравнивать imersão de linha de base com живыми окружениями.

## Дальнейшие шаги

- Coloque o portão de segurança no CI para absorver o teste (camada de fumaça).
- Расширить панель Grafana целями raspar Torii после запуска эксportов telemetria в прод.