---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry
titre : Registre des profils de chunker de SoraFS
sidebar_label : Registre du chunker
description : ID de profil, paramètres et plan de négociation pour le registre de chunker de SoraFS.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_registry.md`. Assurez-vous d'avoir des copies synchronisées jusqu'à ce que le ensemble de documents Sphinx héréditaire soit retiré.
:::

## Registre des profils de chunker de SoraFS (SF-2a)

La pile de SoraFS négocie le comportement de chunking au milieu d'un petit registre avec des espaces de nombres.
Chaque profil attribué aux paramètres CDC déterministes, métadonnées semver et le digest/multicodec attendu est utilisé dans les manifestes et les archives CAR.

Les auteurs de profils doivent consulter
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
pour les métadonnées requises, la liste de contrôle de validation et la plante de proposition avant d'envoyer de nouvelles entrées.
Une fois que la gouvernance a pris un changement, sigue el
[liste de contrôle du déploiement du registre](./chunker-registry-rollout-checklist.md) et le
[playbook de manifest en staging](./staging-manifest-playbook) pour promoteur
les luminaires, la mise en scène et la production.

### Profils| Espace de noms | Nombre | SemVer | ID de profil | Min (octets) | Cible (octets) | Max (octets) | Masque de coupe | Multihash | Alias ​​| Notes |
|-----------|--------|--------|-------------|-------------|----------------|-------------|-----------------|---------------|-------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Profil canonique utilisé dans les luminaires SF-1 |

L'enregistrement est présent dans le code `sorafs_manifest::chunker_registry` (géré par [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Chaque entrée
est exprimé comme un `ChunkerProfileDescriptor` avec :

* `namespace` – agrupación logique de perfiles relacionados (p. ej., `sorafs`).
* `name` – étiquette lisible para humanos (`sf1`, `sf1-fast`, …).
* `semver` – chaîne de versions sémantiques pour le ensemble de paramètres.
* `profile` – el `ChunkProfile` réel (min/cible/max/masque).
* `multihash_code` – le multihash est utilisé pour produire des résumés de chunk (`0x1f`
  pour la valeur par défaut de SoraFS).El manifeste serializa perfiles mediante `ChunkingProfileV1`. La structure enregistrée
les métadonnées du registre (espace de noms, nom, semestre) avec les paramètres CDC
en brut et la liste des alias affichés. Les consommateurs doivent tenter un
recherchez le registre par `profile_id` et répétez les paramètres en ligne lorsque
aparezcan IDs desconocidos; la liste des alias garantis que les clients HTTP peuvent utiliser
seguir enviando handles heredados en `Accept-Chunker` sin adivinar. Les verres de la
la carte du registre exige que la poignée canonique (`namespace.name@semver`) soit la
première entrée en `profile_aliases`, suivie par tout alias héritier.

Pour vérifier le registre des outils, lancez l'assistant CLI :

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

Tous les indicateurs de la CLI qui écrivent JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) acceptez `-` comme route, pour transmettre la charge utile vers la sortie standard à l'emplacement de
créer un fichier. Ceci facilite la canalisation des données d'outillage pendant la maintenance du
comportement par défaut d’impression du rapport principal.

### Matrice de déploiement et plan de déroulement


Le tableau suivant capture l'état actuel du support pour `sorafs.sf1@1.0.0` fr
composants principaux. "Bridge" se réfère au véhicule CARv1 + SHA-256
qui nécessite une négociation explicite du client (`Accept-Chunker` + `Accept-Digest`).| Composants | État | Notes |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ Supporté | Validez la poignée canonique + alias, transmettez les rapports via `--json-out=-` et appliquez la carte du registre avec `ensure_charter_compliance()`. |
| `sorafs_manifest_stub` | ⚠️ Retraité | Constructeur de manifeste hors support ; usa `iroha app sorafs toolkit pack` para empaquetado CAR/manifest y mantén `--plan=-` para revalidación determinista. |
| `sorafs_provider_advert_stub` | ⚠️ Retraité | Aide à la validation hors ligne uniquement ; Les annonces du fournisseur doivent être produites par le pipeline de publication et validées via `/v2/sorafs/providers`. |
| `sorafs_fetch` (orchestrateur développeur) | ✅ Supporté | Lee `chunk_fetch_specs`, contient des charges utiles de capacité `range` et un ensemble de sortie CARv2. |
| Montages du SDK (Rust/Go/TS) | ✅ Supporté | Régénérées via `export_vectors` ; Le manche canonique apparaît d’abord sur chaque liste d’alias et est confirmé par le conseiller. |
| Négociation de profils et passerelle Torii | ✅ Supporté | Implémentez la grammaire complète de `Accept-Chunker`, y compris les en-têtes `Content-Chunker` et exposez le pont CARv1 uniquement dans les demandes de rétrogradation explicites. |

Despligue de télémétrie:- **Télémétrie de récupération des fragments** — la CLI de Iroha `sorafs toolkit pack` émet des résumés de fragments, des métadonnées CAR et des sources PoR pour l'ingestion dans les tableaux de bord.
- **Annonces du fournisseur** — les charges utiles des annonces incluent des métadonnées de capacités et d'alias ; valida cobertura via `/v2/sorafs/providers` (p. ex., présence de la capacité `range`).
- **Surveillance de la passerelle** — les opérateurs doivent signaler les pareos `Content-Chunker`/`Content-Digest` pour détecter les déclassements indésirables ; j'espère que l'utilisation du pont sera à zéro avant la dépréciation.

Politique de dépréciation : une fois que vous avez ratifié un profil successeur, programmez une double fenêtre de publication
(documenté sur la propriété) avant de marquer `sorafs.sf1@1.0.0` comme obsolète dans l'enregistrement et l'élimination du
pont CARv1 des passerelles en production.

Pour inspecter un test de PoR spécifique, en fournissant des indices de chunk/segment/hoja et facultativement
persister la prueba a disco:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Vous pouvez sélectionner un profil par identifiant numérique (`--profile-id=1`) ou par poignée d'enregistrement
(`--profile=sorafs.sf1@1.0.0`); la forme avec handle est pratique pour les scripts que
passer un espace de noms/nom/semver directement à partir des métadonnées de gouvernance.

Utilisez `--promote-profile=<handle>` pour émettre un bloc de métadonnées JSON (y compris tous les alias
registrados) que vous pouvez utiliser en `chunker_registry_data.rs` pour promouvoir un nouveau profil par défaut :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```Le rapport principal (et l'archive d'essai facultative) inclut le résumé, les octets de la période actuelle
(codifiés en hexadécimal) et les résumés hermanos de segmento/chunk pour que les vérificateurs puissent répéter
la capacité de 64 KiB/4 KiB contre la valeur `por_root_hex`.

Pour valider une vérification existante contre une charge utile, passez la route par
`--por-proof-verify` (la CLI ajoute `"por_proof_verified": true` lorsque le test
coïncide avec la raison calculée) :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Pour la récolte en lot, utilisez `--por-sample=<count>` et fournissez éventuellement un itinéraire de graines/saison.
La CLI garantit un ordre déterminé (sembrado avec `splitmix64`) et un tronc de forme transparent lorsque
la sollicitude dépasse les heures disponibles :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

El manifest stub refleja los mismos datos, lo que es conveniente al automatizar la selección de
`--chunker-profile-id` en pipelines. Ambos CLIs de chunk store también aceptan la forma de handle canónico
(`--profile=sorafs.sf1@1.0.0`) para que los scripts de build puedan evitar hard-codear IDs numéricos:

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

El campo `handle` (`namespace.name@semver`) coincide con lo que aceptan los CLIs vía
`--profile=…`, por lo que es seguro copiarlo directamente a la automatización.

### Negociar chunkers

Gateways y clientes anuncian perfiles soportados vía provider adverts:

```
FournisseurAnnonceBodyV1 {
    ...
    chunk_profile : profile_id (implique via l'enregistrement)
    capacités : [...]
}
```

La programación de chunks multi-source se anuncia vía la capacidad `range`. El CLI la acepta con
`--capability=range[:streams]`, donde el sufijo numérico opcional codifica la concurrencia preferida
de fetch por rango del proveedor (por ejemplo, `--capability=range:64` anuncia un presupuesto de 64 streams).
Cuando se omite, los consumidores vuelven al hint general `max_streams` publicado en otra parte del advert.

Al solicitar datos CAR, los clientes deben enviar un header `Accept-Chunker` que liste tuplas
`(namespace, name, semver)` en orden de preferencia:

```Les passerelles sélectionnent mutuellement un profil pris en charge (par défaut `sorafs.sf1@1.0.0`)
et réfléchit à la décision via l'en-tête de réponse `Content-Chunker`. Los manifeste
intégrer le profil élégant pour que les nœuds en aval puissent valider la disposition des morceaux
ne dépend pas de la négociation HTTP.

### Support CAR

nous retendrons une route d'exportation CARv1+SHA-2 :

* **Ruta primaria** – CARv2, résumé de la charge utile BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, profil de morceau enregistré comme arrivé.
  PUEDEN expose cette variante lorsque le client omite `Accept-Chunker` ou sollicite
  `Accept-Digest: sha2-256`.

ajouts pour la transition mais il ne faut pas remplacer le résumé canonique.

### Conformité

* Le profil `sorafs.sf1@1.0.0` est attribué aux luminaires publics
  `fixtures/sorafs_chunker` et les sociétés enregistrées fr
  `fuzz/sorafs_chunker`. La parité de bout en bout s'exerce sur Rust, Go et Node
  mediante las pruebas provistas.
* `chunker_registry::lookup_by_profile` confirme les paramètres du descripteur
  coïncide avec `ChunkProfile::DEFAULT` pour éviter les divergences accidentelles.
* Les manifestes produits par `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` incluent les métadonnées du registre.