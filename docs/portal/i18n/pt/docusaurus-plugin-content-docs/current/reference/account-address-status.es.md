---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: status do endereço da conta
título: Cumplimiento de direcciones de cuenta
description: Resumo do fluxo de trabalho do fixture ADDR-2 e como os equipamentos do SDK são mantidos sincronizados.
---

O pacote canônico ADDR-2 (`fixtures/account/address_vectors.json`) captura fixtures I105 (preferencial), compactado (`sora`, segundo melhor; meia/largura total), multiassinatura e negativo. Cada superfície do SDK + Torii é usada no mesmo JSON para detectar qualquer derivação do codec antes de iniciar a produção. Esta página reflete o resumo do estado interno (`docs/source/account_address_status.md` no repositório raiz) para que os leitores do portal consultem o fluxo sem buscar no mono-repo.

## Regenerar ou verificar o pacote

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bandeiras:

- `--stdout` - emite o JSON como stdout para inspeção ad-hoc.
- `--out <path>` - escribe en una ruta diferente (p. ej., al comparar cambios localmente).
- `--verify` - compara a cópia de trabalho com o conteúdo recebido (não pode ser combinado com `--stdout`).

O fluxo de trabalho de CI **Address Vector Drift** é executado `cargo xtask address-vectors --verify`
cada vez que muda o dispositivo, o gerador ou os documentos para alertar os revisores imediatamente.

## Quem consome o aparelho?

| Superfície | Validação |
|--------|------------|
| Modelo de dados Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada chicote faz round-trip de bytes canônicos + I105 + codificações comprimidas e verifica que os códigos de erro estilo Norito coincidem com o fixture para os casos negativos.

## Necessita automatização?

Las herramientas de release podem automatizar refrescos de fixtures com o helper
`scripts/account_fixture_helper.py`, que obtém ou verifica o pacote canônico sem etapas de copiar/pegar:

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

O auxiliar aceita substituições de `--source` ou a variável de ambiente `IROHA_ACCOUNT_FIXTURE_URL` para que os trabalhos de CI de SDK sejam apontados para seu espelho preferido. Quando é fornecido `--metrics-out`, o auxiliar descreve `account_address_fixture_check_status{target="..."}` junto com o resumo SHA-256 canônico (`account_address_fixture_remote_info`) para os coletores de arquivos de texto de Prometheus e o painel de Grafana `account_address_fixture_status` pode provar que cada superfície segue em sincronia. Alerta quando um relatório de destino `0`. Para automatização multi-superfície usa o wrapper `ci/account_fixture_metrics.sh` (aceita repetições `--target label=path[::source]`) para que os equipamentos de plantão publiquem um único arquivo `.prom` consolidado para o coletor de arquivos de texto do nó-exportador.

## Precisa do briefing completo?

O estado completo de cumprimento ADDR-2 (proprietários, plano de monitoramento, ações abertas) vive em `docs/source/account_address_status.md` dentro do repositório junto com a estrutura de endereço RFC (`docs/account_structure.md`). Usa esta página como recordatório operativo rápido; para guias em profundidade, consulte os documentos do repositório.