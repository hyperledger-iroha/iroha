<!-- Auto-generated stub for Azerbaijani (az) translation. Replace this content with the full translation. -->

---
lang: az
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
title: Hacımari giriş nöqtəsi skeleti
description: Tək ictimai giriş nöqtəsi və dövlət tutacağı olan minimal Kotodama müqavilə iskele.
source: crates/ivm/docs/examples/01_hajimari.ko
---

Tək ictimai giriş nöqtəsi və dövlət tutacağı olan minimal Kotodama müqavilə iskele.

## Ledger prospekti

- `koto_compile --abi 1` ilə müqaviləni [Norito Başlarkən](/norito/getting-started#1-compile-a-kotodama-contract) və ya `cargo test -p ivm developer_portal_norito_snippets_compile` vasitəsilə göstərildiyi kimi tərtib edin.
- Bir noda toxunmazdan əvvəl `info!` jurnalını və ilkin sistem çağırışını yoxlamaq üçün `ivm_run` / `developer_portal_norito_snippets_run` ilə bayt kodunu yerli olaraq sınaqdan keçirin.
- Artefaktı `iroha_cli app contracts deploy` vasitəsilə yerləşdirin və [Norito Başlarkən](/norito/getting-started#4-deploy-via-iroha_cli) bölməsindəki addımlardan istifadə edərək manifesti təsdiqləyin.

## Əlaqədar SDK təlimatları

- [Rust SDK sürətli başlanğıc](/sdks/rust)
- [Python SDK sürətli başlanğıc](/sdks/python)
- [JavaScript SDK sürətli başlanğıc](/sdks/javascript)

[Kotodama mənbəyini endirin](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```