---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/observability.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Portail d'analyse et d'analyse

La carte DOCS-SORA effectue des analyses, des sondes synthétiques et des automatismes
проверки битых ссылок для каждого version préliminaire. В этой заметке описана инфраструктура,
À l'intérieur du portail, les opérateurs peuvent surveiller la surveillance
без утечки данных посетителей.

## Тегирование релиза

- Installez `DOCS_RELEASE_TAG=<identifier>` (repli sur `GIT_COMMIT` ou `dev`) sur le portail.
  Les informations fournies dans `<meta name="sora-release">` permettent d'afficher les sondes et les tableaux de bord.
  развертывания.
- `npm run build` создает `build/release.json` (записывается `scripts/write-checksums.mjs`), где описаны
  tel, horodatage et optionnel `DOCS_RELEASE_SOURCE`. Этот файл упаковывается в preview-артефакты
  et vous pouvez supprimer le vérificateur de liens.

## Аналитика с сохранением приватности

- Настройте `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` для включения легкого трекера.
  Пейлоады содержат `{ event, path, locale, release, ts }` без referrer или IP метаданных, и при
  возможности используется `navigator.sendBeacon`, чтобы не блокировать навигации.
- Управляйте выборкой через `DOCS_ANALYTICS_SAMPLE_RATE` (0-1). Трекер хранит последний отправленный chemin
  и никогда не отправляет дубликаты событий для одной навигации.
- La réalisation est effectuée dans `src/components/AnalyticsTracker.jsx` et globalement à partir de `src/theme/Root.js`.

## Синтетические пробы- `npm run probe:portal` retarde GET-запросы на типичные маршруты (`/`, `/norito/overview`,
  `/reference/torii-swagger`, et t.d.) et prouve que la méta `sora-release` est la solution
  `--expect-release` (ou `DOCS_RELEASE_TAG`). Exemple :

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Si vous recherchez un chemin, vous devez ouvrir le CD de Gate à votre recherche.

## Véhicules automatiques

- `npm run check:links` recherche `build/sitemap.xml` pour savoir ce qui peut être téléchargé sur la carte locale.
  (Prise de secours `index.html`), et `build/link-report.json`, la réponse métadonnée est ici,
  Les connecteurs et l'opérateur SHA-256 `checksums.sha256` (appelés `manifest.id`) peuvent être trouvés
  было связать с манифестом артефакта.
- Le script est ajouté au nouveau code, alors que la page d'accueil est utilisée, le CI peut bloquer les réponses
  при устаревших или сломанных маршрутах. Les autres candidats sont ceux qui doivent s'ouvrir, c'est ça
  помогает проследить регрессию маршрутизации до дерева docs.

## Dashbord Grafana et alertes- `dashboards/grafana/docs_portal.json` publie le dossier Grafana **Publication du portail Docs**.
  Sur les panneaux suivants :
  - *Refus de passerelle (5 m)* Utilisez `torii_sorafs_gateway_refusals_total` en temps réel
    `profile`/`reason`, le SRE peut prendre en charge la poussée politique ou les jetons.
  - *Alias Cache Refresh Outcomes* et *Alias Proof Age p90* отслеживают `torii_sorafs_alias_cache_*`,
    Vous devez fournir une preuve complète avant le basculement DNS.
  - *Nombre de manifestes du registre des broches* et état *Nombre d'alias actifs* élimine l'arriéré du registre des broches et s'en occupe.
    число alias, чтобы gouvernance могла аудировать каждый реLIS.
  - *Expiration de la passerelle TLS (heures)* permet de créer une passerelle de publication de certificats TLS
    (alerte 72 h).
  - *Résultats SLA de réplication* et *Replication Backlog* следят за телеметрией
    `torii_sorafs_replication_*`, vous devez déterminer quelles sont les réponses proposées par GA après la publication.
- Utilisez les modèles prédéfinis (`profile`, `reason`), pour vous concentrer sur
  profil de publication `docs.sora` ou partagez des informations sur chaque étudiant.
- Le routage PagerDuty utilise les panneaux du bord pour la documentation : alertes
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache` et `DocsPortal/TLSExpiry`,
  Cette série est actuellement disponible pour vous. Activez l'alerte Runbook à cet endroit,
  Les agents de garde peuvent utiliser le Prometheus.

## Сводим вместе1. Lors de la mise à jour `npm run build`, installez la version finale/l'analyse et les données post-build
   Téléchargez `checksums.sha256`, `release.json` et `link-report.json`.
2. Sélectionnez `npm run probe:portal` pour prévisualiser le nom d'hôte avec `--expect-release`, en vous connectant à votre sujet.
   Connectez la sortie standard à la liste de publication.
3. Cliquez sur `npm run check:links` pour télécharger le plan du site et l'archiver.
   Le générateur JSON ouvre la fenêtre des éléments d'aperçu. CI cladет последний отчет в
   `artifacts/docs_portal/link-report.json`, la gouvernance peut télécharger un ensemble de preuves pour la création de logos.
4. Connectez le point de terminaison d'analyse à votre collecteur préservant la confidentialité (ingestion OTEL plausible et auto-hébergée et ainsi de suite)
   et déterminez quelle fréquence d'échantillonnage est documentée pour chaque version, quels tableaux de bord sont correctement interprétés
   счетчики.
5. CI propose ces workflows de prévisualisation/déploiement
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), les courses à sec locales doivent être effectuées
   только поведение, связанное с секретами.