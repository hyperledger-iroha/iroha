---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plan de contrôle de prévisualisation par somme de contrôle

Ce plan décrit le travail restant nécessaire pour rendre chaque artefact de prévisualisation du portail vérifiable avant publication. L'objectif est de garantir que les rélecteurs téléchargent exactement le snapshot construit en CI, que le manifeste de checksum est immuable et que la prévisualisation est découvrable via SoraFS avec des métadonnées Norito.

## Objectifs- **Builds déterministes:** S'assurer que `npm run build` produit une sortie reproductible et emet toujours `build/checksums.sha256`.
- **Prévisualisations vérifiées :** Exiger que chaque artefact de prévisualisation fournisse un manifeste de checksum et refuser la publication lorsque la vérification échoue.
- **Métadonnées publiées via Norito :** Persister les descripteurs de prévisualisation (métadonnées de commit, digest de checksum, CID SoraFS) en JSON Norito afin que les outils de gouvernance puissent auditer les releases.
- **Outillage Operator:** Fournir un script de vérification en une étape que les consommateurs peuvent exécuter localement (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); le script encapsule désormais le flux de validation checksum + descripteur de bout en bout. La commande de prévisualisation standard (`npm run serve`) invoque maintenant cet assistant automatiquement avant `docusaurus serve` afin que les instantanés locaux restent sous contrôle de somme de contrôle (avec `npm run serve:verified` conservés comme alias explicite).

## Phase 1 – Application CI1. Mettre à jour `.github/workflows/docs-portal-preview.yml` pour :
   - Executer `node docs/portal/scripts/write-checksums.mjs` après le build Docusaurus (deja invoque localement).
   - Executer `cd build && sha256sum -c checksums.sha256` et echouer le job en cas de divergence.
   - Empaqueter le répertoire build en `artifacts/preview-site.tar.gz`, copier le manifeste de checksum, exécuter `scripts/generate-preview-descriptor.mjs`, et exécuter `scripts/sorafs-package-preview.sh` avec une configuration JSON (voir `docs/examples/sorafs_preview_publish.json`) afin que le workflow emette à la fois les métadonnées et un bundle SoraFS déterministe.
   - Téléverser le site statique, les artefacts de métadonnées (`docs-portal-preview`, `docs-portal-preview-metadata`) et le bundle SoraFS (`docs-portal-preview-sorafs`) afin que le manifeste, le CV CAR et le plan puissent être inspectés sans relancer le build.
2. Ajoutez un commentaire de badge CI qui reprend le résultat de la vérification checksum dans les pull request (implémenté via l'étape de commentaire GitHub Script de `docs-portal-preview.yml`).
3. Documenter le workflow dans `docs/portal/README.md` (section CI) et lier les étapes de vérification dans la checklist de publication.

## Script de vérification

`docs/portal/scripts/preview_verify.sh` valide les artefacts de prévisualisation téléchargés sans exiger d'invocations manuelles de `sha256sum`. Utilisez `npm run serve` (ou l'alias explicite `npm run serve:verified`) pour exécuter le script et lancer `docusaurus serve` en une seule étape lorsque vous partagez des instantanés locaux. La logique de vérification :1. Exécutez l'outil SHA approprié (`sha256sum` ou `shasum -a 256`) contre `build/checksums.sha256`.
2. Comparez optionnellement le digest/nom de fichier du descripteur de prévisualisation `checksums_manifest` et, lorsque fourni, le digest/nom de fichier de l'archive de prévisualisation.
3. Trier avec un code non nul lorsqu'une divergence est détectée afin que les rélecteurs puissent bloquer les prévisualisations modifiées.

Exemple d'utilisation (après extraction des artefacts CI) :

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Les ingénieurs CI et release doivent appeler le script chaque fois qu'ils téléchargent un bundle de prévisualisation ou attachent des artefacts à un ticket de release.

## Phase 2 - Publication SoraFS

1. Étendre le workflow de prévisualisation avec un job qui :
   - Televerse le site construit vers la passerelle de staging SoraFS en utilisant `sorafs_cli car pack` et `manifest submit`.
   - Capturez le digest du manifeste renvoyé et le CID SoraFS.
   - Sérialiser `{ commit, branch, checksum_manifest, cid }` en JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Stocker le descripteur avec l'artefact de build et exposer le CID dans le commentaire de pull request.
3. Ajouter des tests d'intégration qui exercent `sorafs_cli` en mode dry-run afin d'assurer que les évolutions futures conservent la cohérence du schéma de métadonnées.

## Phase 3 - Gouvernance et audit1. Publier un schéma Norito (`PreviewDescriptorV1`) décrivant la structure du descripteur sous `docs/portal/schemas/`.
2. Mettre à jour la checklist de publication DOCS-SORA pour exiger :
   - Lancer `sorafs_cli manifest verify` sur charge CID.
   - Enregistrer le digest du manifeste de checksum et le CID dans la description de la PR de release.
3. Connecter l'automatisation de gouvernance pour croiser le descripteur avec le manifeste de checksum pendant les votes de release.

## Livrables et responsabilités

| Jalón | Propriétaire(s) | Câble | Remarques |
|-------|-------|-------|-------|
| Application des sommes de contrôle en CI livree | Documents sur les infrastructures | Semaine 1 | Ajout d'une porte de validation et des téléchargements d'artefacts. |
| Publication des avant-premières SoraFS | Documents d'infrastructure / Stockage Equipe | Semaine 2 | Nécessite l'accès aux identifiants de staging et des mises à jour du schéma Norito. |
| Gouvernance d'intégration | Responsable Docs/DevRel / GT Gouvernance | Semaine 3 | Publie le schéma et met à jour les checklists et les entrées du roadmap. |

## Questions ouvertes- Quel environnement SoraFS doit héberger les artefacts de prévisualisation (staging vs. lane de prévisualisation dédiée) ?
- Avons-nous besoin de doubles signatures (Ed25519 + ML-DSA) sur le descripteur de prévisualisation avant publication ?
- Le workflow CI doit-il épingler la configuration de l'orchestrateur (`orchestrator_tuning.json`) lors de l'exécution de `sorafs_cli` pour garder des manifestes reproductibles ?

Consignez les décisions dans `docs/portal/docs/reference/publishing-checklist.md` et mettez à jour ce plan une fois les inconnues résolues.