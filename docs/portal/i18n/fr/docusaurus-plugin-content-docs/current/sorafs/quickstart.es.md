---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Démarrage rapide de SoraFS

Ce guide pratique recorre le profil déterministe du chunker SF-1,
la société de manifestes et le flux de récupération multi-fournisseurs qui soutiennent le
pipeline de stockage de SoraFS. Complète avec le
[analyse approfondie du pipeline des manifestes](manifest-pipeline.md)
pour les notes de conception et la référence aux drapeaux de la CLI.

## Conditions préalables

- Toolchain de Rust (`rustup update`), espace de travail cloné localement.
- Facultatif : [par des clés Ed25519 générées avec OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  para firmar manifestes.
- Facultatif : Node.js ≥ 18 si vous planifiez de prévisualiser le portail de Docusaurus.

Définissez `export RUST_LOG=info` pendant vos expériences pour afficher les messages utiles de la CLI.

## 1. Actualisation des rencontres déterministes

Régénère les vecteurs canoniques de chunking SF-1. Le commandant émet également
sobres de manifiesto firmados cuando se proporciona `--signing-key` ; Etats-Unis
`--allow-unsigned` seul pendant le développement local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Salidas:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si ferme)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Fragmenter une charge utile et inspecter le plan

Utilisez `sorafs_chunker` pour fragmenter un fichier ou un fichier compressé arbitrairement :

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Clé de Campos :

- `profile` / `break_mask` – confirme les paramètres de `sorafs.sf1@1.0.0`.
- `chunks[]` – compense les ordres, les longitudes et digère BLAKE3 des morceaux.

Pour les rencontres les plus grandes, la régression est exécutée par proptest pour
garantir que le chunking en streaming et pour beaucoup soit synchronisé :

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construire et annoncer un manifeste

Envoyez le plan de morceaux, les alias et les entreprises de gouvernement dans un manifeste d'utilisation
`sorafs-manifest-stub`. Le commandant de bord doit afficher une charge utile d'un seul fichier ; pasa
un itinéraire de répertoire pour empaqueter un arbre (la CLI l'enregistrera dans l'ordre lexicográfico).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Révision `/tmp/docs.report.json` para :

- `chunking.chunk_digest_sha3_256` – résumé SHA3 des décalages/longitudes, coïncide avec les
  luminaires del chunker.
- `manifest.manifest_blake3` – digérer BLAKE3 confirmé dans le cadre du manifeste.
- `chunk_fetch_specs[]` – instructions de récupération ordonnées pour les orchestres.

Lorsque cette liste est destinée à porter des entreprises réelles, ajoutez les arguments `--signing-key` et
`--signer`. La commande vérifie chaque entreprise Ed25519 avant d'écrire sur le sujet.

## 4. Simula la récupération multi-fournisseur

Utilisez la CLI pour récupérer le téléchargement pour reproduire le plan de morceaux contre un ou plus
fournisseurs. Il est idéal pour les tests de fumée de CI et les prototypes d'orquestador.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Comprobations :- `payload_digest_hex` doit coïncider avec l'information du manifeste.
- `provider_reports[]` doit contenir des contes d'exito/fallo por provenedor.
- Un `chunk_retry_total` distinct de zéro permet de régler la contre-pression.
- Pas `--max-peers=<n>` pour limiter le nombre de fournisseurs programmés pour une exécution
  et maintenir les simulations de CI enfocadas chez les candidats principaux.
- `--retry-budget=<n>` indique le constat de défaut de réintention du morceau (3) pour
  détecter les régressions les plus rapides de l'observateur des chutes d'injecteurs.

Ajouter `--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` pour une chute rapide
lorsque la charge sera reconstruite à la fin du manifeste.

## 5. Suivantes étapes

- **Integración de gobernanza** – canaliser le résumé du manifeste et
  `manifest_signatures.json` dans le flux du conseil pour que le registre Pin puisse être
  annonce la disponibilité.
- **Négociation du registre** – consultation [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar nouveaux profils. L'automatisation doit préférer les opérateurs canoniques
  (`namespace.name@semver`) sur les identifiants numériques.
- **Automatización de CI** – ajouter les commandes antérieures aux pipelines de publication pour cela
  la documentation, les installations et les artefacts publics manifestes déterminants conjointement avec
  métadonnées firmados.