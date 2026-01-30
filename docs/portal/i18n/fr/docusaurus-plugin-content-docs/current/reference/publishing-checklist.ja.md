---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/reference/publishing-checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 407cbb40da60f465b4d25b3f1198ad3565eabf4dfa30d4955b0aab131a0b25e7
source_last_modified: "2025-11-14T04:43:20.994637+00:00"
translation_last_reviewed: 2026-01-30
---

# Checklist de publication

Utilisez cette checklist chaque fois que vous mettez a jour le portail developpeur. Elle garantit que le build CI, le deploiement GitHub Pages et les smoke tests manuels couvrent chaque section avant qu'un release ou un jalon du roadmap n'arrive.

## 1. Validation locale

- `npm run sync-openapi -- --version=current --latest` (ajoutez un ou plusieurs flags `--mirror=<label>` lorsque Torii OpenAPI change pour un snapshot fige).
- `npm run build` - confirmez que le texte du hero `Build on Iroha with confidence` apparait toujours dans `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - verifiez le manifeste des checksums (ajoutez `--descriptor`/`--archive` lors des tests d'artefacts CI telecharges).
- `npm run serve` - lance le helper de preview protege par checksum qui verifie le manifeste avant d'appeler `docusaurus serve`, afin que les reviewers ne parcourent jamais un snapshot non signe (l'alias `serve:verified` reste disponible pour les appels explicites).
- Faites un spot-check du markdown que vous avez modifie via `npm run start` et le serveur de live reload.

## 2. Verifications de pull request

- Verifiez que le job `docs-portal-build` a reussi dans `.github/workflows/check-docs.yml`.
- Confirmez que `ci/check_docs_portal.sh` a tourne (les logs CI affichent le hero smoke check).
- Assurez-vous que le workflow de preview a uploade un manifeste (`build/checksums.sha256`) et que le script de verification preview a reussi (les logs affichent la sortie `scripts/preview_verify.sh`).
- Ajoutez l'URL de preview publiee depuis l'environnement GitHub Pages a la description du PR.

## 3. Validation par section

| Section | Owner | Checklist |
|---------|-------|-----------|
| Homepage | DevRel | Le hero s'affiche, les cartes quickstart pointent vers des routes valides, les boutons CTA resolvent. |
| Norito | Norito WG | Les guides overview et getting-started referencent les derniers flags du CLI et la documentation du schema Norito. |
| SoraFS | Storage Team | Le quickstart s'execute jusqu'au bout, les champs du rapport de manifest sont documentes, les instructions de simulation de fetch sont verifiees. |
| Guides SDK | Leads SDK | Les guides Rust/Python/JS compilent les exemples actuels et renvoient vers des repos live. |
| Reference | Docs/DevRel | L'index liste les specs les plus recentes, la reference du codec Norito correspond a `norito.md`. |
| Artefact de preview | Docs/DevRel | L'artefact `docs-portal-preview` est attache au PR, les smoke checks passent, le lien est partage avec les reviewers. |
| Security & Try it sandbox | Docs/DevRel  Security | OAuth device-code login configure (`DOCS_OAUTH_*`), checklist `security-hardening.md` executee, en-tetes CSP/Trusted Types verifies via `npm run build` ou `npm run probe:portal`. |

Cochez chaque ligne lors de la revue du PR, ou notez toute action de suivi pour que le suivi de statut reste exact.

## 4. Notes de release

- Incluez `https://docs.iroha.tech/` (ou l'URL d'environnement issue du job de deploiement) dans les notes de release et les mises a jour de statut.
- Signalez explicitement toute section nouvelle ou modifiee afin que les equipes downstream sachent ou relancer leurs propres smoke tests.
