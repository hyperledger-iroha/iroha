---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry
titre : Registre des profils chunker SoraFS
sidebar_label : chunker de registre
description : ID de profil, paramètres et plan de négociation pour le registre chunker SoraFS.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_registry.md`. Gardez les deux copies synchronisées jusqu'à la retraite complète du set Sphinx subsister.
:::

## Registre des profils chunker SoraFS (SF-2a)

La stack SoraFS négocie le comportement de chunking via un petit registre namespace.
Chaque profil attribue des paramètres CDC déterministes, des métadonnées semver et le digest/multicodec attendu utilisé dans les manifestes et les archives CAR.

Les auteurs de profils doivent consulter
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
pour les métadonnées requises, la checklist de validation et le modèle de proposition avant
de soumettre de nouvelles entrées. Une fois qu'une modification est approuvée par la
gouvernance, suivez la
[checklist de rollout du registre](./chunker-registry-rollout-checklist.md) et le
[playbook de manifest en staging](./staging-manifest-playbook) pour promouvoir
les montages vers la mise en scène et la production.

### Profils| Espace de noms | Nom | SemVer | ID de profil | Min (octets) | Cible (octets) | Max (octets) | Masque de rupture | Multihash | Alias ​​| Remarques |
|-----------|-----|--------|-------------|--------------|----------------|--------------|------------------|---------------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Profil canonique utilisé dans les luminaires SF-1 |

Le registre vit dans le code sous `sorafs_manifest::chunker_registry` (régi par [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Chaque entrée
est exprimée comme un `ChunkerProfileDescriptor` avec :

* `namespace` – regroupement logique de profils liés (ex., `sorafs`).
* `name` – libellé lisible (`sf1`, `sf1-fast`, …).
* `semver` – chaîne de version sémantique pour le jeu de paramètres.
* `profile` – le `ChunkProfile` réel (min/target/max/mask).
* `multihash_code` – le multihash utilisé lors de la production des digests de chunks (`0x1f`
  pour le défaut SoraFS).Le manifeste sérialise les profils via `ChunkingProfileV1`. La structure enregistre les métadonnées
du registre (namespace, name, semver) aux côtés des paramètres CDC bruts et de la liste d'alias ci-dessus.
Les consommateurs doivent d'abord tenter une recherche dans le registre par `profile_id` et revenir aux
paramètres en ligne lorsque des ID inconnus apparaissent ; la liste d'alias garantit que les clients HTTP
du registre exigeant que le handle canonique (`namespace.name@semver`) soit la première entrée de
`profile_aliases`, suivi des alias hérités.

Pour inspecter le registre depuis les outils, exécutez le CLI helper :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Tous les flags du CLI qui écrivent du JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) acceptent `-` comme chemin, ce qui streame le payload vers stdout au lieu de
créer un fichier. Cela facilite le piping des données vers les outils tout en conservant le
comportement par défaut d'imprimer le rapport principal.

### Matrice de déploiement et plan de déploiement


Le tableau ci-dessous capture le statut de support actuel pour `sorafs.sf1@1.0.0` dans les
composants principaux. "Bridge" désigne la voie CARv1 + SHA-256 ici
nécessite une négociation explicite côté client (`Accept-Chunker` + `Accept-Digest`).| Composant | Statuts | Remarques |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅Supporté | Validez le handle canonique + alias, streamez les rapports via `--json-out=-` et appliquez la charte du registre via `ensure_charter_compliance()`. |
| `sorafs_fetch` (orchestrateur développeur) | ✅Supporté | Lit `chunk_fetch_specs`, comprend les charges utiles de capacité `range` et assembler une sortie CARv2. |
| SDK de luminaires (Rust/Go/TS) | ✅Supporté | Régénérées via `export_vectors` ; le handle canonique apparaît en premier dans chaque liste d'alias et est signé par des enveloppes du conseil. |
| Négociation de profils du gateway Torii | ✅Supporté | Implémente toute la grammaire `Accept-Chunker`, inclut les en-têtes `Content-Chunker` et n'expose le pont CARv1 que sur des demandes de downgrade explicites. |

Déploiement de la télémétrie :- **Télémétrie de fetch de chunks** — le CLI Iroha `sorafs toolkit pack` émet des digests de chunks, des métadonnées CAR et des racines PoR pour l'ingestion dans les tableaux de bord.
- **Provider adverts** — les payloads d'adverts incluent des métadonnées de capacités et d'alias ; validez la couverture via `/v1/sorafs/providers` (ex., présence de la capacité `range`).
- **Surveillance gateway** — les opérateurs doivent rapporter les couplages `Content-Chunker`/`Content-Digest` pour détecter les downgrades inattendus ; l'usage du bridge est censé tendre vers zéro avant la dépréciation.

Politique de dépréciation : une fois qu'un profil successeur est ratifié, planifiez une fenêtre de double publication
(documentée dans la proposition) avant de marquer `sorafs.sf1@1.0.0` comme déprécié dans le registre et de retirer le
bridge CARv1 des passerelles en production.

Pour inspecter un témoin PoR spécifique, fournissez des indices de chunk/segment/feuille et, en option,
persistez la preuve sur disque :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Vous pouvez sélectionner un profil par identifiant numérique (`--profile-id=1`) ou par handle de registre
(`--profile=sorafs.sf1@1.0.0`) ; la forme handle est pratique pour les scripts qui
transmettent namespace/name/semver directement depuis les métadonnées de gouvernance.Utilisez `--promote-profile=<handle>` pour écrire un bloc JSON de métadonnées (y compris tous les alias
enregistré) qui peut être collé dans `chunker_registry_data.rs` lors de la promotion d'un nouveau profil
par défaut :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Le rapport principal (et le fichier de preuve optionnel) comprend le résumé racine, les octets de feuille échantillonnés
(encodés en hexadécimal) et les digests frères de segment/chunk afin que les vérificateurs puissent rehacher
les couches de 64 KiB/4 KiB face à la valeur `por_root_hex`.

Pour valider une preuve existante contre un payload, passez le chemin via
`--por-proof-verify` (le CLI ajoute `"por_proof_verified": true` lorsque le témoin
correspondent à la racine exploitée) :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Pour l'échantillonnage par lots, utilisez `--por-sample=<count>` et fournissez éventuellement un chemin de seed/sortie.
La CLI garantit un ordre déterministe (seedé avec `splitmix64`) et tronque automatiquement lorsque
la requête dépasse les feuilles disponibles :```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Le manifest stub reflète les mêmes données, ce qui est pratique pour scripter la sélection de
`--chunker-profile-id` dans les pipelines. Les deux CLIs de chunk store acceptent aussi la forme de handle canonique
(`--profile=sorafs.sf1@1.0.0`) afin que les scripts de build évitent de coder en dur des IDs numériques :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    "ID_profil": 1,
    "espace de noms": "sorafs",
    "nom": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "taille_cible": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

Le champ `handle` (`namespace.name@semver`) correspond à ce que les CLIs acceptent via
`--profile=…`, ce qui permet de le copier directement dans l'automatisation.

### Négocier les chunkers

Les gateways et les clients annoncent les profils supportés via des provider adverts :

```
FournisseurAnnonceBodyV1 {
    ...
    chunk_profile : profile_id (implicite via registre)
    capacités : [...]
}
```

La planification multi-source des chunks est annoncée via la capacité `range`. Le CLI l'accepte avec
`--capability=range[:streams]`, où le suffixe numérique optionnel encode la concurrence de fetch par range préférée
par le provider (par exemple, `--capability=range:64` annonce un budget de 64 streams).
Lorsqu'il est omis, les consommateurs reviennent à l'indication générale `max_streams` publiée ailleurs dans l'advert.

Lorsqu'ils demandent des données CAR, les clients doivent envoyer un header `Accept-Chunker` listant des tuples
`(namespace, name, semver)` par ordre de préférence :

```

Les gateways sélectionnent un profil supporté mutuellement (par défaut `sorafs.sf1@1.0.0`)
et prendre la décision via le header de réponse `Content-Chunker`. Les manifestes
intègre le profil choisi afin que les nœuds en aval puissent valider la disposition des chunks
sans s'appuyer sur la négociation HTTP.

### Soutenir la RCA

nous conservons une voie d'export CARv1+SHA-2 :

* **Chemin principal** – CARv2, résumé de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, profil de chunk enregistré comme ci-dessus.
  PEUVENT exposer cette variante lorsque le client omet `Accept-Chunker` ou demande
  `Accept-Digest: sha2-256`.

supplémentaires pour la transition mais ne doivent pas remplacer le digest canonique.

### Conformité* Le profil `sorafs.sf1@1.0.0` correspond aux luminaires publics dans
  `fixtures/sorafs_chunker` et aux corpus enregistrés sous
  `fuzz/sorafs_chunker`. La parité de bout en bout est exercée en Rust, Go et Node
  via les tests fournis.
* `chunker_registry::lookup_by_profile` affirme que les paramètres du descripteur
  correspondant à `ChunkProfile::DEFAULT` pour éviter toute divergence accidentelle.
* Les manifestes produits par `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` incluent les métadonnées du registre.