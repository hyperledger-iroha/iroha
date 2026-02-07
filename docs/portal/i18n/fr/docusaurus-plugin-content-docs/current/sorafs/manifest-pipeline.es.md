---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chunking de SoraFS → Pipeline des manifestes

Ce complément au démarrage permet d'enregistrer rapidement le pipeline d'extrême à extrême qui est converti
octets bruts dans les manifestes Norito adaptés au registre Pin de SoraFS. Le contenu est là
adapté de [`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
Consultez ce document pour les spécifications canoniques et le journal des modifications.

## 1. Fragmenter de forme déterministe

SoraFS utilise le profil SF-1 (`sorafs.sf1@1.0.0`) : un hash rodant inspiré par FastCDC avec un
taille minimale d'un morceau de 64 Ko, un objectif de 256 KiB, un maximum de 512 KiB et un masque
de corte `0x0000ffff`. Le profil est enregistré en `sorafs_manifest::chunker_registry`.

### Aides de Rust

- `sorafs_car::CarBuildPlan::single_file` – Emite offsets de chunks, longitudes y digests
  BLAKE3 entreprend de préparer les métadonnées de CAR.
- `sorafs_car::ChunkStore` – Charges utiles Streamea, conservation des métadonnées de morceaux et dérivée de l'arbre
  de muestreo Proof-of-Retrievability (PoR) de 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Assistant de bibliothèque pour les autres CLI.

### Outils de CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

Le JSON contient les décalages ordonnés, les longitudes et les résumés des morceaux. Garde
le plan pour la construction des manifestes ou des spécifications à récupérer de l'orquestador.

### Testigos PoR`ChunkStore` expose `--por-proof=<chunk>:<segment>:<leaf>` et `--por-sample=<count>` pour que
les auditeurs peuvent solliciter des ensembles de tests déterministes. Combinez ces drapeaux avec
`--por-proof-out` ou `--por-sample-out` pour enregistrer le JSON.

## 2. Envolver un manifeste

`ManifestBuilder` combine les métadonnées des morceaux avec les outils de gestion :

- CID raíz (dag-cbor) et compromissos de CAR.
- Pruebas de pseudonyme et réclamations de capacité des fournisseurs.
- Entreprises du conseil et métadonnées optionnelles (par exemple, ID de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Salaires importants :

- `payload.manifest` – Octets du manifeste codifiés en Norito.
- `payload.report.json` – CV lisible pour les humains/automatisation, inclus
  `chunk_fetch_specs`, `payload_digest_hex`, digère CAR et métadonnées d'alias.
- `payload.manifest_signatures.json` – Sur ce qui contient le résumé BLAKE3 du manifeste, le
  digérer SHA3 du plan de morceaux et des entreprises Ed25519 ordonnées.

Usa `--manifest-signatures-in` pour vérifier les informations fournies par les entreprises externes
avant de retourner à l'écriture, et `--chunker-profile-id` ou `--chunker-profile=<handle>` pour
fijar la sélection du registre.

## 3. Publique et pin1. **Envío a gobernanza** – Proportionna el digest del manifiesto y el sobre de firmas al
   consejo para que el pin pueda être admis. Les auditeurs externes doivent almacenar el
   digérer SHA3 du plan de morceaux avec le condensé du manifeste.
2. **Charges utiles Pinear** – Accédez au fichier CAR (et à l'indice CAR facultatif) référencé dans le
   manifeste au registre des broches. Assurez-vous que le manifeste et la RCA partagent le même CID.
3. **Registrar telemetría** – Conserver le rapport JSON, les tests PoR et tout ce qui est métrique
   de récupérer les artefacts de sortie. Ces enregistrements alimentent les tableaux de bord de
   opérateurs et aidant à reproduire des incidents sans télécharger de grandes charges utiles.

## 4. Simulation de récupération multi-fournisseurs

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` augmente le parallélisme du fournisseur (`#4` arrive).
- `@<weight>` ajuster le processus de planification ; par défaut, c'est 1.
- `--max-peers=<n>` limite le nombre de fournisseurs programmés pour une exécution lorsque le
  descubrimiento produit más candidats de los deseados.
- `--expect-payload-digest` et `--expect-payload-len` protègent contre la corruption silencieuse.
- `--provider-advert=name=advert.to` vérifier les capacités du fournisseur avant de l'utiliser
  dans la simulation.
- `--retry-budget=<n>` remplace le compte rendu de répétitions par morceau (par défaut : 3) pour cela
  CI peut exposer les régressions plus rapidement à l'épreuve des scénarios de chute.

`fetch_report.json` muestra métricas agregadas (`chunk_retry_total`, `provider_failure_rate`,
etc.) adéquats pour les vérifications de CI et d'observabilité.

## 5. Actualisations du registre et de la gouvernance

Les nouveaux profils du chunker proposent :

1. Rédiger le descripteur en `sorafs_manifest::chunker_registry_data`.
2. Actualiza `docs/source/sorafs/chunker_registry.md` y los charters relacionados.
3. Régénérez les luminaires (`export_vectors`) et capturez les manifestes firmados.
4. Envoyez les informations de remplissage de la charte avec les sociétés de gouvernement.

L'automatisation doit préférer les poignées canoniques (`namespace.name@semver`) et réapparaître avec les identifiants
les numéros sont seuls lorsqu'ils sont nécessaires au registre.