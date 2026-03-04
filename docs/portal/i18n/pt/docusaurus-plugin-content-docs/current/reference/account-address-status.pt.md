---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: status do endereço da conta
título: Conformidade de endereços de conta
descrição: Resumo do fluxo do fixture ADDR-2 e como as equipes do SDK ficam sincronizadas.
---

O pacote canônico ADDR-2 (`fixtures/account/address_vectors.json`) captura fixtures IH58 (preferencial), compactado (`sora`, segundo melhor; meia/largura total), multiassinatura e negativo. Cada superfície do SDK + Torii usa o mesmo JSON para detectar qualquer desvio do codec antes de chegar à produção. Esta página reflete o breve interno de status (`docs/source/account_address_status.md` no repositório raiz) para que os leitores do portal consultem o fluxo sem vasculhar o mono-repo.

## Regenerar ou verificar o pacote

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bandeiras:

- `--stdout` - emite o JSON em stdout para inspeção ad-hoc.
- `--out <path>` - grave em um caminho diferente (ex.: ao comparar mudancas localmente).
- `--verify` - compare a cópia de trabalho com o conteúdo recém-gerado (não pode ser combinado com `--stdout`).

O fluxo de trabalho de CI **Address Vector Drift** roda `cargo xtask address-vectors --verify`
sempre que o aparelho, o gerador ou os documentos mudam para alertar revisores imediatamente.

## Quem consome o aparelho?

| Superfície | Validação |
|--------|------------|
| Modelo de dados Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada chicote faz round-trip de bytes canonicos + IH58 + encodings comprimidos e verifica se os códigos de erro no estilo Norito batem com o fixture para casos negativos.

## Precisa de automação?

Ferramentas de liberação podem automatizar atualizações de fixture com ou helper
`scripts/account_fixture_helper.py`, que busca ou verifica o pacote canônico sem copiar/colar:

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

O helper aceita overrides de `--source` ou uma variável de ambiente `IROHA_ACCOUNT_FIXTURE_URL` para que jobs de CI de SDK apontem para seu espelho preferido. Quando `--metrics-out` e fornecido, o auxiliar escreve `account_address_fixture_check_status{target=\"...\"}` junto com o digest SHA-256 canonico (`account_address_fixture_remote_info`) para que os coletores de arquivos de texto do Prometheus e o painel Grafana `account_address_fixture_status` provem que cada superfície permanece em sincronia. Gere alerta quando um alvo reportar `0`. Para automação multi-superfície use o wrapper `ci/account_fixture_metrics.sh` (aceita `--target label=path[::source]` repetições) para que equipes de plantão publiquem um arquivo único `.prom` consolidado para o coletor de arquivo de texto do node-exporter.

## Precisa do briefing completo?

O status completo de conformidade ADDR-2 (proprietários, plano de monitoramento, itens de ação em aberto)
fica em `docs/source/account_address_status.md` dentro do repositório junto com o Address Structure RFC (`docs/account_structure.md`). Use esta página como lembrete operacional rápido; para orientação detalhada, consulte os documentos do repositório.