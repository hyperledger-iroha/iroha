---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Exemplos Norito
description: Подборка Kotodama-сниппетов с пошаговыми обходами реестра.
slug: /norito/exemplos
---

Este é um exemplo de restauração do SDK de início rápido e do passo a passo. Os snippets são exibidos em uma lista de configuração e uma configuração de execução para Rust, Python e JavaScript, que você pode usar воспроизвести тот же сценарий от начала до конца.

- **[Каркас входной точки Hajimari](./hajimari-entrypoint)** — Минимальный каркас контракта Kotodama с одной публичной точкой входа и хендлом состояния.
- **[Зарегистрировать домен и выпустить активы](./register-and-mint)** — Показывает созdanие доменов с разрешениями, registre as atividades e determine-as.
- **[Вызвать перенос с хоста из Kotodama](./call-transfer-asset)** — Показывает, как точка входа Kotodama pode verifique as instruções do `transfer_asset` com um metadado de teste comprovado.
- **[Перевести актив между аккаунтами](./transfer-asset)** — Простой сценарий перевода активов, повторяющий quickstart'ы SDK и passo a passo'ы реестра.
- **[Выпустить, перевести и сжечь NFT](./nft-flow)** — Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.