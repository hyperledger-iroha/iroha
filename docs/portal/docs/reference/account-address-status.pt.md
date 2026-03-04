---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f18b4a15e42363483c4f65945aba2cc208f9a2e59b6f31f143e5dc792d8d9071
source_last_modified: "2025-11-27T10:35:59.095888+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: account-address-status
title: Conformidade de enderecos de conta
description: Resumo do fluxo do fixture ADDR-2 e como as equipes de SDK ficam sincronizadas.
---

O pacote canonico ADDR-2 (`fixtures/account/address_vectors.json`) captura fixtures IH58 (preferred), compressed (`sora`, second-best; half/full width), multisignature e negative. Cada superficie de SDK + Torii usa o mesmo JSON para detectar qualquer drift de codec antes de chegar a producao. Esta pagina espelha o brief interno de status (`docs/source/account_address_status.md` no repositorio raiz) para que leitores do portal consultem o fluxo sem vasculhar o mono-repo.

## Regenerar ou verificar o pacote

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` - emite o JSON em stdout para inspecao ad-hoc.
- `--out <path>` - grava em um caminho diferente (ex.: ao comparar mudancas localmente).
- `--verify` - compara a copia de trabalho com o conteudo recem-gerado (nao pode ser combinado com `--stdout`).

O workflow de CI **Address Vector Drift** roda `cargo xtask address-vectors --verify`
sempre que o fixture, o gerador ou os docs mudam para alertar reviewers imediatamente.

## Quem consome o fixture?

| Surface | Validation |
|---------|------------|
| Rust data-model | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| JavaScript SDK | `javascript/iroha_js/test/address.test.js` |
| Swift SDK | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| Android SDK | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada harness faz round-trip de bytes canonicos + IH58 + encodings comprimidos e verifica se os codigos de erro no estilo Norito batem com o fixture para casos negativos.

## Precisa de automacao?

Ferramentas de release podem automatizar refreshes de fixture com o helper
`scripts/account_fixture_helper.py`, que busca ou verifica o pacote canonico sem copy/paste:

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

O helper aceita overrides de `--source` ou a variavel de ambiente `IROHA_ACCOUNT_FIXTURE_URL` para que jobs de CI de SDK apontem para seu mirror preferido. Quando `--metrics-out` e fornecido, o helper escreve `account_address_fixture_check_status{target=\"...\"}` junto com o digest SHA-256 canonico (`account_address_fixture_remote_info`) para que os textfile collectors do Prometheus e o dashboard Grafana `account_address_fixture_status` provem que cada superficie permanece em sincronia. Gere alerta quando um target reportar `0`. Para automacao multi-superficie use o wrapper `ci/account_fixture_metrics.sh` (aceita `--target label=path[::source]` repetidos) para que equipes on-call publiquem um unico arquivo `.prom` consolidado para o textfile collector do node-exporter.

## Precisa do brief completo?

O status completo de conformidade ADDR-2 (owners, plano de monitoramento, itens de acao em aberto)
fica em `docs/source/account_address_status.md` dentro do repositorio junto com o Address Structure RFC (`docs/account_structure.md`). Use esta pagina como lembrete operacional rapido; para orientacao detalhada, consulte os docs do repositorio.
