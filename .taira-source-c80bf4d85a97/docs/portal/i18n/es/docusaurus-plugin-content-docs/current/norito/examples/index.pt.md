---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Ejemplos de Norito
descripción: Trechos Kotodama seleccionados con roteiros do livro razao.
babosa: /norito/ejemplos
---

Estos ejemplos incluyen los inicios rápidos del SDK y los controladores del libro razao. Cada trecho incluye una lista de verificación del libro razao y utiliza las guías de Rust, Python y JavaScript para que puedas repetir el mismo escenario del inicio de la semana.

- **[Esqueleto do Entrypoint Hajimari](./hajimari-entrypoint)** - Estructura mínima de contrato Kotodama con un único punto de entrada público y un identificador de estado.
- **[Registrar dominio e cunhar ativos](./register-and-mint)** - Mostra a criacao de dominios com permissao, o registro de ativos e a cunhagem deterministica.
- **[Invocar transferencia do host a partir de Kotodama](./call-transfer-asset)** - Mostra como un punto de entrada Kotodama puede chamar a instrucciones del host `transfer_asset` con validación en línea de metadados.
- **[Transferir ativo entre contas](./transfer-asset)** - Flujo directo de transferencia de activos que espelha los inicios rápidos del SDK y los roteiros del libro razao.
- **[Cunhar, transferir e queimar um NFT](./nft-flow)** - Percorre o ciclo de vida de un NFT do inicio ao fim: cunhagem para o dono, transferencia, marcacao de metadados e queima.