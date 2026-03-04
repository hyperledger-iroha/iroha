---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Observabilité et analyse du portail

La feuille de route DOCS-SORA exige des analyses, des sondes synthétiques et des liens automatisés
pour chaque build d'aperçu. Cette note documente l'infrastructure qui accompagne maintenant le portail
pour que les opérateurs connectent la surveillance aux données des visiteurs.

## Marquage de la version

- Définir `DOCS_RELEASE_TAG=<identifier>` (repli pour `GIT_COMMIT` ou `dev`) entre autres
  construire un portail. La valeur injectée dans `<meta name="sora-release">`
  pour que les sondes et les tableaux de bord distinguent les déploiements.
- `npm run build` émet `build/release.json` (écrit par
  `scripts/write-checksums.mjs`) décrivant la balise, l'horodatage et l'heure
  `DOCS_RELEASE_SOURCE` en option. O mesmo archquivo e empacotado nos artefatos de preview e
  référence au rapport du vérificateur de liens.

## Analytics avec préservation de la confidentialité

- Configurer `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` pour
  habilitar o tracker niveau. Charges utiles contenant `{ event, path, locale, release, ts }`
  sem métadonnées de référent ou IP, et `navigator.sendBeacon` et utilisées toujours que possible
  pour éviter de bloquer les navigateurs.
- Contrôle de l'échantillonnage avec `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). O traqueur armazena
  Le dernier chemin emprunté et n'émet jamais d'événements dupliqués pour votre navigation.
- Une implémentation fica em `src/components/AnalyticsTracker.jsx` et montée
  globalement via `src/theme/Root.js`.

## Sondes synthétiques- `npm run probe:portal` dispara demande GET contra rotas comuns
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) et vérifier
  La balise méta `sora-release` correspond à un `--expect-release` (ou `DOCS_RELEASE_TAG`).
  Exemple :

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Falhas sao reportadas por path, facilitando gatear o CD pelo successesso dos sondes.

## Automatisation des liens connectés

- `npm run check:links` varre `build/sitemap.xml`, garante que cada entrada mapeia para
  un fichier local (replis de remplacement `index.html`), et écrit
  `build/link-report.json` contiennent des métadonnées de version, totales, fausses et impressionnantes
  SHA-256 de `checksums.sha256` (exposé comme `manifest.id`) pour chaque relation possible
  Il est lié au manifeste de l'artefato.
- Le script se termine avec un code nao-zéro lorsque vous ne parvenez pas à une page et que CI peut bloquer
  libère em rotas antigas ou quebradas. Os relatorios citam os caminhos candidatsos tentados,
  o qui ajuda a rastrear régressoes de roteamento de volta para avore de docs.

## Dashboard Grafana et alertes- `dashboards/grafana/docs_portal.json` carte publique Grafana **Publication du portail Docs**.
  Il comprend les pains suivants :
  - *Refus de passerelle (5 m)* États-Unis `torii_sorafs_gateway_refusals_total` avec escopo
    `profile`/`reason` pour que les SRE détectent des ruines politiques ou des pertes de jetons.
  - *Alias Cache Refresh Outcomes* et *Alias Proof Age p90* acompanham
    `torii_sorafs_alias_cache_*` pour prouver que des preuves récentes existent avant la coupe
    sur le DNS.
  - *Nombre de manifestes du registre des broches* et statistiques *Nombre d'alias actifs* affichés
    arriéré du registre des broches et du total des alias pour que la gouvernance puisse être auditée
    version cada.
  - *Expiration de la passerelle TLS (heures)* lorsque le certificat TLS est effectué sur la passerelle de publication
    se rapproche du vencimento (limiar de alerta em 72 h).
  - *Résultats SLA de réplication* et *Replication Backlog* accompagnent la télémétrie
    `torii_sorafs_replication_*` pour garantir que toutes les répliques atendam o
    Patamar GA a déposé une publication.
- Utiliser comme variable de modèle embutidas (`profile`, `reason`) pour se concentrer sur aucun profil
  de publier `docs.sora` ou d'étudier les picos dans toutes les passerelles.
- Le routage de PagerDuty utilise les éléments du tableau de bord comme preuve : alertes
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry`
  Disparam quando a serie correspondant ultrapassa seus limiares. Ligue ou runbookalerter cette page pour que l'astreinte consiga se répète en tant que requêtes exatas sur Prometheus.

## Juntando tout

1. Durante `npm run build`, définie comme variables ambiantes de release/analytics et
   deixe o pos-build émetteur `checksums.sha256`, `release.json` e
   `link-report.json`.
2. Rode `npm run probe:portal` contre le nom d'hôte de l'aperçu avec
   `--expect-release` connecté à la balise mésmo. Salve la sortie standard pour une liste de contrôle de publication.
3. Rode `npm run check:links` pour accéder rapidement aux informations sur le plan du site et l'archiver
   Le rapport JSON est ajouté aux éléments d'aperçu. Un dépôt CI o
   Dernier rapport sur `artifacts/docs_portal/link-report.json` pour la gouvernance
   j'utilise le bundle de preuves directement vers les journaux de build.
4. Trouver le point final d'analyse pour votre gestionnaire de préservation de la vie privée (Plausible,
   OTEL ingère auto-hébergé, etc.) et garantit que les taxes d'hébergement sont documentées par
   release pour que les tableaux de bord interprètent les volumes correctement.
5. Un CI est connecté à ces étapes de nos workflows de prévisualisation/déploiement
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), entrez les essais à sec localement avec précision
   prendre un comportement spécifique de secret.