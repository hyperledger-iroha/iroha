---
lang: fr
direction: ltr
source: docs/portal/README.md
status: complete
translator: manual
source_hash: 4b0d6c295c7188355e2c03d7c8240271da147095ff557fae2152f42e27bd17fa
source_last_modified: "2025-11-14T04:43:03.939564+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/portal/README.md (SORA Nexus Developer Portal) -->

# Portail développeur SORA Nexus

Ce répertoire héberge le workspace Docusaurus pour le portail développeur interactif. Le
portail agrège les guides Norito, les quickstarts SDK et la référence OpenAPI générée par
`cargo xtask openapi`, le tout habillé avec le branding SORA Nexus utilisé dans l’ensemble
du programme de documentation.

## Prérequis

- Node.js 18.18 ou plus récent (baseline Docusaurus v3).
- Yarn 1.x ou npm ≥ 9 pour la gestion de dépendances.
- Toolchain Rust (utilisé par le script de synchronisation OpenAPI).

## Bootstrap

```bash
cd docs/portal
npm install    # ou yarn install
```

## Scripts disponibles

| Commande | Description |
|----------|-------------|
| `npm run start` / `yarn start` | Lance un serveur de développement local avec live reload (par défaut `http://localhost:3000`). |
| `npm run build` / `yarn build` | Produit un build de production dans `build/`. |
| `npm run serve` / `yarn serve` | Sert le dernier build localement (utile pour des smoke tests). |
| `npm run docs:version -- <label>` | Fige la documentation actuelle dans `versioned_docs/version-<label>` (wrapper autour de `docusaurus docs:version`). |
| `npm run sync-openapi` / `yarn sync-openapi` | Régénère `static/openapi/torii.json` via `cargo xtask openapi` (ajoutez `--mirror=<label>` pour copier la spec vers d’autres snapshots de version). |
| `npm run tryit-proxy` | Lance le proxy de staging qui alimente la console « Try it » (voir configuration ci‑dessous). |
| `npm run probe:tryit-proxy` | Effectue un probe `/healthz` + requête d’exemple contre le proxy (helper CI/monitoring). |
| `npm run manage:tryit-proxy -- <update|rollback>` | Met à jour ou restaure la cible `.env` du proxy avec gestion des sauvegardes. |
| `npm run sync-i18n` | S’assure que des stubs de traduction existent pour le japonais, l’hébreu, l’espagnol, le portugais, le français, le russe, l’arabe et l’ourdou sous `i18n/`. |
| `npm run sync-norito-snippets` | Régénère les docs d’exemples Kotodama + snippets téléchargeables (également invoqué automatiquement par le plugin du serveur de dev). |
| `npm run test:tryit-proxy` | Exécute les tests unitaires du proxy via le test runner Node (`node --test`). |

Le script de synchronisation OpenAPI nécessite que `cargo xtask openapi` soit disponible
depuis la racine du dépôt ; il génère un JSON déterministe dans `static/openapi/` et
suppose que le routeur Torii expose une spec « live » (ne recourir à
`cargo xtask openapi --allow-stub` que pour une sortie temporaire d’urgence).

## Versionnage des docs & snapshots OpenAPI

- **Créer une version de docs :** exécuter `npm run docs:version -- 2025-q3` (ou tout
  label convenu). Commiter `versioned_docs/version-<label>`, `versioned_sidebars` et
  `versions.json`. Le menu déroulant de version dans la navbar expose automatiquement le
  nouveau snapshot.
- **Synchroniser les artefacts OpenAPI :** après la création d’une version, actualiser la
  spec canonique et le manifest via `cargo xtask openapi --sign <chemin-vers-clé-ed25519>`,
  puis capturer un snapshot correspondant avec
  `npm run sync-openapi -- --version=2025-q3 --mirror=current --latest`. Le script écrit
  `static/openapi/versions/2025-q3/torii.json`, copie la spec dans
  `versions/current/torii.json`, met à jour `versions.json`, rafraîchit
  `/openapi/torii.json` et clone le `manifest.json` signé dans chaque dossier de version
  pour que les specs historiques embarquent le même metadata de provenance. Ajoutez autant
  de flags `--mirror=<label>` que nécessaire pour répliquer la spec dans d’autres
  snapshots historiques.
- **Attentes CI :** les commits modifiant la documentation doivent inclure, le cas
  échéant, le bump de version ainsi que des snapshots OpenAPI actualisés, afin que les
  panneaux Swagger, RapiDoc et Redoc puissent basculer entre specs historiques sans
  erreurs de fetch.
- **Contrôle du manifest :** le script `sync-openapi` ne copie les manifests que lorsque
  le `manifest.json` sur disque correspond à la spec fraîchement générée. Si la copie est
  ignorée, relancez `cargo xtask openapi --sign <clé>` pour rafraîchir le manifest
  canonique puis relancez la synchronisation afin que les snapshots versionnés héritent du
  metadata signé. Le script `ci/check_openapi_spec.sh` rejoue le générateur et valide le
  manifest avant d’autoriser un merge.

## Structure

```text
docs/portal/
├── docs/                 # Contenu Markdown/MDX du portail
├── i18n/                 # Overrides de locale (ja/he) générés par sync-i18n
├── src/                  # Pages/composants React (scaffolding)
├── static/               # Assets statiques servis tel quels (inclut le JSON OpenAPI)
├── scripts/              # Scripts d’aide (synchronisation OpenAPI)
├── docusaurus.config.js  # Configuration principale du site
└── sidebars.js           # Modèle de navigation / sidebars
```

### Configuration du proxy Try It

Le sandbox « Try it » fait transiter les requêtes via `scripts/tryit-proxy.mjs`.
Configurez le proxy avec des variables d’environnement avant de le lancer :

```bash
export TRYIT_PROXY_TARGET="https://torii.staging.sora"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
```

En staging/production, définissez ces variables dans le système de configuration de votre
plateforme (par exemple, secrets GitHub Actions, variables d’environnement du
orchestrateur de conteneurs, etc.).

### URLs de prévisualisation & notes de version

- Préversion publique beta : `https://docs.iroha.tech/`
- GitHub expose également le build sous l’environnement **github-pages** pour chaque
  déploiement.
- Les pull requests touchant au contenu du portail incluent des artefacts Actions
  (`docs-portal-preview`, `docs-portal-preview-metadata`) contenant le site compilé, un
  manifest de checksums, une archive compressée et un descriptor ; les relecteurs peuvent
  télécharger `index.html`, l’ouvrir localement et vérifier les checksums avant de
  partager des préversions. Le workflow ajoute un commentaire de synthèse (hashes du
  manifest/de l’archive et état SoraFS) à chaque PR pour fournir un signal rapide de
  réussite de la vérification.
- Utilisez `./docs/portal/scripts/preview_verify.sh --build-dir <build extrait> --descriptor <descripteur> --archive <archive>` après avoir téléchargé un bundle de préversion afin de confirmer que les artefacts correspondent au build CI avant de partager le lien en externe.
- Lors de la préparation de release notes ou de bilans d’état, référencez l’URL de
  préversion afin que les relecteurs externes puissent parcourir le dernier snapshot du
  portail sans cloner le dépôt.
- Coordonnez les vagues de préversion via
  `docs/portal/docs/devportal/preview-invite-flow.md` et associez‑les à
  `docs/portal/docs/devportal/reviewer-onboarding.md` de sorte que chaque invitation,
  export de télémétrie et étape d’offboarding réutilise la même piste d’audit.

