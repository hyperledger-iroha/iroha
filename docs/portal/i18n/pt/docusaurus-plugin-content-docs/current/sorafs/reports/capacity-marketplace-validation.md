---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Checklist de validacao do mercado de capacidade SoraFS

**Janela de revisao:** 2026-03-18 -> 2026-03-24  
**Responsaveis do programa:** Storage Team (`@storage-wg`), Governance Council (`@council`), Treasury Guild (`@treasury`)  
**Escopo:** Pipelines de onboarding de providers, fluxos de adjudicacao de disputas e processos de reconciliacao do tesouro requeridos para a GA SF-2c.

O checklist abaixo deve ser revisado antes de habilitar o mercado para operadores externos. Cada linha conecta evidencias deterministicas (tests, fixtures ou documentacao) que auditores podem reproduzir.

## Checklist de aceitacao

### Onboarding de providers

| Checagem | Validacao | Evidencia |
|-------|------------|----------|
| O registry aceita declaracoes canonicas de capacidade | O teste de integracao exercita `/v1/sorafs/capacity/declare` via app API, verificando tratamento de assinaturas, captura de metadata e handoff para o registry do node. | `crates/iroha_torii/src/routing.rs:7654` |
| O smart contract rejeita payloads divergentes | O teste unitario garante que IDs de provider e campos GiB comprometidos correspondem a declaracao assinada antes de persistir. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| O CLI emite artefatos canonicos de onboarding | O harness de CLI escreve saidas Norito/JSON/Base64 deterministicas e valida round-trips para que operadores possam preparar declaracoes offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| O guia do operador cobre o workflow de admissao e guardrails de governanca | A documentacao enumera o schema de declaracao, defaults de policy e passos de revisao para o council. | `../storage-capacity-marketplace.md` |

### Resolucao de disputas

| Checagem | Validacao | Evidencia |
|-------|------------|----------|
| Registros de disputa persistem com digest canonico do payload | O teste unitario registra uma disputa, decodifica o payload armazenado e afirma status pending para garantir determinismo do ledger. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| O gerador de disputas do CLI corresponde ao schema canonico | O teste do CLI cobre saidas Base64/Norito e resumos JSON para `CapacityDisputeV1`, garantindo que evidence bundles tenham hash deterministico. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| O teste de replay prova determinismo de disputa/penalidade | A telemetry de proof-failure reproduzida duas vezes gera snapshots identicos de ledger, creditos e disputa para que slashes sejam deterministicos entre peers. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| O runbook documenta o fluxo de escalonamento e revogacao | O guia de operacoes captura o workflow do council, requisitos de evidencia e procedimentos de rollback. | `../dispute-revocation-runbook.md` |

### Reconciliacao do tesouro

| Checagem | Validacao | Evidencia |
|-------|------------|----------|
| O accrual do ledger corresponde a projecao de soak de 30 dias | O teste de soak abrange cinco providers em 30 janelas de settlement, comparando entradas do ledger com a referencia de payout esperada. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| A reconciliacao de exportes do ledger e registrada a cada noite | `capacity_reconcile.py` compara expectativas do fee ledger com exportes XOR executados, emite metricas Prometheus e faz gate da aprovacao do tesouro via Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Dashboards de billing expoem penalidades e telemetry de accrual | O import do Grafana plota o accrual GiB-hour, contadores de strikes e collateral bonded para visibilidade on-call. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| O relatorio publicado arquiva a metodologia de soak e comandos de replay | O relatorio detalha o escopo do soak, comandos de execucao e hooks de observabilidade para auditores. | `./sf2c-capacity-soak.md` |

## Notas de execucao

Reexecute a suite de validacao antes do sign-off:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Os operadores devem regenerar payloads de solicitacao de onboarding/disputa com `sorafs_manifest_stub capacity {declaration,dispute}` e arquivar os bytes JSON/Norito resultantes junto ao ticket de governanca.

## Artefatos de aprovacao

| Artefato | Caminho | blake2b-256 |
|----------|------|-------------|
| Pacote de aprovacao de onboarding de providers | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprovacao de resolucao de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovacao de reconciliacao do tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Guarde as copias assinadas desses artefatos com o bundle de release e vincule-as no registro de mudancas de governanca.

## Aprovacoes

- Storage Team Lead - @storage-tl (2026-03-24)  
- Governance Council Secretary - @council-sec (2026-03-24)  
- Treasury Operations Lead - @treasury-ops (2026-03-24)
