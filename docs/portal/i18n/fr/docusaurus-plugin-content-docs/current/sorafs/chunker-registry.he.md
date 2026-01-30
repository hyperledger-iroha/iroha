---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/chunker-registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e9d973b40a3e9c218660566814b64e36e80ae235e37ddc14ea328123e8944a72
source_last_modified: "2025-11-14T04:43:21.540128+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_registry.md`. Gardez les deux copies synchronisées jusqu'à la retraite complète du set Sphinx hérité.
:::

## Registre des profils chunker SoraFS (SF-2a)

Le stack SoraFS négocie le comportement de chunking via un petit registre namespacé.
Chaque profil assigne des paramètres CDC déterministes, des métadonnées semver et le digest/multicodec attendu utilisé dans les manifests et les archives CAR.

Les auteurs de profils doivent consulter
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
pour les métadonnées requises, la checklist de validation et le modèle de proposition avant
de soumettre de nouvelles entrées. Une fois qu'une modification est approuvée par la
gouvernance, suivez la
[checklist de rollout du registre](./chunker-registry-rollout-checklist.md) et le
[playbook de manifest en staging](./staging-manifest-playbook) pour promouvoir
les fixtures vers staging et production.

### Profils

| Namespace | Nom | SemVer | ID de profil | Min (octets) | Cible (octets) | Max (octets) | Masque de rupture | Multihash | Alias | Notes |
|-----------|-----|--------|-------------|--------------|----------------|--------------|------------------|-----------|-------|-------|
| `sorafs`  | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Profil canonique utilisé dans les fixtures SF-1 |

Le registre vit dans le code sous `sorafs_manifest::chunker_registry` (régi par [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Chaque entrée
est exprimée comme un `ChunkerProfileDescriptor` avec :

* `namespace` – regroupement logique de profils liés (ex., `sorafs`).
* `name` – libellé lisible (`sf1`, `sf1-fast`, …).
* `semver` – chaîne de version sémantique pour le jeu de paramètres.
* `profile` – le `ChunkProfile` réel (min/target/max/mask).
* `multihash_code` – le multihash utilisé lors de la production des digests de chunks (`0x1f`
  pour le défaut SoraFS).

Le manifest sérialise les profils via `ChunkingProfileV1`. La structure enregistre les métadonnées
du registre (namespace, name, semver) aux côtés des paramètres CDC bruts et de la liste d'alias ci-dessus.
Les consommateurs doivent d'abord tenter une recherche dans le registre par `profile_id` et revenir aux
paramètres inline lorsque des IDs inconnus apparaissent ; la liste d'alias garantit que les clients HTTP
du registre exigent que le handle canonique (`namespace.name@semver`) soit la première entrée de
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

### Matrice de rollout et plan de déploiement


Le tableau ci-dessous capture le statut de support actuel pour `sorafs.sf1@1.0.0` dans les
composants principaux. "Bridge" désigne la voie CARv1 + SHA-256 qui
nécessite une négociation explicite côté client (`Accept-Chunker` + `Accept-Digest`).

| Composant | Statut | Notes |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Supporté | Valide le handle canonique + alias, streame les rapports via `--json-out=-` et applique la charte du registre via `ensure_charter_compliance()`. |
| `sorafs_fetch` (developer orchestrator) | ✅ Supporté | Lit `chunk_fetch_specs`, comprend les payloads de capacité `range` et assemble une sortie CARv2. |
| Fixtures SDK (Rust/Go/TS) | ✅ Supporté | Régénérées via `export_vectors` ; le handle canonique apparaît en premier dans chaque liste d'alias et est signé par des enveloppes du conseil. |
| Négociation de profils du gateway Torii | ✅ Supporté | Implémente toute la grammaire `Accept-Chunker`, inclut les headers `Content-Chunker` et n'expose le bridge CARv1 que sur des demandes de downgrade explicites. |

Déploiement de la télémétrie :

- **Télémétrie de fetch de chunks** — le CLI Iroha `sorafs toolkit pack` émet des digests de chunks, des métadonnées CAR et des racines PoR pour ingestion dans les dashboards.
- **Provider adverts** — les payloads d'adverts incluent des métadonnées de capacités et d'alias ; validez la couverture via `/v1/sorafs/providers` (ex., présence de la capacité `range`).
- **Surveillance gateway** — les opérateurs doivent rapporter les couplages `Content-Chunker`/`Content-Digest` pour détecter les downgrades inattendus ; l'usage du bridge est censé tendre vers zéro avant la dépréciation.

Politique de dépréciation : une fois qu'un profil successeur est ratifié, planifiez une fenêtre de double publication
(documentée dans la proposition) avant de marquer `sorafs.sf1@1.0.0` comme déprécié dans le registre et de retirer le
bridge CARv1 des gateways en production.

Pour inspecter un témoin PoR spécifique, fournissez des indices de chunk/segment/feuille et, en option,
persistez la preuve sur disque :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Vous pouvez sélectionner un profil par id numérique (`--profile-id=1`) ou par handle de registre
(`--profile=sorafs.sf1@1.0.0`) ; la forme handle est pratique pour les scripts qui
transmettent namespace/name/semver directement depuis les métadonnées de gouvernance.

Utilisez `--promote-profile=<handle>` pour émettre un bloc JSON de métadonnées (y compris tous les alias
enregistrés) qui peut être collé dans `chunker_registry_data.rs` lors de la promotion d'un nouveau profil
par défaut :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Le rapport principal (et le fichier de preuve optionnel) incluent le digest racine, les octets de feuille échantillonnés
(encodés en hexadécimal) et les digests frères de segment/chunk afin que les vérificateurs puissent rehacher
les couches de 64 KiB/4 KiB face à la valeur `por_root_hex`.

Pour valider une preuve existante contre un payload, passez le chemin via
`--por-proof-verify` (le CLI ajoute `"por_proof_verified": true` lorsque le témoin
correspond à la racine calculée) :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Pour l'échantillonnage par lots, utilisez `--por-sample=<count>` et fournissez éventuellement un chemin de seed/sortie.
Le CLI garantit un ordre déterministe (seedé avec `splitmix64`) et tronque automatiquement lorsque
la requête dépasse les feuilles disponibles :

```
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
    "profile_id": 1,
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "min_size": 65536,
    "target_size": 262144,
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
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (implicite via registre)
    capabilities: [...]
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
et reflètent la décision via le header de réponse `Content-Chunker`. Les manifests
intègrent le profil choisi afin que les nœuds downstream puissent valider le layout des chunks
sans s'appuyer sur la négociation HTTP.

### Support CAR

nous conservons une voie d'export CARv1+SHA-2 :

* **Chemin principal** – CARv2, digest de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, profil de chunk enregistré comme ci-dessus.
  PEUVENT exposer cette variante lorsque le client omet `Accept-Chunker` ou demande
  `Accept-Digest: sha2-256`.

supplémentaires pour la transition mais ne doivent pas remplacer le digest canonique.

### Conformité

* Le profil `sorafs.sf1@1.0.0` correspond aux fixtures publiques dans
  `fixtures/sorafs_chunker` et aux corpora enregistrés sous
  `fuzz/sorafs_chunker`. La parité de bout en bout est exercée en Rust, Go et Node
  via les tests fournis.
* `chunker_registry::lookup_by_profile` affirme que les paramètres du descripteur
  correspondent à `ChunkProfile::DEFAULT` pour éviter toute divergence accidentelle.
* Les manifests produits par `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` incluent les métadonnées du registre.
