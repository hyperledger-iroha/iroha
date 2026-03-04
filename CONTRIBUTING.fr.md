---
lang: fr
direction: ltr
source: CONTRIBUTING.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 71baf5d038cbe6518fd294fcc1b279dff8aaf092e4a83f6159b699a378e51467
source_last_modified: "2025-12-08T10:55:43+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guide de contribution

Merci d'avoir pris le temps de contribuer à Iroha 2 !

Veuillez lire ce guide pour savoir comment vous pouvez contribuer et quelles directives nous attendons de vous que vous suiviez. Cela inclut les directives sur le code et la documentation ainsi que nos conventions concernant le workflow git.

La lecture de ces directives vous fera gagner du temps plus tard.

## Comment puis-je contribuer ?

Il existe de nombreuses façons de contribuer à notre projet :

- Rapport [bugs](#reporting-bugs) et [vulnérabilités](#reporting-vulnerabilities)
- [Suggérer des améliorations](#suggesting-improvements) et les mettre en œuvre
- [Posez des questions](#asking-questions) et engagez-vous avec la communauté

Nouveau dans notre projet ? [Faites votre première contribution](#your-first-code-contribution) !

### TL;DR

- Recherchez [ZenHub](https://app.zenhub.com/workspaces/iroha-v2-60ddb820813b9100181fc060/board?repos=181739240).
- Fourche [Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Résolvez le problème de votre choix.
- Assurez-vous de suivre nos [guides de style](#style-guides) pour le code et la documentation.
- Écrire des [tests](https://doc.rust-lang.org/cargo/commands/cargo-test.html). Assurez-vous qu’ils réussissent tous (`cargo test --workspace`). Si vous touchez la pile de cryptographie SM, exécutez également `cargo test -p iroha_crypto --features "sm sm_proptest"` pour exécuter le faisceau fuzz/propriété en option.
  - Remarque : les tests qui exercent l'exécuteur IVM synthétiseront automatiquement un bytecode d'exécuteur minimal et déterministe si `defaults/executor.to` n'est pas présent. Aucune étape préalable n’est requise pour exécuter les tests. Pour générer le bytecode canonique pour la parité, vous pouvez exécuter :
    -`cargo run --manifest-path scripts/generate_executor_to/Cargo.toml`
    -`cargo run --manifest-path scripts/regenerate_codec_samples/Cargo.toml`
- Si vous modifiez les caisses derive/proc-macro, exécutez les suites trybuild UI via
  `make check-proc-macro-ui` (ou
  `PROC_MACRO_UI_CRATES="crate1 crate2" make check-proc-macro-ui`) et actualiser
  Appareils `.stderr` lorsque les diagnostics changent pour maintenir les messages stables.
- Exécutez `make dev-workflow` (wrapper autour de `scripts/dev_workflow.sh`) pour exécuter fmt/clippy/build/test avec `--locked` plus `swift test` ; attendez-vous à ce que `cargo test --workspace` prenne des heures et utilisez `--skip-tests` uniquement pour des boucles locales rapides. Voir `docs/source/dev_workflow.md` pour le runbook complet.
- Appliquer des garde-corps avec `make check-agents-guardrails` pour bloquer les modifications `Cargo.lock` et les nouvelles caisses d'espace de travail, `make check-dependency-discipline` pour échouer sur les nouvelles dépendances sauf autorisation explicite, et `make check-missing-docs` pour empêcher les nouvelles cales `#[allow(missing_docs)]`, les documents manquants au niveau de la caisse sur touchés caisses, ou de nouveaux éléments publics sans commentaires de document (le garde actualise `docs/source/agents/missing_docs_inventory.{json,md}` via `scripts/inventory_missing_docs.py`). Ajoutez `make check-tests-guard` pour que les fonctions modifiées échouent à moins que les tests unitaires ne les référencent (blocs `#[cfg(test)]`/`#[test]` en ligne ou caisse `tests/` ; nombre de couvertures existantes) et `make check-docs-tests-metrics` afin que les modifications de la feuille de route soient associées aux documents, aux tests et aux métriques/tableaux de bord. Conservez l'application de TODO via `make check-todo-guard` afin que les marqueurs TODO ne soient pas supprimés sans les documents/tests associés. `make check-env-config-surface` régénère l'inventaire de bascule d'environnement et échoue désormais lorsque de nouvelles cales d'environnement de **production** apparaissent par rapport à `AGENTS_BASE_REF` ; définissez `ENV_CONFIG_GUARD_ALLOW=1` uniquement après avoir documenté les ajouts intentionnels dans le suivi de migration. `make check-serde-guard` actualise l'inventaire serde et échoue en cas d'instantanés obsolètes ou de nouveaux hits de production `serde`/`serde_json` ; définissez `SERDE_GUARD_ALLOW=1` uniquement avec un plan de migration approuvé. Gardez les reports importants visibles via le fil d'Ariane TODO et les tickets de suivi au lieu de différer silencieusement. Exécutez `make check-std-only` pour capturer les cfgs `no_std`/`wasm32` et `make check-status-sync` pour vous assurer que les éléments ouverts `roadmap.md` restent ouverts uniquement et que les changements de feuille de route/statut atterrissent ensemble ; définissez `STATUS_SYNC_ALLOW_UNPAIRED=1` uniquement pour les rares corrections de fautes de frappe concernant l'état uniquement après avoir épinglé `AGENTS_BASE_REF`. Pour un seul appel, utilisez `make agents-preflight` pour exécuter tous les garde-corps ensemble.
- Exécutez les gardes de sérialisation locaux avant de pousser : `make guards`.
  - Cela refuse le `serde_json` direct dans le code de production, interdit les nouveaux dépôts de serde directs en dehors de la liste autorisée et empêche les assistants AoS/NCB ad hoc en dehors de `crates/norito`.
- Matrice de fonctionnalités Norito éventuellement exécutée à sec localement : `make norito-matrix` (utilise un sous-ensemble rapide).
  - Pour une couverture complète, exécutez `scripts/run_norito_feature_matrix.sh` sans `--fast`.
  - Pour inclure une fumée en aval par combo (caisse par défaut `iroha_data_model`) : `make norito-matrix-downstream` ou `scripts/run_norito_feature_matrix.sh --fast --downstream [crate]`.
- Pour les caisses proc-macro, ajoutez un faisceau d'interface utilisateur `trybuild` (`tests/ui.rs` + `tests/ui/pass`/`tests/ui/fail`) et validez les diagnostics `.stderr` pour les cas d'échec. Maintenir les diagnostics stables et sans panique ; actualisez les appareils avec `TRYBUILD=overwrite cargo test -p <crate> -F trybuild-tests` et protégez-les avec `cfg(all(feature = "trybuild-tests", not(coverage)))`.
- Effectuer une routine de pré-validation comme le formatage et la régénération des artefacts (voir [`pre-commit.sample`](./hooks/pre-commit.sample))
- Avec le `upstream` configuré pour suivre le [référentiel Hyperledger Iroha] (https://github.com/hyperledger-iroha/iroha), `git pull -r upstream main`, `git commit -s`, `git push <your-fork>` et [créer un pull request](https://github.com/hyperledger-iroha/iroha/compare) à la branche `main`. Assurez-vous qu'il respecte les [directives de demande d'extraction] (#pull-request-etiquette).

### Démarrage rapide du workflow AGENTS

- Exécutez `make dev-workflow` (wrapper autour de `scripts/dev_workflow.sh`, documenté dans `docs/source/dev_workflow.md`). Il englobe `cargo fmt --all`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo build/test --workspace --locked` (les tests peuvent prendre plusieurs heures) et `swift test`.
- Utilisez `scripts/dev_workflow.sh --skip-tests` ou `--skip-swift` pour des itérations plus rapides ; réexécutez la séquence complète avant d’ouvrir une pull request.
- Garde-corps : évitez de toucher à `Cargo.lock`, d'ajouter de nouveaux membres d'espace de travail, d'introduire de nouvelles dépendances, d'ajouter de nouvelles cales `#[allow(missing_docs)]`, d'omettre des documents au niveau de la caisse, de sauter des tests lors de la modification de fonctions, de supprimer des marqueurs TODO sans documents/tests ou de réintroduire `no_std`/`wasm32`. cfgs sans approbation. Exécutez `make check-agents-guardrails` (ou `AGENTS_BASE_REF=origin/main bash ci/check_agents_guardrails.sh`) plus `make check-dependency-discipline`, `make check-missing-docs` (actualise `docs/source/agents/missing_docs_inventory.{json,md}`), `make check-tests-guard` (échoue lorsque les fonctions de production changent sans preuve de test unitaire : soit les tests changent dans le diff, soit les tests existants doivent faire référence au fonction), `make check-docs-tests-metrics` (échec lorsque les modifications de la feuille de route manquent de mises à jour des documents/tests/métriques), `make check-todo-guard`, `make check-env-config-surface` (échec sur les inventaires obsolètes ou les nouvelles bascules d'environnement de production ; remplacement par `ENV_CONFIG_GUARD_ALLOW=1` uniquement après la mise à jour des documents) et `make check-serde-guard`. (échec sur les inventaires de serde obsolètes ou sur les nouveaux hits de serde de production ; remplacement par `SERDE_GUARD_ALLOW=1` uniquement avec un plan de migration approuvé) localement pour un signal précoce, `make check-std-only` pour la garde standard uniquement, et garder `roadmap.md`/`status.md` synchronisé avec `make check-status-sync` (défini `STATUS_SYNC_ALLOW_UNPAIRED=1` uniquement pour les rares corrections de fautes de frappe concernant l'état uniquement après avoir épinglé `AGENTS_BASE_REF`). Utilisez `make agents-preflight` si vous souhaitez qu'une seule commande exécute toutes les gardes avant d'ouvrir un PR.

### Signaler des bogues

Un *bug* est une erreur, un défaut de conception, un échec ou un défaut dans Iroha qui l'amène à produire un résultat ou un comportement incorrect, inattendu ou involontaire.

Nous suivons les bogues Iroha via [GitHub Issues](https://github.com/hyperledger-iroha/iroha/issues?q=is%3Aopen+is%3Aissue+label%3ABug) étiquetés avec la balise `Bug`.

Lorsque vous créez un nouveau problème, vous devez remplir un modèle. Voici la liste de contrôle de ce que vous devez faire lorsque vous signalez des bogues :
- [ ] Ajouter la balise `Bug`
- [ ] Expliquez le problème
- [ ] Fournir un exemple de travail minimum
- [ ] Joindre une capture d'écran

<details> <summary>Exemple de travail minimal</summary>

Pour chaque bogue, vous devez fournir un [exemple de travail minimum](https://en.wikipedia.org/wiki/Minimal_working_example). Par exemple :

```
# Minting negative Assets with value spec `Numeric`.

I was able to mint negative values, which shouldn't be possible in Iroha. This is bad because <X>.

# Given

I managed to mint negative values by running
<paste the code here>

# I expected

not to be able to mint negative values

# But, I got

<code showing negative value>

<paste a screenshot>
```

</détails>

---
**Remarque :** Les problèmes tels qu'une documentation obsolète, une documentation insuffisante ou des demandes de fonctionnalités doivent utiliser les étiquettes `Documentation` ou `Enhancement`. Ce ne sont pas des bugs.

---

### Signalement des vulnérabilités

Bien que nous soyons proactifs dans la prévention des problèmes de sécurité, il est possible que vous rencontriez une vulnérabilité de sécurité avant nous.

- Avant la première version majeure (2.0), toutes les vulnérabilités sont considérées comme des bugs, alors n'hésitez pas à les soumettre en tant que bugs [en suivant les instructions ci-dessus] (#reporting-bugs).
- Après la première version majeure, utilisez notre [programme de prime aux bogues](https://hackerone.com/hyperledger) pour soumettre des vulnérabilités et obtenir votre récompense.

:exclamation : Pour minimiser les dommages causés par une vulnérabilité de sécurité non corrigée, vous devez divulguer la vulnérabilité directement à Hyperledger dès que possible et **éviter de divulguer publiquement la même vulnérabilité** pendant une période de temps raisonnable.

Si vous avez des questions concernant notre gestion des vulnérabilités de sécurité, n'hésitez pas à contacter l'un des responsables actuellement actifs dans les messages privés de Rocket.Chat.

### Suggérer des améliorations

Créez [un problème] (https://github.com/hyperledger-iroha/iroha/issues/new) sur GitHub avec les balises appropriées (`Optimization`, `Enhancement`) et décrivez l'amélioration que vous proposez. Vous pouvez laisser cette idée à nous ou à quelqu'un d'autre pour la développer, ou vous pouvez la mettre en œuvre vous-même.

Si vous avez l'intention de mettre en œuvre la suggestion vous-même, procédez comme suit :

1. Attribuez-vous le problème que vous avez créé **avant** de commencer à travailler dessus.
2. Travaillez sur la fonctionnalité que vous avez suggérée et suivez nos [directives pour le code et la documentation] (#style-guides).
3. Lorsque vous êtes prêt à ouvrir une pull request, assurez-vous de suivre les [directives de la pull request] (#pull-request-etiquette) et de la marquer comme implémentant le problème créé précédemment :

   ```
   feat: Description of the feature

   Explanation of the feature

   Closes #1234
   ```

4. Si votre modification nécessite une modification de l'API, utilisez la balise `api-changes`.

   **Remarque :** les fonctionnalités qui nécessitent des modifications de l'API peuvent prendre plus de temps à mettre en œuvre et à approuver, car elles nécessitent que les créateurs de bibliothèques Iroha mettent à jour leur code.### Poser des questions

Une question est toute discussion qui n'est ni un bug ni une demande de fonctionnalité ou d'optimisation.

<détails> <résumé> Comment poser une question ? </résumé>

Veuillez poster vos questions sur [l'une de nos plateformes de messagerie instantanée](#contact) afin que le personnel et les membres de la communauté puissent vous aider en temps opportun.

En tant que membre de la communauté susmentionnée, vous devriez également envisager d’aider les autres. Si vous décidez d'aider, veuillez le faire de manière [respectueuse] (CODE_OF_CONDUCT.md).

</détails>

## Votre première contribution au code

1. Trouvez un numéro adapté aux débutants parmi les numéros portant le label [good-first-issue](https://github.com/hyperledger-iroha/iroha/labels/good%20first%20issue).
2. Assurez-vous que personne d'autre ne travaille sur les problématiques que vous avez choisies en vérifiant qu'elles ne sont attribuées à personne.
3. Attribuez-vous le problème afin que les autres puissent voir que quelqu'un y travaille.
4. Lisez notre [Rust Style Guide](#rust-style-guide) avant de commencer à écrire du code.
5. Lorsque vous êtes prêt à valider vos modifications, lisez les [directives de demande d'extraction] (#pull-request-etiquette).

## Étiquette des demandes de tirage

Veuillez [fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) le [dépôt](https://github.com/hyperledger-iroha/iroha/tree/main) et [créer une branche de fonctionnalités](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) pour vos contributions. Lorsque vous travaillez avec des **PR de forks**, consultez [ce manuel](https://help.github.com/articles/checking-out-pull-requests-locally).

#### Travailler sur la contribution au code :
- Suivez le [Rust Style Guide](#rust-style-guide) et le [Documentation Style Guide](#documentation-style-guide).
- Assurez-vous que le code que vous avez écrit est couvert par des tests. Si vous avez corrigé un bug, veuillez transformer l'exemple de travail minimum qui reproduit le bug en test.
- Lorsque vous touchez des caisses derive/proc-macro, exécutez `make check-proc-macro-ui` (ou
  filtre avec `PROC_MACRO_UI_CRATES="crate1 crate2"`), alors essayez de créer des appareils d'interface utilisateur
  restez synchronisé et les diagnostics restent stables.
- Documenter les nouvelles API publiques (`//!` au niveau de la caisse et `///` sur les nouveaux éléments) et exécuter
  `make check-missing-docs` pour vérifier le garde-corps. Appelez les documents/tests que vous
  ajouté dans la description de votre demande de tirage.

#### Valider votre travail :
- Suivez le [Git Style Guide](#git-workflow).
- Écrasez vos commits [soit avant](https://www.git-tower.com/learn/git/faq/git-squash/) soit [pendant la fusion](https://rietta.com/blog/github-merge-types/).
- Si lors de la préparation de votre pull request votre branche est devenue obsolète, rebasez-la localement avec `git pull --rebase upstream main`. Vous pouvez également utiliser le menu déroulant du bouton `Update branch` et choisir l'option `Update with rebase`.

  Dans l'intérêt de rendre ce processus plus facile pour tout le monde, essayez de ne pas avoir plus d'une poignée de commits pour une pull request et évitez de réutiliser les branches de fonctionnalités.

#### Création d'une pull request :
- Utilisez une description de demande d'extraction appropriée en suivant les instructions de la section [Étiquette de demande d'extraction] (#pull-request-etiquette). Évitez si possible de vous écarter de ces directives.
- Ajoutez un [titre de la demande d'extraction] au format approprié (#pull-request-titles).
- Si vous sentez que votre code n'est pas prêt à être fusionné, mais que vous souhaitez que les responsables l'examinent, créez un brouillon de demande d'extraction.

#### Fusionner votre travail :
- Une pull request doit réussir toutes les vérifications automatisées avant d'être fusionnée. Au minimum, le code doit être formaté, avoir réussi tous les tests et ne présenter aucune charpie `clippy` exceptionnelle.
- Une pull request ne peut pas être fusionnée sans deux avis d'approbation des responsables actifs.
- Chaque pull request informera automatiquement les propriétaires du code. Une liste à jour des responsables actuels peut être trouvée dans [MAINTAINERS.md](MAINTAINERS.md).

#### Étiquette de révision :
- Ne résolvez pas une conversation par vous-même. Laissez l'examinateur prendre une décision.
- Accepter les commentaires de l'évaluation et interagir avec l'évaluateur (d'accord, en désaccord, clarifier, expliquer, etc.). N'ignorez pas les commentaires.
- Pour les suggestions simples de modification de code, si vous les appliquez directement, vous pouvez résoudre la conversation.
- Évitez d'écraser vos commits précédents lorsque vous appliquez de nouvelles modifications. Cela obscurcit ce qui a changé depuis le dernier examen et oblige le réviseur à repartir de zéro. Les commits sont écrasés avant de fusionner automatiquement.

### Titres des demandes d'extraction

Nous analysons les titres de toutes les demandes d'extraction fusionnées pour générer des journaux de modifications. On vérifie également que le titre respecte la convention via la vérification *`check-PR-title`*.

Pour réussir la vérification *`check-PR-title`*, le titre de la demande d'extraction doit respecter les directives suivantes :

<details> <summary> Développez pour lire les directives détaillées du titre</summary>

1. Suivez le format [commits conventionnels] (https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers).

2. Si la demande d'extraction comporte un seul commit, le titre du PR doit être le même que le message de commit.

</détails>

### Flux de travail Git

- [Fork](https://docs.github.com/en/get-started/quickstart/fork-a-repo) le [dépôt](https://github.com/hyperledger-iroha/iroha/tree/main) et [créer une branche de fonctionnalités](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/creating-and-deleting-branches-within-your-repository) pour vos contributions.
- [Configurer la télécommande](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/configuring-a-remote-repository-for-a-fork) pour synchroniser votre fork avec le [référentiel Hyperledger Iroha](https://github.com/hyperledger-iroha/iroha/tree/main).
- Utilisez le [Git Rebase Workflow](https://git-rebase.io/). Évitez d'utiliser `git pull`. Utilisez plutôt `git pull --rebase`.
- Utilisez les [git hooks](./hooks/) fournis pour faciliter le processus de développement.

Suivez ces directives de validation :

- **Signez chaque commit**. Si vous ne le faites pas, [DCO](https://github.com/apps/dco) ne vous permettra pas de fusionner.

  Utilisez `git commit -s` pour ajouter automatiquement `Signed-off-by: $NAME <$EMAIL>` comme dernière ligne de votre message de validation. Votre nom et votre adresse e-mail doivent être les mêmes que ceux spécifiés dans votre compte GitHub.

  Nous vous encourageons également à signer vos commits avec la clé GPG en utilisant `git commit -sS` ([en savoir plus](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)).

  Vous pouvez utiliser [le hook `commit-msg`](./hooks/) pour signer automatiquement vos commits.

- Les messages de validation doivent suivre les [validations conventionnelles] (https://www.conventionalcommits.org/en/v1.0.0/#commit-message-with-multi-paragraph-body-and-multiple-footers) et le même schéma de dénomination que pour les [titres des demandes d'extraction] (#pull-request-titles). Cela signifie :
  - **Utilisez le présent** (« Ajouter une fonctionnalité », et non « Fonctionnalité ajoutée »)
  - **Utilisez le mode impératif** ("Déployer sur Docker..." et non "Déploiement sur Docker...")
- Écrivez un message de validation significatif.
- Essayez de garder un message de validation court.
- Si vous avez besoin d'un message de commit plus long :
  - Limitez la première ligne de votre message de commit à 50 caractères ou moins.
  - La première ligne de votre message de commit doit contenir le résumé du travail que vous avez effectué. Si vous avez besoin de plus d'une ligne, laissez une ligne vide entre chaque paragraphe et décrivez vos modifications au milieu. La dernière ligne doit être la signature.
- Si vous modifiez le schéma (vérifiez en générant le schéma avec `kagami schema` et diff), vous devez apporter toutes les modifications au schéma dans un commit séparé avec le message `[schema]`.
- Essayez de vous en tenir à un seul engagement par changement significatif.
  - Si vous avez résolu plusieurs problèmes dans un seul PR, attribuez-leur des validations distinctes.
  - Comme mentionné précédemment, les modifications apportées au `schema` et à l'API doivent être effectuées dans des commits appropriés distincts du reste de votre travail.
  - Ajoutez des tests de fonctionnalité dans le même commit que cette fonctionnalité.

## Tests et benchmarks

- Pour exécuter les tests basés sur le code source, exécutez [`cargo test`](https://doc.rust-lang.org/cargo/commands/cargo-test.html) à la racine Iroha. Notez qu'il s'agit d'un long processus.
- Pour exécuter des tests de performances, exécutez [`cargo bench`](https://doc.rust-lang.org/cargo/commands/cargo-bench.html) à partir de la racine Iroha. Pour faciliter le débogage des sorties du test de performance, définissez la variable d'environnement `debug_assertions` comme suit : `RUSTFLAGS="--cfg debug_assertions" cargo bench`.
- Si vous travaillez sur un composant particulier, n'oubliez pas que lorsque vous exécutez `cargo test` dans un [espace de travail] (https://doc.rust-lang.org/cargo/reference/workspaces.html), il exécutera uniquement les tests pour cet espace de travail, qui n'inclut généralement aucun [test d'intégration] (https://www.testingxperts.com/blog/what-is-integration-testing).
- Si vous souhaitez tester vos modifications sur un réseau minimal, le [`docker-compose.yml`](defaults/docker-compose.yml) fourni crée un réseau de 4 homologues Iroha dans des conteneurs Docker qui peuvent être utilisés pour tester le consensus et la logique liée à la propagation des actifs. Nous vous recommandons d'interagir avec ce réseau à l'aide de [`iroha-python`](https://github.com/hyperledger-iroha/iroha-python) ou de la CLI client Iroha incluse.
- Ne supprimez pas les tests ayant échoué. Même les tests ignorés seront éventuellement exécutés dans notre pipeline.
- Si possible, veuillez comparer votre code avant et après avoir apporté vos modifications, car une régression significative des performances peut interrompre les installations des utilisateurs existants.

### Contrôles de garde de sérialisation

Exécutez `make guards` pour valider les stratégies du référentiel localement :

- Liste de refus directe `serde_json` dans les sources de production (préférez `norito::json`).
- Interdire les dépendances/importations directes `serde`/`serde_json` en dehors de la liste autorisée.
- Empêcher la réintroduction des assistants AoS/NCB ad hoc en dehors de `crates/norito`.

### Tests de débogage

<details> <summary> Développez pour savoir comment modifier le niveau de journalisation ou écrire des journaux dans un format JSON.</summary>

Si l'un de vos tests échoue, vous souhaiterez peut-être diminuer le niveau de journalisation maximum. Par défaut, Iroha enregistre uniquement les messages de niveau `INFO`, mais conserve la possibilité de produire des journaux de niveau `DEBUG` et `TRACE`. Ce paramètre peut être modifié soit à l'aide de la variable d'environnement `LOG_LEVEL` pour les tests basés sur le code, soit à l'aide du point de terminaison `/configuration` sur l'un des homologues d'un réseau déployé.Bien que les journaux imprimés dans `stdout` soient suffisants, vous trouverez peut-être plus pratique de produire des journaux au format `json` dans un fichier séparé et de les analyser à l'aide de [node-bunyan](https://www.npmjs.com/package/bunyan) ou de [rust-bunyan](https://crates.io/crates/bunyan).

Définissez la variable d'environnement `LOG_FILE_PATH` sur un emplacement approprié pour stocker les journaux et analysez-les à l'aide des packages ci-dessus.

</détails>

### Débogage à l'aide de la console Tokio

<details> <summary> Développez pour découvrir comment compiler Iroha avec la prise en charge de la console Tokio.</summary>

Parfois, il peut être utile pour le débogage d'analyser les tâches Tokio à l'aide de [tokio-console](https://github.com/tokio-rs/console).

Dans ce cas, vous devez compiler Iroha avec le support de la console Tokyo comme ça :

```bash
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
```

Le port pour la console Tokyo peut être configuré via le paramètre de configuration `LOG_TOKIO_CONSOLE_ADDR` (ou variable d'environnement).
L'utilisation de la console Tokio nécessite que le niveau de journalisation soit `TRACE`, peut être activé via le paramètre de configuration ou la variable d'environnement `LOG_LEVEL`.

Exemple d'exécution de Iroha avec la prise en charge de la console Tokyo en utilisant `scripts/test_env.sh` :

```bash
# 1. Compile Iroha
RUSTFLAGS="--cfg tokio_unstable" cargo build --features tokio-console
# 2. Run Iroha with TRACE log level
LOG_LEVEL=TRACE ./scripts/test_env.sh setup
# 3. Access Iroha. Peers will be available on ports 5555, 5556, ...
tokio-console http://127.0.0.1:5555
```

</détails>

### Profilage

<détails> <résumé> Développez pour découvrir comment créer le profil Iroha. </résumé>

Pour optimiser les performances, il est utile de profiler Iroha.

Les builds de profilage nécessitent actuellement une chaîne d'outils nocturne. Pour en préparer un, compilez Iroha avec le profil et la fonctionnalité `profiling` à l'aide de `cargo +nightly` :

```bash
RUSTFLAGS="-C force-frame-pointers=on" cargo +nightly -Z build-std build --target your-desired-target --profile profiling --features profiling
```

Ensuite, démarrez Iroha et attachez le profileur de votre choix au pid Iroha.

Alternativement, il est possible de créer Iroha dans Docker avec la prise en charge du profileur et de profiler Iroha de cette façon.

```bash
docker build -f Dockerfile.glibc --build-arg="PROFILE=profiling" --build-arg='RUSTFLAGS=-C force-frame-pointers=on' --build-arg='FEATURES=profiling' --build-arg='CARGOFLAGS=-Z build-std' -t iroha:profiling .
```

Par ex. en utilisant perf (disponible uniquement sous Linux):

```bash
# to capture profile
sudo perf record -g -p <PID>
# to analyze profile
sudo perf report
```

Pour pouvoir observer le profil de l'exécuteur pendant le profilage Iroha, l'exécuteur doit être compilé sans supprimer les symboles.
Cela peut être fait en exécutant :

```bash
# compile executor without optimizations
cargo run --bin kagami -- ivm build ./path/to/executor --out-file executor.to
```

Avec la fonctionnalité de profilage activée, Iroha expose le point final aux profils pprof supprimés :

```bash
# profile Iroha for 30 seconds and download the profile data
curl host:port/debug/pprof/profile?seconds=30 -o profile.pb
# analyze profile in browser (required installed go)
go tool pprof -web profile.pb
```

</détails>

## Guides de style

Veuillez suivre ces directives lorsque vous apportez des contributions de code à notre projet :

### Guide de style Git

:book : [Lire les directives de git](#git-workflow)

### Guide des styles de rouille

<details> <summary> :book : Lire les directives du code</summary>

- Utilisez `cargo fmt --all` (édition 2024) pour formater le code.

Lignes directrices du code :

- Sauf indication contraire, reportez-vous aux [Meilleures pratiques Rust] (https://github.com/mre/idiomatic-rust).
- Utilisez le style `mod.rs`. Les [modules auto-nommés](https://rust-lang.github.io/rust-clippy/master/) ne réussiront pas l'analyse statique, sauf dans le cadre des tests [`trybuild`](https://crates.io/crates/trybuild).
- Utilisez une structure de modules axée sur le domaine.

  Exemple : ne faites pas `constants::logger`. Inversez plutôt la hiérarchie en plaçant en premier l'objet pour lequel elle est utilisée : `iroha_logger::constants`.
- Utilisez [`expect`](https://learning-rust.github.io/docs/unwrap-and-expect/) avec un message d'erreur explicite ou une preuve d'infaillibilité au lieu de `unwrap`.
- N'ignorez jamais une erreur. Si vous ne pouvez pas `panic` et que vous ne pouvez pas récupérer, cela doit au moins être enregistré dans le journal.
- Préférez retourner un `Result` au lieu de `panic!`.
- Regrouper les fonctionnalités liées spatialement, de préférence à l'intérieur de modules appropriés.

  Par exemple, au lieu d'avoir un bloc avec les définitions `struct`, puis les `impl` pour chaque structure individuelle, il est préférable d'avoir les `impl` liés à ce `struct` à côté.
- Déclarer avant implémentation : instructions et constantes `use` en haut, tests unitaires en bas.
- Essayez d'éviter les instructions `use` si le nom importé n'est utilisé qu'une seule fois. Cela facilite le déplacement de votre code dans un autre fichier.
- Ne faites pas taire les peluches `clippy` sans discernement. Si vous le faites, expliquez votre raisonnement par un commentaire (ou un message `expect`).
- Préférez `#[outer_attribute]` à `#![inner_attribute]` si l'un ou l'autre est disponible.
- Si votre fonction ne mute aucune de ses entrées (et qu'elle ne devrait muter rien d'autre), marquez-la comme `#[must_use]`.
- Évitez si possible `Box<dyn Error>` (nous préférons un typage fort).
- Si votre fonction est un getter/setter, marquez-la `#[inline]`.
- Si votre fonction est un constructeur (c'est-à-dire qu'elle crée une nouvelle valeur à partir des paramètres d'entrée et appelle `default()`), marquez-la `#[inline]`.
- Évitez de lier votre code à des structures de données concrètes ; `rustc` est suffisamment intelligent pour transformer un `Vec<InstructionExpr>` en `impl IntoIterator<Item = InstructionExpr>` et vice versa lorsque cela est nécessaire.

Directives de dénomination :
- Utilisez uniquement des mots complets dans les noms de structure *publique*, de variable, de méthode, de trait, de constante et de module. Toutefois, les abréviations sont autorisées si :
  - Le nom est local (par exemple arguments de fermeture).
  - Le nom est abrégé selon la convention Rust (par exemple `len`, `typ`).
  - Le nom est une abréviation acceptée (par exemple `tx`, `wsv`, etc.) ; voir le [glossaire du projet](https://docs.iroha.tech/reference/glossary.html) pour les abréviations canoniques.
  - Le nom complet aurait été masqué par une variable locale (par exemple `msg <- message`).
  - Le nom complet aurait rendu le code encombrant avec plus de 5 à 6 mots (par exemple `WorldStateViewReceiverTrait -> WSVRecvTrait`).
- Si vous modifiez les conventions de dénomination, assurez-vous que le nouveau nom que vous avez choisi est _beaucoup_ plus clair que celui que nous avions auparavant.

Lignes directrices pour les commentaires :
- Lorsque vous écrivez des commentaires non-document, au lieu de décrire *ce* que fait votre fonction, essayez d'expliquer *pourquoi* elle fait quelque chose d'une manière particulière. Cela vous fera gagner du temps, à vous et à l'examinateur.
- Vous pouvez laisser les marqueurs `TODO` dans le code tant que vous faites référence à un problème que vous avez créé pour celui-ci. Ne pas créer de problème signifie qu'il ne sera pas fusionné.

Nous utilisons des dépendances épinglées. Suivez ces directives pour la gestion des versions :

- Si votre travail dépend d'une caisse particulière, vérifiez si elle n'a pas déjà été installée à l'aide de [`cargo tree`](https://doc.rust-lang.org/cargo/commands/cargo-tree.html) (utilisez `bat` ou `grep`) et essayez d'utiliser cette version au lieu de la dernière version.
- Utiliser la version complète "X.Y.Z" en `Cargo.toml`.
- Fournir des changements de version dans un PR séparé.

</détails>

### Guide de style de la documentation

<details> <summary> :book : Lire les directives de la documentation</summary>


- Utilisez le format [`Rust Docs`](https://doc.rust-lang.org/cargo/commands/cargo-doc.html).
- Préférez la syntaxe de commentaire sur une seule ligne. Utilisez `///` au-dessus des modules en ligne et `//!` pour les modules basés sur des fichiers.
- Si vous pouvez créer un lien vers la documentation d'une structure/module/fonction, faites-le.
- Si vous pouvez donner un exemple d'utilisation, faites-le. Ceci [est aussi un test](https://doc.rust-lang.org/rustdoc/documentation-tests.html).
- Si une fonction peut générer une erreur ou paniquer, évitez les verbes modaux. Exemple : `Fails if disk IO fails` au lieu de `Can possibly fail, if disk IO happens to fail`.
- Si une fonction peut provoquer une erreur ou une panique pour plusieurs raisons, utilisez une liste à puces de conditions d'échec, avec les variantes `Error` appropriées (le cas échéant).
- Les fonctions *font* des choses. Utilisez l’humeur impérative.
- Les structures *sont* des choses. Allez droit au but. Par exemple, `Log level for reloading from the environment` est meilleur que `This struct encapsulates the idea of logging levels, and is used for reloading from the environment`.
- Les structures ont des champs, qui *sont* aussi des choses.
- Les modules *contiennent* des choses, et nous le savons. Allez droit au but. Exemple : utilisez `Logger-related traits.` au lieu de `Module which contains logger-related logic`.


</détails>

## Contacter

Les membres de notre communauté sont actifs dans :

| Services | Lien |
|---------------|-------------------------------------------------------------------------|
| StackOverflow | https://stackoverflow.com/questions/tagged/hyperledger-iroha |
| Liste de diffusion | https://lists.lfdecentralizedtrust.org/g/iroha |
| Télégramme | https://t.me/hyperledgeriroha |
| Discorde | https://discord.com/channels/905194001349627914/905205848547155968 |

---