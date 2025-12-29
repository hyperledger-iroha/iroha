<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
lang: pt
direction: ltr
source: docs/portal/docs/reference/account-address-status.md
status: complete
translator: manual
source_hash: 448093b8efd6e92c8265691d6a4dafcf1d3b9c369cc6ae42ee8ae418b27bc36b
source_last_modified: "2025-11-08T16:40:11.557769+00:00"
translation_last_reviewed: 2025-11-14
---

---
id: account-address-status
title: Conformidade de endereços de conta
description: Resumo do fluxo ADDR-2 de fixtures e de como as equipes de SDK se mantêm alinhadas.
---

O bundle canônico ADDR‑2 (`fixtures/account/address_vectors.json`) captura fixtures IH58,
endereços comprimidos (full/half‑width), multisig e casos negativos. Todos os SDKs e
superfícies Torii consomem o mesmo JSON para que possamos detectar qualquer drift de codec
antes de chegar em produção. Esta página reflete o status brief interno
(`docs/source/account_address_status.md` no repositório raiz), de forma que leitores do
portal possam consultar o fluxo de trabalho sem percorrer o mono‑repo.

## Regenerar ou verificar o bundle

```bash
# Atualiza o fixture canônico (escreve fixtures/account/address_vectors.json)
cargo xtask address-vectors --out fixtures/account/address_vectors.json

# Falha rapidamente se o arquivo comitado estiver desatualizado
cargo xtask address-vectors --verify
```

Flags:

- `--stdout` — emite o JSON em stdout para inspeção ad‑hoc.
- `--out <path>` — grava em outro caminho (ex.: ao comparar mudanças localmente).
- `--verify` — compara a cópia de trabalho com o conteúdo recém‑gerado (não pode ser
  combinado com `--stdout`).

O workflow de CI **Address Vector Drift** executa `cargo xtask address-vectors --verify`
sempre que o fixture, o gerador ou os docs mudam, para alertar revisores imediatamente.

## Quem consome o fixture?

| Superfície | Validação |
|-----------|-----------|
| Data‑model Rust | `crates/iroha_data_model/tests/account_address_vectors.rs` |
| Torii (server) | `crates/iroha_torii/tests/account_address_vectors.rs` |
| SDK JavaScript | `javascript/iroha_js/test/address.test.js` |
| SDK Swift | `IrohaSwift/Tests/IrohaSwiftTests/AccountAddressTests.swift` |
| SDK Android | `java/iroha_android/src/test/java/org/hyperledger/iroha/android/address/AccountAddressTests.java` |

Cada harness faz round‑trip dos bytes canônicos + IH58 + codificações comprimidas e
verifica se os códigos de erro em estilo Norito batem com o fixture nos casos negativos.

## Precisa de automação?

Ferramentas de release podem scriptar o refresh de fixtures usando o helper
`scripts/account_fixture_helper.py`, que busca ou verifica o bundle canônico sem
copy/paste:

```bash
# Faz download para um caminho customizado (padrão fixtures/account/address_vectors.json)
python3 scripts/account_fixture_helper.py fetch --output path/to/sdk/address_vectors.json

# Verifica se uma cópia local bate com a fonte canônica (HTTPS ou file://)
python3 scripts/account_fixture_helper.py check --target path/to/sdk/address_vectors.json --quiet
```

O helper aceita overrides via `--source` ou pela variável de ambiente
`IROHA_ACCOUNT_FIXTURE_URL`, de forma que jobs de CI dos SDKs possam apontar para o mirror
de sua preferência.

## Precisa do resumo completo?

O status completo de conformidade ADDR‑2 (owners, plano de monitoramento, ações em aberto)
está em `docs/source/account_address_status.md` dentro do repositório, junto com o RFC de
Estrutura de Conta (`docs/account_structure.md`). Use esta página como lembrete operacional
rápido; para detalhes aprofundados, consulte os docs no repo.
