<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9d7b44d46ef97c20058221aedf1f0b4a27ba85d204c3be4fe4933da31d9e207
source_last_modified: "2025-11-10T17:57:27.868097+00:00"
translation_last_reviewed: 2025-12-30
---

# Checklist de publication

Utilisez cette checklist chaque fois que vous mettez à jour le portail développeur. Elle garantit que le build CI, le déploiement GitHub Pages et les smoke tests manuels couvrent chaque section avant qu'un release ou un jalon du roadmap n'arrive.

## 1. Validation locale

- `npm run sync-openapi -- --version=current --latest` (ajoutez un ou plusieurs flags `--mirror=<label>` lorsque Torii OpenAPI change pour un snapshot figé).
- `npm run build` – confirmez que le texte du hero `Build on Iroha with confidence` apparaît toujours dans `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – vérifiez le manifeste des checksums (ajoutez `--descriptor`/`--archive` lors des tests d'artefacts CI téléchargés).
- `npm run serve` – lance le helper de preview protégé par checksum qui vérifie le manifeste avant d'appeler `docusaurus serve`, afin que les reviewers ne parcourent jamais un snapshot non signé (l'alias `serve:verified` reste disponible pour les appels explicites).
- Faites un spot-check du markdown que vous avez modifié via `npm run start` et le serveur de live reload.

## 2. Vérifications de pull request

- Vérifiez que le job `docs-portal-build` a réussi dans `.github/workflows/check-docs.yml`.
- Confirmez que `ci/check_docs_portal.sh` a tourné (les logs CI affichent le hero smoke check).
- Assurez-vous que le workflow de preview a uploadé un manifeste (`build/checksums.sha256`) et que le script de vérification preview a réussi (les logs affichent la sortie `scripts/preview_verify.sh`).
- Ajoutez l'URL de preview publiée depuis l'environnement GitHub Pages à la description du PR.

## 3. Validation par section

| Section | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | Le hero s'affiche, les cartes quickstart pointent vers des routes valides, les boutons CTA résolvent. |
| Norito | Norito WG | Les guides overview et getting-started référencent les derniers flags du CLI et la documentation du schéma Norito. |
| SoraFS | Storage Team | Le quickstart s'exécute jusqu'au bout, les champs du rapport de manifest sont documentés, les instructions de simulation de fetch sont vérifiées. |
| Guides SDK | Leads SDK | Les guides Rust/Python/JS compilent les exemples actuels et renvoient vers des repos live. |
| Référence | Docs/DevRel | L'index liste les specs les plus récentes, la référence du codec Norito correspond à `norito.md`. |
| Artefact de preview | Docs/DevRel | L'artefact `docs-portal-preview` est attaché au PR, les smoke checks passent, le lien est partagé avec les reviewers. |
| Security & Try it sandbox | Docs/DevRel · Security | OAuth device-code login configuré (`DOCS_OAUTH_*`), checklist `security-hardening.md` exécutée, en-têtes CSP/Trusted Types vérifiés via `npm run build` ou `npm run probe:portal`. |

Cochez chaque ligne lors de la revue du PR, ou notez toute action de suivi pour que le suivi de statut reste exact.

## 4. Notes de release

- Incluez `https://docs.iroha.tech/` (ou l'URL d'environnement issue du job de déploiement) dans les notes de release et les mises à jour de statut.
- Signalez explicitement toute section nouvelle ou modifiée afin que les équipes downstream sachent où relancer leurs propres smoke tests.
