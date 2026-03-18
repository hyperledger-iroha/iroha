---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: status do endereço da conta
título: Соответствие адресов аккаунтов
description: Você está trabalhando no dispositivo ADDR-2 e no SDK de comando de sincronização.
---

O pacote canônico ADDR-2 (`fixtures/account/address_vectors.json`) possui fixtures I105 (preferencial), compactado (`sora`, segundo melhor; meia/largura total), multiassinatura e negativo. Quando o SDK + Torii é baseado em Odin e JSON, ele fornece o codec do produto para você. Esta página está fora do status atual do arquivo (`docs/source/account_address_status.md` no repositório principal), este site está disponível Você pode usar o fluxo de trabalho sem copiar o mono-repo.

## Перегенерация ou проверка pacета

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bandeiras:

- `--stdout` — envia JSON em stdout para testes ad-hoc.
- `--out <path>` — записывает в другой путь (por exemplo, при локальном сравнении изменений).
- `--verify` — cria uma cópia de segurança com uma solução segura (não é compatível com `--stdout`).

Fluxo de trabalho de CI **Desvio de vetor de endereço** `cargo xtask address-vectors --verify`
каждый раз, когда меняется fixture, генератор или docs, чтобы немедленно предупредить ревьюеров.

## O que você está usando?

| Superfície | Validação |
|--------|------------|
| Modelo de dados Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

O chicote de fios выполняет канонических байт + I105 + сжатых кодировок e проверяет, что коды ошибок в стиле Norito fornece suporte para luminárias negativas.

## Você não precisa de automação?

Ferramentas de liberação podem ser automatizadas para auxiliar no dispositivo de fixação
`scripts/account_fixture_helper.py`, este é o pacote ou o pacote canônico fornecido pela cópia/exibição:

```bash
# Download to a custom path (defaults to fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verify that a local copy matches the canonical source (HTTPS or file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet

# Emit Prometheus textfile metrics for dashboards/alerts
python3 scripts/account_fixture_helper.py check \
  --target path/to/sdk/address_vectors.json \
  --metrics-out /var/lib/node_exporter/textfile_collector/address_fixture.prom \
  --metrics-label android
```

O helper substitui o `--source` ou a configuração `IROHA_ACCOUNT_FIXTURE_URL` do CI, mas o SDK do CI pode ser usado предпочтительное зеркало. Para usar o auxiliar `--metrics-out`, use o `account_address_fixture_check_status{target="…"}` com o resumo SHA-256 canônico (`account_address_fixture_remote_info`), чтобы Coletores de arquivo de texto Prometheus e Grafana `account_address_fixture_status` podem fornecer sincronização de dados. Alerta de emergência, o alvo é `0`. Para multi-superfícies, o wrapper `ci/account_fixture_metrics.sh` é usado (por exemplo, `--target label=path[::source]`), comandos de plantão публиковали единый `.prom` arquivo para o coletor de arquivos de texto node-exporter.

## Нужен полный бриф?

Полный статус соответствия ADDR-2 (proprietários, план мониторинга, открытые задачи)
A localização em `docs/source/account_address_status.md` é um repositório baseado na Estrutura de Endereço RFC (`docs/account_structure.md`). Utilize esta página como operação operacional; para o repositório de documentos do repositório de documentos.