---
lang: fr
direction: ltr
source: docs/source/dev_workflow.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3be11bea2cb39520bc3c09f4614555d4ac97760e3590609e2f8df27bf28d1a1a
source_last_modified: "2026-01-18T17:14:31.034360+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Flux de travail de développement d'AGENTS

Ce runbook consolide les garde-fous des contributeurs de la feuille de route AGENTS afin
les nouveaux correctifs suivent les mêmes portes par défaut.

## Objectifs de démarrage rapide

- Exécutez `make dev-workflow` (wrapper autour de `scripts/dev_workflow.sh`) pour exécuter :
  1.`cargo fmt --all`
  2. `cargo clippy --workspace --all-targets --locked -- -D warnings`
  3.`cargo build --workspace --locked`
  4.`cargo test --workspace --locked`
  5. `swift test` de `IrohaSwift/`
- `cargo test --workspace` dure longtemps (souvent des heures). Pour des itérations rapides,
  utilisez `scripts/dev_workflow.sh --skip-tests` ou `--skip-swift`, puis exécutez le
  Séquence avant l'expédition.
- Si `cargo test --workspace` bloque sur les verrous du répertoire de construction, réexécutez avec
  `scripts/dev_workflow.sh --target-dir target/codex-tests` (ou définir
  `CARGO_TARGET_DIR` vers un chemin isolé) pour éviter les conflits.
- Toutes les étapes de chargement utilisent `--locked` pour respecter la politique de conservation du référentiel
  `Cargo.lock` intact. Préférez étendre les caisses existantes plutôt que d’en ajouter
  nouveaux membres de l'espace de travail ; demander l’approbation avant d’introduire une nouvelle caisse.

## Garde-corps- `make check-agents-guardrails` (ou `ci/check_agents_guardrails.sh`) échoue si un
  La branche modifie `Cargo.lock`, introduit de nouveaux membres d'espace de travail ou ajoute de nouveaux
  dépendances. Le script compare l'arborescence de travail et `HEAD` avec
  `origin/main` par défaut ; définissez `AGENTS_BASE_REF=<ref>` pour remplacer la base.
- `make check-dependency-discipline` (ou `ci/check_dependency_discipline.sh`)
  compare les dépendances `Cargo.toml` à la base et échoue sur les nouvelles caisses ; ensemble
  `DEPENDENCY_DISCIPLINE_ALLOW=<dep1,dep2>` pour accuser réception intentionnelle
  ajouts.
- `make check-missing-docs` (ou `ci/check_missing_docs_guard.sh`) bloque les nouveaux
  Entrées `#[allow(missing_docs)]`, drapeaux touchés aux caisses (`Cargo.toml` le plus proche)
  dont `src/lib.rs`/`src/main.rs` manque de documents `//!` au niveau de la caisse et rejette les nouveaux
  éléments publics sans documents `///` relatifs à la référence de base ; ensemble
  `MISSING_DOCS_GUARD_ALLOW=1` uniquement avec l'approbation de l'examinateur. Le gardien aussi
  vérifie que les `docs/source/agents/missing_docs_inventory.{json,md}` sont récents ;
  régénérer avec `python3 scripts/inventory_missing_docs.py`.
- `make check-tests-guard` (ou `ci/check_tests_guard.sh`) signale les caisses dont
  Les fonctions Rust modifiées manquent de preuves de tests unitaires. Les cartes de garde ont changé de lignes
  aux fonctions, réussit si les tests de caisse ont changé dans le diff, et sinon analyse
  fichiers de test existants pour faire correspondre les appels de fonction, donc couverture préexistante
  compte; les caisses sans aucun test de correspondance échoueront. Ensemble `TEST_GUARD_ALLOW=1`
  seulement lorsque les changements sont vraiment neutres en termes de test et que l'examinateur est d'accord.
- `make check-docs-tests-metrics` (ou `ci/check_docs_tests_metrics_guard.sh`)
  applique la politique de la feuille de route selon laquelle les jalons se déplacent parallèlement à la documentation,
  tests et métriques/tableaux de bord. Lorsque `roadmap.md` change par rapport à
  `AGENTS_BASE_REF`, le gardien attend au moins un changement de document, un changement de test,
  et un changement de métrique/télémétrie/tableau de bord. Ensemble `DOC_TEST_METRIC_GUARD_ALLOW=1`
  uniquement avec l'approbation du réviseur.
- `make check-todo-guard` (ou `ci/check_todo_guard.sh`) échoue lorsque les marqueurs TODO
  disparaître sans les modifications accompagnant les documents/tests. Ajouter ou mettre à jour une couverture
  lors de la résolution d'un TODO, ou définissez `TODO_GUARD_ALLOW=1` pour les suppressions intentionnelles.
- `make check-std-only` (ou `ci/check_std_only.sh`) bloque `no_std`/`wasm32`
  cfgs afin que l'espace de travail reste uniquement `std`. Réglez `STD_ONLY_GUARD_ALLOW=1` uniquement pour
  expériences CI sanctionnées.
- `make check-status-sync` (ou `ci/check_status_sync.sh`) maintient la feuille de route ouverte
  section exempte d'éléments terminés et nécessite `roadmap.md`/`status.md` pour
  changer ensemble pour que le plan/statut reste aligné ; ensemble
  `STATUS_SYNC_ALLOW_UNPAIRED=1` uniquement pour les rares corrections de fautes de frappe concernant le statut uniquement après
  épinglant `AGENTS_BASE_REF`.
- `make check-proc-macro-ui` (ou `ci/check_proc_macro_ui.sh`) exécute le trybuild
  Suites d'interface utilisateur pour les caisses de dérive/proc-macro. Exécutez-le en touchant les macros de procédure pour
  maintenir les diagnostics `.stderr` stables et détecter les régressions paniques de l'interface utilisateur ; ensemble
  `PROC_MACRO_UI_CRATES="crate1 crate2"` pour se concentrer sur des caisses spécifiques.
- Reconstructions `make check-env-config-surface` (ou `ci/check_env_config_surface.sh`)
  l'inventaire env-toggle (`docs/source/agents/env_var_inventory.{json,md}`),
  échoue s'il est obsolète, **et** échoue lorsque de nouvelles cales d'environnement de production apparaissent
  par rapport à `AGENTS_BASE_REF` (détecté automatiquement ; défini explicitement si nécessaire).
  Actualisez le tracker après avoir ajouté/supprimé des recherches d'environnement via
  `python3 scripts/inventory_env_toggles.py --json docs/source/agents/env_var_inventory.json --md docs/source/agents/env_var_inventory.md` ;
  utilisez `ENV_CONFIG_GUARD_ALLOW=1` uniquement après avoir documenté les boutons d'environnement intentionnelsdans le suivi des migrations.
- `make check-serde-guard` (ou `ci/check_serde_guard.sh`) régénère le serde
  inventaire d'utilisation (`docs/source/norito_json_inventory.{json,md}`) dans un fichier temporaire
  emplacement, échoue si l'inventaire validé est obsolète et rejette tout nouveau
  la production `serde`/`serde_json` arrive par rapport à `AGENTS_BASE_REF`. Ensemble
  `SERDE_GUARD_ALLOW=1` uniquement pour les expériences CI après dépôt d'un plan de migration.
- `make guards` applique la politique de sérialisation Norito : il refuse les nouveaux
  Utilisation de `serde`/`serde_json`, assistants AoS ad hoc et dépendances SCALE en dehors
  les bancs Norito (`scripts/deny_serde_json.sh`,
  `scripts/check_no_direct_serde.sh`, `scripts/deny_handrolled_aos.sh`,
  `scripts/check_no_scale.sh`).
- **Politique de l'interface utilisateur Proc-macro :** chaque caisse proc-macro doit expédier un `trybuild`
  faisceau (`tests/ui.rs` avec globes réussite/échec) derrière le `trybuild-tests`
  fonctionnalité. Placez les échantillons du chemin heureux sous `tests/ui/pass`, les cas de rejet sous
  `tests/ui/fail` avec sorties `.stderr` validées et maintien des diagnostics
  sans panique et stable. Actualisez les luminaires avec
  `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` (en option avec
  `CARGO_TARGET_DIR=target-codex` pour éviter d'encombrer les versions existantes) et
  évitez de vous fier aux builds de couverture (des gardes `cfg(not(coverage))` sont attendus).
  Pour les macros qui n'émettent pas de point d'entrée binaire, préférez
  `// compile-flags: --crate-type lib` dans les appareils pour garder les erreurs concentrées. Ajouter
  de nouveaux cas négatifs chaque fois que les diagnostics changent.
- CI exécute les scripts de garde-corps via `.github/workflows/agents-guardrails.yml`
  les demandes d'extraction échouent donc rapidement lorsque les politiques sont violées.
- L'exemple de hook git (`hooks/pre-commit.sample`) exécute un garde-corps, une dépendance,
  scripts de documents manquants, std-only, env-config et status-sync pour que les contributeurs
  détecter les violations de politique avant CI. Conservez le fil d'Ariane TODO pour tout problème intentionnel
  des suivis au lieu de reporter silencieusement les changements importants.