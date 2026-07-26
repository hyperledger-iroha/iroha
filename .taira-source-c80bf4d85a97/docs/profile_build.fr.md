---
lang: fr
direction: ltr
source: docs/profile_build.md
status: complete
translator: manual
source_hash: 9698d31da47926ae882dc1c93152ecd3865767be6262f14b71253dbd8b2a0fa9
source_last_modified: "2025-11-02T04:40:28.811778+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/profile_build.md (Profiling iroha_data_model Build) -->

# Profiler la compilation de `iroha_data_model`

Pour identifier les étapes lentes dans la compilation de `iroha_data_model`, exécutez le
script d’aide :

```sh
./scripts/profile_build.sh
```

Ce script lance `cargo build -p iroha_data_model --timings` et écrit les rapports de temps
dans `target/cargo-timings/`.
Ouvrez `cargo-timing.html` dans un navigateur et triez les tâches par durée pour voir
quels crates ou étapes de build sont les plus coûteux.

Servez‑vous de ces timings pour concentrer les efforts d’optimisation sur les tâches les
plus lentes.

