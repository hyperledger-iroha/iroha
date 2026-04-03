<!-- Auto-generated stub for Mongolian (mn) translation. Replace this content with the full translation. -->

---
lang: mn
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
title: Хажимари орох цэгийн араг яс
description: Хамгийн бага Kotodama гэрээт шат нь нэг нийтийн нэвтрэх цэг болон муж улсын бариултай.
source: crates/ivm/docs/examples/01_hajimari.ko
---

Хамгийн бага Kotodama гэрээт шат, нийтийн нэвтрэх нэг цэг, муж улсын бариултай.

## Бүртгэлийн дэвтэр

- Гэрээг `koto_compile --abi 1`-тэй [Norito Эхлэл]-д үзүүлсний дагуу (/norito/getting-started#1-compile-a-kotodama-contract) эсвэл `cargo test -p ivm developer_portal_norito_snippets_compile`-ээр дамжуулан эмхэтгэнэ.
- Зангилаанд хүрэхээс өмнө `info!` бүртгэл болон анхны системийн дуудлагыг баталгаажуулахын тулд `ivm_run` / `developer_portal_norito_snippets_run` ашиглан байт кодыг локалаар утааны тест хийнэ.
- `iroha_cli app contracts deploy`-ээр дамжуулан олдворыг байрлуулж, [Norito Эхлэх](/norito/getting-started#4-deploy-via-iroha_cli) хэсэгт байгаа алхмуудыг ашиглан манифестийг баталгаажуулна уу.

## Холбогдох SDK гарын авлага

- [Зэв SDK хурдан эхлүүлэх](/sdks/rust)
- [Python SDK хурдан эхлүүлэх](/sdks/python)
- [JavaScript SDK хурдан эхлүүлэх](/sdks/javascript)

[Kotodama эх сурвалжийг татаж авах](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```