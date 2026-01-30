---
lang: es
direction: ltr
source: docs/portal/docs/norito/examples/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71585d0aa92f516519d69df408cf4255b90f5bce89ce8022f6c7b60871dc057b
source_last_modified: "2025-11-11T09:31:19.097792+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Ejemplos de Norito
description: Fragmentos de Kotodama seleccionados con recorridos del libro mayor.
slug: /norito/examples
---

Estos ejemplos reflejan los quickstarts de los SDK y los recorridos del libro mayor. Cada fragmento incluye una lista de verificación del libro mayor y enlaza a las guías de Rust, Python y JavaScript para que puedas repetir el mismo escenario de principio a fin.

- **[Esqueleto del entrypoint Hajimari](./hajimari-entrypoint)** — Andamiaje mínimo de contrato Kotodama con un único entrypoint público y un manejador de estado.
- **[Registrar dominio y acuñar activos](./register-and-mint)** — Demuestra la creación de dominios con permisos, el registro de activos y la acuñación determinista.
- **[Invocar transferencia del host desde Kotodama](./call-transfer-asset)** — Demuestra cómo un entrypoint de Kotodama puede llamar a la instrucción de host `transfer_asset` con validación de metadatos en línea.
- **[Transferir activo entre cuentas](./transfer-asset)** — Flujo directo de transferencia de activos que refleja los quickstarts de los SDK y los recorridos del libro mayor.
- **[Acuñar, transferir y quemar un NFT](./nft-flow)** — Recorre el ciclo de vida de un NFT de extremo a extremo: acuñación al propietario, transferencias, etiquetado de metadatos y quema.
