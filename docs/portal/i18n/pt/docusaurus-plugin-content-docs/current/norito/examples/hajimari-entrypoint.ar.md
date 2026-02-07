---
lang: pt
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
slug: /norito/examples/hajimari-entrypoint
título: هيكل نقطة دخول Hajimari
description: O tamanho do Kotodama é um problema que pode ser feito por qualquer pessoa.
fonte: crates/ivm/docs/examples/01_hajimari.ko
---

O Kotodama é um dispositivo que pode ser danificado e danificado.

## جولة دفتر الأستاذ

- أو Modelo `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Você pode usar o dispositivo `ivm_run` / `developer_portal_norito_snippets_run` para obter mais informações `info!` é um problema que está acontecendo.
- Verifique se o `iroha_cli app contracts deploy` e o número de telefone estão em [Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## O SDK está disponível

- [Desbloquear o Rust SDK](/sdks/rust)
- [Implementar para Python SDK](/sdks/python)
- [Escolha o JavaScript SDK](/sdks/javascript)

[Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```