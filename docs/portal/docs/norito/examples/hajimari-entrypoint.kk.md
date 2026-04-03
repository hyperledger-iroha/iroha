<!-- Auto-generated stub for Kazakh (kk) translation. Replace this content with the full translation. -->

---
lang: kk
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
title: Хаджимари кіру нүктесінің қаңқасы
description: Жалғыз қоғамдық кіру нүктесі және күй дескрипті бар ең аз Kotodama келісімшарттық тірек.
source: crates/ivm/docs/examples/01_hajimari.ko
---

Жалғыз қоғамдық кіру нүктесі және күй дескрипті бар ең аз Kotodama келісімшарттық тірек.

## Бухгалтерлік кітапшаға шолу

- `koto_compile --abi 1` келісім-шартын [Norito Жұмысқа кірісу](/norito/getting-started#1-compile-a-kotodama-contract) бөлімінде көрсетілгендей немесе `cargo test -p ivm developer_portal_norito_snippets_compile` арқылы құрастырыңыз.
- Түйінге қол тигізбес бұрын `info!` журналын және бастапқы жүйе қоңырауын тексеру үшін `ivm_run` / `developer_portal_norito_snippets_run` көмегімен байт-кодты жергілікті түрде сынап көріңіз.
- `iroha_cli app contracts deploy` арқылы артефактты орналастырыңыз және [Norito Жұмысқа кірісу](/norito/getting-started#4-deploy-via-iroha_cli) бөліміндегі қадамдарды пайдаланып манифестті растаңыз.

## Қатысты SDK нұсқаулықтары

- [Rust SDK жылдам іске қосу](/sdks/rust)
- [Python SDK жылдам іске қосу](/sdks/python)
- [JavaScript SDK жылдам іске қосу](/sdks/javascript)

[Kotodama көзін жүктеп алыңыз](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```