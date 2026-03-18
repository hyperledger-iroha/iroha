---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry
titre : Chunker de profil de fichier SoraFS
sidebar_label : Rechercher le chunker
description : profil d'identification, paramètres et paramètres de plan pour le chunker de restauration SoraFS.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/chunker_registry.md`. Vous avez la possibilité de copier des copies de synchronisation, si vous êtes un star du Sphinx, vous ne pourrez pas vous en servir.
:::

## Morceau de profil de fichier SoraFS (SF-2a)

Le modèle SoraFS permet de réaliser le chunking sans aucun espace de noms.
Le profil permet de déterminer les paramètres CDC, les métadonnées et les digests/multicodecs utilisés dans les manifestes et les archives CAR.

Les profils des auteurs s'intéressent à
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Pour la mise en œuvre des métadonnées, des vérifications de validation et des prévisions préalables
отправкой новых записей. Après la planification de la gouvernance des entreprises
[liste de déploiement de la liste](./chunker-registry-rollout-checklist.md) et
[Manifeste du playbook dans la mise en scène](./staging-manifest-playbook), à produire
montages dans la mise en scène et la production.

### Profils| Espace de noms | Je suis | SemVer | Profil d'identification | Мин (байты) | Цель (байты) | Макс (байты) | Masque coupé | Multihash | Alias ​​| Première |
|-----------|-----|--------|------------|-------------|--------------|--------------|---------------|---------------|--------|------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | Profil canonique utilisé dans les luminaires SF-1 |

Enregistrez-vous dans le code `sorafs_manifest::chunker_registry` (régularisé [`chunker_registry_charter.md`](./chunker-registry-charter.md)). Каждая запись
Vous devez utiliser `ChunkerProfileDescriptor` pour les polymères suivants :

* `namespace` – profil de groupe logique (par exemple, `sorafs`).
* `name` – profil de personne âgée (`sf1`, `sf1-fast`, …).
* `semver` – version sémantique pour les paramètres.
* `profile` – factuel `ChunkProfile` (min/cible/max/masque).
* `multihash_code` – multihash, utilisé pour votre digestion (`0x1f`
  pour SoraFS pour l'installation).Les profils de série du manifeste sont `ChunkingProfileV1`. La structure du restaurant métadonnée
(espace de noms, nom, semestre) est composé de vos paramètres CDC et de vos différents alias. Potrebiteli
Vous devez maintenant utiliser la recherche dans le fichier `profile_id` et utiliser les paramètres en ligne,
когда встречаются неизвестные ID; Les alias garantissent que les clients HTTP peuvent produire
Poignée canonique (`namespace.name@semver`) à mettre en place avant `profile_aliases`, pour votre maison

Pour pouvoir restaurer les outils, utilisez la CLI d'assistance :

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

Dans la CLI, vous trouverez JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`), en utilisant `-` dans la carte, qui définit la charge utile dans la sortie standard
создания файла. C'est une obligation d'investir dans l'outillage en fonction de la norme
поведения печати основного отчета.

### Déploiement de la matrice de développement et du plan


Le tableau indique l'état de votre téléphone en fonction du `sorafs.sf1@1.0.0` dans les principaux composants.
"Bridge" est connecté au canal soviétique CARv1 + SHA-256, qui est actuellement disponible
клиентской переговорной фазы (`Accept-Chunker` + `Accept-Digest`).| Composant | Statut | Première |
|---------------|--------|------------|
| `sorafs_manifest_chunk_store` | ✅ Подддерживается | Vous pouvez valider la poignée canonique + l'alias en vous connectant directement à `--json-out=-` et en premier le serveur de charte à partir de `ensure_charter_compliance()`. |
| `sorafs_fetch` (orchestrateur développeur) | ✅ Подддерживается | En utilisant `chunk_fetch_specs`, vous indiquez les charges utiles `range` et utilisez CARv2. |
| Montages SDK (Rust/Go/TS) | ✅ Подддерживается | Перегенерированы через `export_vectors` ; La poignée canonique est idéale pour les enveloppes du conseil municipal. |
| Profil de négociation dans la passerelle Torii | ✅ Подддерживается | Réalisez le grammaire `Accept-Chunker`, activez les paramètres `Content-Chunker` et ouvrez le pont CARv1 si vous souhaitez rétrograder. |

Развертывание телеметрии:

- **Les paramètres de récupération des paramètres** — CLI Iroha `sorafs toolkit pack` émettent des résumés de paramètres, des métadonnées CAR et des fichiers PoR pour l'ingestion dans les tableaux de bord.
- **Annonces du fournisseur** — les annonces de charges utiles incluent les capacités métadonnées et les alias ; Vérifiez la fonction `/v1/sorafs/providers` (par exemple, la capacité `range`).
- **Monitoring gateway** — Les opérateurs doivent choisir pour `Content-Chunker`/`Content-Digest`, pour éviter les rétrogradations inutiles ; ожидается, что использование bridge снизится до нуля до депрекации.La dépréciation politique : si vous renversez le profil-preemnik, planifiez-vous également les publications suivantes
Pont CARv1 vers les passerelles de production.

Pour vérifier le PoR concret, recherchez les indices chunk/segment/leaf et pour les détails
сохраните preuve на disque:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Vous pouvez créer un profil avec votre identifiant (`--profile-id=1`) ou avec la poignée de restauration
(`--profile=sorafs.sf1@1.0.0`); forme de poignée utilisée pour les scripts, les couleurs
Il s'agit d'un espace de noms/nom/semver pour les métadonnées de gouvernance.

Utilisez `--promote-profile=<handle>` pour votre bloc JSON (avec tous les
зарегистрированные aliasы), который можно вставить в `chunker_registry_data.rs` при
le nouveau profil de production pour l'utilisation :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (и необязательный файл proof) включает корневой digest, байты выбранного feuille
(en hexadécimal) et les résumés des frères et sœurs sont des segments/tâches, les vérificateurs peuvent utiliser les champs de hachage
64 KiB/4 KiB disponibles gratuitement `por_root_hex`.

Pour valider la preuve globale de la charge utile, avant de la mettre ici
`--por-proof-verify` (la CLI ajoute `"por_proof_verified": true`, ainsi que
совпадает с вычисленным корнем):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Pour l'emballage, utilisez le `--por-sample=<count>` et, si vous le souhaitez, placez la graine/la graine.
CLI garantit la détermination du produit (`splitmix64` ensemencé) et l'utilisation progressive des produits,
Voici la liste précédente des listes terminées :```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub отражает те же данные, что удобно при скриптинге выбора `--chunker-profile-id`
в пайплайнах. Оба chunk store CLI также принимают канонический формат handle
(`--profile=sorafs.sf1@1.0.0`), поэтому build-скрипты могут избежать жесткого
хардкода числовых ID:

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

Поле `handle` (`namespace.name@semver`) совпадает с тем, что CLIs принимают через
`--profile=…`, поэтому его можно безопасно копировать в автоматизацию.

### Согласование chunker

Gateways и клиенты объявляют поддерживаемые профили через provider adverts:

```
FournisseurAnnonceBodyV1 {
    ...
    chunk_profile : profile_id (pas disponible ici)
    capacités : [...]
}
```

Планирование multi-source чанков объявляется через capability `range`. CLI принимает ее с
`--capability=range[:streams]`, где опциональный числовой суффикс кодирует предпочтительную
параллельность range-fetch у провайдера (например, `--capability=range:64` объявляет бюджет 64 streams).
Когда суффикс отсутствует, потребители возвращаются к общему hint `max_streams`, опубликованному в другом
месте advert.

При запросе CAR-данных клиенты должны отправлять заголовок `Accept-Chunker`, перечисляя кортежи
`(namespace, name, semver)` в порядке предпочтения:

```

Les passerelles affichent un profil parfaitement adapté (en utilisant `sorafs.sf1@1.0.0`) et activent
résolution du problème actuel `Content-Chunker`. Manifestes встраивают выбранный профиль, чтобы
Les utilisateurs en aval peuvent valider les connexions sans ouvrir les ports HTTP.

### Совместимость VOITURE

Il s'agit de l'exportation CARv1+SHA-2 :

* **Prise en main** – CARv2, résumé de charge utile BLAKE3 (multihash `0x1f`),
  `MultihashIndexSorted`, le morceau de profil vous intéresse.
  Vous devez choisir cette variante lorsque votre client utilise `Accept-Chunker` ou l'installer.
  `Accept-Digest: sha2-256`.

Il n'est pas possible de lire un résumé canonique.

### Соответствие* Le profil `sorafs.sf1@1.0.0` concerne les luminaires publics
  `fixtures/sorafs_chunker` et corpus enregistrés dans
  `fuzz/sorafs_chunker`. Test de serveurs de bout en bout dans Rust, Go et Node
  через предоставленные тесты.
* `chunker_registry::lookup_by_profile` modifie la description des paramètres
  Совпадают с `ChunkProfile::DEFAULT`, чтобы защититься от случайной дивергенции.
* Manifestes, en utilisant `iroha app sorafs toolkit pack` et `sorafs_manifest_stub`, ainsi que la restauration des métadonnées.