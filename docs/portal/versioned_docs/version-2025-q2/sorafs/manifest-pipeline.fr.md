---
lang: fr
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/manifest-pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e77b792e19fbfa8e1efeddd042adbe68a48287a582a1be76aa518af7830774e2
source_last_modified: "2026-01-04T10:50:53.604570+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Chunking → Pipeline de manifeste

Ce compagnon du démarrage rapide retrace le pipeline de bout en bout qui devient brut
octets dans les manifestes Norito adaptés au registre de broches SoraFS. Le contenu est
adapté de [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md) ;
consultez ce document pour la spécification canonique et le journal des modifications.

## 1. Morceau de manière déterministe

SoraFS utilise le profil SF-1 (`sorafs.sf1@1.0.0`) : un profil roulant inspiré de FastCDC
hachage avec une taille de fragment minimale de 64 Ko, une cible de 256 Ko, un maximum de 512 Ko et un
Masque de rupture `0x0000ffff`. Le profil est enregistré dans
`sorafs_manifest::chunker_registry`.

### Aides Rust

- `sorafs_car::CarBuildPlan::single_file` – Émet des décalages de fragments, des longueurs et
  BLAKE3 digère tout en préparant les métadonnées CAR.
- `sorafs_car::ChunkStore` : diffuse les charges utiles, conserve les métadonnées des fragments et
  dérive l'arbre d'échantillonnage de preuve de récupérabilité (PoR) de 64 Ko/4 Ko.
- `sorafs_chunker::chunk_bytes_with_digests` – Assistant de bibliothèque derrière les deux CLI.

### Outils CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Le JSON contient les décalages ordonnés, les longueurs et les résumés de morceaux. Persistez le
planifier lors de la construction de manifestes ou de spécifications de récupération de l'orchestrateur.

### Témoins PoR

`ChunkStore` expose `--por-proof=<chunk>:<segment>:<leaf>` et
`--por-sample=<count>` afin que les auditeurs puissent demander des ensembles de témoins déterministes. Paire
ces drapeaux avec `--por-proof-out` ou `--por-sample-out` pour enregistrer le JSON.

## 2. Enveloppez un manifeste

`ManifestBuilder` combine des métadonnées de fragments avec des pièces jointes de gouvernance :

- Engagements Root CID (dag-cbor) et CAR.
- Preuves d'alias et revendications de capacité du fournisseur.
- Signatures du conseil et métadonnées facultatives (par exemple, identifiants de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Résultats importants :

- `payload.manifest` – Octets manifestes codés en Norito.
- `payload.report.json` – Résumé lisible par l'homme/l'automatisation, comprenant
  `chunk_fetch_specs`, `payload_digest_hex`, résumés CAR et métadonnées d'alias.
- `payload.manifest_signatures.json` – Enveloppe contenant le manifeste BLAKE3
  résumé, résumé SHA3 de plan de fragments et signatures Ed25519 triées.

Utilisez `--manifest-signatures-in` pour vérifier les enveloppes fournies par des
signataires avant de les réécrire, et `--chunker-profile-id` ou
`--chunker-profile=<handle>` pour verrouiller la sélection du registre.

## 3. Publier et épingler

1. **Soumission de gouvernance** – Fournir le résumé du manifeste et la signature
   enveloppe au conseil pour que l'épinglette puisse être admise. Les auditeurs externes devraient
   stockez le résumé SHA3 du plan de fragments à côté du résumé du manifeste.
2. **Épingler les charges utiles** – Téléchargez l'archive CAR (et l'index CAR facultatif) référencée
   dans le manifeste du registre Pin. Assurez-vous que le manifeste et la CAR partagent le
   même CID racine.
3. **Enregistrer la télémétrie** – Conserver le rapport JSON, les témoins PoR et toute récupération
   métriques dans les artefacts de version. Ces enregistrements alimentent les tableaux de bord des opérateurs et
   aider à reproduire les problèmes sans télécharger de charges utiles volumineuses.

## 4. Simulation de récupération multi-fournisseurs

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` augmente le parallélisme par fournisseur (`#4` ci-dessus).
- `@<weight>` règle le biais de planification ; la valeur par défaut est 1.
- `--max-peers=<n>` limite le nombre de fournisseurs programmés pour une exécution lorsque
  la découverte donne plus de candidats que souhaité.
- `--expect-payload-digest` et `--expect-payload-len` protègent contre le silence
  la corruption.
- `--provider-advert=name=advert.to` verifies provider capabilities before
  en les utilisant dans la simulation.
- `--retry-budget=<n>` remplace le nombre de nouvelles tentatives par morceau (par défaut : 3), donc CI
  peut faire apparaître des régressions plus rapidement lors du test de scénarios de défaillance.

`fetch_report.json` fait apparaître des métriques agrégées (`chunk_retry_total`,
`provider_failure_rate`, etc.) adapté aux assertions CI et à l'observabilité.

## 5. Mises à jour et gouvernance du registre

Lors de la proposition de nouveaux profils de chunker :

1. Créez le descripteur dans `sorafs_manifest::chunker_registry_data`.
2. Mettre à jour `docs/source/sorafs/chunker_registry.md` et les chartes associées.
3. Régénérez les appareils (`export_vectors`) et capturez les manifestes signés.
4. Soumettre le rapport de conformité à la charte avec les signatures de gouvernance.

L'automatisation devrait préférer les handles canoniques (`namespace.name@semver`) et tomber