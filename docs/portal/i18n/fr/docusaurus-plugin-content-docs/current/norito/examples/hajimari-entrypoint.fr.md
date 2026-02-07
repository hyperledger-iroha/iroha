---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/examples/hajimari-entrypoint
titre : Squelette du point d'entrée Hajimari
description : Structure minimale de contrat Kotodama avec un seul point d'entrée public et un gestionnaire d'état.
source : crates/ivm/docs/examples/01_hajimari.ko
---

Structure minimale de contrat Kotodama avec un seul point d'entrée public et un gestionnaire d'état.

## Parcours du registre

- Compilez le contrat avec `koto_compile --abi 1` comme indiqué dans [Démarrage de Norito](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Effectuez un smoke-test du bytecode en local avec `ivm_run` / `developer_portal_norito_snippets_run` pour vérifier le log `info!` et le syscall initial avant de toucher un nœud.
- Déployez l'artefact via `iroha_cli app contracts deploy` et confirmez le manifeste en suivant les étapes de [Démarrage de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guides SDK associés

- [Démarrage rapide SDK Rust](/sdks/rust)
- [SDK de démarrage rapide Python](/sdks/python)
- [Démarrage rapide SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```