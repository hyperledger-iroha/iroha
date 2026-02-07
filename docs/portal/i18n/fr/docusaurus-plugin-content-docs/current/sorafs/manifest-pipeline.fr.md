---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking SoraFS → Pipeline de manifeste

Ce complément au quickstart retrace le pipeline de bout en bout qui transforme des octets bruts
en manifeste Norito adapté au Pin Registry de SoraFS. Le contenu est adapté de
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Consultez ce document pour la spécification canonique et le changelog.

## 1. Chunker de manière déterministe

SoraFS utilise le profil SF-1 (`sorafs.sf1@1.0.0`) : un hash roulant inspiré de FastCDC avec
une taille minimale de chunk de 64 KiB, une cible de 256 KiB, un maximum de 512 KiB et un masque
de rupture `0x0000ffff`. Le profil est enregistré dans `sorafs_manifest::chunker_registry`.

### Aides Rouille

- `sorafs_car::CarBuildPlan::single_file` – Émet les décalages, longueurs et empreintes BLAKE3
  des chunks pendant la préparation des métadonnées CAR.
- `sorafs_car::ChunkStore` – Stream les payloads, persiste les métadonnées de chunks et dérive
  l'arbre d'échantillonnage Proof-of-Retrievability (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Helper de bibliothèque derrière les deux CLI.

### Outils CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Le JSON contient les offsets ordonnés, les longueurs et les empreintes des chunks. Conservez le
plan lors de la construction des manifestes ou des spécifications de fetch pour l'orchestrateur.

### Témoins PoR`ChunkStore` expose `--por-proof=<chunk>:<segment>:<leaf>` et `--por-sample=<count>` afin que les
les auditeurs peuvent demander des ensembles de témoins déterministes. Associez ces drapeaux à
`--por-proof-out` ou `--por-sample-out` pour enregistrer le JSON.

## 2. Envelopper un manifeste

`ManifestBuilder` combine les métadonnées de chunks avec des pièces de gouvernance :

- CID racine (dag-cbor) et engagements CAR.
- Preuves d'alias et déclarations de capacité des fournisseurs.
- Signatures du conseil et métadonnées optionnelles (par ex., IDs de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Sorties importantes :

- `payload.manifest` – Octets de manifeste encodés en Norito.
- `payload.report.json` – Résumé lisible pour humains/automatisation, incluant
  `chunk_fetch_specs`, `payload_digest_hex`, empreintes CAR et métadonnées d'alias.
- `payload.manifest_signatures.json` – Enveloppe contenant l'empreinte BLAKE3 du manifeste,
  l'empreinte SHA3 du plan de chunks et des signatures Ed25519 triées.

Utilisez `--manifest-signatures-in` pour vérifier les enveloppes fournies par des signataires
externes avant de les réécrire, et `--chunker-profile-id` ou `--chunker-profile=<handle>` pour
verrouiller la sélection du registre.

## 3. Publier et épingler1. **Soumission à la gouvernance** – Fournissez l'empreinte du manifeste et l'enveloppe de
   signatures au conseil pour que le pin puisse être admis. Les auditeurs externes doivent
   conserver l'empreinte SHA3 du plan de chunks avec l'empreinte du manifeste.
2. **Pinner les payloads** – Chargez l'archive CAR (et l'index CAR optionnel) référencé dans
   le manifeste vers le Pin Registry. Assurez-vous que le manifeste et le CAR partagent le
   même CID racine.
3. **Enregistrer la télémétrie** – Conservez le rapport JSON, les témoins PoR et toute métrique
   de fetch dans les artefacts de release. Ces enregistrements alimentent les tableaux de bord
   Les opérateurs et facilitent la reproduction des incidents sans télécharger de lourdes charges utiles.

## 4. Simulation de récupération multi-fournisseur

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` augmente le parallélisme par fournisseur (`#4` ci-dessus).
- `@<weight>` ajuster le biais d'ordonnancement ; la valeur par défaut est 1.
- `--max-peers=<n>` limite le nombre de fournisseurs planifiés pour une exécution lorsque la
  découverte renvoyer plus de candidats que souhaités.
- `--expect-payload-digest` et `--expect-payload-len` protègent contre la corruption silencieuse.
- `--provider-advert=name=advert.to` vérifier les capacités du fournisseur avant de l'utiliser
  dans la simulation.
- `--retry-budget=<n>` remplace le nombre de tentatives par chunk (par défaut : 3) afin que la
  CI révèle plus rapidement les régressions lors des scénarios de panne.

`fetch_report.json` expose des métriques agrégées (`chunk_retry_total`, `provider_failure_rate`,
etc.) adapté aux assertions CI et à l'observabilité.

## 5. Mises à jour du registre et gouvernance

Lors de la proposition de nouveaux profils de chunker :

1. Rédigez le descripteur dans `sorafs_manifest::chunker_registry_data`.
2. Mettez à jour `docs/source/sorafs/chunker_registry.md` et les chartes associées.
3. Régénérez les luminaires (`export_vectors`) et capturez les manifestes signés.
4. Soumettez le rapport de conformité de la charte avec des signatures de gouvernance.

L'automatisation doit privilégier les poignées canoniques (`namespace.name@semver`) et ne
revenir aux IDs numériques que lorsque c'est nécessaire pour le registre.