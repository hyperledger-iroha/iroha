---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Planifier le contrôle avec la somme de contrôle

Ce plan prévoit un travail sur le robot, qui ne devrait pas être fait pour que tout l'art du projet avant le port puisse être vérifié. avant la publication. Il s'agit d'une garantie pour que la somme de contrôle manifeste soit manquante et que le paiement soit effectué ici. SoraFS avec les métadonnées Norito.

## Celi

- **Детерминированные сборки:** обеспечить, что `npm run build` дает воспроизводимый результат и всегда создает `build/checksums.sha256`.
- **Projets de simulation :** vous devez faire en sorte que l'objet de l'art soit inclus dans la somme de contrôle du manifeste et soit publié avant la validation. проверки.
- **Articles publiés à partir de Norito :** indique les descriptions du fournisseur (comités, somme de contrôle résumé, CID SoraFS) comme Norito JSON, les outils permettant de mettre en place des versions audio.
- **Instruments pour les opérateurs :** predostavitь одношаговый скрипт проверки, который потребитель могут запускать локально (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`) ; Le script contient la somme de contrôle + la description du montant. Le commutateur de commande standard (`npm run serve`) doit être automatiquement activé avant `docusaurus serve`, avec des images locales Définissez la somme de contrôle du contrôle (par exemple `npm run serve:verified` correspondant à votre pseudonyme).

## Partie 1 — Contrôle dans CI1. Téléchargez `.github/workflows/docs-portal-preview.yml` pour :
   - Lancez `node docs/portal/scripts/write-checksums.mjs` après la prise Docusaurus (à sélectionner localement).
   - Validez `cd build && sha256sum -c checksums.sha256` et validez le travail en cas de besoin.
   - Installez le répertoire build dans `artifacts/preview-site.tar.gz`, copiez la somme de contrôle du manifeste, sélectionnez `scripts/generate-preview-descriptor.mjs` et activez `scripts/sorafs-package-preview.sh`. Configuration JSON (par exemple `docs/examples/sorafs_preview_publish.json`), pour le flux de travail et les métadonnées, ainsi que pour la bande SoraFS.
   - Connectez-vous au site statistique, aux métadonnées des objets (`docs-portal-preview`, `docs-portal-preview-metadata`) et au bracelet SoraFS (`docs-portal-preview-sorafs`). Le manifeste, la société CAR et le plan peuvent être prouvés sans les dépenses publiques.
2. Ajoutez des commentaires sur CI-BI, en récupérant la somme de contrôle du résultat dans la demande d'extraction (réalisée à partir des commentaires du script GitHub dans `docs-portal-preview.yml`).
3. Téléchargez le flux de travail dans `docs/portal/README.md` (à l'adresse CI) et indiquez les résultats des vérifications dans les publications officielles.

## Скрипт проверки

`docs/portal/scripts/preview_verify.sh` fournit des objets d'art avant le projet `sha256sum`. Utilisez `npm run serve` (ou votre alias `npm run serve:verified`) pour télécharger le script et `docusaurus serve` pour votre projet de déploiement. локальных снимков. Les preuves logiques :1. Ouvrez l'outil SHA (`sha256sum` ou `shasum -a 256`) pour le projet `build/checksums.sha256`.
2. Lorsque vous avez besoin de lire digest/fichier description du produit `checksums_manifest` et, si vous l'utilisez, digest/fifa archivage pré-promotion.
3. Assurez-vous de ne pas avoir de problème lors de l'utilisation de l'appareil afin de pouvoir bloquer les mesures préventives.

Première utilisation (après l'utilisation des articles CI) :

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Les utilitaires CI et les versions de script vous permettent de télécharger des paquets avant ou d'acheter des articles en temps réel. billet.

## Partie 2 — Publication SoraFS

1. Optimisez le flux de travail avant de procéder :
   - Connectez-vous à l'étape de préparation SoraFS avec `sorafs_cli car pack` et `manifest submit`.
   - Votre manifeste de résumé et CID SoraFS.
   - Sérialisation `{ commit, branch, checksum_manifest, cid }` dans Norito JSON (`docs/portal/preview/preview_descriptor.json`).
2. Écrivez le descripteur correspondant à l'artéfact et créez le CID dans les commentaires de la demande d'extraction.
3. Effectuez des tests d'intégration en utilisant `sorafs_cli` dans le cadre d'un essai à sec pour déterminer la configuration actuelle. схемы метаданных.

## Partie 3 — Mise en route et audit1. Ouvrir le module Norito (`PreviewDescriptorV1`), en décrivant la description de la structure, dans `docs/portal/schemas/`.
2. Consultez les publications du spécialiste DOCS-SORA, en consultant :
   - Ouvrez `sorafs_cli manifest verify` pour le CID.
   - Corrigez la somme de contrôle du manifeste du résumé et le CID dans la description du PR fiable.
3. Effectuez la mise en œuvre automatique des descriptions de la somme de contrôle du manifeste lors de la mise en œuvre.

## Résultats et résultats

| ÉTAPE | Владелец(ы) | Цель | Première |
|------|------------|------|------------|
| Contrôle de la somme de contrôle dans CI внедрен | Documents sur l'infrastructure | Néerlandais 1 | Добавляет gate отказа и загрузку ARTEFACTOVS. |
| Publication des projets dans SoraFS | Documents d'infrastructure / Stockage de commandes | Négation 2 | Nous travaillons sur les dates de mise en scène et les programmes de mise à jour Norito. |
| Mise en œuvre de l'intégration | Lied Docs/DevRel / WG pour la mise en œuvre | Néerlandais 3 | Publier le schéma et mettre à jour les listes et la feuille de route. |

## Открытые вопросы

- Pourquoi la page SoraFS peut-elle créer des œuvres d'art avant le projet (mise en scène ou voie de prévisualisation actuelle) ?
- Avez-vous trouvé votre auteur (Ed25519 + ML-DSA) pour la description du fournisseur avant sa publication ?
- Vous avez utilisé le workflow CI pour configurer l'orchestrateur (`orchestrator_tuning.json`) en installant `sorafs_cli`, que souhaitez-vous utiliser pour les MANIFESTOS ?Téléchargez la solution `docs/portal/docs/reference/publishing-checklist.md` et consultez ce plan après une utilisation inappropriée.