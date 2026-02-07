---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry
titre : Registre de profils de chunker SoraFS
sidebar_label : registre de morceaux
description : Registre de chunker SoraFS avec identifiants de profil, paramètres et plan de négociation
---

:::note مستند ماخذ
:::

## SoraFS Registre de profils de chunker (SF-2a)

Comportement de segmentation de pile SoraFS pour un registre avec espace de noms et pour négocier des transactions
Paramètres CDC déterministes du profil, métadonnées Semver et attribution de résumés/multicodecs attendus et manifestes et archives CAR.

Auteurs du profil کو
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
Ajouter des métadonnées, une liste de contrôle de validation et un modèle de proposition pour que les entrées soient soumises.
جب gouvernance تبدیلی approuver کر دے تو
[liste de contrôle de déploiement du registre](./chunker-registry-rollout-checklist.md) اور
[Manuel de jeu du manifeste de mise en scène] (./staging-manifest-playbook) Les montages et la mise en scène et la production font la promotion des projets

### Profils

| Espace de noms | Nom | SemVer | Identifiant du profil | Min (octets) | Cible (octets) | Max (octets) | Briser le masque | Multihash | Alias ​​| Remarques |
|---------------|------|--------|------------|-------------|----------------|-------------|------------|---------------|---------|-------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (BLAKE3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 luminaires میں استعمال ہونے والا profil canonique |Le code de registre میں `sorafs_manifest::chunker_registry` کے طور پر موجود ہے (جسے [`chunker_registry_charter.md`](./chunker-registry-charter.md) gouverne کرتا ہے)۔ L'entrée `ChunkerProfileDescriptor` correspond à l'article suivant :

* `namespace` – Profils de groupe et regroupement logique (`sorafs`)
* `name` – Étiquette de profil lisible (`sf1`, `sf1-fast`, …)۔
* `semver` – jeu de paramètres کے ou chaîne de version sémantique۔
* `profile` – Voir `ChunkProfile` (min/cible/max/masque)۔
* `multihash_code` – résumés de fragments et multihash (`0x1f`
  SoraFS par défaut کے لیے)۔

Manifeste `ChunkingProfileV1` pour les profils et sérialiser les fichiers یہ métadonnées du registre de structure
(espace de noms, nom, semver) et les paramètres bruts du CDC ainsi que la liste d'alias et l'enregistrement d'un enregistrement.
Les consommateurs utilisent `profile_id` pour une recherche dans le registre et des identifiants inconnus avec des paramètres en ligne et des paramètres de secours en ligne.

Les outils de registre et inspectent les outils CLI d'assistance :

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

Les drapeaux CLI et les drapeaux JSON sont également disponibles (`--json-out`, `--por-json-out`, `--por-proof-out`,
`--por-sample-out`) chemin d'accès vers `-` chemin d'accès vers la sortie standard de charge utile vers flux et flux بنانے کے۔
Outils d'outillage, canal de données, rapport principal, comportement par défaut, rapport principal```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```
```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub یہی data mirror کرتا ہے، جو pipelines میں `--chunker-profile-id` selection کو script کرنے کے لیے convenient ہے۔ دونوں chunk store CLIs canonical handle form (`--profile=sorafs.sf1@1.0.0`) بھی accept کرتے ہیں تاکہ build scripts numeric IDs hard-code کرنے سے بچ سکیں:

```
```

`handle` field (`namespace.name@semver`) وہی ہے جو CLIs `--profile=…` کے ذریعے accept کرتے ہیں، اس لیے اسے automation میں براہ راست copy کرنا محفوظ ہے۔

### Negotiating chunkers

Gateways اور clients provider adverts کے ذریعے supported profiles advertise کرتے ہیں:

```
```

Multi-source chunk scheduling `range` capability کے ذریعے announce ہوتی ہے۔ CLI اسے `--capability=range[:streams]` کے ساتھ accept کرتا ہے، جہاں optional numeric suffix provider کی preferred range-fetch concurrency encode کرتا ہے (مثلاً `--capability=range:64` 64-stream budget advertise کرتا ہے)۔ جب یہ omit ہو تو consumers advert میں کہیں اور شائع شدہ general `max_streams` hint پر fallback کرتے ہیں۔

CAR data request کرتے وقت clients کو `Accept-Chunker` header بھیجنا چاہیے جو preference order میں `(namespace, name, semver)` tuples list کرے:

```

Profil mutuellement pris en charge par les passerelles. Manifestes avec profil intégré pour les nœuds en aval négociation HTTP pour la mise en page des blocs valider pour les utilisateurs



* **Chemin principal** – CARv2, résumé de la charge utile BLAKE3 (multihash `0x1f`)
  `MultihashIndexSorted`, un profil de bloc et un enregistrement enregistré


### Conformité

* `sorafs.sf1@1.0.0` profil appareils publics (`fixtures/sorafs_chunker`) et `fuzz/sorafs_chunker` کے تحت enregistrer des corpus سے match کرتا ہے۔ Parité de bout en bout Rust, Go et Node pour les tests et les exercices
* `chunker_registry::lookup_by_profile` affirme que les paramètres du descripteur `ChunkProfile::DEFAULT` correspondent à une divergence accidentelle ou à une divergence accidentelle.
* `iroha app sorafs toolkit pack` et `sorafs_manifest_stub` manifestent les métadonnées du registre en question.