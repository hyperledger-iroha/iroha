---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Примеры Norito
описание: Trechos Kotodama выбрано с помощью roteiros do livro razao.
слизень: /норито/examples
---

Это примеры кратких руководств по использованию SDK и рекомендаций по раздаче книг. В этот список включен список проверенных книг и помощник для работы с Rust, Python и JavaScript, чтобы вы могли повторить свой сценарий, начиная с начала фильма.

- **[Эскелет для точки входа Хаджимари](./hajimari-entrypoint)** - Минимальная структура контракта Kotodama с единой публичной точкой входа и дескриптором состояния.
- **[Регистратор доминио и активных действий](./register-and-mint)** - Доступ к зарегистрированному домену с разрешениями, или регистр активный и детерминированный.
- **[Вызов передачи хоста на часть Kotodama](./call-transfer-asset)** - Используйте точку входа Kotodama, чтобы получить инструкции для хоста `transfer_asset` с подтверждением встроенных метаданных.
- **[Transferir ativo entre contas](./transfer-asset)** - Fluxo direto de Transferencia de ativos que espelha os faststarts do SDK и os roteiros do livro razao.
- **[Cunhar, Transferir e queimar um NFT](./nft-flow)** - Выполните цикл жизни NFT по началу: cunhagem para o dono, Transferencia, marcacao de Metadados e queima.