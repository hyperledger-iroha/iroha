---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Démarrage rapide du SoraFS

Ce guide pratique permet de découvrir le profil déterministe du chunker SF-1,
l'Assinatura de manifestes e o fluxo de busca multi-provedor que sustentam o
pipeline d'armement du SoraFS. Combiner-o avec o
[fusion profonde dans le pipeline de manifestes](manifest-pipeline.md)
pour les notes de conception et les références aux drapeaux de la CLI.

## Pré-requis

- Toolchain do Rust (`rustup update`), espace de travail cloné localement.
- Facultatif : [par de chaves Ed25519 compatible avec OpenSSL](https://github.com/hyperledger-iroha/iroha/tree/master/defaults/dev-keys#readme)
  manifestes para assinar.
- Facultatif : Node.js ≥ 18 si vous souhaitez pré-visualiser le portail Docusaurus.

Définissez `export RUST_LOG=info` pendant les tests pour exporter les messages utilisés par la CLI.

## 1. Actualiser les luminaires determinísticos

Nous avons récemment les anciens canoniques du chunking SF-1. Le commandant émet également
enveloppes de manifeste assinados quando `--signing-key` é fornecido; utiliser
`--allow-unsigned` ne fonctionne pas localement.

```bash
cargo run -p sorafs_chunker --bin export_vectors -- --allow-unsigned
```

Saïdas :

-`fixtures/sorafs_chunker/sf1_profile_v1.{json,rs,ts,go}`
-`fixtures/sorafs_chunker/manifest_blake3.json`
- `fixtures/sorafs_chunker/manifest_signatures.json` (assisté)
-`fuzz/sorafs_chunker/sf1_profile_v1_{input,backpressure}.json`

## 2. Fragmenter la charge utile et inspecter le plan

Utilisez `sorafs_chunker` pour fragmenter un fichier ou un fichier compacté arbitraire :

```bash
echo "SoraFS deterministic chunking" > /tmp/docs.txt
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- /tmp/docs.txt \
  > /tmp/docs.chunk-plan.json
```

Campos-chave :

- `profile` / `break_mask` – confirmer les paramètres de `sorafs.sf1@1.0.0`.
- `chunks[]` – compense les ordres, les compressions et les résumés de BLAKE3 dos chunks.

Pour les matchs majeurs, exécutez une régression avec le bon test pour garantir que le
chunking en streaming et en beaucoup permanent synchronisé:

```bash
cargo test -p sorafs_chunker streaming_backpressure_fuzz_matches_batch
```

## 3. Construisez et assine un manifeste

Empaqueter le plan de morceaux, les alias et les assassins de gouvernance dans un manifeste
en utilisant `sorafs-manifest-stub`. Le commandant abaixo montre une charge utile d'archive unique ; passer
un chemin de diretório pour emacotar un árvore (a CLI percorre em ordem lexicográfica).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  /tmp/docs.txt \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=/tmp/docs.manifest \
  --manifest-signatures-out=/tmp/docs.manifest_signatures.json \
  --json-out=/tmp/docs.report.json \
  --allow-unsigned
```

Réviser le paragraphe `/tmp/docs.report.json` :

- `chunking.chunk_digest_sha3_256` – résumé SHA3 des compensations/comprimements, correspondant également
  les luminaires font du chunker.
- `manifest.manifest_blake3` – digest BLAKE3 assassiné sans enveloppe du manifeste.
- `chunk_fetch_specs[]` – instructions de travail ordonnées pour les orchestres.

Quando estiver pronto para fornecer assinaturas reais, adicione os argumentos
`--signing-key` et `--signer`. Le commandement de vérifier chaque assassinat Ed25519 avant de graver
o enveloppe.

## 4. Simuler une récupération multi-fournisseurs

Utilisez une CLI de récupération de développement pour reproduire le plan de morceaux contre votre ou
mais des fournisseurs. C'est donc idéal pour les tests de fumée de CI et les prototypes d'explorateur.

```bash
cargo run -p sorafs_car --bin sorafs_fetch -- \
  --plan=/tmp/docs.report.json \
  --provider=primary=/tmp/docs.txt \
  --output=/tmp/docs.reassembled \
  --json-out=/tmp/docs.fetch-report.json
```

Vérifications :- `payload_digest_hex` doit être correspondant concernant le manifeste.
- `provider_reports[]` montre les contagènes de succès/falha par le fournisseur.
- `chunk_retry_total` différents de zéro destaca ajuste la contre-pression.
- Passe `--max-peers=<n>` pour limiter le nombre de fournisseurs programmés pour une exécution
  et il s'agit de simulations de CI focalisées sur nos candidats principaux.
- `--retry-budget=<n>` remplace le tampon de contamination par les tentatives du bloc (3) pour l'exportation
  la régression de l'orchestre est plus rapide jusqu'aux fausses injections.

Adicione `--expect-payload-digest=<hex>` et `--expect-payload-len=<bytes>` pour falhar
rapidement lorsque la charge utile a été reconstruite divergente du manifeste.

## 5. Nous passons à côté

- **Integração de gouvernance** – envie du résumé du manifeste et `manifest_signatures.json`
  pour le flux du conseil afin que le registre des broches puisse annoncer la disponibilité.
- **Négociation d'enregistrement** – consulter [`sorafs/chunker_registry.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/chunker_registry.md)
  antes de registrar novos perfis. L'automatisation doit préférer les identifiants canoniques
  (`namespace.name@semver`) à proximité des identifiants numériques.
- **Automation de CI** – ajouter les commandes ainsi que les pipelines de publication pour que
  documentation, agencements et artefatos publicsm manifestos determinísticos junto com
  métadados assassinés.