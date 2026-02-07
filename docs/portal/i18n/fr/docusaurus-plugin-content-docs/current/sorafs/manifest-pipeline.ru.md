---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/manifest-pipeline.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Chanking SoraFS → Manifestations de pipeline

Ce matériel a été ajouté au démarrage rapide et vous propose la carte principale qui permet de préparer le démarrage.
Il s'agit du manifeste Norito, associé au registre Pin SoraFS. Текст адаптирован из
[`docs/source/sorafs/manifest_pipeline.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/manifest_pipeline.md);
обращайтесь к этому документу за канонической спецификацией и журналом изменений.

## 1. Contrôle de détermination

SoraFS utilise le profil SF-1 (`sorafs.sf1@1.0.0`) : roulements à billes, rapideCDC, avec
Taille mini de 64 Ko, taille maximale de 256 KiB, taille maximale de 512 KiB et taille maximale du masque
`0x0000ffff`. Profil d'enregistrement dans `sorafs_manifest::chunker_registry`.

### Aides Rust

- `sorafs_car::CarBuildPlan::single_file` – Découvrez les événements, les soirées et les soirées BLAKE3
  при подготовке метаданных CAR.
- `sorafs_car::ChunkStore` – Déchargez les charges utiles, connectez-vous aux métadonnées et à votre travail
  выборки Preuve de récupérabilité (PoR) 64 KiB / 4 KiB.
- `sorafs_chunker::chunk_bytes_with_digests` – Assistant de bibliothèque, disponible à la CLI.

### Instruments CLI

```bash
cargo run -p sorafs_chunker --bin sorafs-chunk-dump -- ./payload.bin \
  > chunk-plan.json
```

JSON prend en charge les événements, les dates et les dates. Planifiez votre travail
manifestes ou spécifications spécifiques pour l'opérateur.

### Vidéos PoR

`ChunkStore` précède `--por-proof=<chunk>:<segment>:<leaf>` et `--por-sample=<count>`,
Les auditeurs peuvent prendre des mesures auprès des médias. Сочетайте эти флаги с
`--por-proof-out` ou `--por-sample-out`, vous pouvez télécharger JSON.

## 2. Afficher le manifeste`ManifestBuilder` concerne les métadonnées liées à la gouvernance :

- Корневой CID (dag-cbor) и коммитменты CAR.
- Доказательства alias и claims возможностей провайдеров.
- Ajoutez des solutions et des métadonnées optionnelles (par exemple, les identifiants de build).

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub -- \
  ./payload.bin \
  --chunker-profile=sorafs.sf1@1.0.0 \
  --manifest-out=payload.manifest \
  --manifest-signatures-out=payload.manifest_signatures.json \
  --json-out=payload.report.json
```

Voici vos dates :

- `payload.manifest` – Norito-кодированные байты манифеста.
- `payload.report.json` – Eau pour les jeunes/automobiles, en passant par `chunk_fetch_specs`,
  `payload_digest_hex`, alias CAR et métadonnées.
- `payload.manifest_signatures.json` – Convertisseur, pour BLAKE3-дайджест манифеста,
  Le plan SHA3 est fourni avec les composants Ed25519.

Utilisez `--manifest-signatures-in` pour vérifier les conversions de tous les appareils avant
pré-commander, et `--chunker-profile-id` ou `--chunker-profile=<handle>` pour la fixation des prix
restaurant.

## 3. Publication et épinglage1. **Отправка в gouvernance** – Передайте дайджест манифеста и конверт подписей совету, чтобы
   pin мог быть принят. L'auditeur doit gérer le plan SHA3 pour les chaînes de production
   дайджестом манифеста.
2. **Charges utiles** – Enregistrez l'archive CAR (et l'index optionnel CAR), en cliquant sur
   manifeste, dans le registre des broches. En outre, le manifeste et la voiture utilisent le CID.
3. **Ajouter des données** – Sélectionnez l'option JSON, envoyez PoR et les principales mesures à récupérer dans
   релизных артефактах. Cela vous amènera à l'opérateur du bord et vous permettra de le faire.
   проблемы без загрузки больших charges utiles.

## 4. La simulation des fournisseurs non-professionnels

`cargo run -p sorafs_car --bin sorafs_fetch -- --plan=payload.report.json \
  --provider=alpha=providers/alpha.bin --provider=beta=providers/beta.bin#4@3 \
  --output=payload.bin --json-out=fetch_report.json`- `#<concurrency>` est mis en parallèle avec le fournisseur (`#4` est activé).
- `@<weight>` pour la planification des événements ; по умолчанию 1.
- `--max-peers=<n>` montre les fournisseurs, planifiant la batterie, maintenant
  обнаружение возвращает больше кандидатов, чем нужно.
- `--expect-payload-digest` et `--expect-payload-len` correspondent à ces porches.
- `--provider-advert=name=advert.to` vérifie les spécifications du fournisseur avant l'utilisation
  en simulation.
- `--retry-budget=<n>` prévient les ports de la montre (par exemple : 3), qui sont CI
  Vous avez vérifié la régression lors des tests d'achat.

`fetch_report.json` vous permet de mesurer les mesures d'agrégation (`chunk_retry_total`, `provider_failure_rate`,
et т. д.), подходящие для CI-asserтов и наблюдаемости.

## 5. Обновления реестра и gouvernance

Lors de la présentation d'un nouveau chunker de profil :

1. Ajoutez le descripteur dans `sorafs_manifest::chunker_registry_data`.
2. Ouvrez `docs/source/sorafs/chunker_registry.md` et votre carte.
3. Activez les fichiers (`export_vectors`) et indiquez les manifestes.
4. Ouvrez la voie à la charte de gouvernance.

L'automatisme permet ensuite de préparer les poignées canoniques (`namespace.name@semver`) et
возвращаться к числовым ID только при необходимости обратной совместимости.