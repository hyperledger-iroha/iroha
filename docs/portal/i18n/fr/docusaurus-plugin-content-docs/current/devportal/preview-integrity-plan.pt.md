---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Plan de pré-visualisation avec somme de contrôle

Ce plan décrit le travail restant nécessaire pour tornar cada artefato de pre-visualizacao do portal verificavel antes da publicacao. L'objectif est de garantir que les réviseurs abaissent exactement l'instantané construit dans CI, que le manifeste de somme de contrôle soit immuable et que la pré-visualisation soit découverte via SoraFS avec les métadonnées Norito.

## Objets

- **Builds deterministicas :** Garantir que `npm run build` produit une reproduction et semper gere `build/checksums.sha256`.
- **Pré-visualisations vérifiées :** Exiger que chaque artéfact de pré-visualisation comprenne un manifeste de somme de contrôle et récuser la publication en cas de vérification incorrecte.
- **Métadonnées publiées via Norito :** Persister les descriptions de pré-visualisation (métadonnées de commit, résumé de somme de contrôle, CID SoraFS) comme JSON Norito pour que les ferramentas de gouvernance puissent auditer les versions.
- **Ferramentas para operadores :** Fornecer um script de verificacao de um passo que les consommateurs peuvent exécuter localement (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`) ; Le script agora implique le flux de validation de la somme de contrôle + le descripteur de pont à pont. La commande de pré-visualisation (`npm run serve`) appelle maintenant automatiquement cette aide avant `docusaurus serve` pour que les instantanés locaux soient protégés en permanence par la somme de contrôle (avec `npm run serve:verified` comme alias explicite).## Phase 1 - Application sur CI

1. Actualiser `.github/workflows/docs-portal-preview.yml` para :
   - Exécuter `node docs/portal/scripts/write-checksums.mjs` après la construction de Docusaurus (et invoqué localement).
   - Exécuter `cd build && sha256sum -c checksums.sha256` et exécuter le travail en cas de divergence.
   - Employez le répertoire build comme `artifacts/preview-site.tar.gz`, copiez le manifeste de contrôle, exécutez `scripts/generate-preview-descriptor.mjs` et exécutez `scripts/sorafs-package-preview.sh` avec une configuration JSON (version `docs/examples/sorafs_preview_publish.json`) pour que le workflow émette autant de métadonnées qu'un bundle SoraFS déterministe.
   - Envoyez le site statique, les articles de métadonnées (`docs-portal-preview`, `docs-portal-preview-metadata`) et le bundle SoraFS (`docs-portal-preview-sorafs`) pour que le manifeste, le CV CAR et le plan puissent être inspectés sans refaire la construction.
2. Ajouter un commentaire sur le badge CI en résumant le résultat de la vérification de la somme de contrôle de nos demandes d'extraction (implémenté via l'étape de commentaire du script GitHub de `docs-portal-preview.yml`).
3. Documentez le flux de travail sur `docs/portal/README.md` (secao CI) et reliez les étapes de vérification à la liste de contrôle de publication.

## Script de vérification

`docs/portal/scripts/preview_verify.sh` valide les artéfacts de pré-visualisation basés sur les instructions manuelles de `sha256sum`. Utilisez `npm run serve` (ou l'alias explicite `npm run serve:verified`) pour exécuter le script et lancer `docusaurus serve` dans une seule étape pour partager des instantanés locaux. Une logique de vérification :1. Exécutez les ferramenta SHA appropriés (`sha256sum` ou `shasum -a 256`) contre `build/checksums.sha256`.
2. Comparez éventuellement le résumé/le nom de l'archive du descripteur de pré-visualisation `checksums_manifest` et lorsque vous indiquez le résumé/le nom de l'archive de l'archive de pré-visualisation.
3. Vous avez le code nao zéro lorsqu'une divergence est détectée pour que les réviseurs puissent bloquer les pré-visualisations adultérées.

Exemple d'utilisation (après avoir extrait les artefatos de CI) :

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Les ingénieurs de CI et la version doivent exécuter le script toujours pour obtenir un bundle de pré-visualisation ou ajouter des articles à un ticket de sortie.

## Phase 2 - Publication dans SoraFS

1. Créez le flux de travail de pré-visualisation avec une tâche :
   - Envie du site construit pour la passerelle de staging par SoraFS en utilisant `sorafs_cli car pack` et `manifest submit`.
   - Capturez le résumé du manifeste retourné et le CID du SoraFS.
   - Sérialiser `{ commit, branch, checksum_manifest, cid }` dans JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Armazenar o descriptor junto ao artefato de build e expor o CID no commentario do pull request.
3. Ajouter les tests d'intégration qui exercent `sorafs_cli` en mode d'exécution à sec pour garantir que les futurs changements seront conformes au schéma de métadonnées.

## Phase 3 - Gouvernance et auditorium1. Publier le schéma Norito (`PreviewDescriptorV1`) décrit la structure du descripteur dans `docs/portal/schemas/`.
2. Créer une liste de contrôle de publication DOCS-SORA pour effectuer :
   - Rodar `sorafs_cli manifest verify` contre l'enviado CID.
   - Registrar o digest do manifeste de checksum et o CID na description do PR de release.
3. Mettre en place une gouvernance automatique pour croiser le descripteur avec le manifeste de contrôle lors des votes de libération.

## Entrepreneur et responsable

| Marc | Propriétaire(s) | Alvo | Notes |
|-------|-------|------|-------|
| Application de somme de contrôle dans CI conclue | Infrastructure de documents | Semaine 1 | Adiciona gate de falha e uploads de artefatos. |
| Publication de pré-visualisation no SoraFS | Infrastructure de documents/Équipe de stockage | Semaine 2 | Demander l'accès aux informations d'identification de staging et d'actualisation du schéma Norito. |
| Intégration de gouvernance | Lider de Docs/DevRel / WG de Governanca | Semaine 3 | Publier le schéma et actualiser les listes de contrôle et les entrées de la feuille de route. |

## Perguntas em ouvert- Quelle ambiance le SoraFS doit-il accueillir des artefatos de pré-visualisation (mise en scène ou voie de pré-visualisation dédiée) ?
- Precisamos de asinaturas duplas (Ed25519 + ML-DSA) pas de descripteur de pré-visualisation avant la publication ?
- Le workflow de CI doit-il réparer la configuration de l'ordinateur (`orchestrator_tuning.json`) pour exécuter `sorafs_cli` pour la reproduction des manifestes ?

Enregistrez-vous comme décisions dans `docs/portal/docs/reference/publishing-checklist.md` et actualisez ce plan lorsque les duvidas sont résolus.