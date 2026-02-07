---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : conformité du chunker
titre : Guide de conformité du chunker SoraFS
sidebar_label : chunker de conformité
description : Exigences et workflows pour préserver le profil chunker SF1 déterministe dans les luminaires et SDK.
---

:::note Source canonique
:::

Ce guide codifie les exigences que chaque mise en œuvre doit suivre pour rester
aligner avec le profil chunker déterministe de SoraFS (SF1). Il documente aussi
le workflow de régénération, la politique de signatures et les étapes de vérification pour que
les consommateurs de luminaires dans les SDK restent synchronisés.

## Profil canonique

- Seed d'entrée (hex) : `0000000000dec0ded`
- Taille cible : 262144 octets (256 KiB)
- Taille minimale : 65536 octets (64 Ko)
- Taille maximale : 524288 octets (512 KiB)
- Polynôme de roulage : `0x3DA3358B4DC173`
- Engrenage de table de graine : `sorafs-v1-gear`
- Masque de rupture : `0x0000FFFF`

Implémentation de référence : `sorafs_chunker::chunk_bytes_with_digests_profile`.
Toute accélération SIMD doit produire des limites et des digestions identiques.

## Bundle de luminaires

`cargo run --locked -p sorafs_chunker --bin export_vectors` régénère les
luminaires et émettre les fichiers suivants sous `fixtures/sorafs_chunker/` :- `sf1_profile_v1.{json,rs,ts,go}` — limites de chunks canoniques pour les
  consommateurs Rust, TypeScript et Go. Chaque fichier annonce le handle canonique
  `sorafs.sf1@1.0.0`, puis `sorafs.sf1@1.0.0`). L'ordre est imposé par
  `ensure_charter_compliance` et NE DOIT PAS être modifié.
- `manifest_blake3.json` — manifeste vérifié BLAKE3 comprenant chaque fichier de luminaires.
- `manifest_signatures.json` — signatures du conseil (Ed25519) sur le digest du manifeste.
- `sf1_profile_v1_backpressure.json` et corpus bruts dans `fuzz/` —
  scénarios de streaming déterministes utilisés par les tests de contre-pression du chunker.

### Politique de signature

La régénération des luminaires **doit** inclut une signature valide du conseil. Le générateur
rejette la sortie non signée sauf si `--allow-unsigned` est passée précisée (prévu
uniquement pour l'expérimentation locale). Les enveloppes de signature sont en annexe seulement et
sont dédupliquées par signataire.

Pour ajouter une signature du conseil :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Vérification

Le helper CI `ci/check_sorafs_fixtures.sh` rejoue le générateur avec
`--locked`. Si les luminaires divergent ou si des signatures manquent, le travail échoue. Utiliser
ce script dans les workflows de nuit et avant de soumettre des changements de luminaires.

Étapes de vérification manuelle :

1. Exécutez `cargo test -p sorafs_chunker`.
2. Lancez `ci/check_sorafs_fixtures.sh` localement.
3. Confirmez que `git status -- fixtures/sorafs_chunker` est propre.

## Playbook de mise à niveauLorsqu'on propose un nouveau profil chunker ou qu'on met à jour SF1 :

Voir aussi : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) pour les
exigences de métadonnées, les modèles de proposition et les listes de contrôle de validation.

1. Rédigez un `ChunkProfileUpgradeProposalV1` (voir RFC SF-1) avec de nouveaux paramètres.
2. Régénérez les luminaires via `export_vectors` et consignez le nouveau résumé du manifeste.
3. Signez le manifeste avec le quorum requis. Toutes les signatures doivent être
   annexes à `manifest_signatures.json`.
4. Mettez à jour les luminaires SDK concernés (Rust/Go/TS) et assurez-vous de la parité cross-runtime.
5. Régénérez les corps fuzz si les paramètres changent.
6. Mettez à jour ce guide avec le nouveau handle de profil, les graines et le digest.
7. Soumettez la modification avec des tests et des mises à jour du roadmap.

Les changements qui impliquent les limites de chunk ou les digests sans suivre ce processus
sont invalides et ne doivent pas être fusionnés.