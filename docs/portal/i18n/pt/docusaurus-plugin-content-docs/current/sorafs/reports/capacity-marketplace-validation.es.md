---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validação do mercado de capacidade SoraFS
tags: [SF-2c, aceitação, lista de verificação]
resumo: Checklist de aceitação que cobre onboarding de provedores, fluxos de disputa e conciliação de tesouraria que habilitam a disponibilidade geral do mercado de capacidade SoraFS.
---

# Lista de verificação de validação do mercado de capacidade SoraFS

**Venda de revisão:** 18/03/2026 -> 24/03/2026  
**Responsáveis pelo programa:** Equipe de armazenamento (`@storage-wg`), Conselho de governança (`@council`), Treasury Guild (`@treasury`)  
**Alcance:** Pipelines de integração de provedores, fluxos de julgamento de disputas e processos de conciliação de tesouraria necessários para o GA SF-2c.

A lista de verificação a seguir deve ser revisada antes de ativar o mercado para operadores externos. Cada fila enlaza evidência determinística (testes, acessórios ou documentação) que os auditores podem reproduzir.

## Checklist de aceitação

### Onboarding de provedores

| Cheque | Validação | Evidência |
|-------|------------|----------|
| O registro aceita declarações canônicas de capacidade | O teste de integração executa `/v2/sorafs/capacity/declare` por meio da API do aplicativo, verificando o gerenciamento de firmas, a captura de metadados e a transferência para o registro do nó. | `crates/iroha_torii/src/routing.rs:7654` |
| O contrato inteligente rechaza cargas úteis dessalineadas | O unitário de teste garante que os IDs de provedor e os campos de GiB comprometidos coincidam com a declaração firmada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| El CLI emite artefatos canônicos de integração | O chicote de CLI descreve saídas Norito/JSON/Base64 determinísticas e valida viagens de ida e volta para que os operadores possam preparar declarações offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| O guia dos operadores captura o fluxo de admissão e as grades de proteção | A documentação enumera o esquema de declaração, os padrões de política e os passos de revisão para o conselho. | `../storage-capacity-marketplace.md` |

### Resolução de disputas

| Cheque | Validação | Evidência |
|-------|------------|----------|
| Os registros de disputa persistem com o resumo canônico da carga útil | O teste unitário registra uma disputa, decodifica a carga útil armazenada e afirma o estado pendente para garantir o determinismo do razão. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| O gerador de disputas do CLI coincide com o esquema canônico | O teste da CLI cria saídas Base64/Norito e resume JSON para `CapacityDisputeV1`, garantindo que os pacotes de evidências hashean de forma determinística. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| O teste de repetição testa o determinismo de disputa/penalização | A telemetria de prova-falha reproduzida duas vezes produz instantâneos idênticos de razão, créditos e disputas para que as barras sejam deterministas entre pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| O runbook documenta o fluxo de escalada e revogação | O guia de operações captura o fluxo do conselho, requisitos de evidência e procedimentos de reversão. | `../dispute-revocation-runbook.md` |### Conciliação de Tesoreria

| Cheque | Validação | Evidência |
|-------|------------|----------|
| A acumulação do livro-razão coincide com a projeção de absorção de 30 dias | O teste de imersão abarca cinco provedores em 30 janelas de liquidação, comparando entradas do razão com a referência de pagamento esperada. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| A conciliação de exportações do livro-razão é registrada a cada noite | `capacity_reconcile.py` compara as expectativas do razão de taxas com exportações XOR executadas, emite métricas Prometheus e permite a aprovação de tesouraria via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Os painéis de faturamento expõem penalizações e telemetria de acumulação | A importação de Grafana acumulação gráfica GiB-hora, contadores de greves e garantias vinculadas para visibilidade de plantão. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| O relatório publicado arquiva a metodologia de imersão e comandos de repetição | O relatório detalha o alcance do mergulho, comandos de execução e ganchos de observação para auditores. | `./sf2c-capacity-soak.md` |

## Notas de execução

Reejecuta o conjunto de validação antes da assinatura:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Os operadores devem regenerar as cargas úteis de solicitação de integração/disputa com `sorafs_manifest_stub capacity {declaration,dispute}` e arquivar os bytes JSON/Norito resultantes junto com o ticket de governança.

## Artefatos de aprovação

| Artefato | Rota | blake2b-256 |
|----------|------|------------|
| Pacote de aprovação de integração de provedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprovação de resolução de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovação de conciliação de tesouraria | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde as cópias firmadas desses artefatos no pacote de lançamento e enlazalas no registro de mudanças de governo.

## Aprovações

- Líder do equipamento de armazenamento — @storage-tl (2026-03-24)  
- Secretaria del Governance Council — @council-sec (2026-03-24)  
- Líder de Operações de Tesoreria — @treasury-ops (24/03/2026)