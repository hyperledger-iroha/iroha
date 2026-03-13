---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-registry.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-registry
כותרת: Registre des profils chunker SoraFS
sidebar_label: רשום chunker
תיאור: תעודות זהות, פרמטרים ותוכנית משא ומתן על נתח הרישום SoraFS.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/chunker_registry.md`. Gardez les deux copies Syncées jusqu'à la retraite complète du set Sphinx hérité.
:::

## Registre des profils chunker SoraFS (SF-2a)

מחסנית SoraFS נגישות לנתיחה באמצעות מרחב שמות רישום קטן.
Chaque profil assigne des paramètres CDC déterministes, des métadonnées semver et le digest/multicodec attendu utilisé dans les manifests et les archives CAR.

Les auteurs de profils doivent יועץ
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
pour les métadonnées requises, la checklist de validation et le modèle de proposition avant
de soumettre de nouvelles entrées. Une fois qu'une modification est approuvée par la
ממשל, suivez la
[Checklist de rollout du registre](./chunker-registry-rollout-checklist.md) et le
[playbook de manifest en staging](./staging-manifest-playbook) pour promouvoir
les fixtures vers staging and production.

### פרופילים

| מרחב שמות | נום | SemVer | ID de profil | דקות (אוקטטים) | Cible (אוקטטים) | מקסימום (אוקטטים) | מסקה דה קרע | Multihash | כינוי | הערות |
|-----------|-----|--------|------------------------------------------------------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | פרופיל canonique utilisé dans les fixtures SF-1 |

Le registre vit dans le code sous `sorafs_manifest::chunker_registry` (régi par [`chunker_registry_charter.md`](./chunker-registry-charter.md)). כניסת צ'אקה
est exprimée comme un `ChunkerProfileDescriptor` avec:

* `namespace` - לוגיקה מחדש של פרופילים (לדוגמה, `sorafs`).
* `name` – לשון הרע (`sf1`, `sf1-fast`, …).
* `semver` – chaîne de version sémantique pour le jeu de paramètres.
* `profile` – סליל `ChunkProfile` (מינימום/יעד/מקסימום/מסכה).
* `multihash_code` – le multihash utilisé lors de la production des digests de chunks (`0x1f`
  pour le défaut SoraFS).

המניפסט מתמחה בפרופילים באמצעות `ChunkingProfileV1`. La structure enregistre les métadonnées
du registre (מרחב שם, שם, semver) aux côtés des paramètres CDC bruts et de la list d'alias ci-dessus.
Les consommateurs doivent d'abord tenter une recherche dans le registre par `profile_id` et revenir aux
paramètres inline lorsque des IDs inconnus apparaissent; la list d'alias garantit que les clients HTTP
du registre exigent que le handle canonique (`namespace.name@semver`) soit la première entrée de
`profile_aliases`, suivi des alias hérités.

Pour inspecter le registre depuis les outils, exécutez le CLI עוזר:

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
```Tous les flags du CLI qui écrivent du JSON (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) acceptent `-` comme chemin, ce qui streame le payload vers stdout au lieu de
קריר ונפלא. Cela facilite le piping des données vers les outils tout en conservant le
comportement par défaut d'imprimer le rapport principal.

### פריסה ותכנון תכנון


Le tableau ci-dessous capture le statut de support actuel pour `sorafs.sf1@1.0.0` dans les
composants principaux. "גשר" עיצוב לה voie CARv1 + SHA-256 qui
nécessite une négociation לקוח מפורש (`Accept-Chunker` + `Accept-Digest`).

| קומפוזיטור | סטטוט | הערות |
|-----------|--------|-------|
| `sorafs_manifest_chunk_store` | ✅ תמיכה | תקף את הידית הקנונית + כינוי, זרימת קשרים דרך `--json-out=-` ואפליקציית תרשים רישום דרך `ensure_charter_compliance()`. |
| `sorafs_fetch` (מתזמר מפתח) | ✅ תמיכה | מואר `chunk_fetch_specs`, הגדר את המטענים של הקיבול `range` והרכב את המיון CARv2. |
| Fixtures SDK (Rust/Go/TS) | ✅ תמיכה | Régénérées via `export_vectors` ; le handle canonique apparaît en premier dans chaque list d'alias et est signné par des enveloppes du conseil. |
| Négociation de profils du gateway Torii | ✅ תמיכה | יש ליישם את הדקדוק `Accept-Chunker`, כולל כותרות `Content-Chunker` וחשיפה לגשר CARv1 que sur des demandes de degradation explicites. |

Déploiement de la télémétrie:

- **Télémétrie de fetch de chunks** — le CLI Iroha `sorafs toolkit pack` émet des digests de chunks, des métadonnées CAR et des racines PoR pour ingestion dans les לוחות מחוונים.
- **מודעות ספק** - les payloads d'adverts incluent des métadonnées de capacités et d'alias; validez la couverture דרך `/v2/sorafs/providers` (לדוגמה, presence de la capacité `range`).
- **שער מעקב** - המפעילים דואגים לדווח על ההצמדות `Content-Chunker`/`Content-Digest` pour dettecter les downgrades inattendus; l'usage du bridge est censé tendre versus zéro avant la dépréciation.

Politique de dépréciation : une fois qu'un profil successeur est ratifié, planifiez une fenêtre de פרסום כפול
(documentée dans la proposition) avant de marquer `sorafs.sf1@1.0.0` comme déprécié dans le registre et de retirer le
גשר CARv1 des gateways בהפקה.

Pour inspecter un témoin PoR spécifique, fournissez des indices de chunk/segment/feuille et, in option,
persistez la preuve sur disk :

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

Vous pouvez sélectionner un profil par id numérique (`--profile-id=1`) או par handle de registre
(`--profile=sorafs.sf1@1.0.0`); ידית הפורמה est pratique pour les scripts qui
מרחב שמות/שם/semver directement transmettent depuis les métadonnées de gouvernance.

Utilisez `--promote-profile=<handle>` pour émettre un bloc JSON de métadonnées (y compris tous les alias
נרשמים) qui peut être collé dans `chunker_registry_data.rs` lors de la promotion d'un nouveau profil
par défaut:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```Le Rapport Principal (et le fichier de preuve optionnel) כולל le digest racine, les octets de feuille échantillonnés
(encodés en hexadécimal) et les digests frères de segment/chunk afin que les vérificateurs puissent rehacher
les couches de 64 KiB/4 KiB face à la valeur `por_root_hex`.

Pour valider une preuve existante contre un payload, passz le chemin via
`--por-proof-verify` (le CLI ajoute `"por_proof_verified": true` lorsque le témoin
correspond à la racine calculée):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

Pour l'échantillonnage par lots, usez `--por-sample=<count>` et fournissez éventuellement un chemin de seed/sortie.
Le CLI garantit un order déterministe (seedé avec `splitmix64`) et tronque automatiquement lorsque
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
    "פרופיל_מזהה": 1,
    "namespace": "סורפים",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "גודל_מינימלי": 65536,
    "מטרת_גודל": 262144,
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
    chunk_profile: profile_id (משתמע דרך רישום)
    יכולות: [...]
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
et reflètent la décision via le header de réponse `Content-Chunker`. לס מתבטא
intègrent le profil choisi afin que les nœuds במורד הזרם puissent valider le layout des chunks
sans s'appuyer sur la négociation HTTP.

### תמיכה במכונית

nous conservons une voie d'export CARv1+SHA-2 :

* **Chemin main** – CARv2, digest de payload BLAKE3 (`0x1f` multihash),
  `MultihashIndexSorted`, פרופיל הנתח הרשום בסי-דיסוס.
  PEUVENT exposer cette variante lorsque le client omet `Accept-Chunker` ou demande
  `Accept-Digest: sha2-256`.

supplémentaires pour la transition mais ne doivent pas remplacer le digest canonique.

### קונפורמיות

* Le profil `sorafs.sf1@1.0.0` מתאים aux fixtures publiques dans
  `fixtures/sorafs_chunker` et aux corpora registrés sous
  `fuzz/sorafs_chunker`. La parité de bout en bout est exercée en Rust, Go et Node
  דרך les tests fournis.
* `chunker_registry::lookup_by_profile` מאשרים que les paramètres du descripteur
  כתב à `ChunkProfile::DEFAULT` pour éviter toute divergence accidentelle.
* מוצרי מניפסטים לפי `iroha app sorafs toolkit pack` ו-`sorafs_manifest_stub` כוללים את הרשמה.