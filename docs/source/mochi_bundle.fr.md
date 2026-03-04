---
lang: fr
direction: ltr
source: docs/source/mochi_bundle.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f2dd292b7d15b449f3cec1b79343387a8c23beef3a163367bd5fa8ced8593aae
source_last_modified: "2026-01-03T18:08:00.656311+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Outillage du pack MOCHI

MOCHI est livré avec un flux de travail d'empaquetage léger afin que les développeurs puissent produire un
ensemble de bureau portable sans câblage de scripts CI sur mesure. Le `xtask`
la sous-commande gère la compilation, la mise en page, le hachage et (éventuellement) l'archive
création en une seule fois.

## Générer un bundle

```bash
cargo xtask mochi-bundle
```

Par défaut, la commande crée les binaires de version, assemble le bundle sous
`target/mochi-bundle/`, et émet une archive `mochi-<os>-<arch>-release.tar.gz`
aux côtés d'un `manifest.json` déterministe. Le manifeste répertorie chaque fichier avec
sa taille et son hachage SHA-256 afin que les pipelines CI puissent réexécuter la vérification ou publier
attestations. L'assistant garantit à la fois le shell de bureau `mochi` et le
Les binaires de l'espace de travail `kagami` sont présents afin que la génération Genesis fonctionne hors du
boîte.

### Drapeaux

| Drapeau | Descriptif |
|-----------|--------------------------------------------------------------------------------------------|
| `--out <dir>` | Remplacez le répertoire de sortie (par défaut, `target/mochi-bundle`).         |
| `--profile <name>` | Créez avec un profil Cargo spécifique (par exemple, `debug` pour les tests).              |
| `--no-archive` | Ignorez l'archive `.tar.gz`, ne laissant que le dossier préparé.               |
| `--kagami <path>` | Utilisez un binaire `kagami` explicite au lieu de créer `iroha_kagami`.         |
| `--matrix <path>` | Ajoutez des métadonnées de bundle à une matrice JSON pour le suivi de la provenance des CI.         |
| `--smoke` | Exécutez `mochi --help` à partir du bundle fourni en tant que porte d'exécution de base.      |
| `--stage <dir>` | Copiez le bundle terminé (et archivez-le, le cas échéant) dans un dossier intermédiaire. |

`--stage` est destiné aux pipelines CI dans lesquels chaque agent de build télécharge son
objets vers un emplacement partagé. L'assistant recrée le répertoire du bundle et
copie l'archive générée dans le répertoire intermédiaire afin que les tâches de publication puissent
collectez les sorties spécifiques à la plate-forme sans script shell.

La disposition à l’intérieur du bundle est volontairement simple :

```
bin/mochi              # egui desktop executable
bin/kagami             # kagami helper for genesis generation
config/sample.toml     # starter supervisor configuration
docs/README.md         # bundle overview and verification guide
LICENSE                # repository licence
manifest.json          # generated file manifest with SHA-256 digests
```

### Remplacements d'exécution

L'exécutable `mochi` accepte les remplacements de ligne de commande pour la plupart
paramètres communs du superviseur. Utilisez ces drapeaux au lieu de modifier
`config/local.toml` lors de l'expérimentation :

```
./bin/mochi --data-root ./data --profile four-peer-bft \
    --torii-start 12000 --p2p-start 14000 \
    --irohad /path/to/irohad --kagami /path/to/kagami
```

Toute valeur CLI est prioritaire sur les entrées et l'environnement `config/local.toml`
variables.

## Automatisation des instantanés

`manifest.json` enregistre l'horodatage de génération, le triple cible, le profil Cargo,
et l'inventaire complet des dossiers. Les pipelines peuvent différer le manifeste pour détecter quand
de nouveaux artefacts apparaissent, téléchargez le JSON avec les ressources de la version ou auditez le
hachages avant de promouvoir un bundle auprès des opérateurs.

L'assistant est idempotent : la réexécution de la commande met à jour le manifeste et
écrase l'archive précédente, en conservant `target/mochi-bundle/` comme unique
source de vérité pour le dernier bundle sur la machine actuelle.