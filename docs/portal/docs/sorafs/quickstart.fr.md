<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c93ecca74f97ebe64c8b8a529a92de44b19ad3b692add43796d9413d5c08ae4b
source_last_modified: "2025-11-02T17:51:26.194476+00:00"
translation_last_reviewed: 2025-12-28
---

# Démarrage rapide SoraFS

Ce guide pratique passe en revue le profil de chunker SF-1 déterministe,
la signature des manifestes et le flux de récupération multi-fournisseurs qui
sous-tendent le pipeline de stockage SoraFS. Complétez-le par
l'[analyse approfondie du pipeline de manifestes](manifest-pipeline.md)
pour les notes de conception et la référence des flags CLI.

## Prérequis

- Toolchain Rust (`rustup update`), workspace cloné localement.
- Optionnel : [paire de clés Ed25519 générée par OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  pour signer les manifestes.
- Optionnel : Node.js ≥ 18 si vous prévoyez de prévisualiser le portail Docusaurus.

Définissez `export RUST_LOG=info` pendant les essais pour afficher des messages CLI utiles.

## 1. Rafraîchir les fixtures déterministes

Régénérez les vecteurs de découpage SF-1 canoniques. La commande produit aussi des
enveloppes de manifeste signées lorsque `--signing-key` est fourni ; utilisez
`--allow-unsigned` uniquement en développement local.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Sorties :

- `fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
- `fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (si signé)
- `fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Découpez un payload et inspectez le plan

Utilisez `sorafs_chunker` pour découper un fichier ou une archive arbitraire :

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Champs clés :

- `profile` / `break_mask` – confirme les paramètres de `sorafs.sf1@1.0.0`.
- `chunks[]` – offsets ordonnés, longueurs et empreintes BLAKE3 des chunks.

Pour des fixtures plus volumineuses, exécutez la régression basée sur proptest afin
d'assurer que le découpage en streaming et par lot reste synchronisé :

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construire et signer un manifeste

Enveloppez le plan de chunks, les alias et les signatures de gouvernance dans un
manifeste via `sorafs-manifest-stub`. La commande ci-dessous illustre un payload à
fichier unique ; passez un chemin de répertoire pour empaqueter un arbre (la CLI le
parcourt en ordre lexicographique).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Examinez `/tmp/docs.report.json` pour :

- `chunking.chunk_digest_sha3_256` – empreinte SHA3 des offsets/longueurs, correspond aux
  fixtures du chunker.
- `manifest.manifest_blake3` – empreinte BLAKE3 signée dans l'enveloppe du manifeste.
- `chunk_fetch_specs[]` – instructions de récupération ordonnées pour les orchestrateurs.

Quand vous êtes prêt à fournir de vraies signatures, ajoutez les arguments
`--signing-key` et `--signer`. La commande vérifie chaque signature Ed25519 avant
d'écrire l'enveloppe.

## 4. Simuler une récupération multi-fournisseurs

Utilisez la CLI de fetch de développement pour rejouer le plan de chunks contre un ou
plusieurs fournisseurs. C'est idéal pour les smoke tests CI et le prototypage
d'orchestrateur.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Vérifications :

- `payload_digest_hex` doit correspondre au rapport du manifeste.
- `provider_reports[]` expose les comptes de succès/échec par fournisseur.
- Un `chunk_retry_total` non nul met en évidence les ajustements de back-pressure.
- Passez `--max-peers=<n>` pour limiter le nombre de fournisseurs planifiés pour une
  exécution et garder les simulations CI centrées sur les candidats principaux.
- `--retry-budget=<n>` remplace le nombre de tentatives par chunk par défaut (3) afin de
  mettre en évidence plus vite les régressions de l'orchestrateur lors de l'injection
  d'échecs.

Ajoutez `--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` pour échouer
rapidement lorsque le payload reconstruit s'écarte du manifeste.

## 5. Étapes suivantes

- **Intégration gouvernance** – acheminer l'empreinte du manifeste et
  `manifest_signatures.json` dans le flux du conseil afin que le Pin Registry puisse
  annoncer la disponibilité.
- **Négociation du registre** – consultez [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  avant d'enregistrer de nouveaux profils. L'automatisation doit privilégier les handles
  canoniques (`namespace.name@semver`) plutôt que les ID numériques.
- **Automatisation CI** – ajoutez les commandes ci-dessus aux pipelines de release pour que
  la documentation, les fixtures et les artefacts publient des manifestes déterministes
  avec des métadonnées signées.
