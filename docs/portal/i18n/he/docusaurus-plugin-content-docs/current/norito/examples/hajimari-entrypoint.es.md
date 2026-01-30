---
lang: es
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8e070301537006e5463f5b94c6876dd1edc4d2c4d48955b6f7ca9e133b30751e
source_last_modified: "2025-11-04T12:26:02.940076+00:00"
translation_last_reviewed: 2026-01-30
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
