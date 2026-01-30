---
lang: fr
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 195b70e63ccabb93a8dd88b699e0a96d9460e9da13a63ec8dce792b1ebc3b437
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Hebrew (he) translation. Replace this content with the full translation. -->

---
lang: he
direction: rtl
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
slug: /norito/examples/hajimari-entrypoint
title: שלד נקודת כניסה Hajimari
description: שלד חוזה Kotodama מינימלי עם נקודת כניסה ציבורית אחת וידית מצב.
source: crates/ivm/docs/examples/01_hajimari.ko
---

שלד חוזה Kotodama מינימלי עם נקודת כניסה ציבורית אחת וידית מצב.

## סיור בספר החשבונות

- קומפלו את החוזה עם `koto_compile --abi 1` כפי שמוצג ב-[Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) או באמצעות `cargo test -p ivm developer_portal_norito_snippets_compile`.
- בצעו בדיקת עשן לבייטקוד מקומית עם `ivm_run` / `developer_portal_norito_snippets_run` כדי לאמת את לוג `info!` ואת ה-syscall הראשוני לפני שנוגעים בצומת.
- פרסו את הארטיפקט באמצעות `iroha_cli app contracts deploy` ואמתו את המניפסט באמצעות השלבים ב-[Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## מדריכי SDK קשורים

- [Quickstart של Rust SDK](/sdks/rust)
- [Quickstart של Python SDK](/sdks/python)
- [Quickstart של JavaScript SDK](/sdks/javascript)

[הורדת מקור Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
