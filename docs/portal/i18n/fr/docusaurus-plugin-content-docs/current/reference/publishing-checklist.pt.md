---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Checklist de publication

Utilisez cette liste de contrôle toujours pour actualiser le portail des utilisateurs. Vous garantissez que la construction de CI, le déploiement de pages GitHub et les tests de fumée sont manuels tous les jours avant la sortie ou la feuille de route.

## 1. Validacao local

- `npm run sync-openapi -- --version=current --latest` (ajouter un ou plusieurs drapeaux `--mirror=<label>` lorsque ou Torii OpenAPI change pour un instantané gelé).
- `npm run build` - confirmez que la copie du héros `Build on Iroha with confidence` apparaît également dans `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - vérifier le manifeste des sommes de contrôle (adicione `--descriptor`/`--archive` au test artefatos de CI baixados).
- `npm run serve` - lance l'aide de prévisualisation avec contrôle de somme de contrôle qui vérifie le manifeste avant le chamar `docusaurus serve`, pour que les réviseurs n'utilisent jamais un instantané sans entité (ou alias `serve:verified` permanent pour les chamadas explicites).
- Effectuez une vérification ponctuelle de la modification du markdown via `npm run start` et le serveur de rechargement en direct.

## 2. Vérifie la pull request

- Vérifiez que le travail `docs-portal-build` a été transmis à `.github/workflows/check-docs.yml`.
- Confirmez que `ci/check_docs_portal.sh` rodou (logs de CI mostram o hero smoke check).
- Garantit que le flux de travail de prévisualisation envoie un manifeste (`build/checksums.sha256`) et que le script de vérification de prévisualisation a été réussi (les journaux sont affichés selon `scripts/preview_verify.sh`).
- Ajoutez une URL d'aperçu publiée sur les pages GitHub ambiantes pour la description des relations publiques.## 3. Approbation par secao

| Secao | Propriétaire | Liste de contrôle |
|---------|-------|---------------|
| Page d'accueil | DevRel | Rendu de copie de héros, liens de cartes de démarrage rapide pour les rotations valides, bottes de résolution CTA. |
| Norito | Norito GT | Présentation des guides et références de démarrage pour les indicateurs les plus récents de la CLI et de la documentation du schéma Norito. |
| SoraFS | Équipe de stockage | Quickstart roda ate o fim, campos do report de manifest documentados, instrucoes de simulacao de fetch verificadas. |
| Guides SDK | Pistes du SDK | Guias Rust/Python/JS compile des exemples actuels et des liens pour les dépôts en direct. |
| Référence | Docs/DevRel | Dans la liste d'index des spécifications les plus récentes, la référence du codec Norito coïncide avec `norito.md`. |
| Aperçu de l'artefact | Docs/DevRel | L'artefato `docs-portal-preview` est ajouté aux relations publiques, aux contrôles de fumée, au lien et au partage avec les évaluateurs. |
| Sécurité et test sandbox | Docs/DevRel / Sécurité | Connexion par code de périphérique OAuth configurée (`DOCS_OAUTH_*`), liste de contrôle `security-hardening.md` exécutée, en-têtes CSP/Trusted Types vérifiés via `npm run build` ou `npm run probe:portal`. |

Marquez chaque ligne comme partie de votre examen des relations publiques, ou notez les tarifications de suivi pour le suivi précis du statut.

## 4. Notes de version- Inclut `https://docs.iroha.tech/` (ou une URL pour le travail de déploiement) des notes de version et des mises à jour de l'état.
- Destaque quaisquer secoes novas ou alteradas para que as equipes aval saibam onde reexecutar seus proprios smoke tests.