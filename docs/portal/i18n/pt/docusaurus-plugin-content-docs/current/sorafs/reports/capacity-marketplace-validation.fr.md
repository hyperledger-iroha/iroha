---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validação da marcha de capacidade SoraFS
tags: [SF-2c, aceitação, lista de verificação]
resumo: Checklist de aceitação cobrindo a integração de provedores, o fluxo de litígios e a reconciliação de tesouro condicionando a disponibilidade geral da marcha de capacidade SoraFS.
---

# Checklist de validação da marcha de capacidade SoraFS

**Fenetre de revue:** 18/03/2026 -> 24/03/2026  
**Responsáveis do programa:** Equipe de armazenamento (`@storage-wg`), Conselho de governança (`@council`), Associação do Tesouro (`@treasury`)  
**Porte:** Pipelines de integração de provedores, fluxo de julgamento de litígios e processo de reconciliação de tesouro exigido para o GA SF-2c.

A lista de verificação aqui é feita antes de ativar a marcha para operações externas. Cada linha de envio para uma evidência determinada (testes, acessórios ou documentação) que os auditores podem refazer.

## Checklist de aceitação

### Integração de provedores

| Verifique | Validação | Evidência |
|-------|------------|----------|
| O registro aceita declarações canônicas de capacidade | O teste de integração executa `/v1/sorafs/capacity/declare` por meio da API do aplicativo, verificando o gerenciamento de assinaturas, a captura de metadados e a transferência para o registro do nó. | `crates/iroha_torii/src/routing.rs:7654` |
| O contrato inteligente rejeita cargas incoerentes | O teste unitário garante que os IDs do provedor e os campos GiB se envolvam na declaração assinada antes da persistência. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Le CLI emet des artefacts d'onboarding canonices | O chicote CLI escreve as saídas Norito/JSON/Base64 determina e valida as viagens de ida e volta para que os operadores possam preparar as declarações offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| O guia operacional sobre o fluxo de trabalho de admissão e os procedimentos de governança | A documentação enumera o esquema de declaração, os padrões de política e as etapas de revisão para o conselho. | `../storage-capacity-marketplace.md` |

### Resolução de litígios

| Verifique | Validação | Evidência |
|-------|------------|----------|
| Registros de litígio persistentes com um resumo canônico de carga útil | O teste unitário registra um litígio, decodifica o estoque de carga útil e afirma o status pendente para garantir o determinismo do razão. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| O gerador de litígios da CLI corresponde ao esquema canônico | A CLI de teste cobre as classificações Base64/Norito e os currículos JSON para `CapacityDisputeV1`, garantindo que os pacotes de evidências sejam determinados. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| O teste de repetição prova o determinismo litigioso/penalitário | A telemetria de prova de falha retornou duas vezes ao produzir instantâneos idênticos de razão, crédito e litígio, pois as barras são determinadas entre pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| O runbook documenta o fluxo de escalada e revogação | O guia de operações captura o fluxo de trabalho do conselho, as exigências de prevenção e os procedimentos de reversão. | `../dispute-revocation-runbook.md` |

### Reconciliação do tesouro| Verifique | Validação | Evidência |
|-------|------------|----------|
| A acumulação do livro-razão corresponde à projeção de imersão em 30 dias | O teste absorve cinco provedores em 30 janelas de liquidação, comparando as entradas do livro-razão com a referência de atendimento de pagamento. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| A reconciliação das exportações do livro-razão é registrada toda noite | `capacity_reconcile.py` compara as verificações do razão de taxas nas exportações que o XOR executa, emite as métricas Prometheus e realiza a aprovação do tesouro via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Os painéis de faturamento expõem as penalidades e a telemetria de acumulação | A importação Grafana rastreia a acumulação de GiB-hora, os compteurs de strikes e a garantia se envolvem para a visibilidade de plantão. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| O relatório público arquiva a metodologia de imersão e os comandos de repetição | O relatório detalhado da porta de imersão, os comandos de execução e os ganchos de observação para os auditores. | `./sf2c-capacity-soak.md` |

## Notas de execução

Reinicie o conjunto de validação antes de assinar:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Os operadores devem regenerar as cargas de demanda de integração/litígio com `sorafs_manifest_stub capacity {declaration,dispute}` e arquivar os bytes JSON/Norito resultantes nas costas do ticket de governo.

## Artefatos de aprovação

| Artefato | Caminho | blake2b-256 |
|----------|------|------------|
| Pacote de aprovação de integração de provedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprovação de resolução de litígios | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovação de reconciliação do tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde as cópias assinadas desses artefatos com o pacote de lançamento e guarde-as no registro de mudança de governo.

## Aprovações

- Líder da equipe de armazenamento - @storage-tl (24/03/2026)  
- Secretário do Conselho de Governança — @council-sec (2026/03/24)  
- Líder de Operações de Tesouraria — @treasury-ops (24/03/2026)