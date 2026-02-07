---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/examples/hajimari-entrypoint
titre : Esqueleto del Entrypoint Hajimari
description : Demande minimale du contrat Kotodama avec un seul point d'entrée public et un gestionnaire d'État.
source : crates/ivm/docs/examples/01_hajimari.ko
---

Obtenez un minimum de contrat Kotodama avec un point d'entrée public unique et un gestionnaire d'État.

## Recorrido del libro mayor

- Compila le contrat avec `koto_compile --abi 1` comme indiqué dans [Inicio de Norito](/norito/getting-started#1-compile-a-kotodama-contract) ou intermédiaire `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Vous pouvez effectuer une vérification rapide du bytecode localement avec `ivm_run` / `developer_portal_norito_snippets_run` pour vérifier le journal `info!` et l'appel système initial avant de toucher un nœud.
- Dépliez l'artefact avec `iroha_cli app contracts deploy` et confirmez le manuel d'utilisation des étapes de [Inicio de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guides relatifs au SDK

- [Démarrage rapide du SDK de Rust](/sdks/rust)
- [Démarrage rapide du SDK de Python](/sdks/python)
- [Démarrage rapide du SDK de JavaScript](/sdks/javascript)

[Télécharger la source de Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```