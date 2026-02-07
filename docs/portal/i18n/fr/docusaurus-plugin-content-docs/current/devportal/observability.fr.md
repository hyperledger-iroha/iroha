---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilité et analytique du portail

La feuille de route DOCS-SORA exige des analyses, des sondes synthétiques et une automatisation des liens
casses pour chaque build de prévisualisation. Cette note documente la plomberie livre avec le portail afin
que les opérateurs peuvent brancher le monitoring sans exposer les données des visiteurs.

## Marquage de la version

- Définir `DOCS_RELEASE_TAG=<identifier>` (fallback sur `GIT_COMMIT` ou `dev`) lors du
  build du portail. La valeur est injectée dans `<meta name="sora-release">`
  pour que les sondes et tableaux de bord distinguent les déploiements.
- `npm run build` genre `build/release.json` (écrit par
  `scripts/write-checksums.mjs`) qui décrit le tag, le timestamp et le
  `DOCS_RELEASE_SOURCE` en option. Le meme fichier est embarque dans les artefacts preview et
  référence par le rapport du link checker.

## Analytics respectueuses de la vie privée

- Configurer `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` pour activer
  le tracker léger. Les charges utiles contiennent `{ event, path, locale, release, ts }`
  sans métadonnées de référent ou d'IP, et `navigator.sendBeacon` est utilisé des que possible
  pour éviter de bloquer les navigations.
- Contrôler l'échantillonnage avec `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Le tracker conserve
  le dernier chemin envoyé et n'emmet jamais d'événements dupliques pour la même navigation.
- L'implémentation se trouve dans `src/components/AnalyticsTracker.jsx` et est montée
  globalement via `src/theme/Root.js`.

## Sondes synthétiques- `npm run probe:portal` emet des requêtes GET sur des routes courantes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) et vérifiez que le
  La balise méta `sora-release` correspond à un `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Exemple :

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Les échecs sont rapportés par chemin, ce qui facilite le gate CD sur le succès des sondes.

## Automatisation des liens casses

- `npm run check:links` scanne `build/sitemap.xml`, s'assure que chaque entrée mappe vers un
  fichier local (en vérifiant les fallbacks `index.html`), et ecrit
  `build/link-report.json` contenant les métadonnées de release, les totaux, les echecs et
  l'empreinte SHA-256 de `checksums.sha256` (exposée comme `manifest.id`) afin que chaque
  rapport puisse être rattaché au manifeste d'artefact.
- Le script retourne un code non nul lorsqu'une page manque, afin que la CI puisse bloquer
  les releases sur des routes obsolètes ou cassées. Les rapports citant les chemins candidats
  tentes, ce qui aide à tracer les régressions de routage jusqu'à l'arbre de docs.

## Tableau de bord Grafana et alertes- `dashboards/grafana/docs_portal.json` publie le tableau Grafana **Docs Portal Publishing**.
  Il contient les panneaux suivants :
  - *Gateway Refusals (5m)* utilise `torii_sorafs_gateway_refusals_total` scope par
    `profile`/`reason` pour que les SRE détectent des poussées de politique incorrectes ou des
    échecs de jetons.
  - *Alias Cache Refresh Outcomes* et *Alias Proof Age p90* suivent
    `torii_sorafs_alias_cache_*` pour prouver que des preuves fraîches existent avant une coupe
    via DNS.
  - *Pin Registry Manifest Counts* et la statistique *Active Alias Count* refletent le
    backlog du pin-registry et le total des aliases pour que la gouvernance puisse auditer
    chaque sortie.
  - *Gateway TLS Expiry (hours)* avec en avant l'approche de l'expiration du cert TLS du
    passerelle de publication (seuil d'alerte à 72 h).
  - *Replication SLA Outcomes* et *Replication Backlog* surveillent la telemetrie
    `torii_sorafs_replication_*` pour s'assurer que toutes les replicas respectent le
    niveau GA après publication.
- Utilisez les variables de template integrees (`profile`, `reason`) pour vous concentrer sur
  le profil de publishing `docs.sora` ou enqueter sur des pics sur l'ensemble des gateways.
- Le routage PagerDuty utilise les panneaux du dashboard comme preuve: les alertes
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry`se declenchent lorsque la série correspondante dépasse son seuil. Liez le runbook de
  l'alerte a cette page pour que l'astreinte puisse rejouer les requêtes Prometheus exactes.

## Assembler le tout

1. Pendant `npm run build`, définir les variables d'environnement de release/analytics et
   laisser l'étape post-build emettre `checksums.sha256`, `release.json` et
   `link-report.json`.
2. Executer `npm run probe:portal` contre l'hôte aperçu avec
   `--expect-release` branche sur le tag meme. Sauvegarder la sortie standard pour la checklist
   de publication.
3. Executer `npm run check:links` pour échouer rapidement sur les entrées de sitemap cassées
   et archiver le rapport JSON généré avec les artefacts preview. La CI dépose le
   dernier rapport dans `artifacts/docs_portal/link-report.json` pour que la gouvernance
   peut télécharger le bundle de preuves directement depuis les logs de build.
4. Router l'endpoint Analytics vers votre collecteur respectueux de la vie privée (Plausible,
   OTEL ingest auto-heberge, etc.) et s'assurer que les taux d'echantillonnage sont documentés
   par release afin que les tableaux de bord interprètent correctement les volumes.
5. La CI cable déjà ces étapes via les workflows preview/deploy
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), donc les pistes locales ne doivent pas être couvertes
   que le comportement spécifique aux secrets.