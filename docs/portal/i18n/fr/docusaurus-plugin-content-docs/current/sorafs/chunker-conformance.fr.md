---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: chunker-conformance
title: Guide de conformité du chunker SoraFS
sidebar_label: Conformité chunker
description: Exigences et workflows pour préserver le profil chunker SF1 déterministe dans les fixtures et SDKs.
---

:::note Source canonique
:::

Ce guide codifie les exigences que chaque implémentation doit suivre pour rester
alignée avec le profil chunker déterministe de SoraFS (SF1). Il documente aussi
le workflow de régénération, la politique de signatures et les étapes de vérification pour que
les consommateurs de fixtures dans les SDKs restent synchronisés.

## Profil canonique

- Seed d'entrée (hex) : `0000000000dec0ded`
- Taille cible : 262144 bytes (256 KiB)
- Taille minimum : 65536 bytes (64 KiB)
- Taille maximum : 524288 bytes (512 KiB)
- Polynôme de rolling : `0x3DA3358B4DC173`
- Seed de table gear : `sorafs-v1-gear`
- Masque de rupture : `0x0000FFFF`

Implémentation de référence : `sorafs_chunker::chunk_bytes_with_digests_profile`.
Toute accélération SIMD doit produire des limites et digests identiques.

## Bundle de fixtures

`cargo run --locked -p sorafs_chunker --bin export_vectors` régénère les
fixtures et émet les fichiers suivants sous `fixtures/sorafs_chunker/` :

- `sf1_profile_v1.{json,rs,ts,go}` — limites de chunks canoniques pour les
  consommateurs Rust, TypeScript et Go. Chaque fichier annonce le handle canonique
  `sorafs.sf1@1.0.0`, puis `sorafs.sf1@1.0.0`). L'ordre est imposé par
  `ensure_charter_compliance` et NE DOIT PAS être modifié.
- `manifest_blake3.json` — manifest vérifié BLAKE3 couvrant chaque fichier de fixtures.
- `manifest_signatures.json` — signatures du conseil (Ed25519) sur le digest du manifest.
- `sf1_profile_v1_backpressure.json` et corpora bruts dans `fuzz/` —
  scénarios de streaming déterministes utilisés par les tests de back-pressure du chunker.

### Politique de signature

La régénération des fixtures **doit** inclure une signature valide du conseil. Le générateur
rejette la sortie non signée sauf si `--allow-unsigned` est passé explicitement (prévu
uniquement pour l'expérimentation locale). Les enveloppes de signature sont append-only et
sont dédupliquées par signataire.

Pour ajouter une signature du conseil :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Vérification

Le helper CI `ci/check_sorafs_fixtures.sh` rejoue le générateur avec
`--locked`. Si les fixtures divergent ou si des signatures manquent, le job échoue. Utilisez
ce script dans les workflows de nuit et avant de soumettre des changements de fixtures.

Étapes de vérification manuelle :

1. Exécutez `cargo test -p sorafs_chunker`.
2. Lancez `ci/check_sorafs_fixtures.sh` localement.
3. Confirmez que `git status -- fixtures/sorafs_chunker` est propre.

## Playbook de mise à niveau

Lorsqu'on propose un nouveau profil chunker ou qu'on met à jour SF1 :

Voir aussi : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) pour les
exigences de métadonnées, les templates de proposition et les checklists de validation.

1. Rédigez un `ChunkProfileUpgradeProposalV1` (voir RFC SF-1) avec de nouveaux paramètres.
2. Régénérez les fixtures via `export_vectors` et consignez le nouveau digest du manifest.
3. Signez le manifest avec le quorum requis. Toutes les signatures doivent être
   appendues à `manifest_signatures.json`.
4. Mettez à jour les fixtures SDK concernées (Rust/Go/TS) et assurez la parité cross-runtime.
5. Régénérez les corpora fuzz si les paramètres changent.
6. Mettez à jour ce guide avec le nouveau handle de profil, les seeds et le digest.
7. Soumettez la modification avec des tests et des mises à jour du roadmap.

Les changements qui affectent les limites de chunk ou les digests sans suivre ce processus
sont invalides et ne doivent pas être fusionnés.
