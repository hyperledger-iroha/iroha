---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Démarrage rapide SoraFS

Ceci est pratique pour proposer un profil de détection SF-1,
Il est possible d'obtenir des manifestes et des pots pour les fournisseurs non-scolaires, qui sont à votre disposition
конвейера хранения SoraFS. Donnez-moi votre avis
[Manifestations de convertisseur de mouvement](manifest-pipeline.md)
pour la conception et les instructions du drapeau CLI.

## Требования

- Тулчейн Rust (`rustup update`), espace de travail cloniрован локально.
- En option : [pour le clé Ed25519, compatible avec OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  pour les manifestations.
- Fonctionnellement : Node.js ≥ 18, si vous planifiez de télécharger le portail Docusaurus.

Installez `export RUST_LOG=info` dans vos expériences pour que vous puissiez les utiliser
сообщения CLI.

## 1. Обновить детерминированные фикстуры

Créez un vecteur canonique SF-1. La commande s'en chargera
convertit le manifeste pour l'achat `--signing-key` ; utiliser `--allow-unsigned`
только в локальной разработке.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Résultats :

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si possible)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Récupérez la charge utile et recherchez le plan

Utilisez `sorafs_chunker` pour utiliser le fichier ou l'archive du projet :

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Clés pour:

- `profile` / `break_mask` – modifier les paramètres `sorafs.sf1@1.0.0`.
- `chunks[]` – diffusion des séquences, des heures et des jours de BLAKE3.

Pour tous les projets de sociétés, installez la régression sur la base de données, que vous devez utiliser,
Quels sont les éléments et les paquets de mise sous tension qui sont synchronisés :

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Surveillez et postez le manifeste

Объединия план чанков, alias и подписи управления в manifeste с помощью
`sorafs-manifest-stub`. La commande n'a pas besoin de charger la charge utile de la commande ; avant de mettre
Dans le répertoire, vous devez ouvrir la porte (la CLI est là pour vous dans le texte).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Vérifiez le `/tmp/docs.report.json` pour :

- `chunking.chunk_digest_sha3_256` – SHA3-дайджест смещений/длин, совпадает с фикстурами
  чанкователя.
- `manifest.manifest_blake3` – BLAKE3-дайджест, подписанный в конверте манифеста.
- `chunk_fetch_specs[]` – instructions pour les opérateurs.

Lorsque vous envisagez de prendre des photos réelles, ajoutez les arguments `--signing-key`
et `--signer`. La commande a vérifié que l'Ed25519 était disponible avant d'effectuer la conversion.

## 4. Offrez-vous les services des fournisseurs les plus récents

Utilisez dev-CLI pour vos projets afin de planifier des tâches à votre guise ou
нескольких провайдеров. C'est idéal pour les détecteurs de fumée CI et les prototypes
orchestre.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Provenances :

- `payload_digest_hex` doit être compatible avec le manifeste.
- `provider_reports[]` peut fournir des batteries/ustensiles de stockage chez le fournisseur.
- Le nouveau `chunk_retry_total` permet d'obtenir une contre-pression.
- Avant `--max-peers=<n>`, vous devez organiser le travail des fournisseurs dans le magasin et
  сфокусировать CI-simulation на основных кандидатах.
- `--retry-budget=<n>` est un modèle de montre standard pour la broche (3), les pièces
  Vous devez régler la régression de l'opérateur en cas d'infection.Ajoutez `--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` pour que vous puissiez le faire
завершаться сошибкой, когда восстановленный charge utile s'est ouverte sur le manifeste.

## 5. Les petits chats

- **Интеграция с управлением** – передайте дайджест манифеста и
  `manifest_signatures.json` dans le processus de sélection, le registre des broches peut être téléchargé
  доступность.
- **Переговоры с реестром** – ознакомьтесь с [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  Avant l'enregistrement de nouveaux profils. L'automatisation doit permettre de faire passer le message canonique
  хэндлы (`namespace.name@semver`) числовым ID.
- **Автоматизация CI** – добавьте команды выше в release-пайплайны, чтобы документация,
  matériels et objets d'art publiés dans les manifestations publiques
  métadonnées.