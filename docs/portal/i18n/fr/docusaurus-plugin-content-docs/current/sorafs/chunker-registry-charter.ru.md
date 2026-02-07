---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-charter.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : chunker-registry-charter
titre : Хартия реестра chunker SoraFS
sidebar_label : le chunker du restaurant Hart
description : Mise en place d'un chunker pour les modules et la mise à jour du profil.
---

:::note Канонический источник
Cette page correspond à `docs/source/sorafs/chunker_registry_charter.md`. Vous avez la possibilité de copier des copies de synchronisation, si vous êtes un star du Sphinx, vous ne pourrez pas vous en servir.
:::

# Mise en place du chunker de restauration SoraFS

> **Ратифицировано:** 2025-10-29 Panel sur les infrastructures du Parlement de Sora (см.
> `docs/source/sorafs/council_minutes_2025-10-29.md`). Les lubrifiants populaires
> la gouvernance formelle ; les commandes à utiliser pour lire ce document
> нормативным, пока не будет утверждена новая хартия.

Cette liste s'occupe du processus et du rôle pour l'évolution du chunker SoraFS.
Она дополняет [Руководство по авторингу профилей chunker](./chunker-profile-authoring.md), описывая, как новые
le profil est présenté, partagé, ratifié et dans ce cas-ci, vous êtes intéressé par l'exploitation.

## Область

La carte est prête à être téléchargée dans `sorafs_manifest::chunker_registry` et
Avec les outils de recherche, vous pouvez restaurer (CLI du manifeste, CLI de l'annonce du fournisseur,
SDK). En fixant l'alias et la poignée de l'investisseur, vérifiez
`chunker_registry::ensure_charter_compliance()` :

- Profil d'identification — положительные целые числа, монотонно возрастающие.
- Poignée canonique `namespace.name@semver` **должен** быть первой записью
- Les coups alias обрезаны, уникальны и не конфликтуют с каноническими handle других записей.## Roli

- **Автор(ы)** – obtenir la prévisualisation, la régénération des luminaires et la sauvegarde
  доказательства детерминизма.
- **Groupe de travail sur l'outillage (TWG)** – validation de la pré-production par l'utilisateur
  чеклистам и убеждается, что инварианты реестра соблюдены.
- **Conseil de gouvernance (GC)** – рассматривает отчет TWG, подписывает конверт предложения
  et annuler les publications/dépréciations.
- **Équipe de stockage** – réalisation de la restauration et publication
  обновления документации.

## Жизненный цикл

1. **Prévisions**
   - L'auteur vérifie la validation de la licence de l'auteur et de l'entreprise.
     JSON `ChunkerProfileProposalV1` dans
     `docs/source/sorafs/proposals/`.
   - Cliquez sur CLI pour accéder à :
     ```bash
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
     cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
       --promote-profile=<handle> --json-out=-
     cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
       --chunker-profile=<handle> --json-out=-
     ```
   - Ouvrir les relations publiques, les calendriers, les avant-premières, la recherche de détails et
     обновления реестра.

2. **Outillage complet (TWG)**
   - Validation du processus de validation (fixtures, fuzz, manifeste de pipeline/PoR).
   - Ouvrez `cargo test -p sorafs_car --chunker-registry` et indiquez-le.
     `ensure_charter_compliance()` propose un nouvel enregistrement.
   - Vérifier ce que propose la CLI (`--list-profiles`, `--promote-profile`, streaming
     `--json-out=-`) отражает обновленные alias et poignée.
   - Le statut de réussite/échec est obtenu.3. **Одобрение совета (GC)**
   - Renseignez-vous sur le TWG et les métadonnées.
   - Подписывает digest предложения (`blake3("sorafs-chunker-profile-v1" || bytes)`)
     et j'ajoute des modules de conversion en société, qui s'adaptent aux luminaires.
   - Fixez les résultats de la gouvernance dans le protocole de gouvernance.

4. **Publication**
   - Мержит PR, обновляя:
     - `sorafs_manifest::chunker_registry_data`.
     - Documentation (`chunker_registry.md`, руководства по авторингу/соответствию).
     - Luminaires et informations sur la détermination.
   - Répondez aux opérateurs et aux commandes du SDK pour le nouveau profil et le déploiement du plan.

5. **Депрекация / Закат**
   - Aperçu, présentation du profil de client, doivent être affichés lors de chaque publication
     (грейс-периоды) et plan de mise à niveau.
     lors de la restauration et de la mise en service du grand livre de migration.

6. **Détails supplémentaires**
   - La mise en place ou le correctif d'un correctif entraîne la suppression de la résolution du problème.
   - TWG doit documenter les risques liés à la surveillance et détecter les incidents quotidiens.

## Organisation de l'outillage

- `sorafs_manifest_chunk_store` et `sorafs_manifest_stub` pré-installés :
  - `--list-profiles` pour l'inspection du restaurant.
  - `--promote-profile=<handle>` pour la génération de blocs canoniques,
    utilisé pour le profil du producteur.
  - `--json-out=-` pour les sorties de courant sur la sortie standard, en fonction de la consommation d'eau
    logi ревью.
- `ensure_charter_compliance()` s'applique aux connecteurs binaires pertinents
  (`manifest_chunk_store`, `provider_advert_stub`). CI tests должны падать, если
  новые записи нарушают хартию.## Documentation

- Prenez toutes les mesures de détection dans `docs/source/sorafs/reports/`.
- Les protocoles de résolution du chunker sont disponibles dans
  `docs/source/sorafs/migration_ledger.md`.
- Vérifiez `roadmap.md` et `status.md` après la configuration du restaurant.

## Ссылки

- Nom de l'auteur : [Chunker de profil d'auteur] (./chunker-profile-authoring.md)
- Réponse du client : `docs/source/sorafs/chunker_conformance.md`
- Fichier de recherche : [Cunker de profil de profil] (./chunker-registry.md)