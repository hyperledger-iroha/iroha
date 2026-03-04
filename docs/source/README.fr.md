---
lang: fr
direction: ltr
source: docs/source/README.md
status: complete
translator: manual
source_hash: 4a6e55a3232ff38c5c2f45b0a8a3d97471a14603bea75dc2034d7c9c4fb3f862
source_last_modified: "2025-11-10T19:43:50.185052+00:00"
translation_last_reviewed: 2025-11-14
---

# Index de documentation Iroha VM + Kotodama

Cet index regroupe les principaux documents de conception et de référence pour
l’IVM, Kotodama et le pipeline orienté IVM. Pour la version japonaise, voir
[`README.ja.md`](./README.ja.md).

- Architecture de l’IVM et correspondance avec le langage : `../../ivm.md`
- ABI des appels système (syscalls) de l’IVM : `ivm_syscalls.md`
- Constantes de syscalls générées : `ivm_syscalls_generated.md` (exécuter
  `make docs-syscalls` pour régénérer)
- En‑tête du bytecode IVM : `ivm_header.md`
- Grammaire et sémantique de Kotodama : `kotodama_grammar.md`
- Exemples Kotodama et correspondances de syscalls : `kotodama_examples.md`
- Pipeline de transactions (IVM‑first) : `../../new_pipeline.md`
- API des contrats Torii (manifests) : `torii_contracts_api.md`
- Enveloppe de requête JSON (CLI / outils) : `query_json.md`
- Référence du module de streaming Norito : `norito_streaming.md`
- Exemples d’ABI runtime : `samples/runtime_abi_active.md`,
  `samples/runtime_abi_hash.md`, `samples/find_active_abi_versions.md`
- API d’app ZK (pièces jointes, prover, comptage de votes) : `zk_app_api.md`
- Runbook des pièces jointes/prover ZK dans Torii : `zk/prover_runbook.md`
- Guide opérateur de l’API ZK App de Torii (pièces jointes/prover ; doc de crate) :
  `../../crates/iroha_torii/docs/zk_app_api.md`
- Cycle de vie des VK/proofs (registre, vérification, télémétrie) :
  `zk/lifecycle.md`
- Aides opérateur Torii (endpoints de visibilité) : `references/operator_aids.md`
- Quickstart de la lane par défaut Nexus : `quickstart/default_lane.md`
- Quickstart et architecture du superviseur MOCHI : `mochi/index.md`
- Guides du SDK JavaScript (quickstart, configuration, publication) :
  `sdk/js/index.md`
- Tableaux de bord de parité/CI pour le SDK Swift : `references/ios_metrics.md`
- Gouvernance : `../../gov.md`
- Prompts de coordination et clarification : `coordination_llm_prompts.md`
- Feuille de route : `../../roadmap.md`
- Utilisation de l’image de build Docker : `docker_build.md`

Conseils d’utilisation

- Construisez et exécutez les exemples présents dans `examples/` à l’aide des
  outils externes (`koto_compile`, `ivm_run`) :
  - `make examples-run` (et `make examples-inspect` si `ivm_tool` est disponible)
- Des tests d’intégration optionnels (ignorés par défaut) pour les exemples et
  les vérifications d’en‑tête se trouvent dans `integration_tests/tests/`.

Configuration du pipeline

- Tout le comportement runtime est configuré via des fichiers `iroha_config`.
  Les opérateurs n’utilisent pas de variables d’environnement.
- Des valeurs par défaut raisonnables sont fournies ; la plupart des déploiements
  n’auront pas besoin de changements.
- Clés pertinentes sous `[pipeline]` :
  - `dynamic_prepass` : active le prepass en lecture seule de l’IVM pour dériver
    les ensembles d’accès (par défaut : true).
  - `access_set_cache_enabled` : met en cache les ensembles d’accès dérivés par
    `(code_hash, entrypoint)` ; désactivez‑le pour déboguer les hints (par défaut : true).
  - `parallel_overlay` : construit les overlays en parallèle ; le commit reste
    déterministe (par défaut : true).
  - `gpu_key_bucket` : bucketisation optionnelle des clés pour le prepass du
    planificateur en utilisant un radix stable sur `(key, tx_idx, rw_flag)` ; le
    chemin CPU déterministe reste toujours actif (par défaut : false).
  - `cache_size` : capacité du cache global de pré‑décodage de l’IVM (streams
    décodés). Par défaut : 128. L’augmenter peut réduire le temps de décodage
    pour des exécutions répétées.

Vérifications de synchronisation de la doc

- Constantes de syscalls (`docs/source/ivm_syscalls_generated.md`)
  - Régénérer : `make docs-syscalls`
  - Vérification seule : `bash scripts/check_syscalls_doc.sh`
- Tableau d’ABI de syscalls (`crates/ivm/docs/syscalls.md`)
  - Vérification seule :
    `cargo run -p ivm --bin gen_syscalls_doc -- --check --no-code`
  - Mise à jour de la section générée (et du tableau dans la doc de code) :
    `cargo run -p ivm --bin gen_syscalls_doc -- --write`
- Tableaux de pointer‑ABI (`crates/ivm/docs/pointer_abi.md` et `ivm.md`)
  - Vérification seule : `cargo run -p ivm --bin gen_pointer_types_doc -- --check`
  - Mise à jour des sections :
    `cargo run -p ivm --bin gen_pointer_types_doc -- --write`
- Politique d’en‑tête IVM et hashes d’ABI (`docs/source/ivm_header.md`)
  - Vérification seule :
    `cargo run -p ivm --bin gen_header_doc -- --check` et
    `cargo run -p ivm --bin gen_abi_hash_doc -- --check`
  - Mise à jour des sections :
    `cargo run -p ivm --bin gen_header_doc -- --write` et
    `cargo run -p ivm --bin gen_abi_hash_doc -- --write`

CI

- Le workflow GitHub Actions `.github/workflows/check-docs.yml` exécute ces
  vérifications à chaque push/PR et échoue si les documents générés ne sont plus
  alignés sur l’implémentation.
- [Guide de gouvernance](governance_playbook.md)
