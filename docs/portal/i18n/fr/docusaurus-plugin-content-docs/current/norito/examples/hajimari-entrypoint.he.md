---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/examples/hajimari-entrypoint.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 420a557f03368ff1656e201156634466335e6d16fb258641c3af6aad04f5ebdd
source_last_modified: "2025-11-14T04:43:20.713534+00:00"
translation_last_reviewed: 2026-01-30
---

Structure minimale de contrat Kotodama avec un seul point d'entrée public et un gestionnaire d'état.

## Parcours du registre

- Compilez le contrat avec `koto_compile --abi 1` comme indiqué dans [Démarrage de Norito](/norito/getting-started#1-compile-a-kotodama-contract) ou via `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Effectuez un smoke-test du bytecode en local avec `ivm_run` / `developer_portal_norito_snippets_run` pour vérifier le log `info!` et le syscall initial avant de toucher un noeud.
- Déployez l'artefact via `iroha_cli app contracts deploy` et confirmez le manifeste en suivant les étapes de [Démarrage de Norito](/norito/getting-started#4-deploy-via-iroha_cli).

## Guides SDK associés

- [Quickstart SDK Rust](/sdks/rust)
- [Quickstart SDK Python](/sdks/python)
- [Quickstart SDK JavaScript](/sdks/javascript)

[Télécharger la source Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```
