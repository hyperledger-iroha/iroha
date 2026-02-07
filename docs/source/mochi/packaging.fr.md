---
lang: fr
direction: ltr
source: docs/source/mochi/packaging.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7ab0877a6f43402d6ec13a44c4a7c2b68e4a49e6103bb50d7469d9e71aaa953
source_last_modified: "2026-01-03T18:07:57.001158+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Guide d'emballage MOCHI

Ce guide explique comment créer le bundle de superviseur de bureau MOCHI, inspecter
les artefacts générés et ajustez les remplacements d'exécution fournis avec le
paquet. Il complète le quickstart en se concentrant sur les emballages reproductibles
et l'utilisation de CI.

## Prérequis

- Chaîne d'outils Rust (édition 2024 / Rust 1.82+) avec dépendances d'espace de travail
  déjà construit.
- `irohad`, `iroha_cli` et `kagami` compilés pour la cible souhaitée. Le
  bundler réutilise les binaires de `target/<profile>/`.
- Espace disque suffisant pour la sortie du bundle sous `target/` ou un personnalisé
  destination.

Créez les dépendances une fois avant d'exécuter le bundler :

```bash
cargo build -p irohad -p iroha_cli -p iroha_kagami
```

## Construire le bundle

Appelez la commande dédiée `xtask` depuis la racine du référentiel :

```bash
cargo xtask mochi-bundle
```

Par défaut, cela produit un bundle de versions sous `target/mochi-bundle/` avec un
nom de fichier dérivé du système d'exploitation hôte et de l'architecture (par exemple,
`mochi-macos-aarch64-release.tar.gz`). Utilisez les drapeaux suivants pour personnaliser
la construction :

- `--profile <name>` – choisissez un profil Cargo (`release`, `debug` ou un
  profil personnalisé).
- `--no-archive` – conserver le répertoire développé sans créer de `.tar.gz`
  archive (utile pour les tests locaux).
- `--out <path>` – écrivez les bundles dans un répertoire personnalisé au lieu de
  `target/mochi-bundle/`.
- `--kagami <path>` – fournit un exécutable `kagami` prédéfini à inclure dans le
  archiver. En cas d'omission, le bundler réutilise (ou construit) le binaire à partir du
  profil sélectionné.
- `--matrix <path>` – ajouter les métadonnées du bundle à un fichier matriciel JSON (créé si
  manquant) afin que les pipelines CI puissent enregistrer chaque artefact hôte/profil produit dans un
  courir. Les entrées incluent le répertoire du bundle, le chemin du manifeste et SHA-256, facultatif
  l'emplacement des archives et le dernier résultat du test de fumée.
- `--smoke` – exécutez le `mochi --help` emballé comme un portail de fumée léger
  après le regroupement ; les échecs font apparaître les dépendances manquantes avant de publier un
  artefact.
- `--stage <path>` – copiez le bundle terminé (et archivez-le une fois produit) dans
  un répertoire intermédiaire afin que les versions multiplateformes puissent déposer des artefacts dans un seul
  emplacement sans script supplémentaire.

La commande copie `mochi-ui-egui`, `kagami`, `LICENSE`, l'échantillon
configuration et `mochi/BUNDLE_README.md` dans le bundle. Un déterministe
`manifest.json` est généré avec les binaires afin que les tâches CI puissent suivre le fichier
hachages et tailles.

## Disposition et vérification du bundle

Un ensemble étendu suit la présentation documentée dans `BUNDLE_README.md` :

```
bin/mochi
bin/kagami
config/sample.toml
docs/README.md
manifest.json
LICENSE
```

Le fichier `manifest.json` répertorie chaque artefact avec son hachage SHA-256. Vérifier
le bundle après l'avoir copié sur un autre système :

```bash
jq -r '.files[] | "\(.sha256)  \(.path)"' manifest.json | sha256sum --check
```

Les pipelines CI peuvent mettre en cache le répertoire développé, signer l'archive ou publier
le manifeste aux côtés des notes de version. Le manifeste inclut le générateur
profil, triple cible et horodatage de création pour faciliter le suivi de la provenance.

## Remplacements d'exécution

MOCHI découvre les binaires d'assistance et les emplacements d'exécution via les indicateurs CLI ou
variables d'environnement :- `--data-root` / `MOCHI_DATA_ROOT` – remplace l'espace de travail utilisé pour le homologue
  configurations, stockage et journaux.
- `--profile` – basculer entre les préréglages de topologie (`single-peer`,
  `four-peer-bft`).
- `--torii-start`, `--p2p-start` – modifiez les ports de base utilisés lors de l'allocation
  prestations.
- `--irohad` / `MOCHI_IROHAD` – pointe vers un binaire `irohad` spécifique.
- `--kagami` / `MOCHI_KAGAMI` – remplace le `kagami` fourni.
- `--iroha-cli` / `MOCHI_IROHA_CLI` – remplace l'assistant CLI en option.
- `--restart-mode <never|on-failure>` – désactive les redémarrages automatiques ou force le
  politique d’attente exponentielle.
- `--restart-max <attempts>` – remplace le nombre de tentatives de redémarrage lorsque
  fonctionnant en mode `on-failure`.
- `--restart-backoff-ms <millis>` – définit l'intervalle de base pour les redémarrages automatiques.
- `MOCHI_CONFIG` – fournit un chemin `config/local.toml` personnalisé.

L'aide CLI (`mochi --help`) imprime la liste complète des indicateurs. Remplacements d'environnement
prendre effet au lancement et peut être combiné avec la boîte de dialogue Paramètres à l'intérieur du
Interface utilisateur.

## Conseils d'utilisation de CI

- Exécutez `cargo xtask mochi-bundle --no-archive` pour générer un répertoire pouvant
  être compressé avec des outils spécifiques à la plate-forme (ZIP pour Windows, archives tar pour
  Unix).
- Capturez les métadonnées du bundle avec `cargo xtask mochi-bundle --matrix dist/matrix.json`
  afin que les tâches de publication puissent publier un seul index JSON répertoriant chaque hôte/profil
  artefact produit dans le pipeline.
- Utilisez `cargo xtask mochi-bundle --stage /mnt/staging/mochi` (ou similaire) sur chaque
  agent de build pour télécharger le bundle et l'archiver dans un répertoire partagé que le
  le travail de publication peut consommer.
- Publiez à la fois l'archive et `manifest.json` afin que les opérateurs puissent vérifier l'ensemble.
  intégrité.
- Stockez le répertoire généré en tant qu'artefact de construction pour générer des tests de fumée qui
  exercer le superviseur avec des binaires empaquetés de manière déterministe.
- Enregistrez les hachages du bundle dans les notes de version ou dans le journal `status.md` pour l'avenir
  contrôles de provenance.