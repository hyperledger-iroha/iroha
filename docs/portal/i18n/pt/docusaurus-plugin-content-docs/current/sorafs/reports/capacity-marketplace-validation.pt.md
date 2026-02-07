---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validação do mercado de capacidade SoraFS
tags: [SF-2c, aceitação, lista de verificação]
resumo: Checklist de aceitação abrangendo onboarding de provedores, fluxos de disputa e reconciliação do tesouro que liberam a disponibilidade geral do mercado de capacidade SoraFS.
---

# Checklist de validação do mercado de capacidade SoraFS

**Janela de revisão:** 2026-03-18 -> 2026-03-24  
**Responsáveis do programa:** Equipe de Armazenamento (`@storage-wg`), Conselho de Governança (`@council`), Treasury Guild (`@treasury`)  
**Escopo:** Pipelines de onboarding de provedores, fluxos de adjudicação de disputas e processos de reconciliação do tesouro exigidos para o GA SF-2c.

O checklist abaixo deve ser revisado antes de habilitar o mercado para operadores externos. Cada linha conecta evidências determinísticas (testes, luminárias ou documentação) que os auditores podem reproduzir.

## Checklist de aceitação

### Onboarding de provedores

| Checagem | Validação | Evidência |
|-------|------------|----------|
| O registro aceita declarações canônicas de capacidade | O teste de integração exercita `/v1/sorafs/capacity/declare` via app API, verificando tratamento de assinaturas, captura de metadados e handoff para o registro do node. | `crates/iroha_torii/src/routing.rs:7654` |
| O contrato inteligente rejeita cargas divergentes | O teste unitário garante que IDs de provedor e campos GiB comprometem-se com a declaração assinada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI emite artistas canônicos de onboarding | O chicote de CLI escreve ditas Norito/JSON/Base64 determinísticas e valida round-trips para que os operadores possam preparar declarações offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| O guia do operador cobre o fluxo de trabalho de admissão e guarda-corpos de governança | A documentação enumera o esquema de declaração, padrões de política e passos de revisão para o conselho. | `../storage-capacity-marketplace.md` |

### Resolução de disputas

| Checagem | Validação | Evidência |
|-------|------------|----------|
| Registros de disputa persistem com resumo canônico do payload | O teste unitário registra uma disputa, decodifica a carga armazenada e afirma status pendente para garantir o determinismo do razão. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| O gerador de disputas do CLI corresponde ao esquema canônico | O teste do CLI cobre os ditos Base64/Norito e resumos JSON para `CapacityDisputeV1`, garantindo que os pacotes de evidências tenham hash determinístico. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| O teste de replay prova determinismo de disputa/penalidade | A telemetria de prova de falha reproduzida duas vezes gera instantâneos idênticos de razão, créditos e disputas para que barras sejam determinísticas entre pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| O runbook documenta o fluxo de escalada e revogação | O guia de operações captura o fluxo de trabalho do conselho, requisitos de evidência e procedimentos de reversão. | `../dispute-revocation-runbook.md` |

### Reconciliação do tesouro| Checagem | Validação | Evidência |
|-------|------------|----------|
| O accrual do ledger corresponde a projeção de absorção de 30 dias | O teste de imersão abrange cinco provedores em 30 janelas de liquidação, comparando entradas do razão com a referência de pagamento esperado. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| A reconciliação de exportações do razão e registrada a cada noite | `capacity_reconcile.py` compara expectativas do fee ledger com exportações XOR executadas, emite métricas Prometheus e faz gate da aprovação do tesouro via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Dashboards de exposição de faturamento deliberados e telemetria de acumulação | O import do Grafana plota o accrual GiB-hour, contadores de strikes e collateral bonded para visibilidade on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| O relatorio publicado arquiva a metodologia de imersão e comandos de replay | O relato detalha o escopo do mergulho, comandos de execução e ganchos de observabilidade para auditores. | `./sf2c-capacity-soak.md` |

## Notas de execução

Execute novamente um conjunto de validação antes da assinatura:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Os operadores devem regenerar payloads de solicitação de onboarding/disputa com `sorafs_manifest_stub capacity {declaration,dispute}` e arquivar os bytes JSON/Norito resultantes junto ao ticket de governança.

## Artefatos de aprovação

| Artefato | Caminho | blake2b-256 |
|----------|------|------------|
| Pacote de aprovação de onboarding de provedores | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprovação de resolução de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovação de reconciliação do tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde as cópias assinadas desses artistas com o pacote de lançamento e vincule-as no registro de mudanças de governança.

## Aprovações

- Líder da equipe de armazenamento - @storage-tl (24/03/2026)  
- Secretário do Conselho de Governança - @council-sec (2026-03-24)  
- Líder de Operações de Tesouraria - @treasury-ops (24/03/2026)