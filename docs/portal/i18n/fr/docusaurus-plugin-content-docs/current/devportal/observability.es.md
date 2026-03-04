---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilité et analyse du portail

La feuille de route DOCS-SORA nécessite des analyses, des sondes synthétiques et une automatisation des liens rotatifs
pour chaque build d'aperçu. Cette note documente la plomberie qui vient maintenant avec le portail
pour que les opérateurs puissent connecter le moniteur sans filtrer les données des visiteurs.

## Étiquette de sortie

- Définir `DOCS_RELEASE_TAG=<identifier>` (faire replier un `GIT_COMMIT` ou `dev`) al
  construire le portail. La valeur est injectée dans `<meta name="sora-release">`
  pour que les sondes et les tableaux de bord distinguent les plis.
- `npm run build` émet `build/release.json` (écrit par
  `scripts/write-checksums.mjs`) décrivant la balise, l'horodatage et le
  `DOCS_RELEASE_SOURCE` en option. Le même fichier est empaqueté dans les artefacts de prévisualisation et
  se référence dans le rapport du vérificateur de liens.

## Analyse avec préservation de la vie privée

- Configurer `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` pour
  Habiliter le tracker Liviano. Les charges utiles contiennent `{ événement, chemin, paramètres régionaux,
  version, ts }` sin metadata de referrer o IP, y se usa `navigator.sendBeacon`
  Il est toujours possible d'éviter de bloquer la navigation.
- Contrôler le festival avec `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Le tracker garde
  le dernier chemin emprunté et n'émet jamais d'événements dupliqués pour la même navigation.
- La mise en œuvre vive en `src/components/AnalyticsTracker.jsx` et se monte
  globalement à travers de `src/theme/Root.js`.

## Sondes synthétiques- `npm run probe:portal` émet des requêtes GET contra rutas comunes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) et vérifiez que le
  la balise méta `sora-release` coïncide avec `--expect-release` (o
  `DOCS_RELEASE_TAG`). Exemple :

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Les erreurs se rapportent au chemin, ce qui facilite l'accès au CD avec le résultat de la sonde.

## Automatisation des liens de rotation

- `npm run check:links` escanea `build/sitemap.xml`, assurez-vous que cada entrada mapea a un
  archiver local (chèque et solutions de secours `index.html`), et écrire
  `build/link-report.json` avec métadonnées de publication, totaux, erreurs et empreintes digitales
  SHA-256 de `checksums.sha256` (expédié comme `manifest.id`) pour chaque rapport
  il peut être vinculaire au manifeste de l'artefact.
- Le script sale avec un code distinct de zéro lorsqu'il manque une page, car je peux le faire
  bloquer les versions en routes obsolètes ou en rotations. Los reportes citan las rutas candidats
  qui se tente, ce qui aide à rastrear les régressions de routage jusqu'à l'arbol de docs.

## Tableau de bord et alertes de Grafana- `dashboards/grafana/docs_portal.json` publie le tableau Grafana **Docs Portal Publishing**.
  Inclut les panneaux suivants :
  - *Refus de passerelle (5 m)* USA `torii_sorafs_gateway_refusals_total` avec portée
    `profile`/`reason` pour que SRE détecte des poussées politiques malveillantes ou des erreurs de jetons.
  - *Alias Cache Refresh Outcomes* et *Alias Proof Age p90* ci-dessous
    `torii_sorafs_alias_cache_*` pour démontrer qu'il existe des épreuves fresques avant une coupe
    sur le DNS.
  - *Pin Registry Manifest Counts* et l'état *Active Alias Count* reflète le
    arriéré du registre des broches et des alias totaux pour que la gouvernance puisse auditer
    version cada.
  - *Expiration de la passerelle TLS (heures)* lorsque le certificat TLS de la passerelle de publication est activé
    se acerca al vencimiento (ombre d'alerte en 72 h).
  - *Résultats SLA de réplication* et *Replication Backlog* vigilan la telemetria
    `torii_sorafs_replication_*` pour garantir que toutes les répliques complètent le
    niveau GA après publication.
- Utiliser les variables de plante intégrées (`profile`, `reason`) pour enfocarte en el
  profil de publication `docs.sora` pour étudier les picos et toutes les passerelles.
- Le routage de PagerDuty utilise les panneaux du tableau de bord comme preuve : les alertes
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry`
  disparan cuando la série correspondant cruza su umbral. Afficher le runbook de l'alerteCette page permet de reproduire les requêtes exactes de Prometheus.

## Poniendolo en conjunto

1. Durante `npm run build`, définissez les variables d'environnement de publication/analyse et
   déjà que le post-build émet `checksums.sha256`, `release.json` et
   `link-report.json`.
2. Exécutez `npm run probe:portal` contre le nom d'hôte de l'aperçu avec
   `--expect-release` connecté à l'étiquette mismo. Gardez la sortie standard pour la liste de contrôle de publication.
3. Exécutez `npm run check:links` pour démarrer rapidement les entrées rotatives du plan du site et des archives
   le rapport JSON généré conjointement avec les artefacts de prévisualisation. CI a déjà le dernier rapport en
   `artifacts/docs_portal/link-report.json` pour que le gouvernement puisse télécharger le paquet de preuves
   directement à partir des journaux de build.
4. Enruta el endpoint de analitica hacia votre collecteur avec préservation de la confidentialité (Plausible,
   OTEL ingère auto-hébergé, etc.) et assure que les tâches de muestreo sont documentées par
   release pour que les tableaux de bord interprètent correctement les contenus.
5. CI vous connecte ces étapes aux workflows de prévisualisation/déploiement
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), car les applications locales à sec ne nécessitent que
   adopter un comportement spécifique de secrets.