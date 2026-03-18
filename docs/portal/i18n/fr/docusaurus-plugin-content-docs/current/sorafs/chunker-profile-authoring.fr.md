---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : création de profil chunker
titre : Guide de création des profils chunker SoraFS
sidebar_label : Guide de création du chunker
description : Checklist pour proposer de nouveaux profils chunker SoraFS et des luminaires.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_profile_authoring.md`. Gardez les deux copies synchronisées jusqu'à la retraite complète du set Sphinx subsister.
:::

# Guide de création des profils chunker SoraFS

Ce guide explique comment proposer et publier de nouveaux profils chunker pour SoraFS.
Il complet le RFC d'architecture (SF-1) et la référence du registre (SF-2a)
avec des exigences de rédaction concrètes, des étapes de validation et des modèles de proposition.
Pour un exemple canonique, voir
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
et le log de dry-run associé dans
`docs/source/sorafs/reports/sf1_determinism.md`.

## Vue d'ensemble

Chaque profil qui entre dans le registre doit :

- annoncer des paramètres CDC déterministes et des réglages multihash identiques entre
  architectures ;
- fournir des luminaires rejouables (JSON Rust/Go/TS + corpora fuzz + témoins PoR) que
  les SDK en aval peuvent vérifier sans outillage sur mesure ;
- inclure des métadonnées prêtes pour la gouvernance (namespace, name, semver) ainsi que
  des conseils de déploiement et des fenêtres opérationnelles ; et
- passer la suite de diff déterministe avant la revue du conseil.Suivez la checklist ci-dessous pour préparer une proposition qui respecte ces règles.

## Aperçu de la charte du registre

Avant de rédiger une proposition, vérifier qu'elle respecte la charte du registre appliqué
par `sorafs_manifest::chunker_registry::ensure_charter_compliance()` :

- Les ID de profil sont des entiers positifs qui augmentent de façon monotone sans trous.
- Le handle canonique (`namespace.name@semver`) doit apparaître dans la liste d'alias
- Aucun alias ne peut entrer en collision avec une autre poignée canonique ni apparaître plus d'une fois.
- Les alias doivent être non vides et trimés des espaces.

Aides CLI utiles :

```bash
# Listing JSON de tous les descripteurs enregistrés (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Émettre des métadonnées pour un profil par défaut candidat (handle canonique + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

Ces commandes maintiennent les propositions alignées avec la charte du registre et de la fourniture
les métadonnées canoniques nécessaires aux discussions de gouvernance.

## Métadonnées requises| Champion | Descriptif | Exemple (`sorafs.sf1@1.0.0`) |
|-------|-------------|------------------------------|
| `namespace` | Regroupement logique de profils liés. | `sorafs` |
| `name` | Libellé Lisible. | `sf1` |
| `semver` | Chaîne de version sémantique pour l'ensemble de paramètres. | `1.0.0` |
| `profile_id` | Identifiant numérique monotone attribué une fois le profil intégré. Réservez l'id suivant mais ne réutilisez pas les numéros existants. | `1` |
| `profile.min_size` | Longueur minimale de chunk en octets. | `65536` |
| `profile.target_size` | Longueur cible du chunk en octets. | `262144` |
| `profile.max_size` | Longueur maximale du chunk en octets. | `524288` |
| `profile.break_mask` | Masque adaptatif utilisé par le Rolling Hash (hex). | `0x0000ffff` |
| `profile.polynomial` | Constante du polynôme engrenage (hex). | `0x3da3358b4dc173` |
| `gear_seed` | Seed utilisée pour dériver la table gear de 64 KiB. | `sorafs-v1-gear` |
| `chunk_multihash.code` | Code multihash pour les résumés par chunk. | `0x1f` (BLAKE3-256) |
| `chunk_multihash.digest` | Digest du bundle canonique de luminaires. | `13fa...c482` |
| `fixtures_root` | Répertoire relatif contenant les luminaires régénérées. | `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` | Seed pour l'échantillonnage PoR déterministe (`splitmix64`). | `0xfeedbeefcafebabe` (exemple) |Les métadonnées doivent apparaître à la fois dans le document de proposition et à l'intérieur des
luminaires innovants afin que le registre, le tooling CLI et l'automatisation de gouvernance puissent
confirmer les valeurs sans récupérations manuels. En cas de doute, exécutez les CLIs chunk-store et
manifest avec `--json-out=-` pour streamer les métadonnées recueillies dans les notes de revue.

### Points de contact CLI et registre

- `sorafs_manifest_chunk_store --profile=<handle>` — relancer les métadonnées de chunk,
  le digest du manifest et les checks PoR avec les paramètres proposés.
- `sorafs_manifest_chunk_store --json-out=-` — streamer le rapport chunk-store vers
  stdout pour des comparaisons automatisées.
- `sorafs_manifest_stub --chunker-profile=<handle>` — confirmer que les manifestes et les
  plans CAR embarquent le handle canonique et les alias.
- `sorafs_manifest_stub --plan=-` — réinjecter le `chunk_fetch_specs` précédent pour
  vérifier les offsets/digests après modification.

Consignez la sortie des commandes (digests, racines PoR, hashes de manifest) dans la proposition afin
que les reviewers puissent les reproduire mot pour mot.

## Checklist déterminisme et validation1. **Régénérer les luminaires**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **Exécuter la suite de parité** — `cargo test -p sorafs_chunker` et le harnais diff
   cross-lingual (`crates/sorafs_chunker/tests/vectors.rs`) doit être vert avec les
   nouveaux luminaires en place.
3. **Rejouer les corpora fuzz/back-pression** — exécutez `cargo fuzz list` et le harnais de
   streaming (`fuzz/sorafs_chunker`) contre les actifs régénérés.
4. **Vérifier les témoins Proof-of-Retrievability** — exécuter
   `sorafs_manifest_chunk_store --por-sample=<n>` avec le profil proposé et confirmez que les
   racines correspondant au manifeste de luminaires.
5. **Dry run CI** — invoquez `ci/check_sorafs_fixtures.sh` localement ; le script
   doit réussir avec les nouveaux luminaires et le `manifest_signatures.json` existant.
6. **Confirmation cross-runtime** — Assurez-vous que les liaisons Go/TS consomment le JSON
   régénéré et émettent des limites et des digestions identiques.

Documentez les commandes et les résumés résultants dans la proposition afin que le Tooling WG puisse
les rejouer sans conjecture.

### Manifeste de confirmation / PoR

Après régénération des luminaires, exécutez le pipeline manifeste complet pour garantir que les
métadonnées CAR et les preuves PoR restent cohérentes :

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

Remplacez le fichier d'entrée par un corpus représentatif utilisé par vos luminaires
(ex., le flux déterministe de 1 GiB) et joignez les résumés résultants à la proposition.

## Modèle de propositionLes propositions sont soumises sous forme de records Norito `ChunkerProfileProposalV1` déposées dans
`docs/source/sorafs/proposals/`. Le template JSON ci-dessous illustre la forme attendue
(remplacez par vos valeurs si nécessaire) :


Fournissez un rapport correspondant Markdown (`determinism_report`) qui capture la sortie des
, les résumés de chunk et toute divergence rencontrée lors de la validation des commandes.

## Flux de gouvernance

1. **Soumettre une PR avec proposition + luminaires.** Incluez les actifs générés, la
   proposition Norito et les mises à jour de `chunker_registry_data.rs`.
2. **Revue Tooling WG.** Les reviewers rejouent la checklist de validation et confirment
   que la proposition respecte les règles du registre (pas de réutilisation d'id,
   déterminisme satisfait).
3. **Enveloppe du conseil.** Une fois certifiée, les membres du conseil signent le digest
   de la proposition (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) et ajoute
   leurs signatures à l'enveloppe du profil stockée avec les luminaires.
4. **Publication du registre.** Le merge met à jour le registre, les docs et les luminaires.
   La CLI par défaut reste sur le profil précédent jusqu'à ce que la gouvernance déclare la
   migration prête.
5. **Suivi de dépréciation.** Après la fenêtre de migration, mettez à jour le registre pour
   de migration.

## Conseils de création- Préférez des bornes puissances de deux paires pour minimiser le comportement de chunking en bord.
- Évitez de changer le code multihash sans coordonner les consommateurs manifestes et gateway ;
  incluez une note opérationnelle lorsque vous le faites.
- Gardez les graines de table gear lisibles mais globalement uniques pour simplifier les audits.
- Stockez tout artefact de benchmarking (ex., comparaisons de débit) sous
  `docs/source/sorafs/reports/` pour référence future.

Pour les attentes opérationnelles pendant le déploiement, voir le grand livre de migration
(`docs/source/sorafs/migration_ledger.md`). Pour les règles de conformité runtime, voir
`docs/source/sorafs/chunker_conformance.md`.