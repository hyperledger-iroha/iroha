---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/examples/hajimari-entrypoint
titre : Esqueleto do point d'entrée Hajimari
description : Structure minimale du contrat Kotodama avec un point d'entrée unique public et une poignée d'état.
source : crates/ivm/docs/examples/01_hajimari.ko
---

Structure minimale du contrat Kotodama avec un point d'entrée unique public et une poignée d'état.

## Roteiro do livro razão

- Compilez le contrat avec `koto_compile --abi 1` conformément au [Norito Getting Started](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Effectuez un test de fumée du bytecode local avec `ivm_run` / `developer_portal_norito_snippets_run` pour vérifier le journal `info!` et un appel système initial avant de lancer un nœud.
- Implantez l'artefato via `iroha_cli app contracts deploy` et confirmez le manifeste en utilisant les passos em [Norito Getting Started](/norito/getting-started#4-deploy-via-iroha_cli).

## Guides des utilisateurs du SDK

- [Démarrage rapide du SDK Rust](/sdks/rust)
- [Démarrage rapide du SDK Python](/sdks/python)
- [Démarrage rapide du SDK JavaScript](/sdks/javascript)

[Baixe a fonte Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```