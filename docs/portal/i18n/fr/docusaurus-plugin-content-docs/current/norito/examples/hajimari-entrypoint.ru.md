---
lang: fr
direction: ltr
source: docs/portal/docs/norito/examples/hajimari-entrypoint.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
limace : /norito/examples/hajimari-entrypoint
titre : Каркас входной точки Hajimari
description : Le contrat de carte minimale Kotodama est destiné à une entreprise publique et à un service de ménage.
source : crates/ivm/docs/examples/01_hajimari.ko
---

Le contrat de carte minima Kotodama est destiné à une entreprise publique et à un service de ménage.

## Пошаговый обход реестра

- Compilez le contrat avec `koto_compile --abi 1` en vous rendant dans [Norito Démarrage] (/norito/getting-started#1-compile-a-kotodama-contract) ou ici `cargo test -p ivm developer_portal_norito_snippets_compile`.
- Programmez le serveur de test de fumée localement avec `ivm_run` / `developer_portal_norito_snippets_run` pour vérifier le journal `info!` et l'appel système avant ce thème, afin de le lancer. узел.
- Téléchargez l'artéfact à partir de `iroha_cli app contracts deploy` et téléchargez le manifeste en utilisant les instructions de [Norito Getting Started] (/norito/getting-started#4-deploy-via-iroha_cli).

## SDK de démarrage rapide

- [SDK de démarrage rapide Rust](/sdks/rust)
- [SDK Python de démarrage rapide](/sdks/python)
- [SDK JavaScript de démarrage rapide](/sdks/javascript)

[Скачать исходник Kotodama](/norito-snippets/hajimari-entrypoint.ko)

```text
// Minimal initializer-style function inside a contract.
seiyaku HajimariExample {
  hajimari() {
    info("Hello from hajimari");
  }
}
```