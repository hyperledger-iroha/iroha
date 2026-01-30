---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-integrity-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Plan de previsualisation controle par checksum

Ce plan decrit le travail restant necessaire pour rendre chaque artefact de previsualisation du portail verifiable avant publication. L'objectif est de garantir que les relecteurs telechargent exactement le snapshot construit en CI, que le manifeste de checksum est immuable et que la previsualisation est decouvrable via SoraFS avec des metadonnees Norito.

## Objectifs

- **Builds deterministes:** S'assurer que `npm run build` produit une sortie reproductible et emet toujours `build/checksums.sha256`.
- **Previsualisations verifiees:** Exiger que chaque artefact de previsualisation fournisse un manifeste de checksum et refuser la publication lorsque la verification echoue.
- **Metadonnees publiees via Norito:** Persister les descripteurs de previsualisation (metadonnees de commit, digest de checksum, CID SoraFS) en JSON Norito afin que les outils de gouvernance puissent auditer les releases.
- **Outillage operateur:** Fournir un script de verification en une etape que les consommateurs peuvent executer localement (`./docs/portal/scripts/preview_verify.sh --build-dir build --descriptor <path> --archive <path>`); le script encapsule desormais le flux de validation checksum + descripteur de bout en bout. La commande de previsualisation standard (`npm run serve`) invoque maintenant cet assistant automatiquement avant `docusaurus serve` afin que les snapshots locaux restent sous controle de checksum (avec `npm run serve:verified` conserve comme alias explicite).

## Phase 1 - Application CI

1. Mettre a jour `.github/workflows/docs-portal-preview.yml` pour:
   - Executer `node docs/portal/scripts/write-checksums.mjs` apres le build Docusaurus (deja invoque localement).
   - Executer `cd build && sha256sum -c checksums.sha256` et echouer le job en cas de divergence.
   - Empaqueter le repertoire build en `artifacts/preview-site.tar.gz`, copier le manifeste de checksum, executer `scripts/generate-preview-descriptor.mjs`, et executer `scripts/sorafs-package-preview.sh` avec une configuration JSON (voir `docs/examples/sorafs_preview_publish.json`) afin que le workflow emette a la fois les metadonnees et un bundle SoraFS deterministe.
   - Televerser le site statique, les artefacts de metadonnees (`docs-portal-preview`, `docs-portal-preview-metadata`) et le bundle SoraFS (`docs-portal-preview-sorafs`) afin que le manifeste, le resume CAR et le plan puissent etre inspectes sans relancer le build.
2. Ajouter un commentaire de badge CI qui resume le resultat de la verification checksum dans les pull requests (implemente via l'etape de commentaire GitHub Script de `docs-portal-preview.yml`).
3. Documenter le workflow dans `docs/portal/README.md` (section CI) et lier les etapes de verification dans la checklist de publication.

## Script de verification

`docs/portal/scripts/preview_verify.sh` valide les artefacts de previsualisation telecharges sans exiger d'invocations manuelles de `sha256sum`. Utilisez `npm run serve` (ou l'alias explicite `npm run serve:verified`) pour executer le script et lancer `docusaurus serve` en une seule etape lorsque vous partagez des snapshots locaux. La logique de verification:

1. Execute l'outil SHA approprie (`sha256sum` ou `shasum -a 256`) contre `build/checksums.sha256`.
2. Compare optionnellement le digest/nom de fichier du descripteur de previsualisation `checksums_manifest` et, lorsque fourni, le digest/nom de fichier de l'archive de previsualisation.
3. Sort avec un code non nul lorsqu'une divergence est detectee afin que les relecteurs puissent bloquer des previsualisations alterees.

Exemple d'utilisation (apres extraction des artefacts CI):

```bash
./docs/portal/scripts/preview_verify.sh \
  --build-dir build \
  --descriptor artifacts/preview-descriptor.json \
  --archive artifacts/preview-site.tar.gz
```

Les ingenieurs CI et release doivent appeler le script chaque fois qu'ils telechargent un bundle de previsualisation ou attachent des artefacts a un ticket de release.

## Phase 2 - Publication SoraFS

1. Etendre le workflow de previsualisation avec un job qui:
   - Televerse le site construit vers la passerelle de staging SoraFS en utilisant `sorafs_cli car pack` et `manifest submit`.
   - Capture le digest du manifeste renvoye et le CID SoraFS.
   - Serialise `{ commit, branch, checksum_manifest, cid }` en JSON Norito (`docs/portal/preview/preview_descriptor.json`).
2. Stocker le descripteur avec l'artefact de build et exposer le CID dans le commentaire de pull request.
3. Ajouter des tests d'integration qui exercent `sorafs_cli` en mode dry-run afin d'assurer que les evolutions futures conservent la coherence du schema de metadonnees.

## Phase 3 - Gouvernance et audit

1. Publier un schema Norito (`PreviewDescriptorV1`) decrivant la structure du descripteur sous `docs/portal/schemas/`.
2. Mettre a jour la checklist de publication DOCS-SORA pour exiger:
   - Lancer `sorafs_cli manifest verify` sur le CID charge.
   - Enregistrer le digest du manifeste de checksum et le CID dans la description de la PR de release.
3. Connecter l'automatisation de gouvernance pour croiser le descripteur avec le manifeste de checksum pendant les votes de release.

## Livrables et responsabilites

| Jalon | Proprietaire(s) | Cible | Notes |
|-------|-----------------|-------|-------|
| Application des checksums en CI livree | Infrastructure Docs | Semaine 1 | Ajoute un gate de validation et des uploads d'artefacts. |
| Publication des previews SoraFS | Infrastructure Docs / Equipe Storage | Semaine 2 | Necessite l'acces aux identifiants de staging et des mises a jour du schema Norito. |
| Integration gouvernance | Responsable Docs/DevRel / WG Gouvernance | Semaine 3 | Publie le schema et met a jour les checklists et les entrees du roadmap. |

## Questions ouvertes

- Quel environnement SoraFS doit heberger les artefacts de previsualisation (staging vs. lane de previsualisation dediee)?
- Avons-nous besoin de doubles signatures (Ed25519 + ML-DSA) sur le descripteur de previsualisation avant publication?
- Le workflow CI doit-il epingler la configuration de l'orchestrateur (`orchestrator_tuning.json`) lors de l'execution de `sorafs_cli` pour garder des manifestes reproductibles?

Consignez les decisions dans `docs/portal/docs/reference/publishing-checklist.md` et mettez a jour ce plan une fois les inconnues resolues.
