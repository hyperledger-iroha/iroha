---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS est là pour vous

یہ عملی رہنمائی SoraFS اسٹوریج پائپ لائن کی بنیاد بننے والے
déterministe SF-1 est un système de recherche en ligne et de recherche en ligne
récupérer فلو پر لے جاتی ہے۔ ڈیزائن نوٹس اور CLI فلیگ ریفرنس کے لیے اسے
[pipeline manifeste کی تفصیلی وضاحت](manifest-pipeline.md) کے ساتھ استعمال کریں۔

## ضروریات

- Rust ٹول چین (`rustup update`) ، اور workspace لوکل طور پر کلون ہو۔
- Nom : [Paire de clés OpenSSL et Ed25519] (https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  مینی فیسٹ سائن کرنے کے لیے۔
- Version : Node.js ≥ 18 ans Docusaurus correspond à la version actuelle.

`export RUST_LOG=info` سیٹ کریں تاکہ تجربات کے دوران مفید CLI پیغامات سامنے آئیں۔

## 1. luminaires déterministes تازہ کریں

SF-1 est un vecteur de regroupement canonique. جب `--signing-key` فراہم کیا
جائے تو یہ کمانڈ enveloppes manifestes signées بھی بناتی ہے؛ `--allow-unsigned` ici
مقامی ڈیولپمنٹ میں استعمال کریں۔

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

آؤٹ پٹ:

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (اگر سائن ہوا ہو)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. charge utile et bloc de charge et fragment de charge utile

`sorafs_chunker` est un morceau de morceau:

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

اہم فیلڈز:

- `profile` / `break_mask` – `sorafs.sf1@1.0.0` est en cours de réalisation.
- `chunks[]` – Ajout de décalages, de longueurs et de résumés BLAKE3 de fragments

Les matchs sont en cours de proptest et de régression en streaming et par lots
chunking ہم آہنگ رہے:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. manifeste بنائیں اور سائن کریں

plan de fragments, alias et signatures de gouvernance comme `sorafs-manifest-stub`
manifeste میں لپیٹیں۔ Il s'agit d'une charge utile à fichier unique. درخت پیک کرنے
کے لیے chemin du répertoire دیں (CLI اسے lexicographique ترتیب میں چلتا ہے)۔

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

`/tmp/docs.report.json` correspond à :

- `chunking.chunk_digest_sha3_256` – décalages/longueurs pour SHA3 digest, montages de chunker pour plus de détails
- `manifest.manifest_blake3` – enveloppe manifeste pour le résumé BLAKE3
- `chunk_fetch_specs[]` – orchestrateurs et récupérateurs de fichiers

Les signatures et les arguments `--signing-key` et `--signer` sont également disponibles.
کریں۔ Enveloppe enveloppante en forme de lettre Ed25519 signature en forme de lettre

## 4. Récupération multi-fournisseurs کی سمیولیشن

le développeur récupère la CLI et le plan de bloc ainsi que les fournisseurs et les replays یہ CI
tests de fumée et prototypage d'orchestrateur

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Caractéristiques :

- `payload_digest_hex` et manifeste رپورٹ سے ملنا چاہیے۔
- `provider_reports[]` pour le fournisseur et le nombre de réussites/échecs.
- Système de contre-pression `chunk_retry_total`.
- `--max-peers=<n>` exécutez des fournisseurs de services d'exécution et de fournisseur d'accès à CI
  simulations
- `--retry-budget=<n>` nombre de tentatives par fragment (3) pour remplacer les échecs par injection
  Voici les régressions de l'orchestrateur

`--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` شامل کریں تاکہ reconstruit
payload اگر manifeste سے ہٹے تو فوراً fail ہو جائے۔

## 5. اگلے اقدامات- **Intégration de la gouvernance** – résumé du manifeste اور `manifest_signatures.json` کو conseil
  flux de travail Détails sur la disponibilité du registre Pin Disponibilité du registre
- **Négociation de registre** – Profils de personnes رجسٹر کرنے سے پہلے
  [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  دیکھیں۔ Il existe des identifiants numériques et des poignées canoniques (`namespace.name@semver`).
  ترجیح دینی چاہیے۔
- **Automation CI** – Vous trouverez plus de détails sur les pipelines de publication et les documents de référence.
  luminaires, artefacts, métadonnées signées, manifestes déterministes, etc.