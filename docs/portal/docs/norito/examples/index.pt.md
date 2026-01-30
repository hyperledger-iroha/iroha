---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71585d0aa92f516519d69df408cf4255b90f5bce89ce8022f6c7b60871dc057b
source_last_modified: "2025-11-11T09:31:19.097792+00:00"
translation_last_reviewed: 2026-01-30
---

---
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
