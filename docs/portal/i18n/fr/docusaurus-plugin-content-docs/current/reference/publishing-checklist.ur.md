---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Liste de contrôle de publication

Il existe un portail des développeurs qui contient une liste de contrôle et une liste de contrôle. Il y a plusieurs projets de construction de CI et le déploiement de pages GitHub ainsi que des tests de fumée et des tests de compatibilité. La sortie de la version et l'étape importante de la feuille de route

## 1. Validation locale

- `npm run sync-openapi -- --version=current --latest` (Torii OpenAPI est un instantané gelé OpenAPI flags بنے).
- `npm run build` – Copie de héros `Build on Iroha with confidence` pour `build/index.html` en copie de héros
- `./docs/portal/scripts/preview_verify.sh --build-dir build` – vérification du manifeste de la somme de contrôle (artefacts CI téléchargés par exemple et `--descriptor`/`--archive` par exemple).
- `npm run serve` – assistant de prévisualisation à somme de contrôle pour la vérification du manifeste et `docusaurus serve` pour la vérification du manifeste par les réviseurs Un instantané non signé est un instantané (`serve:verified` alias واضح کالز کیلئے موجود رہتا ہے).
- `npm run start` pour le serveur de rechargement en direct avec démarque et vérification ponctuelle avec touche tactile

## 2. Vérifications des demandes de tirage

- `.github/workflows/check-docs.yml` et `docs-portal-build` travail pour vérifier le travail
- تصدیق کریں کہ `ci/check_docs_portal.sh` چلا (CI enregistre un contrôle de fumée de héros دکھائی دیتا ہے)۔
- Un aperçu du flux de travail et un manifeste (`build/checksums.sha256`) un téléchargement et un script de vérification d'aperçu (journaux CI et une sortie `scripts/preview_verify.sh` sont disponibles). ہے)۔
- Environnement de pages GitHub et URL d'aperçu publiée et description du PR## 3. Signature de la section

| Rubrique | Propriétaire | Liste de contrôle |
|---------|-------|---------------|
| Page d'accueil | DevRel | Rendu de copie du héros et cartes de démarrage rapide, itinéraires valides et boutons CTA résolus |
| Norito | Norito GT | Présentation et guides de démarrage ainsi que les indicateurs CLI et la documentation du schéma Norito et référencement ici |
| SoraFS | Équipe de stockage | Démarrage rapide et champs du rapport manifeste documentés et récupération des instructions de simulation vérification et vérification |
| Guides SDK | Pistes du SDK | Les guides Rust/Python/JS et les exemples compilent des dépôts en direct et un lien vers |
| Référence | Docs/DevRel | Index Spécifications du codec Norito Référence `norito.md` et correspondance avec le codec |
| Aperçu de l'artefact | Docs/DevRel | `docs-portal-preview` artefact PR کے ساتھ joindre ہو، contrôles de fumée passer ہوں، lien réviseurs کے ساتھ partager ہو۔ |
| Sécurité et test sandbox | Docs/DevRel · Sécurité | Configuration de la connexion par code de périphérique OAuth (`DOCS_OAUTH_*`) et liste de contrôle `security-hardening.md`, exécution et en-têtes CSP/Trusted Types `npm run build` et `npm run probe:portal` et vérification et vérification |

ہر rangée کو اپنے PR review کا حصہ بنائیں، یا tâches de suivi لکھیں تاکہ status tracking درست رہے۔

## 4. Notes de version- `https://docs.iroha.tech/` (tâche de déploiement et URL de l'environnement) et notes de version et mises à jour de statut ainsi que l'URL de l'environnement.
- De nombreuses sections de sections et des équipes en aval sont également chargées des tests de fumée. دوبارہ چلانے ہیں۔