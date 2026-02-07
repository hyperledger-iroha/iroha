---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Validação de mercado de capacidade SoraFS
resumo: Lista de verificação de aceitação جو integração do provedor ، fluxos de trabalho de disputa اور reconciliação de tesouraria کو کور کرتی ہے اور Mercado de capacidade SoraFS کی GA کو portão کرتی ہے۔
tags: [SF-2c, aceitação, lista de verificação]
---

# Lista de verificação de validação do mercado de capacidade SoraFS

**Janela de revisão:** 18/03/2026 -> 24/03/2026  
**Proprietários do programa:** Equipe de armazenamento (`@storage-wg`), Conselho de governança (`@council`), Treasury Guild (`@treasury`)  
**Escopo:** Pipelines de integração de provedores, fluxos de adjudicação de disputas, e processos de reconciliação de tesouraria e SF-2c GA کے لئے درکار ہیں۔

نیچے دی گئی checklist کو بیرونی operadores کے لئے marketplace enable کرنے سے پہلے revisão کرنا لازم ہے۔ ہر evidência determinística de linha (testes, acessórios, documentação) سے لنک کرتی ہے جسے repetição dos auditores کر سکتے ہیں۔

## Lista de verificação de aceitação

### Integração do Provedor

| Verifique | Validação | Evidência |
|-------|------------|----------|
| Declarações de capacidade canônica do registro قبول کرتا ہے | API do aplicativo de teste de integração کے ذریعے `/v1/sorafs/capacity/declare` چلایا جاتا ہے, manipulação de assinaturas, captura de metadados, registro de nó, transferência e verificação کرتا ہے۔ | `crates/iroha_torii/src/routing.rs:7654` |
| Cargas úteis incompatíveis de contrato inteligente کو rejeitar کرتا ہے | Teste de unidade یقینی بناتا ہے کہ IDs de provedor اور campos GiB comprometidos declaração assinada کے مطابق ہوں قبل از persistência۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| Artefatos de integração canônicos CLI emitem کرتا ہے | Chicote CLI determinístico Saídas Norito/JSON/Base64 لکھتا ہے اور viagens de ida e volta validam کرتا ہے تاکہ declarações offline de operadores تیار کر سکیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Guia do operador fluxo de trabalho de admissão اور guarda-corpos de governança کو cobertura کرتا ہے | Esquema de declaração de documentação, padrões de política, etapas de revisão do conselho e enumerar کرتی ہے۔ | `../storage-capacity-marketplace.md` |

### Resolução de disputas

| Verifique | Validação | Evidência |
|-------|------------|----------|
| Disputa registra resumo de carga útil canônico کے ساتھ persist ہوتے ہیں | Registro de disputa de teste de unidade کرتا ہے، decodificação de carga útil armazenada کرتا ہے، اور afirmação de status pendente کرتا ہے تاکہ determinismo de razão یقینی ہو۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Esquema canônico do gerador de disputa CLI سے match کرتا ہے | Teste CLI `CapacityDisputeV1` کے لئے Base64/Norito saídas e resumos JSON cobrem کرتا ہے, جس سے pacotes de evidências hash determinístico ہوتے ہیں۔ | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Teste de repetição determinismo de disputa/penalidade کو ثابت کرتا ہے | Telemetria de prova de falha کو دو بار replay کرنے سے razão, crédito اور instantâneos de disputa یکساں بنتے ہیں تاکہ corta pares کے درمیان determinístico رہیں۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Escalação de runbook e fluxo de revogação کو documento کرتا ہے | Guia de operações do fluxo de trabalho do conselho, requisitos de evidência e procedimentos de reversão e captura de dados | `../dispute-revocation-runbook.md` |

### Reconciliação de Tesouraria| Verifique | Validação | Evidência |
|-------|------------|----------|
| Projeção de absorção de 30 dias de acumulação do razão سے correspondência کرتا ہے | Teste de absorção پانچ provedores کو 30 janelas de liquidação میں چلاتا ہے، entradas do razão کو referência de pagamento esperado کے ساتھ diff کرتا ہے۔ | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Reconciliação de exportação do razão ہر رات registro ہوتا ہے | Expectativas do razão de taxas `capacity_reconcile.py` کو exportações de transferência XOR executadas کے ساتھ comparar کرتا ہے, métricas Prometheus emitem کرتا ہے, اور Alertmanager کے ذریعے portão de aprovação do tesouro کرتا ہے۔ | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Penalidades nos painéis de faturamento e superfície de telemetria de acumulação کرتے ہیں | Grafana importar acúmulo de GiB-hora, contadores de greve, e gráfico de garantia vinculada کرتا ہے تاکہ visibilidade de plantão ملے۔ | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Relatório publicado metodologia de imersão e arquivo de comandos de repetição کرتا ہے | Relatório absorve escopo, comandos de execução e ganchos de observabilidade کو auditores کے لئے detalhes کرتا ہے۔ | `./sf2c-capacity-soak.md` |

## Notas de Execução

Assinatura do conjunto de validação دوبارہ چلائیں:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operadores کو `sorafs_manifest_stub capacity {declaration,dispute}` کے ساتھ cargas úteis de solicitação de integração/disputa دوبارہ gerar کرنے چاہئیں اور بنے ہوئے JSON/Norito bytes کو ticket de governança کے ساتھ arquivo کرنا چاہیے۔

## Artefatos de aprovação

| Artefato | Caminho | blake2b-256 |
|----------|------|------------|
| Pacote de aprovação de integração do provedor | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacote de aprovação para resolução de disputas | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacote de aprovação de reconciliação do Tesouro | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

ان artefatos کی cópias assinadas کو pacote de lançamento کے ساتھ محفوظ کریں اور registro de mudança de governança میں لنک کریں۔

## Aprovações

- Líder da equipe de armazenamento - @storage-tl (24/03/2026)  
- Secretário do Conselho de Governança — @council-sec (2026/03/24)  
- Líder de Operações de Tesouraria — @treasury-ops (24/03/2026)