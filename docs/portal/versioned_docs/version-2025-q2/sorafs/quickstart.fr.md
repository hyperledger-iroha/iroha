---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 79a048e6061f7054e14a471004cf7da0dddd3f9bf627d9f1d20ff63803cb0979
source_last_modified: "2026-01-04T17:06:14.405886+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Démarrage rapide

Ce guide pratique présente le profil déterministe du chunker SF-1,
signature du manifeste et flux de récupération multi-fournisseurs qui sous-tendent le SoraFS
canalisation de stockage. Associez-le à la [analyse approfondie du pipeline manifeste](manifest-pipeline.md)
pour les notes de conception et le matériel de référence des indicateurs CLI.

## Prérequis

- Chaîne d'outils Rust (`rustup update`), espace de travail cloné localement.
- Facultatif : [Paire de clés Ed25519 générée par OpenSSL] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  pour signer les manifestes.
- Facultatif : Node.js ≥ 18 si vous prévoyez de prévisualiser le portail Docusaurus.

Définissez `export RUST_LOG=info` tout en expérimentant pour faire apparaître des messages CLI utiles.

## 1. Actualiser les luminaires déterministes

Régénérez les vecteurs de segmentation canoniques SF-1. La commande émet également signé
enveloppes manifestes lorsque `--signing-key` est fourni ; utiliser `--allow-unsigned`
uniquement lors du développement local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Sorties :

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si signé)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Découpez une charge utile et inspectez le plan

Utilisez `sorafs_chunker` pour fragmenter un fichier ou une archive arbitraire :

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Champs clés :

- `profile` / `break_mask` – confirme les paramètres `sorafs.sf1@1.0.0`.
- `chunks[]` – décalages ordonnés, longueurs et résumés BLAKE3 en morceaux.

Pour les appareils plus grands, exécutez la régression basée sur Proptest pour garantir la diffusion en continu et
le regroupement par lots reste synchronisé :

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Créez et signez un manifeste

Enveloppez le plan de fragments, les alias et les signatures de gouvernance dans un manifeste à l'aide de
`sorafs-manifest-stub`. La commande ci-dessous présente une charge utile mono-fichier ; passer
un chemin de répertoire pour empaqueter une arborescence (la CLI le parcourt de manière lexicographique).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Examinez `/tmp/docs.report.json` pour :

- `chunking.chunk_digest_sha3_256` – Résumé SHA3 des décalages/longueurs, correspond au
  luminaires plus gros.
- `manifest.manifest_blake3` – Résumé BLAKE3 signé dans l'enveloppe du manifeste.
- `chunk_fetch_specs[]` – instructions de récupération ordonnées pour les orchestrateurs.

Lorsque vous êtes prêt à fournir de vraies signatures, ajoutez `--signing-key` et `--signer`.
arguments. La commande vérifie chaque signature Ed25519 avant d'écrire le
enveloppe.

## 4. Simuler la récupération multi-fournisseurs

Utilisez la CLI de récupération du développeur pour rejouer le plan de fragmentation sur un ou plusieurs
fournisseurs. C’est idéal pour les tests de fumée CI et le prototypage d’orchestrateur.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Affirmations :

- `payload_digest_hex` doit correspondre au rapport manifeste.
- `provider_reports[]` affiche le nombre de réussites/échecs par fournisseur.
- Un `chunk_retry_total` non nul met en évidence les ajustements de contre-pression.
- Passez `--max-peers=<n>` pour limiter le nombre de fournisseurs programmés pour une exécution
  et gardez les simulations CI concentrées sur les principaux candidats.
- `--retry-budget=<n>` remplace le nombre de tentatives par défaut par morceau (3) afin que vous puissiez
  peut faire apparaître les régressions de l'orchestrateur plus rapidement lors de l'injection d'échecs.

Ajoutez `--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` pour échouer
rapide lorsque la charge utile reconstruite s'écarte du manifeste.

## 5. Prochaines étapes- **Intégration de la gouvernance** – dirigez le résumé du manifeste et
  `manifest_signatures.json` dans le flux de travail du conseil afin que le registre des épingles puisse
  annoncer la disponibilité.
- **Négociation de registre** – consulter [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  avant d'enregistrer de nouveaux profils. L'automatisation devrait préférer les poignées canoniques
  (`namespace.name@semver`) sur les identifiants numériques.
- **Automation CI** – ajoutez les commandes ci-dessus pour publier les pipelines ainsi que la documentation,
  les luminaires et les artefacts publient des manifestes déterministes aux côtés des signatures
  métadonnées.