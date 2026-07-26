---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2380aeb86ec99bb00e7779125168f2fd67b505be8dcf456bf86fd133a6585fc2
source_last_modified: "2026-04-03T07:31:18.036095+00:00"
translation_last_reviewed: 2026-04-08
---

---
title: Примеры Norito
description: Подборка Kotodama-сниппетов с пошаговыми обходами реестра.
slug: /norito/examples
---

Эти примеры повторяют quickstart'ы SDK и walkthrough'ы реестра. Каждый сниппет включает чек-лист по реестру и ссылается на руководства по Rust, Python и JavaScript, чтобы вы могли воспроизвести тот же сценарий от начала до конца.

- **[Каркас входной точки Hajimari](./hajimari-entrypoint)** — Минимальный каркас контракта Kotodama с одной публичной точкой входа и хендлом состояния.
- **[Зарегистрировать домен и выпустить активы](./register-and-mint)** — Показывает создание доменов с разрешениями, регистрацию активов и детерминированный выпуск.
- **[Вызвать перенос с хоста из Kotodama](./call-transfer-asset)** — Показывает, как точка входа Kotodama может вызвать инструкцию хоста `transfer_asset` с встроенной проверкой метаданных.
- **[Перевести актив между аккаунтами](./transfer-asset)** — Простой сценарий перевода активов, повторяющий quickstart'ы SDK и walkthrough'ы реестра.
- **[Выпустить, перевести и сжечь NFT](./nft-flow)** — Проводит по жизненному циклу NFT от начала до конца: выпуск владельцу, перевод, добавление метаданных и сжигание.
