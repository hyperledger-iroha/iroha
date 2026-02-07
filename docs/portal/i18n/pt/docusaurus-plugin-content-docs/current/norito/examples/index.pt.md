---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Exemplos de Norito
description: Trechos Kotodama selecionados com roteiros do livro razão.
slug: /norito/exemplos
---

Estes exemplos refletem os quickstarts do SDK e os roteiros do livro razão. Cada trecho inclui uma lista de verificação do livro razão e aponta para os guias de Rust, Python e JavaScript para que você possa repetir o mesmo cenário do início ao fim.

- **[Esqueleto do ponto de entrada Hajimari](./hajimari-entrypoint)** - Estrutura mínima de contrato Kotodama com um único ponto de entrada público e um identificador de estado.
- **[Registrar domínio e cunhar ativos](./register-and-mint)** - Mostra a criação de domínios com permissão, o registro de ativos e a cunhagem determinística.
- **[Invocar transferência do host a partir de Kotodama](./call-transfer-asset)** - Mostra como um ponto de entrada Kotodama pode chamar uma instrução do host `transfer_asset` com validação inline de metadados.
- **[Transferir ativo entre contas](./transfer-asset)** - Fluxo direto de transferência de ativos que reflete os quickstarts do SDK e os roteiros do livro razão.
- **[Cunhar, transferir e queimar um NFT](./nft-flow)** - Percorrer o ciclo de vida de um NFT do início ao fim: cunhagem para o dono, transferência, marcação de metadados e queima.