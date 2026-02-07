---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: chunker-profile-authoring
כותרת: Guide de création des profils chunker SoraFS
sidebar_label: Guide de création chunker
תיאור: רשימת רשימת מסמכים ל-proposer de nouveaux profils chunker SoraFS et des fixtures.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/chunker_profile_authoring.md`. Gardez les deux copies Syncées jusqu'à la retraite complète du set Sphinx hérité.
:::

# Guide de création des profils chunker SoraFS

Ce guide explique comment proposer and publier de nouveaux profiles chunker pour SoraFS.
Il complète le RFC d'architecture (SF-1) et la référence du registre (SF-2a)
avec des exigences de rédaction concrètes, des étapes de validation et des modèles de proposition.
Pour un exemple canonique, voir
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
et le log de dry-run associé dans
`docs/source/sorafs/reports/sf1_determinism.md`.

## Vue d'ensemble

צ'ק פרופיל qui entre dans le registre doit :

- מודעות des paramètres CDC déterministes et des réglages multihash identiques entre
  ארכיטקטורות;
- fournir des fixtures rejouables (JSON Rust/Go/TS + corpora fuzz + témoins PoR) que
  les SDKs en aval peuvent vérifier sans tooling sur mesure;
- inclure des métadonnées prêtes pour la gouvernance (מרחב שם, שם, semver) ainsi que
  des conseils de rollout et des fenêtres opérationnelles; et
- passer la suite de diff déterministe avant la revue du conseil.

Suivez la checklist ci-dessous pour préparer une proposition qui respecte ces règles.

## Aperçu de la charte du registre

Avant de rédiger une proposition, vérifiez qu'elle respecte la charte du registre appliquée
ערך `sorafs_manifest::chunker_registry::ensure_charter_compliance()` :

- Les ID de profil sont des entiers positifs qui augmentent de façon monotone sans trous.
- Le handle canonique (`namespace.name@semver`) doit apparaître dans la list d'alias
- Aucun alias ne peut entrer en collision avec un autre handle canonique ni apparaître plus d'une fois.
- Les alias doivent être non vides et trimés des espaces.

עוזרי CLI שימושים:

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ces commandes maintiennent les propositions alignées avec la charte du registre et fournissent
les métadonnées canoniques nécessaires aux discusss de gouvernance.

## מתאדון דורש| אלוף | תיאור | דוגמה (`sorafs.sf1@1.0.0`) |
|-------|-------------|--------------------------------|
| `namespace` | Regroupement logique de profils liés. | `sorafs` |
| `name` | Libellé lisible. | `sf1` |
| `semver` | Chaîne de version sémantique pour l'ensemble de paramètres. | `1.0.0` |
| `profile_id` | תכונה מונוטונית מספרית מזהה ויחידה לפרופיל אינטéגré. Réservez l'id suivant mais ne réutilisez pas les numéros existants. | `1` |
| `profile.min_size` | Longueur minimale de chunk en bytes. | `65536` |
| `profile.target_size` | Longueur cible de chunk en bytes. | `262144` |
| `profile.max_size` | Longueur Maxime de chunk en bytes. | `524288` |
| `profile.break_mask` | Masque adaptatif utilisé par le rolling hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante du polynôme הילוך (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed utilisée pour dériver la table gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | קוד multihash pour les digests par chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest du bundle canonique de fixtures. | `13fa...c482` |
| `fixtures_root` | Repertoire relatif contenant les fixtures régénérées. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed pour l'échantillonnage PoR déterministe (`splitmix64`). | `0xfeedbeefcafebabe` (דוגמה) |

Les métadonnées doivent apparaître à la fois dans le document de proposition et à l'intérieur des
מתקנים générées afin que le registre, le tooling CLI et l'automatisation de governance puissent
הוראות אישור les valeurs sans recoupements. En cas de doute, exécutez les CLIs chunk-store et
manifest avec `--json-out=-` pour streamer les métadonnées calculées dans les notes de revue.

### נקודות ליצירת קשר עם CLI והרשמה

- `sorafs_manifest_chunk_store --profile=<handle>` - מפיץ את הנתח,
  le digest du manifest et les checks PoR avec les paramètres proposés.
- `sorafs_manifest_chunk_store --json-out=-` - streamer le rapport chunk-store vers
  stdout pour des comparisons automatisées.
- `sorafs_manifest_stub --chunker-profile=<handle>` — מאשר que les manifests et les
  מתכננת CAR embarquent le handle canonique et les aliases.
- `sorafs_manifest_stub --plan=-` — réinjecter le `chunk_fetch_specs` מקדימה לשפוך
  vérifier les offsets / digests après modification.

Consignez la sortie des commandes (עיכובים, racines PoR, hashes de manifest) dans la proposition afin
que les reviewers puissent les reproduire mot pour mot.

## רשימת רשימת דטרמיניזם ואימות1. **משחקי Regénérer les**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Exécuter la suite de parité** — `cargo test -p sorafs_chunker` et le harness diff
   חוצה שפות (`crates/sorafs_chunker/tests/vectors.rs`) doivent être verts avec les
   מתקנים חדשים במקום.
3. **Rejouer les corpora fuzz/back-pressure** — exécutez `cargo fuzz list` et le harness de
   סטרימינג (`fuzz/sorafs_chunker`) contre les assets régénérés.
4. **Vérifier les témoins הוכחה לשליפה** — exécutez
   `sorafs_manifest_chunk_store --por-sample=<n>` עם פרופיל הצעה ואישור que les
   כתב racines au manifest de fixtures.
5. **ריצה יבשה CI** - invoquez `ci/check_sorafs_fixtures.sh` localement; התסריט
   doit réussir avec les nouvelles fixtures et le `manifest_signatures.json` קיימים.
6. **זמן ריצה חוצה אישור** - assurez-vous que les bindings Go/TS consomment le JSON
   régénéré et émettent des limites et digests identiques.

Documentez les commandes et les digests résultants dans la proposition afin que le Tooling WG puisse
les rejouer sans השערה.

### מניפסט אישור / PoR

Après régénération des fixtures, exécutez le pipeline manifest complet pour garrant que les
מתודות CAR et les preuves PoR restent cohérentes:

```bash
# Valider les métadonnées chunk + PoR avec le nouveau profil
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# Générer manifest + CAR et capturer les chunk fetch specs
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# Relancer avec le plan de fetch sauvegardé (évite les offsets obsolètes)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

Remplacez le fichier d'entrée par un corpus représentatif utilisé par vos fixtures
(לדוגמה, le flux déterministe de 1 GiB) et joignez les digests résultants à la proposition.

## מודל ההצעה

Les propositions sont soumises sous forme de records Norito `ChunkerProfileProposalV1` déposés dans
`docs/source/sorafs/proposals/`. תבנית JSON ci-dessous illustre la forme attendue
(remplacez par vos valeurs si nécessaire):


כתב פורניסס עם מרקדאון (`determinism_report`) qui capture la sortie des
commandes, les digests de chunk et toute divergence rencontrée lors de la validation.

## Flux de Governance

1. **Soumettre une PR avec proposition + fixtures.** Incluez les assets générés, la
   הצעה Norito et les mises à jour de `chunker_registry_data.rs`.
2. **Revue Tooling WG.** הבודקים חוזרים על רשימת האימות והאישור
   que la proposition respecte les règles du registre (pas de réutilisation d'id,
   סיפוק דטרמיניזם).
3. **Enveloppe du conseil.** Une fois approuvée, les membres du conseil signent le digest
   de la proposition (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) et ajoutent
   חתימות leurs à l'enveloppe du profil stockée avec les fixtures.
4. **Publication du registre.** Le merge met à jour le registre, les docs et les fixtures.
   Le CLI par défaut reste sur le profil précédent jusqu'à ce que la governance déclare la
   פריט הגירה.
5. **Suivi de dépréciation.** Après la fenêtre de migration, mettez à jour le registre pour
   דה הגירה.

## Conseils de création- Préférez des bornes puissances de deux pairs pour minimiser le comportement de chunking en bord.
- Évitez de changer le code multihash sans coordonner les consommateurs manifest et gateway;
  incluez une note opérationnelle lorsque vous le faites.
- Gardez les seeds de table gear lisibles mais globalment uniques pour simplifier les audits.
- Stockez tout artefact de benchmarking (למשל, comparisons de débit) sos
  `docs/source/sorafs/reports/` עבור עתיד.

Pour les attentes operationnelles תליון להגשה, voir le Ledger de migration
(`docs/source/sorafs/migration_ledger.md`). Pour les règles de conformité זמן ריצה, voir
`docs/source/sorafs/chunker_conformance.md`.