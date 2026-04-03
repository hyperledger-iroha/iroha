<!-- Auto-generated stub for Armenian (hy) translation. Replace this content with the full translation. -->

---
lang: hy
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
title: Hajimari մուտքի կետի կմախք
description: Նվազագույն Kotodama պայմանագրային փայտամած մեկ հանրային մուտքի կետով և պետական ​​բռնակով:
source: crates/ivm/docs/examples/01_hajimari.ko
---

Նվազագույն Kotodama պայմանագրային փայտամած մեկ հանրային մուտքի կետով և պետական ​​բռնակով:

## Լեջերի քայլարշավ

- Կազմեք պայմանագիրը `koto_compile --abi 1`-ի հետ, ինչպես ցույց է տրված [Norito Getting Started] (/norito/getting-started#1-compile-a-kotodama-contract) կամ `cargo test -p ivm developer_portal_norito_snippets_compile`-ի միջոցով:
- Ծխեք բայթկոդը լոկալ `ivm_run` / `developer_portal_norito_snippets_run`-ով՝ ստուգելու `info!` մատյանը և սկզբնական syscall-ը նախքան հանգույցին հպվելը:
- Տեղադրեք արտեֆակտը `iroha_cli app contracts deploy`-ի միջոցով և հաստատեք մանիֆեստը՝ օգտագործելով [Norito Getting Started] (/norito/getting-started#4-deploy-via-iroha_cli) քայլերը:

## Առնչվող SDK ուղեցույցներ

- [Rust SDK արագ մեկնարկ] (/sdks/rust)
- [Python SDK արագ մեկնարկ] (/sdks/python)
- [JavaScript SDK արագ մեկնարկ] (/sdks/javascript)

[Ներբեռնեք Kotodama աղբյուրը](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```