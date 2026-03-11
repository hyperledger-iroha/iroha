---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: status do endereço da conta
título: Conformidade dos endereços de conta
descrição: Resumo do fluxo de trabalho do dispositivo ADDR-2 e da sincronização de equipes SDK.
---

O pacote canônico ADDR-2 (`fixtures/account/address_vectors.json`) captura os fixtures I105 (preferencial), compactado (`sora`, segundo melhor; meia/largura total), multiassinatura e negativos. Cada superfície SDK + Torii é acionado no meme JSON para detectar todos os codecs derivados antes da produção. Esta página reflete o resumo do status interno (`docs/source/account_address_status.md` no depósito racine) para que os leitores do portal possam consultar o fluxo de trabalho sem preencher o mono-repo.

## Regenera ou verifica o pacote

```bash
# Refresh the canonical fixture (writes fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Fail fast if the committed file is stale
cargo xtask address-vectors --verify
```

Bandeiras:

- `--stdout` - emite o JSON na versão padrão para inspeção ad-hoc.
- `--out <path>` - escrito para outro caminho (por exemplo, para comparação de localidade).
- `--verify` - compare a cópia de trabalho com um conteúdo fraichement genérico (não pode ser combinado com `--stdout`).

O fluxo de trabalho CI **Address Vector Drift** executa `cargo xtask address-vectors --verify`
Cada vez que o aparelho, o gerador ou os documentos são alterados para alertar imediatamente os revisores.

## Qui consomme le fixture ?

| Superfície | Validação |
|--------|------------|
| Modelo de dados Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (servidor) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK rápido | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada chicote faz um retorno aos octetos canônicos + I105 + codifica compressas e verifica se os códigos de erro de estilo Norito correspondem ao dispositivo para os casos negativos.

## Besoin d'automatisation ?

As ferramentas de liberação podem automatizar os rafraichissements de fixtures com o helper
`scripts/account_fixture_helper.py`, que recupera ou verifica o pacote canônico sem copiadora/coletor:

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

O ajudante aceita as substituições `--source` ou a variável de ambiente `IROHA_ACCOUNT_FIXTURE_URL` para que os trabalhos CI do SDK apontem para seu espelho preferido. Quando `--metrics-out` é fornecido, o ajudante escreve `account_address_fixture_check_status{target="..."}` e o resumo SHA-256 canônico (`account_address_fixture_remote_info`) para os coletores de arquivos de texto Prometheus e o painel Grafana `account_address_fixture_status` pode provar que cada superfície permanece sincronizada. Alerte um relatório confiável `0`. Para a automação multi-superfície, use o wrapper `ci/account_fixture_metrics.sh` (aceite as repetições `--target label=path[::source]`) para que as equipes de referência publiquem seu próprio arquivo `.prom` consolidado para o coletor de arquivo de texto do exportador de nó.

## Besoin du brief complet ?

O status completo de conformidade ADDR-2 (proprietários, plano de monitoramento, ações abertas)
você encontra em `docs/source/account_address_status.md` em seu depósito, assim como a estrutura de endereço RFC (`docs/account_structure.md`). Utilize esta página como operação de rappel rápida; consulte os documentos do repositório para um guia aprovado.