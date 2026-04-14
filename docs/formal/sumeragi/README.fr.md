<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/formal/sumeragi/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 56f1412b2db729ba69057ce15ac8bae707310fd5a6d01be2da816fdee18218f7
source_last_modified: "2026-02-23T14:48:46.580877+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Sumeragi Modèle formel (TLA+ / Apalache)

Ce répertoire contient un modèle formel limité pour la sécurité et la vivacité du chemin de validation Sumeragi.

## Portée

Le modèle capture :
- évolution des phases (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- les seuils de vote et de quorum (`CommitQuorum`, `ViewQuorum`),
- quorum de participation pondéré (`StakeQuorum`) pour les commit guards de type NPoS,
- Causalité GR (`Init -> Chunk -> Ready -> Deliver`) avec preuve d'en-tête/digest,
- TPS et faibles hypothèses d'équité par rapport aux actions de progrès honnêtes.

Il supprime intentionnellement les formats de fils, les signatures et tous les détails du réseau.

## Fichiers

- `Sumeragi.tla` : modèle et propriétés du protocole.
- `Sumeragi_fast.cfg` : jeu de paramètres plus petit compatible CI.
- `Sumeragi_deep.cfg` : jeu de paramètres de contrainte plus large.

## Propriétés

Invariants :
-`TypeInvariant`
-`CommitImpliesQuorum`
-`CommitImpliesStakeQuorum`
-`CommitImpliesDelivered`
-`DeliverImpliesEvidence`

Propriété temporelle :
- `EventuallyCommit` (`[] (gst => <> committed)`), avec équité post-TPS codée
  opérationnellement dans `Next` (protections de préemption de délai/défaut activées
  actions de progrès). Cela permet de conserver le modèle vérifiable avec Apalache 0.52.x, qui
  ne prend pas en charge les opérateurs d'équité `WF_` dans les propriétés temporelles vérifiées.

## En cours d'exécution

Depuis la racine du référentiel :

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
```

### Configuration locale reproductible (aucun Docker requis)Installez la chaîne d'outils Apalache locale épinglée utilisée par ce référentiel :

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

Le programme d'exécution détecte automatiquement cette installation à :
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
Après l'installation, `ci/check_sumeragi_formal.sh` devrait fonctionner sans variables d'environnement supplémentaires :

```bash
bash ci/check_sumeragi_formal.sh
```

Si Apalache n'est pas dans `PATH`, vous pouvez :

- définissez `APALACHE_BIN` sur le chemin de l'exécutable, ou
- utiliser le repli Docker (activé par défaut lorsque `docker` est disponible) :
  - image : `APALACHE_DOCKER_IMAGE` (par défaut `ghcr.io/apalache-mc/apalache:latest`)
  - nécessite un démon Docker en cours d'exécution
  - désactiver le repli avec `APALACHE_ALLOW_DOCKER=0`.

Exemples :

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:latest bash scripts/formal/sumeragi_apalache.sh deep
```

## Remarques

- Ce modèle complète (ne remplace pas) les tests de modèles Rust exécutables dans
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  et
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- Les contrôles sont délimités par des valeurs constantes dans les fichiers `.cfg`.
- PR CI effectue ces contrôles dans `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.