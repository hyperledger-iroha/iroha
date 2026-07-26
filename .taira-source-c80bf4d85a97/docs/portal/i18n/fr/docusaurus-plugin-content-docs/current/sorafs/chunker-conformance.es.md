---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : conformité du chunker
titre : Guide de conformité du chunker de SoraFS
sidebar_label : conformité du chunker
description : Requisitos and flujos para preservar el perfil determinista de chunker SF1 en luminaires et SDK.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/chunker_conformance.md`. Gardez les versions synchronisées jusqu'à ce que les documents hérités soient retirés.
:::

Ce guide codifie les exigences que toute la mise en œuvre doit suivre pour le maintien
aligne avec le profil déterministe du chunker de SoraFS (SF1). Aussi
documenter le flux de régénération, la politique des entreprises et les étapes de vérification pour que
Les consommateurs de luminaires et les SDK peuvent être synchronisés en permanence.

## Profil canonique

- Poignée du profil : `sorafs.sf1@1.0.0` (alias heredado `sorafs.sf1@1.0.0`)
- Graine d'entrée (hex) : `0000000000dec0ded`
- Taille de l'objet : 262 144 octets (256 Ko)
- Taille minimale : 65 536 octets (64 Ko)
- Taille maximale : 524 288 octets (512 Ko)
- Polinomio de roulage: `0x3DA3358B4DC173`
- Seed de la tabla gear: `sorafs-v1-gear`
- Masque de rupture : `0x0000FFFF`

Mise en œuvre de la référence : `sorafs_chunker::chunk_bytes_with_digests_profile`.
Toute accélération SIMD doit produire des limites et des digestions identiques.

## Bundle de luminaires

`cargo run --locked -p sorafs_chunker --bin export_vectors` régénérer les
luminaires et émettent les archives suivantes sous `fixtures/sorafs_chunker/` :- `sf1_profile_v1.{json,rs,ts,go}` — Limites des morceaux canoniques pour les consommateurs
  Rust, TypeScript et Go. Chaque fichier annonce la poignée canonique comme la première
  entrada en `profile_aliases`, suivi par cualquier alias heredado (p. ej.,
  `sorafs.sf1@1.0.0`, puis `sorafs.sf1@1.0.0`). El orden se impose por
  `ensure_charter_compliance` et AUCUNE modification DEBE.
- `manifest_blake3.json` — manifeste vérifié avec BLAKE3 qui contient chaque fichier de luminaires.
- `manifest_signatures.json` — firmas del consejo (Ed25519) sur le résumé du manifeste.
- `sf1_profile_v1_backpressure.json` et corpora en brut à l'intérieur de `fuzz/` —
  Scénarios déterminants de streaming utilisés pour les essais de contre-pression du chunker.

### Politique de l'entreprise

La régénération des luminaires **doit** inclure une entreprise valide du conseil. Le générateur
rechaza la salida sin firmar a menos que se pase explícitamente `--allow-unsigned` (pensé
solo para experimentalación local). Les pages de l'entreprise sont en annexe uniquement et se
déduplication par entreprise.

Pour ajouter une entreprise de conseil :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Vérification

L'assistant de CI `ci/check_sorafs_fixtures.sh` lance le générateur avec
`--locked`. Si les luminaires divergent ou s'il y a une mauvaise entreprise, le travail échoue. États-Unis
ce script dans les flux de travail nocturnes et avant d'envoyer des changements de luminaires.

Manuel des étapes de vérification :

1. Projet `cargo test -p sorafs_chunker`.
2. Invoquez `ci/check_sorafs_fixtures.sh` localement.
3. Confirmez que `git status -- fixtures/sorafs_chunker` est propre.

## Playbook de mise à jourLorsque nous proposons un nouveau profil de chunker ou d'actualité SF1 :

Voir aussi : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) pour
conditions requises pour les métadonnées, les fiches de propriété et les listes de contrôle de validation.

1. Rédigez un `ChunkProfileUpgradeProposalV1` (voir RFC SF-1) avec de nouveaux paramètres.
2. Régénérez les appareils via `export_vectors` et enregistrez le nouveau résumé du manifeste.
3. Confirmer le manifeste avec le quórum du conseil requis. Toutes les entreprises doivent
   anexarse a `manifest_signatures.json`.
4. Actualisez les appareils affectés par le SDK (Rust/Go/TS) et assurez la parité cross-runtime.
5. Régénérez le corps du fuzz en modifiant les paramètres.
6. Actualisez ce guide avec la nouvelle poignée de profil, de graines et de digestion.
7. Envoyez le changement avec les essais et actualisations de la feuille de route.

Les changements qui affectent les limites du morceau ou des résumés sans suivre ce processus
son invalide et ne doit pas fusionner.