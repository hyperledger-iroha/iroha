---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/sf2c-capacity-soak.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Relatório de absorção de acumulação de capacidade SF-2c

Data: 21/03/2026

## Portée

Este relatório envia os testes determinados de absorção de acumulação e pagamento de capacidade SoraFS exigidos
na folha da rota SF-2c.

- **Mergulhe multi-provedor por 30 dias:** Executado por
  `capacity_fee_ledger_30_day_soak_deterministic` em
  `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`.
  Os provedores de instância Cinq aproveitam, cobrem 30 janelas de liquidação e
  valide que todos os livros contábeis correspondem a uma projeção de referência
  calculado indépendamment. O teste foi realizado com um resumo Blake3 (`capacity_soak_digest=...`)
  então o CI pode capturar e comparar o snapshot canônico.
- **Pénalités de sous-livraison:** Appliquées par
  `record_capacity_telemetry_penalises_persistent_under_delivery`
  (mesmo arquivo). O teste confirmou que os seus ataques, cooldowns,
  barras de garantia e contas do livro-razão restantes deterministas.

## Execução

Relance as validações de localização de imersão com:

```bash
cargo test -p iroha_core -- record_capacity_telemetry_penalises_persistent_under_delivery
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
```

Os testes terminam em menos de um segundo em um laptop padrão e não
necessário nenhum acessório externo.

## Observabilidade

Torii expõe a manutenção de instantâneos de provedores de crédito nos registros de taxas nos painéis
puissent gate sur les faibles soldes et penal strikes:

- REST: `GET /v1/sorafs/capacity/state` envio de entradas `credit_ledger[*]` aqui
  reflete os campos do livro-razão verificados no teste de imersão. Voir
  `crates/iroha_torii/src/sorafs/registry.rs`.
- Importar arquivos de rastreamento Grafana: `dashboards/grafana/sorafs_capacity_penalties.json`
  administradores de greves exportados, o total de penalidades e as garantias contratadas para que
  A equipe de plantão pode comparar as linhas de base de imersão com os ambientes ao vivo.

## Suivi

- Planeje as execuções hebdomadaires de gate en CI para refazer o teste de imersão (camada de fumaça).
- Crie o quadro Grafana com os cabos de raspagem Torii uma vez que as exportações de telemetria de produção
  seront en ligne.