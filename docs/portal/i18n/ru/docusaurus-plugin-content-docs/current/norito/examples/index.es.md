---
lang: ru
direction: ltr
source: docs/portal/docs/norito/examples/index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: Ejemplos de Norito
описание: Фрагменты Kotodama, выбранные с записями мэра библиотеки.
слизень: /норито/examples
---

В этой статье представлены краткие руководства по SDK и записи библиотеки мэра. Этот фрагмент включает в себя список проверки библиотеки мэра и ссылку на руководства по Rust, Python и JavaScript, чтобы можно было повторить основной сценарий в конце.

- **[Эскелет точки входа Хаджимари](./hajimari-entrypoint)** — Минимальный контракт Kotodama с единственной публичной точкой входа и управляющим государством.
- **[Registrar dominio y acuñar activos](./register-and-mint)** — Demuestra la creación de dominios con Permisos, el registro de activos y la acuñación determinista.
- **[Вызов передачи хоста от Kotodama](./call-transfer-asset)** — Демонстрация точки входа Kotodama может привести к инструкциям хоста `transfer_asset` по проверке метаданных в режиме онлайн.
- **[Transferir activo entre cuentas](./transfer-asset)** — Flujo Directo de Transferencia de Activos que que refleja los Quickstarts de los SDK и los recorridos del libro mayor.
- **[Acuñar, Transferir y quemar un NFT](./nft-flow)** — Восстановите цикл жизни NFT в крайнем случае: учет собственности, передача, этикет метаданных и quema.