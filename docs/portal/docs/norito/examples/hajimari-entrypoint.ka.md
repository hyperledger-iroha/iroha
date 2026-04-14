<!-- Auto-generated stub for Georgian (ka) translation. Replace this content with the full translation. -->

---
lang: ka
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
title: ჰაჯიმარი შესასვლელი წერტილის ჩონჩხი
description: მინიმალური Kotodama საკონტრაქტო ხარაჩო ერთი საჯარო შესასვლელით და სახელმწიფო სახელურით.
source: crates/ivm/docs/examples/01_hajimari.ko
---

მინიმალური Kotodama საკონტრაქტო ხარაჩო ერთი საჯარო შესასვლელით და სახელმწიფო სახელურით.

## ლეჯერის გზამკვლევი

- შეადგინეთ კონტრაქტი `koto_compile --abi 1`-თან, როგორც ნაჩვენებია [Norito დაწყებაში](/norito/getting-started#1-compile-a-kotodama-contract) ან `cargo test -p ivm developer_portal_norito_snippets_compile`-ის მეშვეობით.
- მოწიეთ ბაიტეკოდი ადგილობრივად `ivm_run` / `developer_portal_norito_snippets_run`-ით, რათა გადაამოწმოთ `info!` ჟურნალი და საწყისი syscall კვანძთან შეხებამდე.
- განათავსეთ არტეფაქტი `iroha_cli app contracts deploy`-ის მეშვეობით და დაადასტურეთ მანიფესტი [Norito დაწყება] (/norito/getting-started#4-deploy-via-iroha_cli) ნაბიჯების გამოყენებით.

## დაკავშირებული SDK სახელმძღვანელო

- [Rust SDK სწრაფი დაწყება] (/sdks/rust)
- [Python SDK სწრაფი დაწყება] (/sdks/python)
- [JavaScript SDK სწრაფი დაწყება] (/sdks/javascript)

[ჩამოტვირთეთ Kotodama წყარო](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```