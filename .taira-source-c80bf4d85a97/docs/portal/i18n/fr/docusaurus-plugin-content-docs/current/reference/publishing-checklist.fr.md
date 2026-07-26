---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Checklist de publication

Utilisez cette liste de contrôle chaque fois que vous mettez à jour le développeur du portail. Elle garantit que le build CI, le déploiement des pages GitHub et les manuels de smoke tests couvrent chaque section avant qu'un release ou un jalon du roadmap n'arrive.

## 1. Paramètres régionaux de validation

- `npm run sync-openapi -- --version=current --latest` (ajoutez un ou plusieurs flags `--mirror=<label>` lorsque Torii OpenAPI change pour une figure d'instantané).
- `npm run build` - confirmez que le texte du héros `Build on Iroha with confidence` apparaît toujours dans `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - vérifiez le manifeste des sommes de contrôle (ajoutez `--descriptor`/`--archive` lors des tests d'artefacts CI télécharges).
- `npm run serve` - lance l'assistant de prévisualisation protégé par checksum qui vérifie le manifeste avant d'appeler `docusaurus serve`, afin que les réviseurs ne parcourent jamais un instantané non signé (l'alias `serve:verified` reste disponible pour les appels explicites).
- Faites un spot-check du markdown que vous avez modifié via `npm run start` et le serveur de live reload.

## 2. Vérifications de pull request- Vérifiez que le job `docs-portal-build` a été réussi dans `.github/workflows/check-docs.yml`.
- Confirmez que `ci/check_docs_portal.sh` a tourné (les logs CI affichent le hero smoke check).
- Assurez-vous que le workflow de prévisualisation a téléchargé un manifeste (`build/checksums.sha256`) et que le script de vérification de prévisualisation a réussi (les logs affichent la sortie `scripts/preview_verify.sh`).
- Ajoutez l'URL de prévisualisation publiée depuis l'environnement GitHub Pages à la description du PR.

## 3. Validation par section| Rubrique | Propriétaire | Liste de contrôle |
|---------|-------|---------------|
| Page d'accueil | DevRel | Le héros s'affiche, les cartes quickstart pointent vers des itinéraires valides, les boutons CTA résolus. |
| Norito | Norito GT | Les guides de présentation et de démarrage référencent les derniers flags du CLI et la documentation du schéma Norito. |
| SoraFS | Équipe de stockage | Le quickstart s'exécute jusqu'au bout, les champs du rapport de manifeste sont documentés, les instructions de simulation de fetch sont vérifiées. |
| SDK Guides | Kit de développement logiciel Leads | Les guides Rust/Python/JS compilent les exemples actuels et reviennent vers des repos live. |
| Référence | Docs/DevRel | L'index liste les specs les plus récentes, la référence du codec Norito correspond à `norito.md`. |
| Artefact de prévisualisation | Docs/DevRel | L'artefact `docs-portal-preview` est attaché au PR, les contrôles de fumée passent, le lien est partagé avec les reviewers. |
| Sécurité et test sandbox | Sécurité des documents/DevRel | Configuration de la connexion au code de périphérique OAuth (`DOCS_OAUTH_*`), liste de contrôle `security-hardening.md` exécutée, en-têtes CSP/Trusted Types vérifiés via `npm run build` ou `npm run probe:portal`. |

Cochez chaque ligne lors de la revue du PR, ou notez toute action de suivi pour que le suivi de statut reste exact.

## 4. Notes de version- Incluez `https://docs.iroha.tech/` (ou l'URL d'environnement issue du job de déploiement) dans les notes de release et les mises à jour de statut.
- Signalez préciser toute section nouvelle ou modifiée afin que les équipes en aval s'achent ou relancent leurs propres tests de fumée.