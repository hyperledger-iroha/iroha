<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/source/sorafs/manifest_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2572648c9c5aa1d4c346e66440fd14bff98afd55232ba1a7ba1c5fcd505559c6
source_last_modified: "2025-11-02T17:57:27.798590+00:00"
translation_last_reviewed: 2025-12-28
---

# Chunking SoraFS → Pipeline de manifeste

Cette note capture les étapes minimales nécessaires pour transformer un payload d'octets en
un manifeste encodé Norito adapté au pinning dans le registre SoraFS.

1. **Chunker le payload de manière déterministe**
   - Utilisez `sorafs_car::CarBuildPlan::single_file` (utilise en interne le chunker SF-1)
     pour dériver les offsets, longueurs et empreintes BLAKE3 des chunks.
   - Le plan expose l'empreinte du payload et les métadonnées de chunks que l'outillage downstream
     peut réutiliser pour l'assemblage CAR et la planification de Proof-of-Replication.
   - À défaut, le prototype `sorafs_car::ChunkStore` ingère des octets et
     enregistre des métadonnées de chunks déterministes pour une construction CAR ultérieure.
     Le store dérive désormais l'arbre d'échantillonnage PoR 64 KiB / 4 KiB (tagué par domaine,
     aligné sur les chunks) afin que les planificateurs puissent demander des preuves Merkle sans
     relire le payload.
    Utilisez `--por-proof=<chunk>:<segment>:<leaf>` pour émettre un témoin JSON pour une feuille
    échantillonnée et `--por-json-out` pour écrire le snapshot du digest racine pour une
    vérification ultérieure. Associez `--por-proof` à `--por-proof-out=path` pour persister le
    témoin, et utilisez `--por-proof-verify=path` pour confirmer qu'une preuve existante correspond
    au `por_root_hex` calculé pour le payload actuel. Pour plusieurs feuilles,
    `--por-sample=<count>` (avec `--por-sample-seed` et `--por-sample-out` optionnels) produit
    des échantillons déterministes tout en signalant `por_samples_truncated=true` lorsque la
    requête dépasse les feuilles disponibles.
   - Conservez les offsets/longueurs/empreintes des chunks si vous comptez construire des preuves
     de bundle (manifestes CAR, plannings PoR).
   - Reportez-vous à [`sorafs/chunker_registry.md`](chunker_registry.md) pour les entrées canoniques
     du registre et les conseils de négociation.

2. **Envelopper un manifeste**
   - Injectez les métadonnées de chunking, le CID racine, les engagements CAR, la politique de pin,
     les claims d'alias et les signatures de gouvernance dans `sorafs_manifest::ManifestBuilder`.
   - Appelez `ManifestV1::encode` pour obtenir les octets Norito et
     `ManifestV1::digest` pour obtenir le digest canonique enregistré dans le Pin
     Registry.

3. **Publier**
   - Soumettez le digest du manifeste via la gouvernance (signature du conseil, preuves d'alias)
     et pinnez les octets du manifeste dans SoraFS en utilisant le pipeline déterministe.
   - Assurez-vous que le fichier CAR (et l'index CAR optionnel) référencé par le manifeste est
     stocké dans le même ensemble de pins SoraFS.

### Démarrage rapide CLI

```bash
cargo run -p sorafs_manifest --bin sorafs-manifest-stub   ./docs.tar   --root-cid=0155aa   --car-cid=017112...   --alias-file=docs:sora:alias_proof.bin   --council-signature-file=0123...cafe:council.sig   --metadata=build:ci-123   --manifest-out=docs.manifest   --manifest-signatures-out=docs.manifest.signatures.json   --car-out=docs.car   --json-out=docs.report.json
```

La commande imprime les empreintes des chunks et les détails du manifeste ; lorsque
`--manifest-out` et/ou `--car-out` sont fournis, elle écrit le payload Norito et une archive
CARv2 conforme à la spécification (pragma + header + blocs CARv1 + index Multihash) sur disque.
Si vous passez un chemin de répertoire, l'outil le parcourt récursivement (ordre lexicographique),
chunke chaque fichier et émet un arbre dag-cbor raciné sur un répertoire dont le CID apparaît
comme racine du manifeste et du CAR. Le rapport JSON inclut l'empreinte calculée du payload CAR,
l'empreinte complète de l'archive, la taille, le CID brut et la racine (en séparant le codec
dag-cbor), ainsi que les entrées alias/metadata du manifeste. Utilisez `--root-cid`/`--dag-codec`
pour *vérifier* la racine ou le codec calculé lors des exécutions CI, `--car-digest` pour
imposer le hash du payload, `--car-cid` pour imposer un identifiant CAR brut pré-calculé
(CIDv1, codec `raw`, multihash BLAKE3) et `--json-out` pour persister le JSON imprimé avec
les artefacts manifeste/CAR pour l'automatisation downstream.

Lorsque `--manifest-signatures-out` est fourni (avec au moins un flag `--council-signature*`),
l'outil écrit également une enveloppe `manifest_signatures.json` contenant le digest BLAKE3 du
manifeste, le digest SHA3-256 agrégé du plan de chunks (offsets, longueurs et digests BLAKE3 de
chunks) et les signatures du conseil fournies. L'enveloppe enregistre désormais le profil de
chunker sous la forme canonique `namespace.name@semver`. L'automatisation downstream peut publier l'enveloppe dans les
journaux de gouvernance ou la distribuer avec les artefacts manifeste/CAR. Lorsque vous recevez
une enveloppe d'un signataire externe, ajoutez `--manifest-signatures-in=<path>` afin que la CLI
confirme les digests et vérifie chaque signature Ed25519 par rapport au digest du manifeste
fraîchement calculé.

Lorsque plusieurs profils de chunker sont enregistrés, vous pouvez en sélectionner un
explicitement via `--chunker-profile-id=<id>`. Le flag mappe aux identifiants numériques dans
[`chunker_registry`](chunker_registry.md) et garantit que la passe de chunking et le manifeste
émis référencent la même tuple `(namespace, name, semver)`. Préférez la forme canonique du handle
en automatisation (`--chunker-profile=sorafs.sf1@1.0.0`), ce qui évite de figer des IDs numériques.
Lancez `sorafs_manifest_chunk_store --list-profiles` pour voir les entrées actuelles du registre
(la sortie reflète la liste fournie par `sorafs_manifest_chunk_store`), ou utilisez
`--promote-profile=<handle>` pour exporter le handle canonique et les métadonnées d'alias lors
de la préparation d'une mise à jour du registre.

Les auditeurs peuvent demander l'arbre complet Proof-of-Retrievability via `--por-json-out=path`,
qui sérialise les digests chunk/segment/leaf pour la vérification d'échantillonnage. Les témoins
individuels peuvent être exportés avec `--por-proof=<chunk>:<segment>:<leaf>` (et validés avec
`--por-proof-verify=path`), tandis que `--por-sample=<count>` génère des échantillons déterministes
et dédupliqués pour des contrôles ponctuels.

Tout flag qui écrit du JSON (`--json-out`, `--chunk-fetch-plan-out`, `--por-json-out`, etc.) accepte
également `-` comme chemin, ce qui vous permet de streamer le payload directement vers stdout sans
créer de fichiers temporaires.

Utilisez `--chunk-fetch-plan-out=path` pour persister la spécification ordonnée de fetch de chunks
(index du chunk, offset du payload, longueur, digest BLAKE3) qui accompagne le plan du manifeste.
Les clients multi-sources peuvent alimenter le JSON résultant directement dans l'orchestrateur
de fetch SoraFS sans relire le payload source. Le rapport JSON imprimé par la CLI inclut également
ce tableau sous `chunk_fetch_specs`. La section `chunking` et l'objet `manifest` exposent
`profile_aliases` aux côtés du handle `profile` canonique afin que les SDK migrent depuis la forme

Lors d'une ré-exécution du stub (par exemple en CI ou dans un pipeline de release), vous pouvez
passer `--plan=chunk_fetch_specs.json` ou `--plan=-` pour importer la spécification précédemment
générée. La CLI vérifie que l'index, l'offset, la longueur et le digest BLAKE3 de chaque chunk
correspondent toujours au plan CAR fraîchement dérivé avant de poursuivre l'ingestion, ce qui
protège contre des plans obsolètes ou altérés.

### Smoke-test d'orchestration locale

Le crate `sorafs_car` fournit désormais `sorafs-fetch`, une CLI développeur qui consomme le tableau
`chunk_fetch_specs` et simule une récupération multi-fournisseurs à partir de fichiers locaux.
Pointez-la sur le JSON émis par `--chunk-fetch-plan-out`, fournissez un ou plusieurs chemins de
payload de fournisseurs (optionnellement avec `#N` pour augmenter la concurrence), et elle vérifiera
les chunks, réassemblera le payload et imprimera un rapport JSON résumant les compteurs de succès/
échec par fournisseur et les reçus par chunk :

```
cargo run -p sorafs_car --bin sorafs_fetch --   --plan=chunk_fetch_specs.json   --provider=alpha=./providers/alpha.bin   --provider=beta=./providers/beta.bin#4@3   --output=assembled.bin   --json-out=fetch_report.json   --provider-metrics-out=providers.json   --scoreboard-out=scoreboard.json
```

Utilisez ce flux pour valider le comportement de l'orchestrateur ou comparer des payloads de
fournisseurs avant de connecter des transports réseau réels au nœud SoraFS.

Lorsque vous devez cibler un gateway Torii live plutôt que des fichiers locaux, remplacez les
flags `--provider=/path` par les nouvelles options orientées HTTP :

```
sorafs-fetch   --plan=chunk_fetch_specs.json   --gateway-provider=name=gw-a,provider-id=<hex>,base-url=https://gw-a.example/,stream-token=<base64>   --gateway-manifest-id=<manifest_id_hex>   --gateway-chunker-handle=sorafs.sf1@1.0.0   --gateway-client-id=ci-orchestrator   --json-out=gateway_fetch_report.json
```

La CLI valide le stream token, impose l'alignement chunker/profil et enregistre les métadonnées
du gateway aux côtés des reçus habituels des fournisseurs afin que les opérateurs puissent archiver
le rapport comme preuve de rollout (voir le handbook de déploiement pour le flux blue/green complet).

Si vous passez `--provider-advert=name=/path/to/advert.to`, la CLI décode désormais l'enveloppe
Norito, vérifie la signature Ed25519 et impose que le fournisseur annonce la capacité
`chunk_range_fetch`. Cela maintient la simulation de fetch multi-sources alignée sur la politique
satisfaire des requêtes de chunk par plage.

Le suffixe `#N` augmente la limite de concurrence du fournisseur, tandis que `@W` définit le poids
de planification (par défaut 1 lorsqu'il est omis). Lorsque des adverts ou des descripteurs de
gateway sont fournis, la CLI évalue désormais le scoreboard de l'orchestrateur avant de lancer un
fetch : les fournisseurs éligibles héritent de poids basés sur la télémétrie et le snapshot JSON est
persisté via `--scoreboard-out=<path>` lorsqu'il est fourni. Les fournisseurs qui échouent aux
contrôles de capacité ou aux deadlines de gouvernance sont automatiquement supprimés avec un
avertissement afin que les exécutions restent alignées sur la politique d'admission. Voir
`docs/examples/sorafs_ci_sample/{telemetry.sample.json,scoreboard.json}` pour un exemple
d'entrée/sortie.

Passez `--expect-payload-digest=<hex>` et/ou `--expect-payload-len=<bytes>` pour affirmer que le
payload assemblé correspond aux attentes du manifeste avant d'écrire les sorties — pratique pour
les smoke-tests CI qui veulent s'assurer que l'orchestrateur n'a pas silencieusement supprimé ou
réordonné des chunks.

Si vous avez déjà le rapport JSON créé par `sorafs-manifest-stub`, passez-le directement via
`--manifest-report=docs.report.json`. La CLI de fetch réutilisera les champs intégrés
`chunk_fetch_specs`, `payload_digest_hex` et `payload_len`, vous évitant de gérer des fichiers de
plan ou de validation séparés.

Le rapport fetch expose également une télémétrie agrégée pour aider le monitoring :
`chunk_retry_total`, `chunk_retry_rate`, `chunk_attempt_total`, `chunk_attempt_average`,
`provider_success_total`, `provider_failure_total`, `provider_failure_rate` et
`provider_disabled_total` capturent la santé globale d'une session de fetch et conviennent aux
dashboards Grafana/Loki ou aux assertions CI. Utilisez `--provider-metrics-out` pour n'écrire que
le tableau `provider_reports` si l'outillage downstream n'a besoin que des statistiques par
fournisseur.

### Prochaines étapes

- Capturez les métadonnées CAR à côté des digests de manifeste dans les journaux de gouvernance afin
  que les observateurs puissent vérifier le contenu CAR sans re-télécharger le payload.
- Intégrez le flux de publication manifeste/CAR dans la CI afin que chaque build de docs/artefacts
  produise automatiquement un manifeste, obtienne des signatures et pinne les payloads résultants.
