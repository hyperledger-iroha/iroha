---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plan de prévisualisation avec somme de contrôle

Ce plan détaille le travail restant nécessaire pour que chaque artefact de prévisualisation du portail soit vérifiable avant la publication. L'objectif est de garantir que les réviseurs délivrent exactement l'instantané construit en CI, que le manifeste de somme de contrôle est inmuable et que la prévisualisation est détectable à travers SoraFS avec les métadonnées Norito.

## Objets- **Compilaciones deterministicas:** Asegurar que `npm run build` produit une sortie reproductible et toujours émise `build/checksums.sha256`.
- **Prévisualisations vérifiées :** Exiger que chaque artefact de prévisualisation comprenne un manifeste de somme de contrôle et demander la publication en cas d'erreur de vérification.
- **Métadonnées publiées avec Norito :** Conserver les descripteurs de prévisualisation (métdonnées de commit, résumé de somme de contrôle, CID de SoraFS) comme JSON de Norito pour que les outils de gouvernance puissent auditer les lancements.
- **Herramientas para operadores :** Prouvez un script de vérification d'une seule étape permettant aux consommateurs d'exécuter localement (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`) ; Le script maintenant enveloppe le flux de validation de la somme de contrôle + descripteur de point à point. La commande standard de prévisualisation (`npm run serve`) appelle maintenant cette aide automatiquement avant `docusaurus serve` pour que les instantaneas locales puissent être protégées en permanence par la somme de contrôle (avec `npm run serve:verified` gérée comme alias explicite).

## Phase 1 - Application en CI1. Actualiser `.github/workflows/docs-portal-preview.yml` para :
   - Exécuter `node docs/portal/scripts/write-checksums.mjs` après la build de Docusaurus (vous êtes appelé localement).
   - Exécuter `cd build && sha256sum -c checksums.sha256` et éviter le travail en cas d'écarts.
   - Empaqueter le répertoire build comme `artifacts/preview-site.tar.gz`, copier le manifeste de contrôle, exécuter `scripts/generate-preview-descriptor.mjs` et exécuter `scripts/sorafs-package-preview.sh` avec une configuration JSON (version `docs/examples/sorafs_preview_publish.json`) pour que le workflow émette tant de métadonnées comme un bundle SoraFS déterministe.
   - Subir le site statique, les artefacts de métadonnées (`docs-portal-preview`, `docs-portal-preview-metadata`) et le bundle SoraFS (`docs-portal-preview-sorafs`) pour que le manifeste, le CV CAR et le plan puissent être inspectés sans retourner à l'exécution de la construction.
2. Ajouter un commentaire avec l'insigne de CI qui reprend le résultat de la vérification de la somme de contrôle dans les demandes d'extraction (implémenté via l'étape de commentaire GitHub Script de `docs-portal-preview.yml`).
3. Documentez le flux de travail dans `docs/portal/README.md` (section CI) et suivez les étapes de vérification dans la liste de contrôle de publication.

## Script de vérification

`docs/portal/scripts/preview_verify.sh` valide les artefacts de prévisualisation téléchargés sans demander les instructions manuelles de `sha256sum`. Utilisez `npm run serve` (ou l'alias explicite `npm run serve:verified`) pour exécuter le script et lancer `docusaurus serve` dans une seule étape lorsque vous partagez des paramètres régionaux instantanés. La logique de vérification :1. Exécutez l'outil SHA approprié (`sha256sum` ou `shasum -a 256`) contre `build/checksums.sha256`.
2. Comparez éventuellement le résumé/numéro d'archive du descripteur de prévisualisation `checksums_manifest` et, lorsque vous indiquez, le résumé/numéro d'archive de l'archive de prévisualisation.
3. Sale avec un code distinct de zéro lorsqu'il détecte toute divergence pour que les réviseurs puissent bloquer les prévisualisations manipulées.

Exemple d'utilisation (après avoir extrait les artefacts de CI) :

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Les ingénieurs de CI et de release doivent exécuter le script toujours en téléchargeant un bundle de prévisualisation ou des artefacts complémentaires sur un ticket de release.

## Phase 2 - Publication en SoraFS

1. Extension du flux de travail de prévisualisation avec un travail tel que :
   - Suba el site construit al gateway de staging de SoraFS en utilisant `sorafs_cli car pack` et `manifest submit`.
   - Capturez le résumé du manifeste devuelto et le CID de SoraFS.
   - Serialice `{ commit, branch, checksum_manifest, cid }` et JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Enregistrez le descripteur avec l'artefact de build et exposez le CID dans le commentaire de la pull request.
3. Effectuez les tests d'intégration qui exécutent `sorafs_cli` en mode essai à sec pour garantir que les changements futurs maintiennent la cohérence du schéma de métadonnées.

## Phase 3 - Gobernanza et auditoria1. Publier un croquis Norito (`PreviewDescriptorV1`) qui décrit la structure du descripteur en `docs/portal/schemas/`.
2. Actualisez la liste de contrôle de publication DOCS-SORA pour demander :
   - Exécuter `sorafs_cli manifest verify` contre le CID chargé.
   - Enregistrer le résumé du manifeste de contrôle et le CID dans la description du PR de libération.
3. Connectez l'automatisation de gestion pour croiser le descripteur avec le manifeste de contrôle lors des votes de libération.

## Entregables et responsables

| Hito | Responsable(s) | Objet | Notes |
|------|------|----------|-------|
| Application de somme de contrôle en CI complète | Infrastructure de documents | Semaine 1 | Ajoutez une porte de chute et des chargements d'artefacts. |
| Publication de prévisualisations en SoraFS | Infrastructure de documents/Équipement de stockage | Semaine 2 | Nécessite l'accès aux informations d'identification de mise en scène et d'actualisation du modèle Norito. |
| Intégration de la gouvernance | Guide de Docs/DevRel / WG de Gobernanza | Semaine 3 | Publier l'esquema et actualiser les listes de contrôle et les entrées de la feuille de route. |

## Questions ouvertes- Que l'entorno de SoraFS doit-il alojar los artefactos de previsualizacion (staging vs. carril de prévisualisation dédié) ?
- Nous avons besoin d'entreprises doubles (Ed25519 + ML-DSA) dans le descripteur de prévisualisation avant la publication ?
- Debe el workflow de CI fijar la configuration del orquestador (`orchestrator_tuning.json`) à l'exécution `sorafs_cli` pour maintenir les manifestes reproductibles ?

Enregistrez les décisions en `docs/portal/docs/reference/publishing-checklist.md` et actualisez ce plan lorsque vous résolvez les choses.