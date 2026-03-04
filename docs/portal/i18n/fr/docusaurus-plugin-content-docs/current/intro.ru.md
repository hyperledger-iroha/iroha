---
lang: fr
direction: ltr
source: docs/portal/docs/intro.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Добро пожаловать на разработчиков SORA Nexus

Le portail des robots SORA Nexus fournit une documentation interactive, du matériel SDK et des API utiles pour opérateurs Nexus et contrôleurs Hyperledger Iroha. Après avoir consulté les documents les plus récents, vous trouverez sur le plan de pratique précédent et les spécifications générales des documents généraux. c'est un dépôt. Le prêt à l'emploi comprend des éléments thématiques tels que Norito/SoraFS, des images OpenAPI et d'autres Le service Norito Streaming, qui permet de contrôler les contrats de travail, n'est pas spécifiquement défini.

## Ce que je peux faire maintenant

- **Utilisez Norito** - Accédez à l'écran et au démarrage rapide pour déterminer le modèle de sérialisation et le bytecode des instruments.
- **Démarrez le SDK** - effectuez le démarrage rapide pour JavaScript et Rust en ce moment ; Les applications pour Python, Swift et Android sont disponibles pour de simples récepteurs de migration.
- **Programmer les API** - page Torii OpenAPI présente les spécifications actuelles de REST, et les tableaux Les configurations sont adaptées aux fonctionnalités canoniques de Markdown.
- **Подготовить развертывания** - le runbook d'extraction et (télémétrie, règlement, superpositions Nexus) sont effectués à partir de `docs/source/` et peuvent être téléchargés мере миграции.

## Statut technique- ✅ Thème Docusaurus v3 avec un style moderne, héros/cartes pour les dégradés et les ressources, y compris les ressources Norito Diffusion.
- ✅ Le module Torii OpenAPI est ajouté à `npm run sync-openapi`, avec les sauvegardes d'écran et les paramètres CSP-guard, применяемыми `buildSecurityHeaders`.
- ✅ Aperçu et couverture de la sonde disponibles dans CI (`docs-portal-preview.yml` + `scripts/portal-probe.mjs`), consultez la documentation en streaming, les démarrages rapides SoraFS et les listes de contrôle de référence avant la publication artéfacts.
- ✅ Démarrages rapides Norito, SoraFS et SDK ainsi que les sections suivantes disponibles dans les panneaux de démarrage ; De nouvelles importations depuis `docs/source/` (streaming, orchestration, runbooks) sont disponibles pour plus de détails.

## Как принять участие

- Alors. `docs/portal/README.md` pour les commandes locales (`npm install`, `npm run start`, `npm run build`).
- Les migrations de contenu suivent la feuille de route `DOCS-*`. Lorsque vous êtes privé, activez les sections `docs/source/` et placez-vous dans le panneau ci-dessus.
- Si vous créez un artéfact général (spécifications, tables de configuration), indiquez les commandes qui vous intéressent le plus facilement.