---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5f6d17a605e4b90d9bb9cc041055c43b2f1b384fd13f732c0a56e5de5fe78bbd
source_last_modified: "2025-11-18T09:03:32+00:00"
translation_last_reviewed: 2026-01-01
---

# Observabilite et analytics du portail

La roadmap DOCS-SORA exige des analytics, des probes synthetiques et une automatisation des liens
casses pour chaque build de preview. Cette note documente la plomberie livre avec le portail afin
que les operateurs puissent brancher le monitoring sans exposer les donnees des visiteurs.

## Tagging de release

- Definir `DOCS_RELEASE_TAG=<identifier>` (fallback sur `GIT_COMMIT` ou `dev`) lors du
  build du portail. La valeur est injectee dans `<meta name="sora-release">`
  pour que les probes et dashboards distinguent les deploiements.
- `npm run build` genere `build/release.json` (ecrit par
  `scripts/write-checksums.mjs`) qui decrit le tag, le timestamp et le
  `DOCS_RELEASE_SOURCE` optionnel. Le meme fichier est embarque dans les artefacts preview et
  reference par le rapport du link checker.

## Analytics respectueuses de la vie privee

- Configurer `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` pour activer
  le tracker leger. Les payloads contiennent `{ event, path, locale, release, ts }`
  sans metadata de referrer ou d'IP, et `navigator.sendBeacon` est utilise des que possible
  pour eviter de bloquer les navigations.
- Controler l'echantillonnage avec `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Le tracker conserve
  le dernier path envoye et n'emmet jamais d'evenements dupliques pour la meme navigation.
- L'implementation se trouve dans `src/components/AnalyticsTracker.jsx` et est montee
  globalement via `src/theme/Root.js`.

## Probes synthetiques

- `npm run probe:portal` emet des requetes GET sur des routes courantes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) et verifie que le
  meta tag `sora-release` correspond a `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Exemple:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Les echecs sont rapportes par path, ce qui facilite le gate CD sur le succes des probes.

## Automatisation des liens casses

- `npm run check:links` scanne `build/sitemap.xml`, s'assure que chaque entree mappe vers un
  fichier local (en verifiant les fallbacks `index.html`), et ecrit
  `build/link-report.json` contenant la metadata de release, les totaux, les echecs et
  l'empreinte SHA-256 de `checksums.sha256` (exposee comme `manifest.id`) afin que chaque
  rapport puisse etre rattache au manifeste d'artefact.
- Le script retourne un code non-zero quand une page manque, afin que la CI puisse bloquer
  les releases sur des routes obsoletes ou cassees. Les rapports citent les chemins candidats
  tentes, ce qui aide a tracer les regressions de routage jusqu'a l'arbre de docs.

## Dashboard Grafana et alertes

- `dashboards/grafana/docs_portal.json` publie le tableau Grafana **Docs Portal Publishing**.
  Il contient les panneaux suivants:
  - *Gateway Refusals (5m)* utilise `torii_sorafs_gateway_refusals_total` scope par
    `profile`/`reason` pour que les SRE detectent des pushes de politique incorrects ou des
    echecs de tokens.
  - *Alias Cache Refresh Outcomes* et *Alias Proof Age p90* suivent
    `torii_sorafs_alias_cache_*` pour prouver que des proofs frais existent avant un cut
    over DNS.
  - *Pin Registry Manifest Counts* et la statistique *Active Alias Count* refletent le
    backlog du pin-registry et le total des aliases pour que la gouvernance puisse auditer
    chaque release.
  - *Gateway TLS Expiry (hours)* met en avant l'approche de l'expiration du cert TLS du
    gateway de publishing (seuil d'alerte a 72 h).
  - *Replication SLA Outcomes* et *Replication Backlog* surveillent la telemetrie
    `torii_sorafs_replication_*` pour s'assurer que toutes les replicas respectent le
    niveau GA apres publication.
- Utilisez les variables de template integrees (`profile`, `reason`) pour vous concentrer sur
  le profil de publishing `docs.sora` ou enqueter sur des pics sur l'ensemble des gateways.
- Le routage PagerDuty utilise les panneaux du dashboard comme preuve: les alertes
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry`
  se declenchent lorsque la serie correspondante depasse son seuil. Liez le runbook de
  l'alerte a cette page pour que l'on-call puisse rejouer les requetes Prometheus exactes.

## Assembler le tout

1. Pendant `npm run build`, definir les variables d'environnement de release/analytics et
   laisser l'etape post-build emettre `checksums.sha256`, `release.json` et
   `link-report.json`.
2. Executer `npm run probe:portal` contre l'hote preview avec
   `--expect-release` branche sur le meme tag. Sauvegarder le stdout pour la checklist
   de publishing.
3. Executer `npm run check:links` pour echouer rapidement sur les entrees de sitemap cassees
   et archiver le rapport JSON genere avec les artefacts preview. La CI depose le
   dernier rapport dans `artifacts/docs_portal/link-report.json` pour que la gouvernance
   puisse telecharger le bundle de preuves directement depuis les logs de build.
4. Router l'endpoint analytics vers votre collecteur respectueux de la vie privee (Plausible,
   OTEL ingest auto-heberge, etc.) et s'assurer que les taux d'echantillonnage sont documentes
   par release afin que les dashboards interpretent correctement les volumes.
5. La CI cable deja ces etapes via les workflows preview/deploy
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), donc les dry runs locaux ne doivent couvrir
   que le comportement specifique aux secrets.
