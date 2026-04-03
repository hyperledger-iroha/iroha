<!-- Auto-generated stub for Uzbek (uz) translation. Replace this content with the full translation. -->

---
lang: uz
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8367687fcf43fcb50ab43940a4ebeb8b8ba22a3ab8a6c3ed5088c52b1fdd7baf
source_last_modified: "2026-01-22T15:38:30.521640+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

---
slug: /norito/examples/hajimari-entrypoint
title: Hajimari kirish nuqtasi skeleti
description: Bitta umumiy kirish nuqtasi va davlat tutqichi bilan minimal Kotodama kontrakt iskala.
source: crates/ivm/docs/examples/01_hajimari.ko
---

Bitta umumiy kirish nuqtasi va davlat tutqichi bilan minimal Kotodama kontrakt iskala.

## Buxgalteriya kitobi bo'yicha ko'rsatmalar

- `koto_compile --abi 1` bilan shartnomani [Norito Ishga kirishish](/norito/getting-started#1-compile-a-kotodama-contract) yoki `cargo test -p ivm developer_portal_norito_snippets_compile` orqali ko'rsatilganidek tuzing.
- Tugunga tegmasdan oldin `info!` jurnali va dastlabki tizim qoʻngʻiroqlarini tekshirish uchun `ivm_run` / `developer_portal_norito_snippets_run` bilan bayt-kodni mahalliy ravishda tutun sinovidan oʻtkazing.
- `iroha_cli app contracts deploy` orqali artefaktni o'rnating va [Norito Ishga kirishish](/norito/getting-started#4-deploy-via-iroha_cli) bo'limidagi qadamlar yordamida manifestni tasdiqlang.

## Tegishli SDK qo'llanmalari

- [Rust SDK tezkor ishga tushirish](/sdks/rust)
- [Python SDK tezkor ishga tushirish](/sdks/python)
- [JavaScript SDK tezkor ishga tushirish](/sdks/javascript)

[Kotodama manbasini yuklab oling](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```