---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → Pipeline de manifestes

Ce complément au démarrage rapide accompagne le pipeline de pont à pont qui transforme les octets
brutos em manifestes Norito adéquats avec le registre Pin SoraFS. Le contenu a été adapté de
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Consultez ce document pour la spécification canonique et le journal des modifications.

## 1. Fazer chunking de forme déterministe

SoraFS utilise le profil SF-1 (`sorafs.sf1@1.0.0`) : un hachage continu inspiré par FastCDC com
tamanho minimum de chunk de 64 KiB, environ 256 KiB, maximum de 512 KiB et masque de quebra
`0x0000ffff`. Le profil est enregistré dans le numéro `sorafs_manifest::chunker_registry`.

### Aides dans Rust

- `sorafs_car::CarBuildPlan::single_file` – Emite offsets de chunks, comprimentos e digests
  BLAKE3 prépare actuellement les métadonnées de CAR.
- `sorafs_car::ChunkStore` – Faz streaming des charges utiles, conservation des métadonnées des morceaux et des dérivés
  il s'agit d'une preuve de récupération (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Aide à la bibliothèque via deux CLI.

### Ferramentas de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Le contenu JSON compense les ordres, les compressions et les résumés des morceaux. Préserver le plan ao
Construire des manifestes ou des spécifications pour aller chercher l'orquestrador.

### Testemunhas PoRO `ChunkStore` expose `--por-proof=<chunk>:<segment>:<leaf>` et `--por-sample=<count>` pour cela
les auditeurs peuvent solliciter des conjuntos de testemunhas determinísticos. Combiner ces drapeaux avec
`--por-proof-out` ou `--por-sample-out` pour registraire ou JSON.

## 2. Empacotar un manifeste

`ManifestBuilder` combine des métadonnées de morceaux avec des annexes de gouvernance :

- CID Raiz (Dag-Cbor) et compromissos de CAR.
- Provas de alias et allégations de capacité du fournisseur.
- Assinaturas do conselho e metadados opcionais (par exemple, ID de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Saídas importantes:

- `payload.manifest` – Octets du manifeste codifiés en Norito.
- `payload.report.json` – CV législatif pour les humains/l'automatisation, y compris `chunk_fetch_specs`,
  `payload_digest_hex`, résumés de CAR et métadonnées d'alias.
- `payload.manifest_signatures.json` – Enveloppe contenant le résumé BLAKE3 du manifeste, o
  digérer SHA3 à partir du plan de morceaux et des assimilations Ed25519 ordonnées.

Utilisez `--manifest-signatures-in` pour vérifier les enveloppes fournies par les signataires externes
avant de graver les nouveaux e `--chunker-profile-id` ou `--chunker-profile=<handle>` pour
fixer la sélection du registre.

## 3. Publier et épingler1. **Envio à Governorança** – Forneça o digest do manifesto et o enveloppe de assinaturas ao
   conseil pour que vous puissiez être admis. Les auditeurs externes doivent garder le résumé SHA3
   faire le plan des morceaux en même temps que le résumé du manifeste.
2. **Charges utiles Pinear** – Téléchargement de la façade de l'archive CAR (et de l'indice CAR facultatif) référencé non
   manifeste pour le registre des épingles. Garanta que le manifeste et le partage de la RCA avec le CID Raiz.
3. **Télémétrie du registraire** – Préserver le rapport JSON, comme teste PoR et quaisquer
   métriques de récupérer nos artefatos de release. Esses registros alimentam tableaux de bord de
   Les opérateurs peuvent résoudre des problèmes de reproduction avec de grandes charges utiles.

## 4. Simulation de récupération multi-fournisseurs

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` augmente le parallélisme du fournisseur (`#4` acima).
- `@<weight>` ajuster les vies de agendamento ; padrão est 1.
- `--max-peers=<n>` limite le nombre de fournisseurs agendas pour une exécution lors d'un
  descoberta retorna plus de candidats que le désiré.
- `--expect-payload-digest` et `--expect-payload-len` protègent contre la corruption silencieuse.
- `--provider-advert=name=advert.to` vérifie les capacités du fournisseur avant de l'utiliser
  simulation.
- `--retry-budget=<n>` remplace le virus tentatif par chunk (option : 3) pour le CI
  la régression est plus rapide jusqu'au test des scénarios de fraude.

`fetch_report.json` expõe métricas agregadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adéquates pour les affirmations de CI et d’observabilité.

## 5. Actualisations de l'enregistrement et de la gouvernance

Et pour les nouveaux résultats du chunker :

1. Écrivez le descripteur dans `sorafs_manifest::chunker_registry_data`.
2. Actualiser `docs/source/sorafs/chunker_registry.md` e comme charters relacionadas.
3. Régénérez les appareils (`export_vectors`) et capturez les manifestes assassinés.
4. Envie du rapport de conformité à la charte avec les entités de gouvernance.

L'automatisation doit privilégier les poignées canoniques (`namespace.name@semver`) et enregistrer les identifiants
Les numéros s'appliquent lorsqu'il est nécessaire de s'inscrire.