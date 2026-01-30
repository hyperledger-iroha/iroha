---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/norito/examples/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 609fd0ec5a123cb0d322a2f0fb490a5dbac93107845b096aa7e12026fcb17474
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Exemplos de Norito
description: Trechos Kotodama selecionados com roteiros do livro razao.
slug: /norito/examples
---

Estes exemplos espelham os quickstarts do SDK e os roteiros do livro razao. Cada trecho inclui uma lista de verificacao do livro razao e aponta para os guias de Rust, Python e JavaScript para que voce possa repetir o mesmo cenario do inicio ao fim.

- **[Esqueleto do entrypoint Hajimari](./hajimari-entrypoint)** - Estrutura minima de contrato Kotodama com um unico entrypoint publico e um handle de estado.
- **[Registrar dominio e cunhar ativos](./register-and-mint)** - Mostra a criacao de dominios com permissao, o registro de ativos e a cunhagem deterministica.
- **[Invocar transferencia do host a partir de Kotodama](./call-transfer-asset)** - Mostra como um entrypoint Kotodama pode chamar a instrucao do host `transfer_asset` com validacao inline de metadados.
- **[Transferir ativo entre contas](./transfer-asset)** - Fluxo direto de transferencia de ativos que espelha os quickstarts do SDK e os roteiros do livro razao.
- **[Cunhar, transferir e queimar um NFT](./nft-flow)** - Percorre o ciclo de vida de um NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.
