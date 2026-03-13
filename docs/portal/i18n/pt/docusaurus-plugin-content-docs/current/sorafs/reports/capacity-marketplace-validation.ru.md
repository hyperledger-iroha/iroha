---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/reports/capacity-marketplace-validation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Валидация маркетплейса емкости SoraFS
tags: [SF-2c, aceitação, lista de verificação]
resumo: Чеклист приемки, покрывающий онбординг fornecedores, потоки споров и сверку казначейства, которые закрывают готовность маркетплейса емкости SoraFS к GA.
---

# Verifique a validação do mercado de compras SoraFS

**Provérbios:** 18/03/2026 -> 24/03/2026  
**Programas adicionais:** Equipe de armazenamento (`@storage-wg`), Conselho de governança (`@council`), Guilda do Tesouro (`@treasury`)  
**Exibição:** Provedores de serviços on-line de pacotes, parceiros de negócios e serviços de promoção de negócios, não para GA SF-2c.

Verifique se a nova opção foi testada para o mercado de trabalho para seus operadores. Каждая строка ссылается на детерминированные доказательства (testes, acessórios ou documentação), которые аудиторы могут воспроизвести.

## Verifique as informações

### Provedores de negociação

| Prova | Validação | Documentação |
|-------|------------|----------|
| Registro принимает канонические декларации емкости | O teste de integração usa `/v2/sorafs/capacity/declare` para configurar a API do aplicativo, testar a configuração do arquivo, salvar metadados e transferir para o registro nós. | `crates/iroha_torii/src/routing.rs:7654` |
| Contrato inteligente отклоняет несовпадающие cargas úteis | A unidade de teste é garantida, quais provedores de IDs e cada GiB comprometido são fornecidos com uma declaração de declaração de segurança. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3445` |
| CLI выпускает канонические artefatos onбординга | CLI chicote пишет детерминированные Norito/JSON/Base64 usa e valida viagens de ida e volta, operadores de operação podem ser usados декларации offline. | `crates/sorafs_car/tests/capacity_cli.rs:17` |
| Operador de operação de gerenciamento de fluxo de trabalho, estruturas e guarda-corpos | Документация перечисляет схему декларации, política de padrões e шаги ревью para o conselho. | `../storage-capacity-marketplace.md` |

### Разрешение споров

| Prova | Validação | Documentar |
|-------|------------|----------|
| Записи споров сохраняются каноническим digest payload | O recurso de registro de unidade de unidade, decompõe a carga útil e fornece o status pendente para a determinação da garantia do razão. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:1835` |
| Gerador de conteúdo CLI suporta esquema canônico | O teste CLI usa Base64/Norito e arquivos JSON para `CapacityDisputeV1`, garantindo a determinação de pacotes de evidências de hash. | `crates/sorafs_car/tests/capacity_cli.rs:455` |
| Teste de repetição para determinar a determinação do jogo/repetição | Prova de falha de telemetria, воспроизведенная дважды, дает идентичные instantâneos razão, crédito e disputa, чтобы barras были детерминированны между pares. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3430` |
| Documentação do Runbook com escalas e revogações | Операционное руководство фиксирует conselho de fluxo de trabalho, требования к доказательствам e процедуры rollback. | `../dispute-revocation-runbook.md` |

### Сверка казначейства| Prova | Validação | Documentação |
|-------|------------|----------|
| Livro razão de acumulação совпадает с 30-дневной проекцией imersão | Mergulhe o teste de provedores de serviços para liquidação de 30 minutos, registrando o registro com referência de referência. | `crates/iroha_core/src/smartcontracts/isi/sorafs.rs:3000` |
| Livro-razão de transporte de mercadorias | `capacity_reconcile.py` armazena o razão de taxas de exportação com exportação XOR, métricas públicas Prometheus e gate-it одобрение казначейства через Alertmanager. | `scripts/telemetry/capacity_reconcile.py:1`,`docs/source/sorafs/runbooks/capacity_reconciliation.md:1`,`dashboards/alerts/sorafs_capacity_rules.yml:100` |
| Painéis de faturamento показывают пенализации e acumulação de telemetria | Importe Grafana acumulação de GiB-hora, greves pecuniárias e garantias vinculadas para vídeos de plantão. | `dashboards/grafana/sorafs_capacity_penalties.json:1` |
| Опубликованный отчет архивирует методологию imersão e comandos replay | Selecione a opção de imersão, comandos de segurança e ganchos de observabilidade para auditores. | `./sf2c-capacity-soak.md` |

## Recomendações para você

Faça o teste antes da assinatura:

```bash
cargo test -p iroha_torii --features app_api -- capacity_declaration_handler_accepts_request
cargo test -p iroha_core -- register_capacity_declaration_rejects_provider_mismatch
cargo test -p iroha_core -- register_capacity_dispute_inserts_record
cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic
cargo test -p iroha_core -- capacity_fee_ledger_30_day_soak_deterministic
cargo test -p sorafs_car --features cli --test capacity_cli
python3 scripts/telemetry/capacity_reconcile.py --snapshot <state.json> --ledger <ledger.ndjson> --warn-only
```

Operadores de dados podem gerar cargas úteis para gerar on-line/gerar cargas úteis `sorafs_manifest_stub capacity {declaration,dispute}` e arranjar Os bytes JSON/Norito são usados no ticket de governança.

## Assinatura de artefactos

| Artefato | Caminho | blake2b-256 |
|----------|------|------------|
| fornecedores de serviços de distribuição de pacotes | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_onboarding_signoff.md` | `8f41a745d8d94710fe81c07839651520429d4abea5729bc00f8f45bbb11daa4c` |
| Pacotes de distribuição de esporos | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_dispute_signoff.md` | `c3ac3999ef52857170fedb83cddbff7733ef5699f8b38aea2e65ae507a6229f7` |
| Pacotes de entrega de pacotes | `docs/examples/sorafs_capacity_marketplace_validation/2026-03-24_treasury_signoff.md` | `0511aeed1f5607c329428cd49c94d1af51292c85134c10c3330c172b0140e8c6` |

Faça uma cópia do arquivo do artefacto no pacote de lançamento e use os arquivos na configuração da configuração.

## Подписи

- Líder da equipe de armazenamento - @storage-tl (24/03/2026)  
- Secretário do Conselho de Governança — @council-sec (2026-03-24)  
- Líder de Operações de Tesouraria — @treasury-ops (24/03/2026)