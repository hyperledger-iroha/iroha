---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : conformité du chunker
titre : Recherche sur le chunker SoraFS
sidebar_label : chunker de réponse
description : Travaux et workflow pour la définition du profil chunker SF1 dans les appareils et les SDK.
---

:::note Канонический источник
:::

Ce document fournit un travail de résolution qui doit permettre de réaliser le calendrier, les choses
оставаться совместимой с детерминированным профилем chunker SoraFS (SF1). En fait
Documenter la régénération du flux de travail, les politiques de développement et les vérifications, les choses
Les appareils installés dans les SDK sont synchronisés.

## Profil canonique

- Graine actuelle (hex) : `0000000000dec0ded`
- Taille de la taille : 262 144 octets (256 Ko)
- Taille minimale : 65 536 octets (64 Ko)
- Taille maximale : 524 288 octets (512 Ko)
- Polynôme roulant : `0x3DA3358B4DC173`
- Engrenage des tables de semences : `sorafs-v1-gear`
- Masque de rupture : `0x0000FFFF`

Réalisation détaillée : `sorafs_chunker::chunk_bytes_with_digests_profile`.
L'application SIMD vous permet d'identifier facilement les grains et les résumés.

## Calendrier du Habor

`cargo run --locked -p sorafs_chunker --bin export_vectors` régénérateur
luminaires et photos de votre choix dans `fixtures/sorafs_chunker/` :- `sf1_profile_v1.{json,rs,ts,go}` — Chanfreins canoniques pour
  Utilisez Rust, TypeScript et Go. Le bouton de la poignée est canonique
  `sorafs.sf1@1.0.0`, par exemple `sorafs.sf1@1.0.0`). Порядок фиксируется
  `ensure_charter_compliance` et НЕ ДОЛЖЕН изменяться.
- `manifest_blake3.json` — Manifeste BLAKE3-верифицированный, покрывающий каждый файл luminaires.
- `manifest_signatures.json` — подписи совета (Ed25519) pour le résumé du manifeste.
- `sf1_profile_v1_backpressure.json` et corpus individuels dans `fuzz/` —
  Déterminez les scénarios de streaming en utilisant un chunker à contre-pression testé.

### Poste politique

La régénération des appareils **должна** включать валидную подпись совета. Générateur
Vous n'avez pas besoin de vous connecter, si vous n'êtes pas en contact avec `--allow-unsigned` (avant
только для локальных экспериментов). Les conversions peuvent être ajoutées uniquement et
дедуплицируются по подписанту.

Ce qu'il faut faire pour le thème :

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## Vérification

CI helper `ci/check_sorafs_fixtures.sh` générateur de démarrage automatique
`--locked`. Si les rencontres se déroulent ou si les résultats sont disponibles, le travail sera effectué. Utiliser
C'est un script pour les flux de travail nocturnes et avant l'ouverture des appareils.

Voici les preuves culinaires :

1. Sélectionnez `cargo test -p sorafs_chunker`.
2. Sélectionnez `ci/check_sorafs_fixtures.sh` localement.
3. Indiquez le numéro `git status -- fixtures/sorafs_chunker`.

## Плейбук обновления

Lors de la création d'un nouveau chunker de profil ou de la mise à jour de SF1 :См. comme : [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) pour
требований к метаданным, шаблонов предложений и чеклистов валидации.

1. Ajoutez `ChunkProfileUpgradeProposalV1` (avec RFC SF-1) avec de nouveaux paramètres.
2. Régénérez les appareils à partir de `export_vectors` et affichez le nouveau manifeste de résumé.
3. Подпишите манифест требуемым кворумом совета. Все подписи должны быть
   ajoutés au `manifest_signatures.json`.
4. Ouvrez les appareils du SDK (Rust/Go/TS) et sélectionnez le runtime correspondant.
5. Régénérez les corps de fuzz en modifiant les paramètres.
6. Sélectionnez ce nouveau profil de poignée, les graines et le digest.
7. Procédez à la planification des tests et des feuilles de route.

Изменения, которые затрагивают границы чанков или digests без соблюдения этого процесса,
Il n'y a pas d'acheteurs ni d'acheteurs.