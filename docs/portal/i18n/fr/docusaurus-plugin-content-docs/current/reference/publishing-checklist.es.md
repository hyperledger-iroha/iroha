---
lang: fr
direction: ltr
source: docs/portal/docs/reference/publishing-checklist.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Liste de vérification de publication

Utilisez cette liste chaque fois que vous actualisez le portail des développeurs. Garantissez que la construction de CI, le téléchargement sur les pages GitHub et les manuels d'évaluation sont inclus chaque section avant de publier une version ou un hit de la feuille de route.

## 1. Validation locale

- `npm run sync-openapi -- --version=current --latest` (ajouter un ou plusieurs drapeaux `--mirror=<label>` lorsque Torii OpenAPI change pour un instantané gelé).
- `npm run build` - confirma que la copie du héros `Build on Iroha with confidence` apparaît en `build/index.html`.
- `./docs/portal/scripts/preview_verify.sh --build-dir build` - vérifier le manifeste des sommes de contrôle (ajouter `--descriptor`/`--archive` à la recherche d'artefacts téléchargés de CI).
- `npm run serve` - lancez l'aide de prévisualisation avec le contrôle de la somme de contrôle qui vérifie le manifeste avant d'appeler à `docusaurus serve`, pour que les réviseurs ne puissent pas avoir un instantané sans entreprise (l'alias `serve:verified` est disponible pour les appels explicites).
- Révisez le markdown à lancer via `npm run start` et le serveur de live reload.

## 2. Chèques de pull request- Vérifiez que le travail `docs-portal-build` est passé en `.github/workflows/check-docs.yml`.
- Confirma que `ci/check_docs_portal.sh` est éjecté (les journaux de CI muestran el hero smoke check).
- Assurez-vous que le flux de travail de prévisualisation passe par un manifeste (`build/checksums.sha256`) et que le script de vérification de prévisualisation se termine (les journaux nécessitent la sortie de `scripts/preview_verify.sh`).
- Ajoutez l'URL d'aperçu publiée depuis l'entorno des pages GitHub à la description du PR.

## 3. Approbation par section| Section | Propriétaire | Liste de contrôle |
|---------|-------|---------------|
| Page d'accueil | DevRel | Le héros rend, les tarjetas de quickstart enlazan sur les routes validées, les boutons CTA sont générés. |
| Norito | Norito GT | Les guides de présentation et de démarrage font référence aux indicateurs les plus récents de la CLI et à la documentation du type Norito. |
| SoraFS | Équipe de stockage | Le démarrage rapide est exécuté jusqu'à la finale, les champs du rapport du manifeste sont documentés et les instructions de simulation de récupération vérifiées. |
| SDK Guias | SDK Lire | Les guides de Rust/Python/JS compilent les exemples actuels et les mettent en dépôt réel. |
| Référence | Docs/DevRel | L'indice listant les spécifications les plus récentes, la référence du codec Norito coïncide avec `norito.md`. |
| Artefact de prévisualisation | Docs/DevRel | L'artefact `docs-portal-preview` est ajouté au PR, les contrôles de fumée pasan, l'enlace se partagent avec les évaluateurs. |
| Sécurité et test sandbox | Sécurité des documents/DevRel | Configuration de la connexion par code de périphérique OAuth (`DOCS_OAUTH_*`), liste de contrôle `security-hardening.md` exécutée, encabezados CSP/Trusted Types vérifiés via `npm run build` ou `npm run probe:portal`. |

Marca cada fila como parte de votre revision de PR, o anota tareas pendientes para que el seguimiento de estado siga siendo preciso.

## 4. Notes de lancement- Incluye `https://docs.iroha.tech/` (ou l'URL de l'origine du travail de téléchargement) dans les notes de version et les mises à jour de l'état.
- Destaca cualquier seccion nueva o modificada para que los equipos aval sepan donde volver a ejecutar sus propias pruebas de humo.